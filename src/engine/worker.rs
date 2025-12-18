use std::sync::{Arc, Mutex, atomic::{AtomicU8, AtomicU64, Ordering}};
use std::sync::mpsc::{Sender, sync_channel, Receiver};
use crate::ui::utils::format_size;
use crate::ui::state::{UiMessage, BatchAction, ProcessingState};
use crate::engine::wof::{compress_file, uncompress_file, WofAlgorithm, get_real_file_size, get_wof_algorithm};
use crate::utils::{to_wstring, u64_to_wstring, concat_wstrings};
use windows_sys::Win32::Foundation::{HWND, INVALID_HANDLE_VALUE};
use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;
use windows_sys::Win32::Storage::FileSystem::{
    FindFirstFileExW, FindNextFileW, FindClose,
    FindExInfoBasic, FindExSearchNameMatch,
    FIND_FIRST_EX_LARGE_FETCH, WIN32_FIND_DATAW,
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
};
use windows_sys::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED};

// ===== SYSTEM CRITICAL PATH GUARD =====

fn is_critical_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    // Normalize path separators to backslashes for reliable checking if needed, 
    // or just check loose containment as specific heuristics.
    
    // Basic heuristics for Windows critical paths
    lower.contains("windows\\system32") || 
    lower.contains("windows\\syswow64") ||
    lower.contains("windows\\winsxs") ||
    lower.contains("boot") ||
    lower.ends_with("bootmgr")
}

// ===== EXECUTION STATE GUARD (RAII for Prevent Sleep) =====

/// RAII guard that prevents the system from sleeping during long-running operations.
/// Uses Win32 SetThreadExecutionState to keep the system awake.
/// Automatically resets to normal state when dropped (panic-safe).
struct ExecutionStateGuard {
    _private: (), // Zero-sized marker to prevent external construction
}

impl ExecutionStateGuard {
    /// Creates a new guard that prevents system sleep.
    /// Sets ES_CONTINUOUS | ES_SYSTEM_REQUIRED to keep the system awake.
    fn new() -> Self {
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
        }
        Self { _private: () }
    }
}

impl Drop for ExecutionStateGuard {
    /// Resets the execution state to ES_CONTINUOUS, allowing normal sleep behavior.
    fn drop(&mut self) {
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}

// ===== RESULT TYPE FOR CORE PROCESSING =====

/// Result of processing a single file
#[derive(Debug, Clone)]
pub enum ProcessResult {
    /// File was successfully processed (compressed/decompressed)
    Success,
    /// File was skipped (with reason)
    Skipped(String),
    /// Processing failed (with error message)
    Failed(String),
}

// ===== SINGLE-PASS SCAN STATISTICS =====

/// Aggregated statistics from a single-pass directory traversal.
/// Eliminates redundant I/O by gathering count, logical size, and optionally file paths
/// in one traversal instead of multiple passes.
#[derive(Default)]
pub struct ScanStats {
    /// Total number of files discovered
    pub file_count: u64,
    /// Total logical size (uncompressed) from WIN32_FIND_DATAW
    pub logical_size: u64,
    /// Optional collection of file paths (only populated if collect_paths=true)
    pub file_paths: Vec<String>,
}

// ===== SKIP HEURISTICS =====

/// Extensions that should be skipped during compression (already compressed or incompressible)
const SKIP_EXTENSIONS: &[&str] = &[
    "zip", "7z", "rar", "gz", "bz2", "xz", "zst", "lz4",  // Archives
    "jpg", "jpeg", "png", "gif", "webp", "avif", "heic",  // Images
    "mp4", "mkv", "avi", "webm", "mov", "wmv",            // Video
    "mp3", "flac", "aac", "ogg", "opus", "wma",           // Audio
    "pdf",                                                 // Documents
];

/// Check if a file should be skipped based on extension heuristics
fn should_skip_extension(path: &str) -> bool {
    let path_obj = std::path::Path::new(path);
    if let Some(ext) = path_obj.extension().and_then(|s| s.to_str()) {
        let ext_lower = ext.to_lowercase();
        SKIP_EXTENSIONS.iter().any(|&skip_ext| ext_lower == skip_ext)
    } else {
        false
    }
}

// ===== WIN32 NATIVE DIRECTORY TRAVERSAL =====

/// Generic walker that handles recursion, stop signals, and basic filtering.
/// The `visitor` closure receives: (Full Path, IsDirectory, Reference to FindData).
fn walk_directory_generic<F>(
    path: &str,
    state: Option<&Arc<AtomicU8>>,
    visitor: &mut F,
)
where
    F: FnMut(&str, bool, &WIN32_FIND_DATAW),
{
    // Check stop signal immediately
    if let Some(s) = state {
        if s.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
            return;
        }
    }

