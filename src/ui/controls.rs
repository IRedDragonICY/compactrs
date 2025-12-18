/* --- src/ui/controls.rs --- */
#![allow(unsafe_op_in_unsafe_fn)]
use windows_sys::Win32::Foundation::HWND;
use crate::ui::theme::{self, ControlType};

// Control IDs
pub const IDC_COMBO_ALGO: u16 = 105;
pub const IDC_STATIC_TEXT: u16 = 107;
pub const IDC_PROGRESS_BAR: u16 = 108;
pub const IDC_BTN_CANCEL: u16 = 109;
pub const IDC_BATCH_LIST: u16 = 110;
pub const IDC_BTN_ADD_FOLDER: u16 = 111;
pub const IDC_BTN_REMOVE: u16 = 112;
pub const IDC_BTN_PROCESS_ALL: u16 = 113;
pub const IDC_BTN_ADD_FILES: u16 = 114;
pub const IDC_BTN_SETTINGS: u16 = 115;
pub const IDC_BTN_ABOUT: u16 = 116;
pub const IDC_BTN_CONSOLE: u16 = 118;
pub const IDC_CHK_FORCE: u16 = 119;
pub const IDC_COMBO_ACTION_MODE: u16 = 120;

// Helper untuk update tema dinamis (digunakan saat switch theme)
pub unsafe fn apply_button_theme(hwnd: HWND, is_dark: bool) {
    theme::apply_theme(hwnd, ControlType::Button, is_dark);
}

pub unsafe fn apply_combobox_theme(hwnd: HWND, is_dark: bool) {
    theme::apply_theme(hwnd, ControlType::ComboBox, is_dark);
}
