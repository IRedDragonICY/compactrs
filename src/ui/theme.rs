#![allow(unsafe_op_in_unsafe_fn)]
//! Theme provider module - Centralized theming resource management.
//!
//! This module owns all GDI resources (brushes, fonts) and handles safe interaction
//! with `uxtheme.dll` for advanced styling.

use std::sync::OnceLock;
use windows_sys::Win32::Foundation::{HWND, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontW, CreateSolidBrush, FillRect, GetStockObject, HBRUSH, HDC, HFONT, 
    SetBkMode, SetTextColor, TRANSPARENT, WHITE_BRUSH, DEFAULT_CHARSET, 
    DEFAULT_PITCH, FF_DONTCARE, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, 
    CLEARTYPE_QUALITY, FW_NORMAL,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, WM_CTLCOLORBTN, WM_CTLCOLORSTATIC, WM_ERASEBKGND, WM_CTLCOLORDLG, WM_SETFONT, 
    WM_CTLCOLOREDIT, SendMessageW
};

use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, RegCloseKey, RegOpenKeyExW, RegQueryValueExW, KEY_READ};
use windows_sys::Win32::UI::Controls::SetWindowTheme;

use crate::utils::to_wstring;
use crate::ui::state::AppTheme;

// ============================================================================
// Color Constants
// ============================================================================

pub const COLOR_DARK_BG: u32 = 0x001E1E1E; // Dark Gray
pub const COLOR_DARK_TEXT: u32 = 0x00FFFFFF; // White
pub const COLOR_LIGHT_BG: u32 = 0x00FFFFFF; // White
pub const COLOR_LIGHT_TEXT: u32 = 0x00000000; // Black

// List View Specifics
pub const COLOR_LIST_BG_DARK: u32 = 0x00202020;
pub const COLOR_LIST_TEXT_DARK: u32 = 0x00FFFFFF;
pub const COLOR_LIST_BG_LIGHT: u32 = 0x00FFFFFF;
pub const COLOR_LIST_TEXT_LIGHT: u32 = 0x00000000;

// Header Control
pub const COLOR_HEADER_TEXT_DARK: u32 = 0x00FFFFFF;

// ============================================================================
// Enums & Traits
// ============================================================================

/// Defines the type of control for theming purposes.
pub enum ControlType {
    Window,
    Button,
    List,
    CheckBox,
    ComboBox,
    Header,
    GroupBox,
    RadioButton, // New type for distinct styling
    ItemsView, // For specialized ListView themes
}

/// Trait for components that need to react to theme changes.
pub trait ThemeAware {
    fn on_theme_change(&mut self, is_dark: bool);
}

// ============================================================================
// Static Resources (Singleton & Global)
// ============================================================================

// Global Font Handle
static APP_FONT_HANDLE: OnceLock<isize> = OnceLock::new();

// Global Dark Background Brush
static DARK_BRUSH_HANDLE: OnceLock<isize> = OnceLock::new();

/// Structure to hold resolved Uxtheme function pointers.
struct UxthemeApi {
    allow_dark_mode: Option<extern "system" fn(HWND, bool) -> bool>,
    set_preferred_app_mode: Option<extern "system" fn(i32) -> i32>,
}

// Singleton for Uxtheme DLL loading
static UXTHEME_API: OnceLock<UxthemeApi> = OnceLock::new();

// ============================================================================
// Core Styling Functions
// ============================================================================

/// Applies the visual theme to a specific control or window.
pub unsafe fn apply_theme(hwnd: HWND, control_type: ControlType, is_dark: bool) {
    let (theme, sub_theme) = if is_dark {
        match control_type {
            ControlType::Window => ("DarkMode_Explorer", None),
            ControlType::Button => ("DarkMode_Explorer", None),
            ControlType::List => ("DarkMode_Explorer", None), 
            ControlType::CheckBox => ("DarkMode_Explorer", None),
            ControlType::ComboBox => ("DarkMode_CFD", None),
            ControlType::Header => ("DarkMode_ItemsView", None), 
            // GroupBox and RadioButton: use empty theme in dark mode to force GDI rendering
            // This ensures WM_CTLCOLORSTATIC text color (white) is respected.
            ControlType::GroupBox => ("", None),
            ControlType::RadioButton => ("", None),
            ControlType::ItemsView => ("DarkMode_ItemsView", None),
        }
    } else {
        match control_type {
            ControlType::Window => ("Explorer", None),
            ControlType::Button => ("Explorer", None),
            ControlType::List => ("Explorer", None),
            ControlType::CheckBox => ("Explorer", None),
            ControlType::ComboBox => ("Explorer", None),
            ControlType::Header => ("Explorer", None),
            ControlType::GroupBox => ("Explorer", None),
            ControlType::RadioButton => ("Explorer", None),
            ControlType::ItemsView => ("Explorer", None),
        }
    };

    let theme_w = to_wstring(theme);
    let sub_theme_w = sub_theme.map(to_wstring); 
    
    let psz_sub_app_name = if let Some(ref s) = sub_theme_w {
        s.as_ptr()
    } else {
         // To disable visual styles, passing L"" as the second parameter is often required.
         if theme.is_empty() {
             theme_w.as_ptr()
         } else {
             std::ptr::null()
         }
    };
    
    SetWindowTheme(hwnd, theme_w.as_ptr(), psz_sub_app_name);
}

