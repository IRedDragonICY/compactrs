use std::sync::atomic::{AtomicU8, AtomicUsize, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Condvar};
use std::time::Duration;
use std::sync::mpsc::Sender;

use crate::types::*;
use crate::utils::PathBuffer;
use crate::engine::wof::{get_real_file_size, get_wof_algorithm, WofAlgorithm, CompressionState, detect_compression_state};
use crate::ui::state::{UiMessage, ProcessingState};

// ===== STRUCTS =====

/// Aggregated statistics from a single-pass directory traversal.
#[derive(Default, Debug, Clone)]
pub struct ScanStats {
    pub file_count: u64,
    pub logical_size: u64,
    pub disk_size: u64,
    pub file_paths: Vec<String>,
}

/// Metrics for a single path (file or folder summary).
#[derive(Debug, Clone)]
pub struct PathMetrics {
    pub logical_size: u64,
    pub disk_size: u64,
    pub compression_state: CompressionState,
    pub file_count: u64,
}

// ===== HEURISTICS =====

/// Check if a file path is considered a critical system path that should be protected.
pub fn is_critical_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.contains("windows\\system32") || 
    lower.contains("windows\\syswow64") ||
    lower.contains("windows\\winsxs") ||
    lower.contains("boot") ||
    lower.ends_with("bootmgr")
}

/// Check if a file should be skipped based on extension.
pub fn should_skip_extension(path: &str, enabled: bool, custom_list: &str) -> bool {
    if !enabled { return false; }
    
    let path_obj = std::path::Path::new(path);
    if let Some(ext) = path_obj.extension().and_then(|s| s.to_str()) {
        let ext_lower = ext.to_lowercase();
        custom_list.split(',')
            .map(|s| s.trim())
            .any(|skip_ext| skip_ext.eq_ignore_ascii_case(&ext_lower))
    } else {
        false
    }
}

// ===== SINGLE-THREADED WALKER (PRESERVED FOR FFI COMPATIBILITY) =====

/// Generic single-threaded directory walker.
/// Preserved exactly as requested to ensure 100% compatibility with existing `worker.rs` closures.
pub fn walk_directory<F>(
    path: &str,
    state: Option<&Arc<AtomicU8>>,
    visitor: &mut F,
)
where
    F: FnMut(&str, bool, &WIN32_FIND_DATAW),
{
    let mut buffer = PathBuffer::from(path);
    walk_recursive(&mut buffer, state, visitor);
}

fn walk_recursive<F>(
    buffer: &mut PathBuffer,
    state: Option<&Arc<AtomicU8>>,
    visitor: &mut F,
)
where
    F: FnMut(&str, bool, &WIN32_FIND_DATAW),
{
    if let Some(s) = state {
        loop {
            let current = s.load(Ordering::Relaxed);
            if current == ProcessingState::Stopped as u8 { return; }
            if current == ProcessingState::Paused as u8 {
                std::thread::sleep(Duration::from_millis(100));
            } else {
                break;
            }
        }
    }

    let original_len = buffer.len();
    buffer.push("*");
    
    let mut find_data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };

    unsafe {
        let handle = FindFirstFileExW(
            buffer.as_ptr(),
            FindExInfoBasic,
            &mut find_data as *mut _ as *mut _,
            FindExSearchNameMatch,
            std::ptr::null_mut(),
            FIND_FIRST_EX_LARGE_FETCH,
        );

        buffer.truncate(original_len);

        if handle == INVALID_HANDLE_VALUE {
            return;
        }

        loop {
            if let Some(s) = state {
                 if s.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
                     FindClose(handle);
                     return;
                 }
            }

            let filename_len = find_data.cFileName.iter().position(|&c| c == 0).unwrap_or(find_data.cFileName.len());
            let is_dot = filename_len == 1 && find_data.cFileName[0] == 46;
            let is_dot_dot = filename_len == 2 && find_data.cFileName[0] == 46 && find_data.cFileName[1] == 46;

            if !is_dot && !is_dot_dot {
                let len_before = buffer.len();
                buffer.push_u16_slice(&find_data.cFileName[..filename_len]);
                
                let is_dir = (find_data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
                let is_reparse = (find_data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
                
                let full_path_str = buffer.to_string_lossy();
                visitor(&full_path_str, is_dir, &find_data);

                if is_dir && !is_reparse {
                    walk_recursive(buffer, state, visitor);
                }
                
                buffer.truncate(len_before);
            }

            if FindNextFileW(handle, &mut find_data) == 0 {
                break;
            }
        }
        FindClose(handle);
    }
}

