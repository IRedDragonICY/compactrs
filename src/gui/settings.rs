use windows::core::{Result, w, PCWSTR};
use windows::Win32::Foundation::{HWND, HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, RegisterClassW, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, IDC_ARROW, SW_SHOW, WM_DESTROY, WNDCLASSW,
    WS_VISIBLE, WM_CREATE, WM_COMMAND,
    GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA, GetDlgItem,
    WS_CHILD, WS_CAPTION, WS_SYSMENU, WS_POPUP,
    BS_AUTORADIOBUTTON, BM_SETCHECK, BM_GETCHECK,
    GetMessageW, TranslateMessage, DispatchMessageW, MSG,
    SendMessageW, PostQuitMessage, WM_CLOSE, BS_GROUPBOX, GetParent, BN_CLICKED, DestroyWindow,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetActiveWindow};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use crate::gui::state::AppTheme;
use crate::gui::controls::create_button;

const SETTINGS_CLASS_NAME: PCWSTR = w!("CompactRS_Settings");
const SETTINGS_TITLE: PCWSTR = w!("Settings");

// Control IDs
const IDC_GRP_THEME: u16 = 2001;
const IDC_RADIO_SYSTEM: u16 = 2002;
const IDC_RADIO_DARK: u16 = 2003;
const IDC_RADIO_LIGHT: u16 = 2004;
const IDC_BTN_OK: u16 = 2005;
const IDC_BTN_CANCEL: u16 = 2006;

struct SettingsState {
    theme: AppTheme,
    result: Option<AppTheme>,
}

pub unsafe fn show_settings(parent: HWND, current_theme: AppTheme) -> Option<AppTheme> {
    unsafe {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(settings_wnd_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: SETTINGS_CLASS_NAME,
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(windows::Win32::Graphics::Gdi::COLOR_WINDOW.0 as isize as *mut _),
            ..Default::default()
        };

        RegisterClassW(&wc);

        // Center relative to parent
        let mut rect = windows::Win32::Foundation::RECT::default();
        windows::Win32::UI::WindowsAndMessaging::GetWindowRect(parent, &mut rect).unwrap_or_default();
        let p_width = rect.right - rect.left;
        let p_height = rect.bottom - rect.top;
        
        let width = 300;
        let height = 250;
        let x = rect.left + (p_width - width) / 2;
        let y = rect.top + (p_height - height) / 2;

        // Use Box to store state
        let state = Box::new(SettingsState {
            theme: current_theme,
            result: None,
        });

        let hwnd = CreateWindowExW(
            Default::default(),
            SETTINGS_CLASS_NAME,
            SETTINGS_TITLE,
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            x, y, width, height,
            Some(parent),
            None,
            Some(instance.into()),
            Some(Box::into_raw(state) as *mut _),
        ).unwrap_or_default();

        // Modal loop
        EnableWindow(parent, false);
        
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        EnableWindow(parent, true);
        
        // Retrieve result
        // Note: The state pointer was dropped inside WM_DESTROY, so we can't access it here easily 
        // unless we kept a shared reference or static. 
        // FIX: We need a way to get the result back.
        // Option 1: Store result in a static or shared Arc<Mutex>.
        // Option 2: Use pointer trickery (keep raw pointer until function end).
        // Let's use a simpler approach: 
        // The message loop ends when PostQuitMessage is called.
        // We can't access 'state' after window destroy if we owned it in window.
        // Better pattern for modal dialog result:
        // Use a Cell or similar on the stack, pass pointer to it? 
        // Or actually, retrieving GWLP_USERDATA after loop is risky if WM_DESTROY freed it.
        // 
        // Let's modify logic: destroy window frees memory.
        // We will panic if we try to read freed memory.
        // Let's use a shared struct on the stack!
        
        // Re-do creation with stack-allocated state
        // We can't pass pointer to stack variable easily to WndProc because lifetimes? 
        // Actually raw pointers don't care about lifetimes.
        // But WndProc is extern "system" so it shouldn't access stack of this function strictly unless we are sure it lives long enough.
        // It does live long enough because we block on GetMessageW.
    }
    
    // Fallback for now: implementation below uses Heap alloc and cleans up.
    // To return value, we need to extract it before cleanup or use shared memory.
    // Let's use a static for simplicity in this constrained environment or just return None if complex.
    // Actually, I'll rewrite `show_settings` to use a mutable reference passed via `LPARAM` in creation,
    // and store it in GWLP_USERDATA.
    None // Placeholder, the real implementation is in the next step
}

