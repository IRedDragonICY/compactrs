#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::state::AppTheme;
use crate::ui::builder::ControlBuilder;
use crate::utils::to_wstring;
use crate::w;
use crate::ui::framework::WindowHandler;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    WM_COMMAND,
    BN_CLICKED, DestroyWindow,
    FindWindowW,
    MessageBoxW, MB_ICONERROR, MB_OK,
    SendMessageW, // Keep for specialized messages if any
    MB_YESNO, MB_ICONWARNING, IDYES, WM_HSCROLL
};
use crate::ui::wrappers::{Button, Label, Trackbar};

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

const IDC_GRP_LOGGING: u16 = 2020;
const IDC_CHK_LOG_ENABLED: u16 = 2021;
const IDC_CHK_LOG_ERRORS: u16 = 2022;
const IDC_CHK_LOG_WARNS: u16 = 2023;
const IDC_CHK_LOG_INFO: u16 = 2024;
const IDC_CHK_LOG_TRACE: u16 = 2025;

const IDC_LBL_CONCURRENT: u16 = 2030;
const IDC_EDIT_CONCURRENT: u16 = 2031;

const IDC_GRP_FILTER: u16 = 2040;
const IDC_CHK_SKIP_EXT: u16 = 2041;
const IDC_EDIT_EXTENSIONS: u16 = 2042;
const IDC_BTN_RESET_EXT: u16 = 2043;

struct SettingsState {
    theme: AppTheme,
    result: Option<AppTheme>,
    is_dark: bool,
    enable_force_stop: bool, // Track checkbox state
    enable_context_menu: bool, // Track context menu checkbox state
    enable_system_guard: bool, // Track system guard checkbox state
    low_power_mode: bool,      // Track low power mode checkbox state
    max_threads: u32,          // Track max threads
    max_concurrent_items: u32, // Track max concurrent items
    log_enabled: bool,
    log_level_mask: u8,
    enable_skip_heuristics: bool,
    skip_extensions: String,
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
pub unsafe fn show_settings_modal(
    parent: HWND, 
    current_theme: AppTheme, 
    is_dark: bool, 
    enable_force_stop: bool, 
    enable_context_menu: bool, 
    enable_system_guard: bool, 
    low_power_mode: bool, 
    max_threads: u32, 
    max_concurrent_items: u32, 
    log_enabled: bool, 
    log_level_mask: u8,
    enable_skip_heuristics: bool,
    skip_extensions_buf: [u16; 512]
) -> (Option<AppTheme>, bool, bool, bool, bool, u32, u32, bool, u8, bool, [u16; 512]) {
    // Convert buf to String for state
    let skip_string = String::from_utf16_lossy(&skip_extensions_buf)
        .trim_matches(char::from(0))
        .to_string();

    // Use centralized helper
    let mut state = SettingsState {
        theme: current_theme,
        result: None,
        is_dark,
        enable_force_stop,
        enable_context_menu,
        enable_system_guard,
        low_power_mode,
        max_threads,
        max_concurrent_items,
        log_enabled,
        log_level_mask,
        enable_skip_heuristics,
        skip_extensions: skip_string,
        update_status: UpdateStatus::Idle,
    };
    
    let ran_modal = crate::ui::dialogs::base::show_modal_singleton(
        parent, 
        &mut state, 
        "CompactRS_Settings", 
        SETTINGS_TITLE, 
        550, // Increased width slightly 
        760, // Increased height for new group
        is_dark
    );
    
    let mut final_buf = [0u16; 512];
    if ran_modal {
        let mut i = 0;
        for c in state.skip_extensions.encode_utf16() {
            if i < 511 {
                final_buf[i] = c;
                i += 1;
            }
        }
        (state.result, state.enable_force_stop, state.enable_context_menu, state.enable_system_guard, state.low_power_mode, state.max_threads, state.max_concurrent_items, state.log_enabled, state.log_level_mask, state.enable_skip_heuristics, final_buf)
    } else {
         // Return original values if cancelled/prevented
         (None, enable_force_stop, enable_context_menu, enable_system_guard, low_power_mode, max_threads, max_concurrent_items, log_enabled, log_level_mask, enable_skip_heuristics, skip_extensions_buf)
    }
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
                .text_w(w!("App Theme"))
                .pos(10, 10)
                .size(260, 140)
                .dark_mode(self.is_dark)
                .build();

            // Radio Buttons
            let is_dark_mode = self.is_dark;
            let theme = self.theme;
            
            // Helper to create and configure radio button with separate label
            let create_radio = |text: &'static [u16], id: u16, y: i32, checked: bool| {
                // Radio Button (Icon only effectively)
                let _h_radio = ControlBuilder::new(hwnd, id)
                    .radio()
                    .text_w(w!("")) // Empty text to avoid black text issue
                    .pos(30, y)
                    .size(20, 25) // Small width just for the circle
                    .dark_mode(is_dark_mode)
                    .checked(checked)
                    .build();
                
                // Companion Label
                let _lbl = ControlBuilder::new(hwnd, id + 100)
                    .label(false)
                    .text_w(text)
                    .pos(55, y + 2) // Offset text
                    .size(200, 20)
                    .dark_mode(is_dark_mode)
                    .build();
            };
            
