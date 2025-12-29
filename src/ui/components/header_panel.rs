#![allow(unsafe_op_in_unsafe_fn)]

//! HeaderPanel component - manages the header area with top-right buttons.
//!
//! This component contains the Settings, About, and Console buttons
//! positioned in the top-right corner of the main window.

use crate::types::*;

use super::base::Component;
use crate::ui::builder::ControlBuilder;
use crate::ui::controls::apply_button_theme;
use crate::ui::layout::{LayoutNode, SizePolicy};

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
    hwnd_panel: HWND,
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
            hwnd_panel: std::ptr::null_mut(),
            hwnd_settings: std::ptr::null_mut(),
            hwnd_about: std::ptr::null_mut(),
            hwnd_shortcuts: std::ptr::null_mut(),
            hwnd_console: std::ptr::null_mut(),
            hwnd_watcher: std::ptr::null_mut(),
            ids,
        }
    }

    /// Returns the main container HWND.
    #[inline]
    pub fn hwnd(&self) -> HWND {
        self.hwnd_panel
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
            // Use centralized Panel creation
            self.hwnd_panel = crate::ui::components::panel::Panel::create(
                parent,
                "CompactRsHeaderPanel",
                0, 0, 100, 30
            )?;

            let is_dark = crate::ui::theme::is_system_dark_mode();
            let icon_font = crate::ui::theme::get_icon_font();

            // Create buttons as children of panel
            let parent_hwnd = self.hwnd_panel;

            // Initial positions (will be updated in on_resize)
            self.hwnd_settings = ControlBuilder::new(parent_hwnd, self.ids.btn_settings)
                .text_w(ICON_SETTINGS)
                .pos(0, 0).size(30, 25).dark_mode(is_dark)
                .font(icon_font)
                .build();

            self.hwnd_about = ControlBuilder::new(parent_hwnd, self.ids.btn_about)
                .text_w(ICON_ABOUT)
                .pos(0, 0).size(30, 25).dark_mode(is_dark)
                .font(icon_font)
                .build();

            self.hwnd_shortcuts = ControlBuilder::new(parent_hwnd, self.ids.btn_shortcuts)
                .text_w(ICON_KEYBOARD)
                .pos(0, 0).size(30, 25).dark_mode(is_dark)
                .font(icon_font)
                .build();

            self.hwnd_console = ControlBuilder::new(parent_hwnd, self.ids.btn_console)
                .text_w(ICON_CONSOLE)
                .pos(0, 0).size(30, 25).dark_mode(is_dark)
                .font(icon_font)
                .build();

            self.hwnd_watcher = ControlBuilder::new(parent_hwnd, self.ids.btn_watcher)
                .text_w(ICON_WATCHER)
                .pos(0, 0).size(30, 25).dark_mode(is_dark)
                .font(icon_font)
                .build();

            Ok(())
        }
    }

    fn hwnd(&self) -> Option<HWND> {
        Some(self.hwnd_panel)
    }

    unsafe fn on_resize(&mut self, parent_rect: &RECT) {
        let w = parent_rect.right - parent_rect.left;
        let h = parent_rect.bottom - parent_rect.top;
        
        // Resize container
        SetWindowPos(self.hwnd_panel, std::ptr::null_mut(), parent_rect.left, parent_rect.top, w, h, SWP_NOZORDER);
        self.refresh_layout();
    }

    unsafe fn on_theme_change(&mut self, is_dark: bool) {
        unsafe {
            apply_button_theme(self.hwnd_settings, is_dark);
            apply_button_theme(self.hwnd_about, is_dark);
            apply_button_theme(self.hwnd_shortcuts, is_dark);
            apply_button_theme(self.hwnd_console, is_dark);
            apply_button_theme(self.hwnd_watcher, is_dark);
            
            // Store theme prop for WndProc
            crate::ui::components::panel::Panel::update_theme(self.hwnd_panel, is_dark);
        }
    }
}

// Window Procedure for HeaderPanel


impl HeaderPanel {
    pub unsafe fn refresh_layout(&self) {
        use SizePolicy::Fixed;
        
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(self.hwnd_panel, &mut rect);
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        
        // Layout buttons inside container relative to (0,0)
        let layout_rect = RECT {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };

        // Align right
        LayoutNode::row(0, 5)
            .flex_spacer()
            .with(self.hwnd_watcher, Fixed(30))
            .with(self.hwnd_console, Fixed(30))
            .with(self.hwnd_shortcuts, Fixed(30))
            .with(self.hwnd_about, Fixed(30))
            .with(self.hwnd_settings, Fixed(30))
            .apply_layout(layout_rect);
    }
}
