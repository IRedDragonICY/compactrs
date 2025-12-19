#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::builder::ControlBuilder;
use crate::ui::framework::{Window, WindowHandler};
use crate::utils::to_wstring;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    WS_VISIBLE, WS_CAPTION, WS_SYSMENU, WS_POPUP,
    WM_DESTROY,
    GetWindowRect,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontW, FW_BOLD, FW_NORMAL, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
    DEFAULT_PITCH, FF_DONTCARE, HFONT,
};

const SHORTCUTS_TITLE: &str = "Keyboard Shortcuts";

struct ShortcutsState {
    is_dark: bool,
}

pub unsafe fn show_shortcuts_modal(parent: HWND, is_dark: bool) {
    let instance = GetModuleHandleW(std::ptr::null());
    
    // Calculate center
    let mut rect: RECT = std::mem::zeroed();
    GetWindowRect(parent, &mut rect);
    let p_width = rect.right - rect.left;
    let p_height = rect.bottom - rect.top;
    let width = 500;
    let height = 320;
    let x = rect.left + (p_width - width) / 2;
    let y = rect.top + (p_height - height) / 2;

    let mut state = ShortcutsState { is_dark };
    
     // Check if window already exists mechanism is implicitly handled by the framework if we use a singleton pattern,
    // but here we are creating a unique modal-like (popup) window.
    // The previous implementation used FindWindowW on class name.
    // Since we are changing to a generic class name potentially, or we need to ensure unique class.
    // However, `Window::<T>::create` registers a unique class based on T usually? NO, it uses the passed class name.
    // We will use a unique class name.
    
    let class_name = "CompactRS_Shortcuts";

    // Use the framework to create the window
    let _hwnd = Window::<ShortcutsState>::create(
        &mut state,
        class_name,
        SHORTCUTS_TITLE,
        WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
        0, // ex_style
        x, y, width, height,
        parent,
        crate::ui::framework::load_app_icon(instance),
        crate::ui::theme::get_background_brush(is_dark)
    ).unwrap_or(std::ptr::null_mut());
    
    if _hwnd != std::ptr::null_mut() {
        crate::ui::framework::run_message_loop();
    }
}

impl WindowHandler for ShortcutsState {
    fn on_create(&mut self, hwnd: HWND) -> LRESULT {
        unsafe {
             let is_dark_mode = self.is_dark;
             crate::ui::theme::set_window_frame_theme(hwnd, is_dark_mode);

             // Fonts
             let segoe_ui_var = to_wstring("Segoe UI Variable Display");
             let key_font = CreateFontW(
                 -16, 0, 0, 0, FW_BOLD as i32, 0, 0, 0, DEFAULT_CHARSET as u32,
                 OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32,
                 (DEFAULT_PITCH | FF_DONTCARE) as u32, segoe_ui_var.as_ptr()) as HFONT;

             let desc_font = CreateFontW(
                 -16, 0, 0, 0, FW_NORMAL as i32, 0, 0, 0, DEFAULT_CHARSET as u32,
                 OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32,
                 (DEFAULT_PITCH | FF_DONTCARE) as u32, segoe_ui_var.as_ptr()) as HFONT;

             let shortcuts = [
                 ("Ctrl + O", "Add Files"),
                 ("Ctrl + Shift + O", "Add Folder"),
                 ("Ctrl + V", "Paste Files/Paths from Clipboard"),
                 ("Del", "Remove Selected Items"),
                 ("Ctrl + A", "Select All"),
                 ("Double Click (Path)", "Open File Location"),
                 ("Double Click (Algo)", "Cycle Algorithm"),
                 ("Double Click (Action)", "Toggle Compress/Decompress"),
             ];

             let start_y = 25;
             let row_h = 32;
             let col1_w = 180;
             let margin = 30;
             
             const SS_RIGHT: u32 = 0x2;

             for (i, (key, desc)) in shortcuts.iter().enumerate() {
                 let y = start_y + (i as i32 * row_h);

                 // Key Column (Right Aligned)
                 ControlBuilder::new(hwnd, 0)
                     .label(false)
                     .text(key)
                     .pos(margin, y)
                     .size(col1_w, 25)
                     .font(key_font)
                     .style(SS_RIGHT)
                     .dark_mode(is_dark_mode)
                     .build();

                 // Description Column (Left Aligned)
                 ControlBuilder::new(hwnd, 0)
                     .label(false)
                     .text(desc)
                     .pos(margin + col1_w + 20, y)
                     .size(250, 25)
                     .font(desc_font)
                     .dark_mode(is_dark_mode)
                     .build();
             }
        }
        0
    }

    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, _lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
             if let Some(result) = crate::ui::theme::handle_standard_colors(hwnd, msg, wparam, self.is_dark) {
                return Some(result);
            }

            match msg {
                WM_DESTROY => {
                    windows_sys::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
                    Some(0)
                },
                _ => None
            }
        }
    }
}
