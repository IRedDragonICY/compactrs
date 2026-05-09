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
use crate::utils::{to_wstring, u64_to_wstring, concat_wstrings};
use crate::w;
use crate::ui::framework::{WindowHandler, WindowBuilder, WindowAlignment, load_app_icon};
use crate::config::AppConfig;

const WINDOW_CLASS_NAME: &str = "CompactRS_Class";
const WINDOW_TITLE: &str = "CompactRS";

pub unsafe fn create_main_window(instance: HINSTANCE) -> Result<HWND, String> {
    unsafe {
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
        let win_width = if config.window_width > 0 { config.window_width } else { crate::ui::theme::scale(900) };
        let win_height = if config.window_height > 0 { config.window_height } else { crate::ui::theme::scale(600) };
        
        // Setup State
        let mut state = Box::new(AppState::new());
        // Load watcher tasks
        let loaded_tasks = crate::watcher_config::WatcherConfig::load();
        if loaded_tasks.len() > 0 {
             state.watcher_tasks = std::sync::Arc::new(std::sync::Mutex::new(loaded_tasks));
        }

        // Start Watcher Thread
        let watcher_tasks_ref = state.watcher_tasks.clone();
        let watcher_tx = state.tx.clone();
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

        // Hostile Takeover: Force window to foreground
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
        uCount: 0, 
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
            
            let mut header_panel = HeaderPanel::new(HeaderPanelIds {
                btn_settings: IDC_BTN_SETTINGS,
                btn_about: IDC_BTN_ABOUT,
                btn_shortcuts: IDC_BTN_SHORTCUTS,
                btn_console: IDC_BTN_CONSOLE,
                btn_watcher: crate::ui::controls::IDC_BTN_WATCHER,
            });
            let _ = header_panel.create(hwnd);

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

            let file_list = FileListView::new(hwnd, 10, 100, 860, 320, IDC_BATCH_LIST);
            
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
                lbl_accuracy: crate::ui::controls::IDC_LBL_ACCURACY,
                btn_process: IDC_BTN_PROCESS_ALL,
                btn_cancel: IDC_BTN_CANCEL,
                btn_pause: IDC_BTN_PAUSE,
            });
            let _ = action_panel.create(hwnd);
            
            let mut status_bar = StatusBar::new(StatusBarIds {
                label_id: IDC_STATIC_TEXT,
                progress_id: IDC_PROGRESS_BAR,
            });
            let _ = status_bar.create(hwnd);

            crate::ui::wrappers::Button::new(action_panel.cancel_hwnd()).set_enabled(false);
            crate::ui::wrappers::Button::new(action_panel.pause_hwnd()).set_enabled(false);

            self.populate_ui_combos(&action_panel);

            self.controls = Some(Controls {
                file_list,
                status_bar,
                action_panel,
                header_panel,
                search_panel,
            });
            
            handlers::update_process_button_state(self);

            SetTimer(hwnd, 1, 100, None);
            
            DragAcceptFiles(hwnd, 1);
            ChangeWindowMessageFilterEx(hwnd, WM_DROPFILES, MSGFLT_ALLOW, std::ptr::null_mut());
            ChangeWindowMessageFilterEx(hwnd, WM_COPYDATA, MSGFLT_ALLOW, std::ptr::null_mut());
            ChangeWindowMessageFilterEx(hwnd, 0x0049, MSGFLT_ALLOW, std::ptr::null_mut()); 
            
            let is_dark = theme::resolve_mode(self.theme);
            theme::set_window_frame_theme(hwnd, is_dark);
            if let Some(ctrls) = &mut self.controls {
                ctrls.update_theme(is_dark, hwnd);
            }
            
            self.handle_startup_items(hwnd);

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
                0x8003 => {
                    self.enable_force_stop = wparam != 0;
                    Some(0)
                },
                0x8006 => {
                    self.config.enable_system_guard = wparam != 0;
                    Some(0)
                },
                0x8007 => {
                    self.low_power_mode = wparam != 0;
                    Some(0)
                },
                0x8004 => {
                    let should_kill = self.handle_force_stop_request(hwnd, wparam);
                     Some(if should_kill { 1 } else { 0 })
                },
                0x8001 => {
                    self.handle_theme_change_request(hwnd, wparam);
                    Some(0)
                },
                0x8008 => {
                    let enabled = wparam != 0;
                    let mask = lparam as u8;
                    if enabled {
                        crate::logger::set_log_level(mask);
                    } else {
                        crate::logger::set_log_level(0);
                    }
                    Some(0)
                },
                0x8009 => {
                    self.config.set_compressed_attr = wparam != 0;
                    Some(0)
                },
                WM_COMMAND => Some(self.dispatch_command(hwnd, wparam, lparam)),
                WM_TIMER => Some(self.handle_timer(hwnd, wparam)),
                WM_SIZE => Some(self.handle_size(hwnd)),
                WM_CLOSE => {
                    let state = self.global_state.load(std::sync::atomic::Ordering::Relaxed);
                    if state == crate::ui::state::ProcessingState::Running as u8 {
                        let msg = w!("A compression job is currently running.\n\nAre you sure you want to quit?");
                        let title = w!("Confirm Exit");
                        let res = MessageBoxW(hwnd, msg.as_ptr(), title.as_ptr(), MB_YESNO | MB_ICONWARNING);
                        if res == IDNO {
                            return Some(0);
                        }
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
                    handlers::process_hdrop(hwnd, wparam as _, self, true);
                    Some(0)
                },
                WM_SETTINGCHANGE => {
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
                WM_KEYDOWN => Some(self.handle_keydown(hwnd, wparam, hwnd)),
                1924 => {
                    let source_hwnd = lparam as HWND;
                    Some(self.handle_keydown(hwnd, wparam, source_hwnd))
                },
                WM_LBUTTONDOWN | crate::types::WM_LBUTTONDBLCLK => {
                    if let Some(ctrls) = &self.controls {
                        ctrls.file_list.deselect_all();
                        handlers::update_process_button_state(self);
                    }
                    None
                },
                WM_ERASEBKGND => {
                    let hdc = wparam as HDC;
                    let mut rect: RECT = std::mem::zeroed();
                    GetClientRect(hwnd, &mut rect);
                    let is_dark = theme::resolve_mode(self.theme);
                    let brush = theme::get_background_brush(is_dark);
                    FillRect(hdc, &rect, brush);
                    Some(1)
                },
                _ => None,
            }
        }
    }
}

