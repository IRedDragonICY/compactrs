/* --- src/ui/window.rs --- */
#![allow(unsafe_op_in_unsafe_fn)]

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM, RECT, TRUE, FALSE};
use windows_sys::Win32::Graphics::Gdi::InvalidateRect;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CW_USEDEFAULT, WS_OVERLAPPEDWINDOW, WS_VISIBLE, WM_SIZE, WM_COMMAND,
    WM_DROPFILES, SendMessageW, CB_ADDSTRING, CB_SETCURSEL, CB_GETCURSEL, SetWindowTextW, WM_TIMER, SetTimer,
    WM_NOTIFY, BM_GETCHECK, GetClientRect, GetWindowRect, BM_SETCHECK, 
    ChangeWindowMessageFilterEx, MSGFLT_ALLOW, WM_COPYDATA, WM_CONTEXTMENU, 
    SetForegroundWindow, GetForegroundWindow, GetWindowThreadProcessId, BringWindowToTop, 
    WM_DESTROY, KillTimer, WM_SETTINGCHANGE,
};
use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Shell::DragAcceptFiles;
use windows_sys::Win32::UI::Controls::{
    PBM_SETRANGE32, PBM_SETPOS, NMITEMACTIVATE, NMHDR, NM_CLICK, NM_DBLCLK,
    InitCommonControlsEx, INITCOMMONCONTROLSEX, ICC_WIN95_CLASSES, ICC_STANDARD_CLASSES,
    LVN_ITEMCHANGED, BST_CHECKED, LVN_KEYDOWN, NMLVKEYDOWN, LVN_COLUMNCLICK,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_SHIFT, VK_DELETE};

#[link(name = "user32")]
unsafe extern "system" {
    fn AttachThreadInput(idAttach: u32, idAttachTo: u32, fAttach: i32) -> i32;
}

use crate::ui::controls::*;
use crate::ui::components::{
    Component, FileListView, StatusBar, StatusBarIds, ActionPanel, ActionPanelIds,
    HeaderPanel, HeaderPanelIds,
};
// FIX: Added BatchAction
use crate::ui::state::{AppState, Controls, UiMessage, BatchStatus, AppTheme, BatchAction};
use crate::ui::taskbar::{TaskbarProgress, TaskbarState};
use crate::ui::theme;
use crate::ui::handlers; 
use crate::engine::wof::{WofAlgorithm, CompressionState};
use crate::utils::{to_wstring, u64_to_wstring, concat_wstrings, format_size};
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
        InitCommonControlsEx(&iccex);

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
        let state = Box::new(AppState::new());
        let state_ref = Box::leak(state);

        let hwnd = WindowBuilder::new(state_ref, WINDOW_CLASS_NAME, &title_str)
            .style(WS_OVERLAPPEDWINDOW | WS_VISIBLE)
            .size(win_width, win_height)
            .align(WindowAlignment::Manual(win_x, win_y))
            .icon(icon)
            .background(bg_brush)
            .build(std::ptr::null_mut())?;

        // Hostile Takeover: Force window to foreground (Bypass ASLR/Focus restrictions)
        let foreground_hwnd = GetForegroundWindow();
        if !foreground_hwnd.is_null() {
            let foreground_thread = GetWindowThreadProcessId(foreground_hwnd, std::ptr::null_mut());
            let current_thread = GetCurrentThreadId();
            
            if foreground_thread != current_thread {
                AttachThreadInput(foreground_thread, current_thread, TRUE);
                BringWindowToTop(hwnd);
                SetForegroundWindow(hwnd);
                AttachThreadInput(foreground_thread, current_thread, FALSE);
            } else {
                SetForegroundWindow(hwnd);
            }
        } else {
            SetForegroundWindow(hwnd);
        }

        Ok(hwnd)
    }
}

impl WindowHandler for AppState {
    fn is_dark_mode(&self) -> bool {
        theme::resolve_mode(self.theme)
    }

    fn on_create(&mut self, hwnd: HWND) -> LRESULT {
        unsafe {
             // Taskbar creation
            self.taskbar = Some(TaskbarProgress::new(hwnd));
            
            // 1. StatusBar
            let mut status_bar = StatusBar::new(StatusBarIds {
                label_id: IDC_STATIC_TEXT,
                progress_id: IDC_PROGRESS_BAR,
            });
            let _ = status_bar.create(hwnd);
            
            // 2. FileListView
            let file_list = FileListView::new(hwnd, 10, 40, 860, 380, IDC_BATCH_LIST);
            
            // 3. ActionPanel
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
            });
            let _ = action_panel.create(hwnd);
            
