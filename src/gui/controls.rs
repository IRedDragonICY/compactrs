use windows::Win32::Foundation::{HWND, HINSTANCE};
use windows::Win32::UI::WindowsAndMessaging::{
    HMENU,
    CreateWindowExW, SendMessageW, 
    WS_CHILD, WS_VISIBLE, WS_BORDER, WS_TABSTOP, WS_VSCROLL,
    BS_PUSHBUTTON, BS_AUTOCHECKBOX, CBS_DROPDOWNLIST, CBS_HASSTRINGS,
};
use windows::Win32::UI::Controls::{
    LVM_SETEXTENDEDLISTVIEWSTYLE, LVS_EX_FULLROWSELECT, LVS_EX_DOUBLEBUFFER, LVS_REPORT, LVS_SHOWSELALWAYS,
    SetWindowTheme,
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

// New control IDs for batch UI
pub const IDC_BATCH_LIST: u16 = 110;
pub const IDC_BTN_ADD_FOLDER: u16 = 111;
pub const IDC_BTN_REMOVE: u16 = 112;
pub const IDC_BTN_PROCESS_ALL: u16 = 113;
pub const IDC_BTN_ADD_FILES: u16 = 114;
pub const IDC_BTN_SETTINGS: u16 = 115;
pub const IDC_BTN_ABOUT: u16 = 116;
pub const IDC_BTN_OK: u16 = 117;
pub const IDC_BTN_CONSOLE: u16 = 118;
pub const IDC_CHK_FORCE: u16 = 119;

#[allow(unused_imports)]
use windows::Win32::UI::Controls::{PBS_SMOOTH, PBM_SETRANGE32, PBM_SETPOS, PROGRESS_CLASSW};

pub unsafe fn create_progress_bar(parent: HWND, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
    unsafe {
        let module = GetModuleHandleW(None).unwrap();
        let instance = HINSTANCE(module.0);
        CreateWindowExW(
            Default::default(),
            PROGRESS_CLASSW,
            w!(""),
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | PBS_SMOOTH as u32),
            x, y, w, h,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            Some(instance),
            None
        ).unwrap_or_default()
    }
}

/// Configuration struct for button creation (Factory Pattern)
pub struct ButtonOpts<'a> {
    pub text: PCWSTR,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub id: u16,
    pub is_dark: bool,
    #[allow(dead_code)]
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> ButtonOpts<'a> {
    pub fn new(text: PCWSTR, x: i32, y: i32, w: i32, h: i32, id: u16, is_dark: bool) -> Self {
        Self { text, x, y, w, h, id, is_dark, _marker: std::marker::PhantomData }
    }
}

/// Creates a button with theme applied internally.
/// This is the unified factory function - theme is applied inside, not by caller.
pub unsafe fn create_button(parent: HWND, opts: ButtonOpts) -> HWND {
    let module = GetModuleHandleW(None).unwrap();
    let instance = HINSTANCE(module.0);
    let hwnd = CreateWindowExW(
        Default::default(),
        w!("BUTTON"),
        opts.text,
        windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | BS_PUSHBUTTON as u32),
        opts.x, opts.y, opts.w, opts.h,
        Some(parent),
        Some(HMENU(opts.id as isize as *mut _)),
        Some(instance),
        None
    ).unwrap_or_default();
    
    // Apply button theme immediately (DarkMode_Explorer for dark, Explorer for light)
    apply_button_theme(hwnd, opts.is_dark);
    
    hwnd
}

/// Apply button theme dynamically (for theme changes after creation)
pub unsafe fn apply_button_theme(hwnd: HWND, is_dark: bool) { unsafe {
    if is_dark {
        let _ = SetWindowTheme(hwnd, w!("DarkMode_Explorer"), None);
    } else {
        let _ = SetWindowTheme(hwnd, w!("Explorer"), None);
    }
}}

/// Apply ComboBox theme dynamically
pub unsafe fn apply_combobox_theme(hwnd: HWND, is_dark: bool) { unsafe {
    if is_dark {
        let _ = SetWindowTheme(hwnd, w!("DarkMode_CFD"), None);
    } else {
        let _ = SetWindowTheme(hwnd, w!("Explorer"), None);
    }
}}

pub unsafe fn create_listview(parent: HWND, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
    unsafe {
        let module = GetModuleHandleW(None).unwrap();
        let instance = HINSTANCE(module.0);
        let hwnd = CreateWindowExW(
            Default::default(),
            w!("SysListView32"),
            None,
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | WS_BORDER.0 | LVS_REPORT as u32 | LVS_SHOWSELALWAYS as u32),
            x, y, w, h,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            Some(instance),
            None
        ).unwrap_or_default();

        // Set extended style for full row select & double buffering (flicker usage)
        SendMessageW(
            hwnd, 
            LVM_SETEXTENDEDLISTVIEWSTYLE, 
            Some(windows::Win32::Foundation::WPARAM(0)), 
            Some(windows::Win32::Foundation::LPARAM((LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize))
        );

        hwnd
    }
}

pub unsafe fn create_combobox(parent: HWND, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
    unsafe {
        let module = GetModuleHandleW(None).unwrap();
        let instance = HINSTANCE(module.0);
        let hwnd = CreateWindowExW(
            Default::default(),
            w!("COMBOBOX"),
            None,
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | WS_TABSTOP.0 | WS_VSCROLL.0 | CBS_DROPDOWNLIST as u32 | CBS_HASSTRINGS as u32),
            x, y, w, h,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            Some(instance),
            None
        ).unwrap_or_default();
        hwnd
    }
}

pub unsafe fn create_checkbox(parent: HWND, text: PCWSTR, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
    unsafe {
        let module = GetModuleHandleW(None).unwrap();
        let instance = HINSTANCE(module.0);
        let hwnd = CreateWindowExW(
            Default::default(),
            w!("BUTTON"),
            text,
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | WS_TABSTOP.0 | BS_AUTOCHECKBOX as u32),
            x, y, w, h,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            Some(instance),
            None
        ).unwrap_or_default();
        hwnd
    }
}
