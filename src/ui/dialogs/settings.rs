#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::state::AppTheme;
use crate::ui::builder::ControlBuilder;
use crate::utils::to_wstring;
use crate::w;
use crate::ui::framework::WindowHandler;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    InvalidateRect, CreateFontIndirectW, GetObjectW, DeleteObject, LOGFONTW, FW_BOLD, HFONT,
    GetStockObject, DEFAULT_GUI_FONT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    WM_COMMAND, WM_HSCROLL,
    BN_CLICKED, CBN_SELCHANGE,
    DestroyWindow, FindWindowW, MessageBoxW, SendMessageW,
    MB_ICONERROR, MB_OK, MB_YESNO, MB_ICONWARNING, IDYES,
};
use crate::ui::wrappers::{Button, Label, Trackbar, ComboBox};

const SETTINGS_TITLE: &str = "Settings";

// Control IDs
const IDC_COMBO_THEME: u16 = 2001;
// const IDC_RADIO_SYSTEM: u16 = 2002; // Removed
// const IDC_RADIO_DARK: u16 = 2003;   // Removed
// const IDC_RADIO_LIGHT: u16 = 2004;  // Removed

const IDC_BTN_CANCEL: u16 = 2006;
const IDC_CHK_FORCE_STOP: u16 = 2007;
const IDC_CHK_CONTEXT_MENU: u16 = 2008;
const IDC_CHK_SYSTEM_GUARD: u16 = 2009;
const IDC_CHK_LOW_POWER: u16 = 2013;
const IDC_SLIDER_THREADS: u16 = 2014;
const IDC_LBL_THREADS_VALUE: u16 = 2015;

const IDC_CHK_LOG_ENABLED: u16 = 2021;
const IDC_CHK_LOG_ERRORS: u16 = 2022;
const IDC_CHK_LOG_WARNS: u16 = 2023;
const IDC_CHK_LOG_INFO: u16 = 2024;
const IDC_CHK_LOG_TRACE: u16 = 2025;

const IDC_EDIT_CONCURRENT: u16 = 2031;

const IDC_CHK_SKIP_EXT: u16 = 2041;
const IDC_EDIT_EXTENSIONS: u16 = 2042;
const IDC_BTN_RESET_EXT: u16 = 2043;

// Labels for Titles/Subtitles (Dynamic IDs or just static 0xffff if no interaction needed? 
// We generally don't need to interact with labels unless updating text. 
// We'll let ControlBuilder auto-assign or use a base range if needed. 
// For now, let's keep strict constants usually, but for many labels we can just use -1 (static) if wrappers allow, 
// or simple distinct IDs. Let's assume ControlBuilder can handle simple unique IDs if we increment.)

struct SettingsState {
    theme: AppTheme,
    result: Option<AppTheme>,
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
    skip_extensions: String,
    update_status: UpdateStatus,
    h_font_bold: HFONT, // Store bold font handle
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
        h_font_bold: std::ptr::null_mut(),
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
            // Apply DWM title bar color
            crate::ui::theme::set_window_frame_theme(hwnd, self.is_dark);

            // Create Bold Font
            // Get current message box font from a standard control or SystemParameters (simulation)
            // Simpler: use the default GUI font, get its LOGFONT, make it bold
            let h_default = GetStockObject(DEFAULT_GUI_FONT);
            let mut lf: LOGFONTW = std::mem::zeroed();
            GetObjectW(h_default, std::mem::size_of::<LOGFONTW>() as i32, &mut lf as *mut _ as *mut _);
            lf.lfWeight = FW_BOLD as i32;
            
            // Better approach for Fluent UI: Use Segoe UI explicitly if possible, or just trust the system font
            // We'll trust the modified default font for now.
            self.h_font_bold = CreateFontIndirectW(&lf);
            
            // Create Segoe MDL2 Assets font for flat icons
            let mut lf_icon: LOGFONTW = std::mem::zeroed();
            lf_icon.lfHeight = -16; // 16pt icon size
            lf_icon.lfWeight = 400; // Normal weight
            lf_icon.lfCharSet = 1; // DEFAULT_CHARSET
            let mdl2_name = "Segoe MDL2 Assets";
            for (i, c) in mdl2_name.encode_utf16().enumerate() {
                if i < 32 { lf_icon.lfFaceName[i] = c; }
            }
            let h_font_icon = CreateFontIndirectW(&lf_icon);