// ===== MULTI-THREADED NVMe WORK-STEALING SCANNER =====

struct ScanContext {
    queue: Mutex<Vec<String>>,
    cvar: Condvar,
    active_workers: AtomicUsize,
    
    total_files: AtomicU64,
    total_logical: AtomicU64,
    total_disk: AtomicU64,
    
    algo_scanned: AtomicUsize,
    seen_algos: Mutex<std::collections::HashSet<u32>>,
    
    app_state: Option<Arc<AtomicU8>>,
    
    collect_paths: bool,
    collected_paths: Mutex<Vec<String>>,
}

impl ScanContext {
    fn decrement_worker_and_notify(&self) {
        let active_now = self.active_workers.fetch_sub(1, Ordering::SeqCst) - 1;
        if active_now == 0 {
            // Last worker finished its job. Wake up everyone (aggregator & sleeping workers) to exit.
            let _q = self.queue.lock().unwrap();
            self.cvar.notify_all();
        }
    }
}

fn scan_worker_thread(ctx: Arc<ScanContext>) {
    loop {
        let mut q = ctx.queue.lock().unwrap();
        while q.is_empty() {
            if ctx.active_workers.load(Ordering::SeqCst) == 0 {
                return;
            }
            if let Some(st) = &ctx.app_state {
                if st.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
                    return;
                }
            }
            q = ctx.cvar.wait(q).unwrap();
        }
        
        // Re-check conditions after waking up
        if q.is_empty() && ctx.active_workers.load(Ordering::SeqCst) == 0 {
            return;
        }
        
        let dir = match q.pop() {
            Some(d) => d,
            None => {
                if ctx.active_workers.load(Ordering::SeqCst) == 0 { return; }
                continue;
            }
        };
        
        ctx.active_workers.fetch_add(1, Ordering::SeqCst);
        drop(q);
        
        // --- Process Popped Directory ---
        
        if let Some(s) = &ctx.app_state {
            let mut should_stop = false;
            loop {
                let st = s.load(Ordering::Relaxed);
                if st == ProcessingState::Stopped as u8 {
                    should_stop = true;
                    break;
                }
                if st == ProcessingState::Paused as u8 {
                    std::thread::sleep(Duration::from_millis(100));
                } else {
                    break;
                }
            }
            if should_stop {
                ctx.decrement_worker_and_notify();
                return;
            }
        }
        
        let mut buffer = PathBuffer::from(&dir);
        let original_len = buffer.len();
        buffer.push("*");
        
        let mut find_data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
        let handle = unsafe { FindFirstFileExW(
            buffer.as_ptr(),
            FindExInfoBasic,
            &mut find_data as *mut _ as *mut _,
            FindExSearchNameMatch,
            std::ptr::null_mut(),
            FIND_FIRST_EX_LARGE_FETCH,
        ) };
        
        buffer.truncate(original_len);
        
        if handle != INVALID_HANDLE_VALUE {
            loop {
                if let Some(s) = &ctx.app_state {
                    let st = s.load(Ordering::Relaxed);
                    if st == ProcessingState::Stopped as u8 {
                        unsafe { FindClose(handle); }
                        ctx.decrement_worker_and_notify();
                        return;
                    }
                    if st == ProcessingState::Paused as u8 {
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                }
                
                let filename_len = find_data.cFileName.iter().position(|&c| c == 0).unwrap_or(find_data.cFileName.len());
                let is_dot = filename_len == 1 && find_data.cFileName[0] == 46;
                let is_dot_dot = filename_len == 2 && find_data.cFileName[0] == 46 && find_data.cFileName[1] == 46;
                
                if !is_dot && !is_dot_dot {
                    let len_before = buffer.len();
                    buffer.push_u16_slice(&find_data.cFileName[..filename_len]);
                    let full_path_str = buffer.to_string_lossy();
                    
                    let is_dir = (find_data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
                    let is_reparse = (find_data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
                    
                    if is_dir {
                        if !is_reparse {
                            let mut queue_lock = ctx.queue.lock().unwrap();
                            queue_lock.push(full_path_str);
                            ctx.cvar.notify_one();
                        }
                    } else {
                        // File Processing
                        ctx.total_files.fetch_add(1, Ordering::Relaxed);
                        let logical_size = ((find_data.nFileSizeHigh as u64) << 32) | (find_data.nFileSizeLow as u64);
                        ctx.total_logical.fetch_add(logical_size, Ordering::Relaxed);
                        
                        let disk_size = get_real_file_size(&full_path_str);
                        ctx.total_disk.fetch_add(disk_size, Ordering::Relaxed);
                        
                        if ctx.collect_paths {
                            let mut cp = ctx.collected_paths.lock().unwrap();
                            cp.push(full_path_str.clone());
                        }
                        
                        // Heuristic Algorithm Sampling
                        let scanned = ctx.algo_scanned.load(Ordering::Relaxed);
                        let mut check_algo = false;
                        if scanned < 200 {
                            check_algo = true;
                        } else if scanned < 2000 {
                            let seen = ctx.seen_algos.lock().unwrap();
                            if seen.is_empty() {
                                check_algo = true;
                            }
                        }
                        
                        if check_algo {
                            if let Some(algo) = get_wof_algorithm(&full_path_str) {
                                let mut seen = ctx.seen_algos.lock().unwrap();
                                seen.insert(algo as u32);
                            }
                            ctx.algo_scanned.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    buffer.truncate(len_before);
                }
                
                if unsafe { FindNextFileW(handle, &mut find_data) } == 0 {
                    break;
                }
            }
            unsafe { FindClose(handle); }
        }
        
        ctx.decrement_worker_and_notify();
    }
}

/// Core runner for the multi-threaded Work-Stealing scanner.
fn run_multi_threaded_scan(
    path: &str,
    state: Option<&Arc<AtomicU8>>,
    collect_paths: bool,
    tx_info: Option<(u32, Sender<UiMessage>)>
) -> ScanContext {
    let ctx = Arc::new(ScanContext {
        queue: Mutex::new(vec![path.to_string()]),
        cvar: Condvar::new(),
        active_workers: AtomicUsize::new(0),
        total_files: AtomicU64::new(0),
        total_logical: AtomicU64::new(0),
        total_disk: AtomicU64::new(0),
        algo_scanned: AtomicUsize::new(0),
        seen_algos: Mutex::new(std::collections::HashSet::new()),
        app_state: state.cloned(),
        collect_paths,
        collected_paths: Mutex::new(Vec::new()),
    });
    
    // Spawn workers saturating NVMe and CPU
    let thread_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let mut handles = Vec::with_capacity(thread_count);
    
    for _ in 0..thread_count {
        let worker_ctx = Arc::clone(&ctx);
        handles.push(std::thread::spawn(move || scan_worker_thread(worker_ctx)));
    }
    
    // Main Thread Aggregator / Wait Loop
    let mut q = ctx.queue.lock().unwrap();
    while !q.is_empty() || ctx.active_workers.load(Ordering::SeqCst) > 0 {
        if let Some(st) = &ctx.app_state {
            if st.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
                break;
            }
        }
        
        if let Some((id, ref tx)) = tx_info {
            // Aggregator rate-limits UI updates to 100ms
            let (new_q, res) = ctx.cvar.wait_timeout(q, Duration::from_millis(100)).unwrap();
            q = new_q;
            
            if res.timed_out() {
                let files = ctx.total_files.load(Ordering::Relaxed);
                let logical = ctx.total_logical.load(Ordering::Relaxed);
                let disk = ctx.total_disk.load(Ordering::Relaxed);
                let _ = tx.send(UiMessage::ScanProgress(id, logical, disk, files));
            }
        } else {
            // Blocks until all work is definitively done
            q = ctx.cvar.wait(q).unwrap();
        }
    }
    
    // Force wake all workers to guarantee clean exit
    ctx.cvar.notify_all();
    drop(q);
    
    for h in handles {
        let _ = h.join();
    }
    
    Arc::try_unwrap(ctx).unwrap_or_else(|_| panic!("Failed to unwrap ScanContext"))
}

// ===== PUBLIC API =====

/// Get metrics for a path. Performs high-speed multi-threaded NVMe scan.
pub fn scan_path_metrics(path: &str) -> PathMetrics {
    let p = std::path::Path::new(path);
    
    if p.is_file() {
        let logical = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let disk = get_real_file_size(path);
        let state = detect_compression_state(path);
        return PathMetrics { logical_size: logical, disk_size: disk, compression_state: state, file_count: 1 };
    }
    
    let ctx = run_multi_threaded_scan(path, None, false, None);
    let algos = ctx.seen_algos.into_inner().unwrap();
    
    PathMetrics {
        logical_size: ctx.total_logical.into_inner(),
        disk_size: ctx.total_disk.into_inner(),
        compression_state: resolve_mixed_state(algos),
        file_count: ctx.total_files.into_inner(),
    }
}

/// Scan path and stream progress updates to UI every 100ms using multi-threading.
pub fn scan_path_streaming(
    id: u32,
    path: &str,
    tx: Sender<UiMessage>,
    state: Option<&Arc<AtomicU8>>,
) -> PathMetrics {
    let p = std::path::Path::new(path);

    if p.is_file() {
        let m = scan_path_metrics(path);
        let _ = tx.send(UiMessage::ScanProgress(id, m.logical_size, m.disk_size, 1));
        return m;
    }

    let ctx = run_multi_threaded_scan(path, state, false, Some((id, tx.clone())));
    
    let files = ctx.total_files.into_inner();
    let logical = ctx.total_logical.into_inner();
    let disk = ctx.total_disk.into_inner();
    let algos = ctx.seen_algos.into_inner().unwrap();
    
    // Final precision sync
    let _ = tx.send(UiMessage::ScanProgress(id, logical, disk, files));
    
    PathMetrics {
        logical_size: logical,
        disk_size: disk,
        compression_state: resolve_mixed_state(algos),
        file_count: files,
    }
}

/// Optimized scan that collects file paths into a `Vec<String>`.
pub fn scan_directory_for_processing(
    path: &str,
    state: Option<&Arc<AtomicU8>>,
) -> ScanStats {
    let ctx = run_multi_threaded_scan(path, state, true, None);
    
    ScanStats {
        file_count: ctx.total_files.into_inner(),
        logical_size: ctx.total_logical.into_inner(),
        disk_size: ctx.total_disk.into_inner(),
        file_paths: ctx.collected_paths.into_inner().unwrap(),
    }
}

// ===== UTILS =====

pub fn detect_path_algorithm(path: &str) -> CompressionState {
    scan_path_metrics(path).compression_state
}

pub fn calculate_path_disk_size(path: &str) -> u64 {
    if std::path::Path::new(path).is_file() {
        get_real_file_size(path)
    } else {
        let mut sum = 0;
        walk_directory(path, None, &mut |p, is_dir, _| {
            if !is_dir { sum += get_real_file_size(p); }
        });
        sum
    }
}

fn resolve_mixed_state(algos: std::collections::HashSet<u32>) -> CompressionState {
    if algos.is_empty() { return CompressionState::None; }
    if algos.len() > 1 { return CompressionState::Mixed; }
    
    match algos.into_iter().next().unwrap() {
        0 => CompressionState::Specific(WofAlgorithm::Xpress4K),
        1 => CompressionState::Specific(WofAlgorithm::Lzx),
        2 => CompressionState::Specific(WofAlgorithm::Xpress8K),
        3 => CompressionState::Specific(WofAlgorithm::Xpress16K),
        4 => CompressionState::Specific(WofAlgorithm::Lznt1),
        _ => CompressionState::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_skip_extension() {
        let list = "zip,7z,rar";
        
        // Enabled, match
        assert!(should_skip_extension("test.zip", true, list));
        assert!(should_skip_extension("TEST.ZIP", true, list));
        assert!(should_skip_extension("archive.7z", true, list));
        
        // Enabled, no match
        assert!(!should_skip_extension("test.txt", true, list));
        assert!(!should_skip_extension("image.jpg", true, list)); // Not in custom list
        
        // Disabled
        assert!(!should_skip_extension("test.zip", false, list));
    }
}