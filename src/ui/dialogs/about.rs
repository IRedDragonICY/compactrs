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
        400, 
        480, // Tinggi diperpendek menyesuaikan konten
        is_dark
    );
}

impl WindowHandler for AboutState {
    fn is_dark_mode(&self) -> bool {
        self.is_dark
    }

    fn on_create(&mut self, hwnd: HWND) -> LRESULT {
        unsafe {
            use crate::ui::layout::{LayoutNode, SizePolicy::Fixed, AlignItems, JustifyContent};
            crate::ui::theme::set_window_frame_theme(hwnd, self.is_dark);
            
            let client_rect = crate::utils::get_client_rect(hwnd);

            let segoe_ui_var = w!("Segoe UI Variable Display");
            let segoe_ui = w!("Segoe UI");
            
            let make_font = |h: i32, w: i32, i: u32, face: *const u16| CreateFontW(
                -crate::ui::theme::scale(h), 0, 0, 0, w, i, 0, 0, DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32,
                (DEFAULT_PITCH | FF_DONTCARE) as u32, face);

            let f_title = make_font(26, FW_BOLD as i32, 0, segoe_ui_var.as_ptr());
            let f_ver = make_font(13, FW_NORMAL as i32, 0, segoe_ui_var.as_ptr());
            let f_body = make_font(13, FW_NORMAL as i32, 0, segoe_ui.as_ptr());
            let f_creator = make_font(12, FW_NORMAL as i32, 1, segoe_ui.as_ptr());
            let f_icon = crate::ui::theme::get_icon_font();

            let builder = |id| ControlBuilder::new(hwnd, id).dark_mode(self.is_dark);
            let lbl = |text, font| builder(0).label(true).text(text).font(font).build();
            let btn = |text, id| builder(id).button().text_w(text).font(f_icon).build();
            
            let hicon = LoadImageW(GetModuleHandleW(std::ptr::null()), 1 as *const u16, IMAGE_ICON, 96, 96, LR_DEFAULTCOLOR);
            let h_icon_static = builder(0).icon_display().size(96, 96).build();
            SendMessageW(h_icon_static, STM_SETICON, hicon as WPARAM, 0);

            let h_title = lbl("CompactRS", f_title);
            
            let ver_s = crate::utils::concat_wstrings(&[w!("Version "), &to_wstring(env!("APP_VERSION"))]);
            let h_ver = builder(0).label(true).text_w(&ver_s).font(f_ver).build();
            
            let desc_txt = "Ultra-lightweight, native Windows transparent file compressor built in Rust.\nLeverages the Windows Overlay Filter (WOF) to save disk space without performance loss.\n\nFeatures a modern Win32 GUI, batch processing, and multithreaded (XPRESS/LZX) compression.";
            let h_desc = lbl(desc_txt, f_body);
            
            let h_creator = lbl("Created by IRedDragonICY\n(Mohammad Farid Hendianto)", f_creator);

            let h_btn_gh = btn(&[0xE943, 0], 1001);
            let h_btn_lic = btn(&[0xE929, 0], 1002);
            
            let h_lbl_gh = lbl("GitHub", f_creator);
            let h_lbl_lic = lbl("License", f_creator);

            LayoutNode::col(20, 0)
                .align_items(AlignItems::Stretch)
                .justify_content(JustifyContent::Center) // Memusatkan SELURUH konten vertikal di tengah jendela
                .with_child(LayoutNode::row(0, 0)
                    .justify_content(JustifyContent::Center)
                    .with_child(LayoutNode::fixed(h_icon_static, 96).cross_policy(Fixed(96)))
                )
                .spacer(15)
                .with(h_title, Fixed(35))
                .with(h_ver, Fixed(20))
                .spacer(15)
                .with(h_desc, Fixed(100)) 
                .spacer(10)
                .with(h_creator, Fixed(40))
                .spacer(25) // Menggantikan flex_spacer dengan gap statis 25px yang presisi
                .with_child(LayoutNode::row(0, 30) // Gap presisi antar dua tombol
                    .justify_content(JustifyContent::Center)
                    .align_items(AlignItems::Center)
                    .with_policy(Fixed(70)) // Kunci tinggi seluruh blok tombol
                    .with_child(LayoutNode::col(0, 5)
                        .with_policy(Fixed(80))
                        .align_items(AlignItems::Center)
                        .with_child(LayoutNode::fixed(h_btn_gh, 45).cross_policy(Fixed(45)))
                        .with_child(LayoutNode::fixed(h_lbl_gh, 20).cross_policy(Fixed(80)))
                    )
                    .with_child(LayoutNode::col(0, 5)
                        .with_policy(Fixed(80))
                        .align_items(AlignItems::Center)
                        .with_child(LayoutNode::fixed(h_btn_lic, 45).cross_policy(Fixed(45)))
                        .with_child(LayoutNode::fixed(h_lbl_lic, 20).cross_policy(Fixed(80)))
                    )
                )
                .apply_layout(client_rect);
        }
        0
    }

    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, _lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            match msg {
                0x0111 => { 
                    let id = (wparam & 0xFFFF) as u16;
                    let open = w!("open");
                    let github = to_wstring(GITHUB_URL);
                    let license = to_wstring(LICENSE_URL);
                    
                    if id == 1001 { 
                        ShellExecuteW(std::ptr::null_mut(), open.as_ptr(), github.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
                    } else if id == 1002 { 
                         ShellExecuteW(std::ptr::null_mut(), open.as_ptr(), license.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
                    }
                    Some(0)
                },
                0x8002 => {
                    let new_is_dark = wparam == 1;
                    self.is_dark = new_is_dark;
                    
                    crate::ui::theme::set_window_frame_theme(hwnd, new_is_dark);
                    
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                    Some(0)
                },
                
                _ => None,
            }
        }
    }
}