            let is_dark_mode = self.is_dark;
            
            // --- Layout state ---
            let mut current_y = 10;
            let row_height_base = 50;
            
            // --- Helper to create a Section Header (no icon, just bold text) ---
            let create_section_header = |y: i32, text: &str| -> i32 {
                let _h = ControlBuilder::new(hwnd, 0)
                    .label(false)
                    .text(text)
                    .pos(20, y)
                    .size(500, 25)
                    .dark_mode(is_dark_mode)
                    .font(self.h_font_bold)
                    .build();
                y + 30
            };
            
            // --- Helper to create a Row with MDL2 Icon ---
            let create_row = |y: i32, icon_glyph: &str, title: &str, subtitle: &str, _control_id: u16, ctl_fn: &dyn Fn(i32, i32) -> HWND| -> i32 {
                // Icon (using MDL2 Assets font)
                let _h_icon = ControlBuilder::new(hwnd, 0)
                    .label(false)
                    .text(icon_glyph)
                    .pos(30, y + 5)
                    .size(20, 20)
                    .dark_mode(is_dark_mode)
                    .font(h_font_icon)
                    .build();
                
                // Title (offset for icon)
                let _h_title = ControlBuilder::new(hwnd, 0)
                    .label(false)
                    .text(title)
                    .pos(55, y)
                    .size(280, 20)
                    .dark_mode(is_dark_mode)
                    .font(self.h_font_bold)
                    .build();
                
                // Subtitle
                let _h_sub = ControlBuilder::new(hwnd, 0)
                    .label(false)
                    .text(subtitle)
                    .pos(55, y + 20)
                    .size(330, 20)
                    .dark_mode(is_dark_mode)
                    .build();
                
                // Control (Right Aligned)
                ctl_fn(350, y);
                
                y + row_height_base
            };

            // --- Layout Content --- //

            // 1. Appearance Section
            current_y = create_section_header(current_y, "Appearance");
            
            // App Theme - MDL2 glyph E713 = Settings
            current_y = create_row(current_y, "\u{E713}", "Application Theme", "Choose between Light, Dark, or System Default", IDC_COMBO_THEME, &|x, y| {
                let h_combo = ControlBuilder::new(hwnd, IDC_COMBO_THEME)
                    .combobox()
                    .pos(x, y + 5) // Center vertically roughly
                    .size(150, 100)
                    .dark_mode(is_dark_mode)
                    .build();
                
                let cb = ComboBox::new(h_combo);
                cb.add_string("System Default");
                cb.add_string("Dark Mode");
                cb.add_string("Light Mode");
                
                let sel = match self.theme {
                    AppTheme::System => 0,
                    AppTheme::Dark => 1,
                    AppTheme::Light => 2,
                };
                cb.set_selected_index(sel);
                h_combo
            });
            
            current_y += 10; // Extra spacing

            // 2. Behavior Section
            current_y = create_section_header(current_y, "General Behavior");

            // Force Stop - MDL2 glyph E74D = Stop
            current_y = create_row(current_y, "\u{E74D}", "Force Kill Processes", "Automatically terminate locking processes", IDC_CHK_FORCE_STOP, &|x, y| {
                 ControlBuilder::new(hwnd, IDC_CHK_FORCE_STOP)
                    .checkbox()
                    .text("") // No text, just the box
                    .pos(x + 120, y + 5) // Rightmost
                    .size(20, 20)
                    .dark_mode(is_dark_mode)
                    .checked(self.enable_force_stop)
                    .build()
            });

            // Context Menu - MDL2 glyph E8DE = More
            current_y = create_row(current_y, "\u{E8DE}", "Explorer Context Menu", "Add 'CompactRS' to right-click menu", IDC_CHK_CONTEXT_MENU, &|x, y| {
                ControlBuilder::new(hwnd, IDC_CHK_CONTEXT_MENU)
                    .checkbox()
                    .text("")
                    .pos(x + 120, y + 5)
                    .size(20, 20)
                    .dark_mode(is_dark_mode)
                    .checked(self.enable_context_menu)
                    .build()
            });
            
