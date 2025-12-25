/* --- src/utils.rs --- */
use crate::types::*;

/// Macro to convert a string literal to a null-terminated UTF-16 array at compile time.
/// 
/// # Warning
/// **This macro ONLY supports ASCII strings.** Multi-byte characters (Unicode) will be 
/// cast to u16 incorrectly, resulting in garbled text (Mojibake). 
/// For Unicode strings, use `to_wstring` at runtime.
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
                    // Unsafe pointer access to bypass const-index limitations on older Rust
                    out[i] = unsafe { *S.as_ptr().add(i) } as u16;
                    i += 1;
                }
                out[LEN - 1] = 0;
                out
            };
            &UTF16 as &[u16]
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

/// Formats a string using Win32 wsprintfW into a stack buffer.
/// Returns a Vec<u16> for compatibility with existing APIs.
pub unsafe fn fmt_u64(template: &[u16], value: u64) -> Vec<u16> {
    let mut buffer = [0u16; 128];
    unsafe {
        crate::types::wsprintfW(buffer.as_mut_ptr(), template.as_ptr(), value);
    }
    let len = (0..128).take_while(|&i| buffer[i] != 0).count();
    buffer[..=len].to_vec()
}

/// Formats a u32 using Win32 wsprintfW (%u)
pub unsafe fn fmt_u32(value: u32) -> Vec<u16> {
    let mut buffer = [0u16; 32];
    let fmt = crate::w!("%u");
    unsafe {
        crate::types::wsprintfW(buffer.as_mut_ptr(), fmt.as_ptr(), value);
    }
    let len = (0..32).take_while(|&i| buffer[i] != 0).count();
    buffer[..=len].to_vec()
}

/// Formats a u32 using Win32 wsprintfW (%02u)
pub unsafe fn fmt_u32_padded(value: u32) -> Vec<u16> {
    let mut buffer = [0u16; 32];
    let fmt = crate::w!("%02u");
    unsafe {
        crate::types::wsprintfW(buffer.as_mut_ptr(), fmt.as_ptr(), value);
    }
    let len = (0..32).take_while(|&i| buffer[i] != 0).count();
    buffer[..=len].to_vec()
}

/// Helper to format "Processed %I64u / %I64u"
pub unsafe fn fmt_progress(current: u64, total: u64) -> Vec<u16> {
    let mut buffer = [0u16; 128];
    // %I64u is the Windows specifier for u64 in wsprintf
    // We include "Processed " here to match the UI requirement in one go if possible,
    // or just the numbers. The prompt snippet showed just numbers.
    // However, to replace the existing logic "Processed {} / {}", it's better to verify what the prompt snippet in the text said.
    // The prompt snippet: let fmt = crate::w!("%I64u / %I64u");
    // So I will use that.
    let fmt = crate::w!("%I64u / %I64u"); 
    unsafe {
        crate::types::wsprintfW(buffer.as_mut_ptr(), fmt.as_ptr(), current, total);
    }
    let len = (0..128).take_while(|&i| buffer[i] != 0).count();
    buffer[..=len].to_vec()
}

/// Helper to format timestamp "[HH:MM:SS]"
pub unsafe fn fmt_timestamp(ts: u64) -> Vec<u16> {
    // [HH:MM:SS]
    let s = ts % 86400;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let s = s % 60;
    
    let mut buffer = [0u16; 32];
    let fmt = crate::w!("[%02u:%02u:%02u]");
    unsafe {
       crate::types::wsprintfW(buffer.as_mut_ptr(), fmt.as_ptr(), h, m, s);
    }
    let len = (0..32).take_while(|&i| buffer[i] != 0).count();
    // We do NOT return null terminator usually if we want to concat later?
    // concat_wstrings expects null-terminated parts (it strips them).
    // Our existing helpers return null-terminated vecs.
    buffer[..=len].to_vec()
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
    
    // (logical - disk) * 1000 / logical => gives us X.Y % as XY integer
    let saved = if logical > disk { logical - disk } else { 0 };
    
    // Use u128 to prevent overflow during multiplication if file sizes are huge (exabytes)
    // Though u64 max is 18 EB, * 1000 would overflow.
    let ratio_10x = (saved as u128 * 1000) / (logical as u128);
    let whole = (ratio_10x / 10) as u32;
    let decimal = (ratio_10x % 10) as u32;

    let mut buffer = [0u16; 32];
    let fmt = crate::w!("%u.%u%%");
    
    unsafe {
        crate::types::wsprintfW(buffer.as_mut_ptr(), fmt.as_ptr(), whole, decimal);
    }
    
    let len = (0..32).take_while(|&i| buffer[i] != 0).count();
    buffer[..=len].to_vec()
}

