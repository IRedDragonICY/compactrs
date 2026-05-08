/* --- src/engine/worker.rs --- */
use std::sync::{Arc, Mutex, atomic::{AtomicU8, AtomicU64, Ordering}};
use std::sync::mpsc::{Sender, sync_channel, Receiver};
use crate::types::*;

use crate::utils::to_wstring;
use crate::ui::state::{UiMessage, BatchAction, ProcessingState};
use crate::engine::wof::{uncompress_file, WofAlgorithm, get_real_file_size, smart_compress, detect_compression_state, CompressionState};

// Correctly import form scanner
use crate::engine::scanner::{
    is_critical_path, should_skip_extension, 
    detect_path_algorithm
};

// Re-export scanner functions so UI code doesn't break
pub use crate::engine::scanner::{scan_path_metrics, scan_path_streaming};

// ===== EXECUTION STATE GUARD =====

/// RAII guard that prevents system sleep during processing.
struct ExecutionStateGuard;

impl ExecutionStateGuard {
    fn new() -> Self {
        unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED); }
        Self
    }
}

impl Drop for ExecutionStateGuard {
    fn drop(&mut self) {
        unsafe { SetThreadExecutionState(ES_CONTINUOUS); }
    }
}

// ===== STRUCTS =====

#[derive(Debug, Clone)]
pub enum ProcessResult {
    Success,
    Skipped(Vec<u16>),
    Failed(Vec<u16>),
}

struct FileTask {
    path: String,
    action: BatchAction,
    item_id: u32,
    algorithm: WofAlgorithm,
}

struct SharedReceiver<T> {
    rx: Mutex<Receiver<T>>,
}

impl<T> SharedReceiver<T> {
    fn new(rx: Receiver<T>) -> Self {
        Self { rx: Mutex::new(rx) }
    }
    
    fn recv(&self) -> Option<T> {
        self.rx.lock().ok()?.recv().ok()
    }
}

// ===== PROCESSING LOGIC =====