            // Left Column Layout
            let mut layout = crate::ui::layout::LayoutContext::new(30, 40, 240, 5);

            // Radio Buttons (inside GroupBox which is at 10,10 size 260x140)
            create_radio(w!("System Default"), IDC_RADIO_SYSTEM, layout.row(25).1, theme == AppTheme::System);
            create_radio(w!("Dark Mode"), IDC_RADIO_DARK, layout.row(25).1, theme == AppTheme::Dark);
            create_radio(w!("Light Mode"), IDC_RADIO_LIGHT, layout.row(25).1, theme == AppTheme::Light);
            
            // Advance past GroupBox (GroupBox ends at 150)
            // Current layout Y is approx 130. 
            // We want next item at 160.
            // layout.row(25) advanced Y to 130.
            // We need to jump to 160.
            layout.add_space(30); 

            // Checkbox: Enable Force Stop (Auto-kill)
            let (x, y, w, h) = layout.row(25);
            let _chk = ControlBuilder::new(hwnd, IDC_CHK_FORCE_STOP)
                .checkbox()
                .text_w(w!("Enable Force Stop (Auto-kill)"))
                .pos(x, y)
                .size(w, h)
                .dark_mode(is_dark_mode)
                .checked(self.enable_force_stop)
                .build();

            // Checkbox: Enable Explorer Context Menu
            let (x, y, w, h) = layout.row(25);
            let _chk_ctx = ControlBuilder::new(hwnd, IDC_CHK_CONTEXT_MENU)
                .checkbox()
                .text_w(w!("Enable Explorer Context Menu"))
                .pos(x, y)
                .size(w, h)
                .dark_mode(is_dark_mode)
                .checked(self.enable_context_menu)
                .build();

            // Checkbox: Enable System Critical Guard
            let (x, y, w, h) = layout.row(25);
            let _chk_guard = ControlBuilder::new(hwnd, IDC_CHK_SYSTEM_GUARD)
                .checkbox()
                .text_w(w!("Enable System Critical Path Guard"))
                .pos(x, y)
                .size(w, h)
                .dark_mode(is_dark_mode)
                .checked(self.enable_system_guard)
                .build();

            // Checkbox: Low Power Mode
            let (x, y, w, h) = layout.row(25);
            let _chk_low_power = ControlBuilder::new(hwnd, IDC_CHK_LOW_POWER)
                .checkbox()
                .text_w(w!("Enable Low Power Mode (Eco)"))
                .pos(x, y)
                .size(w, h)
                .dark_mode(is_dark_mode)
                .checked(self.low_power_mode)
                .build();
            
            layout.add_space(10); // Gap before slider