// Logic Helper Functions (Extensions to AppState for Window Logic)

impl AppState {
    unsafe fn populate_ui_combos(&self, action_panel: &ActionPanel) {
        let h_combo = action_panel.combo_hwnd();
        let algos = [w!("As Listed"), w!("XPRESS4K"), w!("XPRESS8K"), w!("XPRESS16K"), w!("LZX"), w!("LZNT1")];
        
        let combo = ComboBox::new(h_combo);
        for alg in algos {
            combo.add_string(String::from_utf16_lossy(alg).as_str());
        }
        
        combo.set_selected_index(self.config.combo_algo_index as i32);
        
        let h_action_mode = action_panel.action_mode_hwnd();
        let action_mode_combo = ComboBox::new(h_action_mode);
        let action_modes = ["As Listed", "Compress All", "Decompress All"];
        for mode in action_modes {
            action_mode_combo.add_string(mode);
        }
        
        action_mode_combo.set_selected_index(self.config.combo_action_index as i32);
        
        if self.force_compress {
            Button::new(action_panel.force_hwnd()).set_checked(true);
        }
    }

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
                            if let Some(item) = self.get_batch_item_mut(item_id) {
                                item.logical_size = metrics.logical_size;
                                item.disk_size = metrics.disk_size;
                                item.final_state = Some(metrics.compression_state);
                            }
                            self.pending_ipc_ids.push(item_id);
                     }
                }
                
                self.refresh_file_list();
                if let Some(ctrls) = &self.controls {
                     ComboBox::new(ctrls.action_panel.combo_hwnd()).set_selected_index(0);
                     for &id in &self.pending_ipc_ids {
                         if let Some(row) = self.find_ui_row_by_id(id) {
                             ctrls.file_list.set_selected(row, true);
                         }
                     }
                }
                SetTimer(hwnd, 2, 500, None);
            }
        }
    }

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
                                5 => Some(WofAlgorithm::Lznt1),
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

    unsafe fn handle_timer(&mut self, hwnd: HWND, wparam: WPARAM) -> LRESULT {
        unsafe {
            if wparam == 2 {
                KillTimer(hwnd, 2);
                self.ipc_active = false;
                
                if !self.pending_ipc_ids.is_empty() {
                     if let Some(ctrls) = &self.controls {
                         ctrls.file_list.deselect_all();
                         for &id in &self.pending_ipc_ids {
                             if let Some(row) = self.find_ui_row_by_id(id) {
                                 ctrls.file_list.set_selected(row, true);
                             }
                         }
                     }
                     self.pending_ipc_ids.clear();
                }
                SendMessageW(hwnd, WM_COMMAND, IDC_BTN_PROCESS_ALL as usize, 1);
                return 0;
            }
            
            loop {
                match self.rx.try_recv() {
                    Ok(msg) => self.process_ui_message(hwnd, msg),
                    Err(_) => break,
                }
            }
            0
        }
    }

    unsafe fn process_ui_message(&mut self, hwnd: HWND, msg: UiMessage) {
        unsafe {
            match msg {
                 UiMessage::Progress(cur, total) => {
                     if let Some(ctrls) = &self.controls {
                         let pb = ProgressBar::new(ctrls.status_bar.progress_hwnd());
                         pb.set_range(0, total as i32);
                         pb.set_pos(cur as i32);
                         
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
                     if !self.processing_queue.is_empty() {
                         let max = self.config.max_concurrent_items as usize;
                         let next_indices: Vec<usize> = if max > 0 && self.processing_queue.len() > max {
                             self.processing_queue.drain(0..max).collect()
                         } else {
                             self.processing_queue.drain(..).collect()
                         };
                         
                         handlers::start_processing_internal(self, hwnd, next_indices);
                         
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
                         self.global_state.store(crate::ui::state::ProcessingState::Idle as u8, std::sync::atomic::Ordering::Relaxed);
                         if let Some(tb) = &self.taskbar { tb.set_state(TaskbarState::NoProgress); }
                         if let Some(ctrls) = &self.controls {
                             EnableWindow(ctrls.action_panel.cancel_hwnd(), FALSE);
                             ctrls.file_list.redraw_all();
                         }
                         handlers::update_process_button_state(self);

                         if crate::types::GetForegroundWindow() != hwnd {
                             flash_window(hwnd);
                         }
                     }
                 },
                 UiMessage::ScanProgress(id, logical, disk, count) => {
                     if let Some(pos) = self.batch_items.iter().position(|item| item.id == id) {
                         if let Some(item) = self.batch_items.get_mut(pos) {
                             item.logical_size = logical;
                             item.disk_size = disk;
                             let current_state = self.global_state.load(Ordering::Relaxed);
                             if current_state == ProcessingState::Stopped as u8 {
                                 item.status_override = Some("Cancelled".to_string());
                             } else if current_state == ProcessingState::Paused as u8 {
                                 item.status_override = Some("Paused".to_string());
                             } else {
                                 item.status_override = Some(format!("Scanning... {}", count));
                             }
                         }
                         if let Some(row) = self.find_ui_row_by_id(id) {
                             if let Some(ctrls) = &self.controls { ctrls.file_list.redraw_item(row); }
                         }
                         if let Some(ctrls) = &self.controls {
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
                 UiMessage::RowProgress(id, cur, tot, bytes) => {
                     if let Some(pos) = self.batch_items.iter().position(|i| i.id == id) {
                         if let Some(item) = self.batch_items.get_mut(pos) {
                             item.progress = (cur, tot);
                             if bytes > 0 { item.disk_size = bytes; }
                             item.status_override = None;
                             item.status = BatchStatus::Processing;
                         }
                     }
                     if let Some(row) = self.find_ui_row_by_id(id) {
                         if let Some(ctrls) = &self.controls { ctrls.file_list.redraw_item(row); }
                     }
                 },
                 UiMessage::RowFinished(id, final_bytes, total_count, final_state) => {
                     if let Some(pos) = self.batch_items.iter().position(|i| i.id == id) {
                         if let Some(item) = self.batch_items.get_mut(pos) {
                             item.disk_size = final_bytes;
                             item.status = BatchStatus::Complete;
                             item.state_flag = None;
                             item.progress = (total_count, total_count);
                             item.status_override = None;
                             item.final_state = Some(final_state);
                         }
                     }
                     if let Some(row) = self.find_ui_row_by_id(id) {
                         if let Some(ctrls) = &self.controls { ctrls.file_list.redraw_item(row); }
                     }
                     handlers::update_process_button_state(self);
                 },
                 UiMessage::WatcherTrigger(path, algo) => {
                     if !self.batch_items.iter().any(|item| item.path == path) {
                         let id = self.add_batch_item(path.clone());
                         self.set_item_algorithm(id, algo);
                         
                         self.refresh_file_list();
                         
                         let original_idx = if let Some(ctrls) = &self.controls {
                             let combo = crate::ui::wrappers::ComboBox::new(ctrls.action_panel.combo_hwnd());
                             let idx = combo.get_selected_index();
                             combo.set_selected_index(0);
                             idx
                         } else { 0 };
                         
                         if let Some(row) = self.find_ui_row_by_id(id) {
                             handlers::start_processing(self, hwnd, vec![row as usize]);
                         }
                         
                         if let Some(ctrls) = &self.controls {
                             crate::ui::wrappers::ComboBox::new(ctrls.action_panel.combo_hwnd()).set_selected_index(original_idx);
                         }
                     }
                 },
                 UiMessage::BatchItemAnalyzed(id, log, disk, state) => {
                     if let Some(pos) = self.batch_items.iter().position(|item| item.id == id) {
                         if let Some(item) = self.batch_items.get_mut(pos) {
                             item.logical_size = log;
                             item.disk_size = disk;
                             item.status_override = None;
                             item.final_state = Some(state);
                         }
                         if let Some(row) = self.find_ui_row_by_id(id) {
                             if let Some(ctrls) = &self.controls { ctrls.file_list.redraw_item(row); }
                         }
                         if let Some(ctrls) = &self.controls {
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
                             item.cache_estimate(algo, est_size);
                         }
                         if let Some(row) = self.find_ui_row_by_id(id) {
                             if let Some(ctrls) = &self.controls { ctrls.file_list.redraw_item(row); }
                         }
                     }
                     self.update_accuracy_label();
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
                     use crate::ui::layout::{LayoutNode, SizePolicy::{Fixed, Flex}, AlignItems, JustifyContent};

                     LayoutNode::col(0, 0)
                        .with_child(LayoutNode::row(10, 10)
                            .justify_content(JustifyContent::SpaceBetween)
                            .align_items(AlignItems::Center)
                            .with(ctrls.status_bar.label_hwnd(), Flex(1.0))
                            .with(ctrls.header_panel.hwnd(), Fixed(170))
                            .with_policy(Fixed(50))
                        )
                        .with(ctrls.search_panel.panel_hwnd(), Fixed(85))
                        .with_child(LayoutNode::col(10, 0)
                             .with(ctrls.file_list.hwnd(), Flex(1.0))
                             .with_policy(Flex(1.0))
                        )
                        .with_child(LayoutNode::col(10, 5)
                             .with(ctrls.status_bar.progress_hwnd(), Fixed(22))
                             .with_policy(Fixed(32))
                        )
                        .with(ctrls.action_panel.hwnd(), Fixed(60))
                        .apply_layout(client_rect);
                        
                     ctrls.header_panel.refresh_layout();
                     ctrls.search_panel.refresh_layout();
                     ctrls.action_panel.refresh_layout();
                     
                     ctrls.file_list.update_columns();
                }
            }
            0
        }
    }

    unsafe fn update_accuracy_label(&self) {
        if let Some(ctrls) = &self.controls {
            let (mut sum_est, mut sum_disk) = (0u64, 0u64);
            for item in &self.batch_items {
                if item.estimated_size > 0 && item.disk_size > 0 && item.disk_size < item.logical_size {
                    sum_est += item.estimated_size;
                    sum_disk += item.disk_size;
                }
            }
            
            let text = if sum_disk > 0 && sum_est > 0 {
                let accuracy = if sum_est > sum_disk {
                    (sum_disk as f64 / sum_est as f64) * 100.0
                } else {
                    (sum_est as f64 / sum_disk as f64) * 100.0
                };
                crate::utils::to_wstring(&format!("Acc: {:.0}%", accuracy))
            } else {
                crate::utils::to_wstring("Acc: --")
            };
            
            crate::ui::wrappers::Label::new(ctrls.action_panel.accuracy_hwnd()).set_text_w(&text);
        }
    }

    unsafe fn on_global_algo_changed(&mut self) {
        let algo = if let Some(ctrls) = &self.controls {
                let idx = ComboBox::new(ctrls.action_panel.combo_hwnd()).get_selected_index();
                match idx {
                    0 => None,
                    1 => Some(WofAlgorithm::Xpress4K),
                    2 => Some(WofAlgorithm::Xpress8K),
                    3 => Some(WofAlgorithm::Xpress16K),
                    4 => Some(WofAlgorithm::Lzx),
                    5 => Some(WofAlgorithm::Lznt1),
                    _ => Some(WofAlgorithm::Xpress8K),
                }
            } else {
                return;
            };

            let mut items_to_estimate: Vec<(u32, String, WofAlgorithm)> = Vec::new();
            
            for item in self.batch_items.iter_mut() {
                let effective_algo = algo.unwrap_or(item.algorithm);
                if let Some(cached) = item.get_cached_estimate(effective_algo) {
                    item.estimated_size = cached;
                } else {
                    items_to_estimate.push((item.id, item.path.clone(), effective_algo));
                    item.estimated_size = 0; // Clears it to "Estimating..."
                }
            }

            if let Some(ctrls) = &self.controls {
                ctrls.file_list.redraw_all();
            }

            if items_to_estimate.is_empty() {
                return;
            }

            let tx = self.tx.clone();
            std::thread::spawn(move || {
                for (id, path, algo) in items_to_estimate {
                    let estimated = crate::engine::estimator::estimate_path(&path, algo);
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
                let algo_idx = ComboBox::new(ctrls.action_panel.combo_hwnd()).get_selected_index();
                self.config.combo_algo_index = if algo_idx >= 0 { algo_idx as u8 } else { 0 };
                
                let action_idx = ComboBox::new(ctrls.action_panel.action_mode_hwnd()).get_selected_index();
                self.config.combo_action_index = if action_idx >= 0 { action_idx as u8 } else { 0 };

                self.config.force_compress = Button::new(ctrls.action_panel.force_hwnd()).is_checked();
            }
            self.config.theme = self.theme;
            self.config.enable_force_stop = self.enable_force_stop;
            self.config.low_power_mode = self.low_power_mode;
            self.config.save();
            
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
                         "lznt1" => WofAlgorithm::Lznt1,
                         "lzx" => WofAlgorithm::Lzx,
                         _ => WofAlgorithm::Xpress8K,
                     };
                     let action = match parts[2] {
                         "decompress" => BatchAction::Decompress,
                         _ => BatchAction::Compress,
                     };
                     
                     if !self.batch_items.iter().any(|item| item.path == path) {
                         self.ingest_paths(vec![path.clone()]);
                         
                         if let Some(pos) = self.batch_items.iter().position(|i| i.path == path) {
                              let id = self.batch_items[pos].id;
                              if let Some(item) = self.batch_items.get_mut(pos) {
                                   item.algorithm = algo;
                                   item.action = action;
                              }
                              
                              if let Some(ctrls) = &self.controls {
                                   ComboBox::new(ctrls.action_panel.combo_hwnd()).set_selected_index(0);
                                   if !self.ipc_active {
                                       ctrls.file_list.deselect_all();
                                   }
                                   if let Some(row) = self.find_ui_row_by_id(id) {
                                       ctrls.file_list.set_selected(row, true);
                                   }
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
            if crate::ui::handlers::should_block_header_resize(lparam) {
                 return 1; 
            }
            
            let nmhdr = &*(lparam as *const NMHDR);
            if nmhdr.idFrom == IDC_BATCH_LIST as usize {
                if nmhdr.code == LVN_GETDISPINFOW {
                    let pdi = lparam as *mut NMLVDISPINFOW;
                    let row = (*pdi).item.iItem as usize;
                    let col = (*pdi).item.iSubItem;

                    if row < self.filtered_items.len() {
                        let item_idx = self.filtered_items[row];
                        if let Some(item) = self.batch_items.get(item_idx) {
                            let mut text: Option<Vec<u16>> = None;
                            match col {
                                0 => text = Some(to_wstring(&item.path)),
                                1 => {
                                    let state = item.final_state.unwrap_or(CompressionState::None);
                                    text = Some(match state {
                                        CompressionState::None => w!("-").to_vec(),
                                        CompressionState::Specific(algo) => match algo {
                                            WofAlgorithm::Xpress4K => w!("XPRESS4K").to_vec(),
                                            WofAlgorithm::Xpress8K => w!("XPRESS8K").to_vec(),
                                            WofAlgorithm::Xpress16K => w!("XPRESS16K").to_vec(),
                                            WofAlgorithm::Lzx => w!("LZX").to_vec(),
                                            WofAlgorithm::Lznt1 => w!("LZNT1").to_vec(),
                                        },
                                        CompressionState::Mixed => w!("Mixed").to_vec(),
                                    });
                                },
                                2 => {
                                    text = Some(match item.algorithm {
                                        WofAlgorithm::Xpress4K => w!("XPRESS4K").to_vec(),
                                        WofAlgorithm::Xpress8K => w!("XPRESS8K").to_vec(),
                                        WofAlgorithm::Xpress16K => w!("XPRESS16K").to_vec(),
                                        WofAlgorithm::Lzx => w!("LZX").to_vec(),
                                        WofAlgorithm::Lznt1 => w!("LZNT1").to_vec(),
                                    });
                                },
                                3 => text = Some(if item.action == BatchAction::Compress { w!("Compress").to_vec() } else { w!("Decompress").to_vec() }),
                                4 => text = Some(crate::utils::format_size(item.logical_size)),
                                5 => {
                                    if item.estimated_size > 0 {
                                        text = Some(crate::utils::format_size(item.estimated_size));
                                    } else {
                                        text = Some(w!("Estimating...").to_vec());
                                    }
                                },
                                6 => {
                                    if item.disk_size > 0 {
                                         text = Some(crate::utils::format_size(item.disk_size));
                                    } else {
                                         text = Some(w!("-").to_vec());
                                    }
                                },
                                7 => text = Some(crate::utils::calculate_ratio_string(item.logical_size, item.disk_size)),
                                8 => {
                                    let cur_w = u64_to_wstring(item.progress.0);
                                    let tot_w = u64_to_wstring(item.progress.1);
                                    text = Some(crate::utils::concat_wstrings(&[&cur_w, w!(" / "), &tot_w]));
                                },
                                9 => {
                                    if let Some(ref override_msg) = item.status_override {
                                        text = Some(to_wstring(override_msg));
                                    } else {
                                        let st = match &item.status {
                                            BatchStatus::Pending => w!("Pending").to_vec(),
                                            BatchStatus::Processing => w!("Processing").to_vec(),
                                            BatchStatus::Complete => w!("Complete").to_vec(),
                                            BatchStatus::Error(_) => w!("Error").to_vec(),
                                        };
                                        text = Some(st);
                                    }
                                },
                                10 => {
                                    let is_complete = item.status == BatchStatus::Complete;
                                    let global_state = ProcessingState::from_u8(self.global_state.load(std::sync::atomic::Ordering::Relaxed));
                                    let t = if is_complete {
                                        crate::utils::to_wstring("\u{1F441}    ▶")
                                    } else {
                                        match global_state {
                                            ProcessingState::Idle | ProcessingState::Stopped => crate::utils::to_wstring("\u{1F441}    ▶"),
                                            ProcessingState::Running => {
                                                if item.status == BatchStatus::Processing {
                                                    crate::utils::to_wstring("\u{1F441}    \u{23F8}   \u{23F9}")
                                                } else {
                                                    crate::utils::to_wstring("\u{1F441}    ▶")
                                                }
                                            },
                                            ProcessingState::Paused => {
                                                if item.status == BatchStatus::Processing || item.status == BatchStatus::Pending {
                                                    crate::utils::to_wstring("\u{1F441}    \u{25B6}   \u{23F9}")
                                                } else {
                                                    crate::utils::to_wstring("\u{1F441}    ▶")
                                                }
                                            },
                                        }
                                    };
                                    text = Some(t);
                                },
                                _ => {}
                            }

                            if let Some(mut t) = text {
                                if ((*pdi).item.mask & LVIF_TEXT) != 0 {
                                    let max_len = (*pdi).item.cchTextMax as usize;
                                    if max_len > 0 {
                                        if t.last() != Some(&0) { t.push(0); }
                                        let copy_len = std::cmp::min(t.len(), max_len);
                                        std::ptr::copy_nonoverlapping(t.as_ptr(), (*pdi).item.pszText, copy_len);
                                        if copy_len > 0 {
                                            *((*pdi).item.pszText.add(copy_len - 1)) = 0;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return 0; // handled
                } else if nmhdr.code == NM_CLICK || nmhdr.code == NM_DBLCLK {
                    let nmia = &*(lparam as *const NMITEMACTIVATE);
                    handlers::on_list_click(self, hwnd, nmia.iItem, nmia.iSubItem, nmhdr.code);
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
                    if let Some(ctrls) = &self.controls {
                        let is_dark = theme::resolve_mode(self.theme);
                        let list_hwnd = ctrls.file_list.hwnd();
                        if let Some(result) = crate::ui::components::file_list::handle_listview_customdraw(
                            list_hwnd, lparam, is_dark, &self.batch_items, &self.filtered_items
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

    unsafe fn handle_keydown(&mut self, hwnd: HWND, wparam: WPARAM, source_hwnd: HWND) -> LRESULT {
        unsafe {
            use crate::ui::input::{resolve_key_action, handle_global_shortcut};
            let action = resolve_key_action(wparam as i32);
            handle_global_shortcut(hwnd, self, action, source_hwnd);
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
             
             if self.ignored_lock_processes.contains(&name) {
                 return false;
             }
             
             if self.active_lock_dialog.as_deref() == Some(&name) {
                 return false;
             }
             
             self.active_lock_dialog = Some(name.clone());
             
             let is_dark = theme::resolve_mode(self.theme);
             let result = crate::ui::dialogs::show_force_stop_dialog(hwnd, &name, is_dark);
             
             self.active_lock_dialog = None;
             
             if !result {
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