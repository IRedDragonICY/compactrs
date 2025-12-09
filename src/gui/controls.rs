use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    HMENU,
    CreateWindowExW, SendMessageW, 
    WS_CHILD, WS_VISIBLE, WS_BORDER, WS_TABSTOP, WS_VSCROLL,
    BS_PUSHBUTTON, CBS_DROPDOWNLIST, CBS_HASSTRINGS,
};
use windows::Win32::UI::Controls::{
    LVM_SETEXTENDEDLISTVIEWSTYLE, LVS_EX_FULLROWSELECT, LVS_EX_DOUBLEBUFFER, LVS_REPORT, LVS_SHOWSELALWAYS,
};
use windows::core::{w, PCWSTR};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

pub const IDC_LISTVIEW: u16 = 101;
pub const IDC_BTN_SCAN: u16 = 102;
pub const IDC_BTN_COMPRESS: u16 = 103;
pub const IDC_STATUSBAR: u16 = 104;
pub const IDC_COMBO_ALGO: u16 = 105;
pub const IDC_BTN_DECOMPRESS: u16 = 106;
pub const IDC_STATIC_TEXT: u16 = 107;
pub const IDC_PROGRESS_BAR: u16 = 108;
pub const IDC_BTN_CANCEL: u16 = 109;

use windows::Win32::UI::Controls::{PBS_SMOOTH, PBM_SETRANGE32, PBM_SETPOS, PROGRESS_CLASSW};

pub unsafe fn create_progress_bar(parent: HWND, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
    unsafe {
        let instance = GetModuleHandleW(None).unwrap();
        CreateWindowExW(
            Default::default(),
            PROGRESS_CLASSW,
            w!(""),
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | PBS_SMOOTH as u32),
            x, y, w, h,
            parent,
            HMENU(id as isize as *mut _),
            instance,
            None
        ).unwrap_or_default()
    }
}

pub unsafe fn create_button(parent: HWND, text: PCWSTR, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
    unsafe {
        let instance = GetModuleHandleW(None).unwrap();
        let hwnd = CreateWindowExW(
            Default::default(),
            w!("BUTTON"),
            text,
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | WS_TABSTOP.0 | BS_PUSHBUTTON as u32),
            x, y, w, h,
            parent,
            HMENU(id as isize as *mut _),
            instance,
            None
        ).unwrap_or_default();
        // Set modern font here if we had a font handle, for now defaults.
        hwnd
    }
}

pub unsafe fn create_listview(parent: HWND, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
    unsafe {
        let instance = GetModuleHandleW(None).unwrap();
        let hwnd = CreateWindowExW(
            Default::default(),
            w!("SysListView32"),
            None,
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | WS_BORDER.0 | LVS_REPORT as u32 | LVS_SHOWSELALWAYS as u32),
            x, y, w, h,
            parent,
            HMENU(id as isize as *mut _),
            instance,
            None
        ).unwrap_or_default();

        // Set extended style for full row select & double buffering (flicker usage)
        SendMessageW(
            hwnd, 
            LVM_SETEXTENDEDLISTVIEWSTYLE, 
            windows::Win32::Foundation::WPARAM(0), 
            windows::Win32::Foundation::LPARAM((LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize)
        );

        hwnd
    }
}

pub unsafe fn create_combobox(parent: HWND, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
    unsafe {
        let instance = GetModuleHandleW(None).unwrap();
        let hwnd = CreateWindowExW(
            Default::default(),
            w!("COMBOBOX"),
            None,
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | WS_TABSTOP.0 | WS_VSCROLL.0 | CBS_DROPDOWNLIST as u32 | CBS_HASSTRINGS as u32),
            x, y, w, h,
            parent,
            HMENU(id as isize as *mut _),
            instance,
            None
        ).unwrap_or_default();
        hwnd
    }
}