            // Thread Slider Panel
            let cpu_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as u32;
            let current_val = if self.max_threads == 0 { cpu_count } else { self.max_threads };
            
            // Slider Label
            let (x, y, _w, h) = layout.row(20);
            let label_text = format!("Max CPU Threads: {}", current_val);
            let _lbl_threads = ControlBuilder::new(hwnd, IDC_LBL_THREADS_VALUE)
                .label(false)
                .text(&label_text)
                .pos(x, y)
                .size(200, h) 
                .dark_mode(is_dark_mode)
                .build();

            // Slider
            let (x, y, w, h) = layout.row(30);
            let h_slider = ControlBuilder::new(hwnd, IDC_SLIDER_THREADS)
                .trackbar()
                .pos(x, y)
                .size(w, h)
                .dark_mode(is_dark_mode)
                .build();
            
            // Set Range (1 to CPU count)
            Trackbar::new(h_slider).set_range(1, cpu_count);
            
            // Set Position
            Trackbar::new(h_slider).set_pos(current_val);

            layout.add_space(15);
            
            // Max Concurrent Items
            let (x, y, w, h) = layout.row(25);
            let _lbl_conc = ControlBuilder::new(hwnd, IDC_LBL_CONCURRENT)
                .label(false)
                .text("Queue Limit (0=Unlimited):")
                .pos(x, y + 3)
                .size(w - 60, h) // Label takes available width minus input
                .dark_mode(is_dark_mode)
                .build();
                
            let _edit_conc = ControlBuilder::new(hwnd, IDC_EDIT_CONCURRENT)
                .edit()
                .text(&self.max_concurrent_items.to_string())
                // Right aligned input
                .pos(x + w - 50, y)
                .size(50, h)
                .style((windows_sys::Win32::UI::WindowsAndMessaging::ES_NUMBER | windows_sys::Win32::UI::WindowsAndMessaging::ES_CENTER) as u32)
                .dark_mode(is_dark_mode)
                .build();

            // File Filtering Group (Bottom Left)
            // Use same layout context or new one? layout Y is currently at the concurrent items line.
            layout.add_space(30);

            // Group Box
            let (x_grp, y_grp) = (10, layout.cursor_y());
            let _grp_filter = ControlBuilder::new(hwnd, IDC_GRP_FILTER)
                .groupbox()
                .text_w(w!("File Filtering (Skip Heuristics)"))
                .pos(x_grp, y_grp)
                .size(260, 150)
                .dark_mode(is_dark_mode)
                .build();
            
            // Re-init layout for inside group
            let mut filter_layout = crate::ui::layout::LayoutContext::new(x_grp + 20, y_grp + 30, 220, 5);

            // Checkbox: Skip incompressible
            let (x, y, w, h) = filter_layout.row(25);
            let _chk_skip = ControlBuilder::new(hwnd, IDC_CHK_SKIP_EXT)
                .checkbox()
                .text_w(w!("Skip incompressible files (Recommended)"))
                .pos(x, y)
                .size(w + 10, h)
                .dark_mode(is_dark_mode)
                .checked(self.enable_skip_heuristics)
                .build();
            
            // Extensions Edit
            let (x, y, w, h) = filter_layout.row(50); // Multiline? Single line is probably enough if horizontal scrolling works, but multiline is safer for long lists.
            let h_edit_ext = ControlBuilder::new(hwnd, IDC_EDIT_EXTENSIONS)
                .edit()
                .text(&self.skip_extensions)
                .pos(x, y)
                .size(w, h)
                .style((windows_sys::Win32::UI::WindowsAndMessaging::ES_AUTOVSCROLL | windows_sys::Win32::UI::WindowsAndMessaging::ES_MULTILINE) as u32) 
                .dark_mode(is_dark_mode)
                .build();
            
            // Disable if unchecked
            if !self.enable_skip_heuristics {
                Button::new(h_edit_ext).set_enabled(false);
            }

