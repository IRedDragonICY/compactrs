//! Theme provider module - pure resource functions for theming.
//!
//! This module provides pure functions for theme resources without coupling
//! to specific UI control IDs or application state. It's the single source
//! of truth for theme-related resources.

use std::sync::OnceLock;
use windows::core::{w, PCSTR};
use windows::Win32::Foundation::{COLORREF, HWND, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE, DWMWA_SYSTEMBACKDROP_TYPE, DWM_SYSTEMBACKDROP_TYPE};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CreateSolidBrush, GetStockObject, 
    HBRUSH, HDC, HFONT, SetBkMode, SetTextColor, TRANSPARENT, WHITE_BRUSH,
    DEFAULT_CHARSET, DEFAULT_PITCH, FF_DONTCARE, OUT_DEFAULT_PRECIS,
    CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, FW_NORMAL,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, RegCloseKey, RegOpenKeyExW, RegQueryValueExW, KEY_READ};

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

/// Cached font handle (stored as isize for thread safety)
static APP_FONT_HANDLE: OnceLock<isize> = OnceLock::new();

/// Cached dark background brush handle
static DARK_BRUSH_HANDLE: OnceLock<isize> = OnceLock::new();

// ============================================================================
// Pure Functions
// ============================================================================

/// Resolves the effective dark mode state from an AppTheme preference.
///
/// This is a pure function that takes an AppTheme and returns whether
/// dark mode should be active. If the theme is set to System, it queries
/// the Windows registry for the system preference.
///
/// # Arguments
/// * `theme` - The app's theme preference (System, Dark, or Light)
///
/// # Returns
/// `true` if dark mode should be active, `false` otherwise.
pub fn resolve_mode(theme: AppTheme) -> bool {
    match theme {
        AppTheme::Dark => true,
        AppTheme::Light => false,
        AppTheme::System => unsafe { is_system_dark_mode() },
    }
}

/// Returns the application font handle.
///
/// Creates a "Segoe UI Variable Display" font on first call and caches it.
/// Subsequent calls return the cached handle.
///
/// # Returns
/// The cached HFONT handle.
pub fn get_app_font() -> HFONT {
    let handle = *APP_FONT_HANDLE.get_or_init(|| unsafe {
        let font_height = -12; // ~9pt
        let hfont = CreateFontW(
            font_height,
            0, 0, 0,
            FW_NORMAL.0 as i32,
            0, 0, 0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            w!("Segoe UI Variable Display"),
        );
        hfont.0 as isize
    });
    HFONT(handle as *mut _)
}

/// Returns the dark mode background brush.
///
/// Creates a solid brush with color 0x202020 on first call and caches it.
///
/// # Returns
/// The cached HBRUSH handle for dark backgrounds.
pub fn get_dark_brush() -> HBRUSH {
    let handle = *DARK_BRUSH_HANDLE.get_or_init(|| unsafe {
        let brush = CreateSolidBrush(COLORREF(0x00202020));
        brush.0 as isize
    });
    HBRUSH(handle as *mut _)
}

/// Sets the window frame theme (dark/light title bar).
///
/// Applies DWM window attributes to enable dark mode frame and Mica effect.
///
/// # Arguments
/// * `hwnd` - The window handle
/// * `is_dark` - Whether to apply dark mode frame
///
/// # Safety
/// Calls Win32 DwmSetWindowAttribute API.
pub unsafe fn set_window_frame_theme(hwnd: HWND, is_dark: bool) {
    unsafe {
        let dark_mode_val: i32 = if is_dark { 1 } else { 0 };
        
        // Dark Mode Frame (DWMWA_USE_IMMERSIVE_DARK_MODE = 20)
        let dwm_dark_mode = DWMWINDOWATTRIBUTE(20);
        let _ = DwmSetWindowAttribute(
            hwnd,
            dwm_dark_mode,
            &dark_mode_val as *const _ as _,
            std::mem::size_of::<i32>() as u32,
        );

        // Mica Effect (Windows 11)
        let mica = DWM_SYSTEMBACKDROP_TYPE(2);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &mica as *const _ as _,
            std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
        );
    }
}

// ============================================================================
// System Query Functions
// ============================================================================

