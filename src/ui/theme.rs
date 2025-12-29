#![allow(unsafe_op_in_unsafe_fn)]
//! Theme provider module - Centralized theming resource management.
//!
//! This module owns all GDI resources (brushes, fonts) and handles safe interaction
//! with `uxtheme.dll` for advanced styling.

use std::sync::OnceLock;
use crate::types::*;
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlType {
    Window,
    Button,
    AccentButton, // Windows 11 Fluent blue accent button
    List,
    CheckBox,
    ComboBox,
    Header,
    GroupBox,
    RadioButton, // New type for distinct styling
    ItemsView, // For specialized ListView themes
    Trackbar, 
    Edit,
    ProgressBar,
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
            ControlType::AccentButton => ("DarkMode_Explorer", None),
            ControlType::List => ("DarkMode_Explorer", None), 
            ControlType::CheckBox => ("DarkMode_Explorer", None),
            ControlType::ComboBox => ("DarkMode_CFD", None),
            ControlType::Header => ("DarkMode_ItemsView", None), 
            // GroupBox: native for readable text. 
            // RadioButton: Explorer for Blue Accent (text must be handled via separate label).
            ControlType::GroupBox => ("", None),
            ControlType::RadioButton => ("Explorer", None),
            ControlType::ItemsView => ("DarkMode_ItemsView", None),
            ControlType::Trackbar => ("", None), // Use default drawing but with dark background
            ControlType::Edit => ("DarkMode_Explorer", None),
            ControlType::ProgressBar => ("", None),
        }
    } else {
        match control_type {
            ControlType::Window => ("Explorer", None),
            ControlType::Button => ("Explorer", None),
            ControlType::AccentButton => ("Explorer", None),
            ControlType::List => ("Explorer", None),
            ControlType::CheckBox => ("Explorer", None),
            ControlType::ComboBox => ("Explorer", None),
            ControlType::Header => ("Explorer", None),
            ControlType::GroupBox => ("Explorer", None),
            ControlType::RadioButton => ("Explorer", None),
            ControlType::ItemsView => ("Explorer", None),
            ControlType::Trackbar => ("Explorer", None),
            ControlType::Edit => ("Explorer", None),
            ControlType::ProgressBar => ("Explorer", None),
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
    
    // IMPORTANT: Allow Dark Mode *BEFORE* setting the theme.
    // This ensures that when SetWindowTheme triggers a resource load, it sees the dark mode flag.
    allow_dark_mode_for_window(hwnd, is_dark);
    
    SetWindowTheme(hwnd, theme_w.as_ptr(), psz_sub_app_name);

    if is_dark {
        // Force the control to update its theme data immediately
        SendMessageW(hwnd, WM_THEMECHANGED, 0, 0);
    }
}

/// Applies flat (borderless, no grid lines) theme to a ListView control.
/// This is the centralized helper for modern Windows 11 style ListViews.
/// Both FileListView and wrapper ListViews should use this.
pub unsafe fn apply_flat_listview_theme(hwnd: HWND, is_dark: bool) {
    // Enable dark mode for the window FIRST
    allow_dark_mode_for_window(hwnd, is_dark);
    
    // Remove borders for flat look
    let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
    SetWindowLongW(hwnd, GWL_STYLE, (style & !WS_BORDER) as i32);
    
    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    SetWindowLongW(hwnd, GWL_EXSTYLE, (ex_style & !(WS_EX_CLIENTEDGE | WS_EX_STATICEDGE)) as i32);
    
    // Force frame update
    SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 0, 0, 
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);
    
    // Use empty theme to disable visual styles that add grid lines/dividers
    // This gives us a clean flat look without column separators
    let empty = crate::w!("");
    SetWindowTheme(hwnd, empty.as_ptr(), std::ptr::null());
    
    // Apply colors
    let (bg, text) = if is_dark {
        (COLOR_LIST_BG_DARK, COLOR_LIST_TEXT_DARK)
    } else {
        (COLOR_LIST_BG_LIGHT, COLOR_LIST_TEXT_LIGHT)
    };
    
    SendMessageW(hwnd, LVM_SETBKCOLOR, 0, bg as isize);
    SendMessageW(hwnd, LVM_SETTEXTCOLOR, 0, text as isize);
    SendMessageW(hwnd, LVM_SETTEXTBKCOLOR, 0, bg as isize);
    
    // Apply flat theme to header (uses custom draw for no dividers)
    let h_header = SendMessageW(hwnd, LVM_GETHEADER, 0, 0) as HWND;
    if !h_header.is_null() {
        allow_dark_mode_for_window(h_header, is_dark);
        SetWindowTheme(h_header, empty.as_ptr(), std::ptr::null());
        InvalidateRect(h_header, std::ptr::null(), 1);
    }
}

