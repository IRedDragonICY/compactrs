#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::builder::ControlBuilder;
use crate::ui::framework::WindowHandler;
use crate::ui::layout::{LayoutNode, SizePolicy, AlignItems, JustifyContent};

use crate::types::*;

const SHORTCUTS_TITLE: &str = "Keyboard Shortcuts";

struct ShortcutsState {
    is_dark: bool,
    h_font_bold: HFONT,
    h_font_regular: HFONT,
}

pub unsafe fn show_shortcuts_modal(parent: HWND, is_dark: bool) {
    let mut state = ShortcutsState { 
        is_dark,
        h_font_bold: std::ptr::null_mut(),
        h_font_regular: std::ptr::null_mut(),
    };
    crate::ui::dialogs::base::show_modal_singleton(
        parent, 
        &mut state, 
        "CompactRS_Shortcuts", 
        SHORTCUTS_TITLE, 
        480, 
        400, 
        is_dark
    );
}

impl WindowHandler for ShortcutsState {
    fn is_dark_mode(&self) -> bool {
        self.is_dark
    }

    fn on_create(&mut self, hwnd: HWND) -> LRESULT {
        unsafe {
            let is_dark_mode = self.is_dark;
            crate::ui::theme::set_window_frame_theme(hwnd, is_dark_mode);

            let h_default = GetStockObject(DEFAULT_GUI_FONT);
            let mut lf: LOGFONTW = std::mem::zeroed();
            GetObjectW(h_default, std::mem::size_of::<LOGFONTW>() as i32, &mut lf as *mut _ as *mut _);
            
            lf.lfWeight = FW_BOLD as i32;
            lf.lfHeight = crate::ui::theme::scale(-14);
            self.h_font_bold = CreateFontIndirectW(&lf);
            
            lf.lfWeight = FW_NORMAL as i32;
            self.h_font_regular = CreateFontIndirectW(&lf);

            let shortcuts = [
                ("Ctrl + O", "Add Files"),
                ("Ctrl + Shift + O", "Add Folder"),
                ("Ctrl + V", "Paste Files from Clipboard"),
                ("Del", "Remove Selected Items"),
                ("Ctrl + A", "Select All Items"),
                ("Double Click (Path)", "Open File Location"),
                ("Double Click (Algo)", "Cycle Compression Algorithm"),
                ("Double Click (Action)", "Toggle Compress/Decompress"),
                ("Space", "Start Processing Selected"),
                ("Ctrl + Space", "Pause/Resume Selected"),
            ];
            
            const SS_RIGHT: u32 = 0x2;

            let mut col_node = LayoutNode::col(20, 8).align_items(AlignItems::Stretch);

            for (key, desc) in shortcuts.iter() {
                let h_key = ControlBuilder::new(hwnd, 0)
                    .label(false)
                    .text(key)
                    .font(self.h_font_bold)
                    .style(SS_RIGHT)
                    .dark_mode(is_dark_mode)
                    .build();

                let h_desc = ControlBuilder::new(hwnd, 0)
                    .label(false)
                    .text(desc)
                    .font(self.h_font_regular)
                    .dark_mode(is_dark_mode)
                    .build();
                    
                col_node.add_child(LayoutNode::row(0, 15)
                    .justify_content(JustifyContent::Center)
                    .with(h_key, SizePolicy::Fixed(180))
                    .with(h_desc, SizePolicy::Fixed(200))
                    .with_policy(SizePolicy::Fixed(24))
                );
            }

            let client_rect = crate::utils::get_client_rect(hwnd);
            col_node.apply_layout(client_rect);

            crate::ui::theme::apply_theme_recursive(hwnd, is_dark_mode);
        }
        0
    }

    fn on_message(&mut self, _hwnd: HWND, msg: u32, _wparam: WPARAM, _lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            if msg == WM_DESTROY {
                if self.h_font_bold != std::ptr::null_mut() {
                    DeleteObject(self.h_font_bold);
                }
                if self.h_font_regular != std::ptr::null_mut() {
                    DeleteObject(self.h_font_regular);
                }
            }
        }
        None
    }
}