/// Applies the application font to the specified window.
pub fn apply_font(hwnd: HWND) {
    let font = get_app_font();
    unsafe {
        SendMessageW(hwnd, WM_SETFONT, font as WPARAM, 1);
    }
}

/// Helper to get the global background brush.
pub fn get_background_brush(is_dark: bool) -> HBRUSH {
    if is_dark {
        get_dark_brush()
    } else {
        unsafe { GetStockObject(WHITE_BRUSH) }
    }
}

/// Enables Dark Mode for a specific window handle using undocumented API.
pub fn allow_dark_mode_for_window(hwnd: HWND, allow: bool) {
    let api = UXTHEME_API.get_or_init(|| init_uxtheme());
    if let Some(func) = api.allow_dark_mode {
        func(hwnd, allow);
    }
}

/// Sets the preferred app mode (Global dark mode setting).
pub fn set_preferred_app_mode(allow_dark: bool) {
    let api = UXTHEME_API.get_or_init(|| init_uxtheme());
    if let Some(func) = api.set_preferred_app_mode {
        // 2 = AllowDark, 0 = Default
        let mode = if allow_dark { 2 } else { 0 }; 
        func(mode);
    }
}

// ============================================================================
// Internal Initializers
// ============================================================================

fn init_uxtheme() -> UxthemeApi {
    unsafe {
        let uxtheme_name = to_wstring("uxtheme.dll");
        let uxtheme = LoadLibraryW(uxtheme_name.as_ptr());
        
        if uxtheme == std::ptr::null_mut() {
            return UxthemeApi { allow_dark_mode: None, set_preferred_app_mode: None };
        }

        let allow_dark_mode = GetProcAddress(uxtheme, 133 as *const u8).map(|f| {
             std::mem::transmute(f)
        });

        let set_preferred_app_mode = GetProcAddress(uxtheme, 135 as *const u8).map(|f| {
             std::mem::transmute(f)
        });

        UxthemeApi {
            allow_dark_mode,
            set_preferred_app_mode,
        }
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

fn get_dark_brush() -> HBRUSH {
    let handle = *DARK_BRUSH_HANDLE.get_or_init(|| unsafe {
        CreateSolidBrush(COLOR_DARK_BG) as isize
    });
    handle as HBRUSH
}

// ============================================================================
// Message Handlers
// ============================================================================

pub unsafe fn handle_standard_colors(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    is_dark: bool,
) -> Option<LRESULT> {
    match msg {
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLORDLG | WM_CTLCOLOREDIT => {
            let hdc = wparam as HDC;
            if is_dark {
                SetTextColor(hdc, COLOR_DARK_TEXT);
                SetBkMode(hdc, TRANSPARENT as i32);
                Some(get_dark_brush() as LRESULT)
            } else {
                SetTextColor(hdc, COLOR_LIGHT_TEXT);
                SetBkMode(hdc, TRANSPARENT as i32);
                Some(unsafe { GetStockObject(WHITE_BRUSH) } as LRESULT)
            }
        }
        WM_ERASEBKGND => {
            let brush = get_background_brush(is_dark);
            let hdc = wparam as HDC;
            let mut rc = unsafe { std::mem::zeroed() };
            GetClientRect(hwnd, &mut rc);
            FillRect(hdc, &rc, brush);
            Some(1)
        }
        _ => None,
    }
}

pub unsafe fn set_window_frame_theme(hwnd: HWND, is_dark: bool) {
    let dark_mode_val: i32 = if is_dark { 1 } else { 0 };
    
    // Dark Mode Frame (DWMWA_USE_IMMERSIVE_DARK_MODE = 20)
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
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

pub unsafe fn is_app_dark_mode(hwnd: HWND) -> bool {
    use crate::ui::state::AppState;
    use crate::ui::utils::get_window_state;

    if let Some(st) = get_window_state::<AppState>(hwnd) {
        crate::ui::theme::resolve_mode(st.theme)
    } else {
        is_system_dark_mode()
    }
}

pub fn resolve_mode(theme: AppTheme) -> bool {
    match theme {
        AppTheme::Dark => true,
        AppTheme::Light => false,
        AppTheme::System => unsafe { is_system_dark_mode() },
    }
}

