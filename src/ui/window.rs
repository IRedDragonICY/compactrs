/* --- src/ui/window.rs --- */
#![allow(unsafe_op_in_unsafe_fn, non_snake_case)]

use crate::types::*;
use std::sync::atomic::Ordering;



use crate::ui::controls::*;
use crate::ui::wrappers::{Button, ComboBox, Label, ProgressBar};
use crate::ui::state::{AppState, Controls, UiMessage, BatchStatus, AppTheme, BatchAction, ProcessingState};
use crate::ui::taskbar::{TaskbarProgress, TaskbarState};
use crate::ui::components::{
    Component, FileListView, StatusBar, StatusBarIds, ActionPanel, ActionPanelIds,
    HeaderPanel, HeaderPanelIds, SearchPanel, SearchPanelIds,
};
use crate::ui::theme;
use crate::ui::handlers; 
use crate::engine::wof::{WofAlgorithm, CompressionState};
use crate::utils::{to_wstring, u64_to_wstring, concat_wstrings, format_size};
use crate::w;
use crate::ui::framework::{WindowHandler, WindowBuilder, WindowAlignment, load_app_icon};
use crate::config::AppConfig;
// use crate::engine::dynamic_import; // Removed

const WINDOW_CLASS_NAME: &str = "CompactRS_Class";
const WINDOW_TITLE: &str = "CompactRS";

pub unsafe fn create_main_window(instance: HINSTANCE) -> Result<HWND, String> {
    unsafe {
        // Initialize Dynamic Imports removed
        // Dynamic Import init removed



        // Enable dark mode for the application
        theme::set_preferred_app_mode(true);

        // Initialize Common Controls
        let iccex = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_WIN95_CLASSES | ICC_STANDARD_CLASSES,
        };
        crate::types::InitCommonControlsEx(&iccex as *const _ as *const _);

        // Check dark mode
        let is_dark = theme::is_system_dark_mode();
        let bg_brush = theme::get_background_brush(is_dark);

        // Load icon
        let icon = load_app_icon(instance);

        let mut title_str = WINDOW_TITLE.to_string();
        if crate::engine::elevation::is_system_or_ti() {
            title_str.push_str(" [TrustedInstaller]");
        } else if crate::is_admin() {
            title_str.push_str(" [Administrator]");
        }
        
        let config = AppConfig::load();
        let (win_x, win_y) = if config.window_x < 0 || config.window_y < 0 {
            (CW_USEDEFAULT, CW_USEDEFAULT)
        } else {
            (config.window_x, config.window_y)
        };
        let win_width = if config.window_width > 0 { config.window_width } else { 900 };
        let win_height = if config.window_height > 0 { config.window_height } else { 600 };
        
        // Setup State
        // Main window state must live for the app lifetime.
        // We use Box::leak (conceptually similar to the previous manual pointer management).
        let mut state = Box::new(AppState::new());
        // Load watcher tasks
        let loaded_tasks = crate::watcher_config::WatcherConfig::load();
        if loaded_tasks.len() > 0 {
             state.watcher_tasks = std::sync::Arc::new(std::sync::Mutex::new(loaded_tasks));
        }

        // Start Watcher Thread
        let watcher_tasks_ref = state.watcher_tasks.clone();
        let watcher_tx = state.tx.clone();
        // We spawn it detached, global state management handles exit
        std::thread::spawn(move || {
            crate::engine::watcher::start_watcher_thread(watcher_tasks_ref, watcher_tx);
        });

        let state_ref = Box::leak(state);

        let hwnd = WindowBuilder::new(state_ref, WINDOW_CLASS_NAME, &title_str)
            .style(WS_OVERLAPPEDWINDOW | WS_VISIBLE)
            .size(win_width, win_height)
            .align(WindowAlignment::Manual(win_x, win_y))
            .icon(icon)
            .background(bg_brush)
            .build(std::ptr::null_mut())?;

        // Hostile Takeover: Force window to foreground (Bypass ASLR/Focus restrictions)
        // Hostile Takeover: Force window to foreground (Bypass ASLR/Focus restrictions)
        let foreground_hwnd = crate::types::GetForegroundWindow();
        if !foreground_hwnd.is_null() {
            let foreground_thread = crate::types::GetWindowThreadProcessId(foreground_hwnd, std::ptr::null_mut());
            let current_thread = crate::types::GetCurrentThreadId();
            
            if foreground_thread != current_thread {
                crate::types::AttachThreadInput(foreground_thread, current_thread, TRUE);
                crate::types::BringWindowToTop(hwnd);
                crate::types::SetForegroundWindow(hwnd);
                crate::types::AttachThreadInput(foreground_thread, current_thread, FALSE);
            } else {
                crate::types::SetForegroundWindow(hwnd);
            }
        } else {
            crate::types::SetForegroundWindow(hwnd);
        }

        Ok(hwnd)
    }
}

unsafe fn flash_window(hwnd: HWND) {
    let mut fwi = FLASHWINFO {
        cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
        hwnd,
        dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
        uCount: 0, // Flash until window comes to foreground
        dwTimeout: 0,
    };
    crate::types::FlashWindowEx(&mut fwi as *mut _ as *const _);
}

impl WindowHandler for AppState {
    fn is_dark_mode(&self) -> bool {
        theme::resolve_mode(self.theme)
    }

