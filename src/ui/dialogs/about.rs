#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::builder::ControlBuilder;
use crate::utils::to_wstring;
use crate::w;
use crate::ui::framework::WindowHandler;
use crate::types::*;

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
            use crate::ui::layout::{LayoutNode, SizePolicy::{Fixed}};
            crate::ui::theme::set_window_frame_theme(hwnd, self.is_dark);
            
            let client_rect = crate::utils::get_client_rect(hwnd);

            
            // Fonts
            let segoe_ui_var = w!("Segoe UI Variable Display");
            let segoe_ui = w!("Segoe UI");
            
            let make_font = |h: i32, w: i32, i: u32, face: *const u16| CreateFontW(
                -h, 0, 0, 0, w, i, 0, 0, DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32,
                (DEFAULT_PITCH | FF_DONTCARE) as u32, face);

            let f_title = make_font(28, FW_BOLD as i32, 0, segoe_ui_var.as_ptr());
            let f_ver = make_font(14, FW_LIGHT as i32, 0, segoe_ui_var.as_ptr());
            let f_body = make_font(13, FW_NORMAL as i32, 0, segoe_ui.as_ptr());
            let f_creator = make_font(12, FW_NORMAL as i32, 1, segoe_ui.as_ptr());
            let f_icon = crate::ui::theme::get_icon_font();

            // Helpers
            let builder = |id| ControlBuilder::new(hwnd, id).dark_mode(self.is_dark);
            let lbl = |text, font, _h| builder(0).label(true).text(text).font(font).build();
            let btn = |text, id| builder(id).button().text_w(text).font(f_icon).build();
            
            // 1. Icon (Centered manually because it's static/fixed pos often easier, but we can LayoutNode it)
            // Icon specific loading...
            let hicon = LoadImageW(GetModuleHandleW(std::ptr::null()), 1 as *const u16, IMAGE_ICON, 128, 128, LR_DEFAULTCOLOR);
            let h_icon_static = builder(0).icon_display().size(128, 128).build();
            SendMessageW(h_icon_static, STM_SETICON, hicon as WPARAM, 0);

            // 2. Texts
            let h_title = lbl("CompactRS", f_title, 40);
            
            let ver_s = crate::utils::concat_wstrings(&[w!("Version "), &to_wstring(env!("APP_VERSION"))]);
            let h_ver = builder(0).label(true).text_w(&ver_s).font(f_ver).build();
            
            let desc_txt = "Ultra-lightweight, native Windows transparent file compressor built in Rust. Leverages the Windows Overlay Filter (WOF) to save disk space without performance loss.\n\nFeatures a modern, bloat-free Win32 GUI, batch processing, and multithreaded compression (XPRESS/LZX). Zero dependencies, <1MB binary.";
            let h_desc = lbl(desc_txt, f_body, 130);
            
            let h_creator = lbl("Created by IRedDragonICY\n(Mohammad Farid Hendianto)", f_creator, 40);

            // 3. Action Buttons (GitHub, License)
            let h_btn_gh = btn(&[0xE943, 0], 1001);
            let h_btn_lic = btn(&[0xE929, 0], 1002);
            
            let h_lbl_gh = lbl("GitHub", f_creator, 20);
            let h_lbl_lic = lbl("License", f_creator, 20);

            // Layout
            LayoutNode::col(0, 0)
                .spacer(20) // Top Margin
                .with_child(LayoutNode::row(0, 0).flex_spacer().with(h_icon_static, Fixed(128)).flex_spacer())
                .spacer(12)
                .with(h_title, Fixed(40))
                .spacer(20)
                .with(h_ver, Fixed(20))
                .spacer(10)
                .with_child(LayoutNode::row(0, 0).flex_spacer().with(h_desc, Fixed(410)).flex_spacer())
                .spacer(20)
                .with(h_creator, Fixed(40))
                .spacer(30)
                .with_child(LayoutNode::row(0, 10).flex_spacer()
                    .with_child(LayoutNode::col(0, 5).with(h_btn_gh, Fixed(45)).with(h_lbl_gh, Fixed(20)))
                    .with_child(LayoutNode::col(0, 5).with(h_btn_lic, Fixed(45)).with(h_lbl_lic, Fixed(20)))
                    .flex_spacer()
                )
                .apply_layout(client_rect);
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
