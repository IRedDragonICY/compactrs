use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering}};
use std::sync::mpsc::Sender;
use ignore::WalkBuilder;
use humansize::{format_size, BINARY};
use crate::gui::state::{UiMessage, BatchAction};
use crate::engine::wof::{compress_file, uncompress_file, WofAlgorithm, get_real_file_size, get_wof_algorithm};
use windows::Win32::Foundation::{HWND, WPARAM, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::SendMessageW;

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

// ===== HELPER FUNCTIONS (Moved from window.rs) =====

fn create_walk_builder(path: &str) -> WalkBuilder {
    let mut builder = WalkBuilder::new(path);
    builder
        .hidden(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .ignore(false);
    builder
}

/// Calculate total LOGICAL size of all files in a folder (uncompressed content size)
/// This counts ALL files including hidden and .gitignored files
pub fn calculate_folder_logical_size(path: &str) -> u64 {
    create_walk_builder(path)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .filter_map(|e| std::fs::metadata(e.path()).ok())
        .map(|m| m.len())
        .sum()
}

/// Calculate total DISK size of all files in a folder (actual space used, respects compression)
/// Uses GetCompressedFileSizeW to get real disk usage for WOF-compressed files
pub fn calculate_folder_disk_size(path: &str) -> u64 {
    create_walk_builder(path)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .map(|e| get_real_file_size(&e.path().to_string_lossy()))
        .sum()
}

/// Check if a folder has any WOF-compressed files
/// Returns true if disk size < logical size (meaning compression is active)
pub fn is_folder_compressed(logical_size: u64, disk_size: u64) -> bool {
    // If disk size is noticeably smaller, folder has compressed files
    // Use a small threshold to account for rounding
    disk_size < logical_size && (logical_size - disk_size) > 1024
}

/// Detect the predominant WOF algorithm used in a folder
/// Samples up to 10 files to determine the algorithm
pub fn detect_folder_algorithm(path: &str) -> Option<WofAlgorithm> {
    let mut algo_counts = [0u32; 4]; // Xpress4K, Lzx, Xpress8K, Xpress16K
    let mut sampled = 0;
    
    for result in create_walk_builder(path)
        .build()
    {
        if sampled >= 20 { break; } // Sample enough files
        
        if let Ok(entry) = result {
            if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                let file_path = entry.path().to_string_lossy().to_string();
                if let Some(algo) = get_wof_algorithm(&file_path) {
                    algo_counts[algo as usize] += 1;
                    sampled += 1;
                }
            }
        }
    }
    
    // Find the most common algorithm
    let max_idx = algo_counts.iter().enumerate().max_by_key(|(_, v)| *v).map(|(i, _)| i);
    
    if let Some(idx) = max_idx {
        if algo_counts[idx] > 0 {
            return match idx {
                0 => Some(WofAlgorithm::Xpress4K),
                1 => Some(WofAlgorithm::Lzx),
                2 => Some(WofAlgorithm::Xpress8K),
                3 => Some(WofAlgorithm::Xpress16K),
                _ => None,
            };
        }
    }
    None
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
pub fn detect_path_algorithm(path: &str) -> Option<WofAlgorithm> {
    let p = std::path::Path::new(path);
    if p.is_file() {
        get_wof_algorithm(path)
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
                         let name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
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

pub fn batch_process_worker(
    items: Vec<(String, BatchAction, usize)>, 
    algo: WofAlgorithm, 
    tx: Sender<UiMessage>, 
    cancel: Arc<AtomicBool>,
    force: bool,
    main_hwnd: usize // Passed as usize to avoid Send/Sync issues if any (HWND is usually fine though)
) {
    // 1. Discovery Phase
    let _ = tx.send(UiMessage::Status("Discovering files...".to_string()));
    
    // Store tasks as (path, action, row_index)
    let mut tasks: Vec<(String, BatchAction, usize)> = Vec::new();
    // Track total files per row (row_index -> count)
    let mut row_totals: std::collections::HashMap<usize, u64> = std::collections::HashMap::new();
    
    for (path, action, row) in &items {
        if cancel.load(Ordering::Relaxed) { break; }
        
        let mut row_count = 0;
        
        // If it's a file, just add it
        if std::path::Path::new(path).is_file() {
            tasks.push((path.clone(), *action, *row));
            row_count = 1;
        } else {
            // If it's a directory, walk it
            // FIX: Ensure we do NOT ignore any files (hidden, gitignore, etc.)
            for result in create_walk_builder(path)
                .build() 
            {
                if let Ok(entry) = result {
                    if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                        tasks.push((entry.path().to_string_lossy().to_string(), *action, *row));
                        row_count += 1;
                    }
                }
            }
        }
        
        row_totals.insert(*row, row_count);
        // Initialize row progress
        let _ = tx.send(UiMessage::RowUpdate(*row as i32, format!("0/{}", row_count), "Running".to_string(), "".to_string()));
    }
    
    let total_files = tasks.len() as u64;
    let _ = tx.send(UiMessage::Progress(0, total_files));
    let num_threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let _ = tx.send(UiMessage::Status(format!("Processing {} files with {} threads...", total_files, num_threads)));
    
    if total_files == 0 {
        let _ = tx.send(UiMessage::Status("No files found to process.".to_string()));
        let _ = tx.send(UiMessage::Finished);
        return;
    }

    // 2. Parallel Processing Phase
    let processed = AtomicU64::new(0);
    let success = AtomicU64::new(0);
    let failed = AtomicU64::new(0);
    
    // Per-row processed counters.
    let max_row = items.iter().map(|(_, _, r)| *r).max().unwrap_or(0);
    let row_processed_counts: Vec<AtomicU64> = (0..=max_row).map(|_| AtomicU64::new(0)).collect();
    
    // Process in parallel using std::thread::scope with Dynamic Load Balancing (Atomic Cursor)
    let next_idx = AtomicUsize::new(0);
    let tasks_len = tasks.len();

    std::thread::scope(|s| {
        for _ in 0..num_threads {
            let processed_ref = &processed;
            let success_ref = &success;
            let failed_ref = &failed;
            let row_counts_ref = &row_processed_counts;
            let row_totals_ref = &row_totals;
            let tx_clone = tx.clone();
            let cancel_ref = &cancel;
            let algo_copy = algo;
            let next_idx_ref = &next_idx;
            let tasks_ref = &tasks; // Reference to the full vector
            let force_copy = force; // Pass force to the thread
            let hwnd_val = main_hwnd; // Capture HWND value

            s.spawn(move || {
                // let hwnd = HWND(hwnd_val as *mut _); // HWND handled in helper
                loop {
                    // Claim the next task index
                    let i = next_idx_ref.fetch_add(1, Ordering::Relaxed);
                    if i >= tasks_len {
                        break; // No more tasks
                    }
                    
                    if cancel_ref.load(Ordering::Relaxed) {
                         break; 
                    }

                    let (file_path, action, row_idx) = &tasks_ref[i];
                    
                    // Use the core processing function
                    let result = process_file_core(
                        file_path, 
                        algo_copy, 
                        *action, 
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
                    if let Some(counter) = row_counts_ref.get(*row_idx) {
                        let current_row = counter.fetch_add(1, Ordering::Relaxed) + 1;
                        let total_row = *row_totals_ref.get(row_idx).unwrap_or(&1);
                        
                        // Update row UI (throttled)
                        if current_row % 5 == 0 || current_row == total_row {
                             let _ = tx_clone.send(UiMessage::RowUpdate(*row_idx as i32, format!("{}/{}", current_row, total_row), "Running".to_string(), "".to_string()));
                        }
                    }
                    
                    // Throttled Global updates
                    if current_global % 20 == 0 || current_global == tasks_len as u64 {
                         let _ = tx_clone.send(UiMessage::Progress(current_global, tasks_len as u64));
                         let _ = tx_clone.send(UiMessage::Status(format!("Processed {}/{} files...", current_global, tasks_len)));
                    }
                }
            });
        }
    });
    
    if cancel.load(Ordering::Relaxed) {
        let _ = tx.send(UiMessage::Status("Batch processing cancelled.".to_string()));
         let _ = tx.send(UiMessage::Finished);
        return;
    }

    // 3. Final Report & Cleanup
    // Update all rows to "Done" and calculate sizes
    for (path, _, row_idx) in items {
        let size_after = if std::path::Path::new(&path).is_file() {
            calculate_path_disk_size(&path)
        } else {
            calculate_folder_disk_size(&path)
        };
        let size_str = format_size(size_after, BINARY);
        
        // Check if there were failed items for this row?
        // We tracked global failures, but not per-row failures. 
        // For now just mark as Done.
        let _ = tx.send(UiMessage::ItemFinished(row_idx as i32, "Done".to_string(), size_str));
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
    cancel: Arc<AtomicBool>,
    force: bool,
    main_hwnd: usize
) {
    let mut total_files = 0u64;
    let mut processed = 0u64;
    let mut success = 0u64;
    let mut failed = 0u64;
    
    let is_single_file = std::path::Path::new(&path).is_file();
    
    // Count files first
    if is_single_file {
        total_files = 1;
    } else {
        // FIX: Ensure we do NOT ignore any files (hidden, gitignore, etc.)
        for result in create_walk_builder(&path)
            .build()
        {
            if let Ok(entry) = result {
                if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    total_files += 1;
                }
            }
        }
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
        // Process single file directly
        if cancel.load(Ordering::Relaxed) {
            let _ = tx.send(UiMessage::ItemFinished(row, "Cancelled".to_string(), "".to_string()));
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
                let disk_str = format_size(compressed_size, BINARY);
                let _ = tx.send(UiMessage::ItemFinished(row, "Done".to_string(), disk_str));
            }
            ProcessResult::Skipped(_) => {
                success += 1;
                let _ = tx.send(UiMessage::ItemFinished(row, "Skipped".to_string(), "".to_string()));
            }
            ProcessResult::Failed(_) => {
                failed += 1;
                let _ = tx.send(UiMessage::ItemFinished(row, "Failed".to_string(), "".to_string()));
            }
        }

        processed += 1;
        let _ = tx.send(UiMessage::RowUpdate(row, "1/1".to_string(), "Running".to_string(), "".to_string()));
        let _ = tx.send(UiMessage::Progress(1, 1));
    } else {
        // Process folder in PARALLEL
        let mut tasks: Vec<String> = Vec::new();
        // FIX: Ensure we do NOT ignore any files (hidden, gitignore, etc.)
        for result in create_walk_builder(&path)
            .build() 
        {
            if let Ok(entry) = result {
                if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                     tasks.push(entry.path().to_string_lossy().to_string());
                }
            }
        }
        
        total_files = tasks.len() as u64;
        
        let processed_atomic = AtomicU64::new(0);
        let success_atomic = AtomicU64::new(0);
        let failed_atomic = AtomicU64::new(0);
        
        // Process folder in PARALLEL with Dynamic Load Balancing
        let next_idx = AtomicUsize::new(0);
        let tasks_len = tasks.len();
        let num_threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);

        std::thread::scope(|s| {
            for _ in 0..num_threads {
                 let processed_ref = &processed_atomic;
                 let success_ref = &success_atomic;
                 let failed_ref = &failed_atomic;
                 let cancel_ref = &cancel;
                 let algo_copy = algo;
                 let action_copy = action;
                 let tx_clone = tx.clone();
                 let next_idx_ref = &next_idx;
                 let tasks_ref = &tasks;
                 let row_copy = row; // Capture row for use in closure
                 let force_copy = force;
                 let hwnd_val = main_hwnd; 

                 s.spawn(move || {
                     // let hwnd = HWND(hwnd_val as *mut _); // HWND handled in helper
                     loop {
                        let i = next_idx_ref.fetch_add(1, Ordering::Relaxed);
                        if i >= tasks_len {
                            break;
                        }
                        
                        if cancel_ref.load(Ordering::Relaxed) {
                             break;
                        }
                        
                        let file_path = &tasks_ref[i];
                        
                        // Use the core processing function
                        let result = process_file_core(
                            file_path, 
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
                        
                        // Check cancel every N items
                        if i % 100 == 0 {
                             if cancel_ref.load(Ordering::Relaxed) {
                                 break;
                             }
                             // Send status update
                             let progress_str = format!("{}/{}", i, tasks_len);
                             let _ = tx_clone.send(UiMessage::RowUpdate(row_copy, progress_str, "Running".to_string(), "".to_string())); 
                        }
                        let _ = tx_clone.send(UiMessage::Progress(current, tasks_len as u64));
                     }
                 });
            }
        });
        
        if cancel.load(Ordering::Relaxed) {
            let _ = tx.send(UiMessage::ItemFinished(row, "Cancelled".to_string(), "".to_string()));
            let _ = tx.send(UiMessage::Status("Cancelled.".to_string()));
            let _ = tx.send(UiMessage::Finished);
            return;
        }
        
        // Sync back to local variables for the final report
        processed += processed_atomic.load(Ordering::Relaxed);
        success = success_atomic.load(Ordering::Relaxed);
        failed = failed_atomic.load(Ordering::Relaxed);
    }
    
    // Calculate size after
    let size_after = calculate_folder_disk_size(&path);
    let size_after_str = format_size(size_after, BINARY);
    
    // Send final status with disk size for On Disk column
    let status = if failed > 0 { format!("Done+{} err", failed) } else { "Done".to_string() };
    let _ = tx.send(UiMessage::ItemFinished(row, status, size_after_str));
    
    let report = format!("Done! {} files | Success: {} | Failed: {}", 
        processed, success, failed);
    
    let _ = tx.send(UiMessage::Status(report));
    let _ = tx.send(UiMessage::Progress(total_files, total_files));
    let _ = tx.send(UiMessage::Finished);
}