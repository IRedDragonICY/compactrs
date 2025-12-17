//! Builder pattern for Win32 control creation.
//!
//! Provides a generic, fluent `ControlBuilder` API for creating Windows controls,
//! reducing boilerplate and centralizing theme application logic.

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, WS_CHILD, WS_VISIBLE, WS_TABSTOP, WS_VSCROLL,
    BS_PUSHBUTTON, BS_AUTOCHECKBOX,
    CBS_DROPDOWNLIST, CBS_HASSTRINGS, HMENU,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use crate::utils::to_wstring;
use crate::ui::controls::{apply_button_theme, apply_combobox_theme};

/// Fluent builder for creating various Win32 controls.
pub struct ControlBuilder {
    parent: HWND,
    id: u16,
    text: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    class_name: String,
    style: u32,
    ex_style: u32,
    is_dark: bool,
}

impl ControlBuilder {
    /// Creates a new `ControlBuilder` with required parent window and control ID.
    pub fn new(parent: HWND, id: u16) -> Self {
        Self {
            parent,
            id,
            text: String::new(),
            x: 0,
            y: 0,
            w: 100,
            h: 25,
            class_name: "BUTTON".to_string(), // Default
            style: WS_VISIBLE | WS_CHILD,
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
        self.class_name = "BUTTON".to_string();
        self.style |= BS_PUSHBUTTON as u32;
        self
    }

    /// Configures as an auto checkbox.
    /// Sets class to "BUTTON" and adds `BS_AUTOCHECKBOX | WS_TABSTOP`.
    pub fn checkbox(mut self) -> Self {
        self.class_name = "BUTTON".to_string();
        self.style |= (BS_AUTOCHECKBOX as u32) | WS_TABSTOP;
        self
    }

    /// Configures as a dropdown combobox.
    /// Sets class to "COMBOBOX" and adds `CBS_DROPDOWNLIST | CBS_HASSTRINGS | WS_TABSTOP | WS_VSCROLL`.
    pub fn combobox(mut self) -> Self {
        self.class_name = "COMBOBOX".to_string();
        self.style |= (CBS_DROPDOWNLIST as u32) | (CBS_HASSTRINGS as u32) | WS_TABSTOP | WS_VSCROLL;
        self
    }

    /// Builds and creates the control.
    pub fn build(self) -> HWND {
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let text_wide = to_wstring(&self.text);
            let class_wide = to_wstring(&self.class_name);

            let hwnd = CreateWindowExW(
                self.ex_style,
                class_wide.as_ptr(),
                text_wide.as_ptr(),
                self.style,
                self.x,
                self.y,
                self.w,
                self.h,
                self.parent,
                self.id as isize as HMENU,
                instance,
                std::ptr::null(),
            );

            // Auto-apply theme based on class name
            if self.class_name == "BUTTON" {
                apply_button_theme(hwnd, self.is_dark);
            } else if self.class_name == "COMBOBOX" {
                apply_combobox_theme(hwnd, self.is_dark);
            }

            hwnd
        }
    }
}

// Backwards compatibility alias
pub type ButtonBuilder = ControlBuilder;
