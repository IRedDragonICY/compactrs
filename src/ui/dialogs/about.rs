#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::builder::ControlBuilder;
use crate::utils::to_wstring;
use crate::w;
use crate::ui::framework::WindowHandler;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    STM_SETICON, LoadImageW, IMAGE_ICON, LR_DEFAULTCOLOR,
    SendMessageW, GetClientRect,
};

use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
// unused imports removed
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
        600, 
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
            
            let mut rc = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            let client_width = rc.right - rc.left;
            
            // Cleanup Link Control Logic (removed)

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
            let icon_x = (client_width - icon_size) / 2;
            
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

            // Use simple local variables for text content layout
            // Start below icon (20 + 128 = 148). Let's start at 160.
            let content_width = 410; // Wider content area
            let x_start = (client_width - content_width) / 2; // Auto-center content column
            let mut current_y = 160;

            // App Name - Large bold title using ControlBuilder
            let h_title = 40;
            let _app_name = ControlBuilder::new(hwnd, 0)
                .label(true) // center-aligned
                .text_w(w!("CompactRS"))
                .pos(x_start, current_y)
                .size(content_width, h_title)
                .font(title_font)
                .dark_mode(self.is_dark)
                .build();
            current_y += h_title + 20; // Extra spacing after title

            // Version - Lighter font using ControlBuilder
            let h_ver = 20;
            let ver_string = format!("Version {}", env!("APP_VERSION"));
            let _version = ControlBuilder::new(hwnd, 0)
                .label(true)
                .text(&ver_string)
                .pos(x_start, current_y)
                .size(content_width, h_ver)
                .font(version_font)
                .dark_mode(self.is_dark)
                .build();
            current_y += h_ver + 10; // Space

            // Description - Regular body text using ControlBuilder
            let h_desc = 130;
            let _desc = ControlBuilder::new(hwnd, 0)
                .label(true)
                .text_w(w!("Ultra-lightweight, native Windows transparent file compressor built in Rust. Leverages the Windows Overlay Filter (WOF) to save disk space without performance loss.\n\nFeatures a modern, bloat-free Win32 GUI, batch processing, and multithreaded compression (XPRESS/LZX). Zero dependencies, <1MB binary."))
                .pos(x_start, current_y)
                .size(content_width, h_desc)
                .font(body_font)
                .dark_mode(self.is_dark)
                .build();
            current_y += h_desc + 10;

            // Created by - Italic style using ControlBuilder
            let h_creator = 40;
            let _creator = ControlBuilder::new(hwnd, 0)
                .label(true)
                .text_w(w!("Created by IRedDragonICY\n(Mohammad Farid Hendianto)"))
                .pos(x_start, current_y)
                .size(content_width, h_creator)
                .font(creator_font)
                .dark_mode(self.is_dark)
                .build();
            current_y += h_creator + 20;

            // Modern Action Buttons (Centered Row)
            
            // Allocate row for buttons (Height 40)
            let y_btn = current_y;
            
            // Calculate center position for two button groups
            // Group width = 60 (Button) + 10 (Space) + 60 (Button) = 130
            // Start X = (client_width - 130) / 2
            let start_x = (client_width - 130) / 2;
            
            let icon_font = crate::ui::theme::get_icon_font();
            
            // GitHub Button
            // Icon: Code (\u{E943})
            let _btn_github = ControlBuilder::new(hwnd, 1001)
                .button()
                .text_w(&[0xE943, 0]) 
                .pos(start_x, y_btn) 
                .size(60, 40)
                .font(icon_font)
                .dark_mode(self.is_dark)
                .build();
                
            // License Button
            // Icon: Certificate (\u{E929})
            let _btn_license = ControlBuilder::new(hwnd, 1002)
                .button()
                .text_w(&[0xE929, 0])
                .pos(start_x + 70, y_btn)
                .size(60, 40)
                .font(icon_font)
                .dark_mode(self.is_dark)
                .build();

            // Labels for buttons
            let _lbl_github = ControlBuilder::new(hwnd, 0)
                .label(true) // Center
                .text("GitHub")
                .pos(start_x, y_btn + 45)
                .size(60, 20)
                .font(creator_font) // Reusing small font
                .dark_mode(self.is_dark)
                .build();

            let _lbl_license = ControlBuilder::new(hwnd, 0)
                .label(true)
                .text("License")
                .pos(start_x + 70, y_btn + 45)
                .size(60, 20)
                .font(creator_font)
                .dark_mode(self.is_dark)
                .build();
        }
        0
    }

    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, _lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            match msg {
                // WM_COMMAND: Handle Button Clicks
                0x0111 => { // WM_COMMAND
                    let id = (wparam & 0xFFFF) as u16;
                    let open = w!("open");
                    let github = to_wstring(GITHUB_URL);
                    let license = to_wstring(LICENSE_URL);
                    
                    if id == 1001 { // GitHub
                        ShellExecuteW(std::ptr::null_mut(), open.as_ptr(), github.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
                    } else if id == 1002 { // License
                         ShellExecuteW(std::ptr::null_mut(), open.as_ptr(), license.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
                    }
                    Some(0)
                },
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
                
                _ => None,
            }
        }
    }
}
