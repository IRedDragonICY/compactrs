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
    GetWindowRect, WM_SETTEXT, AdjustWindowRect,
};
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::Graphics::Gdi::InvalidateRect;

const SETTINGS_TITLE: &str = "Settings";

// Control IDs
const IDC_GRP_THEME: u16 = 2001;
const IDC_RADIO_SYSTEM: u16 = 2002;
const IDC_RADIO_DARK: u16 = 2003;
const IDC_RADIO_LIGHT: u16 = 2004;

const IDC_BTN_CANCEL: u16 = 2006;
const IDC_CHK_FORCE_STOP: u16 = 2007;
const IDC_CHK_CONTEXT_MENU: u16 = 2008;
const IDC_CHK_SYSTEM_GUARD: u16 = 2009;

struct SettingsState {
    theme: AppTheme,
    result: Option<AppTheme>,
    is_dark: bool,
    // dark_brush removed
    enable_force_stop: bool, // Track checkbox state
    enable_context_menu: bool, // Track context menu checkbox state
    enable_system_guard: bool, // Track system guard checkbox state
    update_status: UpdateStatus,
}

#[derive(Clone, Debug, PartialEq)]
enum UpdateStatus {
    Idle,
    Checking,
    Available(String, String), // Version, URL
    UpToDate,
    Error(String),
}

const WM_APP_UPDATE_CHECK_RESULT: u32 = 0x8000 + 10;
const IDC_BTN_CHECK_UPDATE: u16 = 2010;
const IDC_LBL_UPDATE_STATUS: u16 = 2011;
const IDC_BTN_RESTART_TI: u16 = 2012;


// Drop trait removed - resources managed globally