    let pattern = if path.ends_with('\\') || path.ends_with('/') {
        let mut p = path.to_string();
        p.push('*');
        p
    } else {
        let mut p = path.to_string();
        p.push_str("\\*");
        p
    };
    let pattern_wide = to_wstring(&pattern);

    // Default isn't implemented for WIN32_FIND_DATAW in windows-sys usually without feature, 
    // but zero-init is safe.
    let mut find_data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };

    unsafe {
        let handle = FindFirstFileExW(
            pattern_wide.as_ptr(),
            FindExInfoBasic,
            &mut find_data as *mut _ as *mut _,
            FindExSearchNameMatch,
            std::ptr::null(),
            FIND_FIRST_EX_LARGE_FETCH,
        );

        if handle == INVALID_HANDLE_VALUE {
            // Check if it's just "file not found" or actual error? 
            // In Rust `ignore` crate logic, we just return.
            return;
        }

        loop {
            // Check stop signal in loop
            if let Some(s) = state {
                if s.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
                    FindClose(handle);
                    return;
                }
            }

            let filename_len = find_data
                .cFileName
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(find_data.cFileName.len());
            let filename = String::from_utf16_lossy(&find_data.cFileName[..filename_len]);

            if filename != "." && filename != ".." {
                let full_path = if path.ends_with('\\') || path.ends_with('/') {
                    let mut p = path.to_string();
                    p.push_str(&filename);
                    p
                } else {
                    let mut p = path.to_string();
                    p.push('\\');
                    p.push_str(&filename);
                    p
                };

                let is_dir = (find_data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
                // CRITICAL: Check for Junctions/Symlinks to prevent infinite loops
                let is_reparse = (find_data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;

                // Invoke visitor
                visitor(&full_path, is_dir, &find_data);

                // Recurse into directories ONLY if they are real directories (not reparse points)
                if is_dir && !is_reparse {
                    walk_directory_generic(&full_path, state, visitor);
                }
            }

            if FindNextFileW(handle, &mut find_data) == 0 {
                break;
            }
        }

        FindClose(handle);
    }
}

/// Single-pass directory scanner that gathers all metadata in one traversal.
/// 
/// This eliminates redundant I/O by collecting count, logical size, and optionally
/// file paths in a single pass instead of multiple separate walks.
/// 
/// # Arguments
/// * `path` - Directory path to scan
/// * `collect_paths` - If true, collects file paths into the result (allocates Vec)
/// * `state` - Optional cancellation token for cooperative cancellation
/// 
/// # Returns
/// `ScanStats` containing file count, logical size, and optionally file paths.
/// Logical size is calculated directly from WIN32_FIND_DATAW to avoid extra fs::metadata calls.
fn scan_directory_optimized(
    path: &str,
    collect_paths: bool,
    state: Option<&Arc<AtomicU8>>,
) -> ScanStats {
    let mut stats = ScanStats::default();
    
    walk_directory_generic(path, state, &mut |full_path, is_dir, find_data| {
        if !is_dir {
            stats.file_count += 1;
            // Calculate logical size directly from WIN32_FIND_DATAW
            // Uses bitwise shift: (high << 32) | low for correct 64-bit size
            let size = ((find_data.nFileSizeHigh as u64) << 32) | (find_data.nFileSizeLow as u64);
            stats.logical_size += size;
            
            if collect_paths {
                stats.file_paths.push(full_path.to_string());
            }
        }
    });
    
    stats
}

/// Accumulates disk size of all files using Win32 traversal
fn walk_directory_win32_disk_size(path: &str, total: &mut u64) {
    walk_directory_generic(path, None, &mut |full_path, is_dir, _| {
        if !is_dir {
            *total += get_real_file_size(full_path);
        }
    });
}

