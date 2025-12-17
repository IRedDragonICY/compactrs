#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::state::AppTheme;
use crate::ui::builder::ButtonBuilder;
use crate::ui::utils::{get_window_state, to_wstring};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, RegisterClassW,
    CS_HREDRAW, CS_VREDRAW, IDC_ARROW, WM_DESTROY, WNDCLASSW,
    WS_VISIBLE, WM_CREATE, WM_COMMAND, SetWindowLongPtrW, GWLP_USERDATA,
    WS_CHILD, WS_CAPTION, WS_SYSMENU, WS_POPUP,
    BS_AUTORADIOBUTTON, BM_SETCHECK,
    SendMessageW, PostQuitMessage, WM_CLOSE, BS_GROUPBOX, GetParent, BN_CLICKED, DestroyWindow,
    FindWindowW, HMENU, CREATESTRUCTW,
    BM_GETCHECK, MessageBoxW, MB_ICONERROR, MB_OK,
    GetWindowRect,
};
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::SetWindowTheme;
use windows_sys::Win32::Graphics::Gdi::{HBRUSH, COLOR_WINDOW, InvalidateRect};

const SETTINGS_TITLE: &str = "Settings";

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
    // dark_brush removed
    enable_force_stop: bool, // Track checkbox state
    enable_context_menu: bool, // Track context menu checkbox state
}

// Drop trait removed - resources managed globally


// Main settings modal function with proper data passing
pub unsafe fn show_settings_modal(parent: HWND, current_theme: AppTheme, is_dark: bool, enable_force_stop: bool, enable_context_menu: bool) -> (Option<AppTheme>, bool, bool) {
    let instance = GetModuleHandleW(std::ptr::null());
    
    // Load App Icon using centralized helper
    let icon = crate::ui::utils::load_app_icon(instance);
    
    let class_name = to_wstring("CompactRS_Settings");
    let title = to_wstring(SETTINGS_TITLE);
    
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(settings_wnd_proc),
        hInstance: instance,
        hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
        hIcon: icon,
        lpszClassName: class_name.as_ptr(),
        hbrBackground: crate::ui::theme::get_background_brush(is_dark),
        cbClsExtra: 0,
        cbWndExtra: 0,
        lpszMenuName: std::ptr::null(),
    };
    RegisterClassW(&wc);

    let mut rect: RECT = std::mem::zeroed();
    GetWindowRect(parent, &mut rect);
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
        enable_force_stop,
        enable_context_menu,
    };

    let _hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
        x, y, width, height,
        parent,
        std::ptr::null_mut(),
        instance,
        &mut state as *mut _ as *mut std::ffi::c_void,
    );

    // Message loop
    crate::ui::utils::run_message_loop();
    
    (state.result, state.enable_force_stop, state.enable_context_menu)
}


