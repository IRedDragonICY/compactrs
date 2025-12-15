use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering}};
use std::sync::mpsc::Sender;
use ignore::WalkBuilder;
use humansize::{format_size, BINARY};
use crate::gui::state::{UiMessage, BatchAction};
use crate::engine::wof::{compress_file, uncompress_file, WofAlgorithm, get_real_file_size, get_wof_algorithm};

// ===== HELPER FUNCTIONS (Moved from window.rs) =====

/// Calculate total LOGICAL size of all files in a folder (uncompressed content size)
/// This counts ALL files including hidden and .gitignored files
pub fn calculate_folder_logical_size(path: &str) -> u64 {
    WalkBuilder::new(path)
        .hidden(false)          // Include hidden files
        .git_ignore(false)      // Don't respect .gitignore
        .git_global(false)      // Don't respect global gitignore
        .git_exclude(false)     // Don't respect .git/info/exclude
        .ignore(false)          // Don't respect .ignore files
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
    WalkBuilder::new(path)
        .hidden(false)          // Include hidden files
        .git_ignore(false)      // Don't respect .gitignore
        .git_global(false)      // Don't respect global gitignore
        .git_exclude(false)     // Don't respect .git/info/exclude
        .ignore(false)          // Don't respect .ignore files
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
    
    for result in WalkBuilder::new(path)
        .hidden(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .ignore(false)
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

pub fn batch_process_worker(
    items: Vec<(String, BatchAction, usize)>, 
    algo: WofAlgorithm, 
    tx: Sender<UiMessage>, 
    cancel: Arc<AtomicBool>,
    force: bool
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
            for result in WalkBuilder::new(path)
                .hidden(false)
                .git_ignore(false)
                .git_global(false)
                .git_exclude(false)
                .ignore(false)
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

            s.spawn(move || {
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
                    
                    let result = match action {
                        BatchAction::Compress => {
                            // Check if compressible / beneficial, UNLESS forced
                            let path = std::path::Path::new(file_path);
                            // Simple extension check
                            let compressible = if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                                !matches!(ext.to_lowercase().as_str(), "zip" | "7z" | "rar" | "jpg" | "png" | "mp4" | "mkv" | "mp3") 
                            } else {
                                true
                            };

                            if !force_copy && !compressible {
                                // Not compressible extension or other rule
                                // Treat as success (skipped)
                                success_ref.fetch_add(1, Ordering::Relaxed);
                                // Log skipped
                                let _ = tx_clone.send(UiMessage::Log(format!("Skipped (filtered): {}", file_path)));
                                continue; // Skip to next file
                            }
                            
                            // If force is ON, we might try compressing even if extension is bad.
                            // But usually we just pass it to compress_file.
                            compress_file(file_path, algo_copy, force_copy)
                        },
                        BatchAction::Decompress => uncompress_file(file_path).map(|_| true),
                    };
                    
                    match result {
                        Ok(true) => {
                             success_ref.fetch_add(1, Ordering::Relaxed);
                        },
                        Ok(false) => {
                             // Driver said no (not beneficial)
                             success_ref.fetch_add(1, Ordering::Relaxed);
                             let _ = tx_clone.send(UiMessage::Log(format!("Skipped (OS: Not Beneficial): {}", file_path)));
                        },
                        Err(e) => {
                            failed_ref.fetch_add(1, Ordering::Relaxed);
                            let _ = tx_clone.send(UiMessage::Error(format!("Failed {}: {:?}", file_path, e)));
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
    force: bool
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
        for result in WalkBuilder::new(&path)
            .hidden(false)
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .ignore(false)
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

        let path_obj = std::path::Path::new(&path);
        let compressible = if let Some(ext) = path_obj.extension().and_then(|s| s.to_str()) {
            !matches!(ext.to_lowercase().as_str(), "zip" | "7z" | "rar" | "jpg" | "png" | "mp4" | "mkv" | "mp3") 
        } else {
            true
        };

        if action == BatchAction::Compress && !force && !compressible {
             let _ = tx.send(UiMessage::Log(format!("Skipped (filtered): {}", path)));
             let _ = tx.send(UiMessage::ItemFinished(row, "Skipped".to_string(), "".to_string()));
             let _ = tx.send(UiMessage::Status("Skipped.".to_string()));
             let _ = tx.send(UiMessage::Finished);
             return;
        }
        
        // If force is ON, we proceed to compress_file even if !compressible.
        
        let result = match action {
            BatchAction::Compress => compress_file(&path, algo, force),
            BatchAction::Decompress => uncompress_file(&path).map(|_| true),
        };
        
        match result {
            Ok(true) => {
                 success += 1;
                 let compressed_size = get_real_file_size(&path);
                 let disk_str = format_size(compressed_size, BINARY);
                 let _ = tx.send(UiMessage::ItemFinished(row, "Done".to_string(), disk_str));
            },
            Ok(false) => {
                 // Skipped (not beneficial)
                 success += 1;
                 let _ = tx.send(UiMessage::Log(format!("Skipped (OS: Not Beneficial): {}", path)));
                 let _ = tx.send(UiMessage::ItemFinished(row, "Skipped".to_string(), "".to_string()));
            },
            Err(e) => {
                failed += 1;
                let _ = tx.send(UiMessage::Error(format!("Failed {}: {}", path, e)));
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
        for result in WalkBuilder::new(&path)
            .hidden(false)
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .ignore(false)
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

                 s.spawn(move || {
                     loop {
                        let i = next_idx_ref.fetch_add(1, Ordering::Relaxed);
                        if i >= tasks_len {
                            break;
                        }
                        
                        if cancel_ref.load(Ordering::Relaxed) {
                             break;
                        }
                        
                        let file_path = &tasks_ref[i];
                        
                        let result = match action_copy {
                            BatchAction::Compress => {
                                // Check if compressible / beneficial, UNLESS forced
                                let path = std::path::Path::new(file_path);
                                // Simple extension check
                                let compressible = if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                                    !matches!(ext.to_lowercase().as_str(), "zip" | "7z" | "rar" | "jpg" | "png" | "mp4" | "mkv" | "mp3") 
                                } else {
                                    true
                                };

                                if !force_copy && !compressible {
                                    // Not compressible extension or other rule
                                    // Treat as success (skipped)
                                    success_ref.fetch_add(1, Ordering::Relaxed);
                                    // Log skipped
                                    let _ = tx_clone.send(UiMessage::Log(format!("Skipped (filtered): {}", file_path)));
                                    continue; // Skip to next file
                                }
                                
                                compress_file(file_path, algo_copy, force_copy)
                            },
                            BatchAction::Decompress => uncompress_file(file_path).map(|_| true),
                        };
                        
                        match result {
                            Ok(true) => {
                                 success_ref.fetch_add(1, Ordering::Relaxed);
                            },
                            Ok(false) => {
                                 // Skipped (not beneficial)
                                 success_ref.fetch_add(1, Ordering::Relaxed);
                                 let _ = tx_clone.send(UiMessage::Log(format!("Skipped (OS: Not Beneficial): {}", file_path)));
                            },
                            Err(e) => {
                                failed_ref.fetch_add(1, Ordering::Relaxed);
                                let _ = tx_clone.send(UiMessage::Error(format!("Failed {}: {:?}", file_path, e)));
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