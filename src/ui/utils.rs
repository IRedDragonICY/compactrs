//! Central utility module for Win32 helper functions.
//! 
//! Provides abstractions to reduce boilerplate code:
//! - `ToWide` trait for UTF-16 null-terminated string conversion
//! - `get_window_state` for safe window state retrieval
//! - `load_app_icon` for centralized icon loading

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GWLP_USERDATA, LoadImageW, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED, HICON,
};
use windows::Win32::UI::Shell::StrFormatByteSizeW;

/// Extension trait for converting Rust strings to null-terminated UTF-16 vectors.
/// 
/// # Example
/// ```
/// use crate::ui::utils::ToWide;
/// 
/// let wide = "Hello".to_wide();
/// // wide is now a Vec<u16> with null terminator
/// ```
pub trait ToWide {
    /// Allocates a vector of u16 with a null terminator.
    fn to_wide(&self) -> Vec<u16>;
}

impl ToWide for str {
    fn to_wide(&self) -> Vec<u16> {
        self.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

impl ToWide for String {
    fn to_wide(&self) -> Vec<u16> {
        self.as_str().to_wide()
    }
}

/// Safely retrieves a mutable reference to window state from GWLP_USERDATA.
/// 
/// Returns `None` if the pointer is null.
/// 
/// # Safety
/// Caller must ensure that:
/// - The pointer stored in GWLP_USERDATA is valid and points to type `T`
/// - The returned reference is not used after the data is freed
/// - No aliasing violations occur (only one mutable reference at a time)
/// 
/// # Example
/// ```
/// let state = unsafe { get_window_state::<AppState>(hwnd) };
/// if let Some(st) = state {
///     // Use st...
/// }
/// ```
#[inline]
pub unsafe fn get_window_state<'a, T>(hwnd: HWND) -> Option<&'a mut T> { unsafe {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if ptr == 0 {
        None
    } else {
    Some(&mut *(ptr as *mut T))
    }
}}

/// Loads the application icon from resources.
///
/// This centralizes the icon loading logic that was previously duplicated
/// across multiple window creation sites.
///
/// # Safety
/// Caller must ensure `instance` is a valid module handle.
///
/// # Arguments
/// * `instance` - The HINSTANCE of the module containing the icon resource
///
/// # Returns
/// The loaded HICON, or a default empty icon if loading fails.
#[inline]
pub unsafe fn load_app_icon(instance: HINSTANCE) -> HICON {
    unsafe {
        let handle = LoadImageW(
            Some(instance),
            PCWSTR(1 as *const u16),
            IMAGE_ICON,
            0, 0,
            LR_DEFAULTSIZE | LR_SHARED,
        ).unwrap_or_default();
        HICON(handle.0)
    }
}

/// Formats a byte size into a human-readable string using the Windows Shell API.
///
/// This is a pure Win32 implementation that uses `StrFormatByteSizeW` from shlwapi.dll
/// to format file sizes according to the user's locale settings (e.g., "1.5 GB").
///
/// # Arguments
/// * `bytes` - The size in bytes to format
///
/// # Returns
/// A human-readable string representation of the size (e.g., "1.5 GB", "256 KB")
///
/// # Example
/// ```
/// let size_str = format_size(1073741824); // "1.00 GB"
/// ```
#[inline]
pub fn format_size(bytes: u64) -> String {
    // Stack buffer for the result (32 chars is plenty for size strings)
    let mut buffer: [u16; 32] = [0; 32];
    
    unsafe {
        // StrFormatByteSizeW takes an i64 (LONGLONG) for the size
        // For values > i64::MAX, we clamp to i64::MAX (unlikely in practice)
        let size_i64 = if bytes > i64::MAX as u64 {
            i64::MAX
        } else {
            bytes as i64
        };
        
        let result = StrFormatByteSizeW(size_i64, &mut buffer);
        
        // Find the null terminator and convert to String
        let len = result.as_wide().len();
        String::from_utf16_lossy(&buffer[..len])
    }
}