/// Orchestrates batch processing with producer-consumer threading model.
pub fn batch_process_worker(
    items: Vec<(String, BatchAction, u32, WofAlgorithm)>, 
    tx: Sender<UiMessage>, 
    state: Arc<AtomicU8>,
    force: bool,
    main_hwnd: usize,
    guard_enabled: bool,
    low_power_mode: bool,
    max_threads: u32,
    global_current: Arc<AtomicU64>,
    global_total: Arc<AtomicU64>,
    enable_skip: bool,
    skip_extensions: String,
    set_compressed_attr: bool,
) {
    let _sleep_guard = ExecutionStateGuard::new();
    let _ = tx.send(UiMessage::StatusText(to_wstring("Discovering files...")));
    
    // 1. Discovery Phase
    let mut item_totals = std::collections::HashMap::new();
    let mut item_paths = std::collections::HashMap::new();
    let mut total_files = 0u64;

    for (path, _, id, _) in &items {
        // Use scanner for discovery
        let count = if std::path::Path::new(path).is_file() {
            1
        } else {
            // Non-allocating fast scan for count only
            crate::engine::scanner::scan_path_metrics(path).file_count
        };
        
        item_totals.insert(*id, count);
        item_paths.insert(*id, path.clone());
        total_files += count;
        
        // Initial Item Progress (0/count)
        let _ = tx.send(UiMessage::RowProgress(*id, 0, count, 0));
    }
    
    global_total.fetch_add(total_files, Ordering::Relaxed);
    let _ = tx.send(UiMessage::Progress(global_current.load(Ordering::Relaxed), global_total.load(Ordering::Relaxed)));

    let parallelism = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let num_threads = if max_threads > 0 { max_threads as usize } 
                      else if low_power_mode { std::cmp::max(1, parallelism / 4) } 
                      else { parallelism };
    
    crate::log_info!(&["Processing ", &total_files.to_string(), " files with ", &num_threads.to_string(), " CPU Threads..."].concat());
    


    // 2. Execution Phase
    let (file_tx, file_rx) = sync_channel::<FileTask>(1024);
    let shared_rx = Arc::new(SharedReceiver::new(file_rx));
    
    let success = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    
    // Use DashMap or concurrent Maps in real scenario, but since we know maximum possible id during run
    // we can't easily pre-allocate a perfect flat array if IDs are sparse. 
    // We'll use a simple Mutexed HashMap for cross-thread counters since item count is small vs file count.
    
    // Initialize the maps for the items we are processing
    let mut m1 = std::collections::HashMap::new();
    let mut m2 = std::collections::HashMap::new();
    for (_, _, id, _) in &items {
         m1.insert(*id, Arc::new(AtomicU64::new(0)));
         m2.insert(*id, Arc::new(AtomicU64::new(0)));
    }
    let item_processed_counts = Arc::new(m1);
    let item_disk_sizes = Arc::new(m2);

    let item_totals = Arc::new(item_totals);
    let item_paths = Arc::new(item_paths);

    // Producer Thread
    let state_producer = Arc::clone(&state);
    let items_producer = items.clone();
    let producer_handle = std::thread::spawn(move || {
        for (path, action, id, algo) in items_producer {
            if check_stop_signal(&state_producer) { break; }
            
            // Determine directory attribute logic
            let enable_attr = set_compressed_attr && action == BatchAction::Compress;
            let disable_attr = action == BatchAction::Decompress;

            if std::path::Path::new(&path).is_file() {
                let _ = file_tx.send(FileTask { path, action, item_id: id, algorithm: algo });
            } else {
                // IT IS A DIRECTORY
                crate::log_info!(&["Processing dir: ", &path, " AttrEnable: ", &enable_attr.to_string()].concat());
                
                // It is a directory: Apply to root first
                if enable_attr {
                    crate::engine::wof::set_compressed_attribute(&path, true);
                } else if disable_attr {
                    crate::engine::wof::set_compressed_attribute(&path, false);
                }

                // Stream files and apply recursive attributes
                crate::engine::scanner::walk_directory(&path, Some(&state_producer), &mut |full_path, is_dir, _| {
                    if is_dir {
                        if enable_attr {
                            crate::engine::wof::set_compressed_attribute(full_path, true);
                        } else if disable_attr {
                            crate::engine::wof::set_compressed_attribute(full_path, false);
                        }
                    } else {
                        // File found: send to worker
                        let _ = file_tx.send(FileTask { path: full_path.to_string(), action, item_id: id, algorithm: algo });
                    }
                });
            }
        }
        // Channel closes on drop
    });

    // Consumer Threads
    std::thread::scope(|s| {
        for _ in 0..num_threads {
            let rx = Arc::clone(&shared_rx);
            let g_cur = Arc::clone(&global_current);
            let g_tot = Arc::clone(&global_total);
            let success = Arc::clone(&success);
            let failed = Arc::clone(&failed);
            let row_proc = Arc::clone(&item_processed_counts);
            let row_size = Arc::clone(&item_disk_sizes);
            let row_tot = Arc::clone(&item_totals);
            let row_p = Arc::clone(&item_paths);
            let tx = tx.clone();
            let st = Arc::clone(&state);
            let force = force;
            let hwnd = main_hwnd;
            let guard = guard_enabled;
            let skip_en = enable_skip;
            let skip_ext = skip_extensions.clone();
            let set_attr = set_compressed_attr;

            s.spawn(move || {
                crate::engine::wof::enable_backup_privileges();
                if low_power_mode { crate::engine::power::enable_eco_mode(); }

                while let Some(task) = rx.recv() {
                    wait_if_paused(&st);
                    if st.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 { break; }

                    let (res, size) = process_file_core(
                        &task.path, task.algorithm, task.action, force, hwnd, guard, skip_en, &skip_ext, set_attr
                    );

                    match res {
                        ProcessResult::Success | ProcessResult::Skipped(_) => { success.fetch_add(1, Ordering::Relaxed); }
                        ProcessResult::Failed(_) => { failed.fetch_add(1, Ordering::Relaxed); }
                    }

                    // Global Progress
                    let cur = g_cur.fetch_add(1, Ordering::Relaxed) + 1;
                    let tot = g_tot.load(Ordering::Relaxed);
                    
                    if cur % 20 == 0 || cur >= tot {
                         let _ = tx.send(UiMessage::Progress(cur, tot));
                    }

                    // Item Progress
                    if let Some(counter) = row_proc.get(&task.item_id) {
                        let r_cur = counter.fetch_add(1, Ordering::Relaxed) + 1;
                        let r_tot = *row_tot.get(&task.item_id).unwrap_or(&1);
                        
                        if let Some(sz) = row_size.get(&task.item_id) {
                            sz.fetch_add(size, Ordering::Relaxed);
                        }

                         if r_cur % 5 == 0 || r_cur == r_tot {
                              // Current processed bytes
                              let current_bytes = row_size.get(&task.item_id).map(|a| a.load(Ordering::Relaxed)).unwrap_or(0);
                              
                              if r_cur == r_tot {
                                  // Row Finished
                                  let algo_st = if let Some(p) = row_p.get(&task.item_id) {
                                      detect_path_algorithm(p)
                                  } else {
                                      crate::engine::wof::CompressionState::None
                                  };
                                  
                                  let _ = tx.send(UiMessage::RowFinished(task.item_id, current_bytes, r_tot, algo_st));
                              } else {
                                  let _ = tx.send(UiMessage::RowProgress(task.item_id, r_cur, r_tot, current_bytes));
                              }
                         }
                    }
                }
            });
        }
    });

    let _ = producer_handle.join();

    // Explicitly finish empty rows (consumers never ran for them)
    for (id, count) in item_totals.iter() {
        if *count == 0 {
             let algo_st = if let Some(p) = item_paths.get(id) {
                  detect_path_algorithm(p)
             } else {
                  crate::engine::wof::CompressionState::None
             };
             let _ = tx.send(UiMessage::RowFinished(*id, 0, 0, algo_st));
        }
    }

    if state.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
        let _ = tx.send(UiMessage::StatusText(to_wstring("Cancelled.")));
        let _ = tx.send(UiMessage::Finished);
        return;
    }

    // Final global sync
    let g_cur = global_current.load(Ordering::Relaxed);
    let g_tot = global_total.load(Ordering::Relaxed);
    let _ = tx.send(UiMessage::Progress(g_cur, g_tot));
    if g_cur >= g_tot {
        let _ = tx.send(UiMessage::Finished);
    }
}

