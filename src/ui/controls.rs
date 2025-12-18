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
pub const IDC_LBL_ACTION_MODE: u16 = 121;
pub const IDC_LBL_ALGO: u16 = 122;
pub const IDC_LBL_INPUT: u16 = 123;

// Helper untuk update tema dinamis (digunakan saat switch theme)
pub unsafe fn apply_button_theme(hwnd: HWND, is_dark: bool) {
    theme::apply_theme(hwnd, ControlType::Button, is_dark);
}

pub unsafe fn apply_combobox_theme(hwnd: HWND, is_dark: bool) {
    theme::apply_theme(hwnd, ControlType::ComboBox, is_dark);
}

/// Applies Windows 11 Fluent blue accent styling to a button.
/// Uses owner-draw for custom painting with accent blue color.
pub unsafe fn apply_accent_button_theme(hwnd: HWND, _is_dark: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowLongW, SetWindowLongW, GWL_STYLE, BS_OWNERDRAW};
    use windows_sys::Win32::Graphics::Gdi::InvalidateRect;
    
    // Change to owner-draw style for custom painting
    let style = GetWindowLongW(hwnd, GWL_STYLE);
    // Remove BS_PUSHBUTTON (0x00) and other BS_ styles that conflict, add BS_OWNERDRAW
    let new_style = (style & !0x0F) | BS_OWNERDRAW as i32;
    SetWindowLongW(hwnd, GWL_STYLE, new_style);
    
    // Force redraw
    InvalidateRect(hwnd, std::ptr::null(), 1);
}

/// Windows 11 Fluent accent blue color (0x0078D4 in RGB, but GDI uses BGR)
pub const COLOR_ACCENT_BLUE: u32 = 0x00D47800; // BGR format for GDI
pub const COLOR_ACCENT_BLUE_HOVER: u32 = 0x00E08800; // Lighter blue for hover
pub const COLOR_ACCENT_BLUE_PRESSED: u32 = 0x00C06000; // Darker blue for pressed

/// Draws an accent button with Windows 11 Fluent blue style.
/// Call this from WM_DRAWITEM handler.
pub unsafe fn draw_accent_button(lparam: isize) {
    use windows_sys::Win32::UI::Controls::{DRAWITEMSTRUCT, ODS_SELECTED};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateSolidBrush, DeleteObject, SetBkMode, SetTextColor, 
        SelectObject, TRANSPARENT, RoundRect, CreatePen, PS_SOLID,
    };
    
    let dis = &*(lparam as *const DRAWITEMSTRUCT);
    
    // Determine button state
    let is_pressed = (dis.itemState & ODS_SELECTED) != 0;
    
    // Choose color based on state
    let bg_color = if is_pressed {
        COLOR_ACCENT_BLUE_PRESSED
    } else {
        COLOR_ACCENT_BLUE
    };
    
    // Create rounded rect brush and pen
    let brush = CreateSolidBrush(bg_color);
    let pen = CreatePen(PS_SOLID as i32, 1, bg_color);
    let old_brush = SelectObject(dis.hDC, brush);
    let old_pen = SelectObject(dis.hDC, pen);
    
    // Draw rounded rectangle (Windows 11 style with 4px radius)
    RoundRect(dis.hDC, dis.rcItem.left, dis.rcItem.top, 
              dis.rcItem.right, dis.rcItem.bottom, 8, 8);
    
    // Restore and cleanup
    SelectObject(dis.hDC, old_brush);
    SelectObject(dis.hDC, old_pen);
    DeleteObject(brush);
    DeleteObject(pen);
    
    // Draw text
    SetBkMode(dis.hDC, TRANSPARENT as i32);
    SetTextColor(dis.hDC, 0x00FFFFFF); // White text
    
    // Get button text
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    let mut text_buf: [u16; 64] = [0; 64];
    let text_len = GetWindowTextW(dis.hwndItem, text_buf.as_mut_ptr(), 64);
    
    if text_len > 0 {
        use windows_sys::Win32::Graphics::Gdi::{DrawTextW, DT_CENTER, DT_VCENTER, DT_SINGLELINE};
        let mut rc = dis.rcItem;
        DrawTextW(dis.hDC, text_buf.as_ptr(), text_len, &mut rc, 
                  DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    }
}

