use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, RegisterClassW,
    CS_HREDRAW, CS_VREDRAW, IDC_ARROW, WM_DESTROY, WNDCLASSW,
    WS_VISIBLE, WM_CREATE, WM_COMMAND, SetWindowLongPtrW, GWLP_USERDATA,
    WS_CHILD, WS_CAPTION, WS_SYSMENU, WS_POPUP,
    GetMessageW, TranslateMessage, DispatchMessageW, MSG,
    PostQuitMessage, WM_CLOSE, WM_TIMER, SetTimer, KillTimer,
    DestroyWindow, SetWindowTextW, GetDlgItem,
    WM_CTLCOLORSTATIC, WM_CTLCOLORBTN, WM_ERASEBKGND, GetClientRect,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::Graphics::Gdi::{HBRUSH, COLOR_WINDOW, FillRect, HDC};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};

use crate::ui::builder::ButtonBuilder;
use crate::ui::theme;
use crate::ui::utils::{ToWide, get_window_state};

const DIALOG_CLASS_NAME: PCWSTR = w!("CompactRS_ForceStopDialog");
const TIMER_ID: usize = 1;
const IDC_BTN_YES: u16 = 3001;
const IDC_BTN_NO: u16 = 3002;
const IDC_LBL_MSG: u16 = 3003;

struct DialogState {
    process_name: String,
    seconds_left: u32,
    result: bool,
    is_dark: bool,
}

pub unsafe fn show_force_stop_dialog(parent: HWND, process_name: &str, is_dark: bool) -> bool {
    let instance = unsafe { GetModuleHandleW(None).unwrap_or_default() };
    
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(dialog_wnd_proc),
        hInstance: instance.into(),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or_default() },
        lpszClassName: DIALOG_CLASS_NAME,
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize as *mut _),
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc) };

    let mut rect = windows::Win32::Foundation::RECT::default();
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowRect(parent, &mut rect).unwrap_or_default() };
    let p_width = rect.right - rect.left;
    let p_height = rect.bottom - rect.top;
    
    // Increased width to 380 to fit text comfortably
    let width = 380; 
    let height = 180;
    let x = rect.left + (p_width - width) / 2;
    let y = rect.top + (p_height - height) / 2;

    let mut state = DialogState {
        process_name: process_name.to_string(),
        seconds_left: 5,
        result: false, // Default cancel
        is_dark,
    };

    let _hwnd = unsafe { CreateWindowExW(
        Default::default(),
        DIALOG_CLASS_NAME,
        w!("File Locked"),
        WS_POPUP | WS_VISIBLE | WS_CAPTION | WS_SYSMENU,
        x, y, width, height,
        Some(parent),
        None,
        Some(instance.into()),
        Some(&mut state as *mut _ as *mut _),
    ).unwrap_or_default() };

    unsafe { EnableWindow(parent, false) };
    
    // Message Loop
    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0).as_bool() } {
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    
    unsafe { EnableWindow(parent, true) };
    
    state.result
}

unsafe extern "system" fn dialog_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT { unsafe {
    // Use centralized helper for state access
    let get_state = || get_window_state::<DialogState>(hwnd);

    match msg {
        WM_CREATE => {
            let createstruct = &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
            let state_ptr = createstruct.lpCreateParams as *mut DialogState;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
            
            let instance = GetModuleHandleW(None).unwrap_or_default();
            
            if let Some(st) = unsafe { state_ptr.as_ref() } {
                let is_dark = st.is_dark;
                
                // Apply Dark Mode to Title Bar
                let dark_mode_val: u32 = if is_dark { 1 } else { 0 };
                let _ = DwmSetWindowAttribute(
                     hwnd, 
                     DWMWA_USE_IMMERSIVE_DARK_MODE, 
                     &dark_mode_val as *const u32 as *const _, 
                     4
                );
            
                // Message Label
                let msg_text = format!("Process '{}' is locking this file.\nForce Stop and try again?", st.process_name);
                let msg_wide = msg_text.to_wide();
                
                let _h_msg = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(msg_wide.as_ptr()),
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0),
                    20, 20, 340, 60, // Widened label
                    Some(hwnd),
                    Some(windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_LBL_MSG as isize as *mut _)),
                    Some(instance.into()),
                    None
                ).unwrap_or_default();
                
                // Yes Button
                ButtonBuilder::new(hwnd, IDC_BTN_YES)
                    .text("Force Stop (Yes)").pos(40, 90).size(130, 32).dark_mode(is_dark).build();
                
                // No Button with Timer
                let no_text = format!("Cancel ({})", st.seconds_left);
                ButtonBuilder::new(hwnd, IDC_BTN_NO)
                    .text(&no_text).pos(190, 90).size(130, 32).dark_mode(is_dark).build();
                
                // Start Timer
                SetTimer(Some(hwnd), TIMER_ID, 1000, None);
            }
            LRESULT(0)
        },
        
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            if let Some(st) = get_state() {
                if let Some(result) = theme::handle_ctl_color(hwnd, wparam, st.is_dark) {
                    return result;
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        },

        WM_ERASEBKGND => {
            if let Some(st) = get_state() {
                let is_dark = st.is_dark;
                let (brush, _, _) = theme::get_theme_colors(is_dark);
                
                let hdc = HDC(wparam.0 as *mut _);
                let mut rc = windows::Win32::Foundation::RECT::default();
                GetClientRect(hwnd, &mut rc);
                FillRect(hdc, &rc, brush);
                return LRESULT(1);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        },
        
        WM_TIMER => {
            if wparam.0 == TIMER_ID {
                if let Some(st) = get_state() {
                    if st.seconds_left > 0 {
                        st.seconds_left -= 1;
                        
                        // Update No button text
                        let no_text = format!("Cancel ({})", st.seconds_left);
                        let no_wide = no_text.to_wide();
                        if let Ok(h_btn) = GetDlgItem(Some(hwnd), IDC_BTN_NO.into()) {
                             let _ = SetWindowTextW(h_btn, PCWSTR(no_wide.as_ptr()));
                        }

                        if st.seconds_left == 0 {
                            st.result = false; // Auto-cancel
                            DestroyWindow(hwnd);
                        }
                    }
                }
            }
            LRESULT(0)
        },
        
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as u16;
            match id {
                IDC_BTN_YES => {
                    if let Some(st) = get_state() {
                        st.result = true;
                    }
                    DestroyWindow(hwnd);
                },
                IDC_BTN_NO => {
                    if let Some(st) = get_state() {
                        st.result = false;
                    }
                    DestroyWindow(hwnd);
                },
                _ => {}
            }
            LRESULT(0)
        },
        
        WM_CLOSE => {
             if let Some(st) = get_state() {
                st.result = false;
            }
            DestroyWindow(hwnd);
            LRESULT(0)
        }

        WM_DESTROY => {
            KillTimer(Some(hwnd), TIMER_ID);
            PostQuitMessage(0);
            LRESULT(0)
        },
        
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}}
