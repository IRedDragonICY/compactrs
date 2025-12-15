#![allow(unsafe_op_in_unsafe_fn)]
use windows::core::{w, PCWSTR, Result};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, HINSTANCE, COLORREF};
use windows::Win32::Graphics::Gdi::{HBRUSH, COLOR_WINDOW, SetTextColor, SetBkColor, CreateSolidBrush, HDC};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::UI::WindowsAndMessaging::{
    WM_CTLCOLORSTATIC, WM_CTLCOLOREDIT,
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, 
    LoadCursorW, PostQuitMessage, RegisterClassW, ShowWindow, TranslateMessage,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, MSG, SW_SHOW, WM_DESTROY, WNDCLASSW,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE, WM_CREATE, WM_SIZE, SetWindowPos, SWP_NOMOVE, SWP_NOACTIVATE, SWP_NOZORDER,
    WS_CHILD, WS_VSCROLL, ES_MULTILINE, ES_READONLY, ES_AUTOVSCROLL, WM_SETFONT,
    SendMessageW, GetWindowTextLengthW,
};
use windows::Win32::UI::Controls::{EM_SETSEL, EM_REPLACESEL}; // Correct import location
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

const CONSOLE_CLASS_NAME: PCWSTR = w!("CompactRS_Console");
const CONSOLE_TITLE: PCWSTR = w!("Debug Console");
const IDC_EDIT_CONSOLE: i32 = 1001;

static mut CONSOLE_HWND: Option<HWND> = None;
static mut EDIT_HWND: Option<HWND> = None;
static mut IS_DARK_MODE: bool = false;
static mut DARK_BRUSH: Option<HBRUSH> = None;

pub unsafe fn show_console_window(parent: HWND, initial_logs: &[String], is_dark: bool) {
    IS_DARK_MODE = is_dark;
    
    if let Some(hwnd) = CONSOLE_HWND {
        // Update theme if window exists
        update_console_theme(hwnd, is_dark);
        // If already exists, just show and focus
        ShowWindow(hwnd, SW_SHOW);
        return;
    }


    let instance = GetModuleHandleW(None).unwrap();

    let wc = WNDCLASSW {
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
        hInstance: instance.into(),
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
             LRESULT(0)
        },
        WM_SIZE => {
            let width = (lparam.0 & 0xFFFF) as i32;
            let height = ((lparam.0 >> 16) & 0xFFFF) as i32;
            if let Some(edit) = EDIT_HWND {
                SetWindowPos(edit, None, 0, 0, width, height, SWP_NOZORDER);
            }
            LRESULT(0)
        },
        WM_DESTROY => {
            CONSOLE_HWND = None;
            EDIT_HWND = None;
            if let Some(brush) = DARK_BRUSH {
                windows::Win32::Graphics::Gdi::DeleteObject(windows::Win32::Graphics::Gdi::HGDIOBJ(brush.0));
                DARK_BRUSH = None;
            }
            LRESULT(0)
        },
        WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC => {
            if IS_DARK_MODE {
                let hdc = HDC(wparam.0 as *mut _);
                SetTextColor(hdc, COLORREF(0x00FFFFFF)); // White text
                SetBkColor(hdc, COLORREF(0x001E1E1E));   // Dark background
                
                let brush = if let Some(b) = DARK_BRUSH {
                    b
                } else {
                    let new_brush = CreateSolidBrush(COLORREF(0x001E1E1E));
                    DARK_BRUSH = Some(new_brush);
                    new_brush
                };
                return LRESULT(brush.0 as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        },
        // WM_APP + 2: Theme change broadcast from Settings
        0x8002 => {
            let new_is_dark = wparam.0 == 1;
            IS_DARK_MODE = new_is_dark;
            
            // Delete old brush if switching themes
            if let Some(brush) = DARK_BRUSH {
                windows::Win32::Graphics::Gdi::DeleteObject(windows::Win32::Graphics::Gdi::HGDIOBJ(brush.0));
                DARK_BRUSH = None;
            }
            
            // Update DWM title bar
            update_console_theme(hwnd, new_is_dark);
            
            // Force repaint
            if let Some(edit) = EDIT_HWND {
                windows::Win32::Graphics::Gdi::InvalidateRect(Some(edit), None, true);
            }
            LRESULT(0)
        },
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn update_console_theme(hwnd: HWND, is_dark: bool) {
    let dark_mode = if is_dark { 1 } else { 0 };
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
        &dark_mode as *const i32 as *const _,
        4
    );
    
    // Force redraw to apply GDI colors
    if let Some(edit) = EDIT_HWND {
        windows::Win32::Graphics::Gdi::InvalidateRect(Some(edit), None, true);
    }
}
