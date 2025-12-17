use std::sync::{Arc, Mutex, atomic::{AtomicU8, AtomicU64, Ordering}};
use std::sync::mpsc::{Sender, sync_channel, Receiver};
use crate::ui::utils::format_size;
use crate::ui::state::{UiMessage, BatchAction, ProcessingState};
use crate::engine::wof::{compress_file, uncompress_file, WofAlgorithm, get_real_file_size, get_wof_algorithm};
use crate::ui::utils::ToWide;
use windows::Win32::Foundation::{HWND, WPARAM, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::SendMessageW;
use windows::Win32::Storage::FileSystem::{
    FindFirstFileExW, FindNextFileW, FindClose,
    FindExInfoBasic, FindExSearchNameMatch,
    FIND_FIRST_EX_LARGE_FETCH, WIN32_FIND_DATAW,
    FILE_ATTRIBUTE_DIRECTORY,
};
use windows::core::PCWSTR;

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
        format!("{}*", path)
    } else {
        format!("{}\\*", path)
    };
    let pattern_wide = pattern.to_wide();

    let mut find_data = WIN32_FIND_DATAW::default();

    unsafe {
        let handle = FindFirstFileExW(
            PCWSTR(pattern_wide.as_ptr()),
            FindExInfoBasic,
            &mut find_data as *mut _ as *mut _,
            FindExSearchNameMatch,
            None,
            FIND_FIRST_EX_LARGE_FETCH,
        );

        if handle.is_err() {
            return;
        }
        let handle = handle.unwrap();

        loop {
            // Check stop signal in loop
            if let Some(s) = state {
                if s.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
                    let _ = FindClose(handle);
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
                    format!("{}{}", path, filename)
                } else {
                    format!("{}\\{}", path, filename)
                };

                let is_dir = (find_data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0) != 0;

                // Invoke visitor
                visitor(&full_path, is_dir, &find_data);

                // Recurse into directories
                if is_dir {
                    walk_directory_generic(&full_path, state, visitor);
                }
            }

            if FindNextFileW(handle, &mut find_data).is_err() {
                break;
            }
        }

        let _ = FindClose(handle);
    }
}

/// Recursively walks a directory using Win32 FindFirstFileExW/FindNextFileW APIs.
/// Collects file paths into a vector.
fn walk_directory_win32_collect(path: &str, files: &mut Vec<String>, state: &Arc<AtomicU8>) {
    walk_directory_generic(path, Some(state), &mut |full_path, is_dir, _| {
        if !is_dir {
            files.push(full_path.to_string());
        }
    });
}

/// Counts files in a directory using Win32 traversal (for size calculations)
fn walk_directory_win32_count(path: &str, counter: &mut u64) {
    walk_directory_generic(path, None, &mut |_, is_dir, _| {
        if !is_dir {
            *counter += 1;
        }
    });
}

