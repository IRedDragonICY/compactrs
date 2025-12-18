
use windows_sys::Win32::UI::WindowsAndMessaging::wsprintfW;

/// Convert a Rust string to a null-terminated UTF-16 vector.
pub fn to_wstring(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Convert a Rust string to a null-terminated UTF-16 vector, handling long paths.
/// Automatically adds `\\?\` prefix if the path is absolute and long.
/// Also normalizes forward slashes to backslashes.
pub fn to_wstring_long_path(value: &str) -> Vec<u16> {
    // 1. Normalize separators (Windows extended paths require backslashes)
    let clean_path = value.replace('/', "\\");
    
    let mut s = String::from(clean_path);
    
    // 2. Apply extended path prefix logic
    // Heuristic: If it's an absolute path (contains ':'), lacks the prefix, 
    // and exceeds standard limits (or is close to it), apply the prefix.
    // We check for len > 240 to be safe near the 260 limit.
    if s.len() > 240 && s.contains(':') && !s.starts_with("\\\\?\\") {
        s.insert_str(0, "\\\\?\\");
    }
    
    to_wstring(&s)
}

/// Helper macro to check BOOL return values from Win32 APIs.
/// Returns Ok(()) if TRUE (1), Err(GetLastError()) if FALSE (0).
#[macro_export]
macro_rules! check_bool {
    ($e:expr) => {
        if $e != 0 {
            Ok(())
        } else {
            use windows_sys::Win32::Foundation::GetLastError;
            Err(unsafe { GetLastError() })
        }
    };
}

/// Helper trait for easy string passing to Win32 APIs
pub trait ToPcwstr {
    fn to_pcwstr(&self) -> Vec<u16>;
}

impl ToPcwstr for str {
    fn to_pcwstr(&self) -> Vec<u16> {
        to_wstring(self)
    }
}

impl ToPcwstr for String {
    fn to_pcwstr(&self) -> Vec<u16> {
        to_wstring(self)
    }
}

/// Converts an i32 to a null-terminated UTF-16 vector using wsprintfW.
pub fn i32_to_wstring(val: i32) -> Vec<u16> {
    let mut buf = [0u16; 32];
    // "%d" for signed integer
    let fmt = [0x0025, 0x0064, 0x0000]; // "%d\0"
    unsafe {
        wsprintfW(buf.as_mut_ptr(), fmt.as_ptr(), val);
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    buf[..=len].to_vec()
}

/// Converts a u64 to a null-terminated UTF-16 vector.
/// Note: wsprintfW does not support u64 on all platforms consistently with %llu in all contexts for Win32.
/// We use a custom simple implementation to avoid large dependencies.
pub fn u64_to_wstring(mut val: u64) -> Vec<u16> {
    if val == 0 {
        return vec![0x0030, 0x0000]; // "0\0"
    }

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

/// Concatenates multiple null-terminated UTF-16 slices into a single null-terminated vector.
/// Removes intermediate null terminators.
pub fn concat_wstrings(parts: &[&[u16]]) -> Vec<u16> {
    let total_len = parts.iter().map(|p| p.len().saturating_sub(1)).sum::<usize>();
    let mut res = Vec::with_capacity(total_len + 1);
    
    for (_i, part) in parts.iter().enumerate() {
        if part.is_empty() { continue; }
        // If it looks null-terminated (last char is 0), exclude it, unless it's the very last part?
        // Actually, we want to exclude nulls from all parts, and append one at the end.
        let has_null = part.last() == Some(&0);
        let len = if has_null { part.len() - 1 } else { part.len() };
        res.extend_from_slice(&part[..len]);
    }
    res.push(0);
    res
}