/// Samples files to detect WOF algorithm using Win32 traversal
fn walk_directory_win32_detect_algo(
    path: &str,
    found_algos: &mut std::collections::HashSet<u32>,
    scanned: &mut usize,
    max_scan: usize,
) {
    walk_directory_generic(path, None, &mut |full_path, is_dir, _| {
        if *scanned >= max_scan {
            return;
        }
        if !is_dir {
            if let Some(algo) = get_wof_algorithm(full_path) {
                found_algos.insert(algo as u32);
            }
            *scanned += 1;
        }
    });
}

// ===== HELPER FUNCTIONS =====

/// Calculate total LOGICAL size of all files in a folder (uncompressed content size)
/// This counts ALL files including hidden and .gitignored files.
/// Uses single-pass traversal for efficiency.
pub fn calculate_folder_logical_size(path: &str) -> u64 {
    scan_directory_optimized(path, false, None).logical_size
}

/// Calculate total DISK size of all files in a folder (actual space used, respects compression)
/// Uses GetCompressedFileSizeW to get real disk usage for WOF-compressed files
pub fn calculate_folder_disk_size(path: &str) -> u64 {
    let mut total = 0u64;
    walk_directory_win32_disk_size(path, &mut total);
    total
}

/// Detect the predominant WOF algorithm used in a folder
/// Returns Mixed if multiple algorithms are found
pub fn detect_folder_algorithm(path: &str) -> crate::engine::wof::CompressionState {
    use crate::engine::wof::CompressionState;
    
    let mut found_algos = std::collections::HashSet::new();
    let mut scanned = 0usize;
    
    walk_directory_win32_detect_algo(path, &mut found_algos, &mut scanned, 50);
    
    if found_algos.is_empty() {
        return CompressionState::None;
    }
    
    if found_algos.len() > 1 {
        return CompressionState::Mixed;
    }
    
    // Only one algorithm found
    let algo_val = found_algos.into_iter().next().unwrap();
    match algo_val {
        0 => CompressionState::Specific(WofAlgorithm::Xpress4K),
        1 => CompressionState::Specific(WofAlgorithm::Lzx),
        2 => CompressionState::Specific(WofAlgorithm::Xpress8K),
        3 => CompressionState::Specific(WofAlgorithm::Xpress16K),
        _ => CompressionState::None,
    }
}

// ===== PATH-AWARE FUNCTIONS (work for both files and folders) =====

/// Calculate logical size for a path (file or folder)
pub fn calculate_path_logical_size(path: &str) -> u64 {
    let p = std::path::Path::new(path);
    if p.is_file() {
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    } else {
        calculate_folder_logical_size(path)
    }
}

/// Calculate disk size for a path (file or folder)
pub fn calculate_path_disk_size(path: &str) -> u64 {
    let p = std::path::Path::new(path);
    if p.is_file() {
        get_real_file_size(path)
    } else {
        calculate_folder_disk_size(path)
    }
}

/// Detect WOF algorithm for a path (file or folder)  
pub fn detect_path_algorithm(path: &str) -> crate::engine::wof::CompressionState {
    use crate::engine::wof::CompressionState;
    
    let p = std::path::Path::new(path);
    if p.is_file() {
        match get_wof_algorithm(path) {
            Some(algo) => CompressionState::Specific(algo),
            None => CompressionState::None,
        }
    } else {
        detect_folder_algorithm(path)
    }
}

// ===== WORKER FUNCTIONS =====