/// Returns true if the SYSTEM is in Dark Mode.
///
/// Queries the Windows registry for the current system theme preference.
///
/// # Safety
/// Calls Win32 registry APIs.
pub unsafe fn is_system_dark_mode() -> bool {
    unsafe {
        let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        let val_name = w!("AppsUseLightTheme");
        let mut hkey: HKEY = Default::default();
        
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, Some(0), KEY_READ, &mut hkey).is_ok() {
            let mut data: u32 = 0;
            let mut cb_data = std::mem::size_of::<u32>() as u32;
            let result = RegQueryValueExW(
                hkey,
                val_name,
                None,
                None,
                Some(&mut data as *mut _ as _),
                Some(&mut cb_data),
            );
            let _ = RegCloseKey(hkey);
            
            if result.is_ok() {
                return data == 0; // 0 = dark mode, 1 = light mode
            }
        }
        false
    }
}

/// Enables dark mode for the application using SetPreferredAppMode.
///
/// Should be called once at application startup before creating windows.
///
/// # Safety
/// Calls undocumented uxtheme APIs.
#[allow(non_snake_case)]
pub unsafe fn allow_dark_mode() {
    unsafe {
        if let Ok(uxtheme) = LoadLibraryW(w!("uxtheme.dll")) {
            // Ordinal 135: SetPreferredAppMode
            if let Some(set_preferred_app_mode) = GetProcAddress(uxtheme, PCSTR(135 as *const u8)) {
                let set_preferred_app_mode: extern "system" fn(i32) -> i32 =
                    std::mem::transmute(set_preferred_app_mode);
                set_preferred_app_mode(2); // 2 = AllowDark
            }
        }
    }
}

// ============================================================================
// Helper Functions (kept for compatibility)
// ============================================================================

/// Returns the Background Brush and Text Color for the given mode.
///
/// # Returns
/// (Brush, TextColor, BackgroundColor)
pub unsafe fn get_theme_colors(is_dark: bool) -> (HBRUSH, COLORREF, COLORREF) {
    unsafe {
        if is_dark {
            let brush = CreateSolidBrush(COLORREF(COLOR_DARK_BG));
            (brush, COLORREF(COLOR_DARK_TEXT), COLORREF(COLOR_DARK_BG))
        } else {
            let brush = HBRUSH(GetStockObject(WHITE_BRUSH).0);
            (brush, COLORREF(COLOR_LIGHT_TEXT), COLORREF(COLOR_LIGHT_BG))
        }
    }
}

/// Configures a generic control (Button, Checkbox, Radio) for the target theme.
///
/// Strips visual styles to allow WM_CTLCOLOR* to take effect for dark mode.
///
/// # Arguments
/// * `h_ctrl` - Control window handle
/// * `is_dark` - Whether to apply dark mode
///
/// # Safety
/// Calls Win32 SetWindowTheme API.
pub unsafe fn apply_control_theme(h_ctrl: HWND, is_dark: bool) {
    use windows::Win32::UI::Controls::SetWindowTheme;
    
    unsafe {
        if is_dark {
            // Strip visual styles to allow WM_CTLCOLORSTATIC to work
            let _ = SetWindowTheme(h_ctrl, w!(""), w!(""));
        } else {
            // Restore default visual styles
            let _ = SetWindowTheme(h_ctrl, w!("Explorer"), w!(""));
        }
    }
}

/// Handle WM_CTLCOLORSTATIC and WM_CTLCOLORBTN messages.
///
/// Returns Some(LRESULT) with brush if is_dark, None otherwise.
pub unsafe fn handle_ctl_color(
    _hwnd: HWND,
    hdc_raw: WPARAM,
    is_dark: bool,
) -> Option<windows::Win32::Foundation::LRESULT> {
    unsafe {
        if is_dark {
            let (brush, text_col, _) = get_theme_colors(true);
            let hdc = HDC(hdc_raw.0 as *mut _);
            SetTextColor(hdc, text_col);
            SetBkMode(hdc, TRANSPARENT);
            Some(windows::Win32::Foundation::LRESULT(brush.0 as isize))
        } else {
            None
        }
    }
}
