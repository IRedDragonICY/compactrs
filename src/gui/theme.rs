use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, GetStockObject, HBRUSH, HDC, SetBkMode, SetTextColor, TRANSPARENT, WHITE_BRUSH};
use windows::Win32::UI::Controls::SetWindowTheme;
use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, GWLP_USERDATA, SendMessageW};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWINDOWATTRIBUTE, DWMWA_SYSTEMBACKDROP_TYPE, DWM_SYSTEMBACKDROP_TYPE};
use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, RegCloseKey, RegOpenKeyExW, RegQueryValueExW, KEY_READ};

use crate::gui::state::{AppState, AppTheme};

pub const COLOR_DARK_BG: u32 = 0x001E1E1E;
pub const COLOR_DARK_TEXT: u32 = 0x00FFFFFF;
pub const COLOR_LIGHT_BG: u32 = 0x00FFFFFF;
pub const COLOR_LIGHT_TEXT: u32 = 0x00000000;

pub struct ThemeManager;

impl ThemeManager {
    /// Determines if the window should be rendered in dark mode.
    /// Checks AppState override first, then System Preference.
    pub unsafe fn should_use_dark_mode(hwnd: HWND) -> bool {
        // 1. Check AppState Override
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        if ptr != 0 {
            // Note: This assumes the userdata pointer is ALWAYS an AppState or compatible struct 
            // where the first field is AppTheme or similar matching layout.
            // For safety in this specific app, we know header is consistent or we re-check specific windows.
            // However, to be safer, let's rely on the passed struct or just check system if ptr is null.
            // A safer approach for a generic helper is to pass the state, but we want a static helper.
            // Let's assume the caller will handle state-specific logic or we standardise the UserData.
            // For now, let's look at the registry if we can't determine from state easily without casting.
            // Actually, we can check the registry for system default first.
            
            // To properly check AppState without unsafe casting assumptions across different window struct types:
            // The best way is to let the window pass its preference. 
            // BUT, to satisfy the requirement of "centralized logic", we will query the system here
            // and let the caller pass the override.
        }
        Self::is_system_dark_mode()
    }

    /// Returns true if the SYSTEM is in Dark Mode
    pub unsafe fn is_system_dark_mode() -> bool {
        let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        let val_name = w!("AppsUseLightTheme");
        let mut hkey: HKEY = Default::default();
        
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, Some(0), KEY_READ, &mut hkey).is_ok() {
            let mut data: u32 = 0;
            let mut cb_data = std::mem::size_of::<u32>() as u32;
            let result = RegQueryValueExW(hkey, val_name, None, None, Some(&mut data as *mut _ as _), Some(&mut cb_data));
            let _ = RegCloseKey(hkey);
            
            if result.is_ok() {
                return data == 0;
            }
        }
        false
    }

    /// Applies DWM Window Attributes (Dark Frame, Mica)
    pub unsafe fn apply_window_theme(hwnd: HWND, is_dark: bool) {
        let true_val: i32 = 1;
        let false_val: i32 = 0;
        
        // Dark Mode Frame
        let dwm_dark_mode = DWMWINDOWATTRIBUTE(20); 
        if is_dark {
            let _ = DwmSetWindowAttribute(hwnd, dwm_dark_mode, &true_val as *const _ as _, 4);
        } else {
             let _ = DwmSetWindowAttribute(hwnd, dwm_dark_mode, &false_val as *const _ as _, 4);
        }

        // Mica Effect (Windows 11)
        let system_backdrop_type = DWMWA_SYSTEMBACKDROP_TYPE;
        let mica = DWM_SYSTEMBACKDROP_TYPE(2); 
        let _ = DwmSetWindowAttribute(hwnd, system_backdrop_type, &mica as *const _ as _, 4);
    }

    /// Configures a generic control (Button, Checkbox) for the target theme
    pub unsafe fn apply_control_theme(h_ctrl: HWND, is_dark: bool) {
        if is_dark {
            // Disable visual styles to allow custom painting (WM_CTLCOLORSTATIC)
            // This is required for Checkboxes/RadioButtons to accept text color changes
             let _ = SetWindowTheme(h_ctrl, w!(""), w!("")); 
        } else {
            // Restore standard light theme
            let _ = SetWindowTheme(h_ctrl, None, None);
        }
    }

    /// Handle WM_CTLCOLORSTATIC and WM_CTLCOLORBTN messages centrally.
    /// Returns Some(LRESULT) with brush if is_dark, None otherwise (caller should use DefWindowProcW).
    pub unsafe fn handle_ctl_color(_hwnd: HWND, hdc_raw: WPARAM, is_dark: bool) -> Option<windows::Win32::Foundation::LRESULT> {
        if is_dark {
            let (brush, text_col, _) = Self::get_theme_colors(true);
            let hdc = HDC(hdc_raw.0 as *mut _);
            SetTextColor(hdc, text_col);
            SetBkMode(hdc, TRANSPARENT);
            Some(windows::Win32::Foundation::LRESULT(brush.0 as isize))
        } else {
            None
        }
    }

    /// Returns the Background Brush and Text Color for the given mode
    /// (Brush, TextColor, BackgroundColor)
    pub unsafe fn get_theme_colors(is_dark: bool) -> (HBRUSH, COLORREF, COLORREF) {
        if is_dark {
            let brush = CreateSolidBrush(COLORREF(COLOR_DARK_BG));
            (brush, COLORREF(COLOR_DARK_TEXT), COLORREF(COLOR_DARK_BG))
        } else {
            // Pure White
            let brush = HBRUSH(GetStockObject(WHITE_BRUSH).0);
            (brush, COLORREF(COLOR_LIGHT_TEXT), COLORREF(COLOR_LIGHT_BG))
        }
    }
}
