use windows::Win32::Foundation::{HWND, HINSTANCE};
use windows::Win32::UI::WindowsAndMessaging::{
    HMENU,
    CreateWindowExW, 
    WS_CHILD, WS_VISIBLE, WS_TABSTOP,
    BS_AUTOCHECKBOX,
};
use windows::Win32::UI::Controls::SetWindowTheme;
use windows::core::{w, PCWSTR};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;


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
pub unsafe fn apply_button_theme(hwnd: HWND, is_dark: bool) { unsafe {
    if is_dark {
        let _ = SetWindowTheme(hwnd, w!("DarkMode_Explorer"), None);
    } else {
        let _ = SetWindowTheme(hwnd, w!("Explorer"), None);
    }
}}

/// Apply ComboBox theme dynamically
pub unsafe fn apply_combobox_theme(hwnd: HWND, is_dark: bool) { unsafe {
    if is_dark {
        let _ = SetWindowTheme(hwnd, w!("DarkMode_CFD"), None);
    } else {
        let _ = SetWindowTheme(hwnd, w!("Explorer"), None);
    }
}}





pub unsafe fn create_checkbox(parent: HWND, text: PCWSTR, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
    unsafe {
        let module = GetModuleHandleW(None).unwrap();
        let instance = HINSTANCE(module.0);
        let hwnd = CreateWindowExW(
            Default::default(),
            w!("BUTTON"),
            text,
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | WS_TABSTOP.0 | BS_AUTOCHECKBOX as u32),
            x, y, w, h,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            Some(instance),
            None
        ).unwrap_or_default();
        hwnd
    }
}
