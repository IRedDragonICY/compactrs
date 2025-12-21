use std::sync::{Arc, Mutex, atomic::{AtomicU8, AtomicU64, Ordering}};
use std::sync::mpsc::{Sender, sync_channel, Receiver};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;
use windows_sys::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED};

use crate::utils::to_wstring;
use crate::ui::state::{UiMessage, BatchAction, ProcessingState};
use crate::engine::wof::{uncompress_file, WofAlgorithm, get_real_file_size, smart_compress};

// Correctly import form scanner
use crate::engine::scanner::{
    scan_directory_for_processing, is_critical_path, should_skip_extension, 
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
    Skipped(String),
    Failed(String),
}

struct FileTask {
    path: String,
    action: BatchAction,
    row_idx: usize,
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
    items: Vec<(String, BatchAction, usize, WofAlgorithm)>, 
    tx: Sender<UiMessage>, 
    state: Arc<AtomicU8>,
    force: bool,
    main_hwnd: usize,
    guard_enabled: bool,
    low_power_mode: bool,
    max_threads: u32,
    global_current: Arc<AtomicU64>,
    global_total: Arc<AtomicU64>,
) {
    let _sleep_guard = ExecutionStateGuard::new();
    let _ = tx.send(UiMessage::StatusText("Discovering files...".to_string()));
    
    // 1. Discovery Phase
    let mut row_totals = std::collections::HashMap::new();
    let mut row_paths = std::collections::HashMap::new();
    let mut total_files = 0u64;

    for (path, _, row, _) in &items {
        // Use scanner for discovery
        let count = if std::path::Path::new(path).is_file() {
            1
        } else {
            // Non-allocating fast scan for count only
            crate::engine::scanner::scan_path_metrics(path).file_count
        };
        
        row_totals.insert(*row, count);
        row_paths.insert(*row, path.clone());
        total_files += count;
        
        // Initial Row Progress (0/count)
        let _ = tx.send(UiMessage::RowProgress(*row as i32, 0, count, 0));
    }
    
    global_total.fetch_add(total_files, Ordering::Relaxed);
    let _ = tx.send(UiMessage::Progress(global_current.load(Ordering::Relaxed), global_total.load(Ordering::Relaxed)));

    let parallelism = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let num_threads = if max_threads > 0 { max_threads as usize } 
                      else if low_power_mode { std::cmp::max(1, parallelism / 4) } 
                      else { parallelism };
    
    crate::log_info!("Processing {} files with {} CPU Threads...", total_files, num_threads);
    
    if total_files == 0 {
        let _ = tx.send(UiMessage::StatusText("No files found.".to_string()));
        let _ = tx.send(UiMessage::Finished);
        return;
    }

    // 2. Execution Phase
    let (file_tx, file_rx) = sync_channel::<FileTask>(1024);
    let shared_rx = Arc::new(SharedReceiver::new(file_rx));
    
    let success = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    
    // Track row progress
    let max_row = items.iter().map(|(_, _, r, _)| *r).max().unwrap_or(0);
    let row_processed_counts = Arc::new((0..=max_row).map(|_| AtomicU64::new(0)).collect::<Vec<_>>());
    let row_disk_sizes = Arc::new((0..=max_row).map(|_| AtomicU64::new(0)).collect::<Vec<_>>());
    let row_totals = Arc::new(row_totals);
    let row_paths = Arc::new(row_paths);

    // Producer Thread
    let state_producer = Arc::clone(&state);
    let items_producer = items.clone();
    let producer_handle = std::thread::spawn(move || {
        for (path, action, row, algo) in items_producer {
            if check_stop_signal(&state_producer) { break; }
            
            if std::path::Path::new(&path).is_file() {
                let _ = file_tx.send(FileTask { path, action, row_idx: row, algorithm: algo });
            } else {
                // Collect files for processing
                let stats = scan_directory_for_processing(&path, Some(&state_producer));
                for file_path in stats.file_paths {
                    if check_stop_signal(&state_producer) { break; }
                    let _ = file_tx.send(FileTask { path: file_path, action, row_idx: row, algorithm: algo });
                }
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
            let row_proc = Arc::clone(&row_processed_counts);
            let row_size = Arc::clone(&row_disk_sizes);
            let row_tot = Arc::clone(&row_totals);
            let row_p = Arc::clone(&row_paths);
            let tx = tx.clone();
            let st = Arc::clone(&state);
            let force = force;
            let hwnd = main_hwnd;
            let guard = guard_enabled;

            s.spawn(move || {
                crate::engine::wof::enable_backup_privileges();
                if low_power_mode { crate::engine::power::enable_eco_mode(); }

                while let Some(task) = rx.recv() {
                    wait_if_paused(&st);
                    if st.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 { break; }

                    let (res, size) = process_file_core(
                        &task.path, task.algorithm, task.action, force, hwnd, guard
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
                         // Status update "Processed X/Y" is now handled by UI window.rs
                    }

                    // Row Progress
                    if let Some(counter) = row_proc.get(task.row_idx) {
                        let r_cur = counter.fetch_add(1, Ordering::Relaxed) + 1;
                        let r_tot = *row_tot.get(&task.row_idx).unwrap_or(&1);
                        
                        if let Some(sz) = row_size.get(task.row_idx) {
                            sz.fetch_add(size, Ordering::Relaxed);
                        }

                         if r_cur % 5 == 0 || r_cur == r_tot {
                              // Current processed bytes
                              let current_bytes = row_size.get(task.row_idx).map(|a| a.load(Ordering::Relaxed)).unwrap_or(0);
                              
                              if r_cur == r_tot {
                                  // Row Finished
                                  let algo_st = if let Some(p) = row_p.get(&task.row_idx) {
                                      detect_path_algorithm(p)
                                  } else {
                                      crate::engine::wof::CompressionState::None
                                  };
                                  
                                  // Final progress update implies finished
                                  let _ = tx.send(UiMessage::RowFinished(task.row_idx as i32, current_bytes, r_tot, algo_st));
                              } else {
                                  let _ = tx.send(UiMessage::RowProgress(task.row_idx as i32, r_cur, r_tot, current_bytes));
                              }
                         }
                    }
                }
            });
        }
    });

    let _ = producer_handle.join();

    if state.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
        let _ = tx.send(UiMessage::StatusText("Cancelled.".to_string()));
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
) -> (ProcessResult, u64) {
    match action {
        BatchAction::Compress => {
            // Heuristics checks
            if guard_enabled && !force && is_critical_path(path) {
                crate::log_info!("Skipped (Critical): {}", path);
                return (ProcessResult::Skipped("System Path".to_string()), get_real_file_size(path));
            }
            if !force {
                 if let Some(curr) = crate::engine::wof::get_wof_algorithm(path) {
                     if curr == algo {
                         crate::log_info!("Skipped (Optimal): {}", path);
                         return (ProcessResult::Skipped("Already optimal".to_string()), get_real_file_size(path));
                     }
                 }
                 if should_skip_extension(path) {
                     crate::log_info!("Skipped (Ext): {}", path);
                     return (ProcessResult::Skipped("Filtered extension".to_string()), get_real_file_size(path));
                 }
            }

            // Attempt Compression
            match try_compress_with_lock_handling(path, algo, force, main_hwnd) {
                Ok(true) => {
                    crate::log_trace!("Compressed: {}", path);
                    (ProcessResult::Success, get_real_file_size(path))
                },
                Ok(false) => {
                    crate::log_info!("Skipped (Not beneficial): {}", path);
                    (ProcessResult::Skipped("Not beneficial".to_string()), get_real_file_size(path))
                },
                Err(e) => {
                    crate::log_error!("Failed {}: {}", path, e);
                    (ProcessResult::Failed(e), get_real_file_size(path))
                }
            }
        },
        BatchAction::Decompress => {
            match uncompress_file(path) {
                Ok(_) => {
                    crate::log_trace!("Decompressed: {}", path);
                    (ProcessResult::Success, get_real_file_size(path))
                },
                Err(e) => {
                    let msg = format!("Error {}", e);
                    crate::log_error!("Failed {}: {}", path, msg);
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