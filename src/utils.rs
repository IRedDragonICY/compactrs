/* --- src/utils.rs --- */
use windows_sys::Win32::UI::Shell::{StrFormatByteSizeW, ShellExecuteW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Macro to convert a string literal to a null-terminated UTF-16 array at compile time.
/// 
/// # Example
/// ```rust
/// let wide_str = w!("Hello");
/// assert_eq!(wide_str, &[72, 101, 108, 108, 111, 0]);
/// ```
#[macro_export]
macro_rules! w {
    ($s:literal) => {
        {
            const S: &[u8] = $s.as_bytes();
            const LEN: usize = S.len() + 1;
            const UTF16: [u16; LEN] = {
                let mut out = [0u16; LEN];
                let mut i = 0;
                while i < S.len() {
                    out[i] = S[i] as u16;
                    i += 1;
                }
                out[LEN - 1] = 0;
                out
            };
            &UTF16[..]
        }
    };
}

/// Convert a Rust string to a null-terminated UTF-16 vector.
pub fn to_wstring(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Helper trait for easy string passing to Win32 APIs
pub trait ToPcwstr {
    fn to_pcwstr(&self) -> Vec<u16>;
}

impl ToPcwstr for str {
    fn to_pcwstr(&self) -> Vec<u16> { to_wstring(self) }
}

impl ToPcwstr for String {
    fn to_pcwstr(&self) -> Vec<u16> { to_wstring(self) }
}

/// Converts u64 to a null-terminated UTF-16 vector (Manual implementation to avoid large fmt deps).
pub fn u64_to_wstring(mut val: u64) -> Vec<u16> {
    if val == 0 { return vec![0x0030, 0x0000]; } // "0\0"
    let mut buf = Vec::with_capacity(24);
    while val > 0 {
        let digit = (val % 10) as u16;
        buf.push(0x0030 + digit);
        val /= 10;
    }
    buf.reverse();
    buf.push(0x0000);
    buf
}

/// Efficiently concatenates multiple UTF-16 slices into a single null-terminated vector.
/// Calculates total size upfront to perform exactly one allocation.
pub fn concat_wstrings(parts: &[&[u16]]) -> Vec<u16> {
    // Calculate exact required capacity excluding intermediate nulls
    let total_len = parts.iter().map(|p| {
        if p.last() == Some(&0) { p.len() - 1 } else { p.len() }
    }).sum::<usize>();

    let mut res = Vec::with_capacity(total_len + 1);
    
    for part in parts {
        if part.is_empty() { continue; }
        let len = if part.last() == Some(&0) { part.len() - 1 } else { part.len() };
        res.extend_from_slice(&part[..len]);
    }
    res.push(0);
    res
}

/// Formats a byte size into a human-readable string using the Windows Shell API.
pub fn format_size(bytes: u64) -> Vec<u16> {
    let mut buffer: [u16; 32] = [0; 32];
    
    unsafe {
        let size_i64 = if bytes > i64::MAX as u64 {
            i64::MAX
        } else {
            bytes as i64
        };
        
        let ptr = StrFormatByteSizeW(size_i64, buffer.as_mut_ptr(), buffer.len() as u32);
        
        if ptr.is_null() {
            return vec![0];
        }

        // Buffer is filled with null-terminated string
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        buffer[..=len].to_vec() // Include null terminator
    }
}

/// Reveal a file or folder in Windows Explorer
pub fn reveal_path_in_explorer(path: &str) {
    let select_prefix = w!("/select,\"");
    let path_w = to_wstring(path); // Normal simple conversion usually fine here for UI click
    let suffix = w!("\"");
    
    let args = concat_wstrings(&[select_prefix, &path_w[..], suffix]);
    
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            w!("open").as_ptr(),
            w!("explorer.exe").as_ptr(),
            args.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL
        );
    }
}

/// Calculates the percentage of space saved.
pub fn calculate_saved_percentage(logical: u64, disk: u64) -> f64 {
    if logical == 0 { return 0.0; }
    if disk >= logical { return 0.0; }
    100.0 - ((disk as f64 / logical as f64) * 100.0)
}

/// Calculates the compression ratio string (e.g. "40.5%")
pub fn calculate_ratio_string(logical: u64, disk: u64) -> Vec<u16> {
    if logical == 0 { return w!("-").to_vec(); }
    
    let ratio = calculate_saved_percentage(logical, disk);
    let s = format!("{:.1}%", ratio);
    to_wstring(&s)
}

// ===== PATH BUFFER OPTIMIZATION =====

/// A specialized buffer for building null-terminated UTF-16 paths efficiently.
/// Minimizes reallocations by reusing the underlying vector.
/// Handles path normalization (`/` -> `\`) during append to avoid intermediate strings.
pub struct PathBuffer {
    // Invariant: Always contains a null-terminated UTF-16 string if not empty.
    buf: Vec<u16>,
}