    fn on_create(&mut self, hwnd: HWND) -> LRESULT {
        unsafe {
            self.taskbar = Some(TaskbarProgress::new(hwnd));
            
            // 1. StatusBar
            let mut status_bar = StatusBar::new(StatusBarIds {
                label_id: IDC_STATIC_TEXT,
                progress_id: IDC_PROGRESS_BAR,
            });
            let _ = status_bar.create(hwnd);
            
            // 2. FileListView
            // Y position will be adjusted in on_resize to account for SearchPanel
            let file_list = FileListView::new(hwnd, 10, 100, 860, 320, IDC_BATCH_LIST);
            
            // 5. Search Panel
            let search_ids = SearchPanelIds {
                edit_search: IDC_SEARCH_EDIT,
                combo_filter_col: IDC_COMBO_FILTER_COL,
                combo_algo: IDC_COMBO_FILTER_ALGO,
                combo_size: IDC_COMBO_FILTER_SIZE,
                chk_case: IDC_CHK_CASE,
                chk_regex: IDC_CHK_REGEX,
                lbl_results: IDC_LBL_RESULTS,
                lbl_filter_by: IDC_LBL_FILTER_BY,
                lbl_algo: IDC_LBL_FILTER_ALGO,
                lbl_size: IDC_LBL_SIZE,
            };
            let mut search_panel = SearchPanel::new(search_ids);
            let _ = search_panel.create(hwnd);
            
            // 3. ActionPanel
            // Initial layout Y will be set during resize, but we need a value.
            let mut action_panel = ActionPanel::new(ActionPanelIds {
                btn_files: IDC_BTN_ADD_FILES,
                btn_folder: IDC_BTN_ADD_FOLDER,
                btn_remove: IDC_BTN_REMOVE,
                btn_clear: IDC_BTN_CLEAR,
                
                lbl_input: IDC_LBL_INPUT,
                
                combo_action_mode: IDC_COMBO_ACTION_MODE,
                lbl_action_mode: IDC_LBL_ACTION_MODE,
                
                combo_algo: IDC_COMBO_ALGO,
                lbl_algo: IDC_LBL_ALGO,
                
                chk_force: IDC_CHK_FORCE,
                
                btn_process: IDC_BTN_PROCESS_ALL,
                btn_cancel: IDC_BTN_CANCEL,
                btn_pause: IDC_BTN_PAUSE,
            });
            let _ = action_panel.create(hwnd);
            
            // 4. HeaderPanel
            let mut header_panel = HeaderPanel::new(HeaderPanelIds {
                btn_settings: IDC_BTN_SETTINGS,
                btn_about: IDC_BTN_ABOUT,
                btn_shortcuts: IDC_BTN_SHORTCUTS,
                btn_console: IDC_BTN_CONSOLE,
                btn_watcher: crate::ui::controls::IDC_BTN_WATCHER,
            });
            let _ = header_panel.create(hwnd);
            // Disable cancel and pause buttons initially
            crate::ui::wrappers::Button::new(action_panel.cancel_hwnd()).set_enabled(false);
            crate::ui::wrappers::Button::new(action_panel.pause_hwnd()).set_enabled(false);

            // Populate Combos
            self.populate_ui_combos(&action_panel);

            self.controls = Some(Controls {
                file_list,
                status_bar,
                action_panel,
                header_panel,
                search_panel,
            });
            
            // Set initial state of buttons (Disabled if empty)
            handlers::update_process_button_state(self);

            // Timer for UI updates
            SetTimer(hwnd, 1, 100, None);
            
            // Drag and Drop
            DragAcceptFiles(hwnd, 1);
            ChangeWindowMessageFilterEx(hwnd, WM_DROPFILES, MSGFLT_ALLOW, std::ptr::null_mut());
            ChangeWindowMessageFilterEx(hwnd, WM_COPYDATA, MSGFLT_ALLOW, std::ptr::null_mut());
            ChangeWindowMessageFilterEx(hwnd, 0x0049, MSGFLT_ALLOW, std::ptr::null_mut()); 
            
            // Apply saved theme
            let is_dark = theme::resolve_mode(self.theme);
            theme::set_window_frame_theme(hwnd, is_dark);
            if let Some(ctrls) = &mut self.controls {
                ctrls.update_theme(is_dark, hwnd);
            }
            
            // Process startup items (CLI args)
            self.handle_startup_items(hwnd);

            // Initialize global logger
            crate::logger::init_logger(self.tx.clone());
            if self.config.log_enabled {
                crate::logger::set_log_level(self.config.log_level_mask);
            } else {
                crate::logger::set_log_level(0);
            }
        }
        0
    }

    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            if msg == WM_DRAWITEM {
                let dis = &*(lparam as *const DRAWITEMSTRUCT);
                if dis.CtlID == IDC_BTN_PROCESS_ALL as u32 {
                    crate::ui::controls::draw_accent_button(lparam);
                    return Some(1);
                }
            }
            
            match msg {
                 // Custom Message: Set Enable Force Stop
                0x8003 => {
                    self.enable_force_stop = wparam != 0;
                    Some(0)
                },
                
                // Custom Message: Set System Guard
                0x8006 => {
                    self.config.enable_system_guard = wparam != 0;
                    Some(0)
                },
                
                // Custom Message: Set Low Power Mode
                0x8007 => {
                    self.low_power_mode = wparam != 0;
                    Some(0)
                },
                
                // Custom Message: Query Force Stop
                0x8004 => {
                    let should_kill = self.handle_force_stop_request(hwnd, wparam);
                     Some(if should_kill { 1 } else { 0 })
                },
                
                // Custom Message: Theme Changed
                0x8001 => {
                    self.handle_theme_change_request(hwnd, wparam);
                    Some(0)
                },
                
                // Custom Message: Log Settings Changed (0x8008)
                0x8008 => {
                    let enabled = wparam != 0;
                    let mask = lparam as u8;
                    // Update state config (best effort, though handlers.rs does it usually on close)
                    // For now, let's just update the global atomic so it takes effect instantly.
                    if enabled {
                        crate::logger::set_log_level(mask);
                    } else {
                        crate::logger::set_log_level(0);
                    }
                    Some(0)
                },
                
                // Custom Message: Set Compressed Attr (0x8009)
                0x8009 => {
                    self.config.set_compressed_attr = wparam != 0;
                    Some(0)
                },

                WM_COMMAND => Some(self.dispatch_command(hwnd, wparam, lparam)),
                WM_TIMER => Some(self.handle_timer(hwnd, wparam)),
                // FIX: handle_resize -> handle_size
                WM_SIZE => Some(self.handle_size(hwnd)),
                WM_CLOSE => {
                    // Check if processing is active
                    let state = self.global_state.load(std::sync::atomic::Ordering::Relaxed);
                    if state == crate::ui::state::ProcessingState::Running as u8 {
                        let msg = w!("A compression job is currently running.\n\nAre you sure you want to quit?");
                        let title = w!("Confirm Exit");
                        let res = MessageBoxW(hwnd, msg.as_ptr(), title.as_ptr(), 
                            MB_YESNO | MB_ICONWARNING);
                        
                        if res == IDNO {
                            return Some(0); // Cancel closure
                        }
                        // If YES, signal worker to stop before destroying to ensure resources clean up
                        self.global_state.store(crate::ui::state::ProcessingState::Stopped as u8, std::sync::atomic::Ordering::Relaxed);
                    }
                    DestroyWindow(hwnd);
                    Some(0)
                },
                WM_DESTROY => {
                    self.handle_destroy(hwnd);
                    Some(0)
                },
                WM_COPYDATA => Some(self.handle_copy_data(hwnd, lparam)),
                WM_DROPFILES => {
                    handlers::process_hdrop(hwnd, wparam as _, self);
                    Some(0)
                },
                // FIX: Use WM_SETTINGCHANGE to ensure it matches the import, not a variable capture
                WM_SETTINGCHANGE => {
                    // System theme changed?
                    if self.theme == AppTheme::System {
                         let is_dark = theme::resolve_mode(self.theme);
                         theme::set_window_frame_theme(hwnd, is_dark);
                         if let Some(ctrls) = &mut self.controls {
                             ctrls.update_theme(is_dark, hwnd);
                         }
                         InvalidateRect(hwnd, std::ptr::null(), 1);
                    }
                    None 
                },
                WM_NOTIFY => Some(self.handle_notify(hwnd, lparam)),
                WM_CONTEXTMENU => Some(self.handle_context_menu(hwnd, wparam)),
                0x0100 => Some(self.handle_keydown(hwnd, wparam)), // WM_KEYDOWN
                WM_LBUTTONDOWN | crate::types::WM_LBUTTONDBLCLK => {
                    // Clicking on the window background (outside listview) deselects all
                    if let Some(ctrls) = &self.controls {
                        ctrls.file_list.deselect_all();
                        handlers::update_process_button_state(self);
                    }
                    None
                },
                
                _ => None,
            }
        }
    }
}

