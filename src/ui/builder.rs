//! Builder pattern for Win32 control creation.
//!
//! Provides a generic, fluent `ControlBuilder` API for creating Windows controls,
//! reducing boilerplate and centralizing theme application logic.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, WS_CHILD, WS_VISIBLE, WS_TABSTOP, WS_VSCROLL,
    BS_PUSHBUTTON, BS_AUTOCHECKBOX,
    CBS_DROPDOWNLIST, CBS_HASSTRINGS,
    HMENU, WINDOW_STYLE, WINDOW_EX_STYLE,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use crate::ui::utils::ToWide;
use crate::ui::controls::{apply_button_theme, apply_combobox_theme};

/// Fluent builder for creating various Win32 controls.
///
/// # Example
/// ```ignore
/// // Create a button
/// let btn = ControlBuilder::new(hwnd, IDC_BTN_OK)
///     .button()
///     .text("OK")
///     .pos(10, 10)
///     .size(100, 30)
///     .dark_mode(is_dark)
///     .build();
///
/// // Create a checkbox
/// let chk = ControlBuilder::new(hwnd, IDC_CHK_FORCE)
///     .checkbox()
///     .text("Force")
///     .pos(10, 50)
///     .size(60, 25)
///     .dark_mode(is_dark)
///     .build();
///
/// // Create a combobox
/// let combo = ControlBuilder::new(hwnd, IDC_COMBO_ALGO)
///     .combobox()
///     .pos(10, 90)
///     .size(120, 200) // Height is dropdown height
///     .dark_mode(is_dark)
///     .build();
/// ```
pub struct ControlBuilder {
    parent: HWND,
    id: u16,
    text: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    class_name: PCWSTR,
    style: u32,
    ex_style: u32,
    is_dark: bool,
}

impl ControlBuilder {
    /// Creates a new `ControlBuilder` with required parent window and control ID.
    ///
    /// Default values:
    /// - Position: (0, 0)
    /// - Size: (100, 25)
    /// - Text: empty
    /// - Class: "BUTTON"
    /// - Style: WS_VISIBLE | WS_CHILD
    /// - Dark mode: false
    pub fn new(parent: HWND, id: u16) -> Self {
        Self {
            parent,
            id,
            text: String::new(),
            x: 0,
            y: 0,
            w: 100,
            h: 25,
            class_name: w!("BUTTON"),
            style: WS_VISIBLE.0 | WS_CHILD.0,
            ex_style: 0,
            is_dark: false,
        }
    }

    /// Sets the control text.
    pub fn text(mut self, text: &str) -> Self {
        self.text = text.to_string();
        self
    }

    /// Sets the control position (x, y).
    pub fn pos(mut self, x: i32, y: i32) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    /// Sets the control size (width, height).
    pub fn size(mut self, w: i32, h: i32) -> Self {
        self.w = w;
        self.h = h;
        self
    }

    /// Adds additional styles using bitwise OR.
    pub fn style(mut self, style: u32) -> Self {
        self.style |= style;
        self
    }

    /// Sets whether to apply dark mode theme.
    pub fn dark_mode(mut self, is_dark: bool) -> Self {
        self.is_dark = is_dark;
        self
    }

    // --- Preset Methods ---

    /// Configures as a push button (default button type).
    /// Sets class to "BUTTON" and adds `BS_PUSHBUTTON`.
    pub fn button(mut self) -> Self {
        self.class_name = w!("BUTTON");
        self.style |= BS_PUSHBUTTON as u32;
        self
    }

    /// Configures as an auto checkbox.
    /// Sets class to "BUTTON" and adds `BS_AUTOCHECKBOX | WS_TABSTOP`.
    pub fn checkbox(mut self) -> Self {
        self.class_name = w!("BUTTON");
        self.style |= BS_AUTOCHECKBOX as u32 | WS_TABSTOP.0;
        self
    }

    /// Configures as a dropdown combobox.
    /// Sets class to "COMBOBOX" and adds `CBS_DROPDOWNLIST | CBS_HASSTRINGS | WS_TABSTOP | WS_VSCROLL`.
    pub fn combobox(mut self) -> Self {
        self.class_name = w!("COMBOBOX");
        self.style |= CBS_DROPDOWNLIST as u32 | CBS_HASSTRINGS as u32 | WS_TABSTOP.0 | WS_VSCROLL.0;
        self
    }

    /// Builds and creates the control.
    ///
    /// This method:
    /// 1. Converts the text to UTF-16
    /// 2. Calls `CreateWindowExW` to create the control
    /// 3. Applies the appropriate theme based on `is_dark` and control class
    /// 4. Returns the control's `HWND`
    pub fn build(self) -> HWND {
        unsafe {
            let instance = GetModuleHandleW(None).unwrap_or_default();
            let text_wide = self.text.to_wide();

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(self.ex_style),
                self.class_name,
                PCWSTR::from_raw(text_wide.as_ptr()),
                WINDOW_STYLE(self.style),
                self.x,
                self.y,
                self.w,
                self.h,
                Some(self.parent),
                Some(HMENU(self.id as isize as *mut _)),
                Some(instance.into()),
                None,
            )
            .unwrap_or_default();

            // Auto-apply theme based on class name
            // Note: Checkbox uses button theme (same "BUTTON" class)
            if self.class_name == w!("BUTTON") {
                apply_button_theme(hwnd, self.is_dark);
            } else if self.class_name == w!("COMBOBOX") {
                apply_combobox_theme(hwnd, self.is_dark);
            }

            hwnd
        }
    }
}

// Backwards compatibility alias
pub type ButtonBuilder = ControlBuilder;