/// Accumulates logical size of all files using Win32 traversal
fn walk_directory_win32_logical_size(path: &str, total: &mut u64) {
    walk_directory_generic(path, None, &mut |_, is_dir, find_data| {
        if !is_dir {
            let size = ((find_data.nFileSizeHigh as u64) << 32) | (find_data.nFileSizeLow as u64);
            *total += size;
        }
    });
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
/// This counts ALL files including hidden and .gitignored files
pub fn calculate_folder_logical_size(path: &str) -> u64 {
    let mut total = 0u64;
    walk_directory_win32_logical_size(path, &mut total);
    total
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
             if force && e.code().0 == -2147024864 {
                 // Try to get blockers; catch unwind in case it panics
                 let blockers_res = std::panic::catch_unwind(|| {
                     crate::engine::process::get_file_blockers(path)
                 });
                 
                 if let Ok(blockers) = blockers_res {
                     if !blockers.is_empty() {
                         // Found a blocker. Ask Main Thread.
                         let name = &blockers[0].name;
                         let name_wide = name.to_wide();
                         let hwnd = HWND(main_hwnd as *mut _); // Cast usize back to HWND handle
                         
                         // Synchronous call to Main UI
                         let res = unsafe { 
                             SendMessageW(hwnd, 0x8004, Some(WPARAM(name_wide.as_ptr() as usize)), Some(LPARAM(0)))
                         };
                         
                         if res.0 == 1 {
                             // Kill approved
                             for b in blockers {
                                 let _ = crate::engine::process::kill_process(b.pid);
                             }
                             // Slight delay to allow OS to release lock
                             std::thread::sleep(std::time::Duration::from_millis(100));
                             
                             // Retry Compression
                             return compress_file(path, algo, force)
                                .map_err(|e2| format!("Failed retry {}: {:?}", path, e2));
                         }
                     }
                 }
             }
             Err(format!("Failed {}: {:?}", path, e))
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
) -> ProcessResult {
    match action {
        BatchAction::Compress => {
            // Check extension filter (unless force is enabled)
            if !force && should_skip_extension(path) {
                let _ = tx.send(UiMessage::Log(format!("Skipped (filtered): {}", path)));
                return ProcessResult::Skipped("Filtered extension".to_string());
            }
            
            // Attempt compression with lock handling
            match try_compress_with_lock_handling(path, algo, force, main_hwnd) {
                Ok(true) => ProcessResult::Success,
                Ok(false) => {
                    // OS driver said compression not beneficial
                    let _ = tx.send(UiMessage::Log(format!("Skipped (OS: Not Beneficial): {}", path)));
                    ProcessResult::Skipped("Not beneficial".to_string())
                }
                Err(msg) => {
                    let _ = tx.send(UiMessage::Error(msg.clone()));
                    ProcessResult::Failed(msg)
                }
            }
        }
        BatchAction::Decompress => {
            match uncompress_file(path) {
                Ok(_) => ProcessResult::Success,
                Err(e) => {
                    let msg = format!("Failed {}: {:?}", path, e);
                    let _ = tx.send(UiMessage::Error(msg.clone()));
                    ProcessResult::Failed(msg)
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
    main_hwnd: usize
) {
    let _ = tx.send(UiMessage::Status("Discovering files...".to_string()));
    
    // Track total files per row (row_index -> count)
    let mut row_totals: std::collections::HashMap<usize, u64> = std::collections::HashMap::new();
    
    // First pass: count files for progress tracking
    let mut total_files = 0u64;
    for (path, _, row) in &items {
        let mut row_count = 0u64;
        
        if std::path::Path::new(path).is_file() {
            row_count = 1;
        } else {
            walk_directory_win32_count(path, &mut row_count);
        }
        
        row_totals.insert(*row, row_count);
        total_files += row_count;
        
        // Initialize row progress
        let _ = tx.send(UiMessage::RowUpdate(*row as i32, format!("0/{}", row_count), "Running".to_string(), "".to_string()));
    }
    
    let _ = tx.send(UiMessage::Progress(0, total_files));
    let num_threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let _ = tx.send(UiMessage::Status(format!("Processing {} files with {} threads...", total_files, num_threads)));
    
    if total_files == 0 {
        let _ = tx.send(UiMessage::Status("No files found to process.".to_string()));
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
                // Directory - collect files then send
                let mut files = Vec::new();
                walk_directory_win32_collect(&path, &mut files, &state_producer);
                
                for file_path in files {
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
                        &tx_clone
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
                             let _ = tx_clone.send(UiMessage::RowUpdate(task.row_idx as i32, format!("{}/{}", current_row, total_row), "Running".to_string(), "".to_string()));
                        }
                    }
                    
                    // Throttled Global updates
                    if current_global % 20 == 0 || current_global == total_files_copy {
                         let _ = tx_clone.send(UiMessage::Progress(current_global, total_files_copy));
                         let _ = tx_clone.send(UiMessage::Status(format!("Processed {}/{} files...", current_global, total_files_copy)));
                    }
                }
            });
        }
    });
    
    // Wait for producer to finish
    let _ = producer_handle.join();
    
    if state.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
        let _ = tx.send(UiMessage::Status("Batch processing cancelled.".to_string()));
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
        let size_str = format_size(size_after);
        
        let end_state = detect_path_algorithm(&path);
        let _ = tx.send(UiMessage::ItemFinished(row_idx as i32, "Done".to_string(), size_str, end_state));
    }

    let s = success.load(Ordering::Relaxed);
    let f = failed.load(Ordering::Relaxed);
    let p = processed.load(Ordering::Relaxed);
    
    let report = format!("Batch complete! Processed: {} files | Success: {} | Failed: {}", p, s, f);
    
    let _ = tx.send(UiMessage::Log(report.clone()));
    let _ = tx.send(UiMessage::Status(report));
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
    main_hwnd: usize
) {
    let mut total_files = 0u64;
    let mut success = 0u64;
    let mut failed = 0u64;
    
    let is_single_file = std::path::Path::new(&path).is_file();
    
    // Count files first
    if is_single_file {
        total_files = 1;
    } else {
        walk_directory_win32_count(&path, &mut total_files);
    }
    
    let _ = tx.send(UiMessage::Progress(0, total_files));
    let action_str = match action {
        BatchAction::Compress => "Compressing",
        BatchAction::Decompress => "Decompressing",
    };
    let _ = tx.send(UiMessage::Status(format!("{} {} ({} files)...", action_str, path, total_files)));
    
    // Send initial row update
    let _ = tx.send(UiMessage::RowUpdate(row, format!("0/{}", total_files), "Running".to_string(), "".to_string()));
    
    if is_single_file {
        // Handle pause for single file
        while state.load(Ordering::Relaxed) == ProcessingState::Paused as u8 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        
        // Check if stopped
        if state.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
            let _ = tx.send(UiMessage::ItemFinished(row, "Cancelled".to_string(), "".to_string(), crate::engine::wof::CompressionState::None));
            let _ = tx.send(UiMessage::Status("Cancelled.".to_string()));
            let _ = tx.send(UiMessage::Finished);
            return;
        }

        // Use the core processing function for single file
        let result = process_file_core(&path, algo, action, force, main_hwnd, &tx);
        
        // Handle result and update UI accordingly
        match result {
            ProcessResult::Success => {
                success += 1;
                let compressed_size = get_real_file_size(&path);
                let disk_str = format_size(compressed_size);
                let final_state = detect_path_algorithm(&path);
                let _ = tx.send(UiMessage::ItemFinished(row, "Done".to_string(), disk_str, final_state));
            }
            ProcessResult::Skipped(_) => {
                success += 1;
                let final_state = detect_path_algorithm(&path);
                let _ = tx.send(UiMessage::ItemFinished(row, "Skipped".to_string(), "".to_string(), final_state));
            }
            ProcessResult::Failed(_) => {
                failed += 1;
                let _ = tx.send(UiMessage::ItemFinished(row, "Failed".to_string(), "".to_string(), crate::engine::wof::CompressionState::None));
            }
        }

        let _ = tx.send(UiMessage::RowUpdate(row, "1/1".to_string(), "Running".to_string(), "".to_string()));
        let _ = tx.send(UiMessage::Progress(1, 1));
    } else {
        // Process folder using streaming producer-consumer model
        let num_threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        
        // Create bounded channel for streaming
        let (file_tx, file_rx) = sync_channel::<String>(1024);
        let shared_rx = Arc::new(SharedReceiver::new(file_rx));
        
        let processed = Arc::new(AtomicU64::new(0));
        let success_atomic = Arc::new(AtomicU64::new(0));
        let failed_atomic = Arc::new(AtomicU64::new(0));
        
        // Collect files first (required because we need to count them)
        let mut files = Vec::new();
        walk_directory_win32_collect(&path, &mut files, &state);
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
                            &tx_clone
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
                             let progress_str = format!("{}/{}", current, total_files_copy);
                             let _ = tx_clone.send(UiMessage::RowUpdate(row_copy, progress_str, "Running".to_string(), "".to_string())); 
                        }
                        let _ = tx_clone.send(UiMessage::Progress(current, total_files_copy));
                    }
                });
            }
        });
        
        // Wait for producer
        let _ = producer_handle.join();
        
        if state.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
            let _ = tx.send(UiMessage::ItemFinished(row, "Cancelled".to_string(), "".to_string(), crate::engine::wof::CompressionState::None));
            let _ = tx.send(UiMessage::Status("Cancelled.".to_string()));
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
    let status = if failed > 0 { format!("Done+{} err", failed) } else { "Done".to_string() };
    let final_state = detect_path_algorithm(&path);
    let _ = tx.send(UiMessage::ItemFinished(row, status, size_after_str, final_state));
    
    let report = format!("Done! {} files | Success: {} | Failed: {}", 
        total_files, success, failed);
    
    let _ = tx.send(UiMessage::Status(report));
    let _ = tx.send(UiMessage::Progress(total_files, total_files));
    let _ = tx.send(UiMessage::Finished);
}