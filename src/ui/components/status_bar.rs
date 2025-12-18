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
};
use windows_sys::Win32::Graphics::Gdi::HFONT;

use super::base::Component;
use crate::utils::to_wstring;

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
            let label_text = to_wstring("Drag and drop files or folders, or use 'Files'/'Folder' buttons. Then click 'Process All'.");
            let static_cls = to_wstring("STATIC");
            
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
            let empty_text = to_wstring("");
            
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
            let progress_height = 25;
            let btn_height = 30;
            let lbl_height = 18;  // Space for labels above dropdowns

            // Calculate ListView height to determine where progress bar goes
            // Account for label height above action buttons
            let list_height = height - header_height - progress_height - btn_height - lbl_height - (padding * 5);

            // Cache values for potential future use
            self.x = padding;
            self.width = width - padding * 2;

            // Position label at top (leave space for top-right buttons: 120px)
            let label_y = padding;
            SetWindowPos(
                self.hwnd_label,
                std::ptr::null_mut(),
                padding,
                label_y,
                width - padding - 120,
                header_height,
                SWP_NOZORDER,
            );

            // Position progress bar after ListView
            let progress_y = padding + header_height + padding + list_height + padding;
            self.y = progress_y;
            SetWindowPos(
                self.hwnd_progress,
                std::ptr::null_mut(),
                padding,
                progress_y,
                width - padding * 2,
                progress_height,
                SWP_NOZORDER,
            );
        }
    }

    unsafe fn on_theme_change(&mut self, is_dark: bool) {
        unsafe {
            // Apply theme to progress bar
            if self.hwnd_progress != std::ptr::null_mut() {
                crate::ui::theme::allow_dark_mode_for_window(self.hwnd_progress, is_dark);
                crate::ui::theme::apply_theme(self.hwnd_progress, crate::ui::theme::ControlType::Window, is_dark);
            }
        }
    }
}