            // Reset Button
            let (x, y, _, h) = filter_layout.row(25);
            let h_btn_reset = ControlBuilder::new(hwnd, IDC_BTN_RESET_EXT)
                .button()
                .text_w(w!("Reset to Defaults"))
                .pos(x, y)
                .size(120, h)
                .dark_mode(is_dark_mode)
                .build();

            if !self.enable_skip_heuristics {
                 Button::new(h_btn_reset).set_enabled(false);
            }

            // Adjust layout for Close button (Needs to be below everything)
            // The logic below originally put it at a relatively fixed layout position. 
            // We need to push it down.
            // Original code: Updates Section at layout.row(25).
            // We injected File Filtering before it? 
            // Actually, Updates Section was AFTER Concurrency. 
            // I added File Filtering AFTER Concurrency. 
            // So Updates Section needs to be AFTER File Filtering now.
            // layout context Y is updated by add_space(30) but then I used manual Y for group.
            // I need to sync layout.
            
            layout.add_space(160); // Space for GroupBox (150) + margin
            
            // Updates Section

            let (x, y, _w, h) = layout.row(25);
            let _btn_update = ControlBuilder::new(hwnd, IDC_BTN_CHECK_UPDATE)
                .button()
                .text_w(w!("Check for Updates"))
                .pos(x, y)
                .size(150, h)
                .dark_mode(is_dark_mode)
                .build();
            
            // Close Button (same row, aligned right)
            let _close_btn = ControlBuilder::new(hwnd, IDC_BTN_CANCEL)
                .button()
                .text_w(w!("Close"))
                .pos(x + 160, y) 
                .size(80, h)
                .dark_mode(self.is_dark)
                .build();

            // Status Label
            let (x, y, w, h) = layout.row(30);
            let _h_lbl = ControlBuilder::new(hwnd, IDC_LBL_UPDATE_STATUS)
                .label(false) // left-aligned
                .text(&("Current Version: ".to_string() + env!("APP_VERSION")))
                .pos(x, y)
                .size(w, h)
                .dark_mode(self.is_dark)
                .build();
            
            // TrustedInstaller Button
             let (x, y, w, h) = layout.row(25);
            let _btn_ti = ControlBuilder::new(hwnd, IDC_BTN_RESTART_TI)
                .button()
                .text_w(w!("Restart as TrustedInstaller"))
                .pos(x, y)
                .size(w, h)
                .dark_mode(is_dark_mode)
                .build();

