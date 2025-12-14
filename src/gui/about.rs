use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, RegisterClassW,
    CS_HREDRAW, CS_VREDRAW, IDC_ARROW, WM_DESTROY, WNDCLASSW,
    WS_VISIBLE, WM_CREATE, WM_COMMAND,
    WS_CHILD, WS_CAPTION, WS_SYSMENU, WS_POPUP,
    GetMessageW, TranslateMessage, DispatchMessageW, MSG,
    PostQuitMessage, WM_CLOSE, GetParent, DestroyWindow, 
    WM_NOTIFY,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetActiveWindow};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    NMHDR, NM_CLICK, NM_RETURN, NMLINK, WC_LINK, ICC_LINK_CLASS, INITCOMMONCONTROLSEX, InitCommonControlsEx,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use crate::gui::controls::{create_button, IDC_BTN_OK};

const ABOUT_CLASS_NAME: PCWSTR = w!("CompactRS_About");
const ABOUT_TITLE: PCWSTR = w!("About CompactRS");
const GITHUB_URL: PCWSTR = w!("https://github.com/IRedDragonICY/compactrs");
const LICENSE_URL: PCWSTR = w!("https://github.com/IRedDragonICY/compactrs/blob/main/LICENSE");
const SS_CENTER: u32 = 1;

pub unsafe fn show_about_modal(parent: HWND) {
    unsafe {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(about_wnd_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: ABOUT_CLASS_NAME,
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(windows::Win32::Graphics::Gdi::COLOR_WINDOW.0 as isize as *mut _),
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

        let _hwnd = CreateWindowExW(
            Default::default(),
            ABOUT_CLASS_NAME,
            ABOUT_TITLE,
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            x, y, width, height,
            Some(parent),
            None,
            Some(instance.into()),
            None,
        ).unwrap_or_default();

        EnableWindow(parent, false);
        
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        EnableWindow(parent, true);
    }
}

unsafe extern "system" fn about_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let instance = GetModuleHandleW(None).unwrap_or_default();
                
                // Initialize Link Control
                let iccex = INITCOMMONCONTROLSEX {
                    dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                    dwICC: ICC_LINK_CLASS,
                };
                InitCommonControlsEx(&iccex);

                // App Name
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    w!("CompactRS"),
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | SS_CENTER),
                    10, 20, 360, 30,
                    Some(hwnd),
                    None,
                    Some(instance.into()),
                    None
                );
                
                // Set font for App Name (Bold, Larger) - TODO: Proper font creation

                // Version
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    w!("Version 0.1.0"),
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | SS_CENTER),
                    10, 50, 360, 20,
                    Some(hwnd),
                    None,
                    Some(instance.into()),
                    None
                );

                // Description
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    w!("Ultra-lightweight, native Windows transparent file compressor built in Rust. Leverages the Windows Overlay Filter (WOF) to save disk space without performance loss. Features a modern, bloat-free Win32 GUI, batch processing, and multithreaded compression (XPRESS/LZX). Zero dependencies, <1MB binary."),
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | SS_CENTER),
                    10, 80, 360, 140,
                    Some(hwnd),
                    None,
                    Some(instance.into()),
                    None
                );

                // GitHub Link (SysLink)
                // Note: The markup <a>...</a> is safely handled by SysLink
                let link_text = w!("Check out the code on <a href=\"https://github.com/IRedDragonICY/compactrs\">GitHub</a> or view the <a href=\"https://github.com/IRedDragonICY/compactrs/blob/main/LICENSE\">License</a>.");
                CreateWindowExW(
                    Default::default(),
                    WC_LINK,
                    link_text,
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | windows::Win32::UI::WindowsAndMessaging::WS_TABSTOP.0),
                    10, 230, 360, 20,
                    Some(hwnd),
                    None, // ID
                    Some(instance.into()),
                    None
                );

                // OK Button
                 create_button(hwnd, w!("OK"), 160, 260, 80, 25, IDC_BTN_OK);

                LRESULT(0)
            },
            
            WM_COMMAND => {
                 let id = (wparam.0 & 0xFFFF) as u16;
                 match id {
                     IDC_BTN_OK => {
                         if let Ok(parent) = GetParent(hwnd) {
                             let _ = EnableWindow(parent, true);
                             SetActiveWindow(parent);
                         }
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
                     // Check if it is our link control (we didn't assign specific ID, but it sends NM_CLICK)
                     // A cleaner way is checking class name or ID, but for now assuming it's the one.
                     // Actually NM_CLICK for SysLink casts to NMLINK
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
                if let Ok(parent) = GetParent(hwnd) {
                    let _ = EnableWindow(parent, true);
                    SetActiveWindow(parent);
                }
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
