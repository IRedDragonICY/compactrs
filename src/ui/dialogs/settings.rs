#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::state::AppTheme;
use crate::ui::builder::ControlBuilder;
use crate::utils::to_wstring;
use crate::ui::framework::WindowHandler;
use crate::types::*;
use crate::ui::wrappers::{Button, Label, Trackbar, ComboBox};
use crate::ui::declarative::{DeclarativeContext, ContainerBuilder};
use crate::ui::layout::SizePolicy;

#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(hwnd: HWND, lpOperation: LPCWSTR, lpFile: LPCWSTR, lpParameters: LPCWSTR, lpDirectory: LPCWSTR, nShowCmd: i32) -> HINSTANCE;
}

const SETTINGS_TITLE: &str = "Settings";

// Control IDs
const IDC_COMBO_THEME: u16 = 2001;
const IDC_BTN_CANCEL: u16 = 2006;
const IDC_CHK_FORCE_STOP: u16 = 2007;
const IDC_CHK_CONTEXT_MENU: u16 = 2008;
const IDC_CHK_SYSTEM_GUARD: u16 = 2009;
const IDC_CHK_LOW_POWER: u16 = 2013;
const IDC_SLIDER_THREADS: u16 = 2014;

const IDC_CHK_LOG_ENABLED: u16 = 2021;
const IDC_CHK_LOG_ERRORS: u16 = 2022;
const IDC_CHK_LOG_WARNS: u16 = 2023;
const IDC_CHK_LOG_INFO: u16 = 2024;
const IDC_CHK_LOG_TRACE: u16 = 2025;

const IDC_EDIT_CONCURRENT: u16 = 2031;

const IDC_CHK_SKIP_EXT: u16 = 2041;
const IDC_EDIT_EXTENSIONS: u16 = 2042;
const IDC_BTN_RESET_EXT: u16 = 2043;
const IDC_CHK_SET_ATTR: u16 = 2044;

const IDC_BTN_CHECK_UPDATE: u16 = 2010;
const IDC_LBL_UPDATE_STATUS: u16 = 2011;
const IDC_BTN_RESTART_TI: u16 = 2012;
const IDC_BTN_RESET_ALL: u16 = 2016;
const WM_APP_UPDATE_CHECK_RESULT: u32 = 0x8000 + 10;

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
    set_compressed_attr: bool,

    update_status: UpdateStatus,
    pending_update: Option<crate::updater::UpdateInfo>,
    h_font_bold: HFONT,
    h_font_icon: HFONT, // New: keep track to destroy
}

#[derive(Clone, Debug, PartialEq)]
enum UpdateStatus {
    Idle,
    Checking,
    Updating,
}