/// Core processing logic for a single file.
fn process_file_core(
    path: &str,
    algo: WofAlgorithm,
    action: BatchAction,
    force: bool,
    main_hwnd: usize,
    guard_enabled: bool,
    enable_skip: bool,
    skip_ext_list: &str,
    _set_compressed_attr: bool,
) -> (ProcessResult, u64) {
    match action {
        BatchAction::Compress => {
            // Heuristics checks
            if guard_enabled && !force && is_critical_path(path) {
                crate::log_info!(&["Skipped (Critical): ", path].concat());
                return (ProcessResult::Skipped(crate::utils::to_wstring("System Path")), get_real_file_size(path));
            }
            if !force {
                 if let Some(curr) = crate::engine::wof::get_wof_algorithm(path) {
                     if curr == algo {

                         crate::log_info!(&["Skipped (Optimal): ", path].concat());
                         return (ProcessResult::Skipped(crate::utils::to_wstring("Already optimal")), get_real_file_size(path));
                     }
                 }
                     if should_skip_extension(path, enable_skip, skip_ext_list) {
                         crate::log_info!(&["Skipped (Ext): ", path].concat());
                         return (ProcessResult::Skipped(crate::utils::to_wstring("Filtered extension")), get_real_file_size(path));
                     }
            }

            // Attempt Compression
            match try_compress_with_lock_handling(path, algo, force, main_hwnd) {
                Ok(true) => {
                    let disk_size = get_real_file_size(path);
                    let logical_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                    
                    // Check if space was actually saved
                    if logical_size > 0 && disk_size < logical_size {
                        crate::log_trace!(&["Compressed: ", path].concat());
                        (ProcessResult::Success, disk_size)
                    } else if force {
                        // Force mode: Check if file is actually compressed (WOF or LZNT1)
                        let state = detect_compression_state(path);
                        match state {
                            CompressionState::Specific(_) => {
                                crate::log_trace!(&["Compressed (forced, no savings): ", path].concat());
                                (ProcessResult::Success, disk_size)
                            },
                            _ => {
                                crate::log_info!(&["Skipped (No savings): ", path].concat());
                                (ProcessResult::Skipped(crate::utils::to_wstring("No savings")), disk_size)
                            }
                        }
                    } else {
                        crate::log_info!(&["Skipped (No savings): ", path].concat());
                        (ProcessResult::Skipped(crate::utils::to_wstring("No savings")), disk_size)
                    }
                },
                Ok(false) => {
                    crate::log_info!(&["Skipped (Not beneficial): ", path].concat());
                    (ProcessResult::Skipped(crate::utils::to_wstring("Not beneficial")), get_real_file_size(path))
                },
                Err(e) => {
                    crate::log_error!(&["Failed ", path, ": ", &e].concat());
                    (ProcessResult::Failed(crate::utils::to_wstring(&e)), get_real_file_size(path))
                }
            }
        },
        BatchAction::Decompress => {
            match uncompress_file(path) {
                Ok(_) => {
                    crate::log_trace!(&["Decompressed: ", path].concat());
                    (ProcessResult::Success, get_real_file_size(path))
                },
                Err(e) => {
                    let prefix = crate::w!("Error ");
                    let err_msg = e.to_string();
                    let err_w = crate::utils::to_wstring(&err_msg);
                    let msg = crate::utils::concat_wstrings(&[prefix, &err_w]);
                    crate::log_error!(&["Failed ", path, ": ", &err_msg].concat());
                    (ProcessResult::Failed(msg), get_real_file_size(path))
                }
            }
        }
    }
}

