//! Central utility module for Win32 helper functions.
//! 
//! Provides abstractions to reduce boilerplate code:
//! - `ToWide` trait for UTF-16 null-terminated string conversion
//! - `get_window_state` for safe window state retrieval

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, GWLP_USERDATA};

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