            // System Guard - MDL2 glyph EA18 = Shield
            current_y = create_row(current_y, "\u{EA18}", "System Safety Guard", "Prevent compression of critical system files", IDC_CHK_SYSTEM_GUARD, &|x, y| {
                ControlBuilder::new(hwnd, IDC_CHK_SYSTEM_GUARD)
                    .checkbox()
                    .text("")
                    .pos(x + 120, y + 5)
                    .size(20, 20)
                    .dark_mode(is_dark_mode)
                    .checked(self.enable_system_guard)
                    .build()
            });
            
            // Low Power - MDL2 glyph EC48 = Battery Saver
            current_y = create_row(current_y, "\u{EC48}", "Efficiency Mode", "Reduce background resource usage (Low Power)", IDC_CHK_LOW_POWER, &|x, y| {
                ControlBuilder::new(hwnd, IDC_CHK_LOW_POWER)
                    .checkbox()
                    .text("")
                    .pos(x + 120, y + 5)
                    .size(20, 20)
                    .dark_mode(is_dark_mode)
                    .checked(self.low_power_mode)
                    .build()
            });

            current_y += 10; 

            // 3. Performance Section
            current_y = create_section_header(current_y, "Performance");
            
            // Threads
            let cpu_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as u32;
            let current_threads = if self.max_threads == 0 { cpu_count } else { self.max_threads };
            
            // CPU Threads - MDL2 glyph E9D9 = Processing
            current_y = create_row(current_y, "\u{E9D9}", "CPU Thread Limit", &format!("Maximum worker threads (Current: {})", current_threads), IDC_SLIDER_THREADS, &|x, y| {
                 // Label for value updates
                 let _lbl = ControlBuilder::new(hwnd, IDC_LBL_THREADS_VALUE)
                    .label(false)
                    .text("") // Handled by subtitle mostly, but we can put value here or just on slider tooltip? 
                              // Current subtitle has value. We'll update logic to update subtitle text if possible, or aux label.
                    .pos(0, 0).size(0,0).build(); // Invisible anchor if needed, or just update the one we passed?
                                                  // Simpler: Just put slider. We update subtitle logic later or repaint?
                    
                 let h_slider = ControlBuilder::new(hwnd, IDC_SLIDER_THREADS)
                    .trackbar()
                    .pos(x, y)
                    .size(140, 30)
                    .dark_mode(is_dark_mode)
                    .build();
                 Trackbar::new(h_slider).set_range(1, cpu_count);
                 Trackbar::new(h_slider).set_pos(current_threads);
                 h_slider
            });

            // Queue - MDL2 glyph E902 = List
            current_y = create_row(current_y, "\u{E902}", "Concurrent File Queue", "Files compressed simultaneously (0 = Unlimited)", IDC_EDIT_CONCURRENT, &|x, y| {
                 ControlBuilder::new(hwnd, IDC_EDIT_CONCURRENT)
                    .edit()
                    .text(&self.max_concurrent_items.to_string())
                    .pos(x + 90, y + 5)
                    .size(50, 20)
                    .style((windows_sys::Win32::UI::WindowsAndMessaging::ES_NUMBER | windows_sys::Win32::UI::WindowsAndMessaging::ES_CENTER) as u32)
                    .dark_mode(is_dark_mode)
                    .build()
            });
            
            current_y += 10;
            
            // 4. Filtering Section
            current_y = create_section_header(current_y, "File Filtering");
            
            // Skip Heuristics - MDL2 glyph E71C = Filter
            current_y = create_row(current_y, "\u{E71C}", "Smart Compression Skip", "Skip files that are unlikely to compress further", IDC_CHK_SKIP_EXT, &|x, y| {
                ControlBuilder::new(hwnd, IDC_CHK_SKIP_EXT)
                    .checkbox()
                    .text("")
                    .pos(x + 120, y + 5)
                    .size(20, 20)
                    .dark_mode(is_dark_mode)
                    .checked(self.enable_skip_heuristics)
                    .build()
            });
            
            // Extensions (Multi-line area below)
            let _h_lbl_ext = ControlBuilder::new(hwnd, 0)
                .label(false)
                .text("Excluded Extensions (comma separated):")
                .pos(30, current_y)
                .size(300, 20)
                .dark_mode(is_dark_mode)
                .font(self.h_font_bold) // Make this bold too? Or just subtitle style? Bold looks like header.
                // Let's keep it regular or use helper.
                .build();
            current_y += 25;
            
