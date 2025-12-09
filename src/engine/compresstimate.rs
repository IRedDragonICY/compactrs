use std::fs::File;
use std::io::Read;
use std::path::Path;

// Simple "Compresstimation"
// We read the first 4KB of a file, try to "compress" it using a cheap heuristic
// (or just check entropy), and if it looks bad, we recommend skipping.
//
// Since we don't want to drag in a full LZ4 dependency just for this if we can avoid it, 
// we can implement a very crude RLE or entropy check, OR just rely on the fact that 
// most incompressible files (videos, archives) have high entropy headers.
//
// However, the prompt mentioned "try to compress in memory". 
// A very lightweight RLE check is often sufficient for sparse files or log files (highly compressible).
// For dense files, we might just lean on WOF's return code, but pre-filtering saves IO.
// 
// Let's implement a rudimentary RLE ratio check for the first 4KB.

pub fn is_compressible(path: &Path) -> bool {
    // 1. Open File
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false, // Can't read, skip
    };

    // 2. Read 4KB
    let mut buffer = [0u8; 4096];
    let bytes_read = match file.read(&mut buffer) {
        Ok(n) if n > 0 => n,
        Ok(_) => return false, // Empty file, skip
        Err(_) => return false,
    };

    if bytes_read < 128 {
        // Too small to matter, Win32 might overhead it.
        // WOF generally ignores files < 4KB anyway, but let's just say "sure" 
        // if user wants to force it, or "no" if we stick to WOF defaults.
        // Let's say yes for now, let WOF decide.
        return true; 
    }

    // 3. Simple Entropy / Compressibility Check
    // We'll use a very fast heuristic:
    // If we can find many repeated byte sequences, it's compressible.
    // If it looks like random noise (high entropy), it's not.
    
    let compressed_estimate = estimate_rle_size(&buffer[..bytes_read]);
    let ratio = compressed_estimate as f32 / bytes_read as f32;

    // If we can squash it to < 90% of original, try WOF.
    // Otherwise, assume it's already compressed (JPG, ZIP, PNG).
    ratio < 0.95
}

pub fn estimate_size(path: &Path) -> u64 {
    // 1. Open File
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    
    let meta = match file.metadata() {
        Ok(m) => m,
        Err(_) => return 0,
    };
    let total_size = meta.len();
    if total_size == 0 { return 0; }

    // 2. Read 4KB
    let mut buffer = [0u8; 4096];
    let bytes_read = match file.read(&mut buffer) {
        Ok(n) if n > 0 => n,
        _ => return total_size,
    };

    // Calculate ratio on header
    let est_header = estimate_rle_size(&buffer[..bytes_read]); // This calls entropy internally fallback
    let ratio = est_header as f64 / bytes_read as f64;
    
    (total_size as f64 * ratio) as u64
}

fn estimate_rle_size(data: &[u8]) -> usize {
    let mut estimated_size = 0;
    let mut i = 0;
    while i < data.len() {
        let current = data[i];
        let mut run_length = 1;
        while i + run_length < data.len() && data[i + run_length] == current && run_length < 255 {
            run_length += 1;
        }
        
        // In a hypothetical RLE, we'd store (count, byte) -> 2 bytes.
        // If run_length > 1, we save space. 
        // But RLE is terrible for normal text. 
        // Let's just count repetitive sequences?
        
        // This is a naive placeholder. A better 'estimate' without deps 
        // might simply check how many unique bytes exist in the window (Shannon entropy).
        i += run_length;
        estimated_size += 2; // (count, value)
    }
    
    // Fallback: This RLE is bad for text. 
    // Let's actually use a Shannon Entropy check for better accuracy without deps.
    // If entropy > 7.5 bits per byte, it's likely compressed.
    // (Random headers might fool this, but it's a start).
    if estimated_size > data.len() {
        // RLE failed hard (data is noisy), likely high entropy or just text.
        // Let's proceed to entropy check.
        // For standard "Text" files, RLE is bad but they ARE compressible.
        // So RLE failure isn't proof of incompressibility.
        
        return shannon_entropy_check(data);
    }

    estimated_size
}

fn shannon_entropy_check(data: &[u8]) -> usize {
    // If entropy is high, size estimate should be close to original.
    // If entropy is low (text, logs), size estimate should be lower.
    
    let mut counts = [0usize; 256];
    for &b in data {
        counts[b as usize] += 1;
    }

    let mut entropy = 0.0;
    let len_f = data.len() as f32;
    for &count in counts.iter() {
        if count == 0 { continue; }
        let p = count as f32 / len_f;
        entropy -= p * p.log2();
    }

    // Max entropy is 8.0.
    // Compressed data is usually > 7.5
    // Text is usually < 5.0
    // Executables ~ 6.0
    
    if entropy > 7.5 {
        return data.len(); // "Full size", incompressible
    } else {
        return (data.len() as f32 * (entropy / 8.0)) as usize; // Estimate based on entropy
    }
}