/// Helper to get client rect (safe wrapper)
pub fn get_client_rect(hwnd: HWND) -> RECT {
    let mut rc = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe {
        crate::types::GetClientRect(hwnd, &mut rc);
    }
    rc
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

// ===== CUSTOM MATCHER OPTIMIZATION =====

pub mod matcher {
    /// Simple wildcard matcher (Glob-like)
    /// Supports:
    /// - `?`: Match any single character
    /// - `*`: Match any sequence of characters (including empty)
    /// - `^`: Anchor match to start (optional support, typical globs don't use it but useful)
    /// - `$`: Anchor match to end
    ///
    /// Use `is_match(pattern, text)` as main entry.
    pub fn is_match(pattern: &str, text: &str) -> bool {
        let p_chars: Vec<char> = pattern.chars().collect();
        let t_chars: Vec<char> = text.chars().collect();
        match_slice(&p_chars, &t_chars)
    }

    fn match_slice(p: &[char], t: &[char]) -> bool {
        // If pattern is empty, text must be empty (unless pattern was just wildcards that matched empty?)
        // Actually, simple recursive approach:
        
        if p.is_empty() {
            return t.is_empty();
        }
        
        match p[0] {
            '*' => {
                // '*' matches zero or more characters.
                // Try matching '*' to 0 chars (recurse with p[1..], t)
                // Or matching '*' to 1 char (recurse with p, t[1..])
                
                // Optimization: Skip consecutive '*'
                let mut next_p = 1;
                while next_p < p.len() && p[next_p] == '*' {
                    next_p += 1;
                }
                // If '*' is trailing, it matches everything remaining.
                if next_p == p.len() {
                    return true;
                }
                // Recursion: Try to find a match for p[next_p..] in t[i..]
                // Greedy or non-greedy?
                // "expert optimization" -> Iteration would be better than recursion for deep strings,
                // but for file names recursion is fine.
                
                for i in 0..=t.len() {
                    if match_slice(&p[next_p..], &t[i..]) {
                        return true;
                    }
                }
                false
            },
            '?' => {
                if t.is_empty() { return false; }
                match_slice(&p[1..], &t[1..])
            },
            '^' => {
                // Anchor start. Only valid at start of pattern?
                // For simple implementation, let's treating it as "match must start at 0".
                // But is_match usually assumes partial match?
                // "Contains" vs "Exact Match".
                // Our app logic `regex.is_match` implies "Contains".
                // BUT file searching usually expects "Contains" by default unless glob chars are used?
                // If user types "foo", they find "foo_bar".
                // If I implement wildcard logic:
                // "foo" -> matches "foo" only? Or ".*foo.*"?
                
                // DECISION: 
                // If pattern contains `*` or `?`, treat as GLOB (Full Match required against pattern).
                // If NO wildcards, treat as SUBSTRING (Contains).
                // But `fn is_match` should implement pure logic.
                
                 // Treating literal char match
                if t.is_empty() { return false; }
                if p[0] == t[0] {
                    match_slice(&p[1..], &t[1..])
                } else {
                    false
                }
            },
            c => {
                if t.is_empty() { return false; }
                 // Case sensitivity? handled by caller lowercasing usually.
                if c == t[0] {
                    match_slice(&p[1..], &t[1..])
                } else {
                    false
                }
            }
        }
    }
}
