//! Size Estimation using Windows Compression API
//!
//! This module provides compressed size estimation by sampling file content
//! and using the native Windows Compression API (CreateCompressor/Compress).
//!
//! Note: The Win32 Compression API algorithms (XPRESS_HUFF, LZMS) don't directly
//! map to WOF algorithms (XPRESS4K/8K/16K, LZX), but provide reasonable estimates.

use std::fs::File;
use std::io::Read;
use std::ptr;
use std::path::Path;

use windows_sys::Win32::Storage::Compression::{
    CloseCompressor, Compress, CreateCompressor,
    COMPRESS_ALGORITHM_XPRESS_HUFF,
    COMPRESSOR_HANDLE,
};

use crate::engine::wof::WofAlgorithm;

/// Sample size for individual file reading (256 KB)
const SAMPLE_SIZE: usize = 256 * 1024;

/// Minimum file size to bother estimating (below this, just return file size)
const MIN_ESTIMATE_SIZE: u64 = 4096;

/// Maximum total bytes to read from disk during folder estimation.
/// Prevents IO bottlenecks on massive directories (e.g., 50MB limit).
const MAX_TOTAL_SAMPLE_BYTES: u64 = 50 * 1024 * 1024; 

/// Sampling rate for small files (1 in N files)
const SMALL_FILE_SAMPLING_RATE: usize = 20;

/// Threshold to consider a file "Large" (1 MB). Large files are always sampled until cap.
const LARGE_FILE_THRESHOLD: u64 = 1024 * 1024;

/// Maps WofAlgorithm to Windows Compression API algorithm.
///
/// We always use XPRESS_HUFF for speed, then apply heuristic adjustments.
/// This avoids the slow LZMS algorithm while still providing reasonable estimates.
fn map_algorithm(_algo: WofAlgorithm) -> u32 {
    // Always use XPRESS_HUFF for fast estimation
    // We apply heuristic multipliers later based on the target algorithm
    COMPRESS_ALGORITHM_XPRESS_HUFF
}

/// Estimates the compressed size of a path (file or folder).
///
/// This is the main entry point for estimation. It automatically detects
/// whether the path is a file or folder and calls the appropriate function.
///
/// # Arguments
/// * `path` - Path to the file or folder to estimate
/// * `algo` - The WOF algorithm to simulate
///
/// # Returns
/// Estimated compressed size in bytes
pub fn estimate_path(path: &str, algo: WofAlgorithm) -> u64 {
    let p = Path::new(path);
    
    if p.is_dir() {
        // For folders, use recursive sampling estimation
        let (_logical, estimated) = estimate_folder_size(path, algo);
        estimated
    } else if p.is_file() {
        // For files, use direct estimation
        estimate_compressed_size(path, algo)
    } else {
        // Path doesn't exist or is inaccessible
        0
    }
}

/// Estimates the compressed size of a single file using Windows Compression API.
///
/// This function:
/// 1. Reads the first 256KB of the file (or entire file if smaller)
/// 2. Compresses the sample using the appropriate algorithm
/// 3. Calculates the compression ratio
/// 4. Extrapolates to the full file size
///
/// # Arguments
/// * `path` - Path to the file to estimate
/// * `algo` - The WOF algorithm to simulate
///
/// # Returns
/// Estimated compressed size in bytes, or original size on error
pub fn estimate_compressed_size(path: &str, algo: WofAlgorithm) -> u64 {
    // Open file and get metadata
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return 0,
    };

    let total_size = match file.metadata() {
        Ok(m) => m.len(),
        Err(_) => return 0,
    };

    // Skip very small files
    if total_size < MIN_ESTIMATE_SIZE {
        return total_size;
    }

    // Read sample buffer
    let sample_len = std::cmp::min(SAMPLE_SIZE, total_size as usize);
    let mut sample = vec![0u8; sample_len];
    let bytes_read = match file.read(&mut sample) {
        Ok(n) if n > 0 => n,
        _ => return total_size,
    };
    sample.truncate(bytes_read);

    // Compress sample using Windows API
    let compressed_size = compress_buffer(&sample, algo);

    if compressed_size == 0 || compressed_size >= bytes_read {
        // Compression failed or didn't help - assume incompressible
        return total_size;
    }

    // Calculate base ratio and extrapolate
    let base_ratio = compressed_size as f64 / bytes_read as f64;
    
    // Apply block size / algorithm adjustment
    // All estimates are based on XPRESS_HUFF, so we adjust accordingly:
    // - Smaller XPRESS blocks = slightly worse compression (more overhead)
    // - LZX typically achieves ~20-25% better compression than XPRESS16K
    let adjusted_ratio = match algo {
        WofAlgorithm::Xpress4K => base_ratio * 1.12,   // ~12% worse than XPRESS16K
        WofAlgorithm::Xpress8K => base_ratio * 1.05,   // ~5% worse than XPRESS16K
        WofAlgorithm::Xpress16K => base_ratio,         // Base measurement
        WofAlgorithm::Lzx => base_ratio * 0.78,        // ~22% better than XPRESS16K
    };
    
    let estimated = (total_size as f64 * adjusted_ratio) as u64;

    // Clamp to reasonable bounds (at least 1 byte, at most original size)
    estimated.clamp(1, total_size)
}