// Logic Helper Functions (Extensions to AppState for Window Logic)

impl AppState {
    /// Refresh the file list based on current filters
    unsafe fn refresh_file_list(&mut self) {
        if let Some(ctrls) = &self.controls {
            ctrls.file_list.clear_all();
            
            let search = &self.search_state;
            let filter_text = search.text.trim().to_lowercase();
            // "use_regex" now means "use custom wildcard/regex matcher"
            let use_custom_match = search.use_regex && !search.text.trim().is_empty();

            for item in &self.batch_items {
                // 1. Column/Text Filter
                let text_match = if search.text.is_empty() {
                    true
                } else {
                    let haystack = match search.filter_column {
                        crate::ui::state::FilterColumn::Path => item.path.to_lowercase(),
                        crate::ui::state::FilterColumn::Status => {
                            use crate::ui::state::BatchStatus;
                            match item.status {
                                BatchStatus::Pending => "pending".to_string(),
                                BatchStatus::Processing => "processing".to_string(),
                                BatchStatus::Complete => "complete".to_string(),
                                BatchStatus::Error(_) => "error".to_string(),
                            }
                        },
                    };
                    
                    if use_custom_match {
                        let pattern = &search.text;
                        if search.case_sensitive {
                             let haystack_raw = match search.filter_column {
                                crate::ui::state::FilterColumn::Path => item.path.clone(),
                                crate::ui::state::FilterColumn::Status => {
                                    use crate::ui::state::BatchStatus;
                                    match &item.status {
                                        BatchStatus::Pending => "Pending".to_string(),
                                        BatchStatus::Processing => "Processing".to_string(),
                                        BatchStatus::Complete => "Complete".to_string(),
                                        BatchStatus::Error(e) => ["Error(", e, ")"].concat(),
                                    }
                                },
                            };
                            crate::utils::matcher::is_match(pattern, &haystack_raw)
                        } else {
                            // Pattern lowercased? filter_text is already lowercased search.text
                           crate::utils::matcher::is_match(&filter_text, &haystack)
                        }
                    } else if search.case_sensitive {
                         // Re-read without lowercase if case sensitive
                         let haystack_raw = match search.filter_column {
                            crate::ui::state::FilterColumn::Path => item.path.clone(),
                            crate::ui::state::FilterColumn::Status => {
                                    use crate::ui::state::BatchStatus;
                                    match &item.status {
                                        BatchStatus::Pending => "Pending".to_string(),
                                        BatchStatus::Processing => "Processing".to_string(),
                                        BatchStatus::Complete => "Complete".to_string(),
                                        BatchStatus::Error(e) => ["Error(", e, ")"].concat(),
                                    }
                            },
                        };
                        haystack_raw.contains(&search.text)
                    } else {
                        haystack.contains(&filter_text)
                    }
                };
                
                if !text_match { continue; }
                
                // 2. Algorithm Filter
                if let Some(target_algo) = search.algorithm_filter {
                    if item.algorithm != target_algo { continue; }
                }
                
                // 3. Size Filter
                let size_match = match search.size_filter {
                    1 => item.logical_size < 1_000_000, // Small < 1MB
                    2 => item.logical_size > 100_000_000, // Large > 100MB
                    _ => true,
                };
                if !size_match { continue; }

                // Add to List
                let logical_str = format_size(item.logical_size);
                let disk_str = format_size(item.disk_size);
                
                ctrls.file_list.add_item(
                    item.id, 
                    item, 
                    &logical_str, 
                    &disk_str, 
                    w!("Estimating..."), 
                    crate::engine::wof::CompressionState::None 
                );
            }
            
            // Update "Showing results" label
            let current_count = ctrls.file_list.get_item_count();
            let total_count = self.batch_items.len();
            
            let is_default_state = search.text.trim().is_empty() 
                && search.algorithm_filter.is_none() 
                && search.size_filter == 0
                && !search.use_regex;

            let msg = if is_default_state {
                if total_count == 0 {
                    crate::utils::to_wstring("Ready.")
                } else {
                    let prefix = crate::w!("Ready. ");
                    let count_str = crate::utils::fmt_u32(total_count as u32);
                    let suffix = crate::w!(" items loaded.");
                    crate::utils::concat_wstrings(&[prefix, &count_str, suffix])
                }
            } else {
                 if current_count == 0 {
                     crate::utils::to_wstring("No matching items found.")
                 } else {
                     let prefix = crate::w!("Found ");
                     let count_str = crate::utils::fmt_u32(current_count as u32);
                     let suffix = crate::w!(" matching items.");
                     crate::utils::concat_wstrings(&[prefix, &count_str, suffix])
                 }
            };
            
            crate::ui::wrappers::Label::new(ctrls.search_panel.results_hwnd()).set_text_w(&msg);
        }
    }

    /// Populate initial values for comboboxes and checkboxes
    unsafe fn populate_ui_combos(&self, action_panel: &ActionPanel) {
        // Algorithm Combo
        let h_combo = action_panel.combo_hwnd();
        let algos = [w!("As Listed"), w!("XPRESS4K"), w!("XPRESS8K"), w!("XPRESS16K"), w!("LZX")];
        
        let combo = ComboBox::new(h_combo);
        for alg in algos {
            combo.add_string(String::from_utf16_lossy(alg).as_str());
        }
        let algo_index = match self.config.default_algo {
            WofAlgorithm::Xpress4K => 1,
            WofAlgorithm::Xpress8K => 2,
            WofAlgorithm::Xpress16K => 3,
            WofAlgorithm::Lzx => 4,
        };
        combo.set_selected_index(algo_index);
        
        // Action Mode Combo
        let h_action_mode = action_panel.action_mode_hwnd();
        let action_mode_combo = ComboBox::new(h_action_mode);
        let action_modes = ["As Listed", "Compress All", "Decompress All"];
        for mode in action_modes {
            action_mode_combo.add_string(mode);
        }
        action_mode_combo.set_selected_index(0);
        
        // Force Checkbox
        if self.force_compress {
            Button::new(action_panel.force_hwnd()).set_checked(true);
        }
    }

