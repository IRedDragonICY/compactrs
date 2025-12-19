#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::builder::ControlBuilder;
use crate::ui::utils::get_window_state;
use crate::utils::to_wstring;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, RegisterClassW, SendMessageW,
    CS_HREDRAW, CS_VREDRAW, IDC_ARROW, WM_DESTROY, WNDCLASSW,
    WS_VISIBLE, WM_CREATE,
    WS_CHILD, WS_CAPTION, WS_SYSMENU, WS_POPUP,
    PostQuitMessage, WM_CLOSE, DestroyWindow, 
    WM_NOTIFY, SetWindowLongPtrW, GWLP_USERDATA,
    STM_SETICON, LoadImageW, IMAGE_ICON, LR_DEFAULTCOLOR,
    GetWindowRect, CREATESTRUCTW, WS_TABSTOP,
};
use windows_sys::Win32::Foundation::RECT;

use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{
    NMHDR, NM_CLICK, NM_RETURN, NMLINK, WC_LINK, ICC_LINK_CLASS, INITCOMMONCONTROLSEX, InitCommonControlsEx,

};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontW, FW_BOLD, FW_NORMAL, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, 
    DEFAULT_PITCH, FF_DONTCARE, FW_LIGHT, InvalidateRect, HFONT,
};


const ABOUT_TITLE: &str = "About CompactRS";
const GITHUB_URL: &str = "https://github.com/IRedDragonICY/compactrs";
const LICENSE_URL: &str = "https://github.com/IRedDragonICY/compactrs/blob/main/LICENSE";

struct AboutState {
    is_dark: bool,
}

pub unsafe fn show_about_modal(parent: HWND, is_dark: bool) {
    let instance = GetModuleHandleW(std::ptr::null());
    let class_name = to_wstring("CompactRS_About");

    // Check if window already exists
    let existing_hwnd = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(class_name.as_ptr(), std::ptr::null());
    if existing_hwnd != std::ptr::null_mut() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SetForegroundWindow, SW_RESTORE};
        ShowWindow(existing_hwnd, SW_RESTORE);
        SetForegroundWindow(existing_hwnd);
        return;
    }
    let title = to_wstring(ABOUT_TITLE);
    
    // Load App Icon
    let icon = crate::ui::utils::load_app_icon(instance);
    
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(about_wnd_proc),
        hInstance: instance,
        hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
        lpszClassName: class_name.as_ptr(),
        hIcon: icon,
        hbrBackground: crate::ui::theme::get_background_brush(is_dark),
        cbClsExtra: 0,
        cbWndExtra: 0,
        lpszMenuName: std::ptr::null(),
    };
    RegisterClassW(&wc);

    // Calculate center position
    let mut rect: RECT = std::mem::zeroed();
    GetWindowRect(parent, &mut rect);
    let p_width = rect.right - rect.left;
    let p_height = rect.bottom - rect.top;
    let width = 450;  // Wider for better text display
    let height = 500; // Reduced height (no OK button)
    let x = rect.left + (p_width - width) / 2;
    let y = rect.top + (p_height - height) / 2;

    let mut state = AboutState {
        is_dark,
    };

    let _hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
        x, y, width, height,
        parent,
        std::ptr::null_mut(),
        instance,
        &mut state as *mut _ as *mut std::ffi::c_void,
    );

    // Message loop
    crate::ui::utils::run_message_loop();
}

