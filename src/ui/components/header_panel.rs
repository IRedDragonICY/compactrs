#![allow(unsafe_op_in_unsafe_fn)]

//! HeaderPanel component - manages the header area with top-right buttons.
//!
//! This component contains the Settings, About, and Console buttons
//! positioned in the top-right corner of the main window.

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, SWP_NOZORDER,
};
use windows_sys::Win32::Graphics::Gdi::HFONT;

use super::base::Component;
use crate::ui::builder::ButtonBuilder;
use crate::ui::controls::apply_button_theme;

const ICON_SETTINGS: &[u16] = &[0xE713, 0]; // Settings
const ICON_KEYBOARD: &[u16] = &[0xE765, 0]; // Keyboard
const ICON_ABOUT: &[u16] = &[0xE946, 0];    // Info
const ICON_CONSOLE: &[u16] = &[0xE756, 0];  // CommandPrompt
const ICON_WATCHER: &[u16] = &[0xE9D2, 0];  // Clock/Alarm

/// Configuration for HeaderPanel control IDs.
pub struct HeaderPanelIds {
    pub btn_settings: u16,
    pub btn_about: u16,
    pub btn_shortcuts: u16,
    pub btn_console: u16,
    pub btn_watcher: u16,
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
    hwnd_shortcuts: HWND,
    hwnd_console: HWND,
    hwnd_watcher: HWND,
    ids: HeaderPanelIds,
}

impl HeaderPanel {
    /// Creates a new HeaderPanel with uninitialized handles.
    ///
    /// Call `create()` to actually create the Win32 controls.
    pub fn new(ids: HeaderPanelIds) -> Self {
        Self {
            hwnd_settings: std::ptr::null_mut(),
            hwnd_about: std::ptr::null_mut(),
            hwnd_shortcuts: std::ptr::null_mut(),
            hwnd_console: std::ptr::null_mut(),
            hwnd_watcher: std::ptr::null_mut(),
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

    /// Returns the Shortcuts button HWND.
    #[inline]
    pub fn shortcuts_hwnd(&self) -> HWND {
        self.hwnd_shortcuts
    }

    /// Returns the Console button HWND.
    #[inline]
    pub fn console_hwnd(&self) -> HWND {
        self.hwnd_console
    }

    /// Returns the Watcher button HWND.
    #[inline]
    pub fn watcher_hwnd(&self) -> HWND {
        self.hwnd_watcher
    }

    /// Sets the font for all child controls.
    ///
    /// # Arguments
    /// * `hfont` - The font handle to apply
    ///
    /// # Safety
    /// Calls Win32 SendMessageW API.
    pub unsafe fn set_font(&self, hfont: HFONT) {
        let _ = hfont;
        // Do NOT apply app font to these buttons as they use specific Icon Font.
    }
}

impl Component for HeaderPanel {
    unsafe fn create(&mut self, parent: HWND) -> Result<(), String> {
        unsafe {
            let _module = GetModuleHandleW(std::ptr::null());

            let is_dark = crate::ui::theme::is_system_dark_mode();
            let icon_font = crate::ui::theme::get_icon_font();

            // Initial positions (will be updated in on_resize)
            // These are just placeholders - real positions set in on_resize
            self.hwnd_settings = ButtonBuilder::new(parent, self.ids.btn_settings)
                .text_w(ICON_SETTINGS)
                .pos(0, 0).size(30, 25).dark_mode(is_dark)
                .font(icon_font)
                .build();

            self.hwnd_about = ButtonBuilder::new(parent, self.ids.btn_about)
                .text_w(ICON_ABOUT)
                .pos(0, 0).size(30, 25).dark_mode(is_dark)
                .font(icon_font)
                .build();

            self.hwnd_shortcuts = ButtonBuilder::new(parent, self.ids.btn_shortcuts)
                .text_w(ICON_KEYBOARD)
                .pos(0, 0).size(30, 25).dark_mode(is_dark)
                .font(icon_font)
                .build();

            self.hwnd_console = ButtonBuilder::new(parent, self.ids.btn_console)
                .text_w(ICON_CONSOLE)
                .pos(0, 0).size(30, 25).dark_mode(is_dark)
                .font(icon_font)
                .build();

            self.hwnd_watcher = ButtonBuilder::new(parent, self.ids.btn_watcher)
                .text_w(ICON_WATCHER)
                .pos(0, 0).size(30, 25).dark_mode(is_dark)
                .font(icon_font)
                .build();

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
                std::ptr::null_mut(),
                width - padding - btn_width,
                padding,
                btn_width,
                header_height,
                SWP_NOZORDER,
            );

            // Position About button (Left of Settings)
            SetWindowPos(
                self.hwnd_about,
                std::ptr::null_mut(),
                width - padding - btn_width - 35,
                padding,
                btn_width,
                header_height,
                SWP_NOZORDER,
            );

            // Position Shortcuts button (Left of About)
            SetWindowPos(
                self.hwnd_shortcuts,
                std::ptr::null_mut(),
                width - padding - btn_width - 70,
                padding,
                btn_width,
                header_height,
                SWP_NOZORDER,
            );

            // Position Console button (Left of Shortcuts)
            SetWindowPos(
                self.hwnd_console,
                std::ptr::null_mut(),
                width - padding - btn_width - 105,
                padding,
                btn_width,
                header_height,
                SWP_NOZORDER,
            );

            // Position Watcher button (Left of Console)
            SetWindowPos(
                self.hwnd_watcher,
                std::ptr::null_mut(),
                width - padding - btn_width - 140,
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
            apply_button_theme(self.hwnd_shortcuts, is_dark);
            apply_button_theme(self.hwnd_console, is_dark);
            apply_button_theme(self.hwnd_watcher, is_dark);
        }
    }
}
