//! HeaderPanel component - manages the header area with top-right buttons.
//!
//! This component contains the Settings, About, and Console buttons
//! positioned in the top-right corner of the main window.

use windows::core::{w, Result, PCWSTR};
use windows::Win32::Foundation::{HWND, HINSTANCE, RECT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    BS_PUSHBUTTON, CreateWindowExW, SetWindowPos, HMENU, SWP_NOZORDER, WINDOW_STYLE,
    WS_CHILD, WS_VISIBLE,
};

use super::base::Component;
use crate::ui::controls::apply_button_theme;
use crate::ui::utils::ToWide;

/// Configuration for HeaderPanel control IDs.
pub struct HeaderPanelIds {
    pub btn_settings: u16,
    pub btn_about: u16,
    pub btn_console: u16,
}

/// HeaderPanel component containing the top-right action buttons.
///
/// # Layout
/// Buttons are positioned in the top-right corner:
/// [>_] [?] [⚙]
/// Console, About, Settings (right to left)
pub struct HeaderPanel {
    hwnd_settings: HWND,
    hwnd_about: HWND,
    hwnd_console: HWND,
    ids: HeaderPanelIds,
}

impl HeaderPanel {
    /// Creates a new HeaderPanel with uninitialized handles.
    ///
    /// Call `create()` to actually create the Win32 controls.
    pub fn new(ids: HeaderPanelIds) -> Self {
        Self {
            hwnd_settings: HWND::default(),
            hwnd_about: HWND::default(),
            hwnd_console: HWND::default(),
            ids,
        }
    }

    /// Returns the Settings button HWND.
    #[inline]
    pub fn settings_hwnd(&self) -> HWND {
        self.hwnd_settings
    }

    /// Returns the About button HWND.
    #[inline]
    pub fn about_hwnd(&self) -> HWND {
        self.hwnd_about
    }

    /// Returns the Console button HWND.
    #[inline]
    pub fn console_hwnd(&self) -> HWND {
        self.hwnd_console
    }

    /// Sets the font for all child controls.
    ///
    /// # Arguments
    /// * `hfont` - The font handle to apply
    ///
    /// # Safety
    /// Calls Win32 SendMessageW API.
    pub unsafe fn set_font(&self, hfont: windows::Win32::Graphics::Gdi::HFONT) {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_SETFONT};
        
        unsafe {
            let wparam = WPARAM(hfont.0 as usize);
            let lparam = LPARAM(1); // Redraw
            
            SendMessageW(self.hwnd_settings, WM_SETFONT, Some(wparam), Some(lparam));
            SendMessageW(self.hwnd_about, WM_SETFONT, Some(wparam), Some(lparam));
            SendMessageW(self.hwnd_console, WM_SETFONT, Some(wparam), Some(lparam));
        }
    }

    /// Helper to create a button.
    unsafe fn create_button(
        parent: HWND,
        instance: HINSTANCE,
        text: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        id: u16,
        is_dark: bool,
    ) -> Result<HWND> {
        unsafe {
            let text_wide = text.to_wide();
            let hwnd = CreateWindowExW(
                Default::default(),
                w!("BUTTON"),
                PCWSTR::from_raw(text_wide.as_ptr()),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | BS_PUSHBUTTON as u32),
                x,
                y,
                w,
                h,
                Some(parent),
                Some(HMENU(id as isize as *mut _)),
                Some(instance),
                None,
            )?;
            apply_button_theme(hwnd, is_dark);
            Ok(hwnd)
        }
    }
}

impl Component for HeaderPanel {
    unsafe fn create(&mut self, parent: HWND) -> Result<()> {
        unsafe {
            let module = GetModuleHandleW(None)?;
            let instance = HINSTANCE(module.0);

            let is_dark = crate::ui::theme::is_system_dark_mode();

            // Initial positions (will be updated in on_resize)
            // These are just placeholders - real positions set in on_resize
            self.hwnd_settings = Self::create_button(
                parent,
                instance,
                "\u{2699}",  // Gear icon
                0,
                0,
                30,
                25,
                self.ids.btn_settings,
                is_dark,
            )?;

            self.hwnd_about = Self::create_button(
                parent,
                instance,
                "?",
                0,
                0,
                30,
                25,
                self.ids.btn_about,
                is_dark,
            )?;

            self.hwnd_console = Self::create_button(
                parent,
                instance,
                ">_",
                0,
                0,
                30,
                25,
                self.ids.btn_console,
                is_dark,
            )?;

            Ok(())
        }
    }

    fn hwnd(&self) -> Option<HWND> {
        // This component doesn't have a single "main" HWND
        None
    }

    unsafe fn on_resize(&mut self, parent_rect: &RECT) {
        unsafe {
            let width = parent_rect.right - parent_rect.left;

            let padding = 10;
            let btn_width = 30;
            let header_height = 25;

            // Position Settings button (Rightmost)
            SetWindowPos(
                self.hwnd_settings,
                None,
                width - padding - btn_width,
                padding,
                btn_width,
                header_height,
                SWP_NOZORDER,
            );

            // Position About button (Left of Settings)
            SetWindowPos(
                self.hwnd_about,
                None,
                width - padding - btn_width - 35,
                padding,
                btn_width,
                header_height,
                SWP_NOZORDER,
            );

            // Position Console button (Left of About)
            SetWindowPos(
                self.hwnd_console,
                None,
                width - padding - btn_width - 70,
                padding,
                btn_width,
                header_height,
                SWP_NOZORDER,
            );
        }
    }

    unsafe fn on_theme_change(&mut self, is_dark: bool) {
        unsafe {
            apply_button_theme(self.hwnd_settings, is_dark);
            apply_button_theme(self.hwnd_about, is_dark);
            apply_button_theme(self.hwnd_console, is_dark);
        }
    }
}