fn try_compress_with_lock_handling(
    path: &str, 
    algo: WofAlgorithm, 
    force: bool, 
    main_hwnd: usize
) -> Result<bool, String> {
    match smart_compress(path, algo, force) {
        Ok(res) => Ok(res),
        Err(e) => {
             // 32 = ERROR_SHARING_VIOLATION
             if force && e == 32 {
                 if let Ok(blockers) = std::panic::catch_unwind(|| crate::engine::process::get_file_blockers(path)) {
                     if !blockers.is_empty() {
                         let name_w = to_wstring(&blockers[0].name);
                         let res = unsafe { SendMessageW(main_hwnd as HWND, 0x8004, name_w.as_ptr() as usize, 0) };
                         
                         if res == 1 {
                             for b in blockers { let _ = crate::engine::process::kill_process(b.pid); }
                             std::thread::sleep(std::time::Duration::from_millis(100));
                             return smart_compress(path, algo, force).map_err(|e2| e2.to_string());
                         }
                     }
                 }
             }
             Err(e.to_string())
        }
    }
}

// ===== HELPERS =====

fn check_stop_signal(state: &Arc<AtomicU8>) -> bool {
    state.load(Ordering::Relaxed) == ProcessingState::Stopped as u8
}

fn wait_if_paused(state: &Arc<AtomicU8>) {
    while state.load(Ordering::Relaxed) == ProcessingState::Paused as u8 {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}