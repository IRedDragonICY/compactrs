#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::state::AppTheme;
use crate::ui::builder::ControlBuilder;
use crate::utils::to_wstring;
use crate::ui::framework::{WindowHandler, WindowBuilder, WindowAlignment, show_modal};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    WS_VISIBLE, WM_COMMAND,
    WS_CAPTION, WS_SYSMENU, WS_POPUP,
    BM_SETCHECK,
    SendMessageW, BN_CLICKED, DestroyWindow,
    FindWindowW,
    BM_GETCHECK, MessageBoxW, MB_ICONERROR, MB_OK,
    WM_SETTEXT,
    ShowWindow, SetForegroundWindow, SW_RESTORE,
    MB_YESNO, MB_ICONWARNING, IDYES
};

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
const IDC_CHK_LOW_POWER: u16 = 2013;
const IDC_SLIDER_THREADS: u16 = 2014;
const IDC_LBL_THREADS_VALUE: u16 = 2015;

const TBM_GETPOS: u32 = 0x0400;
const TBM_SETPOS: u32 = 0x0405;
const TBM_SETRANGE: u32 = 0x0406;
const WM_HSCROLL: u32 = 0x0114;

struct SettingsState {
    theme: AppTheme,
    result: Option<AppTheme>,
    is_dark: bool,
    enable_force_stop: bool, // Track checkbox state
    enable_context_menu: bool, // Track context menu checkbox state
    enable_system_guard: bool, // Track system guard checkbox state
    low_power_mode: bool,      // Track low power mode checkbox state
    max_threads: u32,          // Track max threads
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


// Main settings modal function with proper data passing
pub unsafe fn show_settings_modal(parent: HWND, current_theme: AppTheme, is_dark: bool, enable_force_stop: bool, enable_context_menu: bool, enable_system_guard: bool, low_power_mode: bool, max_threads: u32) -> (Option<AppTheme>, bool, bool, bool, bool, u32) {
    // Check if window already exists
    let class_name = to_wstring("CompactRS_Settings");
    let existing_hwnd = FindWindowW(class_name.as_ptr(), std::ptr::null());
    if existing_hwnd != std::ptr::null_mut() {
        ShowWindow(existing_hwnd, SW_RESTORE);
        SetForegroundWindow(existing_hwnd);
        return (None, enable_force_stop, enable_context_menu, enable_system_guard, low_power_mode, max_threads);
    }
    
    let mut state = SettingsState {
        theme: current_theme,
        result: None,
        is_dark,
        enable_force_stop,
        enable_context_menu,
        enable_system_guard,
        low_power_mode,
        max_threads,
        update_status: UpdateStatus::Idle,
    };

    let bg_brush = crate::ui::theme::get_background_brush(is_dark);

    // Use Builder to create and show modal
    show_modal(
        WindowBuilder::new(&mut state, "CompactRS_Settings", SETTINGS_TITLE)
            .style(WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE)
            .size(300, 520) // Increased height for slider
            .align(WindowAlignment::CenterOnParent)
            .background(bg_brush), // Optional, builder handles it if passed
        parent
    );
    
    (state.result, state.enable_force_stop, state.enable_context_menu, state.enable_system_guard, state.low_power_mode, state.max_threads)
}

impl WindowHandler for SettingsState {
    fn is_dark_mode(&self) -> bool {
        self.is_dark
    }

