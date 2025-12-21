/* --- src/ui/builder.rs --- */
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

// STATIC styles
const SS_LEFT: u32 = 0x0000;
const SS_CENTER: u32 = 0x0001;
const SS_ICON: u32 = 0x0003;
const SS_REALSIZEIMAGE: u32 = 0x0800;
const SS_CENTERIMAGE: u32 = 0x0200;

pub struct ControlBuilder {
    parent: HWND,
    id: u16,
    text: String,
    x: i32, y: i32, w: i32, h: i32,
    class_name: String,
    style: u32,
    ex_style: u32,
    is_dark: bool,
    custom_font: Option<HFONT>,
    checked: bool, // New: support default checked state
}

impl ControlBuilder {
    pub fn new(parent: HWND, id: u16) -> Self {
        Self {
            parent, id,
            text: String::new(),
            x: 0, y: 0, w: 100, h: 25,
            class_name: "BUTTON".to_string(),
            style: WS_VISIBLE | WS_CHILD,
            ex_style: 0,
            is_dark: false,
            custom_font: None,
            checked: false,
        }
    }

    pub fn text(mut self, text: &str) -> Self { self.text = text.to_string(); self }
    pub fn pos(mut self, x: i32, y: i32) -> Self { self.x = x; self.y = y; self }
    pub fn size(mut self, w: i32, h: i32) -> Self { self.w = w; self.h = h; self }
    pub fn style(mut self, style: u32) -> Self { self.style |= style; self }
    pub fn dark_mode(mut self, is_dark: bool) -> Self { self.is_dark = is_dark; self }
    pub fn font(mut self, font: HFONT) -> Self { self.custom_font = Some(font); self }
    pub fn checked(mut self, checked: bool) -> Self { self.checked = checked; self }

    // --- Presets ---
    pub fn button(mut self) -> Self {
        self.class_name = "BUTTON".to_string();
        self.style |= BS_PUSHBUTTON as u32;
        self
    }

    pub fn checkbox(mut self) -> Self {
        self.class_name = "BUTTON".to_string();
        self.style |= (BS_AUTOCHECKBOX as u32) | WS_TABSTOP;
        self
    }

    pub fn combobox(mut self) -> Self {
        self.class_name = "COMBOBOX".to_string();
        self.style |= (CBS_DROPDOWNLIST as u32) | (CBS_HASSTRINGS as u32) | WS_TABSTOP | WS_VSCROLL;
        self
    }

    pub fn label(mut self, align_center: bool) -> Self {
        self.class_name = "STATIC".to_string();
        self.style |= if align_center { SS_CENTER } else { SS_LEFT };
        self
    }

    pub fn groupbox(mut self) -> Self {
        self.class_name = "BUTTON".to_string();
        self.style |= BS_GROUPBOX as u32;
        self
    }

    pub fn icon_display(mut self) -> Self {
        self.class_name = "STATIC".to_string();
        self.style |= SS_ICON | SS_REALSIZEIMAGE | SS_CENTERIMAGE;
        self
    }

    pub fn radio(mut self) -> Self {
        self.class_name = "BUTTON".to_string();
        self.style |= (BS_AUTORADIOBUTTON as u32) | WS_TABSTOP;
        self
    }

    pub fn trackbar(mut self) -> Self {
        self.class_name = "msctls_trackbar32".to_string();
        // TBS_AUTOTICKS | WS_TABSTOP
        self.style |= 0x0001 | WS_TABSTOP;
        self
    }

    pub fn build(self) -> HWND {
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let text_wide = to_wstring(&self.text);
            let class_wide = to_wstring(&self.class_name);

            let wants_visible = (self.style & WS_VISIBLE) != 0;
            let style_initial = self.style & !WS_VISIBLE;

            let hwnd = CreateWindowExW(
                self.ex_style, class_wide.as_ptr(), text_wide.as_ptr(), style_initial,
                self.x, self.y, self.w, self.h,
                self.parent, self.id as isize as HMENU, instance, std::ptr::null(),
            );

            let ctl_type = self.detect_control_type();
            theme::apply_theme(hwnd, ctl_type, self.is_dark);

            let font = self.custom_font.unwrap_or_else(theme::get_app_font);
            SendMessageW(hwnd, WM_SETFONT, font as usize, 1);

            if self.checked {
                 use windows_sys::Win32::UI::WindowsAndMessaging::BM_SETCHECK;
                 SendMessageW(hwnd, BM_SETCHECK, 1, 0);
            }

            if wants_visible {
                use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOW};
                ShowWindow(hwnd, SW_SHOW);
            }

            hwnd
        }
    }

    fn detect_control_type(&self) -> ControlType {
        if self.class_name == "STATIC" { ControlType::GroupBox } 
        else if self.class_name == "BUTTON" {
            if (self.style & (BS_GROUPBOX as u32)) != 0 { ControlType::GroupBox }
            else if (self.style & (BS_AUTOCHECKBOX as u32)) != 0 { ControlType::CheckBox }
            else if (self.style & (BS_AUTORADIOBUTTON as u32)) != 0 { ControlType::RadioButton }
            else { ControlType::Button }
        } else if self.class_name == "COMBOBOX" { ControlType::ComboBox }
        else if self.class_name == "msctls_trackbar32" { ControlType::Trackbar }
        else { ControlType::Button }
    }
}

pub type ButtonBuilder = ControlBuilder;