            let h_edit_ext = ControlBuilder::new(hwnd, IDC_EDIT_EXTENSIONS)
                .edit()
                .text(&self.skip_extensions)
                .pos(30, current_y)
                .size(480, 50)
                .style((windows_sys::Win32::UI::WindowsAndMessaging::ES_AUTOVSCROLL | windows_sys::Win32::UI::WindowsAndMessaging::ES_MULTILINE) as u32) 
                .dark_mode(is_dark_mode)
                .build();
                
            let h_btn_reset = ControlBuilder::new(hwnd, IDC_BTN_RESET_EXT)
                .button()
                .text_w(w!("Reset Defaults"))
                .pos(390, current_y + 55) // Below edit
                .size(120, 25)
                .dark_mode(is_dark_mode)
                .build();
            
            if !self.enable_skip_heuristics {
                Button::new(h_edit_ext).set_enabled(false);
                Button::new(h_btn_reset).set_enabled(false);
            }
            current_y += 90;

            // 5. Diagnostics
             current_y = create_section_header(current_y, "Diagnostics");
             
             // Logging Master - MDL2 glyph E9D9 = Bug
             current_y = create_row(current_y, "\u{EBE8}", "Enable Diagnostic Logging", "Show real-time logs in a console window", IDC_CHK_LOG_ENABLED, &|x, y| {
                 ControlBuilder::new(hwnd, IDC_CHK_LOG_ENABLED)
                    .checkbox()
                    .text("")
                    .pos(x + 120, y + 5)
                    .size(20, 20)
                    .dark_mode(is_dark_mode)
                    .checked(self.log_enabled)
                    .build()
             });
             
             // Log Levels (Horizontal or indented?)
             // Let's do a horizontal row of small checkboxes for levels if enabled
             let level_y = current_y - 10; // Pull up slightly or just next line
             let _lbl_lv = ControlBuilder::new(hwnd, 0).label(false).text("Levels:").pos(30, level_y).size(50, 20).dark_mode(is_dark_mode).build();
             
             let mk_chk = |id, txt, x, checked| {
                 ControlBuilder::new(hwnd, id)
                    .checkbox()
                    .text(txt)
                    .pos(x, level_y)
                    .size(100, 20)
                    .dark_mode(is_dark_mode)
                    .checked(checked)
                    .build()
             };
             
             mk_chk(IDC_CHK_LOG_ERRORS, "Errors", 80, self.log_level_mask & crate::logger::LOG_LEVEL_ERROR != 0);
             mk_chk(IDC_CHK_LOG_WARNS, "Warnings", 170, self.log_level_mask & crate::logger::LOG_LEVEL_WARN != 0);
             mk_chk(IDC_CHK_LOG_INFO, "Info", 260, self.log_level_mask & crate::logger::LOG_LEVEL_INFO != 0);
             mk_chk(IDC_CHK_LOG_TRACE, "Trace", 340, self.log_level_mask & crate::logger::LOG_LEVEL_TRACE != 0);
             
             current_y += 40;

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

            // 6. About & Updates
            current_y = create_section_header(current_y, "About & Updates");

            // MANUAL ROW for Updates (to bind IDC_LBL_UPDATE_STATUS to subtitle)
            // Title
            ControlBuilder::new(hwnd, 0)
                .label(false)
                .text("Application Version")
                .pos(30, current_y)
                .size(300, 20)
                .dark_mode(is_dark_mode)
                .font(self.h_font_bold)
                .build();
            
            // Subtitle (Dynamic Status)
            let version_str = format!("{} - Check for updates", env!("APP_VERSION"));
            ControlBuilder::new(hwnd, IDC_LBL_UPDATE_STATUS)
                .label(false)
                .text(&version_str)
                .pos(30, current_y + 20)
                .size(350, 20)
                .dark_mode(is_dark_mode)
                .build();
            
            // Check Update Button
            ControlBuilder::new(hwnd, IDC_BTN_CHECK_UPDATE)
                .button()
                .text_w(w!("Check for Updates"))
                .pos(400, current_y) // Right aligned
                .size(140, 30)
                .dark_mode(is_dark_mode)
                .build();
            
            current_y += 50;