unsafe extern "system" fn about_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Use centralized helper for state access
    let get_state = || get_window_state::<AboutState>(hwnd);

    // Centralized handler for theme-related messages
    if let Some(st) = get_state() {
        if let Some(result) = crate::ui::theme::handle_standard_colors(hwnd, msg, wparam, st.is_dark) {
            return result;
        }
    }

    match msg {
        // WM_APP + 2: Theme change broadcast from Settings
        0x8002 => {
            if let Some(st) = get_state() {
                let new_is_dark = wparam == 1;
                st.is_dark = new_is_dark;
                
                // Update DWM title bar using centralized helper
                crate::ui::theme::set_window_frame_theme(hwnd, new_is_dark);
                
                // Force repaint
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            0
        },
        WM_CREATE => {
            let createstruct = &*(lparam as *const CREATESTRUCTW);
            let state_ptr = createstruct.lpCreateParams as *mut AboutState;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
            
            let is_dark_mode = if let Some(st) = state_ptr.as_ref() { st.is_dark } else { false };
            
            // Apply DWM title bar color using centralized helper
            crate::ui::theme::set_window_frame_theme(hwnd, is_dark_mode);
            
            let instance = GetModuleHandleW(std::ptr::null());
            
            // Initialize Link Control
            let iccex = INITCOMMONCONTROLSEX {
                dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_LINK_CLASS,
            };
            InitCommonControlsEx(&iccex);

            let content_width = 410; // Wider content area
            let margin = 20;

            // Create modern fonts for visual hierarchy
            let segoe_ui_var = to_wstring("Segoe UI Variable Display");
            let segoe_ui = to_wstring("Segoe UI");

            let title_font = CreateFontW(
                -28, 0, 0, 0, FW_BOLD as i32, 0, 0, 0, DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32,
                (DEFAULT_PITCH | FF_DONTCARE) as u32, segoe_ui_var.as_ptr()) as HFONT;
            
            let version_font = CreateFontW(
                -14, 0, 0, 0, FW_LIGHT as i32, 0, 0, 0, DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32,
                (DEFAULT_PITCH | FF_DONTCARE) as u32, segoe_ui_var.as_ptr()) as HFONT;
            
            let body_font = CreateFontW(
                -13, 0, 0, 0, FW_NORMAL as i32, 0, 0, 0, DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32,
                (DEFAULT_PITCH | FF_DONTCARE) as u32, segoe_ui.as_ptr()) as HFONT;
            
            let creator_font = CreateFontW(
                -12, 0, 0, 0, FW_NORMAL as i32, 1, 0, 0, DEFAULT_CHARSET as u32, // Italic
                OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32,
                (DEFAULT_PITCH | FF_DONTCARE) as u32, segoe_ui.as_ptr()) as HFONT;

            // Icon - Centered at top (Large Hero Icon)
            let icon_size = 128;
            let icon_x = (450 - icon_size) / 2;
            
            // Load large icon for about dialog
            let hicon = LoadImageW(
                instance,
                1 as *const u16, // Win32 MAKEINTRESOURCE(1)
                IMAGE_ICON,
                icon_size, icon_size,
                LR_DEFAULTCOLOR
            );

            // Icon display using ControlBuilder
            let icon_static = ControlBuilder::new(hwnd, 0)
                .icon_display()
                .pos(icon_x, 20)
                .size(icon_size, icon_size)
                .dark_mode(is_dark_mode)
                .build();
            
            // Set the icon image
            SendMessageW(icon_static, STM_SETICON, hicon as WPARAM, 0);

            // App Name - Large bold title using ControlBuilder
            let _app_name = ControlBuilder::new(hwnd, 0)
                .label(true) // center-aligned
                .text("CompactRS")
                .pos(margin, 160)
                .size(content_width, 40)
                .font(title_font)
                .dark_mode(is_dark_mode)
                .build();

            // Version - Lighter font using ControlBuilder
            let ver_string = format!("Version {}", env!("APP_VERSION"));
            let _version = ControlBuilder::new(hwnd, 0)
                .label(true)
                .text(&ver_string)
                .pos(margin, 205)
                .size(content_width, 20)
                .font(version_font)
                .dark_mode(is_dark_mode)
                .build();

            // Description - Regular body text using ControlBuilder
            let _desc = ControlBuilder::new(hwnd, 0)
                .label(true)
                .text("Ultra-lightweight, native Windows transparent file compressor built in Rust. Leverages the Windows Overlay Filter (WOF) to save disk space without performance loss.\n\nFeatures a modern, bloat-free Win32 GUI, batch processing, and multithreaded compression (XPRESS/LZX). Zero dependencies, <1MB binary.")
                .pos(margin, 240)
                .size(content_width, 130)
                .font(body_font)
                .dark_mode(is_dark_mode)
                .build();

            // Created by - Italic style using ControlBuilder
            let _creator = ControlBuilder::new(hwnd, 0)
                .label(true)
                .text("Created by IRedDragonICY\n(Mohammad Farid Hendianto)")
                .pos(margin, 385)
                .size(content_width, 40)
                .font(creator_font)
                .dark_mode(is_dark_mode)
                .build();

            // GitHub Link (SysLink) - Centered (still raw CreateWindowExW as it's a special control)
            let link_text = to_wstring("<a href=\"https://github.com/IRedDragonICY/compactrs\">GitHub</a>  •  <a href=\"https://github.com/IRedDragonICY/compactrs/blob/main/LICENSE\">License</a>");
            let link_cls = WC_LINK;
            let link = CreateWindowExW(
                0,
                link_cls,
                link_text.as_ptr(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                margin, 440, content_width, 25,
                hwnd,
                std::ptr::null_mut(),
                instance,
                std::ptr::null()
            );
            SendMessageW(link, windows_sys::Win32::UI::WindowsAndMessaging::WM_SETFONT, body_font as WPARAM, 1);
            if is_dark_mode {
                crate::ui::theme::apply_theme(link, crate::ui::theme::ControlType::Window, true);
            }

            0
        },

        WM_NOTIFY => {
            let nmhdr = &*(lparam as *const NMHDR);
            // Handle SysLink clicks
            if nmhdr.code == NM_CLICK || nmhdr.code == NM_RETURN {
                 let nmlink = &*(lparam as *const NMLINK);
                 let item = nmlink.item;
                 
                 let open = to_wstring("open");
                 let github = to_wstring(GITHUB_URL);
                 let license = to_wstring(LICENSE_URL);
                 
                 // Open URL
                 if item.iLink == 0 {
                      ShellExecuteW(std::ptr::null_mut(), open.as_ptr(), github.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
                 } else if item.iLink == 1 {
                      ShellExecuteW(std::ptr::null_mut(), open.as_ptr(), license.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
                 }
            }
            0
        },

         WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        },
        
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        },

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
