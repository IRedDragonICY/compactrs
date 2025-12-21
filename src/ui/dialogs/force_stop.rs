use crate::ui::builder::ButtonBuilder;
use crate::ui::theme;
use crate::ui::framework::get_window_state;
use crate::utils::to_wstring;
use crate::w;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, RegisterClassW,
    CS_HREDRAW, CS_VREDRAW, IDC_ARROW, WM_DESTROY, WNDCLASSW,
    WS_VISIBLE, WM_CREATE, WM_COMMAND, SetWindowLongPtrW, GWLP_USERDATA,
    WS_CHILD, WS_CAPTION, WS_SYSMENU, WS_POPUP,
    PostQuitMessage, WM_CLOSE, WM_TIMER, SetTimer, KillTimer,
    DestroyWindow, SetWindowTextW, GetDlgItem,
    GetWindowRect, HMENU, CREATESTRUCTW,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::Graphics::Gdi::{HBRUSH, COLOR_WINDOW};


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

pub unsafe fn show_force_stop_dialog(parent: HWND, process_name: &str, is_dark: bool) -> bool { unsafe {
    let instance = GetModuleHandleW(std::ptr::null());
    let cls = w!("CompactRS_ForceStopDialog");
    let title = w!("File Locked");
    
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(dialog_wnd_proc),
        hInstance: instance,
        hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
        lpszClassName: cls.as_ptr(),
        hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
        cbClsExtra: 0,
        cbWndExtra: 0,
        hIcon: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
    };
    RegisterClassW(&wc);

    let mut rect: RECT = std::mem::zeroed();
    GetWindowRect(parent, &mut rect);
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

    let _hwnd = CreateWindowExW(
        0,
        cls.as_ptr(),
        title.as_ptr(),
        WS_POPUP | WS_VISIBLE | WS_CAPTION | WS_SYSMENU,
        x, y, width, height,
        parent,
        std::ptr::null_mut(),
        instance,
        &mut state as *mut _ as *mut _,
    );

    EnableWindow(parent, 0);
    
    // Message Loop
    // Message Loop
    crate::ui::framework::run_message_loop(_hwnd);
    
    EnableWindow(parent, 1);
    
    state.result
}}

unsafe extern "system" fn dialog_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT { unsafe {
    // Use centralized helper for state access
    let get_state = || get_window_state::<DialogState>(hwnd);

    // Centralized handler for theme-related messages
    if let Some(st) = get_state() {
        if let Some(result) = theme::handle_standard_colors(hwnd, msg, wparam, st.is_dark) {
            return result;
        }
    }

    match msg {
        WM_CREATE => {
            let createstruct = &*(lparam as *const CREATESTRUCTW);
            let state_ptr = createstruct.lpCreateParams as *mut DialogState;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
            
            let instance = GetModuleHandleW(std::ptr::null());
            
            if let Some(st) = state_ptr.as_ref() {
                let is_dark = st.is_dark;
                
                // Apply Dark Mode to Title Bar using centralized helper
                crate::ui::theme::set_window_frame_theme(hwnd, is_dark);
            
                // Message Label
                let msg_text = "Process '".to_string() + &st.process_name + "' is locking this file.\nForce Stop and try again?";
                let msg_wide = to_wstring(&msg_text);
                let static_cls = w!("STATIC");
                
                let _h_msg = CreateWindowExW(
                    0,
                    static_cls.as_ptr(),
                    msg_wide.as_ptr(),
                    WS_VISIBLE | WS_CHILD,
                    20, 20, 340, 60, // Widened label
                    hwnd,
                    IDC_LBL_MSG as isize as HMENU,
                    instance,
                    std::ptr::null()
                );
                
                // Yes Button
                ButtonBuilder::new(hwnd, IDC_BTN_YES)
                    .text_w(w!("Force Stop (Yes)")).pos(40, 90).size(130, 32).dark_mode(is_dark).build();
                
                // No Button with Timer
                let no_text = "Cancel (".to_string() + &st.seconds_left.to_string() + ")";
                ButtonBuilder::new(hwnd, IDC_BTN_NO)
                    .text(&no_text).pos(190, 90).size(130, 32).dark_mode(is_dark).build();
                
                // Start Timer
                SetTimer(hwnd, TIMER_ID, 1000, None);
            }
            0
        },
        
        WM_TIMER => {
            if wparam == TIMER_ID {
                if let Some(st) = get_state() {
                    if st.seconds_left > 0 {
                        st.seconds_left -= 1;
                        
                        // Update No button text
                        let no_text = "Cancel (".to_string() + &st.seconds_left.to_string() + ")";
                        let no_wide = to_wstring(&no_text);
                        let h_btn = GetDlgItem(hwnd, IDC_BTN_NO as i32);
                        if h_btn != std::ptr::null_mut() {
                             SetWindowTextW(h_btn, no_wide.as_ptr());
                        }

                        if st.seconds_left == 0 {
                            st.result = false; // Auto-cancel
                            DestroyWindow(hwnd);
                        }
                    }
                }
            }
            0
        },
        
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as u16;
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
            0
        },
        
        WM_CLOSE => {
             if let Some(st) = get_state() {
                st.result = false;
            }
            DestroyWindow(hwnd);
            0
        }

        WM_DESTROY => {
            KillTimer(hwnd, TIMER_ID);
            PostQuitMessage(0);
            0
        },
        
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}}