    fn on_create(&mut self, hwnd: HWND) -> LRESULT {
        unsafe {
            // Apply DWM title bar color using centralized helper
            crate::ui::theme::set_window_frame_theme(hwnd, self.is_dark);
            
            // Group Box using ControlBuilder
            let _grp = ControlBuilder::new(hwnd, IDC_GRP_THEME)
                .groupbox()
                .text("App Theme")
                .pos(10, 10)
                .size(260, 140)
                .dark_mode(self.is_dark)
                .build();

            // Radio Buttons
            let is_dark_mode = self.is_dark;
            let theme = self.theme;
            
            // Helper to create and configure radio button
            let create_radio = |text: &str, id: u16, y: i32, checked: bool| {
                let h = ControlBuilder::new(hwnd, id)
                    .radio()
                    .text(text)
                    .pos(30, y)
                    .size(200, 25)
                    .dark_mode(is_dark_mode)
                    .build();
                if checked {
                    SendMessageW(h, BM_SETCHECK, 1, 0);
                }
            };
            
            create_radio("System Default", IDC_RADIO_SYSTEM, 40, theme == AppTheme::System);
            create_radio("Dark Mode", IDC_RADIO_DARK, 70, theme == AppTheme::Dark);
            create_radio("Light Mode", IDC_RADIO_LIGHT, 100, theme == AppTheme::Light);
            
            // Checkbox: Enable Force Stop (Auto-kill)
            let _chk = ControlBuilder::new(hwnd, IDC_CHK_FORCE_STOP)
                .checkbox()
                .text("Enable Force Stop (Auto-kill)")
                .pos(30, 160)
                .size(240, 25)
                .dark_mode(is_dark_mode)
                .checked(self.enable_force_stop)
                .build();

            // Checkbox: Enable Explorer Context Menu
            let _chk_ctx = ControlBuilder::new(hwnd, IDC_CHK_CONTEXT_MENU)
                .checkbox()
                .text("Enable Explorer Context Menu")
                .pos(30, 190)
                .size(240, 25)
                .dark_mode(is_dark_mode)
                .checked(self.enable_context_menu)
                .build();

            // Checkbox: Enable System Critical Guard
            let _chk_guard = ControlBuilder::new(hwnd, IDC_CHK_SYSTEM_GUARD)
                .checkbox()
                .text("Enable System Critical Path Guard")
                .pos(30, 220)
                .size(240, 25)
                .dark_mode(is_dark_mode)
                .checked(self.enable_system_guard)
                .build();

            // Checkbox: Low Power Mode
            let _chk_low_power = ControlBuilder::new(hwnd, IDC_CHK_LOW_POWER)
                .checkbox()
                .text("Enable Low Power Mode (Eco)")
                .pos(30, 250)
                .size(240, 25)
                .dark_mode(is_dark_mode)
                .checked(self.low_power_mode)
                .build();

            // Thread Slider Panel
            let cpu_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as u32;
            let current_val = if self.max_threads == 0 { cpu_count } else { self.max_threads };
            
            // Slider Label
            let label_text = format!("Max CPU Threads: {}", current_val);
            let _lbl_threads = ControlBuilder::new(hwnd, IDC_LBL_THREADS_VALUE)
                .label(false)
                .text(&label_text)
                .pos(30, 290)
                .size(200, 20) // Increased width to fit text
                .dark_mode(is_dark_mode)
                .build();

            // Slider
            let h_slider = ControlBuilder::new(hwnd, IDC_SLIDER_THREADS)
                .trackbar()
                .pos(30, 310)
                .size(240, 30)
                .dark_mode(is_dark_mode)
                .build();
            
            // Set Range (1 to CPU count)
            // TBM_SETRANGE: WPARAM=Redraw(TRUE), LPARAM=LOWORD(Min)|HIWORD(Max)
            let range_lparam = (1 & 0xFFFF) | ((cpu_count << 16) & 0xFFFF0000);
            SendMessageW(h_slider, TBM_SETRANGE, 1, range_lparam as isize);
            
            // Set Position
            SendMessageW(h_slider, TBM_SETPOS, 1, current_val as isize);

            // Updates Section
            let _btn_update = ControlBuilder::new(hwnd, IDC_BTN_CHECK_UPDATE)
                .button()
                .text("Check for Updates")
                .pos(30, 360)
                .size(150, 25)
                .dark_mode(is_dark_mode)
                .build();
            
            let _btn_ti = ControlBuilder::new(hwnd, IDC_BTN_RESTART_TI)
                .button()
                .text("Restart as TrustedInstaller")
                .pos(30, 440) // Moved down
                .size(240, 25)
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
            let _h_lbl = ControlBuilder::new(hwnd, IDC_LBL_UPDATE_STATUS)
                .label(false) // left-aligned
                .text(&("Current Version: ".to_string() + env!("APP_VERSION")))
                .pos(30, 400) // Moved down
                .size(240, 30)
                .dark_mode(self.is_dark)
                .build();

            // Close Button
            let _close_btn = ControlBuilder::new(hwnd, IDC_BTN_CANCEL)
                .button()
                .text("Close")
                .pos(190, 360) 
                .size(80, 25)
                .dark_mode(self.is_dark)
                .build();
        }
        0
    }

    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            match msg {
                WM_HSCROLL => {
                     // Check if it's our slider
                     let h_ctl = lparam as HWND;
                     let h_slider = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_SLIDER_THREADS as i32);
                     if h_ctl == h_slider {
                         // Get Position
                         let pos = SendMessageW(h_slider, TBM_GETPOS, 0, 0);
                         self.max_threads = pos as u32;
                         
                         // Update Label
                         let label_text = format!("Max CPU Threads: {}", pos);
                         let w_text = to_wstring(&label_text);
                         let h_lbl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_LBL_THREADS_VALUE as i32);
                         SendMessageW(h_lbl, WM_SETTEXT, 0, w_text.as_ptr() as LPARAM);
                         
                         // Notify Parent
                         use windows_sys::Win32::UI::WindowsAndMessaging::GetParent;
                         let parent = GetParent(hwnd);
                         if parent != std::ptr::null_mut() {
                             // Custom message for threads? Or just update on close?
                             // Prompt didn't specify real-time parent notification, but consistent with others.
                             // We'll save on close or rely on return value.
                         }
                     }
                     Some(0)
                },
                WM_APP_UPDATE_CHECK_RESULT => {
                    let status_ptr = lparam as *mut UpdateStatus;
                    let status = Box::from_raw(status_ptr); // Take ownership
                    self.update_status = *status;
                    
                    // Update UI based on status
                    let h_btn = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_BTN_CHECK_UPDATE as i32);
                    let h_lbl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_LBL_UPDATE_STATUS as i32);
                    
                    match &self.update_status {
                        UpdateStatus::Available(ver, _) => {
                             let txt = to_wstring("Download and Restart");
                             SendMessageW(h_btn, WM_SETTEXT, 0, txt.as_ptr() as LPARAM);
                             
                             let status_txt = to_wstring(&format!("New version {} available!", ver));
                             SendMessageW(h_lbl, WM_SETTEXT, 0, status_txt.as_ptr() as LPARAM);
                             
                             // Re-enable button so user can click it
                             windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(h_btn, 1);
                        },
                        UpdateStatus::UpToDate => {
                             let txt = to_wstring("Check for Updates");
                             SendMessageW(h_btn, WM_SETTEXT, 0, txt.as_ptr() as LPARAM);
                             
                             let status_txt = to_wstring("You are up to date.");
                             SendMessageW(h_lbl, WM_SETTEXT, 0, status_txt.as_ptr() as LPARAM);
                             
                             // Re-enable button
                             windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(h_btn, 1);
                        },
                        UpdateStatus::Error(e) => {
                             let txt = to_wstring("Check for Updates");
                             SendMessageW(h_btn, WM_SETTEXT, 0, txt.as_ptr() as LPARAM);
                             
                             let status_txt = to_wstring(&format!("Error: {}", e));
                             SendMessageW(h_lbl, WM_SETTEXT, 0, status_txt.as_ptr() as LPARAM);
                             
                             windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(h_btn, 1);
                        },
                        _ => {}
                    }
                    Some(0)
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
                                 self.theme = theme;
                                 self.result = Some(theme);
                                 self.is_dark = new_is_dark;
                                 
                                 // Update Settings window title bar using centralized helper
                                 crate::ui::theme::set_window_frame_theme(hwnd, new_is_dark);
                                    
                                    // 5. Update controls theme
                                    use windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem;
                                    let controls = [IDC_GRP_THEME, IDC_RADIO_SYSTEM, IDC_RADIO_DARK, IDC_RADIO_LIGHT, IDC_CHK_FORCE_STOP, IDC_CHK_CONTEXT_MENU, IDC_CHK_SYSTEM_GUARD, IDC_CHK_LOW_POWER, IDC_BTN_CANCEL, IDC_BTN_CHECK_UPDATE, IDC_LBL_UPDATE_STATUS, IDC_BTN_RESTART_TI, IDC_SLIDER_THREADS, IDC_LBL_THREADS_VALUE];

                                    for &ctrl_id in &controls {
                                        let h_ctl = GetDlgItem(hwnd, ctrl_id as i32);
                                        if h_ctl != std::ptr::null_mut() {
                                            // Map ID to ControlType roughly
                                            let ctl_type = match ctrl_id {
                                                IDC_GRP_THEME => crate::ui::theme::ControlType::GroupBox,
                                                IDC_CHK_FORCE_STOP | IDC_CHK_CONTEXT_MENU | IDC_CHK_SYSTEM_GUARD | IDC_CHK_LOW_POWER => crate::ui::theme::ControlType::CheckBox,
                                                IDC_BTN_CANCEL | IDC_BTN_CHECK_UPDATE | IDC_BTN_RESTART_TI => crate::ui::theme::ControlType::Button,
                                                IDC_SLIDER_THREADS => crate::ui::theme::ControlType::Trackbar,
                                                IDC_LBL_THREADS_VALUE | IDC_LBL_UPDATE_STATUS => crate::ui::theme::ControlType::Window, // Label behaves like static
                                                _ => crate::ui::theme::ControlType::RadioButton, // Radio buttons
                                            };
                                            crate::ui::theme::apply_theme(h_ctl, ctl_type, new_is_dark);
                                            InvalidateRect(h_ctl, std::ptr::null(), 1);
                                        }
                                    }
                                    
                                    // Repaint entire window
                                    InvalidateRect(hwnd, std::ptr::null(), 1);
                                 
                                 // Notify Parent Immediately (WM_APP + 1)
                                 use windows_sys::Win32::UI::WindowsAndMessaging::GetParent;
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
                                   let mut checked = false;
                                   let h_ctl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_CHK_FORCE_STOP as i32);
                                   if h_ctl != std::ptr::null_mut() {
                                       checked = SendMessageW(h_ctl, BM_GETCHECK, 0, 0) == 1; // BST_CHECKED = 1
                                       self.enable_force_stop = checked;
                                   }
                                   
                                   // Notify Parent immediately (WM_APP + 3)
                                   use windows_sys::Win32::UI::WindowsAndMessaging::GetParent;
                                   let parent = GetParent(hwnd);
                                   if parent != std::ptr::null_mut() {
                                       let val = if checked { 1 } else { 0 };
                                       SendMessageW(parent, 0x8000 + 3, val as WPARAM, 0);
                                   }
                              }
                          },
                          IDC_CHK_CONTEXT_MENU => {
                               if (code as u32) == BN_CLICKED {
                                    let mut checked = false;
                                    let h_ctl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_CHK_CONTEXT_MENU as i32);
                                    if h_ctl != std::ptr::null_mut() {
                                        checked = SendMessageW(h_ctl, BM_GETCHECK, 0, 0) == 1;
                                        self.enable_context_menu = checked;
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
                                            self.enable_context_menu = false;
                                            
                                            let h_ctl_revert = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_CHK_CONTEXT_MENU as i32);
                                            if h_ctl_revert != std::ptr::null_mut() {
                                                SendMessageW(h_ctl_revert, BM_SETCHECK, 0, 0);
                                            }
                                        }
                                    } else {
                                        let _ = crate::registry::unregister_context_menu();
                                    }
                                    
                                    // Notify Parent immediately (WM_APP + 5)
                                    use windows_sys::Win32::UI::WindowsAndMessaging::GetParent;
                                    let parent = GetParent(hwnd);
                                    if parent != std::ptr::null_mut() {
                                        let val = if self.enable_context_menu { 1 } else { 0 };
                                        SendMessageW(parent, 0x8000 + 5, val as WPARAM, 0);
                                    }
                               }
                          },
                          IDC_CHK_LOW_POWER => {
                               if (code as u32) == BN_CLICKED {
                                   let mut checked = false;
                                    let h_ctl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_CHK_LOW_POWER as i32);
                                    if h_ctl != std::ptr::null_mut() {
                                        checked = SendMessageW(h_ctl, BM_GETCHECK, 0, 0) == 1;
                                        self.low_power_mode = checked;
                                    }
                                    
                                    // Notify Parent immediately (WM_APP + 7)
                                    use windows_sys::Win32::UI::WindowsAndMessaging::GetParent;
                                    let parent = GetParent(hwnd);
                                    if parent != std::ptr::null_mut() {
                                        let val = if checked { 1 } else { 0 };
                                        SendMessageW(parent, 0x8000 + 7, val as WPARAM, 0);
                                    }
                               }
                          },
                          IDC_CHK_SYSTEM_GUARD => {
                              if (code as u32) == BN_CLICKED {
                                   let mut checked = false;
                                   let h_ctl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_CHK_SYSTEM_GUARD as i32);
                                   if h_ctl != std::ptr::null_mut() {
                                       checked = SendMessageW(h_ctl, BM_GETCHECK, 0, 0) == 1;
                                       self.enable_system_guard = checked;
                                   }
                                   
                                   // Notify Parent immediately (WM_APP + 6)
                                   use windows_sys::Win32::UI::WindowsAndMessaging::GetParent;
                                   let parent = GetParent(hwnd);
                                   if parent != std::ptr::null_mut() {
                                       let val = if checked { 1 } else { 0 };
                                       SendMessageW(parent, 0x8000 + 6, val as WPARAM, 0);
                                   }
                              }
                          },
                          IDC_BTN_CHECK_UPDATE => {
                              if (code as u32) == BN_CLICKED {
                                  let clone_hwnd_ptr = hwnd as usize;
                                  match &self.update_status {
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
                                                    std::process::exit(0);
                                               }
                                          });
                                      },
                                      UpdateStatus::Checking => {}, // Ignore
                                      _ => {
                                          // Check for update
                                          self.update_status = UpdateStatus::Checking;
                                          
                                          let h_btn = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_BTN_CHECK_UPDATE as i32);
                                          windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(h_btn, 0); // Disable button
                                          let h_lbl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_LBL_UPDATE_STATUS as i32);
                                          let loading = to_wstring("Checking for updates...");
                                          SendMessageW(h_lbl, WM_SETTEXT, 0, loading.as_ptr() as LPARAM);

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
                          },
                          IDC_BTN_RESTART_TI => {
                              if (code as u32) == BN_CLICKED {
                                  let msg = to_wstring("This will restart CompactRS as System/TrustedInstaller.\n\nUse this ONLY if you need to compress protected system folders (e.g. WinSxS).\n\nAre you sure?");
                                  let title = to_wstring("Privilege Elevation");
                                  let res = MessageBoxW(hwnd, msg.as_ptr(), title.as_ptr(), MB_YESNO | MB_ICONWARNING);
                                  
                                  if res == IDYES {
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
                     Some(0)
                },
                _ => None,
            }
        }
    }
}
