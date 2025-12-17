#![allow(unsafe_op_in_unsafe_fn)]
//! Helper module for creating UI controls.

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    HMENU, BS_AUTOCHECKBOX, WS_CHILD, WS_VISIBLE, CreateWindowExW, WS_TABSTOP
};
use windows_sys::Win32::UI::Controls::SetWindowTheme;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use crate::utils::to_wstring;

// Control IDs
pub const IDC_COMBO_ALGO: u16 = 105;
pub const IDC_STATIC_TEXT: u16 = 107;
pub const IDC_PROGRESS_BAR: u16 = 108;
pub const IDC_BTN_CANCEL: u16 = 109;

// New control IDs for batch UI
pub const IDC_BATCH_LIST: u16 = 110;
pub const IDC_BTN_ADD_FOLDER: u16 = 111;
pub const IDC_BTN_REMOVE: u16 = 112;
pub const IDC_BTN_PROCESS_ALL: u16 = 113;
pub const IDC_BTN_ADD_FILES: u16 = 114;
pub const IDC_BTN_SETTINGS: u16 = 115;
pub const IDC_BTN_ABOUT: u16 = 116;
pub const IDC_BTN_OK: u16 = 117;
pub const IDC_BTN_CONSOLE: u16 = 118;
pub const IDC_CHK_FORCE: u16 = 119;
pub const IDC_COMBO_ACTION_MODE: u16 = 120;


/// Apply button theme dynamically (for theme changes after creation)
pub unsafe fn apply_button_theme(hwnd: HWND, is_dark: bool) {
    let theme_name = if is_dark { "DarkMode_Explorer" } else { "Explorer" };
    SetWindowTheme(hwnd, to_wstring(theme_name).as_ptr(), std::ptr::null());
}

/// Apply ComboBox theme dynamically
pub unsafe fn apply_combobox_theme(hwnd: HWND, is_dark: bool) {
    let theme_name = if is_dark { "DarkMode_CFD" } else { "Explorer" };
    SetWindowTheme(hwnd, to_wstring(theme_name).as_ptr(), std::ptr::null());
}

/// Helper to create a checkbox.
///
/// # Arguments
/// * `parent` - Parent window handle
/// * `text` - Label text
/// * `x`, `y`, `w`, `h` - Position and dimensions
/// * `id` - Control ID
///
/// # Safety
/// Calls Win32 CreateWindowExW API.
pub unsafe fn create_checkbox(parent: HWND, text: &str, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
    let instance = GetModuleHandleW(std::ptr::null());
    let class_name = to_wstring("BUTTON");
    let text_wide = to_wstring(text);

    let hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        text_wide.as_ptr(),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP | BS_AUTOCHECKBOX as u32,
        x,
        y,
        w,
        h,
        parent,
        id as usize as HMENU,
        instance,
        std::ptr::null(),
    );
    
    // Apply basic theme
    let is_dark = crate::ui::theme::is_system_dark_mode();
    apply_button_theme(hwnd, is_dark);

    hwnd
}