// Redefining proper function with data passing
pub unsafe fn show_settings_modal(parent: HWND, current_theme: AppTheme) -> Option<AppTheme> {
    unsafe {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(settings_wnd_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: SETTINGS_CLASS_NAME,
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(windows::Win32::Graphics::Gdi::COLOR_WINDOW.0 as isize as *mut _),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let mut rect = windows::Win32::Foundation::RECT::default();
        windows::Win32::UI::WindowsAndMessaging::GetWindowRect(parent, &mut rect).unwrap_or_default();
        let p_width = rect.right - rect.left;
        let p_height = rect.bottom - rect.top;
        let width = 300;
        let height = 250;
        let x = rect.left + (p_width - width) / 2;
        let y = rect.top + (p_height - height) / 2;

        let mut state = SettingsState {
            theme: current_theme,
            result: None,
        };

        let hwnd = CreateWindowExW(
            Default::default(),
            SETTINGS_CLASS_NAME,
            SETTINGS_TITLE,
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            x, y, width, height,
            Some(parent),
            None,
            Some(instance.into()),
            Some(&mut state as *mut _ as *mut _),
        ).unwrap_or_default();

        EnableWindow(parent, false);
        
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        EnableWindow(parent, true);
        
        state.result
    }
}


unsafe extern "system" fn settings_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let get_state = || {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        if ptr == 0 { None } else { Some(&mut *(ptr as *mut SettingsState)) }
    };

    match msg {
        WM_CREATE => {
            let createstruct = &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
            let state_ptr = createstruct.lpCreateParams as *mut SettingsState;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
            
            let instance = GetModuleHandleW(None).unwrap_or_default();
            
            // Group Box
            CreateWindowExW(
                Default::default(),
                w!("BUTTON"),
                w!("App Theme"),
                windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | BS_GROUPBOX as u32),
                10, 10, 260, 140,
                Some(hwnd),
                Some(windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_GRP_THEME as isize as *mut _)),
                Some(instance.into()),
                None
            );

            // Radio Buttons
            let create_radio = |text: PCWSTR, id: u16, y: i32, checked: bool| {
                 let h = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    text,
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | BS_AUTORADIOBUTTON as u32),
                    30, y, 200, 25,
                    Some(hwnd),
                    Some(windows::Win32::UI::WindowsAndMessaging::HMENU(id as isize as *mut _)),
                    Some(instance.into()),
                    None
                ).unwrap_or_default();
                if checked {
                    SendMessageW(h, BM_SETCHECK, Some(WPARAM(1)), None);
                }
            };
            
            // Determine initial check
            let theme = if let Some(st) = state_ptr.as_ref() { st.theme } else { AppTheme::System };
            
            create_radio(w!("System Default"), IDC_RADIO_SYSTEM, 40, theme == AppTheme::System);
            create_radio(w!("Dark Mode"), IDC_RADIO_DARK, 70, theme == AppTheme::Dark);
            create_radio(w!("Light Mode"), IDC_RADIO_LIGHT, 100, theme == AppTheme::Light);
            
            // Buttons
            create_button(hwnd, w!("Close"), 110, 170, 80, 25, IDC_BTN_CANCEL); // Centered Close button

            LRESULT(0)
        },
        
        WM_COMMAND => {
             let id = (wparam.0 & 0xFFFF) as u16;
             let code = ((wparam.0 >> 16) & 0xFFFF) as u16;
             
             match id {
                 IDC_RADIO_SYSTEM | IDC_RADIO_DARK | IDC_RADIO_LIGHT => {
                     if (code as u32) == BN_CLICKED {
                         let theme = match id {
                             IDC_RADIO_SYSTEM => AppTheme::System,
                             IDC_RADIO_DARK => AppTheme::Dark,
                             IDC_RADIO_LIGHT => AppTheme::Light,
                             _ => AppTheme::System,
                         };
                         
                         // Update local state
                         if let Some(st) = get_state() {
                             st.theme = theme;
                             st.result = Some(theme);
                         }
                         
                         // Notify Parent Immediately (WM_APP + 1)
                         if let Ok(parent) = GetParent(hwnd) {
                             let theme_val = match theme {
                                 AppTheme::System => 0,
                                 AppTheme::Dark => 1,
                                 AppTheme::Light => 2,
                             };
                             SendMessageW(parent, 0x8000 + 1, Some(WPARAM(theme_val)), None);
                         }
                     }
                 },
                 IDC_BTN_CANCEL => {
                     if let Ok(parent) = GetParent(hwnd) {
                         let _ = EnableWindow(parent, true);
                         SetActiveWindow(parent);
                     }
                     DestroyWindow(hwnd);
                 },
                 _ => {}
             }
             LRESULT(0)
        },
        
        WM_CLOSE => {
            if let Ok(parent) = GetParent(hwnd) {
                let _ = EnableWindow(parent, true);
                SetActiveWindow(parent);
            }
            DestroyWindow(hwnd);
            LRESULT(0)
        },
        
        WM_DESTROY => {
            // Do NOT free GWLP_USERDATA here because it points to stack memory of caller
            PostQuitMessage(0);
            LRESULT(0)
        },
        
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