unsafe extern "system" fn settings_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
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
            let createstruct = &*(lparam as *const CREATESTRUCTW);
            let state_ptr = createstruct.lpCreateParams as *mut SettingsState;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
            
            // Apply DWM title bar color using centralized helper
            let is_dark = (*state_ptr).is_dark;
            crate::ui::theme::set_window_frame_theme(hwnd, is_dark);
            
            let instance = GetModuleHandleW(std::ptr::null());
            let btn_cls = to_wstring("BUTTON");
            let grp_text = to_wstring("App Theme");
            
            // Group Box
            let grp = CreateWindowExW(
                0,
                btn_cls.as_ptr(),
                grp_text.as_ptr(),
                WS_VISIBLE | WS_CHILD | BS_GROUPBOX as u32,
                10, 10, 260, 140,
                hwnd,
                IDC_GRP_THEME as isize as HMENU,
                instance,
                std::ptr::null()
            );
            
            // Apply theme to group box
            if let Some(st) = state_ptr.as_ref() {
                crate::ui::theme::apply_theme(grp, crate::ui::theme::ControlType::GroupBox, st.is_dark);
            }

            // Radio Buttons
            let is_dark_mode = if let Some(st) = state_ptr.as_ref() { st.is_dark } else { false };
            let create_radio = |text: &str, id: u16, y: i32, checked: bool| {
                let instance = GetModuleHandleW(std::ptr::null());
                let cls = to_wstring("BUTTON");
                let txt = to_wstring(text);
                
                 let h = CreateWindowExW(
                    0,
                    cls.as_ptr(),
                    txt.as_ptr(),
                    WS_VISIBLE | WS_CHILD | BS_AUTORADIOBUTTON as u32,
                    30, y, 200, 25,
                    hwnd,
                    id as isize as HMENU,
                    instance,
                    std::ptr::null()
                );
                if checked {
                    SendMessageW(h, BM_SETCHECK, 1, 0);
                }
                // Apply theme to radio button
                crate::ui::theme::apply_theme(h, crate::ui::theme::ControlType::Button, is_dark_mode);
            };
            
            // Determine initial check
            let theme = if let Some(st) = state_ptr.as_ref() { st.theme } else { AppTheme::System };
            
            create_radio("System Default", IDC_RADIO_SYSTEM, 40, theme == AppTheme::System);
            create_radio("Dark Mode", IDC_RADIO_DARK, 70, theme == AppTheme::Dark);
            create_radio("Light Mode", IDC_RADIO_LIGHT, 100, theme == AppTheme::Light);
            
            // Checkbox: Enable Force Stop (Auto-kill)
            let enable_force = if let Some(st) = state_ptr.as_ref() { st.enable_force_stop } else { false };
            let chk = crate::ui::controls::create_checkbox(hwnd, "Enable Force Stop (Auto-kill)", 30, 160, 240, 25, IDC_CHK_FORCE_STOP);
            if enable_force {
                 SendMessageW(chk, BM_SETCHECK, 1, 0);
            }
            if let Some(st) = state_ptr.as_ref() {
                crate::ui::theme::apply_theme(chk, crate::ui::theme::ControlType::CheckBox, st.is_dark);
            }

            // Checkbox: Enable Explorer Context Menu
            let enable_ctx = if let Some(st) = state_ptr.as_ref() { st.enable_context_menu } else { false };
            let chk_ctx = crate::ui::controls::create_checkbox(hwnd, "Enable Explorer Context Menu", 30, 190, 240, 25, IDC_CHK_CONTEXT_MENU);
            if enable_ctx {
                 SendMessageW(chk_ctx, BM_SETCHECK, 1, 0);
            }
            if let Some(st) = state_ptr.as_ref() {
                crate::ui::theme::apply_theme(chk_ctx, crate::ui::theme::ControlType::CheckBox, st.is_dark);
            }

            // Buttons
            let _close_btn = ButtonBuilder::new(hwnd, IDC_BTN_CANCEL)
                .text("Close").pos(110, 235).size(80, 25).dark_mode(is_dark_mode).build();

            0
        },
        
        WM_COMMAND => {
             let id = (wparam & 0xFFFF) as u16;
             let code = ((wparam >> 16) & 0xFFFF) as u16;
             
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
                             
                             // Brush management handled globally

                         }
                         
                         // Update Settings window title bar using centralized helper
                         crate::ui::theme::set_window_frame_theme(hwnd, new_is_dark);
                            
                            // 5. Update controls theme
                            use windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem;
                            let controls = [IDC_GRP_THEME, IDC_RADIO_SYSTEM, IDC_RADIO_DARK, IDC_RADIO_LIGHT, IDC_CHK_FORCE_STOP, IDC_CHK_CONTEXT_MENU, IDC_BTN_CANCEL];
                            
                            for &ctrl_id in &controls {
                                let h_ctl = GetDlgItem(hwnd, ctrl_id as i32);
                                if h_ctl != std::ptr::null_mut() {
                                    // Map ID to ControlType roughly
                                    let ctl_type = match ctrl_id {
                                        IDC_GRP_THEME => crate::ui::theme::ControlType::GroupBox,
                                        IDC_CHK_FORCE_STOP | IDC_CHK_CONTEXT_MENU => crate::ui::theme::ControlType::CheckBox,
                                        IDC_BTN_CANCEL => crate::ui::theme::ControlType::Button,
                                        _ => crate::ui::theme::ControlType::Button, // Radio buttons
                                    };
                                    crate::ui::theme::apply_theme(h_ctl, ctl_type, new_is_dark);
                                    InvalidateRect(h_ctl, std::ptr::null(), 1);
                                }
                            }
                            
                            // Repaint entire window
                            InvalidateRect(hwnd, std::ptr::null(), 1);
                         
                         // Notify Parent Immediately (WM_APP + 1)
                         let parent = GetParent(hwnd);
                         if parent != std::ptr::null_mut() {
                             let theme_val = match theme {
                                 AppTheme::System => 0,
                                 AppTheme::Dark => 1,
                                 AppTheme::Light => 2,
                             };
                             SendMessageW(parent, 0x8000 + 1, theme_val as WPARAM, 0);
                         }
                         
                         // Broadcast to About window if open (WM_APP + 2)
                         let compactrs_about = to_wstring("CompactRS_About");
                         let about_hwnd = FindWindowW(compactrs_about.as_ptr(), std::ptr::null());
                         if about_hwnd != std::ptr::null_mut() {
                             let is_dark_val = if new_is_dark { 1 } else { 0 };
                             SendMessageW(about_hwnd, 0x8000 + 2, is_dark_val as WPARAM, 0);
                         }
                         
                         // Broadcast to Console window if open (WM_APP + 2)
                         let compactrs_console = to_wstring("CompactRS_Console");
                         let console_hwnd = FindWindowW(compactrs_console.as_ptr(), std::ptr::null());
                         if console_hwnd != std::ptr::null_mut() {
                             let is_dark_val = if new_is_dark { 1 } else { 0 };
                             SendMessageW(console_hwnd, 0x8000 + 2, is_dark_val as WPARAM, 0);
                         }
                     }
                 },
                 IDC_BTN_CANCEL => {
                     DestroyWindow(hwnd);
                 },
                  IDC_CHK_FORCE_STOP => {
                      if (code as u32) == BN_CLICKED {
                          if let Some(st) = get_state() {
                               let mut checked = false;
                               let h_ctl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_CHK_FORCE_STOP as i32);
                               if h_ctl != std::ptr::null_mut() {
                                   checked = SendMessageW(h_ctl, BM_GETCHECK, 0, 0) == 1; // BST_CHECKED = 1
                                   st.enable_force_stop = checked;
                               }
                               
                               // Notify Parent immediately (WM_APP + 3)
                               let parent = GetParent(hwnd);
                               if parent != std::ptr::null_mut() {
                                   let val = if checked { 1 } else { 0 };
                                   SendMessageW(parent, 0x8000 + 3, val as WPARAM, 0);
                               }
                          }
                      }
                  },
                  IDC_CHK_CONTEXT_MENU => {
                      if (code as u32) == BN_CLICKED {
                          if let Some(st) = get_state() {
                               let mut checked = false;
                               let h_ctl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_CHK_CONTEXT_MENU as i32);
                               if h_ctl != std::ptr::null_mut() {
                                   checked = SendMessageW(h_ctl, BM_GETCHECK, 0, 0) == 1;
                                   st.enable_context_menu = checked;
                               }
                               
                               // Perform registry operation
                               if checked {
                                   if let Err(_e) = crate::registry::register_context_menu() {
                                       // Show error, revert checkbox
                                       let msg = to_wstring("Failed to register context menu. Run as Administrator.");
                                       let title = to_wstring("Error");
                                       
                                       MessageBoxW(
                                           hwnd,
                                           msg.as_ptr(),
                                           title.as_ptr(),
                                           MB_ICONERROR | MB_OK
                                       );
                                       st.enable_context_menu = false;
                                       
                                       let h_ctl_revert = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_CHK_CONTEXT_MENU as i32);
                                       if h_ctl_revert != std::ptr::null_mut() {
                                           SendMessageW(h_ctl_revert, BM_SETCHECK, 0, 0);
                                       }
                                   }
                               } else {
                                   let _ = crate::registry::unregister_context_menu();
                               }
                               
                               // Notify Parent immediately (WM_APP + 5)
                               let parent = GetParent(hwnd);
                               if parent != std::ptr::null_mut() {
                                   let val = if st.enable_context_menu { 1 } else { 0 };
                                   SendMessageW(parent, 0x8000 + 5, val as WPARAM, 0);
                               }
                          }
                      }
                  },
                  _ => {}
             }
             0
        },
        
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        },
        
        WM_DESTROY => {
            // Do NOT free GWLP_USERDATA here because it points to stack memory of caller
            PostQuitMessage(0);
            0
        },
        
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