fn try_compress_with_lock_handling(
    path: &str, 
    algo: WofAlgorithm, 
    force: bool, 
    main_hwnd: usize
) -> Result<bool, String> {
    match compress_file(path, algo, force) {
        Ok(res) => Ok(res),
        Err(e) => {
             // Check if force is true AND it is a sharing violation (0x80070020 = -2147024864)
             // e is now u32 (raw OS error). ERROR_SHARING_VIOLATION is 32.
             if force && e == 32 { // 0x20
                 // Try to get blockers; catch unwind in case it panics
                 let blockers_res = std::panic::catch_unwind(|| {
                     crate::engine::process::get_file_blockers(path)
                 });
                 
                 if let Ok(blockers) = blockers_res {
                     if !blockers.is_empty() {
                         // Found a blocker. Ask Main Thread.
                         let name = &blockers[0].name;
                         let name_wide = to_wstring(name);
                         let hwnd = main_hwnd as HWND;
                         
                         // Synchronous call to Main UI
                         let res = unsafe { 
                             SendMessageW(hwnd, 0x8004, name_wide.as_ptr() as usize, 0)
                         };
                         
                         if res == 1 {
                             // Kill approved
                             for b in blockers {
                                 let _ = crate::engine::process::kill_process(b.pid);
                             }
                             // Slight delay to allow OS to release lock
                             std::thread::sleep(std::time::Duration::from_millis(100));
                             
                             // Retry Compression
                             return compress_file(path, algo, force)
                                .map_err(|e2| {
                                    // "Failed retry {}: {:?}"
                                    // Manual simple error string
                                    let mut s = "Failed retry ".to_string();
                                    s.push_str(path);
                                    s.push_str(": ");
                                    s.push_str(&e2.to_string()); // minimal dependency on Display for errors
                                    s
                                });
                         }
                     }
                 }
             }
             let mut s = "Failed ".to_string();
             s.push_str(path);
             s.push_str(": ");
             s.push_str(&e.to_string());
             Err(s)
        }
    }
}

/// Core function to process a single file (compress or decompress)
/// 
/// This encapsulates all the business logic:
/// - Extension filtering (skip heuristics)
/// - Force flag handling
/// - Dispatch to compress/decompress
/// - Lock handling for compression
/// 
/// # Arguments
/// * `path` - Path to the file to process
/// * `algo` - WOF algorithm for compression
/// * `action` - Whether to compress or decompress
/// * `force` - If true, bypass extension filtering
/// * `main_hwnd` - Main window handle for lock dialog
/// * `tx` - Channel for sending granular log messages
pub fn process_file_core(
    path: &str,
    algo: WofAlgorithm,
    action: BatchAction,
    force: bool,
    main_hwnd: usize,
    tx: &Sender<UiMessage>,
    guard_enabled: bool,
) -> ProcessResult {
    match action {
        BatchAction::Compress => {
            // Check System Critical Path Guard
            if guard_enabled && !force && is_critical_path(path) {
                let p = to_wstring(path);
                let msg = concat_wstrings(&[&to_wstring("Skipped (Critical System Path): "), &p]);
                let _ = tx.send(UiMessage::Log(msg));
                return ProcessResult::Skipped("System Path".to_string());
            }

            // NEW: Smart Skip - Prevent re-compression of already optimal files
            if !force {
                if let Some(current_algo) = crate::engine::wof::get_wof_algorithm(path) {
                    if current_algo == algo {
                        let p = to_wstring(path);
                        let msg = concat_wstrings(&[&to_wstring("Skipped (Already compressed): "), &p]);
                        let _ = tx.send(UiMessage::Log(msg));
                        return ProcessResult::Skipped("Already optimal".to_string());
                    }
                }
            }

            // Check extension filter (unless force is enabled)
            if !force && should_skip_extension(path) {
                let p = to_wstring(path);
                let msg = concat_wstrings(&[&to_wstring("Skipped (filtered): "), &p]);
                let _ = tx.send(UiMessage::Log(msg));
                return ProcessResult::Skipped("Filtered extension".to_string());
            }
            
            // Attempt compression with lock handling
            match try_compress_with_lock_handling(path, algo, force, main_hwnd) {
                Ok(true) => ProcessResult::Success,
                Ok(false) => {
                    // OS driver said compression not beneficial
                    let p = to_wstring(path);
                    let msg = concat_wstrings(&[&to_wstring("Skipped (OS: Not Beneficial): "), &p]);
                    let _ = tx.send(UiMessage::Log(msg));
                    ProcessResult::Skipped("Not beneficial".to_string())
                }
                Err(msg) => {
                    let _ = tx.send(UiMessage::Error(to_wstring(&msg)));
                    ProcessResult::Failed(msg)
                }
            }
        }
        BatchAction::Decompress => {
            match uncompress_file(path) {
                Ok(_) => ProcessResult::Success,
                Err(e) => {
                    let mut s = "Failed ".to_string();
                    s.push_str(path);
                    s.push_str(": ");
                    s.push_str(&e.to_string());
                    let w = to_wstring(&s);
                    let _ = tx.send(UiMessage::Error(w));
                    ProcessResult::Failed(s)
                }
            }
        }
    }
}

