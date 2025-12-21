#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::builder::ControlBuilder;
use crate::utils::to_wstring;
use crate::w;
use crate::ui::framework::WindowHandler;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, 
    WS_VISIBLE, WS_CHILD,
    WM_NOTIFY,
    STM_SETICON, LoadImageW, IMAGE_ICON, LR_DEFAULTCOLOR,
    WS_TABSTOP, WM_SETFONT, SendMessageW,
};

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
    let mut state = AboutState { is_dark };
    
    crate::ui::dialogs::base::show_modal_singleton(
        parent, 
        &mut state, 
        "CompactRS_About", 
        ABOUT_TITLE, 
        450, 
        500, 
        is_dark
    );
}

impl WindowHandler for AboutState {
    fn is_dark_mode(&self) -> bool {
        self.is_dark
    }

    fn on_create(&mut self, hwnd: HWND) -> LRESULT {
        unsafe {
            // Apply DWM title bar color using centralized helper
            crate::ui::theme::set_window_frame_theme(hwnd, self.is_dark);
            
            // Initialize Link Control
            let iccex = INITCOMMONCONTROLSEX {
                dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_LINK_CLASS,
            };
            InitCommonControlsEx(&iccex);

            let content_width = 410; // Wider content area
            let margin = 20;

            // Create modern fonts for visual hierarchy
            let segoe_ui_var = w!("Segoe UI Variable Display");
            let segoe_ui = w!("Segoe UI");

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
            let instance = GetModuleHandleW(std::ptr::null());
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
                .dark_mode(self.is_dark)
                .build();
            
            // Set the icon image
            SendMessageW(icon_static, STM_SETICON, hicon as WPARAM, 0);

            // Use LayoutBuilder for text content
            // Start below icon (20 + 128 = 148). Let's start at 160.
            let mut layout = crate::ui::layout::LayoutContext::new(margin, 160, content_width, 5);

            // App Name - Large bold title using ControlBuilder
            let (x, y, w, h) = layout.row(40);
            let _app_name = ControlBuilder::new(hwnd, 0)
                .label(true) // center-aligned
                .text_w(w!("CompactRS"))
                .pos(x, y)
                .size(w, h)
                .font(title_font)
                .dark_mode(self.is_dark)
                .build();

            // Version - Lighter font using ControlBuilder
            let ver_string = format!("Version {}", env!("APP_VERSION"));
            let (x, y, w, h) = layout.row(20);
            let _version = ControlBuilder::new(hwnd, 0)
                .label(true)
                .text(&ver_string)
                .pos(x, y)
                .size(w, h)
                .font(version_font)
                .dark_mode(self.is_dark)
                .build();
            
            layout.add_space(10);

            // Description - Regular body text using ControlBuilder
            let (x, y, w, h) = layout.row(130);
            let _desc = ControlBuilder::new(hwnd, 0)
                .label(true)
                .text_w(w!("Ultra-lightweight, native Windows transparent file compressor built in Rust. Leverages the Windows Overlay Filter (WOF) to save disk space without performance loss.\n\nFeatures a modern, bloat-free Win32 GUI, batch processing, and multithreaded compression (XPRESS/LZX). Zero dependencies, <1MB binary."))
                .pos(x, y)
                .size(w, h)
                .font(body_font)
                .dark_mode(self.is_dark)
                .build();
            
            layout.add_space(10);

            // Created by - Italic style using ControlBuilder
            let (x, y, w, h) = layout.row(40);
            let _creator = ControlBuilder::new(hwnd, 0)
                .label(true)
                .text_w(w!("Created by IRedDragonICY\n(Mohammad Farid Hendianto)"))
                .pos(x, y)
                .size(w, h)
                .font(creator_font)
                .dark_mode(self.is_dark)
                .build();

            // GitHub Link (SysLink) - Centered (still raw CreateWindowExW as it's a special control)
            layout.add_space(10);
            let (x, y, w, h) = layout.row(25);
            let link_text = to_wstring("<a href=\"https://github.com/IRedDragonICY/compactrs\">GitHub</a>  •  <a href=\"https://github.com/IRedDragonICY/compactrs/blob/main/LICENSE\">License</a>");
            let link_cls = WC_LINK;
            let link = CreateWindowExW(
                0,
                link_cls,
                link_text.as_ptr(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                x, y, w, h,
                hwnd,
                std::ptr::null_mut(),
                instance,
                std::ptr::null()
            );
            SendMessageW(link, WM_SETFONT, body_font as WPARAM, 1);
            if self.is_dark {
                crate::ui::theme::apply_theme(link, crate::ui::theme::ControlType::Window, true);
            }
        }
        0
    }

    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            match msg {
                // WM_APP + 2: Theme change broadcast from Settings
                0x8002 => {
                    let new_is_dark = wparam == 1;
                    self.is_dark = new_is_dark;
                    
                    // Update DWM title bar using centralized helper
                    crate::ui::theme::set_window_frame_theme(hwnd, new_is_dark);
                    
                    // Force repaint
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                    Some(0)
                },
                
                WM_NOTIFY => {
                    let nmhdr = &*(lparam as *const NMHDR);
                    // Handle SysLink clicks
                    if nmhdr.code == NM_CLICK || nmhdr.code == NM_RETURN {
                         let nmlink = &*(lparam as *const NMLINK);
                         let item = nmlink.item;
                         
                         let open = w!("open");
                         let github = to_wstring(GITHUB_URL);
                         let license = to_wstring(LICENSE_URL);
                         
                         // Open URL
                         if item.iLink == 0 {
                               ShellExecuteW(std::ptr::null_mut(), open.as_ptr(), github.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
                         } else if item.iLink == 1 {
                               ShellExecuteW(std::ptr::null_mut(), open.as_ptr(), license.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
                         }
                    }
                    Some(0)
                },
                _ => None,
            }
        }
    }
}