/// Constants for header custom draw
const CDDS_POSTPAINT: u32 = 0x00000002;
const CDRF_NOTIFYPOSTPAINT: u32 = 0x00000010;
const HDM_GETITEMW: u32 = 0x1200 + 11;
const HDI_TEXT: u32 = 0x0002;
const DT_FLAGS: u32 = 0x20 | 0x4 | 0x8000; // DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS

#[repr(C)]
struct HDITEMW {
    mask: u32, cxy: i32, psz_text: *mut u16, hbm: isize,
    cch_text_max: i32, fmt: i32, l_param: isize, i_image: i32,
    i_order: i32, type_: u32, pv_filter: *mut std::ffi::c_void, state: u32,
}

/// Handles NM_CUSTOMDRAW for header controls to achieve flat look without dividers.
/// Returns Some(LRESULT) if handled, None if should use default processing.
/// This is the centralized helper - both FileListView and wrapper ListViews use this.
pub unsafe fn handle_flat_header_customdraw(
    header_hwnd: HWND,
    nmcd: &mut NMCUSTOMDRAW,
    is_dark: bool,
) -> Option<LRESULT> {
    let bg_color = if is_dark { COLOR_LIST_BG_DARK } else { COLOR_LIST_BG_LIGHT };
    
    if nmcd.dwDrawStage == CDDS_PREPAINT {
        return Some((CDRF_NOTIFYITEMDRAW | CDRF_NOTIFYPOSTPAINT) as LRESULT);
    }
    
    if nmcd.dwDrawStage == CDDS_POSTPAINT {
        // Cover bottom border
        let mut header_rect: RECT = std::mem::zeroed();
        GetClientRect(header_hwnd, &mut header_rect);
        
        let bottom_rect = RECT {
            left: header_rect.left,
            top: header_rect.bottom - 2,
            right: header_rect.right,
            bottom: header_rect.bottom,
        };
        
        let brush = CreateSolidBrush(bg_color);
        FillRect(nmcd.hdc, &bottom_rect, brush);
        DeleteObject(brush as *mut std::ffi::c_void);
        return Some(CDRF_SKIPDEFAULT as LRESULT);
    }
    
    if nmcd.dwDrawStage == CDDS_ITEMPREPAINT {
        let hdc = nmcd.hdc;
        let rc = nmcd.rc;
        
        // Fill background
        let brush = CreateSolidBrush(bg_color);
        FillRect(hdc, &rc, brush);
        DeleteObject(brush as *mut std::ffi::c_void);
        
        // Get header text
        let mut text_buf = [0u16; 64];
        let mut hdi: HDITEMW = std::mem::zeroed();
        hdi.mask = HDI_TEXT;
        hdi.psz_text = text_buf.as_mut_ptr();
        hdi.cch_text_max = 64;
        SendMessageW(header_hwnd, HDM_GETITEMW, nmcd.dwItemSpec, &mut hdi as *mut _ as isize);
        
        // Set colors and draw text
        let text_color = if is_dark { COLOR_HEADER_TEXT_DARK } else { COLOR_LIST_TEXT_LIGHT };
        SetTextColor(hdc, text_color);
        SetBkMode(hdc, TRANSPARENT as i32);
        
        let mut text_rc = rc;
        text_rc.left += 8;
        text_rc.right -= 4;
        
        let text_len = text_buf.iter().position(|&c| c == 0).unwrap_or(0) as i32;
        DrawTextW(hdc, text_buf.as_ptr(), text_len, &mut text_rc, DT_FLAGS);
        
        return Some(CDRF_SKIPDEFAULT as LRESULT);
    }
    
    None
}

/// Handles NM_CUSTOMDRAW for ListView body to cover column divider lines.
/// Call this AFTER default processing to paint over the theme's dividers.
/// Returns Some(LRESULT) if handled (POSTPAINT), None otherwise.
pub unsafe fn handle_listview_column_dividers(
    listview_hwnd: HWND,
    nmcd: &NMCUSTOMDRAW,
    is_dark: bool,
) -> Option<LRESULT> {
    // Only handle POSTPAINT to draw over theme's column dividers
    if nmcd.dwDrawStage == CDDS_POSTPAINT {
        let bg_color = if is_dark { COLOR_LIST_BG_DARK } else { COLOR_LIST_BG_LIGHT };
        let brush = CreateSolidBrush(bg_color);
        
        let h_header = SendMessageW(listview_hwnd, LVM_GETHEADER, 0, 0) as HWND;
        if !h_header.is_null() {
            let col_count = SendMessageW(h_header, 0x1200, 0, 0) as i32; // HDM_GETITEMCOUNT
            
            // Get ListView client rect
            let mut lv_rect: RECT = std::mem::zeroed();
            GetClientRect(listview_hwnd, &mut lv_rect);
            
            // Get header height to know where ListView body starts
            let mut header_rect: RECT = std::mem::zeroed();
            GetWindowRect(h_header, &mut header_rect);
            let header_height = header_rect.bottom - header_rect.top;
            
            // Draw vertical bars at each column boundary to cover dividers
            let mut x_pos = 0i32;
            for col in 0..col_count {
                // Get column width using LVM_GETCOLUMNWIDTH
                let col_width = SendMessageW(listview_hwnd, 0x101D, col as usize, 0) as i32;
                x_pos += col_width;
                
                // Draw a 2-pixel wide bar at the column boundary
                let divider_rect = RECT {
                    left: x_pos - 1,
                    top: header_height,
                    right: x_pos + 1,
                    bottom: lv_rect.bottom,
                };
                FillRect(nmcd.hdc, &divider_rect, brush);
            }
        }
        
        DeleteObject(brush as *mut std::ffi::c_void);
        return Some(0);
    }
    
    None
}

