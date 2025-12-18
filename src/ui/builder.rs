//! Builder pattern for Win32 control creation.
//!
//! Provides a generic, fluent `ControlBuilder` API for creating Windows controls,
//! reducing boilerplate and centralizing theme application logic.

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, SendMessageW, WS_CHILD, WS_VISIBLE, WS_TABSTOP, WS_VSCROLL,
    BS_PUSHBUTTON, BS_AUTOCHECKBOX, BS_AUTORADIOBUTTON, BS_GROUPBOX,
    CBS_DROPDOWNLIST, CBS_HASSTRINGS, HMENU, WM_SETFONT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use crate::utils::to_wstring;
use crate::ui::theme::{self, ControlType};

// STATIC control style constants (feature-gated in windows-sys)
const SS_LEFT: u32 = 0x0000;
const SS_CENTER: u32 = 0x0001;
const SS_ICON: u32 = 0x0003;
const SS_REALSIZEIMAGE: u32 = 0x0800;
const SS_CENTERIMAGE: u32 = 0x0200;

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
    custom_font: Option<HFONT>,
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
            custom_font: None,
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

    /// Sets a custom font for this control (overrides default app font).
    pub fn font(mut self, font: HFONT) -> Self {
        self.custom_font = Some(font);
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

    /// Configures as a static label.
    /// Sets class to "STATIC" and adds appropriate alignment style.
    ///
    /// # Arguments
    /// * `align_center` - If true, text is centered; otherwise left-aligned.
    pub fn label(mut self, align_center: bool) -> Self {
        self.class_name = "STATIC".to_string();
        self.style |= if align_center { SS_CENTER } else { SS_LEFT };
        self
    }

    /// Configures as a group box.
    /// Sets class to "BUTTON" and adds `BS_GROUPBOX`.
    pub fn groupbox(mut self) -> Self {
        self.class_name = "BUTTON".to_string();
        self.style |= BS_GROUPBOX as u32;
        self
    }

    /// Configures as an icon display (STATIC with SS_ICON).
    /// Sets class to "STATIC" and adds `SS_ICON | SS_REALSIZEIMAGE | SS_CENTERIMAGE`.
    pub fn icon_display(mut self) -> Self {
        self.class_name = "STATIC".to_string();
        self.style |= SS_ICON | SS_REALSIZEIMAGE | SS_CENTERIMAGE;
        self
    }

    /// Configures as a radio button.
    /// Sets class to "BUTTON" and adds `BS_AUTORADIOBUTTON | WS_TABSTOP`.
    pub fn radio(mut self) -> Self {
        self.class_name = "BUTTON".to_string();
        self.style |= (BS_AUTORADIOBUTTON as u32) | WS_TABSTOP;
        self
    }

    /// Builds and creates the control.
    ///
    /// Automatically applies theming based on the control type and class name,
    /// and sets the appropriate font.
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

            // Determine ControlType based on class_name and style for smart theming
            let control_type = self.detect_control_type();
            theme::apply_theme(hwnd, control_type, self.is_dark);

            // Apply font: custom font if specified, otherwise default app font
            let font = self.custom_font.unwrap_or_else(theme::get_app_font);
            SendMessageW(hwnd, WM_SETFONT, font as usize, 1);

            hwnd
        }
    }

    /// Detects the appropriate ControlType based on class name and style flags.
    fn detect_control_type(&self) -> ControlType {
        if self.class_name == "STATIC" {
            // STATIC controls use GroupBox theming (neutral background)
            ControlType::GroupBox
        } else if self.class_name == "BUTTON" {
            // Check style flags to determine button subtype
            if (self.style & (BS_GROUPBOX as u32)) != 0 {
                ControlType::GroupBox
            } else if (self.style & (BS_AUTOCHECKBOX as u32)) != 0 {
                ControlType::CheckBox
            } else if (self.style & (BS_AUTORADIOBUTTON as u32)) != 0 {
                ControlType::RadioButton
            } else {
                ControlType::Button
            }
        } else if self.class_name == "COMBOBOX" {
            ControlType::ComboBox
        } else {
            // Default fallback
            ControlType::Button
        }
    }
}

// Backwards compatibility alias
pub type ButtonBuilder = ControlBuilder;
