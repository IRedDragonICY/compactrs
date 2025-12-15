use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, RegisterClassW,
    CS_HREDRAW, CS_VREDRAW, IDC_ARROW, WM_DESTROY, WNDCLASSW,
    WS_VISIBLE, WM_CREATE, WM_COMMAND,
    GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA,
    WS_CHILD, WS_CAPTION, WS_SYSMENU, WS_POPUP,
    BS_AUTORADIOBUTTON, BM_SETCHECK,
    GetMessageW, TranslateMessage, DispatchMessageW, MSG,
    SendMessageW, PostQuitMessage, WM_CLOSE, BS_GROUPBOX, GetParent, BN_CLICKED, DestroyWindow,
    FindWindowW, LoadImageW, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED, HICON,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::SetWindowTheme;
use windows::Win32::Graphics::Gdi::{HBRUSH, COLOR_WINDOW, SetTextColor, CreateSolidBrush, HDC, DeleteObject, HGDIOBJ, InvalidateRect, FillRect, SetBkMode, TRANSPARENT};
use windows::Win32::Foundation::COLORREF;
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::UI::WindowsAndMessaging::{WM_CTLCOLORSTATIC, WM_CTLCOLORBTN, WM_ERASEBKGND, GetClientRect};
use crate::gui::state::AppTheme;
use crate::gui::controls::{create_button, ButtonOpts};

const SETTINGS_CLASS_NAME: PCWSTR = w!("CompactRS_Settings");
const SETTINGS_TITLE: PCWSTR = w!("Settings");

// Control IDs
const IDC_GRP_THEME: u16 = 2001;
const IDC_RADIO_SYSTEM: u16 = 2002;
const IDC_RADIO_DARK: u16 = 2003;
const IDC_RADIO_LIGHT: u16 = 2004;
const IDC_BTN_OK: u16 = 2005;
const IDC_BTN_CANCEL: u16 = 2006;
const IDC_CHK_FORCE_STOP: u16 = 2007;

struct SettingsState {
    theme: AppTheme,
    result: Option<AppTheme>,
    is_dark: bool,
    dark_brush: Option<HBRUSH>,
    enable_force_stop: bool, // Track checkbox state
}