            // 4. HeaderPanel
            let mut header_panel = HeaderPanel::new(HeaderPanelIds {
                btn_settings: IDC_BTN_SETTINGS,
                btn_about: IDC_BTN_ABOUT,
                btn_shortcuts: IDC_BTN_SHORTCUTS,
                btn_console: IDC_BTN_CONSOLE,
            });
            let _ = header_panel.create(hwnd);
            
            // Disable cancel button initially
            windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(action_panel.cancel_hwnd(), 0);

            // Populate Combos
            self.populate_ui_combos(&action_panel);

            self.controls = Some(Controls {
                file_list,
                status_bar,
                action_panel,
                header_panel,
            });

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
        }
        0
    }

    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            // Accent Button Drawing
            const WM_DRAWITEM: u32 = 0x002B;
            if msg == WM_DRAWITEM {
                use windows_sys::Win32::UI::Controls::DRAWITEMSTRUCT;
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
                
                WM_COMMAND => Some(self.dispatch_command(hwnd, wparam, lparam)),
                WM_TIMER => Some(self.handle_timer(hwnd, wparam)),
                // FIX: handle_resize -> handle_size
                WM_SIZE => Some(self.handle_size(hwnd)),
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
                
                _ => None,
            }
        }
    }
}

// Logic Helper Functions (Extensions to AppState for Window Logic)