/// Task passed from producer to consumer threads
struct FileTask {
    path: String,
    action: BatchAction,
    row_idx: usize,
}

/// Wrapper for Receiver to enable sharing between threads via Mutex
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

pub fn batch_process_worker(
    items: Vec<(String, BatchAction, usize)>, 
    algo: WofAlgorithm, 
    tx: Sender<UiMessage>, 
    state: Arc<AtomicU8>,
    force: bool,
    main_hwnd: usize,
    guard_enabled: bool,
) {
    // RAII guard: Prevent system sleep for the duration of batch processing.
    // Automatically resets on drop (panic-safe).
    let _sleep_guard = ExecutionStateGuard::new();
    
    let _ = tx.send(UiMessage::Status(to_wstring("Discovering files...")));
    
    // Track total files per row (row_index -> count)
    let mut row_totals: std::collections::HashMap<usize, u64> = std::collections::HashMap::new();
    
    // Single-pass discovery: count files using optimized scanner
    let mut total_files = 0u64;
    for (path, _, row) in &items {
        let row_count = if std::path::Path::new(path).is_file() {
            1u64
        } else {
            // Single-pass scan: gets count (and logical_size is available if needed later)
            scan_directory_optimized(path, false, None).file_count
        };
        
        row_totals.insert(*row, row_count);
        total_files += row_count;
        
        // Initialize row progress
        let row_cnt_w = u64_to_wstring(row_count);
        let prog_str = concat_wstrings(&[&to_wstring("0/"), &row_cnt_w]);
        let _ = tx.send(UiMessage::RowUpdate(*row as i32, prog_str, to_wstring("Running"), vec![0;1])); // Empty vec for size
    }
    
    let _ = tx.send(UiMessage::Progress(0, total_files));
    let num_threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    
    let total_w = u64_to_wstring(total_files);
    let threads_w = u64_to_wstring(num_threads as u64);
    let msg = concat_wstrings(&[
        &to_wstring("Processing "), &total_w, &to_wstring(" files with "), &threads_w, &to_wstring(" threads...")
    ]);
    let _ = tx.send(UiMessage::Status(msg));
    
    if total_files == 0 {
        let _ = tx.send(UiMessage::Status(to_wstring("No files found to process.")));
        let _ = tx.send(UiMessage::Finished);
        return;
    }

    // Create bounded channel for streaming (backpressure at 1024 items)
    let (file_tx, file_rx) = sync_channel::<FileTask>(1024);
    let shared_rx = Arc::new(SharedReceiver::new(file_rx));
    
    // Counters
    let processed = Arc::new(AtomicU64::new(0));
    let success = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    
    // Per-row processed counters
    let max_row = items.iter().map(|(_, _, r)| *r).max().unwrap_or(0);
    let row_processed_counts: Arc<Vec<AtomicU64>> = Arc::new((0..=max_row).map(|_| AtomicU64::new(0)).collect());
    let row_totals = Arc::new(row_totals);
    
    // Producer thread: walks directories and sends tasks
    let state_producer = Arc::clone(&state);
    let items_for_producer = items.clone();
    let producer_handle = std::thread::spawn(move || {
        for (path, action, row) in items_for_producer {
            // Check if stopped
            if state_producer.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
                break;
            }
            
            if std::path::Path::new(&path).is_file() {
                // Single file
                let _ = file_tx.send(FileTask { path, action, row_idx: row });
            } else {
                // Directory - collect files using single-pass optimized scanner
                let stats = scan_directory_optimized(&path, true, Some(&state_producer));
                
                for file_path in stats.file_paths {
                    if state_producer.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
                        break;
                    }
                    let _ = file_tx.send(FileTask { 
                        path: file_path, 
                        action, 
                        row_idx: row 
                    });
                }
            }
        }
        // Drop sender to signal end of stream
        drop(file_tx);
    });
    
    // Consumer threads: process tasks from channel
    std::thread::scope(|s| {
        for _ in 0..num_threads {
            let shared_rx_clone = Arc::clone(&shared_rx);
            let processed_ref = Arc::clone(&processed);
            let success_ref = Arc::clone(&success);
            let failed_ref = Arc::clone(&failed);
            let row_counts_ref = Arc::clone(&row_processed_counts);
            let row_totals_ref = Arc::clone(&row_totals);
            let tx_clone = tx.clone();
            let state_ref = Arc::clone(&state);
            let algo_copy = algo;
            let force_copy = force;
            let hwnd_val = main_hwnd;
            let guard_enabled_copy = guard_enabled;
            let total_files_copy = total_files;

            s.spawn(move || {
                // Consume tasks from channel
                while let Some(task) = shared_rx_clone.recv() {
                    // Handle pause: busy-wait while paused
                    while state_ref.load(Ordering::Relaxed) == ProcessingState::Paused as u8 {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    
                    // Check if stopped after potential pause
                    if state_ref.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
                        break;
                    }

                    // Use the core processing function
                    let result = process_file_core(
                        &task.path, 
                        algo_copy, 
                        task.action, 
                        force_copy, 
                        hwnd_val, 
                        &tx_clone,
                        guard_enabled_copy
                    );
                    
                    // Update counters based on result
                    match result {
                        ProcessResult::Success | ProcessResult::Skipped(_) => {
                            success_ref.fetch_add(1, Ordering::Relaxed);
                        }
                        ProcessResult::Failed(_) => {
                            failed_ref.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    
                    // Global progress
                    let current_global = processed_ref.fetch_add(1, Ordering::Relaxed) + 1;
                    
                    // Row progress
                    if let Some(counter) = row_counts_ref.get(task.row_idx) {
                        let current_row = counter.fetch_add(1, Ordering::Relaxed) + 1;
                        let total_row = *row_totals_ref.get(&task.row_idx).unwrap_or(&1);
                        
                        // Update row UI (throttled)
                        if current_row % 5 == 0 || current_row == total_row {
                             let cur_w = u64_to_wstring(current_row);
                             let tot_w = u64_to_wstring(total_row);
                             let prog_w = concat_wstrings(&[&cur_w, &to_wstring("/"), &tot_w]);
                             let _ = tx_clone.send(UiMessage::RowUpdate(task.row_idx as i32, prog_w, to_wstring("Running"), vec![0;1]));
                        }
                    }
                    
                    // Throttled Global updates
                    if current_global % 20 == 0 || current_global == total_files_copy {
                         let _ = tx_clone.send(UiMessage::Progress(current_global, total_files_copy));
                         let cur_w = u64_to_wstring(current_global);
                         let tot_w = u64_to_wstring(total_files_copy);
                         let stat_w = concat_wstrings(&[&to_wstring("Processed "), &cur_w, &to_wstring("/"), &tot_w, &to_wstring(" files...")]);
                         let _ = tx_clone.send(UiMessage::Status(stat_w));
                    }
                }
            });
        }
    });
    
    // Wait for producer to finish
    let _ = producer_handle.join();
    
    if state.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
        let _ = tx.send(UiMessage::Status(to_wstring("Batch processing cancelled.")));
         let _ = tx.send(UiMessage::Finished);
        return;
    }

    // Final Report & Cleanup
    // Update all rows to "Done" and calculate sizes
    for (path, _, row_idx) in items {
        let size_after = if std::path::Path::new(&path).is_file() {
            calculate_path_disk_size(&path)
        } else {
            calculate_folder_disk_size(&path)
        };
        let size_w = format_size(size_after);
        
        let end_state = detect_path_algorithm(&path);
        let _ = tx.send(UiMessage::ItemFinished(row_idx as i32, to_wstring("Done"), size_w, end_state));
    }

    let s = success.load(Ordering::Relaxed);
    let f = failed.load(Ordering::Relaxed);
    let p = processed.load(Ordering::Relaxed);
    
    // "Batch complete! Processed: {} files | Success: {} | Failed: {}"
    let p_w = u64_to_wstring(p);
    let s_w = u64_to_wstring(s);
    let f_w = u64_to_wstring(f);
    let report_w = concat_wstrings(&[
        &to_wstring("Batch complete! Processed: "), &p_w, 
        &to_wstring(" files | Success: "), &s_w, 
        &to_wstring(" | Failed: "), &f_w
    ]);
    
    let _ = tx.send(UiMessage::Log(report_w.clone()));
    let _ = tx.send(UiMessage::Status(report_w));
    let _ = tx.send(UiMessage::Progress(total_files, total_files));
    let _ = tx.send(UiMessage::Finished);
}