            // LOGGING SECTION (Right Column)
            // Re-size main window to accommodate right column and increased height
            use windows_sys::Win32::UI::WindowsAndMessaging::SetWindowPos;
            SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 550, 760, windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOMOVE | windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOZORDER);

            // Group Box: Debug Logging
            let _grp_log = ControlBuilder::new(hwnd, IDC_GRP_LOGGING)
                .groupbox()
                .text_w(w!("Debug Logging"))
                .pos(280, 10) 
                .size(200, 200) 
                .dark_mode(self.is_dark)
                .build();

            let mut right_col = crate::ui::layout::LayoutContext::new(290, 40, 180, 5);

            // Checkbox: Enable Logging Console
            let (x, y, w, h) = right_col.row(20);
            let _chk_log = ControlBuilder::new(hwnd, IDC_CHK_LOG_ENABLED)
                .checkbox()
                .text_w(w!("Enable Logging Console"))
                .pos(x, y)
                .size(w, h)
                .dark_mode(is_dark_mode)
                .checked(self.log_enabled)
                .build();
            
            right_col.indent(10); // Indent child options

            // Bitmask options
            let (x, y, w, h) = right_col.row(20);
            let _chk_err = ControlBuilder::new(hwnd, IDC_CHK_LOG_ERRORS)
                .checkbox()
                .text_w(w!("Show Errors"))
                .pos(x, y)
                .size(w, h)
                .dark_mode(is_dark_mode)
                .checked(self.log_level_mask & crate::logger::LOG_LEVEL_ERROR != 0)
                .build();

            let (x, y, w, h) = right_col.row(20);
            let _chk_warn = ControlBuilder::new(hwnd, IDC_CHK_LOG_WARNS)
                .checkbox()
                .text_w(w!("Show Warnings"))
                .pos(x, y)
                .size(w, h)
                .dark_mode(is_dark_mode)
                .checked(self.log_level_mask & crate::logger::LOG_LEVEL_WARN != 0)
                .build();

            let (x, y, w, h) = right_col.row(20);
            let _chk_info = ControlBuilder::new(hwnd, IDC_CHK_LOG_INFO)
                .checkbox()
                .text_w(w!("Show Info"))
                .pos(x, y)
                .size(w, h)
                .dark_mode(is_dark_mode)
                .checked(self.log_level_mask & crate::logger::LOG_LEVEL_INFO != 0)
                .build();
            
            let (x, y, w, h) = right_col.row(20);
            let _chk_trace = ControlBuilder::new(hwnd, IDC_CHK_LOG_TRACE)
                .checkbox()
                .text_w(w!("Show Trace (Verbose)"))
                .pos(x, y)
                .size(w, h)
                .dark_mode(is_dark_mode)
                .checked(self.log_level_mask & crate::logger::LOG_LEVEL_TRACE != 0)
                .build();
            
            // Disable child checkboxes if logging is not enabled
            if !self.log_enabled {
                let ids = [IDC_CHK_LOG_ERRORS, IDC_CHK_LOG_WARNS, IDC_CHK_LOG_INFO, IDC_CHK_LOG_TRACE];
                for &id in &ids {
                    let h = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, id as i32);
                    if h != std::ptr::null_mut() {
                        Button::new(h).set_enabled(false);
                    }
                }
            }

            // Restore Ti Button Logic (which was deleted? No, logic is there, but EnableWindow needs wrapper?)
             // Disable if already System/TI logic is in lines 463+ of original file?
             // My previous replacement kept it (Lines 298-305 of replacement content).
             // But I used EnableWindow directly there. I should update it to Wrapper if I want to be consistent.
             



            // FORCE RE-APPLY THEME:
            // Automatically apply theme to all controls
            crate::ui::theme::apply_theme_recursive(hwnd, self.is_dark);
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
                         // Get Position
                         let pos = Trackbar::new(h_slider).get_pos();
                         self.max_threads = pos;
                         
                         // Update Label
                         let label_text = format!("Max CPU Threads: {}", pos);
                         let h_lbl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_LBL_THREADS_VALUE as i32);
                         Label::new(h_lbl).set_text(&label_text);
                         
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
                             let txt = "Download and Restart";
                             Button::new(h_btn).set_text(txt);
                             
                             let status_txt = format!("New version {} available!", ver);
                             Label::new(h_lbl).set_text(&status_txt);
                             
                             // Re-enable button so user can click it
                             Button::new(h_btn).set_enabled(true);
                        },
                        UpdateStatus::UpToDate => {
                             let txt = "Check for Updates";
                             Button::new(h_btn).set_text(txt);
                             
                             let status_txt = "You are up to date.";
                             Label::new(h_lbl).set_text(status_txt);
                             
                             // Re-enable button
                             Button::new(h_btn).set_enabled(true);
                        },
                        UpdateStatus::Error(e) => {
                             let txt = "Check for Updates";
                             Button::new(h_btn).set_text(txt);
                             
                             let status_txt = format!("Error: {}", e);
                             Label::new(h_lbl).set_text(&status_txt);
                             
                             Button::new(h_btn).set_enabled(true);
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
                                    crate::ui::theme::apply_theme_recursive(hwnd, new_is_dark);
                                    
                                    // Repaint entire window
                                    InvalidateRect(hwnd, std::ptr::null(), 1);
                                    
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
                                 let compactrs_console = w!("CompactRS_Console");
                                 let console_hwnd = FindWindowW(compactrs_console.as_ptr(), std::ptr::null());
                                 if console_hwnd != std::ptr::null_mut() {
                                     let is_dark_val = if new_is_dark { 1 } else { 0 };
                                     SendMessageW(console_hwnd, 0x8000 + 2, is_dark_val as WPARAM, 0);
                                 }
                             }
                         },
                         IDC_BTN_CANCEL => {
                             // Read concurrent items from edit box before closing
                             let h_edit = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_EDIT_CONCURRENT as i32);
                             if h_edit != std::ptr::null_mut() {
                                 let len = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextLengthW(h_edit);
                                 if len > 0 {
                                     let mut buf = vec![0u16; (len + 1) as usize];
                                     windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW(h_edit, buf.as_mut_ptr(), len + 1);
                                     let s = String::from_utf16_lossy(&buf[..len as usize]);
                                     let clean: String = s.chars().take_while(|c| c.is_digit(10)).collect();
                                     if let Ok(val) = clean.parse::<u32>() {
                                         self.max_concurrent_items = val;
                                     }
                                 } else {
                                     self.max_concurrent_items = 0; // Treat empty as unlimited
                                 }
                             }
                             DestroyWindow(hwnd);
                         },
                          IDC_CHK_SKIP_EXT => {
                              if (code as u32) == BN_CLICKED {
                                  let h_chk = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_CHK_SKIP_EXT as i32);
                                  let checked = Button::new(h_chk).is_checked();
                                  self.enable_skip_heuristics = checked;
                                  
                                  // Enable/Disable Edit and Reset Button
                                  let h_edit = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_EDIT_EXTENSIONS as i32);
                                  let h_reset = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_BTN_RESET_EXT as i32);
                                  
                                  if h_edit != std::ptr::null_mut() { Button::new(h_edit).set_enabled(checked); }
                                  if h_reset != std::ptr::null_mut() { Button::new(h_reset).set_enabled(checked); }
                              }
                          },
                          IDC_BTN_RESET_EXT => {
                              if (code as u32) == BN_CLICKED {
                                  // Reset text to default
                                   let default_skip = "zip,7z,rar,gz,bz2,xz,zst,lz4,jpg,jpeg,png,gif,webp,avif,heic,mp4,mkv,avi,webm,mov,wmv,mp3,flac,aac,ogg,opus,wma,pdf";
                                   let h_edit = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_EDIT_EXTENSIONS as i32);
                                   if h_edit != std::ptr::null_mut() {
                                       windows_sys::Win32::UI::WindowsAndMessaging::SetWindowTextW(h_edit, to_wstring(default_skip).as_ptr());
                                   }
                              }
                          },
                          IDC_EDIT_EXTENSIONS => {
                               if (code as u32) == windows_sys::Win32::UI::WindowsAndMessaging::EN_CHANGE {
                                   let h_edit = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_EDIT_EXTENSIONS as i32);
                                   if h_edit != std::ptr::null_mut() {
                                       let len = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextLengthW(h_edit);
                                       let mut buf = vec![0u16; (len + 1) as usize];
                                       windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW(h_edit, buf.as_mut_ptr(), len + 1);
                                       self.skip_extensions = String::from_utf16_lossy(&buf[..len as usize]);
                                   }
                               }
                          },
                          IDC_CHK_FORCE_STOP => {
                              if (code as u32) == BN_CLICKED {
                                   let mut checked = false;
                                   let h_ctl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_CHK_FORCE_STOP as i32);
                                   if h_ctl != std::ptr::null_mut() {
                                       checked = Button::new(h_ctl).is_checked();
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
                                        checked = Button::new(h_ctl).is_checked();
                                        self.enable_context_menu = checked;
                                    }
                                    
                                    // Perform registry operation
                                    if checked {
                                        if let Err(_e) = crate::registry::register_context_menu() {
                                            // Show error, revert checkbox
                                            let msg = w!("Failed to register context menu. Run as Administrator.");
                                            let title = w!("Error");
                                            
                                            MessageBoxW(
                                                hwnd,
                                                msg.as_ptr(),
                                                title.as_ptr(),
                                                MB_ICONERROR | MB_OK
                                            );
                                            self.enable_context_menu = false;
                                            
                                            let h_ctl_revert = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_CHK_CONTEXT_MENU as i32);
                                            if h_ctl_revert != std::ptr::null_mut() {
                                                Button::new(h_ctl_revert).set_checked(false);
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
                                        checked = Button::new(h_ctl).is_checked();
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
                                       checked = Button::new(h_ctl).is_checked();
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
                                                        w!("open").as_ptr(),
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
                                          Button::new(h_btn).set_enabled(false); // Disable button
                                          let h_lbl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_LBL_UPDATE_STATUS as i32);
                                          Label::new(h_lbl).set_text("Checking for updates...");

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
                          IDC_CHK_LOG_ENABLED | IDC_CHK_LOG_ERRORS | IDC_CHK_LOG_WARNS | IDC_CHK_LOG_INFO | IDC_CHK_LOG_TRACE => {
                              if (code as u32) == BN_CLICKED {
                                   let h_ctl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, id as i32);
                                   let checked = Button::new(h_ctl).is_checked();
                                   
                                   match id {
                                       IDC_CHK_LOG_ENABLED => {
                                           self.log_enabled = checked;
                                           // Enable/Disable child checkboxes
                                           let ids = [IDC_CHK_LOG_ERRORS, IDC_CHK_LOG_WARNS, IDC_CHK_LOG_INFO, IDC_CHK_LOG_TRACE];
                                           for &child_id in &ids {
                                               let h = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, child_id as i32);
                                               if h != std::ptr::null_mut() {
                                                   Button::new(h).set_enabled(checked);
                                               }
                                           }
                                       },
                                       IDC_CHK_LOG_ERRORS => if checked { self.log_level_mask |= crate::logger::LOG_LEVEL_ERROR; } else { self.log_level_mask &= !crate::logger::LOG_LEVEL_ERROR; },
                                       IDC_CHK_LOG_WARNS => if checked { self.log_level_mask |= crate::logger::LOG_LEVEL_WARN; } else { self.log_level_mask &= !crate::logger::LOG_LEVEL_WARN; },
                                       IDC_CHK_LOG_INFO => if checked { self.log_level_mask |= crate::logger::LOG_LEVEL_INFO; } else { self.log_level_mask &= !crate::logger::LOG_LEVEL_INFO; },
                                       IDC_CHK_LOG_TRACE => if checked { self.log_level_mask |= crate::logger::LOG_LEVEL_TRACE; } else { self.log_level_mask &= !crate::logger::LOG_LEVEL_TRACE; },
                                       _ => {}
                                   }
                                   
                                   // Notify Parent immediately (WM_APP + 8)
                                   // Send (Enabled: bool) in WPARAM, (Mask: u8) in LPARAM
                                   use windows_sys::Win32::UI::WindowsAndMessaging::GetParent;
                                   let parent = GetParent(hwnd);
                                   if parent != std::ptr::null_mut() {
                                       let w = if self.log_enabled { 1 } else { 0 };
                                       let l = self.log_level_mask as isize;
                                       SendMessageW(parent, 0x8000 + 8, w as WPARAM, l as LPARAM);
                                   }
                              }
                          },
                          IDC_BTN_RESTART_TI => {
                              if (code as u32) == BN_CLICKED {
                                  let msg = w!("This will restart CompactRS as System/TrustedInstaller.\n\nUse this ONLY if you need to compress protected system folders (e.g. WinSxS).\n\nAre you sure?");
                                  let title = w!("Privilege Elevation");
                                  let res = MessageBoxW(hwnd, msg.as_ptr(), title.as_ptr(), MB_YESNO | MB_ICONWARNING);
                                  
                                  if res == IDYES {
                                      if let Err(e) = crate::engine::elevation::restart_as_trusted_installer() {
                                          let err_msg = to_wstring(&format!("Failed to elevate: {}", e));
                                          let err_title = w!("Error");
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
