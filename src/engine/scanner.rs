use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Instant;
use std::sync::mpsc::Sender;

use windows_sys::Win32::Storage::FileSystem::{
    FindFirstFileExW, FindNextFileW, FindClose, FindExInfoBasic, FindExSearchNameMatch,
    FIND_FIRST_EX_LARGE_FETCH, WIN32_FIND_DATAW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
};
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;

use crate::utils::PathBuffer;
use crate::engine::wof::{get_real_file_size, get_wof_algorithm, WofAlgorithm, CompressionState};
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

// ===== CORE SCANNING LOGIC =====

/// Generic directory walker that handles recursion and Win32 iteration.
/// `visitor` is called for every file found.
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
    // Handle Pause/Stop
    if let Some(s) = state {
        loop {
            let current = s.load(Ordering::Relaxed);
            if current == ProcessingState::Stopped as u8 { return; }
            if current == ProcessingState::Paused as u8 {
                std::thread::sleep(std::time::Duration::from_millis(100));
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
            std::ptr::null(),
            FIND_FIRST_EX_LARGE_FETCH,
        );

        buffer.truncate(original_len); // Restore path

        if handle == INVALID_HANDLE_VALUE {
            return;
        }

        loop {
            // Check cancellation inside loop
            if let Some(s) = state {
                 if s.load(Ordering::Relaxed) == ProcessingState::Stopped as u8 {
                     FindClose(handle);
                     return;
                 }
            }

            let filename_len = find_data.cFileName.iter().position(|&c| c == 0).unwrap_or(find_data.cFileName.len());
            let is_dot = filename_len == 1 && find_data.cFileName[0] == 46; // '.'
            let is_dot_dot = filename_len == 2 && find_data.cFileName[0] == 46 && find_data.cFileName[1] == 46; // '..'

            if !is_dot && !is_dot_dot {
                let len_before = buffer.len();
                buffer.push_u16_slice(&find_data.cFileName[..filename_len]);
                
                let is_dir = (find_data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
                let is_reparse = (find_data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
                
                let full_path_str = buffer.to_string_lossy();
                visitor(&full_path_str, is_dir, &find_data);

                // Recurse (skip reparse points to avoid loops/symlinks)
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

// ===== PUBLIC API =====

/// Get metrics for a path. If directory, performs single-pass scan.
pub fn scan_path_metrics(path: &str) -> PathMetrics {
    let p = std::path::Path::new(path);
    
    if p.is_file() {
        let logical = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let disk = get_real_file_size(path);
        let state = get_wof_algorithm(path).map(CompressionState::Specific).unwrap_or(CompressionState::None);
        return PathMetrics { logical_size: logical, disk_size: disk, compression_state: state, file_count: 1 };
    }
    
    let mut metrics = PathMetrics { 
        logical_size: 0, disk_size: 0, compression_state: CompressionState::None, file_count: 0 
    };
    let mut seen_algos = std::collections::HashSet::new();
    let mut algo_scanned = 0usize;

    walk_directory(path, None, &mut |full_path, is_dir, find_data| {
        if !is_dir {
            metrics.file_count += 1;
            let size = ((find_data.nFileSizeHigh as u64) << 32) | (find_data.nFileSizeLow as u64);
            metrics.logical_size += size;
            metrics.disk_size += get_real_file_size(full_path);
            
            if algo_scanned < 50 {
                if let Some(algo) = get_wof_algorithm(full_path) {
                    seen_algos.insert(algo as u32);
                }
                algo_scanned += 1;
            }
        }
    });

    metrics.compression_state = resolve_mixed_state(seen_algos);
    metrics
}

/// Scan path and stream progress updates to UI.
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

    let mut metrics = PathMetrics { 
        logical_size: 0, disk_size: 0, compression_state: CompressionState::None, file_count: 0 
    };
    let mut seen_algos = std::collections::HashSet::new();
    let mut algo_scanned = 0usize;
    let mut last_update = Instant::now();

    walk_directory(path, state, &mut |full_path, is_dir, find_data| {
        if !is_dir {
            metrics.file_count += 1;
            let size = ((find_data.nFileSizeHigh as u64) << 32) | (find_data.nFileSizeLow as u64);
            metrics.logical_size += size;
            metrics.disk_size += get_real_file_size(full_path);

            if algo_scanned < 50 {
                if let Some(algo) = get_wof_algorithm(full_path) {
                    seen_algos.insert(algo as u32);
                }
                algo_scanned += 1;
            }

            if last_update.elapsed().as_millis() >= 100 {
                let _ = tx.send(UiMessage::ScanProgress(id, metrics.logical_size, metrics.disk_size, metrics.file_count));
                last_update = Instant::now();
            }
        }
    });

    let _ = tx.send(UiMessage::ScanProgress(id, metrics.logical_size, metrics.disk_size, metrics.file_count));
    metrics.compression_state = resolve_mixed_state(seen_algos);
    metrics
}

/// Optimized scan that collects file paths for processing logic.
pub fn scan_directory_for_processing(
    path: &str,
    state: Option<&Arc<AtomicU8>>,
) -> ScanStats {
    let mut stats = ScanStats::default();
    
    walk_directory(path, state, &mut |full_path, is_dir, find_data| {
        if !is_dir {
            stats.file_count += 1;
            let size = ((find_data.nFileSizeHigh as u64) << 32) | (find_data.nFileSizeLow as u64);
            stats.logical_size += size;
            stats.file_paths.push(full_path.to_string());
        }
    });
    
    stats
}

// ===== HELPERS =====

pub fn detect_path_algorithm(path: &str) -> CompressionState {
    scan_path_metrics(path).compression_state
}

pub fn calculate_path_disk_size(path: &str) -> u64 {
    if std::path::Path::new(path).is_file() {
        get_real_file_size(path)
    } else {
        // Just walk and sum disk sizes, ignoring logical
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