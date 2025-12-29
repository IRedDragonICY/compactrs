#![allow(unsafe_op_in_unsafe_fn)]
use crate::types::*;


/// Standard Window Procedure for child panels.
/// Handles theming, background erasure, and command forwarding.
unsafe extern "system" fn standard_panel_proc(hwnd: HWND, umsg: u32, wparam: usize, lparam: isize) -> isize {
    match umsg {
        WM_COMMAND | WM_NOTIFY | WM_DRAWITEM => {
             // Forward notifications to parent
             let parent = GetParent(hwnd);
             if parent != std::ptr::null_mut() {
                 SendMessageW(parent, umsg, wparam, lparam);
             }
             if umsg == WM_DRAWITEM {
                 return 1; // Handled
             }
             return 0;
        },
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let prop_val = GetPropW(hwnd, crate::w!("CompactRs_Theme").as_ptr()) as usize;
            let is_dark = if prop_val != 0 { prop_val == 2 } else { crate::ui::theme::is_system_dark_mode() };

            if let Some(res) = crate::ui::theme::handle_standard_colors(hwnd, umsg, wparam, is_dark) {
                return res as isize;
            }
        },
        WM_ERASEBKGND => {
            let hdc = wparam as HDC;
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            
            let prop_val = GetPropW(hwnd, crate::w!("CompactRs_Theme").as_ptr()) as usize;
            let is_dark = if prop_val != 0 { prop_val == 2 } else { crate::ui::theme::is_system_dark_mode() };

            let brush = if is_dark {
                crate::ui::theme::get_dark_brush()
            } else {
                 (COLOR_WINDOW + 1) as HBRUSH
            };
            
            unsafe { FillRect(hdc, &rect, brush); }
            return 1;
        },
        WM_DESTROY => {
            RemovePropW(hwnd, crate::w!("CompactRs_Theme").as_ptr());
        },
        _ => {}
    }
    
    DefWindowProcW(hwnd, umsg, wparam, lparam)
}

/// Helper struct for creating standard child panels.
pub struct Panel;

impl Panel {
    /// Registers and creates a child panel window with standard behavior.
    /// 
    /// # Arguments
    /// * `parent` - The parent window
    /// * `class_id` - Unique string identifier for the window class (e.g. "SearchPanel")
    /// * `x, y, w, h` - Initial position/size
    /// 
    /// Automatically applies `WS_EX_CONTROLPARENT` and a standard WndProc.
    pub unsafe fn create(parent: HWND, class_id: &str, x: i32, y: i32, w: i32, h: i32) -> Result<HWND, String> {
        let instance = GetModuleHandleW(std::ptr::null());
        let class_name_w = crate::utils::to_wstring(class_id);
        
        // Register Class (ignore error if exists)
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(standard_panel_proc);
        wc.hInstance = instance;
        wc.lpszClassName = class_name_w.as_ptr();
        wc.style = CS_HREDRAW | CS_VREDRAW;
        wc.hbrBackground = std::ptr::null_mut(); 
        
        RegisterClassW(&wc);
        
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class_name_w.as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS, 
            x, y, w, h,
            parent,
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        );

        if hwnd == std::ptr::null_mut() {
            Err(format!("Failed to create panel: {}", class_id))
        } else {
            Ok(hwnd)
        }
    }
    
    /// Updates the theme property for the panel.
    pub unsafe fn update_theme(hwnd: HWND, is_dark: bool) {
        let prop_val = if is_dark { 2 } else { 1 };
        SetPropW(hwnd, crate::w!("CompactRs_Theme").as_ptr(), prop_val as isize as _);
        InvalidateRect(hwnd, std::ptr::null(), 1);
    }
}
