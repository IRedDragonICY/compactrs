//! Central utility module for Win32 helper functions.

use windows_sys::Win32::Foundation::{HWND, HINSTANCE};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GWLP_USERDATA, LoadImageW, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED, HICON,
};
use windows_sys::Win32::UI::Shell::StrFormatByteSizeW;
pub use crate::utils::to_wstring;

// Re-export ToWide trait from new utils module for compatibility
pub use crate::utils::ToPcwstr as ToWide;

/// Safely retrieves a mutable reference to window state from GWLP_USERDATA.
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
#[inline]
pub unsafe fn load_app_icon(instance: HINSTANCE) -> HICON { unsafe {
    LoadImageW(
        instance,
        // Helper: Convert integer resource ID (1) to *const u16 using MAKEINTRESOURCE logic
        // But since we can't use MAKEINTRESOURCE macro directly easily, we just cast 1 to pointer
        1 as *const u16, 
        IMAGE_ICON,
        0, 0,
        LR_DEFAULTSIZE | LR_SHARED,
    )
}}

/// Formats a byte size into a human-readable string using the Windows Shell API.
#[inline]
pub fn format_size(bytes: u64) -> String {
    let mut buffer: [u16; 32] = [0; 32];
    
    unsafe {
        let size_i64 = if bytes > i64::MAX as u64 {
            i64::MAX
        } else {
            bytes as i64
        };
        
        let ptr = StrFormatByteSizeW(size_i64, buffer.as_mut_ptr(), buffer.len() as u32);
        
        if ptr.is_null() {
            return String::new();
        }

        // Buffer is filled with null-terminated string
        // Correctly iterate over the buffer to find the null terminator
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..len])
    }
}
