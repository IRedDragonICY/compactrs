#![allow(unsafe_op_in_unsafe_fn)]
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{HBRUSH, COLOR_WINDOW};

use windows::Win32::UI::WindowsAndMessaging::{
    WM_CTLCOLOREDIT, WM_COMMAND,
    CreateWindowExW, DefWindowProcW, DestroyWindow, 
    LoadCursorW, RegisterClassW, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, SW_SHOW, WM_DESTROY, WNDCLASSW,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE, WM_CREATE, WM_SIZE, SetWindowPos, SWP_NOZORDER,
    WS_CHILD, WS_VSCROLL, ES_MULTILINE, ES_READONLY, ES_AUTOVSCROLL,
    SendMessageW, GetWindowTextLengthW, GetWindowTextW, SetWindowTextW,
};
use windows::Win32::UI::Controls::{EM_SETSEL, EM_REPLACESEL, SetWindowTheme};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::DataExchange::{OpenClipboard, CloseClipboard, EmptyClipboard, SetClipboardData};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use crate::ui::controls::apply_button_theme;
use crate::ui::builder::ButtonBuilder;

const CONSOLE_CLASS_NAME: PCWSTR = w!("CompactRS_Console");
const CONSOLE_TITLE: PCWSTR = w!("Debug Console");
const IDC_EDIT_CONSOLE: i32 = 1001;
const IDC_BTN_COPY: i32 = 1002;
const IDC_BTN_CLEAR: i32 = 1003;
const BUTTON_HEIGHT: i32 = 30;

static mut CONSOLE_HWND: Option<HWND> = None;
static mut EDIT_HWND: Option<HWND> = None;
static mut BTN_COPY_HWND: Option<HWND> = None;
static mut BTN_CLEAR_HWND: Option<HWND> = None;
static mut IS_DARK_MODE: bool = false;

pub unsafe fn show_console_window(_parent: HWND, initial_logs: &[String], is_dark: bool) {
    IS_DARK_MODE = is_dark;
    
    if let Some(hwnd) = CONSOLE_HWND {
        // Update theme if window exists
        update_console_theme(hwnd, is_dark);
        // If already exists, just show and focus
        ShowWindow(hwnd, SW_SHOW);
        return;
    }


    let instance = GetModuleHandleW(None).unwrap();

    // Load App Icon using centralized helper
    let icon = crate::ui::utils::load_app_icon(instance.into());

    let wc = WNDCLASSW {
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
        hInstance: instance.into(),
        hIcon: icon,
        lpszClassName: CONSOLE_CLASS_NAME,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize as *mut _),
        ..Default::default()
    };

    let _ = RegisterClassW(&wc);

    let hwnd = CreateWindowExW(
        Default::default(),
        CONSOLE_CLASS_NAME,
        CONSOLE_TITLE,
        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        600,
        400,
        None, 
        None,
        Some(instance.into()),
        None
    ).unwrap();

    CONSOLE_HWND = Some(hwnd);
    
    // Initial theme application
    update_console_theme(hwnd, is_dark);

    
    // Populate initial logs
    for log in initial_logs {
         append_log(EDIT_HWND.unwrap_or_default(), log);
    }
}

pub unsafe fn append_log_msg(msg: &str) {
    if let Some(edit) = EDIT_HWND {
        append_log(edit, msg);
    }
}

unsafe fn append_log(edit: HWND, msg: &str) {
    if edit.0.is_null() { return; }

    // Move caret to end
    let len = GetWindowTextLengthW(edit);
    SendMessageW(edit, EM_SETSEL, Some(WPARAM(len as usize)), Some(LPARAM(len as isize)));
    
    // Append text
    let mut text: Vec<u16> = msg.encode_utf16().collect();
    text.push(13); // CR
    text.push(10); // LF
    text.push(0);  // Null
    
    SendMessageW(edit, EM_REPLACESEL, Some(WPARAM(0)), Some(LPARAM(text.as_ptr() as isize)));
}