    /// Process items passed via CLI
    unsafe fn handle_startup_items(&mut self, hwnd: HWND) {
        unsafe {
            let startup_items = crate::get_startup_items();
            if !startup_items.is_empty() {
                self.ipc_active = true;
                
                for startup_item in startup_items {
                     if !self.batch_items.iter().any(|item| item.path == startup_item.path) {
                            let item_id = self.add_batch_item(startup_item.path.clone());
                            if let Some(batch_item) = self.get_batch_item_mut(item_id) {
                                batch_item.algorithm = startup_item.algorithm;
                                batch_item.action = startup_item.action;
                            }
                            let metrics = crate::engine::worker::scan_path_metrics(&startup_item.path);
                            let logical_size = metrics.logical_size;
                            let disk_size = metrics.disk_size;
                            let detected_algo = metrics.compression_state;
                            let logical_str = format_size(logical_size);
                            let disk_str = format_size(disk_size);
                            
                            if let Some(ctrls) = &self.controls {
                                if let Some(batch_item) = self.batch_items.iter().find(|i| i.id == item_id) {
                                    ctrls.file_list.add_item(item_id, batch_item, &logical_str, &disk_str, w!("Estimating..."), detected_algo);
                                    if let Some(pos) = self.batch_items.iter().position(|i| i.id == item_id) {
                                        ctrls.file_list.set_selected(pos as i32, true);
                                        self.pending_ipc_ids.push(item_id);
                                    }
                                }
                            }
                     }
                }
                
                if let Some(ctrls) = &self.controls {
                     ComboBox::new(ctrls.action_panel.combo_hwnd()).set_selected_index(0);
                }
                SetTimer(hwnd, 2, 500, None);
            }
        }
    }