            // Restart TI Row - MDL2 glyph E7EF = Admin
            current_y = create_row(current_y, "\u{E7EF}", "Advanced Startup", "Restart with TrustedInstaller privileges", IDC_BTN_RESTART_TI, &|_x_pos, y_pos| {
                 // Adjusted x_pos from helper is 350. Let's use 400 manually or stick to helper?
                 // Helper passes 350. 
                 ControlBuilder::new(hwnd, IDC_BTN_RESTART_TI)
                    .button()
                    .text_w(w!("Restart as TI"))
                    .pos(400, y_pos) // Align with above
                    .size(140, 30) // Match width
                    .dark_mode(is_dark_mode)
                    .build()
            });

            current_y += 30; // Spacing before close

            // Close Button (Bottom Right)
            ControlBuilder::new(hwnd, IDC_BTN_CANCEL)
                .button()
                .text_w(w!("Close"))
                .pos(440, current_y) // Far right
                .size(100, 30)
                .dark_mode(is_dark_mode)
                .build();
            
            // Resize Window to fit content
            let final_h = current_y + 80;
            use windows_sys::Win32::UI::WindowsAndMessaging::SetWindowPos;
            SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 580, final_h, windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOMOVE | windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOZORDER);

            // Apply recursively to catch any stragglers
            crate::ui::theme::apply_theme_recursive(hwnd, self.is_dark);
        }
        0
    }

    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            match msg {
                windows_sys::Win32::UI::WindowsAndMessaging::WM_DESTROY => {
                    if self.h_font_bold != std::ptr::null_mut() {
                        DeleteObject(self.h_font_bold);
                        self.h_font_bold = std::ptr::null_mut();
                    }
                    Some(0)
                },
                WM_HSCROLL => {
                     // Check if it's our slider
                     let h_ctl = lparam as HWND;
                     let h_slider = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_SLIDER_THREADS as i32);
                     if h_ctl == h_slider {
                         // Get Position
                         let pos = Trackbar::new(h_slider).get_pos();
                         self.max_threads = pos;
                         
                         // Update Subtitle/Label? Use a dedicated label for now if it exists, or update the main subtitle if we stored it?
                         // We didn't store the subtitle handle. 
                         // But we have `IDC_LBL_THREADS_VALUE` (2015) in the code.
                         // Let's use that if we can.
                         let _label_text = format!("(Current: {})", pos); // Simplified
                         // Wait, in on_create I didn't assign ID 2015 to the Subtitle, I assigned it to a hidden label?
                         // I should probably find the control.
                         // Actually, let's just ignore the real-time subtitle update for now unless we need it perfect. 
                         // Or, find IDC_LBL_THREADS_VALUE:
                         let _h_lbl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_LBL_THREADS_VALUE as i32);
                         // If I commented it out in on_create, this won't work.
                         // Let's rely on standard trackbar tooltip if available, 
                         // or user just sees it when they open it.
                         // For now, let's keep the backend logic.
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
                         IDC_COMBO_THEME => {
                            if (code as u32) == CBN_SELCHANGE {
                                let h_combo = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_COMBO_THEME as i32);
                                let idx = ComboBox::new(h_combo).get_selected_index();
                                let theme = match idx {
                                    0 => AppTheme::System,
                                    1 => AppTheme::Dark,
                                    2 => AppTheme::Light,
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
                                
                                // Update Settings window title bar
                                crate::ui::theme::set_window_frame_theme(hwnd, new_is_dark);
                                
                                // Update controls theme
                                crate::ui::theme::apply_theme_recursive(hwnd, new_is_dark);
                                
                                // Repaint
                                InvalidateRect(hwnd, std::ptr::null(), 1);
                                
                                // Notify Parent Immediately
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
                                
                                // Broadcast to About/Console (Same as before)
                                let compactrs_about = crate::utils::to_wstring("CompactRS_About");
                                let about_hwnd = FindWindowW(compactrs_about.as_ptr(), std::ptr::null());
                                if about_hwnd != std::ptr::null_mut() {
                                    let is_dark_val = if new_is_dark { 1 } else { 0 };
                                    SendMessageW(about_hwnd, 0x8000 + 2, is_dark_val as WPARAM, 0);
                                }
                                
                                let compactrs_console = crate::w!("CompactRS_Console");
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