impl Drop for SettingsState {
    fn drop(&mut self) {
        if let Some(brush) = self.dark_brush {
            unsafe {
                DeleteObject(HGDIOBJ(brush.0));
            }
        }
    }
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
            is_dark: false, // Default for this unused path
            dark_brush: None,
            enable_force_stop: false,
        });

        let _hwnd = CreateWindowExW(
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
pub unsafe fn show_settings_modal(parent: HWND, current_theme: AppTheme, is_dark: bool, enable_force_stop: bool) -> (Option<AppTheme>, bool) {
    unsafe {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        
        // Load App Icon (ID 1)
        let icon_handle = LoadImageW(
            Some(instance.into()),
            PCWSTR(1 as *const u16),
            IMAGE_ICON,
            0, 0,
            LR_DEFAULTSIZE | LR_SHARED
        ).unwrap_or_default();
        
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(settings_wnd_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hIcon: HICON(icon_handle.0),
            lpszClassName: SETTINGS_CLASS_NAME,
            hbrBackground: HBRUSH(if is_dark {
                // Use a dark brush initially if possible, but standard is COLOR_WINDOW
                // We'll rely on WM_CTLCOLORSTATIC to paint background
                // But for the main window background (if any exposed), we want dark.
                // 0x1E1E1E is 30,30,30. 
                // Creating a global brush just for class registration is tricky without leaks.
                // Let's stick to COLOR_WINDOW and rely on DWM/Painting.
                // Actually, for pure dark mode, we often want a dark background class brush.
                // But standard practice is handle WM_ERASEBKGND or WM_CTLCOLOR.
                COLOR_WINDOW.0 + 1
            } else {
                 COLOR_WINDOW.0 + 1
            } as isize as *mut _),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let mut rect = windows::Win32::Foundation::RECT::default();
        windows::Win32::UI::WindowsAndMessaging::GetWindowRect(parent, &mut rect).unwrap_or_default();
        let p_width = rect.right - rect.left;
        let p_height = rect.bottom - rect.top;
        let width = 300;
        let height = 300;
        let x = rect.left + (p_width - width) / 2;
        let y = rect.top + (p_height - height) / 2;

        let mut state = SettingsState {
            theme: current_theme,
            result: None,
            is_dark,
            dark_brush: None,
            enable_force_stop,
        };

        let _hwnd = CreateWindowExW(
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

        // Non-modal: DON'T disable parent window
        // EnableWindow(parent, false);
        
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        // Non-modal: DON'T re-enable parent window
        // EnableWindow(parent, true);
        
        // EnableWindow(parent, true);
        
        (state.result, state.enable_force_stop)
    }
}


unsafe extern "system" fn settings_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT { unsafe {
    let get_state = || {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        if ptr == 0 { None } else { Some(&mut *(ptr as *mut SettingsState)) }
    };

    match msg {
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            if let Some(st) = get_state() {
                if let Some(result) = crate::gui::theme::ThemeManager::handle_ctl_color(hwnd, wparam, st.is_dark) {
                    return result;
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        },
        WM_ERASEBKGND => {
            if let Some(st) = get_state() {
                let is_dark = st.is_dark;
                let (brush, _, _) = crate::gui::theme::ThemeManager::get_theme_colors(is_dark);
                
                let hdc = HDC(wparam.0 as *mut _);
                let mut rc = windows::Win32::Foundation::RECT::default();
                GetClientRect(hwnd, &mut rc);
                FillRect(hdc, &rc, brush);
                return LRESULT(1);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        },
        WM_CREATE => {
            let createstruct = &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
            let state_ptr = createstruct.lpCreateParams as *mut SettingsState;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
            
            // Apply DWM title bar color (must always set, not just for dark)
            let dark_mode: u32 = if let Some(st) = state_ptr.as_ref() { if st.is_dark { 1 } else { 0 } } else { 0 };
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark_mode as *const u32 as *const _,
                4
            );
            
            let instance = GetModuleHandleW(None).unwrap_or_default();
            
            // Group Box
            let grp = CreateWindowExW(
                Default::default(),
                w!("BUTTON"),
                w!("App Theme"),
                windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | BS_GROUPBOX as u32),
                10, 10, 260, 140,
                Some(hwnd),
                Some(windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_GRP_THEME as isize as *mut _)),
                Some(instance.into()),
                None
            ).unwrap_or_default();
            
            // Apply dark theme to group box - disable visual styles to allow WM_CTLCOLORSTATIC
            if let Some(st) = state_ptr.as_ref() {
                if st.is_dark {
                    let _ = SetWindowTheme(grp, w!(""), w!(""));
                }
            }

            // Radio Buttons
            let is_dark_mode = if let Some(st) = state_ptr.as_ref() { st.is_dark } else { false };
            let create_radio = |text: PCWSTR, id: u16, y: i32, checked: bool| {
                let instance = GetModuleHandleW(None).unwrap_or_default();
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
                // Apply dark theme to radio button - disable visual styles for WM_CTLCOLORSTATIC
                if is_dark_mode {
                    let _ = SetWindowTheme(h, w!(""), w!(""));
                }
            };
            
            // Determine initial check
            let theme = if let Some(st) = state_ptr.as_ref() { st.theme } else { AppTheme::System };
            
            create_radio(w!("System Default"), IDC_RADIO_SYSTEM, 40, theme == AppTheme::System);
            create_radio(w!("Dark Mode"), IDC_RADIO_DARK, 70, theme == AppTheme::Dark);
            create_radio(w!("Light Mode"), IDC_RADIO_LIGHT, 100, theme == AppTheme::Light);
            
            // Checkbox: Enable Force Stop (Auto-kill)
            let enable_force = if let Some(st) = state_ptr.as_ref() { st.enable_force_stop } else { false };
            let chk = crate::gui::controls::create_checkbox(hwnd, w!("Enable Force Stop (Auto-kill)"), 30, 160, 240, 25, IDC_CHK_FORCE_STOP);
            if enable_force {
                 SendMessageW(chk, BM_SETCHECK, Some(WPARAM(1)), None);
            }
            if let Some(st) = state_ptr.as_ref() {
                if st.is_dark {
                     let _ = SetWindowTheme(chk, w!(""), w!(""));
                }
            }

            // Buttons
            let close_btn = create_button(hwnd, ButtonOpts::new(w!("Close"), 110, 200, 80, 25, IDC_BTN_CANCEL, is_dark_mode));

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
                         
                         // Determine if new theme is dark
                         let new_is_dark = match theme {
                             AppTheme::Dark => true,
                             AppTheme::Light => false,
                             AppTheme::System => {
                                 crate::gui::theme::ThemeManager::is_system_dark_mode()
                             }
                         };
                         
                         // Update local state including is_dark
                         if let Some(st) = get_state() {
                             st.theme = theme;
                             st.result = Some(theme);
                             st.is_dark = new_is_dark;
                             
                             // Delete old brush if theme changed
                             if let Some(brush) = st.dark_brush.take() {
                                 DeleteObject(HGDIOBJ(brush.0));
                             }
                         }
                         
                         // Update Settings window title bar
                         let dark_mode: u32 = if new_is_dark { 1 } else { 0 };
                            let _ = DwmSetWindowAttribute(
                                hwnd,
                                DWMWA_USE_IMMERSIVE_DARK_MODE,
                                &dark_mode as *const u32 as *const _,
                                4
                            );
                            
                            // 5. Update controls theme
                            use windows::Win32::UI::WindowsAndMessaging::GetDlgItem;
                            let controls = [IDC_GRP_THEME, IDC_RADIO_SYSTEM, IDC_RADIO_DARK, IDC_RADIO_LIGHT, IDC_CHK_FORCE_STOP, IDC_BTN_CANCEL];
                            
                            for &ctrl_id in &controls {
                                if let Ok(h_ctl) = GetDlgItem(Some(hwnd), ctrl_id.into()) {
                                    crate::gui::theme::ThemeManager::apply_control_theme(h_ctl, new_is_dark);
                                    InvalidateRect(Some(h_ctl), None, true);
                                }
                            }
                            
                            // Repaint entire window
                            InvalidateRect(Some(hwnd), None, true);
                         
                         // Notify Parent Immediately (WM_APP + 1)
                         if let Ok(parent) = GetParent(hwnd) {
                             let theme_val = match theme {
                                 AppTheme::System => 0,
                                 AppTheme::Dark => 1,
                                 AppTheme::Light => 2,
                             };
                             SendMessageW(parent, 0x8000 + 1, Some(WPARAM(theme_val)), None);
                         }
                         
                         // Broadcast to About window if open (WM_APP + 2)
                         if let Ok(about_hwnd) = FindWindowW(w!("CompactRS_About"), None) {
                             if !about_hwnd.is_invalid() {
                                 let is_dark_val = if new_is_dark { 1 } else { 0 };
                                 SendMessageW(about_hwnd, 0x8000 + 2, Some(WPARAM(is_dark_val)), None);
                             }
                         }
                         
                         // Broadcast to Console window if open (WM_APP + 2)
                         if let Ok(console_hwnd) = FindWindowW(w!("CompactRS_Console"), None) {
                             if !console_hwnd.is_invalid() {
                                 let is_dark_val = if new_is_dark { 1 } else { 0 };
                                 SendMessageW(console_hwnd, 0x8000 + 2, Some(WPARAM(is_dark_val)), None);
                             }
                         }
                     }
                 },
                 IDC_BTN_CANCEL => {
                     // Non-modal: No need to re-enable parent
                     // if let Ok(parent) = GetParent(hwnd) {
                     //     let _ = EnableWindow(parent, true);
                     //     SetActiveWindow(parent);
                     // }
                     DestroyWindow(hwnd);
                 },
                  IDC_CHK_FORCE_STOP => {
                      if (code as u32) == BN_CLICKED {
                          if let Some(st) = get_state() {
                               let mut checked = false;
                               if let Ok(h_ctl) = windows::Win32::UI::WindowsAndMessaging::GetDlgItem(Some(hwnd), IDC_CHK_FORCE_STOP.into()) {
                                   checked = SendMessageW(h_ctl, windows::Win32::UI::WindowsAndMessaging::BM_GETCHECK, None, None) == LRESULT(1);
                                   st.enable_force_stop = checked;
                               }
                               
                               // Notify Parent immediately (WM_APP + 3)
                               if let Ok(parent) = GetParent(hwnd) {
                                   let val = if checked { 1 } else { 0 };
                                   SendMessageW(parent, 0x8000 + 3, Some(WPARAM(val)), None);
                               }
                          }
                      }
                  },
                  _ => {}
             }
             LRESULT(0)
        },
        
        WM_CLOSE => {
            // Non-modal: No need to re-enable parent
            // if let Ok(parent) = GetParent(hwnd) {
            //     let _ = EnableWindow(parent, true);
            //     SetActiveWindow(parent);
            // }
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
}}
