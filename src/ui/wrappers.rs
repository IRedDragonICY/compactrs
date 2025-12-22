#![allow(dead_code)]
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SendMessageW, SetWindowTextW, BM_GETCHECK, BM_SETCHECK,
    CB_GETCURSEL, CB_SETCURSEL, CB_ADDSTRING, CB_RESETCONTENT,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::UI::Controls::{
    PBM_SETRANGE32, PBM_SETPOS, TBM_SETPOS, TBM_SETRANGE,
};

pub const TBM_GETPOS: u32 = 0x0400;
use crate::utils::to_wstring;

/// Safe wrapper for Button controls (including Checkboxes and RadioButtons)
#[derive(Clone, Copy)]
pub struct Button { hwnd: HWND }
impl Button {
    pub fn new(hwnd: HWND) -> Self { Self { hwnd } }
    
    pub fn is_checked(&self) -> bool { 
        unsafe { SendMessageW(self.hwnd, BM_GETCHECK, 0, 0) == 1 } 
    }
    
    pub fn set_checked(&self, checked: bool) { 
        unsafe { SendMessageW(self.hwnd, BM_SETCHECK, if checked { 1 } else { 0 }, 0); } 
    }
    
    pub fn set_enabled(&self, enabled: bool) { 
        unsafe { EnableWindow(self.hwnd, if enabled { 1 } else { 0 }); } 
    }
    
    pub fn set_text(&self, text: &str) { 
        let w = to_wstring(text);
        unsafe { SetWindowTextW(self.hwnd, w.as_ptr()); }
    }

    pub fn set_text_w(&self, text: &[u16]) {
        unsafe { SetWindowTextW(self.hwnd, text.as_ptr()); }
    }
}

/// Safe wrapper for ComboBox controls
#[derive(Clone, Copy)]
pub struct ComboBox { hwnd: HWND }
impl ComboBox {
    pub fn new(hwnd: HWND) -> Self { Self { hwnd } }
    
    pub fn get_selected_index(&self) -> i32 { 
        unsafe { SendMessageW(self.hwnd, CB_GETCURSEL, 0, 0) as i32 } 
    }
    
    pub fn set_selected_index(&self, index: i32) { 
        unsafe { SendMessageW(self.hwnd, CB_SETCURSEL, index as usize, 0); } 
    }
    
    pub fn add_string(&self, text: &str) {
        let w = to_wstring(text);
        unsafe { SendMessageW(self.hwnd, CB_ADDSTRING, 0, w.as_ptr() as isize); }
    }
    
    pub fn clear(&self) { 
        unsafe { SendMessageW(self.hwnd, CB_RESETCONTENT, 0, 0); } 
    }
    
    pub fn set_enabled(&self, enabled: bool) { 
        unsafe { EnableWindow(self.hwnd, if enabled { 1 } else { 0 }); } 
    }
}

/// Safe wrapper for Static/Label controls
#[derive(Clone, Copy)]
pub struct Label { hwnd: HWND }
impl Label {
    pub fn new(hwnd: HWND) -> Self { Self { hwnd } }
    
    pub fn set_text(&self, text: &str) {
        let w = to_wstring(text);
        unsafe { SetWindowTextW(self.hwnd, w.as_ptr()); }
    }
}

/// Safe wrapper for ProgressBar controls
#[derive(Clone, Copy)]
pub struct ProgressBar { hwnd: HWND }
impl ProgressBar {
    pub fn new(hwnd: HWND) -> Self { Self { hwnd } }
    
    pub fn set_range(&self, min: i32, max: i32) { 
        unsafe { SendMessageW(self.hwnd, PBM_SETRANGE32, min as usize, max as isize); } 
    }
    
    pub fn set_pos(&self, pos: i32) { 
        unsafe { SendMessageW(self.hwnd, PBM_SETPOS, pos as usize, 0); } 
    }
}

/// Safe wrapper for Trackbar (Slider) controls
#[derive(Clone, Copy)]
pub struct Trackbar { hwnd: HWND }
impl Trackbar {
    pub fn new(hwnd: HWND) -> Self { Self { hwnd } }
    
    pub fn set_range(&self, min: u32, max: u32) {
        // TBM_SETRANGE: WPARAM=Redraw(TRUE), LPARAM=LOWORD(Min)|HIWORD(Max)
        let lparam = (min & 0xFFFF) | ((max << 16) & 0xFFFF0000);
        unsafe { SendMessageW(self.hwnd, TBM_SETRANGE, 1, lparam as isize); }
    }
    
    pub fn set_pos(&self, pos: u32) {
        unsafe { SendMessageW(self.hwnd, TBM_SETPOS, 1, pos as isize); }
    }
    
    pub fn get_pos(&self) -> u32 {
        unsafe { SendMessageW(self.hwnd, TBM_GETPOS, 0, 0) as u32 }
    }
}
// Helper to get text from a window
pub fn get_window_text(hwnd: HWND) -> String {
    unsafe {
        let len = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextLengthW(hwnd);
        if len > 0 {
            let mut buf = vec![0u16; (len + 1) as usize];
            let copied = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
            if copied > 0 {
                return String::from_utf16_lossy(&buf[..copied as usize]);
            }
        }
        String::new()
    }
}