impl PathBuffer {
    /// Creates a new PathBuffer with specified capacity.
    /// Uses a larger default capacity to prevent reallocations during recursion.
    pub fn with_capacity(capacity: usize) -> Self {
        // We generally want enough space for MAX_PATH (260) or extended paths (32k).
        // 1024 is a good balance for typical directory recursion depth.
        let cap = std::cmp::max(capacity, 1024);
        Self { buf: Vec::with_capacity(cap) }
    }
    
    /// Creates a PathBuffer from a rust string, normalizing it immediately.
    /// Handles long path prefix `\\?\` if necessary.
    pub fn from(s: &str) -> Self {
        let mut pb = Self::with_capacity(s.len() + 10);
        
        // Basic check for long path requirement (heuristic)
        // If it's absolute, starts with drive letter, and is long, we might strictly need prefix.
        // However, most modern Win32 APIs (Unicode versions) handle paths up to 32k if they start with \\?\
        // For simplicity, we just push normalized. Caller or `push_normalized` logic can handle prefix if needed.
        // But the user request said "PathBuffer handles the \\?\ prefix logic ... or avoid allocations".
        
        // Heuristic: If path is likely absolute and long, prepend \\?\ if not present.
        // We'll do a simple check.
        let needs_prefix = s.len() > 240 && s.contains(':') && !s.starts_with("\\\\?\\");
        if needs_prefix {
            pb.buf.extend_from_slice(w!("\\\\?\\"));
            // Warning: \\?\ disables normalization in Windows APIs usually (requires strict backslashes).
            // But our push_normalized ensures backslashes anyway.
        }
        
        pb.push_normalized(s);
        pb
    }
    
    /// Appends a component to the path, adding a backslash if needed.
    /// Normalizes `/` to `\` on the fly without allocation.
    pub fn push(&mut self, s: &str) {
        self.push_normalized(s);
    }
    
    /// Core logic: Normalizes and appends string.
    fn push_normalized(&mut self, s: &str) {
        if self.buf.is_empty() {
             // First push, just add
             for c in s.chars() {
                 if c == '/' {
                     self.buf.push(b'\\' as u16);
                 } else {
                     // Handle UTF-16 encoding of char
                     let mut buf = [0u16; 2];
                     let encoded = c.encode_utf16(&mut buf);
                     self.buf.extend_from_slice(encoded);
                 }
             }
             self.buf.push(0);
             return;
        }
        
        self.pop_null();
        
        // Add separator if needed
        if !self.buf.is_empty() && *self.buf.last().unwrap() != b'\\' as u16 {
            self.buf.push(b'\\' as u16);
        }
        
        for c in s.chars() {
            if c == '/' {
                self.buf.push(b'\\' as u16);
            } else {
                 let mut buf = [0u16; 2];
                 let encoded = c.encode_utf16(&mut buf);
                 self.buf.extend_from_slice(encoded);
            }
        }
        self.buf.push(0);
    }
    
    /// Appends a raw u16 slice (e.g. from FindFirstFile).
    /// Assumes the slice is already valid path components (usually file names).
    pub fn push_u16_slice(&mut self, slice: &[u16]) {
        if self.buf.is_empty() {
            let len = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
            self.buf.extend_from_slice(&slice[..len]);
            self.buf.push(0);
            return;
        }

        self.pop_null();
        
        if !self.buf.is_empty() && *self.buf.last().unwrap() != b'\\' as u16 {
            self.buf.push(b'\\' as u16);
        }
        
        let len = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
        self.buf.extend_from_slice(&slice[..len]);
        self.buf.push(0);
    }
    
    /// Truncates the buffer to a specific length (restoring null terminator).
    pub fn truncate(&mut self, len: usize) {
        if len >= self.buf.len() { return; }
        
        self.buf.truncate(len);
        // Ensure null terminator
        if self.buf.is_empty() || *self.buf.last().unwrap() != 0 {
            self.buf.push(0);
        }
    }
    
    /// Returns raw pointer to the null-terminated UTF-16 string.
    pub fn as_ptr(&self) -> *const u16 {
        if self.buf.is_empty() {
            return [0u16].as_ptr(); 
        }
        self.buf.as_ptr()
    }
    
    /// Returns the length of the string (excluding null terminator).
    pub fn len(&self) -> usize {
        if self.buf.is_empty() { 0 } else { self.buf.len() - 1 }
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// Helper to remove the trailing null terminator temporarily.
    fn pop_null(&mut self) {
        if let Some(&0) = self.buf.last() {
            self.buf.pop();
        }
    }

    /// Converts to Rust String (lossy).
    pub fn to_string_lossy(&self) -> String {
        if self.buf.is_empty() { return String::new(); }
        let len = if self.buf.last() == Some(&0) { self.buf.len() - 1 } else { self.buf.len() };
        String::from_utf16_lossy(&self.buf[..len])
    }
}
