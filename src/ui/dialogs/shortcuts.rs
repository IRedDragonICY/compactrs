#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::builder::ControlBuilder;
use crate::ui::framework::WindowHandler;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontIndirectW, GetObjectW, GetStockObject, DeleteObject,
    FW_BOLD, FW_NORMAL, HFONT, LOGFONTW, DEFAULT_GUI_FONT,
};

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

            // Create fonts
            let h_default = GetStockObject(DEFAULT_GUI_FONT);
            let mut lf: LOGFONTW = std::mem::zeroed();
            GetObjectW(h_default, std::mem::size_of::<LOGFONTW>() as i32, &mut lf as *mut _ as *mut _);
            
            // Bold font for keys
            lf.lfWeight = FW_BOLD as i32;
            lf.lfHeight = -14;
            self.h_font_bold = CreateFontIndirectW(&lf);
            
            // Regular font for descriptions
            lf.lfWeight = FW_NORMAL as i32;
            self.h_font_regular = CreateFontIndirectW(&lf);

            // Shortcuts: (key, description)
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

            let start_y = 20;
            let row_h = 32;
            let key_x = 30;
            let key_w = 170;
            let desc_x = key_x + key_w + 25;
            
            const SS_RIGHT: u32 = 0x2;

            for (i, (key, desc)) in shortcuts.iter().enumerate() {
                let y = start_y + (i as i32 * row_h);

                // Key Column (Right Aligned, Bold)
                ControlBuilder::new(hwnd, 0)
                    .label(false)
                    .text(key)
                    .pos(key_x, y)
                    .size(key_w, 24)
                    .font(self.h_font_bold)
                    .style(SS_RIGHT)
                    .dark_mode(is_dark_mode)
                    .build();

                // Description Column (Left Aligned, Regular)
                ControlBuilder::new(hwnd, 0)
                    .label(false)
                    .text(desc)
                    .pos(desc_x, y)
                    .size(220, 24)
                    .font(self.h_font_regular)
                    .dark_mode(is_dark_mode)
                    .build();
            }

            crate::ui::theme::apply_theme_recursive(hwnd, is_dark_mode);
        }
        0
    }

    fn on_message(&mut self, _hwnd: HWND, msg: u32, _wparam: WPARAM, _lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            if msg == windows_sys::Win32::UI::WindowsAndMessaging::WM_DESTROY {
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
