/* --- src/ui/builder.rs --- */
use crate::types::*;
use crate::utils::to_wstring;
use crate::ui::theme::{self, ControlType};
use crate::w;
use std::borrow::Cow;

// STATIC styles
const SS_LEFT: u32 = 0x0000;
const SS_CENTER: u32 = 0x0001;
const SS_ICON: u32 = 0x0003;
const SS_REALSIZEIMAGE: u32 = 0x0800;
const SS_CENTERIMAGE: u32 = 0x0200;

pub struct ControlBuilder<'a> {
    parent: HWND,
    id: u16,
    text: Cow<'a, [u16]>,
    x: i32, y: i32, w: i32, h: i32,
    class_name: Cow<'a, [u16]>,
    style: u32,
    ex_style: u32,
    is_dark: bool,
    custom_font: Option<HFONT>,
    checked: bool, // New: support default checked state
}

impl<'a> ControlBuilder<'a> {
    pub fn new(parent: HWND, id: u16) -> Self {
        Self {
            parent, id,
            text: Cow::Borrowed(w!("")),
            x: 0, y: 0, w: 100, h: 25,
            class_name: Cow::Borrowed(w!("BUTTON")),
            style: WS_VISIBLE | WS_CHILD,
            ex_style: 0,
            is_dark: false,
            custom_font: None,
            checked: false,
        }
    }

    pub fn text(mut self, text: &str) -> Self { self.text = Cow::Owned(to_wstring(text)); self }
    pub fn text_w(mut self, text: &'a [u16]) -> Self { self.text = Cow::Borrowed(text); self }
    pub fn pos(mut self, x: i32, y: i32) -> Self { self.x = x; self.y = y; self }
    pub fn size(mut self, w: i32, h: i32) -> Self { self.w = w; self.h = h; self }
    pub fn style(mut self, style: u32) -> Self { self.style |= style; self }
    pub fn dark_mode(mut self, is_dark: bool) -> Self { self.is_dark = is_dark; self }
    pub fn font(mut self, font: HFONT) -> Self { self.custom_font = Some(font); self }
    pub fn checked(mut self, checked: bool) -> Self { self.checked = checked; self }

    // --- Presets ---
    pub fn button(mut self) -> Self {
        self.class_name = Cow::Borrowed(w!("BUTTON"));
        self.style |= (BS_PUSHBUTTON as u32) | WS_TABSTOP;
        self
    }

    pub fn checkbox(mut self) -> Self {
        self.class_name = Cow::Borrowed(w!("BUTTON"));
        self.style |= (BS_AUTOCHECKBOX as u32) | WS_TABSTOP;
        self
    }

    pub fn combobox(mut self) -> Self {
        self.class_name = Cow::Borrowed(w!("COMBOBOX"));
        self.style |= (CBS_DROPDOWNLIST as u32) | (CBS_HASSTRINGS as u32) | WS_TABSTOP | WS_VSCROLL;
        self
    }

    pub fn label(mut self, align_center: bool) -> Self {
        self.class_name = Cow::Borrowed(w!("STATIC"));
        self.style |= if align_center { SS_CENTER } else { SS_LEFT };
        self
    }

    pub fn groupbox(mut self) -> Self {
        self.class_name = Cow::Borrowed(w!("BUTTON"));
        self.style |= BS_GROUPBOX as u32;
        self
    }

    pub fn icon_display(mut self) -> Self {
        self.class_name = Cow::Borrowed(w!("STATIC"));
        self.style |= SS_ICON | SS_REALSIZEIMAGE | SS_CENTERIMAGE;
        self
    }

    pub fn radio(mut self) -> Self {
        self.class_name = Cow::Borrowed(w!("BUTTON"));
        self.style |= (BS_AUTORADIOBUTTON as u32) | WS_TABSTOP;
        self
    }

    pub fn trackbar(mut self) -> Self {
        self.class_name = Cow::Borrowed(w!("msctls_trackbar32"));
        // TBS_AUTOTICKS | WS_TABSTOP
        self.style |= 0x0001 | WS_TABSTOP;
        self
    }

    pub fn edit(mut self) -> Self {
        self.class_name = Cow::Borrowed(w!("EDIT"));
        // WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL (0x0080)
        self.style |= 0x00800000 | WS_TABSTOP | 0x0080; 
        self
    }

    pub fn listview(mut self) -> Self {
        self.class_name = Cow::Borrowed(w!("SysListView32"));
        // WS_BORDER | WS_TABSTOP | LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS
        // LVS_REPORT = 0x0001
        // LVS_SINGLESEL = 0x0004
        // LVS_SHOWSELALWAYS = 0x0008
        self.style |= 0x00800000 | WS_TABSTOP | 0x0001 | 0x0004 | 0x0008;
        self
    }

    pub fn build(self) -> HWND {
        unsafe {
            // GetModuleHandleW takes LPCWSTR (*const u16), so null() is correct
            let instance = GetModuleHandleW(std::ptr::null());
            // No conversion needed, self.text and self.class_name are already [u16]
            let text_ptr = self.text.as_ptr();
            let class_ptr = self.class_name.as_ptr();

            let wants_visible = (self.style & WS_VISIBLE) != 0;
            let style_initial = self.style & !WS_VISIBLE;

            let hwnd = CreateWindowExW(
                self.ex_style, class_ptr, text_ptr, style_initial,
                self.x, self.y, self.w, self.h,
                self.parent, self.id as isize as HMENU, instance, std::ptr::null_mut(),
            );

            let ctl_type = self.detect_control_type();
            theme::apply_theme(hwnd, ctl_type, self.is_dark);

            let font = self.custom_font.unwrap_or_else(theme::get_app_font);
            SendMessageW(hwnd, WM_SETFONT, font as usize, 1);

            if self.checked {
                 const BM_SETCHECK: u32 = 0x00F1;
                 SendMessageW(hwnd, BM_SETCHECK, 1, 0);
            }

            if wants_visible {
                ShowWindow(hwnd, SW_SHOW);
            }

            if self.detect_control_type() == ControlType::Edit {
                crate::ui::edit_subclass::subclass_edit(hwnd);
            }

            hwnd
        }
    }

    fn detect_control_type(&self) -> ControlType {
        if self.class_name == w!("STATIC") { ControlType::GroupBox } 
        else if self.class_name == w!("BUTTON") {
            if (self.style & (BS_GROUPBOX as u32)) != 0 { ControlType::GroupBox }
            else if (self.style & (BS_AUTOCHECKBOX as u32)) != 0 { ControlType::CheckBox }
            else if (self.style & (BS_AUTORADIOBUTTON as u32)) != 0 { ControlType::RadioButton }
            else { ControlType::Button }
        } else if self.class_name == w!("COMBOBOX") { ControlType::ComboBox }
        else if self.class_name == w!("msctls_trackbar32") { ControlType::Trackbar }
        else if self.class_name == w!("EDIT") { ControlType::Edit }
        else if self.class_name == w!("SysListView32") { ControlType::List }
        else { ControlType::Button }
    }
}