/// Compresses a buffer in memory using Windows Compression API.
///
/// # Returns
/// Size of compressed data, or 0 on failure.
fn compress_buffer(data: &[u8], algo: WofAlgorithm) -> usize {
    if data.is_empty() {
        return 0;
    }

    unsafe {
        let mut compressor: COMPRESSOR_HANDLE = ptr::null_mut();
        let win_algo = map_algorithm(algo);

        // Create compressor
        if CreateCompressor(win_algo, ptr::null(), &mut compressor) == 0 {
            return 0;
        }

        // First call to get required output buffer size
        let mut compressed_size: usize = 0;
        let _ = Compress(
            compressor,
            data.as_ptr() as *const _,
            data.len(),
            ptr::null_mut(),
            0,
            &mut compressed_size,
        );

        if compressed_size == 0 {
            CloseCompressor(compressor);
            return 0;
        }

        // Allocate output buffer and compress
        let mut output = vec![0u8; compressed_size];
        let mut final_size: usize = 0;

        let result = Compress(
            compressor,
            data.as_ptr() as *const _,
            data.len(),
            output.as_mut_ptr() as *mut _,
            output.len(),
            &mut final_size,
        );

        CloseCompressor(compressor);

        if result == 0 {
            return 0;
        }

        final_size
    }
}

/// Estimates compressed sizes for a folder by using SMART SAMPLING.
///
/// Instead of reading every file (which kills performance), this function:
/// 1. Scans all files for accurate Logical Size (metadata only).
/// 2. Samples specific files for Compression Ratio (content read).
/// 3. Extrapolates the ratio to the total size.
///
/// # Arguments
/// * `path` - Path to the folder
/// * `algo` - Algorithm to use for estimation
///
/// # Returns
/// Tuple of (total_logical_size, estimated_compressed_size)
pub fn estimate_folder_size(path: &str, algo: WofAlgorithm) -> (u64, u64) {
    let mut total_logical: u64 = 0;
    let mut sampled_logical: u64 = 0;
    let mut sampled_compressed: u64 = 0;
    let mut file_count: usize = 0;

    // Start recursive sampling
    visit_dirs_sampling(
        path, 
        algo, 
        &mut total_logical, 
        &mut sampled_logical, 
        &mut sampled_compressed, 
        &mut file_count
    );

    // Calculate ratio from samples
    let estimated_total = if sampled_logical > 0 {
        let ratio = sampled_compressed as f64 / sampled_logical as f64;
        (total_logical as f64 * ratio) as u64
    } else {
        // Fallback: If no samples were taken (empty folder or all reads failed),
        // assume no compression (1:1).
        total_logical 
    };

    (total_logical, estimated_total)
}

/// Recursive helper for smart sampling
fn visit_dirs_sampling(
    dir: &str, 
    algo: WofAlgorithm, 
    total_log: &mut u64, 
    samp_log: &mut u64, 
    samp_comp: &mut u64,
    count: &mut usize
) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            
            if path.is_dir() {
                if let Some(s) = path.to_str() {
                    visit_dirs_sampling(s, algo, total_log, samp_log, samp_comp, count);
                }
            } else if path.is_file() {
                // Get metadata (cheap operation)
                if let Ok(meta) = path.metadata() {
                    let len = meta.len();
                    *total_log += len;
                    *count += 1;

                    // --- SAMPLING LOGIC ---
                    
                    // Condition 1: Have we exceeded the IO Cap?
                    if *samp_log >= MAX_TOTAL_SAMPLE_BYTES {
                        continue;
                    }

                    // Condition 2: Is the file significant enough or is it its turn?
                    // - Always sample large files (they impact ratio the most)
                    // - Sample small files sparsely (1 in 20)
                    let should_sample = if len > LARGE_FILE_THRESHOLD {
                        true 
                    } else {
                        *count % SMALL_FILE_SAMPLING_RATE == 0
                    };

                    if should_sample {
                        if let Some(p_str) = path.to_str() {
                            // Heavy operation: Read + Compress
                            let est = estimate_compressed_size(p_str, algo);
                            
                            // estimate_compressed_size returns 0 on total failure,
                            // or returns original size if incompressible.
                            // We treat 0 as a read failure and don't add to sample stats.
                            // However, we must ensure we only add if it actually processed data.
                            
                            // Since estimate_compressed_size handles its own fallback, 
                            // we rely on it returning a valid number > 0 for valid files.
                            if est > 0 || len == 0 {
                                *samp_log += len;
                                *samp_comp += est;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_algorithm() {
        assert_eq!(map_algorithm(WofAlgorithm::Xpress4K), COMPRESS_ALGORITHM_XPRESS_HUFF);
        assert_eq!(map_algorithm(WofAlgorithm::Xpress8K), COMPRESS_ALGORITHM_XPRESS_HUFF);
        assert_eq!(map_algorithm(WofAlgorithm::Xpress16K), COMPRESS_ALGORITHM_XPRESS_HUFF);
        // We use XPRESS_HUFF for LZX estimation too, with a multiplier, for performance
        assert_eq!(map_algorithm(WofAlgorithm::Lzx), COMPRESS_ALGORITHM_XPRESS_HUFF);
    }
}