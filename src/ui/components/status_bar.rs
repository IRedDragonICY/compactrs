//! StatusBar component - encapsulates the status label and progress bar.
//!
//! This component manages the status display area at the bottom of the main window,
//! containing a static text label for status messages and a progress bar.

use windows::core::{w, Result};
use windows::Win32::Foundation::{HWND, HINSTANCE, RECT};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Controls::{PBS_SMOOTH, PROGRESS_CLASSW, SetWindowTheme};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, SetWindowPos, HMENU, SWP_NOZORDER, WS_CHILD, WS_VISIBLE,
    WINDOW_STYLE,
};

use super::base::Component;

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
            hwnd_label: HWND::default(),
            hwnd_progress: HWND::default(),
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

    /// Enables/disables dark mode for a window using undocumented uxtheme API.
    #[allow(non_snake_case)]
    unsafe fn allow_dark_mode_for_window(hwnd: HWND, allow: bool) {
        unsafe {
            if let Ok(uxtheme) = LoadLibraryW(w!("uxtheme.dll")) {
                if let Some(func) = GetProcAddress(uxtheme, windows::core::PCSTR(133 as *const u8))
                {
                    let allow_dark: extern "system" fn(HWND, bool) -> bool =
                        std::mem::transmute(func);
                    allow_dark(hwnd, allow);
                }
            }
        }
    }
}

impl Component for StatusBar {
    unsafe fn create(&mut self, parent: HWND) -> Result<()> {
        unsafe {
            let module = GetModuleHandleW(None)?;
            let instance = HINSTANCE(module.0);

            // Create header/status label
            self.hwnd_label = CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                w!("Drag and drop files or folders, or use 'Files'/'Folder' buttons. Then click 'Process All'."),
                WS_CHILD | WS_VISIBLE,
                10,
                10,
                860,
                25,
                Some(parent),
                Some(HMENU(self.label_id as isize as *mut _)),
                Some(instance),
                None,
            )?;

            // Create progress bar
            self.hwnd_progress = CreateWindowExW(
                Default::default(),
                PROGRESS_CLASSW,
                w!(""),
                WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | PBS_SMOOTH as u32),
                10,
                430,
                860,
                20,
                Some(parent),
                Some(HMENU(self.progress_id as isize as *mut _)),
                Some(instance),
                None,
            )?;

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

            // Calculate ListView height to determine where progress bar goes
            let list_height = height - header_height - progress_height - btn_height - (padding * 5);

            // Cache values for potential future use
            self.x = padding;
            self.width = width - padding * 2;

            // Position label at top (leave space for top-right buttons: 120px)
            let label_y = padding;
            SetWindowPos(
                self.hwnd_label,
                None,
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
                None,
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
            if !self.hwnd_progress.is_invalid() {
                Self::allow_dark_mode_for_window(self.hwnd_progress, is_dark);
                if is_dark {
                    let _ = SetWindowTheme(self.hwnd_progress, w!("DarkMode_Explorer"), None);
                } else {
                    let _ = SetWindowTheme(self.hwnd_progress, w!("Explorer"), None);
                }
            }
        }
    }
}
