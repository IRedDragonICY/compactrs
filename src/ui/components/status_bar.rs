#![allow(unsafe_op_in_unsafe_fn)]

//! StatusBar component - encapsulates the status label and progress bar.
//!
//! This component manages the status display area at the bottom of the main window,
//! containing a static text label for status messages and a progress bar.

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{PBS_SMOOTH, PROGRESS_CLASSW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, SetWindowPos, SWP_NOZORDER, WS_CHILD, WS_VISIBLE,
    SendMessageW, WM_SETFONT, HMENU,
    GetWindowLongW, SetWindowLongW, GWL_STYLE, GWL_EXSTYLE,
    WS_BORDER, WS_DLGFRAME, WS_EX_CLIENTEDGE, WS_EX_STATICEDGE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_FRAMECHANGED,
    SetWindowLongPtrW, CallWindowProcW, GWLP_WNDPROC, WM_NCPAINT, WM_NCCALCSIZE,
    WM_DESTROY, SetPropW, GetPropW, RemovePropW,
};
use windows_sys::Win32::Graphics::Gdi::HFONT;

use super::base::Component;

use crate::w;

/// Subclass procedure to strip borders from Progress Bar
unsafe extern "system" fn progress_subclass_proc(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    match msg {
        WM_NCPAINT => {
            // Prevent painting the non-client area (border)
            return 0;
        },
        WM_NCCALCSIZE => {
            // If true, the application should select which part of the window area contains valid information.
            // By default, returning 0 with valid RECT implies client_rect == window_rect (no border).
            if wparam != 0 {
                return 0;
            }
        },
        WM_DESTROY => {
            // Restore original proc just in case (though window is dying)
            // and remove property
            let old_proc_ptr = GetPropW(hwnd, crate::w!("CompactRs_OldProc").as_ptr());
            if old_proc_ptr != std::ptr::null_mut() {
                SetWindowLongPtrW(hwnd, GWLP_WNDPROC, old_proc_ptr as isize);
                RemovePropW(hwnd, crate::w!("CompactRs_OldProc").as_ptr());
            }
        },
        _ => {}
    }
    
    // Call original proc
    let old_proc_ptr = GetPropW(hwnd, crate::w!("CompactRs_OldProc").as_ptr());
    if old_proc_ptr != std::ptr::null_mut() {
        let old_proc: unsafe extern "system" fn(HWND, u32, usize, isize) -> isize = std::mem::transmute(old_proc_ptr);
        return CallWindowProcW(Some(old_proc), hwnd, msg, wparam, lparam);
    }
    
    0
}

/// Configuration for StatusBar control IDs.
pub struct StatusBarIds {
    pub label_id: u16,
    pub progress_id: u16,
}

/// StatusBar component containing a status label and progress bar.
///
/// # Layout
/// The progress bar is positioned at the bottom of the allocated area,
/// with the label positioned just above it.
pub struct StatusBar {
    hwnd_label: HWND,
    hwnd_progress: HWND,
    label_id: u16,
    progress_id: u16,
    // Cached layout values
    x: i32,
    y: i32,
    width: i32,
}

impl StatusBar {
    /// Creates a new StatusBar with uninitialized handles.
    ///
    /// Call `create()` to actually create the Win32 controls.
    pub fn new(ids: StatusBarIds) -> Self {
        Self {
            hwnd_label: std::ptr::null_mut(),
            hwnd_progress: std::ptr::null_mut(),
            label_id: ids.label_id,
            progress_id: ids.progress_id,
            x: 0,
            y: 0,
            width: 0,
        }
    }

    /// Returns the label HWND for direct access (e.g., for SetWindowTextW).
    #[inline]
    pub fn label_hwnd(&self) -> HWND {
        self.hwnd_label
    }

    /// Returns the progress bar HWND for direct access (e.g., for PBM_SETPOS).
    #[inline]
    pub fn progress_hwnd(&self) -> HWND {
        self.hwnd_progress
    }

    /// Sets the font for the label control.
    ///
    /// # Arguments
    /// * `hfont` - The font handle to apply
    ///
    /// # Safety
    /// Calls Win32 SendMessageW API.
    pub unsafe fn set_font(&self, hfont: HFONT) {
        let wparam = hfont as usize;
        let lparam = 1; // Redraw
        
        SendMessageW(self.hwnd_label, WM_SETFONT, wparam, lparam);
    }

    // Local allow_dark_mode_for_window removed


}

impl Component for StatusBar {
    unsafe fn create(&mut self, parent: HWND) -> Result<(), String> {
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());

            // Create header/status label
            let label_text = w!("Drag and drop files or folders, or use 'Files'/'Folder' buttons. Then click 'Process All'.");
            let static_cls = w!("STATIC");
            
            self.hwnd_label = CreateWindowExW(
                0,
                static_cls.as_ptr(),
                label_text.as_ptr(),
                WS_CHILD | WS_VISIBLE,
                10,
                10,
                860,
                25,
                parent,
                self.label_id as usize as HMENU,
                instance,
                std::ptr::null(),
            );
            
            if self.hwnd_label == std::ptr::null_mut() {
                return Err("Failed to create status label".to_string());
            }

