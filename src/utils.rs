/* --- src/utils.rs --- */

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
