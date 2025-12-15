use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, COLORREF};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, RegisterClassW,
    CS_HREDRAW, CS_VREDRAW, IDC_ARROW, WM_DESTROY, WNDCLASSW,
    WS_VISIBLE, WM_CREATE, WM_COMMAND,
    WS_CHILD, WS_CAPTION, WS_SYSMENU, WS_POPUP,
    GetMessageW, TranslateMessage, DispatchMessageW, MSG,
    PostQuitMessage, WM_CLOSE, GetParent, DestroyWindow, 
    WM_NOTIFY, GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA,
    WM_CTLCOLORSTATIC, WM_ERASEBKGND, GetClientRect,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetActiveWindow};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    NMHDR, NM_CLICK, NM_RETURN, NMLINK, WC_LINK, ICC_LINK_CLASS, INITCOMMONCONTROLSEX, InitCommonControlsEx,
    SetWindowTheme,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::Win32::Graphics::Gdi::{HBRUSH, COLOR_WINDOW, SetTextColor, SetBkMode, CreateSolidBrush, HDC, DeleteObject, HGDIOBJ, FillRect, TRANSPARENT};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use crate::gui::controls::{create_button, IDC_BTN_OK};

const ABOUT_CLASS_NAME: PCWSTR = w!("CompactRS_About");
const ABOUT_TITLE: PCWSTR = w!("About CompactRS");
const GITHUB_URL: PCWSTR = w!("https://github.com/IRedDragonICY/compactrs");
const LICENSE_URL: PCWSTR = w!("https://github.com/IRedDragonICY/compactrs/blob/main/LICENSE");
const SS_CENTER: u32 = 1;

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
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize as *mut _),
            ..Default::default()
        };
        RegisterClassW(&wc);

        // Calculate center position
        let mut rect = windows::Win32::Foundation::RECT::default();
        windows::Win32::UI::WindowsAndMessaging::GetWindowRect(parent, &mut rect).unwrap_or_default();
        let p_width = rect.right - rect.left;
        let p_height = rect.bottom - rect.top;
        let width = 400;
        let height = 320;
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
        let get_state = || {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr == 0 { None } else { Some(&mut *(ptr as *mut AboutState)) }
        };

        match msg {
            WM_CTLCOLORSTATIC => {
                if let Some(st) = get_state() {
                    if st.is_dark {
                        let hdc = HDC(wparam.0 as *mut _);
                        SetTextColor(hdc, COLORREF(0x00FFFFFF)); // White text
                        SetBkMode(hdc, TRANSPARENT);
                        
                        let brush = if let Some(b) = st.dark_brush {
                            b
                        } else {
                            let new_brush = CreateSolidBrush(COLORREF(0x001E1E1E));
                            st.dark_brush = Some(new_brush);
                            new_brush
                        };
                        return LRESULT(brush.0 as isize);
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            },
            WM_ERASEBKGND => {
                if let Some(st) = get_state() {
                    if st.is_dark {
                        let hdc = HDC(wparam.0 as *mut _);
                        let mut rc = windows::Win32::Foundation::RECT::default();
                        GetClientRect(hwnd, &mut rc);
                        
                        let brush = if let Some(b) = st.dark_brush {
                            b
                        } else {
                            let new_brush = CreateSolidBrush(COLORREF(0x001E1E1E));
                            st.dark_brush = Some(new_brush);
                            new_brush
                        };
                        FillRect(hdc, &rc, brush);
                        return LRESULT(1);
                    }
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

                // App Name
                let app_name = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    w!("CompactRS"),
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | SS_CENTER),
                    10, 20, 360, 30,
                    Some(hwnd),
                    None,
                    Some(instance.into()),
                    None
                ).unwrap_or_default();
                if is_dark_mode {
                    let _ = SetWindowTheme(app_name, w!(""), w!(""));
                }

                // Version
                let version = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    w!("Version 0.1.0"),
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | SS_CENTER),
                    10, 50, 360, 20,
                    Some(hwnd),
                    None,
                    Some(instance.into()),
                    None
                ).unwrap_or_default();
                if is_dark_mode {
                    let _ = SetWindowTheme(version, w!(""), w!(""));
                }

                // Description
                let desc = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    w!("Ultra-lightweight, native Windows transparent file compressor built in Rust. Leverages the Windows Overlay Filter (WOF) to save disk space without performance loss. Features a modern, bloat-free Win32 GUI, batch processing, and multithreaded compression (XPRESS/LZX). Zero dependencies, <1MB binary."),
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | SS_CENTER),
                    10, 80, 360, 140,
                    Some(hwnd),
                    None,
                    Some(instance.into()),
                    None
                ).unwrap_or_default();
                if is_dark_mode {
                    let _ = SetWindowTheme(desc, w!(""), w!(""));
                }

                // GitHub Link (SysLink)
                let link_text = w!("Check out the code on <a href=\"https://github.com/IRedDragonICY/compactrs\">GitHub</a> or view the <a href=\"https://github.com/IRedDragonICY/compactrs/blob/main/LICENSE\">License</a>.");
                let link = CreateWindowExW(
                    Default::default(),
                    WC_LINK,
                    link_text,
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | windows::Win32::UI::WindowsAndMessaging::WS_TABSTOP.0),
                    10, 230, 360, 20,
                    Some(hwnd),
                    None,
                    Some(instance.into()),
                    None
                ).unwrap_or_default();
                if is_dark_mode {
                    let _ = SetWindowTheme(link, w!(""), w!(""));
                }

                // OK Button
                let ok_btn = create_button(hwnd, w!("OK"), 160, 260, 80, 25, IDC_BTN_OK);
                if is_dark_mode {
                    let _ = SetWindowTheme(ok_btn, w!(""), w!(""));
                }

                LRESULT(0)
            },
            
            WM_COMMAND => {
                 let id = (wparam.0 & 0xFFFF) as u16;
                 match id {
                     IDC_BTN_OK => {
                         // Non-modal: No need to re-enable parent
                         // if let Ok(parent) = GetParent(hwnd) {
                         //     let _ = EnableWindow(parent, true);
                         //     SetActiveWindow(parent);
                         // }
                         DestroyWindow(hwnd);
                     },
                     _ => {}
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
