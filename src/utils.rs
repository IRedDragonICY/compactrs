
/// Convert a Rust string to a null-terminated UTF-16 vector.
pub fn to_wstring(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
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
