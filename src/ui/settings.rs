use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, RegisterClassW,
    CS_HREDRAW, CS_VREDRAW, IDC_ARROW, WM_DESTROY, WNDCLASSW,
    WS_VISIBLE, WM_CREATE, WM_COMMAND, SetWindowLongPtrW, GWLP_USERDATA,
    WS_CHILD, WS_CAPTION, WS_SYSMENU, WS_POPUP,
    BS_AUTORADIOBUTTON, BM_SETCHECK,
    GetMessageW, TranslateMessage, DispatchMessageW, MSG,
    SendMessageW, PostQuitMessage, WM_CLOSE, BS_GROUPBOX, GetParent, BN_CLICKED, DestroyWindow,
    FindWindowW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::SetWindowTheme;
use windows::Win32::Graphics::Gdi::{HBRUSH, COLOR_WINDOW, DeleteObject, HGDIOBJ, InvalidateRect};

use crate::ui::state::AppTheme;
use crate::ui::builder::ButtonBuilder;
use crate::ui::utils::get_window_state;

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
const IDC_CHK_CONTEXT_MENU: u16 = 2008;

struct SettingsState {
    theme: AppTheme,
    result: Option<AppTheme>,
    is_dark: bool,
    dark_brush: Option<HBRUSH>,
    enable_force_stop: bool, // Track checkbox state
    enable_context_menu: bool, // Track context menu checkbox state
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

// Main settings modal function with proper data passing
pub unsafe fn show_settings_modal(parent: HWND, current_theme: AppTheme, is_dark: bool, enable_force_stop: bool, enable_context_menu: bool) -> (Option<AppTheme>, bool, bool) {
    unsafe {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        
        // Load App Icon using centralized helper
        let icon = crate::ui::utils::load_app_icon(instance.into());
        
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(settings_wnd_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hIcon: icon,
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
        let height = 330;
        let x = rect.left + (p_width - width) / 2;
        let y = rect.top + (p_height - height) / 2;

        let mut state = SettingsState {
            theme: current_theme,
            result: None,
            is_dark,
            dark_brush: None,
            enable_force_stop,
            enable_context_menu,
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
        
        (state.result, state.enable_force_stop, state.enable_context_menu)
    }
}


unsafe extern "system" fn settings_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT { unsafe {
    // Use centralized helper for state access
    let get_state = || get_window_state::<SettingsState>(hwnd);

    // Centralized handler for theme-related messages
    if let Some(st) = get_state() {
        if let Some(result) = crate::ui::theme::handle_standard_colors(hwnd, msg, wparam, st.is_dark) {
            return result;
        }
    }

    match msg {
        WM_CREATE => {
            let createstruct = &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
            let state_ptr = createstruct.lpCreateParams as *mut SettingsState;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
            
            // Apply DWM title bar color using centralized helper
            let is_dark = state_ptr.as_ref().map(|st| st.is_dark).unwrap_or(false);
            crate::ui::theme::set_window_frame_theme(hwnd, is_dark);
            
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
            let chk = crate::ui::controls::create_checkbox(hwnd, w!("Enable Force Stop (Auto-kill)"), 30, 160, 240, 25, IDC_CHK_FORCE_STOP);
            if enable_force {
                 SendMessageW(chk, BM_SETCHECK, Some(WPARAM(1)), None);
            }
            if let Some(st) = state_ptr.as_ref() {
                if st.is_dark {
                     let _ = SetWindowTheme(chk, w!(""), w!(""));
                }
            }

            // Checkbox: Enable Explorer Context Menu
            let enable_ctx = if let Some(st) = state_ptr.as_ref() { st.enable_context_menu } else { false };
            let chk_ctx = crate::ui::controls::create_checkbox(hwnd, w!("Enable Explorer Context Menu"), 30, 190, 240, 25, IDC_CHK_CONTEXT_MENU);
            if enable_ctx {
                 SendMessageW(chk_ctx, BM_SETCHECK, Some(WPARAM(1)), None);
            }
            if let Some(st) = state_ptr.as_ref() {
                if st.is_dark {
                     let _ = SetWindowTheme(chk_ctx, w!(""), w!(""));
                }
            }

            // Buttons
            let _close_btn = ButtonBuilder::new(hwnd, IDC_BTN_CANCEL)
                .text("Close").pos(110, 235).size(80, 25).dark_mode(is_dark_mode).build();

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
                                 crate::ui::theme::is_system_dark_mode()
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
                         
                         // Update Settings window title bar using centralized helper
                         crate::ui::theme::set_window_frame_theme(hwnd, new_is_dark);
                            
                            // 5. Update controls theme
                            use windows::Win32::UI::WindowsAndMessaging::GetDlgItem;
                            let controls = [IDC_GRP_THEME, IDC_RADIO_SYSTEM, IDC_RADIO_DARK, IDC_RADIO_LIGHT, IDC_CHK_FORCE_STOP, IDC_CHK_CONTEXT_MENU, IDC_BTN_CANCEL];
                            
                            for &ctrl_id in &controls {
                                if let Ok(h_ctl) = GetDlgItem(Some(hwnd), ctrl_id.into()) {
                                    crate::ui::theme::apply_control_theme(h_ctl, new_is_dark);
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
                  IDC_CHK_CONTEXT_MENU => {
                      if (code as u32) == BN_CLICKED {
                          if let Some(st) = get_state() {
                               let mut checked = false;
                               if let Ok(h_ctl) = windows::Win32::UI::WindowsAndMessaging::GetDlgItem(Some(hwnd), IDC_CHK_CONTEXT_MENU.into()) {
                                   checked = SendMessageW(h_ctl, windows::Win32::UI::WindowsAndMessaging::BM_GETCHECK, None, None) == LRESULT(1);
                                   st.enable_context_menu = checked;
                               }
                               
                               // Perform registry operation
                               if checked {
                                   if let Err(_e) = crate::registry::register_context_menu() {
                                       // Show error, revert checkbox
                                       windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                                           Some(hwnd),
                                           windows::core::w!("Failed to register context menu. Run as Administrator."),
                                           windows::core::w!("Error"),
                                           windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR | windows::Win32::UI::WindowsAndMessaging::MB_OK
                                       );
                                       st.enable_context_menu = false;
                                       if let Ok(h_ctl) = windows::Win32::UI::WindowsAndMessaging::GetDlgItem(Some(hwnd), IDC_CHK_CONTEXT_MENU.into()) {
                                           SendMessageW(h_ctl, windows::Win32::UI::WindowsAndMessaging::BM_SETCHECK, Some(WPARAM(0)), None);
                                       }
                                   }
                               } else {
                                   let _ = crate::registry::unregister_context_menu();
                               }
                               
                               // Notify Parent immediately (WM_APP + 4)
                               if let Ok(parent) = GetParent(hwnd) {
                                   let val = if st.enable_context_menu { 1 } else { 0 };
                                   SendMessageW(parent, 0x8000 + 5, Some(WPARAM(val)), None);
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