// Main settings modal function with proper data passing
pub unsafe fn show_settings_modal(parent: HWND, current_theme: AppTheme, is_dark: bool, enable_force_stop: bool, enable_context_menu: bool, enable_system_guard: bool) -> (Option<AppTheme>, bool, bool, bool) {
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

    // Calculate required window size for desired client area
    // Calculate required window size for desired client area
    // Increased height to 400 to accommodate TI button
    let mut client_rect = RECT { left: 0, top: 0, right: 300, bottom: 400 };
    // AdjustWindowRect calculates the required window size based on client size + styles
    AdjustWindowRect(&mut client_rect, WS_POPUP | WS_CAPTION | WS_SYSMENU, 0);
    
    let width = client_rect.right - client_rect.left;
    let height = client_rect.bottom - client_rect.top;

    let x = rect.left + (p_width - width) / 2;
    let y = rect.top + (p_height - height) / 2;

    let mut state = SettingsState {
        theme: current_theme,
        result: None,
        is_dark,
        enable_force_stop,
        enable_context_menu,
        enable_system_guard,
        update_status: UpdateStatus::Idle,
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
    
    (state.result, state.enable_force_stop, state.enable_context_menu, state.enable_system_guard)
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
                crate::ui::theme::apply_theme(h, crate::ui::theme::ControlType::RadioButton, is_dark_mode);
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

            // Checkbox: Enable System Critical Guard
            let enable_guard = if let Some(st) = state_ptr.as_ref() { st.enable_system_guard } else { true };
            let chk_guard = crate::ui::controls::create_checkbox(hwnd, "Enable System Critical Path Guard", 30, 220, 240, 25, IDC_CHK_SYSTEM_GUARD);
            if enable_guard {
                 SendMessageW(chk_guard, BM_SETCHECK, 1, 0);
            }
            if let Some(st) = state_ptr.as_ref() {
                crate::ui::theme::apply_theme(chk_guard, crate::ui::theme::ControlType::CheckBox, st.is_dark);
            }

            // Updates Section
            let _btn_update = ButtonBuilder::new(hwnd, IDC_BTN_CHECK_UPDATE)
                .text("Check for Updates")
                .pos(30, 250).size(150, 25)
                .dark_mode(is_dark_mode)
                .build();
            
            let _btn_ti = ButtonBuilder::new(hwnd, IDC_BTN_RESTART_TI)
                .text("Restart as TrustedInstaller")
                .pos(30, 320).size(240, 25)
                .dark_mode(is_dark_mode)
                .build();
            
            // Disable if already System/TI
            if crate::engine::elevation::is_system_or_ti() {
                use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
                let btn_ti = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_BTN_RESTART_TI as i32);
                let txt = to_wstring("Running as TrustedInstaller");
                SendMessageW(btn_ti, WM_SETTEXT, 0, txt.as_ptr() as LPARAM);
                EnableWindow(btn_ti, 0);
            }

            // Status Label
            let lbl_status = to_wstring(&("Current Version: ".to_string() + env!("APP_VERSION")));
            let h_lbl = CreateWindowExW(
                0, to_wstring("STATIC").as_ptr(), lbl_status.as_ptr(),
                WS_VISIBLE | WS_CHILD,
                30, 280, 240, 40, // Increased height to 40 for wrapping
                hwnd, IDC_LBL_UPDATE_STATUS as isize as HMENU, instance, std::ptr::null()
            );
            if let Some(st) = state_ptr.as_ref() {
                // Use GroupBox theme for static text as it's usually transparent/neutral
                crate::ui::theme::apply_theme(h_lbl, crate::ui::theme::ControlType::GroupBox, st.is_dark);
            }

            // Buttons
            let _close_btn = ButtonBuilder::new(hwnd, IDC_BTN_CANCEL)
                .text("Close").pos(190, 250).size(80, 25).dark_mode(is_dark_mode).build();

            0
        },
        
        WM_APP_UPDATE_CHECK_RESULT => {
            if let Some(st) = get_state() {
                let status_ptr = lparam as *mut UpdateStatus;
                let status = Box::from_raw(status_ptr); // Take ownership
                st.update_status = *status;
                
                // Update UI based on status
                let h_btn = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_BTN_CHECK_UPDATE as i32);
                let h_lbl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_LBL_UPDATE_STATUS as i32);
                
                match &st.update_status {
                    UpdateStatus::Available(ver, _) => {
                         let txt = to_wstring("Download and Restart");
                         SendMessageW(h_btn, starts_with_wm_settext(), 0, txt.as_ptr() as LPARAM);
                         
                         let status_txt = to_wstring(&format!("New version {} available!", ver));
                         SendMessageW(h_lbl, starts_with_wm_settext(), 0, status_txt.as_ptr() as LPARAM);
                         
                         // Re-enable button so user can click it
                         windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(h_btn, 1);
                    },
                    UpdateStatus::UpToDate => {
                         let txt = to_wstring("Check for Updates");
                         SendMessageW(h_btn, starts_with_wm_settext(), 0, txt.as_ptr() as LPARAM);
                         
                         let status_txt = to_wstring("You are up to date.");
                         SendMessageW(h_lbl, starts_with_wm_settext(), 0, status_txt.as_ptr() as LPARAM);
                         
                         // Re-enable button
                         windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(h_btn, 1);
                    },
                    UpdateStatus::Error(e) => {
                         let txt = to_wstring("Check for Updates");
                         SendMessageW(h_btn, starts_with_wm_settext(), 0, txt.as_ptr() as LPARAM);
                         
                         let status_txt = to_wstring(&format!("Error: {}", e));
                         SendMessageW(h_lbl, starts_with_wm_settext(), 0, status_txt.as_ptr() as LPARAM);
                         
                         windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(h_btn, 1);
                    },
                    _ => {}
                }
            }
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
                            let controls = [IDC_GRP_THEME, IDC_RADIO_SYSTEM, IDC_RADIO_DARK, IDC_RADIO_LIGHT, IDC_CHK_FORCE_STOP, IDC_CHK_CONTEXT_MENU, IDC_CHK_SYSTEM_GUARD, IDC_BTN_CANCEL, IDC_BTN_CHECK_UPDATE, IDC_LBL_UPDATE_STATUS, IDC_BTN_RESTART_TI];

                            
                            for &ctrl_id in &controls {
                                let h_ctl = GetDlgItem(hwnd, ctrl_id as i32);
                                if h_ctl != std::ptr::null_mut() {
                                    // Map ID to ControlType roughly
                                    let ctl_type = match ctrl_id {
                                        IDC_GRP_THEME => crate::ui::theme::ControlType::GroupBox,
                                        IDC_CHK_FORCE_STOP | IDC_CHK_CONTEXT_MENU | IDC_CHK_SYSTEM_GUARD => crate::ui::theme::ControlType::CheckBox,
                                        IDC_BTN_CANCEL | IDC_BTN_CHECK_UPDATE | IDC_BTN_RESTART_TI => crate::ui::theme::ControlType::Button,
                                        _ => crate::ui::theme::ControlType::RadioButton, // Radio buttons
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
                  IDC_CHK_SYSTEM_GUARD => {
                      if (code as u32) == BN_CLICKED {
                          if let Some(st) = get_state() {
                               let mut checked = false;
                               let h_ctl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_CHK_SYSTEM_GUARD as i32);
                               if h_ctl != std::ptr::null_mut() {
                                   checked = SendMessageW(h_ctl, BM_GETCHECK, 0, 0) == 1;
                                   st.enable_system_guard = checked;
                               }
                               
                               // Notify Parent immediately (WM_APP + 6)
                               let parent = GetParent(hwnd);
                               if parent != std::ptr::null_mut() {
                                   let val = if checked { 1 } else { 0 };
                                   SendMessageW(parent, 0x8000 + 6, val as WPARAM, 0);
                               }
                          }
                      }
                  },
                  IDC_BTN_CHECK_UPDATE => {
                      if (code as u32) == BN_CLICKED {
                          let clone_hwnd_ptr = hwnd as usize;
                          if let Some(st) = get_state() {
                              match &st.update_status {
                                  UpdateStatus::Available(_, url) => {
                                      let url = url.clone();
                                      // Start Update
                                      std::thread::spawn(move || {
                                           let clone_hwnd = clone_hwnd_ptr as HWND;
                                           if let Err(e) = crate::updater::download_and_start_update(&url) {
                                                let status = Box::new(UpdateStatus::Error(e));
                                                SendMessageW(clone_hwnd, WM_APP_UPDATE_CHECK_RESULT, 0, Box::into_raw(status) as LPARAM);
                                           } else {
                                                // Restart Application
                                                unsafe {
                                                    use windows_sys::Win32::UI::Shell::ShellExecuteW;
                                                    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW;
                                                    
                                                    let exe = std::env::current_exe().unwrap_or_default();
                                                    let exe_path = crate::utils::to_wstring(exe.to_str().unwrap_or(""));
                                                    
                                                    ShellExecuteW(
                                                        std::ptr::null_mut(),
                                                        crate::utils::to_wstring("open").as_ptr(),
                                                        exe_path.as_ptr(),
                                                        std::ptr::null(),
                                                        std::ptr::null(),
                                                        SW_SHOW
                                                    );
                                                }
                                                std::process::exit(0);
                                           }
                                      });
                                  },
                                  UpdateStatus::Checking => {}, // Ignore
                                  _ => {
                                      // Check for update
                                      st.update_status = UpdateStatus::Checking;
                                      
                                      let h_btn = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_BTN_CHECK_UPDATE as i32);
                                      windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(h_btn, 0); // Disable button
                                      let h_lbl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_LBL_UPDATE_STATUS as i32);
                                      let loading = to_wstring("Checking for updates...");
                                      SendMessageW(h_lbl, starts_with_wm_settext(), 0, loading.as_ptr() as LPARAM);

                                      let loading = to_wstring("Checking for updates...");
                                      SendMessageW(h_lbl, starts_with_wm_settext(), 0, loading.as_ptr() as LPARAM);

                                      let clone_hwnd_ptr = hwnd as usize;
                                      std::thread::spawn(move || {
                                          let clone_hwnd = clone_hwnd_ptr as HWND;
                                          let res = match crate::updater::check_for_updates() {
                                              Ok(Some(info)) => UpdateStatus::Available(info.version, info.download_url),
                                              Ok(None) => UpdateStatus::UpToDate,
                                              Err(e) => UpdateStatus::Error(e),
                                          };
                                          let boxed = Box::new(res);
                                          SendMessageW(clone_hwnd, WM_APP_UPDATE_CHECK_RESULT, 0, Box::into_raw(boxed) as LPARAM);
                                      });
                                  }
                              }
                          }
                      }
                  },
                  IDC_BTN_RESTART_TI => {
                      if (code as u32) == BN_CLICKED {
                          let msg = to_wstring("This will restart CompactRS as System/TrustedInstaller.\n\nUse this ONLY if you need to compress protected system folders (e.g. WinSxS).\n\nAre you sure?");
                          let title = to_wstring("Privilege Elevation");
                          let res = MessageBoxW(hwnd, msg.as_ptr(), title.as_ptr(), windows_sys::Win32::UI::WindowsAndMessaging::MB_YESNO | windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONWARNING);
                          
                          if res == windows_sys::Win32::UI::WindowsAndMessaging::IDYES {
                              if let Err(e) = crate::engine::elevation::restart_as_trusted_installer() {
                                  let err_msg = to_wstring(&format!("Failed to elevate: {}", e));
                                  let err_title = to_wstring("Error");
                                  MessageBoxW(hwnd, err_msg.as_ptr(), err_title.as_ptr(), MB_ICONERROR | MB_OK);
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


fn starts_with_wm_settext() -> u32 { WM_SETTEXT }
