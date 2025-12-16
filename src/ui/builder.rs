//! Builder pattern for Win32 control creation.
//!
//! Provides fluent APIs for creating Windows controls, reducing boilerplate
//! and centralizing theme application logic.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, WS_CHILD, WS_VISIBLE, BS_PUSHBUTTON, HMENU,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use crate::ui::utils::ToWide;
use crate::ui::controls::apply_button_theme;

/// Fluent builder for creating Win32 Button controls.
///
/// # Example
/// ```ignore
/// let btn = ButtonBuilder::new(hwnd, IDC_BTN_OK)
///     .text("OK")
///     .pos(10, 10)
///     .size(100, 30)
///     .dark_mode(is_dark)
///     .build();
/// ```
pub struct ButtonBuilder {
    parent: HWND,
    id: u16,
    text: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    is_dark: bool,
}

impl ButtonBuilder {
    /// Creates a new `ButtonBuilder` with required parent window and control ID.
    ///
    /// Default values:
    /// - Position: (0, 0)
    /// - Size: (100, 30)
    /// - Text: empty
    /// - Dark mode: false
    pub fn new(parent: HWND, id: u16) -> Self {
        Self {
            parent,
            id,
            text: String::new(),
            x: 0,
            y: 0,
            w: 100,
            h: 30,
            is_dark: false,
        }
    }

    /// Sets the button text.
    pub fn text(mut self, text: &str) -> Self {
        self.text = text.to_string();
        self
    }

    /// Sets the button position (x, y).
    pub fn pos(mut self, x: i32, y: i32) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    /// Sets the button size (width, height).
    pub fn size(mut self, w: i32, h: i32) -> Self {
        self.w = w;
        self.h = h;
        self
    }

    /// Sets whether to apply dark mode theme.
    pub fn dark_mode(mut self, is_dark: bool) -> Self {
        self.is_dark = is_dark;
        self
    }

    /// Builds and creates the button control.
    ///
    /// This method:
    /// 1. Converts the text to UTF-16
    /// 2. Calls `CreateWindowExW` to create the button
    /// 3. Applies the appropriate theme based on `is_dark`
    /// 4. Returns the button's `HWND`
    pub fn build(self) -> HWND {
        unsafe {
            let hmenu = HMENU(self.id as isize as *mut _);
            let instance = GetModuleHandleW(None).unwrap_or_default();

            let text_wide = self.text.to_wide();

            let hwnd = CreateWindowExW(
                Default::default(),
                w!("BUTTON"),
                PCWSTR::from_raw(text_wide.as_ptr()),
                windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                    WS_CHILD.0 | WS_VISIBLE.0 | BS_PUSHBUTTON as u32
                ),
                self.x,
                self.y,
                self.w,
                self.h,
                Some(self.parent),
                Some(hmenu),
                Some(instance.into()),
                None,
            )
            .unwrap_or_default();

            // Apply theme immediately after creation
            apply_button_theme(hwnd, self.is_dark);

            hwnd
        }
    }
}
