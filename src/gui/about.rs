use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, RegisterClassW, SendMessageW,
    CS_HREDRAW, CS_VREDRAW, IDC_ARROW, WM_DESTROY, WNDCLASSW, WM_SETFONT,
    WS_VISIBLE, WM_CREATE,
    WS_CHILD, WS_CAPTION, WS_SYSMENU, WS_POPUP,
    GetMessageW, TranslateMessage, DispatchMessageW, MSG,
    PostQuitMessage, WM_CLOSE, DestroyWindow, 
    WM_NOTIFY, GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA,
    WM_CTLCOLORSTATIC, WM_ERASEBKGND, GetClientRect,
    STM_SETICON, LoadImageW, IMAGE_ICON, LR_DEFAULTCOLOR,
};
use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    NMHDR, NM_CLICK, NM_RETURN, NMLINK, WC_LINK, ICC_LINK_CLASS, INITCOMMONCONTROLSEX, InitCommonControlsEx,
    SetWindowTheme,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::Win32::Graphics::Gdi::{
    HBRUSH, COLOR_WINDOW, HDC, DeleteObject, HGDIOBJ, FillRect,
    CreateFontW, FW_BOLD, FW_NORMAL, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, 
    DEFAULT_PITCH, FF_DONTCARE, FW_LIGHT,
};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
// Removed: create_button, ButtonOpts, IDC_BTN_OK - OK button removed from About dialog
use crate::gui::utils::get_window_state;

const ABOUT_CLASS_NAME: PCWSTR = w!("CompactRS_About");
const ABOUT_TITLE: PCWSTR = w!("About CompactRS");
const GITHUB_URL: PCWSTR = w!("https://github.com/IRedDragonICY/compactrs");
const LICENSE_URL: PCWSTR = w!("https://github.com/IRedDragonICY/compactrs/blob/main/LICENSE");
const SS_CENTER: u32 = 1;
const SS_ICON: u32 = 0x3;
const SS_REALSIZEIMAGE: u32 = 0x800;

struct AboutState {
    is_dark: bool,
    dark_brush: Option<HBRUSH>,
}

impl Drop for AboutState {
    fn drop(&mut self) {
        if let Some(brush) = self.dark_brush {
            unsafe {
                DeleteObject(HGDIOBJ(brush.0));
            }
        }
    }
}

pub unsafe fn show_about_modal(parent: HWND, is_dark: bool) {
    unsafe {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(about_wnd_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: ABOUT_CLASS_NAME,
            hIcon: {
                let h = LoadImageW(
                    Some(instance.into()), 
                    PCWSTR(1 as *const u16), 
                    IMAGE_ICON, 
                    0, 0, 
                    windows::Win32::UI::WindowsAndMessaging::LR_DEFAULTSIZE | windows::Win32::UI::WindowsAndMessaging::LR_SHARED
                ).unwrap_or_default();
                windows::Win32::UI::WindowsAndMessaging::HICON(h.0)
            },
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize as *mut _),
            ..Default::default()
        };
        RegisterClassW(&wc);

        // Calculate center position
        let mut rect = windows::Win32::Foundation::RECT::default();
        windows::Win32::UI::WindowsAndMessaging::GetWindowRect(parent, &mut rect).unwrap_or_default();
        let p_width = rect.right - rect.left;
        let p_height = rect.bottom - rect.top;
        let width = 450;  // Wider for better text display
        let height = 500; // Reduced height (no OK button)
        let x = rect.left + (p_width - width) / 2;
        let y = rect.top + (p_height - height) / 2;

        let mut state = AboutState {
            is_dark,
            dark_brush: None,
        };

        let _hwnd = CreateWindowExW(
            Default::default(),
            ABOUT_CLASS_NAME,
            ABOUT_TITLE,
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            x, y, width, height,
            Some(parent),
            None,
            Some(instance.into()),
            Some(&mut state as *mut _ as *mut _),
        ).unwrap_or_default();

        // Non-modal: DON'T disable parent window
        // EnableWindow(parent, false);
        
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        // Non-modal: DON'T re-enable parent window
        // EnableWindow(parent, true);
    }
}