/// Worker to process a single file or folder with its own algorithm setting
pub fn single_item_worker(
    path: String, 
    algo: WofAlgorithm, 
    action: BatchAction, 
    row: i32, 
    tx: Sender<UiMessage>, 
    state: Arc<AtomicU8>,
    force: bool,
    main_hwnd: usize,
    guard_enabled: bool,
) {

    let mut success = 0u64;
    let mut failed = 0u64;
    
    let is_single_file = std::path::Path::new(&path).is_file();
    
    // Count files first using single-pass scanner
    let mut total_files = if is_single_file {
        1
    } else {
        scan_directory_optimized(&path, false, None).file_count
    };
    
    let _ = tx.send(UiMessage::Progress(0, total_files));
    let action_str = match action {
        BatchAction::Compress => "Compressing",
        BatchAction::Decompress => "Decompressing",
    };
    let tot_w = u64_to_wstring(total_files);
    let path_w = to_wstring(&path);
    // "{} {} ({} files)..."
    let stat_w = concat_wstrings(&[
        &to_wstring(action_str), &to_wstring(" "), &path_w, &to_wstring(" ("), &tot_w, &to_wstring(" files)...")
    ]);
    let _ = tx.send(UiMessage::Status(stat_w));
    
    // Send initial row update
    let start_prog = concat_wstrings(&[&to_wstring("0/"), &tot_w]);
    let _ = tx.send(UiMessage::RowUpdate(row, start_prog, to_wstring("Running"), vec![0;1]));
    
    if is_single_file {
        // Handle pause for single file
        while state.load(Ordering::Relaxed) == ProcessingState::Paused as u8 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        
        // Check if stopped
        if state.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
            let _ = tx.send(UiMessage::ItemFinished(row, to_wstring("Cancelled"), vec![0;1], crate::engine::wof::CompressionState::None));
            let _ = tx.send(UiMessage::Status(to_wstring("Cancelled.")));
            let _ = tx.send(UiMessage::Finished);
            return;
        }

        // Use the core processing function for single file
        let result = process_file_core(&path, algo, action, force, main_hwnd, &tx, guard_enabled);
        
        // Handle result and update UI accordingly
        match result {
            ProcessResult::Success => {
                success += 1;
                let compressed_size = get_real_file_size(&path);
                let disk_w = format_size(compressed_size);
                let final_state = detect_path_algorithm(&path);
                let _ = tx.send(UiMessage::ItemFinished(row, to_wstring("Done"), disk_w, final_state));
            }
            ProcessResult::Skipped(_) => {
                success += 1;
                let final_state = detect_path_algorithm(&path);
                let _ = tx.send(UiMessage::ItemFinished(row, to_wstring("Skipped"), vec![0;1], final_state));
            }
            ProcessResult::Failed(_) => {
                failed += 1;
                let _ = tx.send(UiMessage::ItemFinished(row, to_wstring("Failed"), vec![0;1], crate::engine::wof::CompressionState::None));
            }
        }

        let _ = tx.send(UiMessage::RowUpdate(row, to_wstring("1/1"), to_wstring("Running"), vec![0;1]));
        let _ = tx.send(UiMessage::Progress(1, 1));
    } else {
        // Process folder using streaming producer-consumer model
        let num_threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        
        // Create bounded channel for streaming
        let (file_tx, file_rx) = sync_channel::<String>(1024);
        let shared_rx = Arc::new(SharedReceiver::new(file_rx));
        
        // Counters
        let processed = Arc::new(AtomicU64::new(0));
        let success_atomic = Arc::new(AtomicU64::new(0));
        let failed_atomic = Arc::new(AtomicU64::new(0));
        
        // Collect files using single-pass optimized scanner
        let stats = scan_directory_optimized(&path, true, Some(&state));
        let files = stats.file_paths;
        total_files = files.len() as u64;
        
        // Producer thread
        let state_producer = Arc::clone(&state);
        let producer_handle = std::thread::spawn(move || {
            for file_path in files {
                if state_producer.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
                    break;
                }
                let _ = file_tx.send(file_path);
            }
            drop(file_tx);
        });
        
        // Consumer threads
        std::thread::scope(|s| {
            for _ in 0..num_threads {
                let shared_rx_clone = Arc::clone(&shared_rx);
                let processed_ref = Arc::clone(&processed);
                let success_ref = Arc::clone(&success_atomic);
                let failed_ref = Arc::clone(&failed_atomic);
                let state_ref = Arc::clone(&state);
                let algo_copy = algo;
                let action_copy = action;
                let tx_clone = tx.clone();
                let row_copy = row;
                let force_copy = force;
                let hwnd_val = main_hwnd;
                let guard_enabled_copy = guard_enabled;
                let total_files_copy = total_files;

                s.spawn(move || {
                    while let Some(file_path) = shared_rx_clone.recv() {
                        // Handle pause: busy-wait while paused
                        while state_ref.load(Ordering::Relaxed) == ProcessingState::Paused as u8 {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                        
                        // Check if stopped after potential pause
                        if state_ref.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
                            break;
                        }
                        
                        // Use the core processing function
                        let result = process_file_core(
                            &file_path, 
                            algo_copy, 
                            action_copy, 
                            force_copy, 
                            hwnd_val,
                            &tx_clone,
                            guard_enabled_copy,
                        );
                        
                        // Update counters based on result
                        match result {
                            ProcessResult::Success | ProcessResult::Skipped(_) => {
                                success_ref.fetch_add(1, Ordering::Relaxed);
                            }
                            ProcessResult::Failed(_) => {
                                failed_ref.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                         
                        let current = processed_ref.fetch_add(1, Ordering::Relaxed) + 1;
                        
                        // Throttled updates
                        if current % 100 == 0 || current == total_files_copy {
                             // format!("{}/{}", current, total)
                             let cur_w = u64_to_wstring(current);
                             let tot_w = u64_to_wstring(total_files_copy);
                             let prog_w = concat_wstrings(&[&cur_w, &to_wstring("/"), &tot_w]);
                             let _ = tx_clone.send(UiMessage::RowUpdate(row_copy, prog_w, to_wstring("Running"), vec![0;1])); 
                        }
                        let _ = tx_clone.send(UiMessage::Progress(current, total_files_copy));
                    }
                });
            }
        });
        
        // Wait for producer
        let _ = producer_handle.join();
        
        if state.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
            let _ = tx.send(UiMessage::ItemFinished(row, to_wstring("Cancelled"), vec![0;1], crate::engine::wof::CompressionState::None));
            let _ = tx.send(UiMessage::Status(to_wstring("Cancelled.")));
            let _ = tx.send(UiMessage::Finished);
            return;
        }
        
        // Sync back to local variables for the final report
        success = success_atomic.load(Ordering::Relaxed);
        failed = failed_atomic.load(Ordering::Relaxed);
    }
    
    // Calculate size after
    let size_after = calculate_folder_disk_size(&path);
    let size_after_str = format_size(size_after);
    
    // Send final status with disk size for On Disk column
    // Send final status with disk size for On Disk column
    // let status = if failed > 0 { format!("Done+{} err", failed) } else { "Done".to_string() };
    let status_w = if failed > 0 {
        let f_w = u64_to_wstring(failed);
        concat_wstrings(&[&to_wstring("Done+"), &f_w, &to_wstring(" err")])
    } else {
        to_wstring("Done")
    };
    
    let final_state = detect_path_algorithm(&path);
    let _ = tx.send(UiMessage::ItemFinished(row, status_w, size_after_str, final_state));
    
    // format!("Done! {} files | Success: {} | Failed: {}", ...)
    let t_w = u64_to_wstring(total_files);
    let s_w = u64_to_wstring(success);
    let f_w = u64_to_wstring(failed);
    let report_w = concat_wstrings(&[
        &to_wstring("Done! "), &t_w, 
        &to_wstring(" files | Success: "), &s_w, 
        &to_wstring(" | Failed: "), &f_w
    ]);
    
    let _ = tx.send(UiMessage::Status(report_w));
    let _ = tx.send(UiMessage::Progress(total_files, total_files));
    let _ = tx.send(UiMessage::Finished);
}