impl AppState {
    /// Populate initial values for comboboxes and checkboxes
    unsafe fn populate_ui_combos(&self, action_panel: &ActionPanel) {
        // Algorithm Combo
        let h_combo = action_panel.combo_hwnd();
        let algos = ["As Listed", "XPRESS4K", "XPRESS8K", "XPRESS16K", "LZX"];
        unsafe {
            for alg in algos {
                let w = to_wstring(alg);
                SendMessageW(h_combo, CB_ADDSTRING, 0, w.as_ptr() as isize);
            }
            let algo_index = match self.config.default_algo {
                WofAlgorithm::Xpress4K => 1,
                WofAlgorithm::Xpress8K => 2,
                WofAlgorithm::Xpress16K => 3,
                WofAlgorithm::Lzx => 4,
            };
            SendMessageW(h_combo, CB_SETCURSEL, algo_index as usize, 0);
            
            // Action Mode Combo
            let h_action_mode = action_panel.action_mode_hwnd();
            let action_modes = ["As Listed", "Compress All", "Decompress All"];
            for mode in action_modes {
                let w = to_wstring(mode);
                SendMessageW(h_action_mode, CB_ADDSTRING, 0, w.as_ptr() as isize);
            }
            SendMessageW(h_action_mode, CB_SETCURSEL, 0, 0);
            
            // Force Checkbox
            if self.force_compress {
                SendMessageW(action_panel.force_hwnd(), BM_SETCHECK, BST_CHECKED as usize, 0);
            }
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
                                    ctrls.file_list.add_item(item_id, batch_item, logical_str, disk_str, to_wstring("Estimating..."), detected_algo);
                                    if let Some(pos) = self.batch_items.iter().position(|i| i.id == item_id) {
                                        ctrls.file_list.set_selected(pos as i32, true);
                                        self.pending_ipc_ids.push(item_id);
                                    }
                                }
                            }
                     }
                }
                
                if let Some(ctrls) = &self.controls {
                     SendMessageW(ctrls.action_panel.combo_hwnd(), CB_SETCURSEL, 0, 0);
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
                       crate::ui::dialogs::show_console_window(hwnd, &self.logs, is_dark);
                 },
                 IDC_CHK_FORCE => {
                       let hwnd_ctl = lparam as HWND;
                       let state = SendMessageW(hwnd_ctl, BM_GETCHECK, 0, 0);
                       self.force_compress = state as u32 == BST_CHECKED;
                 },
                 IDC_COMBO_ALGO => {
                     // Re-estimate all items when global algorithm changes
                     if notification_code == CBN_SELCHANGE {
                         self.on_global_algo_changed();
                     }
                 },
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
                    Ok(msg) => self.process_ui_message(msg),
                    Err(_) => break,
                }
            }
            0
        }
    }

    /// Process messages from worker threads and update UI
    unsafe fn process_ui_message(&mut self, msg: UiMessage) {
        unsafe {
            match msg {
                 UiMessage::Progress(cur, total) => {
                     if let Some(ctrls) = &self.controls {
                         SendMessageW(ctrls.status_bar.progress_hwnd(), PBM_SETRANGE32, 0, total as isize);
                         SendMessageW(ctrls.status_bar.progress_hwnd(), PBM_SETPOS, cur as usize, 0);
                     }
                     if let Some(tb) = &self.taskbar { tb.set_value(cur, total); }
                 },
                 UiMessage::Status(text) => {
                     if let Some(ctrls) = &self.controls {
                         SetWindowTextW(ctrls.status_bar.label_hwnd(), text.as_ptr());
                     }
                 },
                 UiMessage::Log(text) => {
                     self.logs.push(text.clone());
                     crate::ui::dialogs::append_log_msg(text);
                 },
                 UiMessage::Error(text) => {
                     if let Some(tb) = &self.taskbar { tb.set_state(TaskbarState::Error); }
                     let err_prefix = to_wstring("ERROR: ");
                     let full_err = concat_wstrings(&[&err_prefix, &text]);
                     self.logs.push(full_err.clone());
                     crate::ui::dialogs::append_log_msg(full_err);
                     
                     if let Some(ctrls) = &self.controls {
                         SetWindowTextW(ctrls.status_bar.label_hwnd(), text.as_ptr());
                     }
                 },
                 UiMessage::Finished => {
                     if let Some(tb) = &self.taskbar { tb.set_state(TaskbarState::NoProgress); }
                     if let Some(ctrls) = &self.controls {
                         windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(ctrls.action_panel.cancel_hwnd(), 0);
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
                             let count_str = u64_to_wstring(count);
                             
                             ctrls.file_list.update_item_text(pos as i32, 4, log_str);
                             ctrls.file_list.update_item_text(pos as i32, 6, disk_str);
                             
                             let status_text = concat_wstrings(&[&to_wstring("Scanning... "), &count_str]);
                             ctrls.file_list.update_item_text(pos as i32, 8, status_text);

                             if let Some(item) = self.batch_items.get(pos) {
                                 let sb_msg = concat_wstrings(&[
                                     &to_wstring("Scanning: "), 
                                     &to_wstring(&item.path), 
                                     &to_wstring("... ("), 
                                     &count_str, 
                                     &to_wstring(" files)")
                                 ]);
                                 SetWindowTextW(ctrls.status_bar.label_hwnd(), sb_msg.as_ptr());
                             }
                         }
                     }
                 },
                 UiMessage::RowUpdate(row, progress, status, _) => {
                     if let Some(ctrls) = &self.controls {
                         ctrls.file_list.update_item_text(row, 7, progress);
                         ctrls.file_list.update_item_text(row, 8, status);
                     }
                 },
                 UiMessage::ItemFinished(row, status, disk_size, final_state) => {
                     if let Some(ctrls) = &self.controls {
                         ctrls.file_list.update_item_text(row, 8, status);
                         if !disk_size.is_empty() && disk_size.len() > 1 { ctrls.file_list.update_item_text(row, 6, disk_size); }
                         let state_str = match final_state {
                             CompressionState::None => "-",
                             CompressionState::Specific(algo) => match algo {
                                 WofAlgorithm::Xpress4K => "XPRESS4K", WofAlgorithm::Xpress8K => "XPRESS8K",
                                 WofAlgorithm::Xpress16K => "XPRESS16K", WofAlgorithm::Lzx => "LZX",
                             },
                             CompressionState::Mixed => "Mixed",
                         };
                         ctrls.file_list.update_item_text(row, 1, to_wstring(state_str));
                         ctrls.file_list.update_item_text(row, 9, to_wstring("▶ Start"));
                         if let Some(item) = self.batch_items.get_mut(row as usize) {
                             item.status = BatchStatus::Pending;
                             item.state_flag = None;
                         }
                     }
                 },
                 UiMessage::BatchItemAnalyzed(id, log, disk, state) => {
                     let log_str = format_size(log);
                     let disk_str = format_size(disk);
                     if let Some(pos) = self.batch_items.iter().position(|item| item.id == id) {
                         if let Some(item) = self.batch_items.get_mut(pos) {
                             item.logical_size = log;
                             item.disk_size = disk;
                         }
                         
                         if let Some(ctrls) = &self.controls {
                             ctrls.file_list.update_item_text(pos as i32, 4, log_str);
                             ctrls.file_list.update_item_text(pos as i32, 6, disk_str);
                             let state_str = match state {
                                CompressionState::None => "-",
                                CompressionState::Specific(algo) => match algo {
                                    WofAlgorithm::Xpress4K => "XPRESS4K", WofAlgorithm::Xpress8K => "XPRESS8K",
                                    WofAlgorithm::Xpress16K => "XPRESS16K", WofAlgorithm::Lzx => "LZX",
                                },
                                CompressionState::Mixed => "Mixed",
                             };
                             ctrls.file_list.update_item_text(pos as i32, 1, to_wstring(state_str));
                             ctrls.file_list.update_item_text(pos as i32, 8, to_wstring("Pending"));
                             let count = self.batch_items.len();
                             let count_w = u64_to_wstring(count as u64);
                             let msg = concat_wstrings(&[&count_w, &to_wstring(" item(s) analyzed.")]);
                             SetWindowTextW(ctrls.status_bar.label_hwnd(), msg.as_ptr());
                         }
                     }
                 },
                 UiMessage::UpdateEstimate(id, algo, est_size, est_str) => {
                     if let Some(pos) = self.batch_items.iter().position(|item| item.id == id) {
                         if let Some(item) = self.batch_items.get_mut(pos) {
                             // Cache the result for this algorithm
                             item.cache_estimate(algo, est_size);
                         }
                         if let Some(ctrls) = &self.controls {
                             ctrls.file_list.update_item_text(pos as i32, 5, est_str);
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
                     ctrls.status_bar.on_resize(&client_rect);
                     ctrls.file_list.on_resize(&client_rect);
                     ctrls.action_panel.on_resize(&client_rect);
                     ctrls.header_panel.on_resize(&client_rect);
                }
            }
            0
        }
    }

    /// Re-estimate all items when the global algorithm ComboBox changes
    unsafe fn on_global_algo_changed(&mut self) {
        unsafe {
            // Get the selected algorithm from the ComboBox
            let algo = if let Some(ctrls) = &self.controls {
                let idx = SendMessageW(ctrls.action_panel.combo_hwnd(), CB_GETCURSEL, 0, 0);
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
                        ctrls.file_list.update_item_text(i as i32, 5, est_str);
                    }
                } else {
                    // Need to calculate
                    items_to_estimate.push((item.id, item.path.clone(), effective_algo));
                    if let Some(ctrls) = &self.controls {
                        ctrls.file_list.update_item_text(i as i32, 5, to_wstring("Estimating..."));
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
                    let est_str = crate::utils::format_size(estimated);
                    let _ = tx.send(UiMessage::UpdateEstimate(id, algo, estimated, est_str));
                }
            });
        }
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
                let idx = SendMessageW(ctrls.action_panel.combo_hwnd(), CB_GETCURSEL, 0, 0);
                self.config.default_algo = match idx {
                    1 => WofAlgorithm::Xpress4K,
                    3 => WofAlgorithm::Xpress16K, 
                    4 => WofAlgorithm::Lzx,
                    _ => WofAlgorithm::Xpress8K,
                };
                let force = SendMessageW(ctrls.action_panel.force_hwnd(), BM_GETCHECK, 0, 0);
                self.config.force_compress = force as u32 == BST_CHECKED;
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
                                   SendMessageW(ctrls.action_panel.combo_hwnd(), CB_SETCURSEL, 0, 0); // Reset Global Combo
                                   
                                   // Update UI for Algo/Action
                                   let algo_name = match algo {
                                       WofAlgorithm::Xpress4K => "XPRESS4K", WofAlgorithm::Xpress8K => "XPRESS8K",
                                       WofAlgorithm::Xpress16K => "XPRESS16K", WofAlgorithm::Lzx => "LZX",
                                   };
                                   let action_name = match action {
                                       BatchAction::Compress => "Compress", BatchAction::Decompress => "Decompress",
                                   };
                                   ctrls.file_list.update_item_text(pos as i32, 2, to_wstring(algo_name));
                                   ctrls.file_list.update_item_text(pos as i32, 3, to_wstring(action_name));

                                   if !self.ipc_active {
                                       let count = ctrls.file_list.get_item_count();
                                       for i in 0..count {
                                           ctrls.file_list.set_selected(i, false);
                                       }
                                       self.ipc_active = true;
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
                    if let Some(ctrls) = &self.controls {
                        let count = ctrls.file_list.get_selection_count();
                        let text = if count > 0 { "Process Selected" } else { "Process All" };
                        SetWindowTextW(ctrls.action_panel.process_hwnd(), to_wstring(text).as_ptr());
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
    
    unsafe fn handle_force_stop_request(&self, hwnd: HWND, wparam: WPARAM) -> bool {
         unsafe {
             if self.enable_force_stop { return true; }
             
             let name_ptr = wparam as *const u16;
             let len = (0..).take_while(|&i| *name_ptr.offset(i) != 0).count();
             let slice = std::slice::from_raw_parts(name_ptr, len);
             let name = String::from_utf16_lossy(slice);
             
             let is_dark = theme::resolve_mode(self.theme);
             crate::ui::dialogs::show_force_stop_dialog(hwnd, &name, is_dark)
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