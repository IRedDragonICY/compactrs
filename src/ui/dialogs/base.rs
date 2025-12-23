#![allow(unsafe_op_in_unsafe_fn)]
use crate::types::*;
use crate::ui::framework::{WindowHandler, WindowBuilder, WindowAlignment, show_modal};

/// Helper to show a modal dialog, or bring it to front if already exists.
/// Returns true if the modal loop ran (window closed), false if an existing window was brought to front.
pub unsafe fn show_modal_singleton<T: WindowHandler>(
    parent: HWND, 
    state: &mut T, 
    class_name: &str, 
    title: &str, 
    width: i32, 
    height: i32,
    is_dark: bool
) -> bool {
    let class_name_w = crate::utils::to_wstring(class_name);
    let existing_hwnd = FindWindowW(class_name_w.as_ptr(), std::ptr::null());
    
    if existing_hwnd != std::ptr::null_mut() {
        ShowWindow(existing_hwnd, SW_RESTORE);
        SetForegroundWindow(existing_hwnd);
        return false;
    }
    
    let bg_brush = crate::ui::theme::get_background_brush(is_dark);
    show_modal(
        WindowBuilder::new(state, class_name, title)
            .style(WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE)
            .size(width, height)
            .align(WindowAlignment::CenterOnParent)
            .background(bg_brush),
        parent
    );
    true
}
