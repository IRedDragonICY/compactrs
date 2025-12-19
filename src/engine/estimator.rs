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

use windows_sys::Win32::Storage::Compression::{
    CloseCompressor, Compress, CreateCompressor,
    COMPRESS_ALGORITHM_XPRESS_HUFF,
    COMPRESSOR_HANDLE,
};

use crate::engine::wof::WofAlgorithm;

/// Sample size for estimation (256 KB)
const SAMPLE_SIZE: usize = 256 * 1024;

/// Minimum file size to bother estimating (below this, just return file size)
const MIN_ESTIMATE_SIZE: u64 = 4096;

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
    use std::path::Path;
    
    let p = Path::new(path);
    
    if p.is_dir() {
        // For folders, use recursive estimation
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
/// Estimated compressed size in bytes, or 0 on error
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

/// Estimates compressed sizes for a folder by sampling files.
///
/// # Arguments
/// * `path` - Path to the folder
/// * `algo` - Algorithm to use for estimation
///
/// # Returns
/// Tuple of (total_logical_size, estimated_compressed_size)
pub fn estimate_folder_size(path: &str, algo: WofAlgorithm) -> (u64, u64) {
    use std::path::Path;

    let path = Path::new(path);
    if !path.is_dir() {
        // Single file
        let logical = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let estimated = estimate_compressed_size(path.to_str().unwrap_or(""), algo);
        return (logical, estimated);
    }

    let mut total_logical: u64 = 0;
    let mut total_estimated: u64 = 0;

    // Walk directory
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file() {
                if let Ok(meta) = entry_path.metadata() {
                    total_logical += meta.len();
                    total_estimated += estimate_compressed_size(
                        entry_path.to_str().unwrap_or(""),
                        algo,
                    );
                }
            } else if entry_path.is_dir() {
                // Recursively process subdirectories
                let (sub_logical, sub_estimated) =
                    estimate_folder_size(entry_path.to_str().unwrap_or(""), algo);
                total_logical += sub_logical;
                total_estimated += sub_estimated;
            }
        }
    }

    (total_logical, total_estimated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_algorithm() {
        assert_eq!(map_algorithm(WofAlgorithm::Xpress4K), COMPRESS_ALGORITHM_XPRESS_HUFF);
        assert_eq!(map_algorithm(WofAlgorithm::Xpress8K), COMPRESS_ALGORITHM_XPRESS_HUFF);
        assert_eq!(map_algorithm(WofAlgorithm::Xpress16K), COMPRESS_ALGORITHM_XPRESS_HUFF);
        assert_eq!(map_algorithm(WofAlgorithm::Lzx), COMPRESS_ALGORITHM_LZMS);
    }
}