/// Recursively applies theme to child controls.
pub unsafe fn apply_theme_recursive(parent: HWND, is_dark: bool) {
    let mut child = GetWindow(parent, GW_CHILD);
    while child != std::ptr::null_mut() {
        // Apply to child
        apply_theme_to_child(child, is_dark);
        
        // Recurse to children's children
        apply_theme_recursive(child, is_dark);
        
        child = GetWindow(child, GW_HWNDNEXT);
    }
}

unsafe fn apply_theme_to_child(hwnd: HWND, is_dark: bool) {
     let mut name_buf = [0u16; 256];
     let len = GetClassNameW(hwnd, name_buf.as_mut_ptr(), 256);
     let class_name = String::from_utf16_lossy(&name_buf[..len as usize]).to_lowercase();
     
     // Special handling for ListView - use flat theme helper
     if class_name == "syslistview32" {
         apply_flat_listview_theme(hwnd, is_dark);
         return;
     }
     
     // Skip header - already handled by apply_flat_listview_theme
     if class_name == "sysheader32" {
         return;
     }
     
     let ctype = if class_name == "button" {
         let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
         let bs_typ = style & 0xF; 
         if bs_typ == BS_GROUPBOX as u32 {
             ControlType::GroupBox
         } else if bs_typ == BS_CHECKBOX as u32 || bs_typ == BS_AUTOCHECKBOX as u32 || bs_typ == BS_3STATE as u32 || bs_typ == BS_AUTO3STATE as u32 {
             ControlType::CheckBox
         } else if bs_typ == BS_RADIOBUTTON as u32 || bs_typ == BS_AUTORADIOBUTTON as u32 {
             ControlType::RadioButton
         } else {
             ControlType::Button
         }
     } else if class_name == "combobox" {
         ControlType::ComboBox
     } else if class_name == "msctls_trackbar32" {
         ControlType::Trackbar
     } else if class_name == "edit" {
         ControlType::Edit
     } else if class_name == "msctls_progress32" {
         ControlType::ProgressBar
     } else {
         return; // Unknown or ignored
     };
     
     apply_theme(hwnd, ctype, is_dark);
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

/// Initialize Uxtheme hooks explicitly. Code can call this early to ensure hooks are ready.
pub fn init() {
    UXTHEME_API.get_or_init(|| init_uxtheme());
}

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

pub fn get_dark_brush() -> HBRUSH {
    let handle = *DARK_BRUSH_HANDLE.get_or_init(|| unsafe {
        CreateSolidBrush(COLOR_DARK_BG) as isize
    });
    handle as HBRUSH
}

// Global Icon Font Handle (Segoe Fluent Icons)
static ICON_FONT_HANDLE: OnceLock<isize> = OnceLock::new();

pub fn get_icon_font() -> HFONT {
    let handle = *ICON_FONT_HANDLE.get_or_init(|| unsafe {
        let font_height = -16; // Slightly larger for icons
        let font_name = to_wstring("Segoe Fluent Icons");
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
        WM_CTLCOLOREDIT => {
             let hdc = wparam as HDC;
             if is_dark {
                  SetTextColor(hdc, COLOR_DARK_TEXT);
                  SetBkColor(hdc, COLOR_DARK_BG); // Solid background for text
                  SetBkMode(hdc, OPAQUE as i32);  // Ensure solid background is used
                 Some(get_dark_brush() as LRESULT)
             } else {
                 SetTextColor(hdc, COLOR_LIGHT_TEXT);
                 SetBkMode(hdc, TRANSPARENT as i32);
                 Some(unsafe { GetStockObject(WHITE_BRUSH) } as LRESULT)
             }
        },
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLORDLG => {
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
    use crate::ui::framework::get_window_state;

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