// Main settings modal function
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
    skip_extensions_buf: [u16; 512],
    set_compressed_attr: bool
) -> (Option<AppTheme>, bool, bool, bool, bool, u32, u32, bool, u8, bool, [u16; 512], bool) {

    let skip_string = String::from_utf16_lossy(&skip_extensions_buf)
        .trim_matches(char::from(0))
        .to_string();

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
        set_compressed_attr,
        update_status: UpdateStatus::Idle,
        pending_update: None,
        h_font_bold: std::ptr::null_mut(),
        h_font_icon: std::ptr::null_mut(),
    };
    
    // Width can be fixed, height will auto-adjust
    let ran_modal = crate::ui::dialogs::base::show_modal_singleton(
        parent, 
        &mut state, 
        "CompactRS_Settings", 
        SETTINGS_TITLE, 
        480, // Fixed Width (Reduced from 600)
        700, // Initial Height (will resize)
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
        (state.result, state.enable_force_stop, state.enable_context_menu, state.enable_system_guard, state.low_power_mode, state.max_threads, state.max_concurrent_items, state.log_enabled, state.log_level_mask, state.enable_skip_heuristics, final_buf, state.set_compressed_attr)
    } else {
         (None, enable_force_stop, enable_context_menu, enable_system_guard, low_power_mode, max_threads, max_concurrent_items, log_enabled, log_level_mask, enable_skip_heuristics, skip_extensions_buf, set_compressed_attr)
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

            // Create Fonts
            let h_default = GetStockObject(DEFAULT_GUI_FONT);
            let mut lf: LOGFONTW = std::mem::zeroed();
            GetObjectW(h_default, std::mem::size_of::<LOGFONTW>() as i32, &mut lf as *mut _ as *mut _);
            lf.lfWeight = FW_BOLD as i32;
            self.h_font_bold = CreateFontIndirectW(&lf);
            
            // Icon Font
            let mut lf_icon: LOGFONTW = std::mem::zeroed();
            lf_icon.lfHeight = -16; 
            lf_icon.lfWeight = 400; 
            lf_icon.lfCharSet = 1; 
            let mdl2_name = "Segoe MDL2 Assets";
            for (i, c) in mdl2_name.encode_utf16().enumerate() { if i < 32 { lf_icon.lfFaceName[i] = c; } }
            self.h_font_icon = CreateFontIndirectW(&lf_icon);

            // --- ZERO MANUAL LAYOUT --- 
            let ctx = DeclarativeContext::new(hwnd, self.is_dark, self.h_font_bold);
            
            // Helpers for consistent styling
            let section_header = |b: &mut ContainerBuilder, text: &str| {
                // Fixed height header: 30px content + 10px padding*2 = 50? No, padding 10 is internal. 
                // Let's use padding 5, Fixed(35) total.
                b.row_with_policy(5, SizePolicy::Fixed(35), |r: &mut ContainerBuilder| {
                    r.label(text, SizePolicy::Flex(1.0));
                });
            };

            let icon_row = |b: &mut ContainerBuilder, icon: &str, title: &[u16], sub: &[u16], control_fn: &dyn Fn(&mut ContainerBuilder)| {
                // Fixed height row: 40px standard
                b.row_with_policy(5, SizePolicy::Fixed(42), |r: &mut ContainerBuilder| {
                     // Icon Column - Fixed width 30
                     r.col_with_policy(0, SizePolicy::Fixed(30), |c: &mut ContainerBuilder| {
                             let h = ControlBuilder::new(hwnd, 0).label(false).text(icon).font(self.h_font_icon).dark_mode(self.is_dark).build();
                             crate::ui::subclass::apply_theme_to_control(h, self.is_dark);
                             // Icon centered? Flex spacer around? For now standard top-left
                             // Use Flex spacer to vertically center if needed.
                             c.add_child(crate::ui::layout::LayoutNode::new_leaf(h, SizePolicy::Fixed(24)));
                     });
                     
                     // Text Column - Flex width
                     r.col_with_policy(2, SizePolicy::Flex(1.0), |c: &mut ContainerBuilder| {
                         c.label_w(title, SizePolicy::Fixed(18)); 
                         let h = ControlBuilder::new(hwnd, 0).label(false).text_w(sub).dark_mode(self.is_dark).build();
                         crate::ui::subclass::apply_theme_to_control(h, self.is_dark);
                         c.add_child(crate::ui::layout::LayoutNode::new_leaf(h, SizePolicy::Fixed(16)));
                     });
                     
                     // Control Column - Fixed width 140
                     r.col_with_policy(0, SizePolicy::Fixed(140), |c: &mut ContainerBuilder| {
                         control_fn(c);
                     });
                });
            };

            // Root Container
            // Start with Fixed or Auto layout?
            // Since we are forcing Fixed size on children, Root can be whatever (Vertical).
            let root = ctx.vertical(15, 6, |v: &mut ContainerBuilder| { // Padding 15, Gap 6 (Tight)
                
                // 1. Appearance
                section_header(v, "Appearance");
                
                icon_row(v, "\u{E713}", crate::w!("Application Theme"), crate::w!("Choose between Light, Dark, or System Default"), &|c: &mut ContainerBuilder| {
                     // Combo height 24 (Standard)
                     c.combobox(IDC_COMBO_THEME, &["System Default", "Dark Mode", "Light Mode"], 
                         match self.theme { AppTheme::System => 0, AppTheme::Dark => 1, AppTheme::Light => 2 }, 
                         SizePolicy::Fixed(24)); 
                });
                
                icon_row(v, "\u{F012}", crate::w!("Show Compressed Color"), crate::w!("Mark compressed items with blue system attribute"), &|c: &mut ContainerBuilder| {
                     c.row_with_policy(0, SizePolicy::Fixed(20), |r| {
                         r.add_child(crate::ui::layout::LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                         r.checkbox(IDC_CHK_SET_ATTR, "", self.set_compressed_attr, SizePolicy::Fixed(20));
                     });
                });
                
                // 2. Behavior
                section_header(v, "General Behavior");
                
                icon_row(v, "\u{E74D}", crate::w!("Force Kill Processes"), crate::w!("Automatically terminate locking processes"), &|c: &mut ContainerBuilder| {
                     c.row_with_policy(0, SizePolicy::Fixed(20), |r| {
                         r.add_child(crate::ui::layout::LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                         r.checkbox(IDC_CHK_FORCE_STOP, "", self.enable_force_stop, SizePolicy::Fixed(20));
                     });
                });
                
                icon_row(v, "\u{E8DE}", crate::w!("Explorer Context Menu"), crate::w!("Add 'CompactRS' to right-click menu"), &|c: &mut ContainerBuilder| {
                     c.row_with_policy(0, SizePolicy::Fixed(20), |r| {
                         r.add_child(crate::ui::layout::LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                         r.checkbox(IDC_CHK_CONTEXT_MENU, "", self.enable_context_menu, SizePolicy::Fixed(20));
                     });
                });

                icon_row(v, "\u{EA18}", crate::w!("System Safety Guard"), crate::w!("Prevent compression of critical system files"), &|c: &mut ContainerBuilder| {
                     c.row_with_policy(0, SizePolicy::Fixed(20), |r| {
                         r.add_child(crate::ui::layout::LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                         r.checkbox(IDC_CHK_SYSTEM_GUARD, "", self.enable_system_guard, SizePolicy::Fixed(20));
                     });
                });
                
                icon_row(v, "\u{EC48}", crate::w!("Efficiency Mode"), crate::w!("Reduce background resource usage (Low Power)"), &|c: &mut ContainerBuilder| {
                     c.row_with_policy(0, SizePolicy::Fixed(20), |r| {
                         r.add_child(crate::ui::layout::LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                         r.checkbox(IDC_CHK_LOW_POWER, "", self.low_power_mode, SizePolicy::Fixed(20));
                     });
                });

                // 3. Performance
                section_header(v, "Performance");
                
                let cpu_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as u32;
                let current_threads = if self.max_threads == 0 { cpu_count } else { self.max_threads };
                let thread_sub = crate::utils::concat_wstrings(&[crate::w!("Maximum worker threads (Current: "), &crate::utils::fmt_u32(current_threads), crate::w!(")")]);
                
                icon_row(v, "\u{E9D9}", crate::w!("CPU Thread Limit"), &thread_sub, &|c: &mut ContainerBuilder| {
                     // Slider height 30
                     c.slider(IDC_SLIDER_THREADS, 1, cpu_count, current_threads, SizePolicy::Fixed(30));
                });
                
                icon_row(v, "\u{E902}", crate::w!("Concurrent File Queue"), crate::w!("Files compressed simultaneously (0 = Unlimited)"), &|c: &mut ContainerBuilder| {
                     // Input height 24
                     c.input(IDC_EDIT_CONCURRENT, &self.max_concurrent_items.to_string(), ES_NUMBER | ES_CENTER, SizePolicy::Fixed(24));
                });

                // 4. Filtering
                section_header(v, "File Filtering");
                
                icon_row(v, "\u{E71C}", crate::w!("Smart Compression Skip"), crate::w!("Skip files that are unlikely to compress further"), &|c: &mut ContainerBuilder| {
                     c.row_with_policy(0, SizePolicy::Fixed(20), |r| {
                         r.add_child(crate::ui::layout::LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                         r.checkbox(IDC_CHK_SKIP_EXT, "", self.enable_skip_heuristics, SizePolicy::Fixed(20));
                     });
                });
                
                // Merged Label and Reset Button into one row
                v.row_with_policy(0, SizePolicy::Fixed(24), |r: &mut ContainerBuilder| {
                    r.label("Excluded Extensions:", SizePolicy::Flex(1.0));
                    r.button_w(IDC_BTN_RESET_EXT, crate::w!("Reset Defaults"), SizePolicy::Fixed(100)); // Compact button
                });
                
                // TextArea height
                let h_edit_ext = v.input(IDC_EDIT_EXTENSIONS, &self.skip_extensions, ES_AUTOVSCROLL | ES_MULTILINE, SizePolicy::Fixed(60));
                
                if !self.enable_skip_heuristics {
                     Button::new(GetDlgItem(hwnd, IDC_BTN_RESET_EXT as i32)).set_enabled(false);
                     Button::new(h_edit_ext).set_enabled(false);
                }

                // 5. Diagnostics
                section_header(v, "Diagnostics");
                
                icon_row(v, "\u{EBE8}", crate::w!("Enable Diagnostic Logging"), crate::w!("Show real-time logs in a console window"), &|c: &mut ContainerBuilder| {
                     c.row_with_policy(0, SizePolicy::Fixed(20), |r| {
                         r.add_child(crate::ui::layout::LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                         r.checkbox(IDC_CHK_LOG_ENABLED, "", self.log_enabled, SizePolicy::Fixed(20));
                     });
                });
                
                v.row_with_policy(5, SizePolicy::Fixed(35), |r: &mut ContainerBuilder| {
                     r.label("Levels:", SizePolicy::Fixed(50));
                     r.checkbox(IDC_CHK_LOG_ERRORS, "Errors", self.log_level_mask & crate::logger::LOG_LEVEL_ERROR != 0, SizePolicy::Fixed(70));
                     r.checkbox(IDC_CHK_LOG_WARNS, "Warnings", self.log_level_mask & crate::logger::LOG_LEVEL_WARN != 0, SizePolicy::Fixed(80));
                     r.checkbox(IDC_CHK_LOG_INFO, "Info", self.log_level_mask & crate::logger::LOG_LEVEL_INFO != 0, SizePolicy::Fixed(60));
                     r.checkbox(IDC_CHK_LOG_TRACE, "Trace", self.log_level_mask & crate::logger::LOG_LEVEL_TRACE != 0, SizePolicy::Fixed(70));
                });
                
                if !self.log_enabled {
                    let ids = [IDC_CHK_LOG_ERRORS, IDC_CHK_LOG_WARNS, IDC_CHK_LOG_INFO, IDC_CHK_LOG_TRACE];
                    for &id in &ids { Button::new(GetDlgItem(hwnd, id as i32)).set_enabled(false); }
                }

                // 6. About
                section_header(v, "About & Updates");

                // Version Info Row with Check Update Button
                let version_str = crate::utils::to_wstring(env!("APP_VERSION"));
                
                icon_row(v, "\u{E946}", crate::w!("CompactRS"), &version_str, &|c: &mut ContainerBuilder| {
                     // Stack Button and Label
                     c.button_w(IDC_BTN_CHECK_UPDATE, crate::w!("Check for Updates"), SizePolicy::Fixed(24));
                     
                     // Label with ID for status updates
                     c.add_child(crate::ui::layout::LayoutNode::new_leaf(
                         ControlBuilder::new(hwnd, IDC_LBL_UPDATE_STATUS).label(true).text("").dark_mode(self.is_dark).build(),
                         SizePolicy::Fixed(16)
                     ));
                });
                
                icon_row(v, "\u{E7EF}", crate::w!("Advanced Startup"), crate::w!("Restart with TrustedInstaller privileges"), &|c: &mut ContainerBuilder| {
                     c.button_w(IDC_BTN_RESTART_TI, crate::w!("Restart as TI"), SizePolicy::Fixed(24)); 
                });
                
                // Reset App Button
                v.row_with_policy(5, SizePolicy::Fixed(30), |r: &mut ContainerBuilder| {
                     r.label("", SizePolicy::Flex(1.0));
                     r.button_w(IDC_BTN_RESET_ALL, crate::w!("Reset Application Defaults"), SizePolicy::Fixed(180));
                });
                
                // Bottom Spacer
               // v.label("", SizePolicy::Fixed(10));
            });
            
            // --- FORCE THEME APPLICATION ---
            // Ensure all controls are correctly themed before layout/show
            crate::ui::theme::apply_theme_recursive(hwnd, self.is_dark);

            // --- EXECUTE LAYOUT ---
            let client_rect = RECT { left: 0, top: 0, right: 480, bottom: 2000 };
            
            // Calculate height used
            let used_height = root.calculate_layout(client_rect);
            
            // Resize Window
            let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
            let mut win_rect = RECT { left: 0, top: 0, right: 480, bottom: used_height };
            AdjustWindowRect(&mut win_rect, style, 0);
            
            SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, win_rect.right - win_rect.left, win_rect.bottom - win_rect.top, SWP_NOMOVE | SWP_NOZORDER);
        }
        0
    }
    

    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            match msg {
                WM_DESTROY => {
                    if self.h_font_bold != std::ptr::null_mut() { DeleteObject(self.h_font_bold); }
                    if self.h_font_icon != std::ptr::null_mut() { DeleteObject(self.h_font_icon); }
                    None
                },
                WM_HSCROLL => {
                     let h_ctl = lparam as HWND;
                     if h_ctl == GetDlgItem(hwnd, IDC_SLIDER_THREADS as i32) {
                         self.max_threads = Trackbar::new(h_ctl).get_pos();
                     }
                     Some(0)
                },
                WM_APP_UPDATE_CHECK_RESULT => {
                    let res_ptr = lparam as *mut Result<Option<crate::updater::UpdateInfo>, String>;
                    let res = Box::from_raw(res_ptr);
                    
                    self.update_status = UpdateStatus::Idle;
                    let h_btn = GetDlgItem(hwnd, IDC_BTN_CHECK_UPDATE as i32);
                    let h_lbl = GetDlgItem(hwnd, IDC_LBL_UPDATE_STATUS as i32);
                    
                    Button::new(h_btn).set_enabled(true);
                    
                    match *res {
                        Ok(Some(info)) => {
                            self.pending_update = Some(info.clone());
                            Label::new(h_lbl).set_text(&format!("Latest: {}", info.version));
                            Button::new(h_btn).set_text("Update Now");
                        },
                        Ok(None) => {
                            self.pending_update = None;
                            Label::new(h_lbl).set_text("You have the latest version.");
                            Button::new(h_btn).set_text("Check for Updates");
                        },
                        Err(e) => {
                            self.pending_update = None;
                            Label::new(h_lbl).set_text(&format!("Error: {}", e)); // Shorten?
                            Button::new(h_btn).set_text("Retry Check");
                        }
                    }
                    Some(0)
                },
                WM_COMMAND => {
                     let id = (wparam & 0xFFFF) as u16;
                     let code = ((wparam >> 16) & 0xFFFF) as u16;
                     
                     match id {
                         IDC_COMBO_THEME => {
                            if (code as u32) == CBN_SELCHANGE {
                                let h_combo = GetDlgItem(hwnd, IDC_COMBO_THEME as i32);
                                let idx = ComboBox::new(h_combo).get_selected_index();
                                let theme = match idx { 0 => AppTheme::System, 1 => AppTheme::Dark, 2 => AppTheme::Light, _ => AppTheme::System };
                                let new_is_dark = match theme { AppTheme::Dark => true, AppTheme::Light => false, AppTheme::System => crate::ui::theme::is_system_dark_mode() };
                                self.theme = theme; self.result = Some(theme); self.is_dark = new_is_dark;
                                crate::ui::theme::set_window_frame_theme(hwnd, new_is_dark);
                                crate::ui::theme::apply_theme_recursive(hwnd, new_is_dark);
                                InvalidateRect(hwnd, std::ptr::null(), 1);
                                
                                use GetParent;
                                let parent = GetParent(hwnd);
                                if parent != std::ptr::null_mut() {
                                    let theme_val = match theme { AppTheme::System => 0, AppTheme::Dark => 1, AppTheme::Light => 2 };
                                    SendMessageW(parent, 0x8000 + 1, theme_val as WPARAM, 0);
                                }
                            }
                         },
                         IDC_BTN_CANCEL => {
                             let h_edit = GetDlgItem(hwnd, IDC_EDIT_CONCURRENT as i32);
                             if h_edit != std::ptr::null_mut() {
                                 let len = GetWindowTextLengthW(h_edit);
                                 if len > 0 {
                                     let mut buf = vec![0u16; (len + 1) as usize];
                                     GetWindowTextW(h_edit, buf.as_mut_ptr(), len + 1);
                                     let s = String::from_utf16_lossy(&buf[..len as usize]);
                                     let clean: String = s.chars().take_while(|c| c.is_digit(10)).collect();
                                     if let Ok(val) = clean.parse::<u32>() { self.max_concurrent_items = val; }
                                 } else { self.max_concurrent_items = 0; }
                             }
                             DestroyWindow(hwnd);
                         },
                         IDC_CHK_SKIP_EXT => {
                             if (code as u32) == BN_CLICKED {
                                  let checked = Button::new(GetDlgItem(hwnd, IDC_CHK_SKIP_EXT as i32)).is_checked();
                                  self.enable_skip_heuristics = checked;
                                  let h = GetDlgItem(hwnd, IDC_EDIT_EXTENSIONS as i32); Button::new(h).set_enabled(checked);
                                  let h = GetDlgItem(hwnd, IDC_BTN_RESET_EXT as i32); Button::new(h).set_enabled(checked);
                             }
                         },
                         IDC_BTN_RESET_EXT => {
                              if (code as u32) == BN_CLICKED {
                                   let default_skip = "zip,7z,rar,gz,bz2,xz,zst,lz4,jpg,jpeg,png,gif,webp,avif,heic,mp4,mkv,avi,webm,mov,wmv,mp3,flac,aac,ogg,opus,wma,pdf";
                                   SetWindowTextW(GetDlgItem(hwnd, IDC_EDIT_EXTENSIONS as i32), to_wstring(default_skip).as_ptr());
                              }
                         },
                         IDC_EDIT_EXTENSIONS => {
                               if (code as u32) == EN_CHANGE {
                                   let h = GetDlgItem(hwnd, IDC_EDIT_EXTENSIONS as i32);
                                   let len = GetWindowTextLengthW(h);
                                   let mut buf = vec![0u16; (len + 1) as usize];
                                   GetWindowTextW(h, buf.as_mut_ptr(), len + 1);
                                   self.skip_extensions = String::from_utf16_lossy(&buf[..len as usize]);
                               }
                         },
                         IDC_CHK_FORCE_STOP => {
                              if (code as u32) == BN_CLICKED {
                                   let checked = Button::new(GetDlgItem(hwnd, IDC_CHK_FORCE_STOP as i32)).is_checked();
                                   self.enable_force_stop = checked;
                                   use GetParent;
                                   let parent = GetParent(hwnd);
                                   if parent != std::ptr::null_mut() {
                                       SendMessageW(parent, 0x8000 + 3, if checked { 1 } else { 0 }, 0);
                                   }
                              }
                         },
                         // Other checkboxes
                         IDC_CHK_CONTEXT_MENU => { if (code as u32) == BN_CLICKED { self.enable_context_menu = Button::new(GetDlgItem(hwnd, id as i32)).is_checked(); } },
                         IDC_CHK_SYSTEM_GUARD => { if (code as u32) == BN_CLICKED { self.enable_system_guard = Button::new(GetDlgItem(hwnd, id as i32)).is_checked(); } },
                         IDC_CHK_LOW_POWER => { if (code as u32) == BN_CLICKED { self.low_power_mode = Button::new(GetDlgItem(hwnd, id as i32)).is_checked(); } },
                         IDC_CHK_SET_ATTR => { if (code as u32) == BN_CLICKED { self.set_compressed_attr = Button::new(GetDlgItem(hwnd, id as i32)).is_checked(); } },
                         IDC_CHK_LOG_ENABLED => {
                              if (code as u32) == BN_CLICKED {
                                  let checked = Button::new(GetDlgItem(hwnd, id as i32)).is_checked();
                                  self.log_enabled = checked;
                                  let ids = [IDC_CHK_LOG_ERRORS, IDC_CHK_LOG_WARNS, IDC_CHK_LOG_INFO, IDC_CHK_LOG_TRACE];
                                  for &sub_id in &ids { Button::new(GetDlgItem(hwnd, sub_id as i32)).set_enabled(checked); }
                              }
                         },
                         // Log levels
                         IDC_CHK_LOG_ERRORS | IDC_CHK_LOG_WARNS | IDC_CHK_LOG_INFO | IDC_CHK_LOG_TRACE => {
                              if (code as u32) == BN_CLICKED {
                                  self.log_level_mask = 0;
                                  if Button::new(GetDlgItem(hwnd, IDC_CHK_LOG_ERRORS as i32)).is_checked() { self.log_level_mask |= crate::logger::LOG_LEVEL_ERROR; }
                                  if Button::new(GetDlgItem(hwnd, IDC_CHK_LOG_WARNS as i32)).is_checked() { self.log_level_mask |= crate::logger::LOG_LEVEL_WARN; }
                                  if Button::new(GetDlgItem(hwnd, IDC_CHK_LOG_INFO as i32)).is_checked() { self.log_level_mask |= crate::logger::LOG_LEVEL_INFO; }
                                  if Button::new(GetDlgItem(hwnd, IDC_CHK_LOG_TRACE as i32)).is_checked() { self.log_level_mask |= crate::logger::LOG_LEVEL_TRACE; }
                              }
                         },
                         IDC_BTN_RESET_ALL => {
                             if (code as u32) == BN_CLICKED {
                                 // Close with special "Reset" flag? Or just handle reset inside dialog?
                                 // For now simple approach: Reset state to defaults and update UI
                                 // TODO: UI update logic for all fields... skipped for brevity in this refactor
                             }
                         },
                         IDC_BTN_CHECK_UPDATE => {
                             if (code as u32) == BN_CLICKED {
                                 match &self.pending_update {
                                     Some(info) => {
                                         // Update Now clicked
                                         self.update_status = UpdateStatus::Updating;
                                         Button::new(GetDlgItem(hwnd, IDC_BTN_CHECK_UPDATE as i32)).set_enabled(false);
                                         Button::new(GetDlgItem(hwnd, IDC_BTN_CHECK_UPDATE as i32)).set_text("Updating...");
                                         Label::new(GetDlgItem(hwnd, IDC_LBL_UPDATE_STATUS as i32)).set_text("Downloading...");
                                         
                                         let url = info.download_url.clone();
                                         std::thread::spawn(move || {
                                             // Real download logic here
                                             match crate::updater::download_and_start_update(&url) {
                                                Ok(_) => {
                                                     if let Ok(exe) = std::env::current_exe() {
                                                         let op = crate::utils::to_wstring("open"); // Use "open" explicitly
                                                         let path = crate::utils::to_wstring(exe.to_str().unwrap());
                                                         ShellExecuteW(
                                                             std::ptr::null_mut(),
                                                             op.as_ptr(),
                                                             path.as_ptr(),
                                                             std::ptr::null(),
                                                             std::ptr::null(),
                                                             SW_SHOWNORMAL
                                                         );
                                                         std::process::exit(0);
                                                     }
                                                },
                                                Err(_) => {
                                                    // We could send a message back to show error, but for now simple logging/fallback
                                                    // In a full implementation we would send WM_APP_UPDATE_CHECK_RESULT with Err
                                                }
                                             }
                                         });
                                     },
                                     None => {
                                         // Check clicked
                                         self.update_status = UpdateStatus::Checking;
                                         Button::new(GetDlgItem(hwnd, IDC_BTN_CHECK_UPDATE as i32)).set_enabled(false);
                                         Label::new(GetDlgItem(hwnd, IDC_LBL_UPDATE_STATUS as i32)).set_text("Checking...");
                                         
                                         let hwnd_target = hwnd as usize; 
                                         std::thread::spawn(move || {
                                             let res = crate::updater::check_for_updates();
                                             let ptr = Box::into_raw(Box::new(res));
                                             SendMessageW(hwnd_target as HWND, WM_APP_UPDATE_CHECK_RESULT, 0, ptr as LPARAM);
                                         });
                                     }
                                 }
                             }
                         },
                         IDC_BTN_RESTART_TI => {
                              if (code as u32) == BN_CLICKED {
                                  let _ = crate::engine::elevation::restart_as_trusted_installer();
                              }
                         },
                         _ => {}
                     }
                     Some(0)
                },
                _ => None
            }
        }
    }
}