            // Create progress bar
            let empty_text = w!("");
            
            self.hwnd_progress = CreateWindowExW(
                0,
                PROGRESS_CLASSW,
                empty_text.as_ptr(),
                WS_VISIBLE | WS_CHILD | PBS_SMOOTH,
                10,
                430,
                860,
                20,
                parent,
                self.progress_id as usize as HMENU,
                instance,
                std::ptr::null(),
            );

            if self.hwnd_progress == std::ptr::null_mut() {
                return Err("Failed to create progress bar".to_string());
            }

            // Subclass to remove borders cleanly (WM_NCPAINT/WM_NCCALCSIZE)
            let old_proc = SetWindowLongPtrW(self.hwnd_progress, GWLP_WNDPROC, progress_subclass_proc as *const () as isize);
            SetPropW(self.hwnd_progress, crate::w!("CompactRs_OldProc").as_ptr(), old_proc as isize as _);

            // Also remove styles just to be sure
            let mut style = GetWindowLongW(self.hwnd_progress, GWL_STYLE) as u32;
            style &= !(WS_BORDER | WS_DLGFRAME);
            SetWindowLongW(self.hwnd_progress, GWL_STYLE, style as i32);

            let mut ex_style = GetWindowLongW(self.hwnd_progress, GWL_EXSTYLE) as u32;
            ex_style &= !(WS_EX_CLIENTEDGE | WS_EX_STATICEDGE);
            SetWindowLongW(self.hwnd_progress, GWL_EXSTYLE, ex_style as i32);
            
            SetWindowPos(self.hwnd_progress, std::ptr::null_mut(), 0, 0, 0, 0, 
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);

            Ok(())
        }
    }

    fn hwnd(&self) -> Option<HWND> {
        // Return the progress bar as the "main" HWND (though both are important)
        Some(self.hwnd_progress)
    }

    unsafe fn on_resize(&mut self, parent_rect: &RECT) {
        unsafe {
            let width = parent_rect.right - parent_rect.left;
            let height = parent_rect.bottom - parent_rect.top;

            let padding = 10;
            let header_height = 25;
            let progress_height = 20; // Slightly slimmer

            // Cache values for potential future use
            self.x = padding;
            self.width = width - padding * 2;
            
            // Base Y offset from the provided rect (For Progress Bar only)
            let base_y = parent_rect.top;

            // 1. Position Label at Top (Instruction Text / Header)
            // Always position at absolute top (padding), ignoring the passed rect's top.
            // This ensures it acts as the Window Header regardless of where the Progress Bar is moved.
            let label_abs_y = padding; 
            SetWindowPos(
                self.hwnd_label,
                std::ptr::null_mut(),
                padding,
                label_abs_y, 
                width - padding - 220, // Reserve 220px for header buttons (5 buttons × 35px + padding)
                header_height,
                SWP_NOZORDER,
            );

            // 2. Position Progress Bar (Relative to passed Rect)
            // We want this to occupy the 'status_rect' passed from window.rs
            // Center in the provided vertical space
            let progress_local_y = (height - progress_height) / 2;
            
            self.y = base_y + progress_local_y;
            SetWindowPos(
                self.hwnd_progress,
                std::ptr::null_mut(),
                padding,
                self.y,
                width - padding * 2,
                progress_height,
                SWP_NOZORDER,
            );
        }
    }

    unsafe fn on_theme_change(&mut self, is_dark: bool) {
        unsafe {
            if self.hwnd_progress != std::ptr::null_mut() {
                // Apply theme first (Disables visual styles in dark mode via theme.rs)
                crate::ui::theme::apply_theme(self.hwnd_progress, crate::ui::theme::ControlType::ProgressBar, is_dark);
                
                // Colors
                const PBM_SETBKCOLOR: u32 = 0x2001;
                const PBM_SETBARCOLOR: u32 = 1033;
                
                if is_dark {
                    // Dark Mode: Dark Track, Green Bar
                    SendMessageW(self.hwnd_progress, PBM_SETBKCOLOR, 0, crate::ui::theme::COLOR_DARK_BG as isize);
                    SendMessageW(self.hwnd_progress, PBM_SETBARCOLOR, 0, 0x0000D000); // Slightly darker green for contrast
                } else {
                    // Light Mode: Default system colors
                    // To restore system colors, we usually send CLR_DEFAULT (0xFF000000 or similar?) or just let theme handle it.
                    // But since we enabled "Explorer" theme for light mode, it should ignore these messages or override them.
                    // However, to be safe, we can set them to standard windows colors if needed.
                    // Actually, "Explorer" theme ignores PBM_SETBKCOLOR usually.
                }
            }
            
            // Re-apply label theme/font if needed (Label is usually transparent usage of parent)
             if self.hwnd_label != std::ptr::null_mut() {
                 crate::ui::theme::apply_theme(self.hwnd_label, crate::ui::theme::ControlType::Window, is_dark);
                 SendMessageW(self.hwnd_label, WM_SETFONT, crate::ui::theme::get_app_font() as usize, 1);
             }
        }
    }
}
