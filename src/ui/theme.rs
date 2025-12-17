#![allow(unsafe_op_in_unsafe_fn)]
//! Theme provider module - pure resource functions for theming.

use std::sync::OnceLock;
use windows_sys::Win32::Foundation::{COLORREF, HWND, WPARAM, LRESULT};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE};
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontW, CreateSolidBrush, FillRect, GetStockObject, 
    HBRUSH, HDC, HFONT, SetBkMode, SetTextColor, TRANSPARENT, WHITE_BRUSH,
    DEFAULT_CHARSET, DEFAULT_PITCH, FF_DONTCARE, OUT_DEFAULT_PRECIS,
    CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, FW_NORMAL,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetClientRect, WM_CTLCOLORBTN, WM_CTLCOLORSTATIC, WM_ERASEBKGND};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, RegCloseKey, RegOpenKeyExW, RegQueryValueExW, KEY_READ};
use windows_sys::Win32::UI::Controls::SetWindowTheme;

use crate::utils::to_wstring;
use crate::ui::state::AppTheme;

// ============================================================================
// Color Constants
// ============================================================================

pub const COLOR_DARK_BG: u32 = 0x001E1E1E;
pub const COLOR_DARK_TEXT: u32 = 0x00FFFFFF;
pub const COLOR_LIGHT_BG: u32 = 0x00FFFFFF;
pub const COLOR_LIGHT_TEXT: u32 = 0x00000000;

// ============================================================================
// Static Resources (OnceLock cached)
// ============================================================================

static APP_FONT_HANDLE: OnceLock<isize> = OnceLock::new();
static DARK_BRUSH_HANDLE: OnceLock<isize> = OnceLock::new();

// ============================================================================
// Pure Functions
// ============================================================================

pub fn resolve_mode(theme: AppTheme) -> bool {
    match theme {
        AppTheme::Dark => true,
        AppTheme::Light => false,
        AppTheme::System => unsafe { is_system_dark_mode() },
    }
}

pub fn get_app_font() -> HFONT {
    let handle = *APP_FONT_HANDLE.get_or_init(|| unsafe {
        let font_height = -12; // ~9pt
        let font_name = to_wstring("Segoe UI Variable Display");
        CreateFontW(
            font_height,
            0, 0, 0,
            FW_NORMAL as i32,
            0, 0, 0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            font_name.as_ptr(),
        ) as isize
    });
    handle as HFONT
}

pub fn get_dark_brush() -> HBRUSH {
    let handle = *DARK_BRUSH_HANDLE.get_or_init(|| unsafe {
        CreateSolidBrush(COLOR_DARK_BG) as isize
    });
    handle as HBRUSH
}

pub unsafe fn set_window_frame_theme(hwnd: HWND, is_dark: bool) {
    let dark_mode_val: i32 = if is_dark { 1 } else { 0 };
    
    // Dark Mode Frame (DWMWA_USE_IMMERSIVE_DARK_MODE = 20)
    let _ = DwmSetWindowAttribute(
        hwnd,
        20, // DWMWA_USE_IMMERSIVE_DARK_MODE
        &dark_mode_val as *const _ as _,
        std::mem::size_of::<i32>() as u32,
    );

    // Mica Effect (Windows 11)
    let mica = 2; // DWM_SYSTEMBACKDROP_TYPE(2)
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_SYSTEMBACKDROP_TYPE as u32,
        &mica as *const _ as _,
        std::mem::size_of::<i32>() as u32,
    );
}

// ============================================================================
// System Query Functions
// ============================================================================

pub unsafe fn is_system_dark_mode() -> bool {
    let subkey = to_wstring("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let val_name = to_wstring("AppsUseLightTheme");
    let mut hkey: HKEY = std::ptr::null_mut();
    
    if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey) == 0 {
        let mut data: u32 = 0;
        let mut cb_data = std::mem::size_of::<u32>() as u32;
        let result = RegQueryValueExW(
            hkey,
            val_name.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut data as *mut _ as *mut u8,
            &mut cb_data,
        );
        RegCloseKey(hkey);
        
        if result == 0 {
            return data == 0; // 0 = dark mode, 1 = light mode
        }
    }
    false
}

#[allow(non_snake_case)]
pub unsafe fn allow_dark_mode() {
    let uxtheme_name = to_wstring("uxtheme.dll");
    let uxtheme = LoadLibraryW(uxtheme_name.as_ptr());
    if uxtheme != std::ptr::null_mut() {
        // Ordinal 135: SetPreferredAppMode
        if let Some(set_preferred_app_mode) = GetProcAddress(uxtheme, 135 as *const u8) {
             let set_preferred_app_mode: extern "system" fn(i32) -> i32 =
                std::mem::transmute(set_preferred_app_mode);
             set_preferred_app_mode(2); // 2 = AllowDark
        }
    }
}

pub unsafe fn is_app_dark_mode(hwnd: HWND) -> bool {
    use crate::ui::state::AppState;
    use crate::ui::utils::get_window_state;

    if let Some(st) = get_window_state::<AppState>(hwnd) {
        resolve_mode(st.theme)
    } else {
        is_system_dark_mode()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

pub unsafe fn get_theme_colors(is_dark: bool) -> (HBRUSH, COLORREF, COLORREF) {
    if is_dark {
        let brush = get_dark_brush();
        (brush, COLOR_DARK_TEXT, COLOR_DARK_BG)
    } else {
        let brush = GetStockObject(WHITE_BRUSH);
        (brush, COLOR_LIGHT_TEXT, COLOR_LIGHT_BG)
    }
}

pub unsafe fn apply_control_theme(h_ctrl: HWND, is_dark: bool) {
    if is_dark {
        let empty = to_wstring("");
        SetWindowTheme(h_ctrl, empty.as_ptr(), empty.as_ptr());
    } else {
        let explorer = to_wstring("Explorer");
        let empty = to_wstring("");
        SetWindowTheme(h_ctrl, explorer.as_ptr(), empty.as_ptr());
    }
}

pub unsafe fn handle_ctl_color(
    _hwnd: HWND,
    hdc_raw: WPARAM,
    is_dark: bool,
) -> Option<LRESULT> {
    let hdc = hdc_raw as HDC;
    
    if is_dark {
        let (brush, text_col, _) = get_theme_colors(true);
        SetTextColor(hdc, text_col);
        SetBkMode(hdc, TRANSPARENT as i32);
        Some(brush as LRESULT)
    } else {
        SetTextColor(hdc, 0x00000000); // Black text
        SetBkMode(hdc, TRANSPARENT as i32);
        
        let brush = GetStockObject(WHITE_BRUSH);
        Some(brush as LRESULT)
    }
}

pub unsafe fn handle_standard_colors(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    is_dark: bool,
) -> Option<LRESULT> {
    match msg {
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            handle_ctl_color(hwnd, wparam, is_dark)
        }
        WM_ERASEBKGND => {
            let (brush, _, _) = get_theme_colors(is_dark);
            let hdc = wparam as HDC;
            let mut rc = unsafe { std::mem::zeroed() };
            GetClientRect(hwnd, &mut rc);
            FillRect(hdc, &rc, brush);
            Some(1)
        }
        _ => None,
    }
}