pub unsafe fn close_console() {
    if let Some(hwnd) = CONSOLE_HWND {
        DestroyWindow(hwnd);
        CONSOLE_HWND = None;
        EDIT_HWND = None;
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Centralized handler for theme-related messages
    if let Some(result) = crate::ui::theme::handle_standard_colors(hwnd, msg, wparam, IS_DARK_MODE) {
        return result;
    }

    match msg {
        WM_CREATE => {
             let instance = GetModuleHandleW(None).unwrap();
             // Create Edit Control
             let edit = CreateWindowExW(
                 Default::default(),
                 w!("EDIT"),
                 None,
                 windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                     WS_CHILD.0 | WS_VISIBLE.0 | WS_VSCROLL.0 | 
                     ES_MULTILINE as u32 | ES_READONLY as u32 | ES_AUTOVSCROLL as u32
                 ),
                 0, 0, 0, 0,
                 Some(hwnd),
                 Some(windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_EDIT_CONSOLE as isize as *mut _)),
                 Some(instance.into()),
                 None
             ).unwrap();
             
             EDIT_HWND = Some(edit);
             
             // Create Copy and Clear buttons using ButtonBuilder
             let btn_copy = ButtonBuilder::new(hwnd, IDC_BTN_COPY as u16)
                 .text("Copy").pos(0, 0).size(80, BUTTON_HEIGHT).dark_mode(IS_DARK_MODE).build();
             BTN_COPY_HWND = Some(btn_copy);
             
             let btn_clear = ButtonBuilder::new(hwnd, IDC_BTN_CLEAR as u16)
                 .text("Clear").pos(90, 0).size(80, BUTTON_HEIGHT).dark_mode(IS_DARK_MODE).build();
             BTN_CLEAR_HWND = Some(btn_clear);
             
             // Apply dark theme to edit control if needed
             if IS_DARK_MODE {
                 let _ = SetWindowTheme(edit, w!("DarkMode_Explorer"), None);
             }
             
             LRESULT(0)
        },
        WM_SIZE => {
            let width = (lparam.0 & 0xFFFF) as i32;
            let height = ((lparam.0 >> 16) & 0xFFFF) as i32;
            
            // Position edit control (leave space for buttons at bottom)
            if let Some(edit) = EDIT_HWND {
                SetWindowPos(edit, None, 0, 0, width, height - BUTTON_HEIGHT - 5, SWP_NOZORDER);
            }
            
            // Position buttons at bottom
            let btn_y = height - BUTTON_HEIGHT;
            if let Some(btn) = BTN_COPY_HWND {
                SetWindowPos(btn, None, 5, btn_y, 80, BUTTON_HEIGHT - 5, SWP_NOZORDER);
            }
            if let Some(btn) = BTN_CLEAR_HWND {
                SetWindowPos(btn, None, 90, btn_y, 80, BUTTON_HEIGHT - 5, SWP_NOZORDER);
            }
            
            LRESULT(0)
        },
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            match id {
                IDC_BTN_COPY => {
                    // Copy edit content to clipboard
                    if let Some(edit) = EDIT_HWND {
                        let len = GetWindowTextLengthW(edit);
                        if len > 0 {
                            let mut buffer: Vec<u16> = vec![0; (len + 1) as usize];
                            GetWindowTextW(edit, &mut buffer);
                            
                            // Copy to clipboard
                            if OpenClipboard(Some(hwnd)).is_ok() {
                                let _ = EmptyClipboard();
                                let size = (buffer.len() * 2) as usize;
                                if let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, size) {
                                    let ptr = GlobalLock(hmem);
                                    if !ptr.is_null() {
                                        std::ptr::copy_nonoverlapping(buffer.as_ptr(), ptr as *mut u16, buffer.len());
                                        GlobalUnlock(hmem);
                                        // CF_UNICODETEXT = 13
                                        let _ = SetClipboardData(13, Some(windows::Win32::Foundation::HANDLE(hmem.0)));
                                    }
                                }
                                let _ = CloseClipboard();
                            }
                        }
                    }
                },
                IDC_BTN_CLEAR => {
                    // Clear edit content
                    if let Some(edit) = EDIT_HWND {
                        SetWindowTextW(edit, w!(""));
                    }
                },
                _ => {}
            }
            LRESULT(0)
        },
        WM_DESTROY => {
            CONSOLE_HWND = None;
            EDIT_HWND = None;
            BTN_COPY_HWND = None;
            BTN_CLEAR_HWND = None;
            LRESULT(0)
        },
        WM_CTLCOLOREDIT => {
            // Special handling for edit control colors (not covered by handle_standard_colors)
            if let Some(result) = crate::ui::theme::handle_ctl_color(hwnd, wparam, IS_DARK_MODE) {
                return result;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        },
        // WM_APP + 2: Theme change broadcast from Settings
        0x8002 => {
            let new_is_dark = wparam.0 == 1;
            IS_DARK_MODE = new_is_dark;
            
            // Update edit control theme for scrollbar
            if let Some(edit) = EDIT_HWND {
                if new_is_dark {
                    let _ = SetWindowTheme(edit, w!("DarkMode_Explorer"), None);
                } else {
                    let _ = SetWindowTheme(edit, w!("Explorer"), None);
                }
            }
            
            // Update button themes using shared function
            if let Some(btn) = BTN_COPY_HWND {
                apply_button_theme(btn, new_is_dark);
            }
            if let Some(btn) = BTN_CLEAR_HWND {
                apply_button_theme(btn, new_is_dark);
            }
            
            // Update DWM title bar
            update_console_theme(hwnd, new_is_dark);
            
            // Force repaint entire window
            windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, true);
            LRESULT(0)
        },
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn update_console_theme(hwnd: HWND, is_dark: bool) {
    // Delegate to centralized helper
    crate::ui::theme::set_window_frame_theme(hwnd, is_dark);
    
    // Force redraw to apply GDI colors
    if let Some(edit) = EDIT_HWND {
        windows::Win32::Graphics::Gdi::InvalidateRect(Some(edit), None, true);
    }
}
