/* --- src/utils.rs --- */
use windows_sys::Win32::UI::Shell::{StrFormatByteSizeW, ShellExecuteW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;


/// Convert a Rust string to a null-terminated UTF-16 vector.
pub fn to_wstring(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Convert a Rust string to a null-terminated UTF-16 vector, handling long paths.
pub fn to_wstring_long_path(value: &str) -> Vec<u16> {
    let clean_path = value.replace('/', "\\");
    let mut s = String::from(clean_path);
    if s.len() > 240 && s.contains(':') && !s.starts_with("\\\\?\\") {
        s.insert_str(0, "\\\\?\\");
    }
    to_wstring(&s)
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
    let select_prefix = to_wstring("/select,\"");
    let path_w = to_wstring_long_path(path);
    // Remove null terminator from path_w for concatenation if using concat_wstrings logic manually, 
    // but here we can just construct carefully.
    // Actually, `to_wstring` adds a null terminator. `concat_wstrings` expects null-terminated slices.
    // Note: `concat_wstrings` logic handles stripping nulls from parts except the last one.
    
    // We need strict quoting: /select,"C:\Path\To\File"
    // `path_w` has \0 at end.
    let suffix = to_wstring("\"");
    
    let args = concat_wstrings(&[&select_prefix, &path_w, &suffix]);
    
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            to_wstring("open").as_ptr(),
            to_wstring("explorer.exe").as_ptr(),
            args.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL
        );
    }
}