    /// Dispatch WM_COMMAND messages to appropriate handlers
    unsafe fn dispatch_command(&mut self, hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            let id = (wparam & 0xFFFF) as u16;
            let notification_code = ((wparam >> 16) & 0xFFFF) as u16;
            const CBN_SELCHANGE: u16 = 1;
            
            match id {
                 IDC_BTN_ADD_FILES => handlers::on_add_files(self),
                 IDC_BTN_ADD_FOLDER => handlers::on_add_folder(self),
                 IDC_BTN_REMOVE => handlers::on_remove_selected(self),
                 IDC_BTN_CLEAR => handlers::on_clear_all(self),
                 IDC_BTN_PROCESS_ALL => {
                     let is_auto_start = lparam == 1;
                     handlers::on_process_all(self, hwnd, is_auto_start);
                 },
                 IDC_BTN_CANCEL => handlers::on_stop_processing(self),
                 IDC_BTN_SETTINGS => handlers::on_open_settings(self, hwnd),
                 IDC_BTN_ABOUT => {
                      let is_dark = theme::resolve_mode(self.theme);
                      crate::ui::dialogs::show_about_modal(hwnd, is_dark);
                 },
                 IDC_BTN_SHORTCUTS => {
                      let is_dark = theme::resolve_mode(self.theme);
                      crate::ui::dialogs::show_shortcuts_modal(hwnd, is_dark);
                 },
                 IDC_BTN_CONSOLE => {
                       let is_dark = theme::resolve_mode(self.theme);
                       crate::ui::dialogs::show_console_window(self, hwnd, is_dark);
                 },
                 IDC_BTN_PAUSE => {
                       handlers::on_pause_clicked(self);
                 },
                 IDC_CHK_FORCE => {
                       let hwnd_ctl = lparam as HWND;
                       self.force_compress = Button::new(hwnd_ctl).is_checked();
                 },
                 // --- Search Panel Handlers ---
                 IDC_SEARCH_EDIT => {
                     if notification_code as u32 == EN_CHANGE {
                        if let Some(ctrls) = &self.controls {
                            let text = crate::ui::wrappers::get_window_text(ctrls.search_panel.search_hwnd());
                            self.search_state.text = text;
                            self.refresh_file_list();
                        }
                     }
                 },
                 IDC_COMBO_FILTER_COL => {
                     if notification_code == CBN_SELCHANGE {
                        if let Some(ctrls) = &self.controls {
                            let idx = crate::ui::wrappers::ComboBox::new(ctrls.search_panel.filter_col_hwnd()).get_selected_index();
                            self.search_state.filter_column = if idx == 1 { 
                                crate::ui::state::FilterColumn::Status 
                            } else { 
                                crate::ui::state::FilterColumn::Path 
                            };
                             self.refresh_file_list();
                        }
                     }
                 },
                 IDC_COMBO_FILTER_ALGO => {
                     if notification_code == CBN_SELCHANGE {
                        if let Some(ctrls) = &self.controls {
                            let idx = crate::ui::wrappers::ComboBox::new(ctrls.search_panel.algo_hwnd()).get_selected_index();
                            self.search_state.algorithm_filter = match idx {
                                1 => Some(WofAlgorithm::Xpress4K),
                                2 => Some(WofAlgorithm::Xpress8K),
                                3 => Some(WofAlgorithm::Xpress16K),
                                4 => Some(WofAlgorithm::Lzx),
                                _ => None,
                            };
                            self.refresh_file_list();
                        }
                     }
                 },
                 IDC_COMBO_FILTER_SIZE => {
                     if notification_code == CBN_SELCHANGE {
                        if let Some(ctrls) = &self.controls {
                            let idx = crate::ui::wrappers::ComboBox::new(ctrls.search_panel.size_hwnd()).get_selected_index();
                            self.search_state.size_filter = idx;
                            self.refresh_file_list();
                        }
                     }
                 },
                 IDC_CHK_CASE => {
                     if let Some(ctrls) = &self.controls {
                        self.search_state.case_sensitive = crate::ui::wrappers::Button::new(ctrls.search_panel.case_hwnd()).is_checked();
                        self.refresh_file_list();
                     }
                 },
                 IDC_CHK_REGEX => {
                     if let Some(ctrls) = &self.controls {
                        self.search_state.use_regex = crate::ui::wrappers::Button::new(ctrls.search_panel.regex_hwnd()).is_checked();
                        self.refresh_file_list();
                     }
                 },
                 IDC_COMBO_ALGO => {
                     // Main Process Algorithm Combo
                     if notification_code == CBN_SELCHANGE {
                         self.on_global_algo_changed();
                     }
                 },
                 IDC_BTN_WATCHER => handlers::on_open_watcher_manager(self, hwnd),
                 _ => {}
            }
            0
        }
    }

    /// Handle Timer events (Auto-start and UI updates)
    unsafe fn handle_timer(&mut self, hwnd: HWND, wparam: WPARAM) -> LRESULT {
        unsafe {
            // Timer 2: One-shot auto-start timer for IPC/CLI items
            if wparam == 2 {
                KillTimer(hwnd, 2);
                self.ipc_active = false;
                
                if !self.pending_ipc_ids.is_empty() {
                     if let Some(ctrls) = &self.controls {
                         // clear existing selection
                         let count = ctrls.file_list.get_item_count();
                         for i in 0..count {
                             ctrls.file_list.set_selected(i, false);
                         }
                         // select pending items
                         for &id in &self.pending_ipc_ids {
                             if let Some(pos) = self.batch_items.iter().position(|item| item.id == id) {
                                 ctrls.file_list.set_selected(pos as i32, true);
                             }
                         }
                     }
                     self.pending_ipc_ids.clear();
                }
                // Send lparam=1 to indicate Auto-Start source
                SendMessageW(hwnd, WM_COMMAND, IDC_BTN_PROCESS_ALL as usize, 1);
                return 0;
            }
            
            // Timer 1: Main UI Refresh Loop
            loop {
                match self.rx.try_recv() {
                    Ok(msg) => self.process_ui_message(hwnd, msg),
                    Err(_) => break,
                }
            }
            0
        }
    }

    /// Process messages from worker threads and update UI
    unsafe fn process_ui_message(&mut self, hwnd: HWND, msg: UiMessage) {
        unsafe {
            match msg {
                 UiMessage::Progress(cur, total) => {
                     if let Some(ctrls) = &self.controls {
                         let pb = ProgressBar::new(ctrls.status_bar.progress_hwnd());
                         pb.set_range(0, total as i32);
                         pb.set_pos(cur as i32);
                         
                         // Update Status Bar Text
                         let progress_str = crate::utils::fmt_progress(cur, total);
                         let prefix = crate::w!("Processed ");
                         let msg = crate::utils::concat_wstrings(&[prefix, &progress_str]);
                         SetWindowTextW(ctrls.status_bar.label_hwnd(), msg.as_ptr());
                     }
                     if let Some(tb) = &self.taskbar { tb.set_value(cur, total); }
                 },
                 UiMessage::StatusText(text_w) => {
                     if let Some(ctrls) = &self.controls {
                         SetWindowTextW(ctrls.status_bar.label_hwnd(), text_w.as_ptr());
                     }
                 },
                 UiMessage::Log(entry) => {
                     // Check if Error level to update Taskbar/Status
                     if entry.level == crate::logger::LogLevel::Error {
                         if let Some(tb) = &self.taskbar { tb.set_state(TaskbarState::Error); }
                         
                         let msg_w = to_wstring(&entry.message);
                         if let Some(ctrls) = &self.controls {
                             SetWindowTextW(ctrls.status_bar.label_hwnd(), msg_w.as_ptr());
                         }
                     }
                     
                     self.logs.push_back(entry.clone());
                     if self.logs.len() > 1000 {
                         self.logs.pop_front();
                     }
                     crate::ui::dialogs::append_log_entry(self.console_hwnd, entry);
                 },
                 UiMessage::Finished => {
                     // Check queue for more items
                     if !self.processing_queue.is_empty() {
                         let max = self.config.max_concurrent_items as usize;
                         // Determine next batch
                         let next_indices: Vec<usize> = if max > 0 && self.processing_queue.len() > max {
                             self.processing_queue.drain(0..max).collect()
                         } else {
                             self.processing_queue.drain(..).collect()
                         };
                         
                         // Start next batch without going Idle
                         // Note: We are already in 'Running' state, so we just spawn the next thread
                         handlers::start_processing_internal(self, hwnd, next_indices);
                         
                         // Update queue status in status bar
                         let queued_count = self.processing_queue.len();
                         if queued_count > 0 {
                             if let Some(ctrls) = &self.controls {
                                 let prefix = crate::w!("Processing... (");
                                 let q_str = crate::utils::fmt_u32(queued_count as u32);
                                 let suffix = crate::w!(" queued)");
                                 let q_msg = crate::utils::concat_wstrings(&[prefix, &q_str, suffix]);
                                 Label::new(ctrls.status_bar.label_hwnd()).set_text_w(&q_msg);
                             }
                         }
                     } else {
                         // Queue empty, really finished
                         self.global_state.store(crate::ui::state::ProcessingState::Idle as u8, std::sync::atomic::Ordering::Relaxed);
                         if let Some(tb) = &self.taskbar { tb.set_state(TaskbarState::NoProgress); }
                         if let Some(ctrls) = &self.controls {
                             EnableWindow(ctrls.action_panel.cancel_hwnd(), FALSE);
                         }
                         handlers::update_process_button_state(self);

                         // Re-acquire WinAPI
                         // Re-acquire WinAPI
                         if crate::types::GetForegroundWindow() != hwnd {
                             flash_window(hwnd); // we need to update flash_window too
                         }
                     }
                 },
                 UiMessage::ScanProgress(id, logical, disk, count) => {
                     if let Some(pos) = self.batch_items.iter().position(|item| item.id == id) {
                         if let Some(item) = self.batch_items.get_mut(pos) {
                             item.logical_size = logical;
                             item.disk_size = disk;
                         }

                         if let Some(ctrls) = &self.controls {
                             let log_str = format_size(logical);
                             let disk_str = format_size(disk);
                             let ratio_str = crate::utils::calculate_ratio_string(logical, disk);
                             let count_str = u64_to_wstring(count);
                             
                             // Find visual row by ID
                             if let Some(row) = ctrls.file_list.find_item_by_id(id) {
                                 ctrls.file_list.update_item_text(row, 4, &log_str);
                                 ctrls.file_list.update_item_text(row, 6, &disk_str);
                                 ctrls.file_list.update_item_text(row, 7, &ratio_str); // Ratio
                                 
                                 let current_state = self.global_state.load(Ordering::Relaxed);
                                 if current_state == ProcessingState::Stopped as u8 {
                                     ctrls.file_list.update_item_text(row, 9, w!("Cancelled"));
                                 } else if current_state == ProcessingState::Paused as u8 {
                                     ctrls.file_list.update_item_text(row, 9, w!("Paused"));
                                 } else {
                                     let status_text = concat_wstrings(&[w!("Scanning... "), &count_str]);
                                     ctrls.file_list.update_item_text(row, 9, &status_text);
                                 }
                             }

                             if let Some(item) = self.batch_items.get(pos) {
                                 let prefix = crate::w!("Scanning: ");
                                 let path_w = to_wstring(&item.path);
                                 let mid = crate::w!("... (");
                                 let count_w = crate::utils::fmt_u64(crate::w!("%I64u"), count);
                                 let suffix = crate::w!(" files)");
                                 
                                 let sb_msg = crate::utils::concat_wstrings(&[prefix, &path_w, mid, &count_w, suffix]);
                                 Label::new(ctrls.status_bar.label_hwnd()).set_text_w(&sb_msg);
                             }
                         }
                     }
                 },
                 UiMessage::RowProgress(row_idx, cur, tot, bytes) => {
                     if let Some(ctrls) = &self.controls {
                         // Map batch index to item ID
                         let item_id = self.batch_items.get(row_idx as usize).map(|i| i.id);
                         
                         if let Some(id) = item_id {
                             if let Some(row) = ctrls.file_list.find_item_by_id(id) {
                                 // Update Progress Text (Column 8)
                                 let cur_w = u64_to_wstring(cur);
                                 let tot_w = u64_to_wstring(tot);
                                 let prog_str = concat_wstrings(&[&cur_w, &to_wstring("/"), &tot_w]);
                                 ctrls.file_list.update_item_text(row, 8, &prog_str);
                                 
                                 // Update Size (Column 6) - Live update
                                 if bytes > 0 {
                                     let size_str = format_size(bytes);
                                     ctrls.file_list.update_item_text(row, 6, &size_str);
                                 }

                                 let current_state = self.global_state.load(Ordering::Relaxed);
                                 if current_state == ProcessingState::Stopped as u8 {
                                      ctrls.file_list.update_item_text(row, 9, w!("Cancelled"));
                                 } else if current_state == ProcessingState::Paused as u8 {
                                     // If effectively paused, don't let "Running" overwrite "Paused"
                                      ctrls.file_list.update_item_text(row, 9, w!("Paused"));
                                 } else {
                                      ctrls.file_list.update_item_text(row, 9, w!("Running"));
                                 }
                             }
                         }
                     }
                 },
                 UiMessage::RowFinished(row_idx, final_bytes, total_count, final_state) => {
                     if let Some(ctrls) = &self.controls {
                         let item_id = self.batch_items.get(row_idx as usize).map(|i| i.id);
                         if let Some(id) = item_id {
                             if let Some(row) = ctrls.file_list.find_item_by_id(id) {
                                 ctrls.file_list.update_item_text(row, 9, w!("Done"));
                                 
                                 // Update Progress Text (Column 8)
                                 let tot_w = u64_to_wstring(total_count);
                                 let prog_str = concat_wstrings(&[&tot_w, &to_wstring("/"), &tot_w]);
                                 ctrls.file_list.update_item_text(row, 8, &prog_str);
                                 
                                 // Update final size
                                 if final_bytes > 0 {
                                     let size_str = format_size(final_bytes);
                                     ctrls.file_list.update_item_text(row, 6, &size_str);
                                 }
                                 
                                 // Update visuals for finished item (Watch button only, no Play button)
                                 ctrls.file_list.update_playback_controls(row, ProcessingState::Stopped, true);
                             }
                         }

                         // Update in-memory state
                         if let Some(item) = self.batch_items.get_mut(row_idx as usize) {
                             item.disk_size = final_bytes;
                             item.status = BatchStatus::Complete;
                             item.state_flag = None;
                             item.progress = (total_count, total_count);
                         }

                         // Update UI for final state
                         if let Some(ctrls) = &self.controls {
                             if let Some(id) = item_id {
                                 if let Some(row) = ctrls.file_list.find_item_by_id(id) {
                                     // Calculate Ratio with final sizes
                                     if let Some(item) = self.batch_items.get(row_idx as usize) {
                                          let ratio_str = crate::utils::calculate_ratio_string(item.logical_size, final_bytes);
                                          ctrls.file_list.update_item_text(row, 7, &ratio_str);
                                     }
    
                                     // Update State/Algorithm Column
                                     let state_str = match final_state {
                                         CompressionState::None => w!("-"),
                                         CompressionState::Specific(algo) => match algo {
                                             WofAlgorithm::Xpress4K => w!("XPRESS4K"), WofAlgorithm::Xpress8K => w!("XPRESS8K"),
                                             WofAlgorithm::Xpress16K => w!("XPRESS16K"), WofAlgorithm::Lzx => w!("LZX"),
                                         },
                                         CompressionState::Mixed => w!("Mixed"),
                                     };
                                     ctrls.file_list.update_item_text(row, 1, state_str);
                                     
                                     // Start button text already set above
                                 }
                             }
                         }
                     }
                     handlers::update_process_button_state(self);
                 },
                 UiMessage::WatcherTrigger(path, algo) => {
                     // Auto-add and start processing for watcher
                     if !self.batch_items.iter().any(|item| item.path == path) {
                         // Only add if not already in list
                         let id = self.add_batch_item(path.clone());
                         self.set_item_algorithm(id, algo);
                         
                         // Add to UI
                         if let Some(ctrls) = &self.controls {
                             if let Some(item) = self.batch_items.last() {
                                  ctrls.file_list.add_item(id, item, w!("Pending..."), w!("-"), w!("-"), CompressionState::None);
                             }
                             
                             // Force "As Listed" mode so per-item algorithm is used
                             let combo = crate::ui::wrappers::ComboBox::new(ctrls.action_panel.combo_hwnd());
                             let original_idx = combo.get_selected_index();
                             combo.set_selected_index(0); // "As Listed" is index 0
                             
                             // Trigger processing for this item immediately
                             if let Some(pos) = self.batch_items.iter().position(|i| i.id == id) {
                                 handlers::start_processing(self, hwnd, vec![pos]);
                             }
                             
                             // Restore original combo selection
                             if let Some(ctrls) = &self.controls {
                                 crate::ui::wrappers::ComboBox::new(ctrls.action_panel.combo_hwnd()).set_selected_index(original_idx);
                             }
                         }
                     }
                 },
                 UiMessage::BatchItemAnalyzed(id, log, disk, state) => {
                     let log_str = format_size(log);
                     let disk_str = format_size(disk);
                     let ratio_str = crate::utils::calculate_ratio_string(log, disk);
                     
                     if let Some(pos) = self.batch_items.iter().position(|item| item.id == id) {
                         if let Some(item) = self.batch_items.get_mut(pos) {
                             item.logical_size = log;
                             item.disk_size = disk;
                         }
                         
                         if let Some(ctrls) = &self.controls {
                             ctrls.file_list.update_item_text(pos as i32, 4, &log_str);
                             ctrls.file_list.update_item_text(pos as i32, 6, &disk_str);
                             ctrls.file_list.update_item_text(pos as i32, 7, &ratio_str); // Ratio
                             
                             let state_str = match state {
                                CompressionState::None => w!("-"),
                                CompressionState::Specific(algo) => match algo {
                                    WofAlgorithm::Xpress4K => w!("XPRESS4K"), WofAlgorithm::Xpress8K => w!("XPRESS8K"),
                                    WofAlgorithm::Xpress16K => w!("XPRESS16K"), WofAlgorithm::Lzx => w!("LZX"),
                                },
                                CompressionState::Mixed => w!("Mixed"),
                             };
                             ctrls.file_list.update_item_text(pos as i32, 1, state_str);
                             ctrls.file_list.update_item_text(pos as i32, 9, w!("Pending")); // Status is now 9
                             let count = self.batch_items.len();
                             let count_w = u64_to_wstring(count as u64);
                             let msg = concat_wstrings(&[&count_w, w!(" item(s) analyzed.")]);
                             SetWindowTextW(ctrls.status_bar.label_hwnd(), msg.as_ptr());
                         }
                         handlers::update_process_button_state(self);
                     }
                 },
                 UiMessage::UpdateEstimate(id, algo, est_size) => {
                     if let Some(pos) = self.batch_items.iter().position(|item| item.id == id) {
                         if let Some(item) = self.batch_items.get_mut(pos) {
                             // Cache the result for this algorithm
                             item.cache_estimate(algo, est_size);
                         }
                         if let Some(ctrls) = &self.controls {
                             let est_str_local = crate::utils::format_size(est_size);
                             ctrls.file_list.update_item_text(pos as i32, 5, &est_str_local);
                         }
                     }
                 },
                 _ => {}
            }
        }
    }

    unsafe fn handle_size(&mut self, hwnd: HWND) -> LRESULT {
        unsafe {
            let mut client_rect: RECT = std::mem::zeroed();
            if GetClientRect(hwnd, &mut client_rect) != 0 {
                if let Some(ctrls) = &mut self.controls {
                     let _width = client_rect.right - client_rect.left;
                     let height = client_rect.bottom - client_rect.top;
                     
                     // Layout Constants
                     let header_h = 50;
                     // Reduced height for 2-row layout (Search/Results + Filters)
                     let search_h = 85; 
                     
                     // Dynamic Action Panel Height calculation
                     // Top Padding (4) + Label (16) + Gap (4) + Button (30) + Bottom Padding (6) = 60
                     let action_h = 60;  
                     
                     let status_h = 22;  // Reduced to 22 (tight fit for 20px bar)
                     let padding = 10;
                     
                     // 1. Header Rect (Top Strip)
                     let header_rect = RECT { 
                        left: client_rect.left, 
                        top: client_rect.top, 
                        right: client_rect.right, 
                        bottom: client_rect.top + header_h 
                     };
                     
                     // 2. Search Rect (Below Header)
                     let search_rect = RECT {
                        left: client_rect.left,
                        top: header_rect.bottom,
                        right: client_rect.right,
                        bottom: header_rect.bottom + search_h
                     };
                     
                     // 3. Action Rect (Bottom Strip)
                     // Calculate tops from bottom up
                     // Status is now ABOVE Action, below List.
                     
                     let action_top = height - action_h;
                     let action_rect = RECT {
                        left: client_rect.left,
                        top: client_rect.top + action_top, // Absolute Y
                        right: client_rect.right,
                        bottom: client_rect.top + height
                     };

                     // 4. Status Rect (Above Action)
                     // StatusBar draws label at top, progress at bottom.
                     // We want progress bar here.
                     let status_top = action_top - status_h;
                     let status_rect = RECT {
                         left: client_rect.left,
                         top: client_rect.top + status_top,
                         right: client_rect.right,
                         bottom: client_rect.top + action_top
                     };
                     
                     // 5. List Rect (Middle Fill)
                     // Fills space between Search and Status
                     let list_top = search_rect.bottom;
                     let list_bottom = status_rect.top; // Space before status
                     
                     let list_rect = RECT {
                        left: client_rect.left + padding,
                        top: list_top,
                        right: client_rect.right - padding,
                        bottom: std::cmp::max(list_top + 10, list_bottom) // Ensure min height
                     };

                     // Apply Layouts
                     ctrls.header_panel.on_resize(&header_rect);
                     ctrls.search_panel.on_resize(&search_rect);
                     
                     // Note: Layout order depends on intended Z-order if overlapping, usually doesn't matter for distinct rects.
                     ctrls.file_list.on_resize(&list_rect);
                     ctrls.status_bar.on_resize(&status_rect); // Progress bar now in middle-ish
                     ctrls.action_panel.on_resize(&action_rect); // Controls at bottom
                }
            }
            0
        }
    }

    /// Re-estimate all items when the global algorithm ComboBox changes
    unsafe fn on_global_algo_changed(&mut self) {
        // Get the selected algorithm from the ComboBox
        let algo = if let Some(ctrls) = &self.controls {
                let idx = ComboBox::new(ctrls.action_panel.combo_hwnd()).get_selected_index();
                match idx {
                    0 => None, // "As Listed" - use per-item algorithm
                    1 => Some(WofAlgorithm::Xpress4K),
                    2 => Some(WofAlgorithm::Xpress8K),
                    3 => Some(WofAlgorithm::Xpress16K),
                    4 => Some(WofAlgorithm::Lzx),
                    _ => Some(WofAlgorithm::Xpress8K),
                }
            } else {
                return;
            };

            // Check cache first and collect items that need estimation
            let mut items_to_estimate: Vec<(u32, String, WofAlgorithm)> = Vec::new();
            
            for (i, item) in self.batch_items.iter_mut().enumerate() {
                let effective_algo = algo.unwrap_or(item.algorithm);
                
                // Check cache
                if let Some(cached) = item.get_cached_estimate(effective_algo) {
                    // Use cached value instantly
                    item.estimated_size = cached;
                    let est_str = format_size(cached);
                    if let Some(ctrls) = &self.controls {
                        ctrls.file_list.update_item_text(i as i32, 5, &est_str);
                    }
                } else {
                    // Need to calculate
                    items_to_estimate.push((item.id, item.path.clone(), effective_algo));
                    if let Some(ctrls) = &self.controls {
                        ctrls.file_list.update_item_text(i as i32, 5, w!("Estimating..."));
                    }
                }
            }

            if items_to_estimate.is_empty() {
                return;
            }

            // Spawn estimation thread only for items that need it
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                for (id, path, algo) in items_to_estimate {
                    let estimated = crate::engine::estimator::estimate_path(&path, algo);
                    let _est_str = crate::utils::format_size(estimated);
                    let _ = tx.send(UiMessage::UpdateEstimate(id, algo, estimated));
                }
            });
    }

    unsafe fn handle_destroy(&mut self, hwnd: HWND) {
        unsafe {
            let mut rect: RECT = std::mem::zeroed();
            if GetWindowRect(hwnd, &mut rect) != 0 {
                self.config.window_x = rect.left;
                self.config.window_y = rect.top;
                self.config.window_width = rect.right - rect.left;
                self.config.window_height = rect.bottom - rect.top;
            }
            if let Some(ctrls) = &self.controls {
                let idx = ComboBox::new(ctrls.action_panel.combo_hwnd()).get_selected_index();
                self.config.default_algo = match idx {
                    1 => WofAlgorithm::Xpress4K,
                    3 => WofAlgorithm::Xpress16K, 
                    4 => WofAlgorithm::Lzx,
                    _ => WofAlgorithm::Xpress8K,
                };
                self.config.force_compress = Button::new(ctrls.action_panel.force_hwnd()).is_checked();
            }
            self.config.theme = self.theme;
            self.config.enable_force_stop = self.enable_force_stop;
            self.config.low_power_mode = self.low_power_mode;
            self.config.save();
            
            // Force explicit process exit to ensure all threads terminate
            std::process::exit(0);
        }
    }

    unsafe fn handle_copy_data(&mut self, hwnd: HWND, lparam: LPARAM) -> LRESULT {
         unsafe {
             let cds = &*(lparam as *const COPYDATASTRUCT);
             if cds.dwData == 0xB00B {
                 let len = (cds.cbData / 2) as usize;
                 let slice = std::slice::from_raw_parts(cds.lpData as *const u16, len);
                 let payload = String::from_utf16_lossy(slice).trim_matches('\0').to_string();
                 let parts: Vec<&str> = payload.split('|').collect();
                 
                 if parts.len() >= 3 {
                     let path = parts[0].to_string();
                     let algo = match parts[1] {
                         "xpress4k" => WofAlgorithm::Xpress4K,
                         "xpress8k" => WofAlgorithm::Xpress8K,
                         "xpress16k" => WofAlgorithm::Xpress16K,
                         "lzx" => WofAlgorithm::Lzx,
                         _ => WofAlgorithm::Xpress8K,
                     };
                     let action = match parts[2] {
                         "decompress" => BatchAction::Decompress,
                         _ => BatchAction::Compress,
                     };
                     
                     if !self.batch_items.iter().any(|item| item.path == path) {
                         self.ingest_paths(vec![path.clone()]);
                         
                         // Apply IPC specifics (Algorithm/Action)
                         if let Some(pos) = self.batch_items.iter().position(|i| i.path == path) {
                              let id = self.batch_items[pos].id;
                              if let Some(item) = self.batch_items.get_mut(pos) {
                                   item.algorithm = algo;
                                   item.action = action;
                              }
                              
                              if let Some(ctrls) = &self.controls {
                                   ComboBox::new(ctrls.action_panel.combo_hwnd()).set_selected_index(0); // Reset Global Combo
                                   
                                   // Update UI for Algo/Action
                                   let algo_name = match algo {
                                       WofAlgorithm::Xpress4K => "XPRESS4K", WofAlgorithm::Xpress8K => "XPRESS8K",
                                       WofAlgorithm::Xpress16K => "XPRESS16K", WofAlgorithm::Lzx => "LZX",
                                   };
                                   let action_name = match action {
                                       BatchAction::Compress => "Compress", BatchAction::Decompress => "Decompress",
                                   };
                                   ctrls.file_list.update_item_text(pos as i32, 2, &to_wstring(algo_name));
                                   ctrls.file_list.update_item_text(pos as i32, 3, &to_wstring(action_name));

                                   if !self.ipc_active {
                                       let count = ctrls.file_list.get_item_count();
                                       for i in 0..count {
                                           ctrls.file_list.set_selected(i, false);
                                       }
                                   }
                                   
                                   ctrls.file_list.set_selected(pos as i32, true);
                                   self.pending_ipc_ids.push(id);
                              }
                         }
                         
                         SetTimer(hwnd, 2, 500, None);
                     }
                 }
                 return 1;
             }
             0
         }
    }
    
    unsafe fn handle_notify(&mut self, hwnd: HWND, lparam: LPARAM) -> LRESULT {
        unsafe {
            let nmhdr = &*(lparam as *const NMHDR);
            if nmhdr.idFrom == IDC_BATCH_LIST as usize {
                if nmhdr.code == NM_CLICK || nmhdr.code == NM_DBLCLK {
                    let nmia = &*(lparam as *const NMITEMACTIVATE);
                    handlers::on_list_click(self, hwnd, nmia.iItem, nmia.iSubItem, nmhdr.code);
                } else if nmhdr.code == LVN_KEYDOWN {
                    let nmkd = &*(lparam as *const NMLVKEYDOWN);
                    handlers::on_list_keydown(self, hwnd, nmkd.wVKey as u16);
                } else if nmhdr.code == LVN_COLUMNCLICK {
                    handlers::on_column_click(self, lparam);
                } else if nmhdr.code == LVN_ITEMCHANGED {
                   handlers::update_process_button_state(self);
                } else if nmhdr.code == NM_RCLICK {
                     let nmia = &*(lparam as *const NMITEMACTIVATE);
                     if handlers::on_list_rclick(self, hwnd, nmia.iItem, nmia.iSubItem) {
                         return 1;
                     }
                } else if nmhdr.code == NM_CUSTOMDRAW {
                    // Handle custom draw for Ratio column color coding
                    if let Some(ctrls) = &self.controls {
                        let is_dark = theme::resolve_mode(self.theme);
                        let list_hwnd = ctrls.file_list.hwnd();
                        if let Some(result) = crate::ui::components::file_list::handle_listview_customdraw(
                            list_hwnd, lparam, is_dark, &self.batch_items
                        ) {
                            return result;
                        }
                    }
                }
            }
            0
        }
    }

    unsafe fn handle_context_menu(&mut self, hwnd: HWND, wparam: WPARAM) -> LRESULT {
        unsafe {
            handlers::handle_context_menu(self, hwnd, wparam);
            0
        }
    }

    unsafe fn handle_keydown(&mut self, hwnd: HWND, wparam: WPARAM) -> LRESULT {
        unsafe {
            let vk = wparam as u16;
            let ctrl_pressed = (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
            let shift_pressed = (GetKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;

            if ctrl_pressed {
                match vk {
                    0x56 => handlers::process_clipboard(hwnd, self), // V - Paste
                    0x4F => { // O - Open
                        if shift_pressed { handlers::on_add_folder(self); } 
                        else { handlers::on_add_files(self); }
                    },
                    0x41 => { // A - Select All
                         if let Some(ctrls) = &self.controls {
                             let count = ctrls.file_list.get_item_count();
                             for i in 0..count {
                                 ctrls.file_list.set_selected(i, true);
                             }
                         }
                    },
                    _ => {}
                }
            } else if vk == VK_DELETE as u16 {
                handlers::on_remove_selected(self);
            }
            0
        }
    }
    
    unsafe fn handle_force_stop_request(&mut self, hwnd: HWND, wparam: WPARAM) -> bool {
         unsafe {
             if self.enable_force_stop { return true; }
             
             let name_ptr = wparam as *const u16;
             let len = (0..).take_while(|&i| *name_ptr.offset(i) != 0).count();
             let slice = std::slice::from_raw_parts(name_ptr, len);
             let name = String::from_utf16_lossy(slice);
             
             // 1. Check if ignored
             if self.ignored_lock_processes.contains(&name) {
                 return false;
             }
             
             // 2. Check if dialog already active for this process
             if self.active_lock_dialog.as_deref() == Some(&name) {
                 // Prevent stacking dialogs for the same process.
                 // Return false (don't force stop) to let this specific file fail graciously 
                 // while the user decides on the main dialog.
                 return false;
             }
             
             // 3. Show dialog
             self.active_lock_dialog = Some(name.clone());
             
             let is_dark = theme::resolve_mode(self.theme);
             let result = crate::ui::dialogs::show_force_stop_dialog(hwnd, &name, is_dark);
             
             self.active_lock_dialog = None;
             
             if !result {
                 // User said No (Cancel), ignore this process for the rest of the session/batch
                 self.ignored_lock_processes.insert(name);
             }
             
             result
         }
    }

    unsafe fn handle_theme_change_request(&mut self, hwnd: HWND, wparam: WPARAM) {
        unsafe {
            let new_theme = match wparam {
                0 => AppTheme::System,
                1 => AppTheme::Dark,
                2 => AppTheme::Light,
                _ => self.theme,
            };
            self.theme = new_theme;
            let is_dark = theme::resolve_mode(self.theme);
            theme::set_window_frame_theme(hwnd, is_dark);
            if let Some(ctrls) = &mut self.controls {
                ctrls.update_theme(is_dark, hwnd);
            }
            InvalidateRect(hwnd, std::ptr::null(), 1);
        }
    }
}