unsafe extern "system" fn about_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        // Use centralized helper for state access
        let get_state = || get_window_state::<AboutState>(hwnd);

        match msg {
            WM_CTLCOLORSTATIC => {
                if let Some(st) = get_state() {
                    if let Some(result) = crate::gui::theme::ThemeManager::handle_ctl_color(hwnd, wparam, st.is_dark) {
                        return result;
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            },
            WM_ERASEBKGND => {
                if let Some(st) = get_state() {
                    let is_dark = st.is_dark;
                    let (brush, _, _) = crate::gui::theme::ThemeManager::get_theme_colors(is_dark);
                    
                    let hdc = HDC(wparam.0 as *mut _);
                    let mut rc = windows::Win32::Foundation::RECT::default();
                    GetClientRect(hwnd, &mut rc);
                    FillRect(hdc, &rc, brush);
                    return LRESULT(1);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            },
            // WM_APP + 2: Theme change broadcast from Settings
            0x8002 => {
                if let Some(st) = get_state() {
                    let new_is_dark = wparam.0 == 1;
                    st.is_dark = new_is_dark;
                    
                    // Delete old brush if switching themes
                    if let Some(brush) = st.dark_brush.take() {
                        DeleteObject(HGDIOBJ(brush.0));
                    }
                    
                    // Update DWM title bar
                    let dark_mode: u32 = if new_is_dark { 1 } else { 0 };
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_USE_IMMERSIVE_DARK_MODE,
                        &dark_mode as *const u32 as *const _,
                        4
                    );
                    
                    // Force repaint
                    windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, true);
                }
                LRESULT(0)
            },
            WM_CREATE => {
                let createstruct = &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
                let state_ptr = createstruct.lpCreateParams as *mut AboutState;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
                
                let is_dark_mode = if let Some(st) = state_ptr.as_ref() { st.is_dark } else { false };
                
                // Apply DWM title bar color (must always set, not just for dark)
                let dark_mode: u32 = if is_dark_mode { 1 } else { 0 };
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_USE_IMMERSIVE_DARK_MODE,
                    &dark_mode as *const u32 as *const _,
                    4
                );
                
                let instance = GetModuleHandleW(None).unwrap_or_default();
                
                // Initialize Link Control
                let iccex = INITCOMMONCONTROLSEX {
                    dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                    dwICC: ICC_LINK_CLASS,
                };
                InitCommonControlsEx(&iccex);

                let content_width = 410; // Wider content area
                let margin = 20;

                // Create modern fonts for visual hierarchy
                let title_font = CreateFontW(
                    -28, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, DEFAULT_CHARSET,
                    OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                    (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32, w!("Segoe UI Variable Display"));
                
                let version_font = CreateFontW(
                    -14, 0, 0, 0, FW_LIGHT.0 as i32, 0, 0, 0, DEFAULT_CHARSET,
                    OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                    (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32, w!("Segoe UI Variable Display"));
                
                let body_font = CreateFontW(
                    -13, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, DEFAULT_CHARSET,
                    OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                    (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32, w!("Segoe UI"));
                
                let creator_font = CreateFontW(
                    -12, 0, 0, 0, FW_NORMAL.0 as i32, 1, 0, 0, DEFAULT_CHARSET, // Italic
                    OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                    (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32, w!("Segoe UI"));

                // Icon - Centered at top (Large Hero Icon)
                let icon_size = 128;
                let icon_x = (450 - icon_size) / 2;
                
                // Load icon from resources (ID 1)
                // Instance is HMODULE, need HINSTANCE
                let hinstance = HINSTANCE(instance.0);
                
                let hicon = LoadImageW(
                    Some(hinstance),
                    PCWSTR(1 as *const u16), // Resource ID 1
                    IMAGE_ICON,
                    icon_size, icon_size,
                    LR_DEFAULTCOLOR
                ).unwrap_or_default();
                
                let icon_static = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    None,
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | SS_ICON | SS_REALSIZEIMAGE | SS_CENTER),
                    icon_x, 20, icon_size, icon_size,
                    Some(hwnd),
                    None,
                    Some(instance.into()),
                    None
                ).unwrap_or_default();
                
                // Set the icon image
                SendMessageW(icon_static, STM_SETICON, Some(WPARAM(hicon.0 as usize)), Some(LPARAM(0)));
                
                if is_dark_mode {
                    // For static icons, theme might not be needed but good practice
                    let _ = SetWindowTheme(icon_static, w!(""), w!(""));
                }

                // App Name - Large bold title
                let app_name = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    w!("CompactRS"),
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | SS_CENTER),
                    margin, 160, content_width, 40,
                    Some(hwnd),
                    None,
                    Some(instance.into()),
                    None
                ).unwrap_or_default();
                SendMessageW(app_name, WM_SETFONT, Some(WPARAM(title_font.0 as usize)), Some(LPARAM(1)));
                if is_dark_mode {
                    let _ = SetWindowTheme(app_name, w!(""), w!(""));
                }

                // Version - Lighter font
                let version = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    w!("Version 0.1.0"),
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | SS_CENTER),
                    margin, 205, content_width, 20,
                    Some(hwnd),
                    None,
                    Some(instance.into()),
                    None
                ).unwrap_or_default();
                SendMessageW(version, WM_SETFONT, Some(WPARAM(version_font.0 as usize)), Some(LPARAM(1)));
                if is_dark_mode {
                    let _ = SetWindowTheme(version, w!(""), w!(""));
                }

                // Description - Regular body text
                let desc = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    w!("Ultra-lightweight, native Windows transparent file compressor built in Rust. Leverages the Windows Overlay Filter (WOF) to save disk space without performance loss.\n\nFeatures a modern, bloat-free Win32 GUI, batch processing, and multithreaded compression (XPRESS/LZX). Zero dependencies, <1MB binary."),
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | SS_CENTER),
                    margin, 240, content_width, 130,
                    Some(hwnd),
                    None,
                    Some(instance.into()),
                    None
                ).unwrap_or_default();
                SendMessageW(desc, WM_SETFONT, Some(WPARAM(body_font.0 as usize)), Some(LPARAM(1)));
                if is_dark_mode {
                    let _ = SetWindowTheme(desc, w!(""), w!(""));
                }

                // Created by - Italic style
                let creator = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    w!("Created by IRedDragonICY\n(Mohammad Farid Hendianto)"),
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | SS_CENTER),
                    margin, 385, content_width, 40,
                    Some(hwnd),
                    None,
                    Some(instance.into()),
                    None
                ).unwrap_or_default();
                SendMessageW(creator, WM_SETFONT, Some(WPARAM(creator_font.0 as usize)), Some(LPARAM(1)));
                if is_dark_mode {
                    let _ = SetWindowTheme(creator, w!(""), w!(""));
                }

                // GitHub Link (SysLink) - Centered
                let link_text = w!("<a href=\"https://github.com/IRedDragonICY/compactrs\">GitHub</a>  •  <a href=\"https://github.com/IRedDragonICY/compactrs/blob/main/LICENSE\">License</a>");
                let link = CreateWindowExW(
                    Default::default(),
                    WC_LINK,
                    link_text,
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | windows::Win32::UI::WindowsAndMessaging::WS_TABSTOP.0),
                    margin, 440, content_width, 25,
                    Some(hwnd),
                    None,
                    Some(instance.into()),
                    None
                ).unwrap_or_default();
                SendMessageW(link, WM_SETFONT, Some(WPARAM(body_font.0 as usize)), Some(LPARAM(1)));
                if is_dark_mode {
                    let _ = SetWindowTheme(link, w!(""), w!(""));
                }

                LRESULT(0)
            },

            WM_NOTIFY => {
                let nmhdr = &*(lparam.0 as *const NMHDR);
                // Handle SysLink clicks
                if nmhdr.code == NM_CLICK || nmhdr.code == NM_RETURN {
                     let nmlink = &*(lparam.0 as *const NMLINK);
                     let item = nmlink.item;
                     // Open URL
                     if item.iLink == 0 {
                          ShellExecuteW(None, w!("open"), GITHUB_URL, None, None, SW_SHOWNORMAL);
                     } else if item.iLink == 1 {
                          ShellExecuteW(None, w!("open"), LICENSE_URL, None, None, SW_SHOWNORMAL);
                     }
                }
                LRESULT(0)
            },

             WM_CLOSE => {
                // Non-modal: No need to re-enable parent
                // if let Ok(parent) = GetParent(hwnd) {
                //     let _ = EnableWindow(parent, true);
                //     SetActiveWindow(parent);
                // }
                DestroyWindow(hwnd);
                LRESULT(0)
            },
            
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            },

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
