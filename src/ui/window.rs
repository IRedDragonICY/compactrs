#![allow(unsafe_op_in_unsafe_fn)]

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM, RECT, POINT};
use windows_sys::core::HRESULT;
use windows_sys::Win32::Graphics::Gdi::InvalidateRect;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CW_USEDEFAULT, WS_OVERLAPPEDWINDOW, WS_VISIBLE, WM_SIZE, WM_COMMAND,
    WM_DROPFILES, MessageBoxW, MB_OK,
    SendMessageW, CB_ADDSTRING, CB_SETCURSEL, CB_GETCURSEL, SetWindowTextW, WM_TIMER, SetTimer,
    MB_ICONINFORMATION, WM_NOTIFY, BM_GETCHECK, GetClientRect, GetWindowRect,
    BM_SETCHECK, ChangeWindowMessageFilterEx, MSGFLT_ALLOW, WM_COPYDATA,
    WM_CONTEXTMENU, TrackPopupMenu, CreatePopupMenu, AppendMenuW, MF_STRING, TPM_RETURNCMD, TPM_LEFTALIGN,
    DestroyMenu, GetCursorPos, WM_SETTINGCHANGE, SW_SHOWNORMAL, SetForegroundWindow,
    GetForegroundWindow, GetWindowThreadProcessId, BringWindowToTop, WM_CLOSE, WM_DESTROY, DestroyWindow,
    KillTimer,
};
use windows_sys::core::{GUID, PCWSTR};
use windows_sys::Win32::System::DataExchange::{
    COPYDATASTRUCT, OpenClipboard, GetClipboardData, CloseClipboard, IsClipboardFormatAvailable,
};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::Foundation::{TRUE, FALSE};
use windows_sys::Win32::UI::Shell::{
    DragQueryFileW, DragFinish, HDROP, DragAcceptFiles,
    ShellExecuteW,
};
use windows_sys::Win32::UI::Controls::{
    PBM_SETRANGE32, PBM_SETPOS, NM_DBLCLK, NMITEMACTIVATE, NMHDR, NM_CLICK,
    InitCommonControlsEx, INITCOMMONCONTROLSEX, ICC_WIN95_CLASSES, ICC_STANDARD_CLASSES,
    LVN_ITEMCHANGED, BST_CHECKED, LVN_KEYDOWN, NMLVKEYDOWN, LVN_COLUMNCLICK, NMLISTVIEW,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_DELETE, VK_CONTROL, VK_SHIFT};

#[link(name = "user32")]
unsafe extern "system" {
    fn AttachThreadInput(idAttach: u32, idAttachTo: u32, fAttach: i32) -> i32;
}
use std::cmp::Ordering as CmpOrdering;
use windows_sys::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL};

use crate::ui::controls::{
    IDC_COMBO_ALGO, IDC_STATIC_TEXT, IDC_PROGRESS_BAR, IDC_BTN_CANCEL, IDC_BATCH_LIST,
    IDC_BTN_ADD_FOLDER, IDC_BTN_REMOVE, IDC_BTN_PROCESS_ALL, IDC_BTN_ADD_FILES,
    IDC_BTN_SETTINGS, IDC_BTN_ABOUT, IDC_BTN_SHORTCUTS, IDC_BTN_CONSOLE, IDC_CHK_FORCE, IDC_COMBO_ACTION_MODE,
    IDC_LBL_ACTION_MODE, IDC_LBL_ALGO, IDC_LBL_INPUT,
};
use crate::ui::components::{
    Component, FileListView, StatusBar, StatusBarIds, ActionPanel, ActionPanelIds,
    HeaderPanel, HeaderPanelIds,
};
use crate::ui::settings::show_settings_modal;
use crate::ui::about::show_about_modal;
use crate::ui::shortcuts::show_shortcuts_modal;
use crate::ui::console::{show_console_window, append_log_msg};
use crate::ui::state::{AppState, Controls, UiMessage, BatchAction, BatchStatus, AppTheme, ProcessingState};
use crate::ui::taskbar::{TaskbarProgress, TaskbarState};
use crate::ui::theme;
use std::thread;
use std::sync::atomic::Ordering;
use crate::engine::wof::{WofAlgorithm, CompressionState};
use crate::engine::worker::{
    batch_process_worker,
    calculate_path_logical_size, calculate_path_disk_size, detect_path_algorithm,
};
use crate::utils::{to_wstring, u64_to_wstring, concat_wstrings};
use crate::utils::format_size;
use crate::ui::framework::load_app_icon;
use crate::config::AppConfig;
use crate::ui::framework::{Window, WindowHandler};

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

        let hwnd = Window::<AppState>::create(
            state_ref,
            WINDOW_CLASS_NAME,
            &title_str,
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            0,
            win_x, win_y, win_width, win_height,
            std::ptr::null_mut(),
            icon,
            bg_brush
        )?;

        // Apply initial theme
        // Note: on_create already applied theme, but resolve_mode calls might vary.
        // Actually on_create logic (copied below) handles this.
        
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
            
            // Disable cancel button
            windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(action_panel.cancel_hwnd(), 0);

            // Populate algorithm combo
            let h_combo = action_panel.combo_hwnd();
            let algos = ["As Listed", "XPRESS4K", "XPRESS8K", "XPRESS16K", "LZX"];
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
            
            // Populate action mode combo
            let h_action_mode = action_panel.action_mode_hwnd();
            let action_modes = ["As Listed", "Compress All", "Decompress All"];
            for mode in action_modes {
                let w = to_wstring(mode);
                SendMessageW(h_action_mode, CB_ADDSTRING, 0, w.as_ptr() as isize);
            }
            SendMessageW(h_action_mode, CB_SETCURSEL, 0, 0);
            
            // Set initial force checkbox state
            if self.force_compress {
                SendMessageW(action_panel.force_hwnd(), BM_SETCHECK, BST_CHECKED as usize, 0);
            }

            self.controls = Some(Controls {
                file_list,
                status_bar,
                action_panel,
                header_panel,
            });

            // Timer
            SetTimer(hwnd, 1, 100, None);
            DragAcceptFiles(hwnd, 1);
            
            // Allow Drag and Drop messages
            ChangeWindowMessageFilterEx(hwnd, WM_DROPFILES, MSGFLT_ALLOW, std::ptr::null_mut());
            ChangeWindowMessageFilterEx(hwnd, WM_COPYDATA, MSGFLT_ALLOW, std::ptr::null_mut());
            ChangeWindowMessageFilterEx(hwnd, 0x0049, MSGFLT_ALLOW, std::ptr::null_mut()); 
            
            // Apply saved theme
            let is_dark = theme::resolve_mode(self.theme);
            theme::set_window_frame_theme(hwnd, is_dark);
            if let Some(ctrls) = &mut self.controls {
                ctrls.update_theme(is_dark, hwnd);
            }
            // Not strictly needed in on_create as painted first time, but good for consistency
            // InvalidateRect(hwnd, std::ptr::null(), 1); 
            
            // Process startup items
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
                            let logical_size = calculate_path_logical_size(&startup_item.path);
                            let disk_size = calculate_path_disk_size(&startup_item.path);
                            let detected_algo = detect_path_algorithm(&startup_item.path);
                            let logical_str = format_size(logical_size);
                            let disk_str = format_size(disk_size);
                            
                            if let Some(ctrls) = &self.controls {
                                if let Some(batch_item) = self.batch_items.iter().find(|i| i.id == item_id) {
                                    ctrls.file_list.add_item(item_id, batch_item, logical_str, disk_str, detected_algo);
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
        0
    }

    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        let is_dark = theme::resolve_mode(self.theme);
        
        unsafe {
            // Theme colors
            if let Some(result) = theme::handle_standard_colors(hwnd, msg, wparam, is_dark) {
                return Some(result);
            }
            
            // WM_DRAWITEM
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
                 // WM_APP + 3: Set Enable Force Stop
                0x8003 => {
                    self.enable_force_stop = wparam != 0;
                    Some(0)
                },
                
                // WM_APP + 6: Set System Guard
                0x8006 => {
                    self.config.enable_system_guard = wparam != 0;
                    Some(0)
                },
                
                // WM_APP + 4: Query Force Stop
                0x8004 => {
                    let should_kill = if self.enable_force_stop {
                          true
                     } else {
                         let name_ptr = wparam as *const u16;
                         let len = (0..).take_while(|&i| *name_ptr.offset(i) != 0).count();
                         let slice = std::slice::from_raw_parts(name_ptr, len);
                         let name = String::from_utf16_lossy(slice);
                         let is_dark = theme::resolve_mode(self.theme);
                         crate::ui::dialogs::show_force_stop_dialog(hwnd, &name, is_dark)
                     };
                     Some(if should_kill { 1 } else { 0 })
                },
                
                // WM_THEME_CHANGED
                0x8001 => {
                    let theme_val = wparam;
                    let new_theme = match theme_val {
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
                    Some(0)
                },
                
                WM_COMMAND => Some(handle_command(self, hwnd, wparam, lparam)),
                WM_TIMER => Some(handle_timer(self, hwnd, wparam)),
                WM_SIZE => Some(handle_size(self, hwnd)),
                WM_CLOSE => {
                    DestroyWindow(hwnd);
                    Some(0)
                },
                WM_DESTROY => {
                    handle_destroy(self, hwnd);
                    Some(0)
                },
                WM_COPYDATA => Some(handle_copy_data(self, hwnd, lparam)),
                WM_DROPFILES => {
                    let hdrop = wparam as HDROP;
                    process_hdrop(hwnd, hdrop, self);
                    DragFinish(hdrop);
                    Some(0)
                },
                WM_SETTINGCHANGE => {
                    handle_setting_change(self, hwnd);
                    None // DefWindowProc handles weird cases too
                },
                WM_NOTIFY => Some(handle_notify(self, hwnd, lparam)),
                WM_CONTEXTMENU => Some(handle_context_menu(self, hwnd, wparam)),
                0x0100 => Some(handle_keydown(self, hwnd, wparam)), // WM_KEYDOWN
                
                _ => None,
            }
        }
    }
}

// Logic Helper Functions

unsafe fn handle_command(st: &mut AppState, hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let id = (wparam & 0xFFFF) as u16;
    match id {
         IDC_BTN_ADD_FILES => {
             if let Ok(files) = pick_files() {
                 for file_path in files {
                     if !st.batch_items.iter().any(|item| item.path == file_path) {
                        let item_id = st.add_batch_item(file_path.clone());
                        let logical_size = calculate_path_logical_size(&file_path);
                        let disk_size = calculate_path_disk_size(&file_path);
                        let detected_algo = detect_path_algorithm(&file_path);
                        let logical_str = format_size(logical_size);
                        let disk_str = format_size(disk_size);
                        if let Some(ctrls) = &st.controls {
                            if let Some(batch_item) = st.batch_items.iter().find(|i| i.id == item_id) {
                                ctrls.file_list.add_item(item_id, batch_item, logical_str, disk_str, detected_algo);
                            }
                        }
                     }
                 }
             }
         },
         IDC_BTN_ADD_FOLDER => {
             if let Ok(folder) = pick_folder() {
                 if !st.batch_items.iter().any(|item| item.path == folder) {
                    let item_id = st.add_batch_item(folder.clone());
                    let logical_size = calculate_path_logical_size(&folder);
                    let disk_size = calculate_path_disk_size(&folder);
                    let detected_algo = detect_path_algorithm(&folder);
                    let logical_str = format_size(logical_size);
                    let disk_str = format_size(disk_size);
                    if let Some(ctrls) = &st.controls {
                        if let Some(batch_item) = st.batch_items.iter().find(|i| i.id == item_id) {
                            ctrls.file_list.add_item(item_id, batch_item, logical_str, disk_str, detected_algo);
                        }
                    }
                 }
             }
         },
         IDC_BTN_REMOVE => {
               let mut selected_indices = if let Some(ctrls) = &st.controls {
                   ctrls.file_list.get_selected_indices()
               } else { Vec::new() };
               selected_indices.sort_by(|a, b| b.cmp(a));
               
               let ids_to_remove: Vec<u32> = selected_indices.iter()
                   .filter_map(|&idx| st.batch_items.get(idx).map(|item| item.id))
                   .collect();
               
               for id in ids_to_remove { st.remove_batch_item(id); }
               
               if let Some(ctrls) = &st.controls {
                   for idx in selected_indices { ctrls.file_list.remove_item(idx as i32); }
               }
         },
         IDC_BTN_PROCESS_ALL => {
                if st.batch_items.is_empty() {
                    let w_info = to_wstring("Info");
                    let w_msg = to_wstring("Add folders first!");
                    MessageBoxW(hwnd, w_msg.as_ptr(), w_info.as_ptr(), MB_OK | MB_ICONINFORMATION);
                } else {
                    if let Some(ctrls) = &st.controls {
                        let mut indices_to_process = ctrls.file_list.get_selected_indices();
                        
                        // Auto-Start Safety Check:
                        // If triggered by timer (lparam == 1) and no items are selected, DO NOT process everything.
                        // This prevents "Process All" from accidentally running on existing items.
                        let is_auto_start = lparam == 1;
                        if is_auto_start && indices_to_process.is_empty() {
                            return 0;
                        }

                        if indices_to_process.is_empty() {
                            indices_to_process = (0..st.batch_items.len()).collect();
                        }
                        let idx = SendMessageW(ctrls.action_panel.combo_hwnd(), CB_GETCURSEL, 0, 0);
                        // Index 0 = "As Listed" (use per-item algorithm)
                        // Index 1-4 = specific algorithms
                        let use_as_listed = idx == 0;
                        let global_algo = match idx {
                            1 => WofAlgorithm::Xpress4K,
                            3 => WofAlgorithm::Xpress16K,
                            4 => WofAlgorithm::Lzx,
                            _ => WofAlgorithm::Xpress8K, // Default to Xpress8K
                        };
                        
                        // Update display in list (show "As Listed" or specific algo)
                        if !use_as_listed {
                            let algo_name = match global_algo {
                                WofAlgorithm::Xpress4K => "XPRESS4K",
                                WofAlgorithm::Xpress8K => "XPRESS8K",
                                WofAlgorithm::Xpress16K => "XPRESS16K",
                                WofAlgorithm::Lzx => "LZX",
                            };
                            for &row in &indices_to_process {
                                ctrls.file_list.update_item_text(row as i32, 2, to_wstring(algo_name));
                            }
                        }
                        if let Some(tb) = &st.taskbar { tb.set_state(TaskbarState::Normal); }
                        windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(ctrls.action_panel.cancel_hwnd(), 1); // TRUE

                        let count_w = u64_to_wstring(indices_to_process.len() as u64);
                        let status_msg = concat_wstrings(&[&to_wstring("Processing "), &count_w, &to_wstring(" items...")]);
                        SetWindowTextW(ctrls.status_bar.label_hwnd(), status_msg.as_ptr());
                        
                        let tx = st.tx.clone();
                        let state_global = st.global_state.clone();
                        state_global.store(ProcessingState::Running as u8, Ordering::Relaxed);
                        
                        let action_mode_idx = SendMessageW(ctrls.action_panel.action_mode_hwnd(), CB_GETCURSEL, 0, 0);
                        let items: Vec<_> = indices_to_process.into_iter().filter_map(|idx| {
                            st.batch_items.get(idx).map(|item| {
                                let effective_action = match action_mode_idx {
                                    1 => BatchAction::Compress, 2 => BatchAction::Decompress, _ => item.action,
                                };
                                // Determine effective algorithm PER ITEM
                                let effective_algo = if use_as_listed {
                                    item.algorithm
                                } else {
                                    global_algo
                                };
                                (item.path.clone(), effective_action, idx, effective_algo)
                            })
                        }).collect();
                        
                        let force = st.force_compress;
                        let guard = st.config.enable_system_guard;
                        let main_hwnd_usize = hwnd as usize;
                        thread::spawn(move || {
                            batch_process_worker(items, tx, state_global, force, main_hwnd_usize, guard);
                        });
                    }
                }
         },
         IDC_BTN_CANCEL => {
              st.global_state.store(ProcessingState::Stopped as u8, Ordering::Relaxed);
              for item in &st.batch_items {
                  if let Some(flag) = &item.state_flag { flag.store(ProcessingState::Stopped as u8, Ordering::Relaxed); }
              }
              if let Some(tb) = &st.taskbar { tb.set_state(TaskbarState::Paused); }
              if let Some(ctrls) = &st.controls {
                  windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(ctrls.action_panel.cancel_hwnd(), 0);
                  let w_stop = to_wstring("Stopping...");
                  SetWindowTextW(ctrls.status_bar.label_hwnd(), w_stop.as_ptr());
              }
         },
         IDC_BTN_SETTINGS => {
               let current_theme = st.theme;
               let is_dark = theme::resolve_mode(st.theme);
               let enable_ctx = st.config.enable_context_menu;
               let enable_guard = st.config.enable_system_guard;
               let (new_theme, new_force, new_ctx, new_guard) = show_settings_modal(hwnd, current_theme, is_dark, st.enable_force_stop, enable_ctx, enable_guard);
               if let Some(t) = new_theme {
                   st.theme = t;
                   let new_is_dark = theme::resolve_mode(st.theme);
                   theme::set_window_frame_theme(hwnd, new_is_dark);
                   if let Some(ctrls) = &mut st.controls { ctrls.update_theme(new_is_dark, hwnd); }
                   InvalidateRect(hwnd, std::ptr::null(), 1);
               }
               st.enable_force_stop = new_force;
               st.config.enable_context_menu = new_ctx;
               st.config.enable_system_guard = new_guard;
         },
         IDC_BTN_ABOUT => {
              let is_dark = theme::resolve_mode(st.theme);
              show_about_modal(hwnd, is_dark);
         },
         IDC_BTN_SHORTCUTS => {
              let is_dark = theme::resolve_mode(st.theme);
              show_shortcuts_modal(hwnd, is_dark);
         },
         IDC_BTN_CONSOLE => {
               let is_dark = theme::resolve_mode(st.theme);
               show_console_window(hwnd, &st.logs, is_dark);
         },
         IDC_CHK_FORCE => {
               let hwnd_ctl = lparam as HWND;
               let state = SendMessageW(hwnd_ctl, BM_GETCHECK, 0, 0);
               st.force_compress = state as u32 == BST_CHECKED;
         },
         _ => {}
    }
    0
}

unsafe fn handle_timer(st: &mut AppState, hwnd: HWND, wparam: WPARAM) -> LRESULT {
    if wparam == 2 {
        KillTimer(hwnd, 2);
        st.ipc_active = false;
        
        if !st.pending_ipc_ids.is_empty() {
             if let Some(ctrls) = &st.controls {
                 // clear existing selection
                 let count = ctrls.file_list.get_item_count();
                 for i in 0..count {
                     ctrls.file_list.set_selected(i, false);
                 }
                 // select pending items
                 for &id in &st.pending_ipc_ids {
                     if let Some(pos) = st.batch_items.iter().position(|item| item.id == id) {
                         ctrls.file_list.set_selected(pos as i32, true);
                     }
                 }
             }
             st.pending_ipc_ids.clear();
        }
        // Send lparam=1 to indicate Auto-Start source
        SendMessageW(hwnd, WM_COMMAND, IDC_BTN_PROCESS_ALL as usize, 1);
        return 0;
    }
    
    loop {
        match st.rx.try_recv() {
            Ok(msg) => {
                 match msg {
                     UiMessage::Progress(cur, total) => {
                         if let Some(ctrls) = &st.controls {
                             SendMessageW(ctrls.status_bar.progress_hwnd(), PBM_SETRANGE32, 0, total as isize);
                             SendMessageW(ctrls.status_bar.progress_hwnd(), PBM_SETPOS, cur as usize, 0);
                         }
                         if let Some(tb) = &st.taskbar { tb.set_value(cur, total); }
                     },
                     UiMessage::Status(text) => {
                         if let Some(ctrls) = &st.controls {
                             SetWindowTextW(ctrls.status_bar.label_hwnd(), text.as_ptr());
                         }
                     },
                     UiMessage::Log(text) => {
                         st.logs.push(text.clone());
                         append_log_msg(text);
                     },
                     UiMessage::Error(text) => {
                         if let Some(tb) = &st.taskbar { tb.set_state(TaskbarState::Error); }
                         let err_prefix = to_wstring("ERROR: ");
                         let full_err = concat_wstrings(&[&err_prefix, &text]);
                         st.logs.push(full_err.clone());
                         append_log_msg(full_err);
                         
                         if let Some(ctrls) = &st.controls {
                             SetWindowTextW(ctrls.status_bar.label_hwnd(), text.as_ptr());
                         }
                     },
                     UiMessage::Finished => {
                         if let Some(tb) = &st.taskbar { tb.set_state(TaskbarState::NoProgress); }
                         if let Some(ctrls) = &st.controls {
                             windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(ctrls.action_panel.cancel_hwnd(), 0);
                         }
                     },
                     UiMessage::RowUpdate(row, progress, status, _) => {
                         if let Some(ctrls) = &st.controls {
                             ctrls.file_list.update_item_text(row, 6, progress);
                             ctrls.file_list.update_item_text(row, 7, status);
                         }
                     },
                     UiMessage::ItemFinished(row, status, disk_size, final_state) => {
                         if let Some(ctrls) = &st.controls {
                             ctrls.file_list.update_item_text(row, 7, status);
                             if !disk_size.is_empty() && disk_size.len() > 1 { ctrls.file_list.update_item_text(row, 5, disk_size); }
                             let state_str = match final_state {
                                 CompressionState::None => "-",
                                 CompressionState::Specific(algo) => match algo {
                                     WofAlgorithm::Xpress4K => "XPRESS4K", WofAlgorithm::Xpress8K => "XPRESS8K",
                                     WofAlgorithm::Xpress16K => "XPRESS16K", WofAlgorithm::Lzx => "LZX",
                                 },
                                 CompressionState::Mixed => "Mixed",
                             };
                             ctrls.file_list.update_item_text(row, 1, to_wstring(state_str));
                             ctrls.file_list.update_item_text(row, 8, to_wstring("▶ Start"));
                             if let Some(item) = st.batch_items.get_mut(row as usize) {
                                 item.status = BatchStatus::Pending;
                                 item.state_flag = None;
                             }
                         }
                     },
                     UiMessage::BatchItemAnalyzed(id, log, disk, state) => {
                         let log_str = format_size(log);
                         let disk_str = format_size(disk);
                         if let Some(pos) = st.batch_items.iter().position(|item| item.id == id) {
                             if let Some(item) = st.batch_items.get_mut(pos) {
                                 item.logical_size = log;
                                 item.disk_size = disk;
                             }
                             
                             if let Some(ctrls) = &st.controls {
                                 ctrls.file_list.update_item_text(pos as i32, 4, log_str);
                                 ctrls.file_list.update_item_text(pos as i32, 5, disk_str);
                                 let state_str = match state {
                                    CompressionState::None => "-",
                                    CompressionState::Specific(algo) => match algo {
                                        WofAlgorithm::Xpress4K => "XPRESS4K", WofAlgorithm::Xpress8K => "XPRESS8K",
                                        WofAlgorithm::Xpress16K => "XPRESS16K", WofAlgorithm::Lzx => "LZX",
                                    },
                                    CompressionState::Mixed => "Mixed",
                                 };
                                 ctrls.file_list.update_item_text(pos as i32, 1, to_wstring(state_str));
                                 ctrls.file_list.update_item_text(pos as i32, 7, to_wstring("Pending"));
                                 let count = st.batch_items.len();
                                 let count_w = u64_to_wstring(count as u64);
                                 let msg = concat_wstrings(&[&count_w, &to_wstring(" item(s) analyzed.")]);
                                 SetWindowTextW(ctrls.status_bar.label_hwnd(), msg.as_ptr());
                             }
                         }
                     },
                     _ => {}
                 }
            },
            Err(_) => break,
        }
    }
    0
}

unsafe fn handle_size(st: &mut AppState, hwnd: HWND) -> LRESULT {
    let mut client_rect: RECT = std::mem::zeroed();
    if GetClientRect(hwnd, &mut client_rect) != 0 {
        if let Some(ctrls) = &mut st.controls {
             ctrls.status_bar.on_resize(&client_rect);
             ctrls.file_list.on_resize(&client_rect);
             ctrls.action_panel.on_resize(&client_rect);
             ctrls.header_panel.on_resize(&client_rect);
        }
    }
    0
}

unsafe fn handle_destroy(st: &mut AppState, hwnd: HWND) {
    let mut rect: RECT = std::mem::zeroed();
    if GetWindowRect(hwnd, &mut rect) != 0 {
        st.config.window_x = rect.left;
        st.config.window_y = rect.top;
        st.config.window_width = rect.right - rect.left;
        st.config.window_height = rect.bottom - rect.top;
    }
    if let Some(ctrls) = &st.controls {
        let idx = SendMessageW(ctrls.action_panel.combo_hwnd(), CB_GETCURSEL, 0, 0);
       st.config.default_algo = match idx {
           1 => WofAlgorithm::Xpress4K,
           3 => WofAlgorithm::Xpress16K, 
           4 => WofAlgorithm::Lzx,
           _ => WofAlgorithm::Xpress8K,
       };
        let force = SendMessageW(ctrls.action_panel.force_hwnd(), BM_GETCHECK, 0, 0);
        st.config.force_compress = force as u32 == BST_CHECKED;
    }
    st.config.theme = st.theme;
    st.config.enable_force_stop = st.enable_force_stop;
    st.config.save();
    
    // Cleanup is handled by Box::from_raw in previous logic, but now AppState is Box::leak-ed
    // The OS cleans up memory on exit.
    // If we want to drop, we can:
    // let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    // if ptr != 0 { let _ = Box::from_raw(ptr as *mut AppState); }
    // framework.rs handles memory for generic?
    // framework.rs: wnd_proc doesn't assume ownership (it does `&mut *ptr`). 
    // It doesn't drop.
    // So we should manually drop if we want clean drop semantics (saving config was moved here though).
    
    // Force explicit process exit to prevent zombie processes.
    std::process::exit(0);
}

unsafe fn handle_copy_data(st: &mut AppState, hwnd: HWND, lparam: LPARAM) -> LRESULT {
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
             
             if !st.batch_items.iter().any(|item| item.path == path) {
                 let id = st.add_batch_item(path.clone());
                 if let Some(item) = st.get_batch_item_mut(id) {
                      item.algorithm = algo;
                      item.action = action;
                 }
                 
                 if let Some(ctrls) = &st.controls {
                      SendMessageW(ctrls.action_panel.combo_hwnd(), CB_SETCURSEL, 0, 0);
                      
                      if !st.ipc_active {
                          let count = ctrls.file_list.get_item_count();
                          for i in 0..count {
                              ctrls.file_list.set_selected(i, false);
                          }
                          st.ipc_active = true;
                      }

                      if let Some(pos) = st.batch_items.iter().position(|i| i.id == id) {
                          if let Some(batch_item) = st.batch_items.get(pos) {
                               ctrls.file_list.add_item(id, batch_item, to_wstring("Calculating..."), to_wstring("Calculating..."), CompressionState::None);
                               ctrls.file_list.set_selected(pos as i32, true);
                               st.pending_ipc_ids.push(id);
                          }
                      }
                 }
                 
                 let tx = st.tx.clone();
                 let p = path.clone();
                 thread::spawn(move || {
                      let logical = calculate_path_logical_size(&p);
                      let disk = calculate_path_disk_size(&p);
                      let algo = detect_path_algorithm(&p);
                      let _ = tx.send(UiMessage::BatchItemAnalyzed(id, logical, disk, algo));
                 });
                 
                 SetTimer(hwnd, 2, 500, None);
             }
         }
         return 1;
     }
     0
}

unsafe fn handle_setting_change(st: &mut AppState, hwnd: HWND) {
    if st.theme == AppTheme::System {
         let is_dark = theme::resolve_mode(st.theme);
         theme::set_window_frame_theme(hwnd, is_dark);
         if let Some(ctrls) = &mut st.controls {
             ctrls.update_theme(is_dark, hwnd);
         }
         InvalidateRect(hwnd, std::ptr::null(), 1);
    }
}

unsafe fn handle_notify(st: &mut AppState, hwnd: HWND, lparam: LPARAM) -> LRESULT {
    let nmhdr = &*(lparam as *const NMHDR);
    if nmhdr.idFrom == IDC_BATCH_LIST as usize {
        if nmhdr.code == NM_CLICK || nmhdr.code == NM_DBLCLK {
            let nmia = &*(lparam as *const NMITEMACTIVATE);
            let row = nmia.iItem;
            let col = nmia.iSubItem;
            if row >= 0 {
                 if col == 0 && nmhdr.code == NM_DBLCLK { // Open Path
                     if let Some(item) = st.batch_items.get(row as usize) {
                         let path = &item.path;
                         let select_prefix = to_wstring("/select,\"");
                         let path_w = to_wstring(path);
                         let suffix = to_wstring("\"");
                         let args = concat_wstrings(&[&select_prefix, &path_w, &suffix]);
                         
                         ShellExecuteW(std::ptr::null_mut(), to_wstring("open").as_ptr(), to_wstring("explorer.exe").as_ptr(), args.as_ptr(), std::ptr::null(), SW_SHOWNORMAL);
                     }
                 } else if col == 2 && nmhdr.code == NM_DBLCLK { // Cycle Algo
                      if let Some(item) = st.batch_items.get_mut(row as usize) {
                          item.algorithm = match item.algorithm {
                              WofAlgorithm::Xpress4K => WofAlgorithm::Xpress8K,
                              WofAlgorithm::Xpress8K => WofAlgorithm::Xpress16K,
                              WofAlgorithm::Xpress16K => WofAlgorithm::Lzx,
                              WofAlgorithm::Lzx => WofAlgorithm::Xpress4K,
                          };
                          let name = match item.algorithm {
                              WofAlgorithm::Xpress4K => "XPRESS4K", WofAlgorithm::Xpress8K => "XPRESS8K",
                              WofAlgorithm::Xpress16K => "XPRESS16K", WofAlgorithm::Lzx => "LZX",
                          };
                          if let Some(ctrls) = &st.controls { ctrls.file_list.update_item_text(row, 2, to_wstring(name)); }
                      }
                 } else if col == 3 && nmhdr.code == NM_DBLCLK { // Toggle Action
                      if let Some(item) = st.batch_items.get_mut(row as usize) {
                          item.action = match item.action {
                              BatchAction::Compress => BatchAction::Decompress,
                              BatchAction::Decompress => BatchAction::Compress,
                          };
                          let name = match item.action {
                              BatchAction::Compress => "Compress", BatchAction::Decompress => "Decompress",
                          };
                          if let Some(ctrls) = &st.controls { ctrls.file_list.update_item_text(row, 3, to_wstring(name)); }
                      }
                 } else if col == 8 && nmhdr.code == NM_CLICK { // Start/Pause
                      if let Some(item) = st.batch_items.get_mut(row as usize) {
                           let path = item.path.clone();
                           let action = item.action;
                           let algo = item.algorithm;
                           let row_idx = row as usize;
                           
                           if let Some(ctrls) = &st.controls {
                               ctrls.file_list.update_item_text(row, 7, to_wstring("Starting..."));
                           }
                           
                           let items = vec![(path, action, row_idx, algo)];
                           let tx = st.tx.clone();
                           let state_global = st.global_state.clone();
                           let force = st.force_compress;
                           let guard = st.config.enable_system_guard;
                           let main_hwnd_usize = hwnd as usize;

                           thread::spawn(move || {
                               batch_process_worker(items, tx, state_global, force, main_hwnd_usize, guard);
                           });
                      }
                 }
            }
        }
        
        if nmhdr.code == LVN_KEYDOWN {
            let nmkd = &*(lparam as *const NMLVKEYDOWN);
            let vk = nmkd.wVKey as u16;
            
            if vk == VK_DELETE {
                SendMessageW(hwnd, WM_COMMAND, (IDC_BTN_REMOVE as usize) | ((0 as usize) << 16), 0);
            }
            else if vk == 0x41 { // 'A' key
                let ctrl_state = GetKeyState(VK_CONTROL as i32) as u16;
                if (ctrl_state & 0x8000) != 0 {
                     if let Some(ctrls) = &st.controls {
                         let count = ctrls.file_list.get_item_count();
                         for i in 0..count {
                             ctrls.file_list.set_selected(i, true);
                         }
                     }
                }
            }
            else if vk == 0x56 { // 'V' key
                let ctrl_state = GetKeyState(VK_CONTROL as i32) as u16;
                if (ctrl_state & 0x8000) != 0 {
                    if IsClipboardFormatAvailable(15) != 0 {
                        if OpenClipboard(hwnd) != 0 {
                             let hdrop = GetClipboardData(15) as HDROP;
                             if !hdrop.is_null() {
                                 process_hdrop(hwnd, hdrop, st);
                             }
                             CloseClipboard();
                        }
                    }
                }
            }
        }
        
        if nmhdr.code == LVN_COLUMNCLICK {
             let nmlv = &*(lparam as *const NMLISTVIEW);
             let column = nmlv.iSubItem;
             
             if st.sort_column == column {
                 st.sort_ascending = !st.sort_ascending;
             } else {
                 st.sort_column = column;
                 st.sort_ascending = true;
             }
             
             if let Some(ctrls) = &st.controls {
                 let context = st as *const AppState as isize;
                 ctrls.file_list.sort_items(compare_items, context);
             }
        }
        
        if nmhdr.code == LVN_ITEMCHANGED {
             if let Some(ctrls) = &st.controls {
                 let count = ctrls.file_list.get_selection_count();
                 let text = if count > 0 { "Process Selected" } else { "Process All" };
                 SetWindowTextW(ctrls.action_panel.process_hwnd(), to_wstring(text).as_ptr());
             }
        }
    }
    0
}

unsafe fn handle_context_menu(st: &mut AppState, hwnd: HWND, wparam: WPARAM) -> LRESULT {
    let hwnd_from = wparam as HWND;
    if let Some(ctrls) = &st.controls {
        if hwnd_from == ctrls.file_list.hwnd() {
            let selected = ctrls.file_list.get_selected_indices();
            if !selected.is_empty() {
                let mut pt: POINT = std::mem::zeroed();
                GetCursorPos(&mut pt);
                let menu = CreatePopupMenu();
                if menu != std::ptr::null_mut() {
                    let mut any_processing = false;
                    let mut any_pending = false;
                    
                    for &idx in &selected {
                        if let Some(item) = st.batch_items.get(idx as usize) {
                            match item.status {
                                BatchStatus::Processing => { any_processing = true; },
                                BatchStatus::Pending => { any_pending = true; },
                                _ => {}
                            }
                        }
                    }

                    if any_processing {
                        let _ = AppendMenuW(menu, MF_STRING, 1001, to_wstring("Pause").as_ptr());
                        let _ = AppendMenuW(menu, MF_STRING, 1003, to_wstring("Stop").as_ptr());
                        let _ = AppendMenuW(menu, MF_STRING, 1004, to_wstring("Remove").as_ptr());
                    } else if any_pending {
                        let _ = AppendMenuW(menu, MF_STRING, 1005, to_wstring("Start").as_ptr());
                        let _ = AppendMenuW(menu, MF_STRING, 1004, to_wstring("Remove").as_ptr());
                    } else {
                        let _ = AppendMenuW(menu, MF_STRING, 1004, to_wstring("Remove").as_ptr());
                    }
                    let _ = AppendMenuW(menu, MF_STRING, 1006, to_wstring("Open File Location").as_ptr());

                    let _cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_LEFTALIGN, pt.x, pt.y, 0, hwnd, std::ptr::null());
                    DestroyMenu(menu);
                    
                    if _cmd == 1001 { // Pause
                    } else if _cmd == 1003 { // Stop
                         SendMessageW(hwnd, WM_COMMAND, IDC_BTN_CANCEL as usize, 0);
                    } else if _cmd == 1004 { // Remove
                         SendMessageW(hwnd, WM_COMMAND, IDC_BTN_REMOVE as usize, 0);
                    } else if _cmd == 1005 { // Start Selected
                        let action_mode_idx = SendMessageW(ctrls.action_panel.action_mode_hwnd(), CB_GETCURSEL, 0, 0);
                        let items: Vec<_> = selected.iter().filter_map(|&idx| {
                            st.batch_items.get(idx as usize).map(|item| {
                                let effective_action = match action_mode_idx {
                                    1 => BatchAction::Compress, 2 => BatchAction::Decompress, _ => item.action,
                                };
                                (item.path.clone(), effective_action, idx as usize, item.algorithm)
                            })
                        }).collect();
                        
                        if !items.is_empty() {
                            let tx = st.tx.clone();
                            let state_global = st.global_state.clone();
                            state_global.store(ProcessingState::Running as u8, Ordering::Relaxed);
                            let force = st.force_compress;
                            let guard = st.config.enable_system_guard;
                            let main_hwnd_usize = hwnd as usize;

                            for &idx in &selected {
                                if let Some(ctrls) = &st.controls {
                                    ctrls.file_list.update_item_text(idx as i32, 7, to_wstring("Starting..."));
                                }
                            }

                            thread::spawn(move || {
                                batch_process_worker(items, tx, state_global, force, main_hwnd_usize, guard);
                            });
                        }
                    } else if _cmd == 1006 {
                        if let Some(&first_idx) = selected.first() {
                            if let Some(item) = st.batch_items.get(first_idx as usize) {
                                let select_prefix = to_wstring("/select,\"");
                                let path_w = to_wstring(&item.path);
                                let suffix = to_wstring("\"");
                                let args = concat_wstrings(&[&select_prefix, &path_w, &suffix]);
                                
                                ShellExecuteW(std::ptr::null_mut(), to_wstring("open").as_ptr(), to_wstring("explorer.exe").as_ptr(), args.as_ptr(), std::ptr::null(), SW_SHOWNORMAL);
                            }
                        }
                    }
                }
            }
        }
    }
    0
}

unsafe fn handle_keydown(st: &mut AppState, hwnd: HWND, wparam: WPARAM) -> LRESULT {
    let vk = wparam as u16;
    let ctrl_pressed = (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
    let shift_pressed = (GetKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;

    if ctrl_pressed {
        match vk {
            0x56 => { // 'V' - Paste
                if IsClipboardFormatAvailable(15) != 0 {
                    if OpenClipboard(hwnd) != 0 {
                        let hdrop = GetClipboardData(15) as HDROP;
                        if !hdrop.is_null() {
                            process_hdrop(hwnd, hdrop, st);
                        }
                        CloseClipboard();
                    }
                }
            },
            0x4F => { // 'O' - Open
                if shift_pressed {
                    SendMessageW(hwnd, WM_COMMAND, IDC_BTN_ADD_FOLDER as usize, 0);
                } else {
                    SendMessageW(hwnd, WM_COMMAND, IDC_BTN_ADD_FILES as usize, 0);
                }
            },
            0x41 => { // 'A' - Select All
                 if let Some(ctrls) = &st.controls {
                     let count = ctrls.file_list.get_item_count();
                     if count > 0 {
                        for i in 0..count {
                            ctrls.file_list.set_selected(i, true);
                        }
                     }
                 }
            },
            _ => {}
        }
    } else if vk == VK_DELETE as u16 {
        SendMessageW(hwnd, WM_COMMAND, IDC_BTN_REMOVE as usize, 0);
    }
    0
}

/// Comparison callback for ListView sorting
/// 
/// # Safety
/// Called by Windows. lParam1/lParam2 are user data (BatchItem IDs).
/// lParamSort is pointer to AppState.
unsafe extern "system" fn compare_items(lparam1: isize, lparam2: isize, lparam_sort: isize) -> i32 {
    let state = &*(lparam_sort as *const AppState);
    let id1 = lparam1 as u32;
    let id2 = lparam2 as u32;
    
    let item1 = state.batch_items.iter().find(|i| i.id == id1);
    let item2 = state.batch_items.iter().find(|i| i.id == id2);
    
    match (item1, item2) {
        (Some(i1), Some(i2)) => {
            let ord = match state.sort_column {
                0 => i1.path.to_lowercase().cmp(&i2.path.to_lowercase()), // Path
                2 => format!("{:?}", i1.algorithm).cmp(&format!("{:?}", i2.algorithm)), // Algo
                3 => format!("{:?}", i1.action).cmp(&format!("{:?}", i2.action)), // Action
                4 => i1.logical_size.cmp(&i2.logical_size), // Logical Size
                5 => i1.disk_size.cmp(&i2.disk_size), // Disk Size
                7 => format!("{:?}", i1.status).cmp(&format!("{:?}", i2.status)), // Status
                _ => CmpOrdering::Equal,
            };
            
            let result = match ord {
                CmpOrdering::Less => -1,
                CmpOrdering::Equal => 0,
                CmpOrdering::Greater => 1,
            };
            
            if state.sort_ascending { result } else { -result }
        },
        _ => 0
    }
}



// IFileOpenDialog Interface Definition (Manual, as windows-sys doesn't wrap COM traits nicely)
// We use raw vtables here.
use std::ffi::c_void;

#[repr(C)]
struct IFileOpenDialogVtbl {
    pub query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    pub release: unsafe extern "system" fn(*mut c_void) -> u32,
    // IModalWindow
    pub show: unsafe extern "system" fn(*mut c_void, HWND) -> HRESULT,
    // IFileDialog
    pub set_file_types: unsafe extern "system" fn(*mut c_void, u32, *const c_void) -> HRESULT,
    pub set_file_type_index: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
    pub get_file_type_index: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    pub advise: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut u32) -> HRESULT,
    pub unadvise: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
    pub set_options: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
    pub get_options: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    pub set_default_folder: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    pub set_folder: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    pub get_folder: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub get_current_selection: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub set_file_name: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub get_file_name: unsafe extern "system" fn(*mut c_void, *mut PCWSTR) -> HRESULT,
    pub set_title: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub set_ok_button_label: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub set_file_name_label: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub get_result: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT, // Returns IShellItem
    pub add_place: unsafe extern "system" fn(*mut c_void, *mut c_void, u32) -> HRESULT,
    pub set_default_extension: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub close: unsafe extern "system" fn(*mut c_void, HRESULT) -> HRESULT,
    pub set_client_guid: unsafe extern "system" fn(*mut c_void, *const GUID) -> HRESULT,
    pub clear_client_data: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    pub set_filter: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    // IFileOpenDialog
    pub get_results: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT, // Returns IShellItemArray
    pub get_selected_items: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}

#[repr(C)]
struct IShellItemVtbl {
    pub query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    pub release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub bind_to_handler: unsafe extern "system" fn(*mut c_void, *mut c_void, *const GUID, *const GUID, *mut *mut c_void) -> HRESULT,
    pub get_parent: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub get_display_name: unsafe extern "system" fn(*mut c_void, u32, *mut PCWSTR) -> HRESULT,
    pub get_attributes: unsafe extern "system" fn(*mut c_void, u32, *mut u32) -> HRESULT,
    pub compare: unsafe extern "system" fn(*mut c_void, *mut c_void, u32, *mut i32) -> HRESULT,
}

#[repr(C)]
struct IShellItemArrayVtbl {
    pub query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    pub release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub bind_to_handler: unsafe extern "system" fn(*mut c_void, *mut c_void, *const GUID, *const GUID, *mut *mut c_void) -> HRESULT,
    pub get_property_store: unsafe extern "system" fn(*mut c_void, u32, *const GUID, *mut *mut c_void) -> HRESULT,
    pub get_property_description_list: unsafe extern "system" fn(*mut c_void, *const GUID, *const GUID, *mut *mut c_void) -> HRESULT,
    pub get_attributes: unsafe extern "system" fn(*mut c_void, u32, u32, *mut c_void) -> HRESULT,
    pub get_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    pub get_item_at: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
    pub enum_items: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}

// GUIDs
const CLSID_FILE_OPEN_DIALOG: GUID = GUID { data1: 0xDC1C5A9C, data2: 0xE88A, data3: 0x4DDE, data4: [0xA5, 0xA1, 0x60, 0xF8, 0x2A, 0x20, 0xAE, 0xF7] };
const IID_IFILE_OPEN_DIALOG: GUID = GUID { data1: 0xd57c7288, data2: 0xd4ad, data3: 0x4768, data4: [0xbe, 0x02, 0x9d, 0x96, 0x95, 0x32, 0xd9, 0x60] };

const FOS_PICKFOLDERS: u32 = 0x20;
const FOS_FORCEFILESYSTEM: u32 = 0x40;
const FOS_ALLOWMULTISELECT: u32 = 0x200;
const SIGDN_FILESYSPATH: u32 = 0x80058000;

/// Pick files (multi-select)
unsafe fn pick_files() -> Result<Vec<String>, HRESULT> {
    let mut p_dialog: *mut c_void = std::ptr::null_mut();
    let hr = CoCreateInstance(&CLSID_FILE_OPEN_DIALOG, std::ptr::null_mut(), CLSCTX_ALL, &IID_IFILE_OPEN_DIALOG, &mut p_dialog);
    if hr != 0 { return Err(hr); }

    let dialog = p_dialog as *mut *mut IFileOpenDialogVtbl;
    let vtbl = (*dialog).as_ref().unwrap();

    let mut options = 0;
    (vtbl.get_options)(p_dialog, &mut options);
    (vtbl.set_options)(p_dialog, options | FOS_FORCEFILESYSTEM | FOS_ALLOWMULTISELECT);
    
    let hr = (vtbl.show)(p_dialog, std::ptr::null_mut()); // HWND owner = null
    if hr != 0 {
        (vtbl.release)(p_dialog);
        return Err(hr);
    }

    let mut p_results: *mut c_void = std::ptr::null_mut();
    let hr = (vtbl.get_results)(p_dialog, &mut p_results);
    if hr != 0 {
        (vtbl.release)(p_dialog);
        return Err(hr);
    }

    let results = p_results as *mut *mut IShellItemArrayVtbl;
    let results_vtbl = (*results).as_ref().unwrap();

    let mut count = 0;
    (results_vtbl.get_count)(p_results, &mut count);
    
    let mut paths = Vec::new();
    for i in 0..count {
        let mut p_item: *mut c_void = std::ptr::null_mut();
        if (results_vtbl.get_item_at)(p_results, i, &mut p_item) == 0 {
            let item = p_item as *mut *mut IShellItemVtbl;
            let item_vtbl = (*item).as_ref().unwrap();
            
            let mut name_ptr: PCWSTR = std::ptr::null();
            if (item_vtbl.get_display_name)(p_item, SIGDN_FILESYSPATH, &mut name_ptr) == 0 && !name_ptr.is_null() {
                let len = (0..).take_while(|&i| *name_ptr.offset(i) != 0).count();
                let slice = std::slice::from_raw_parts(name_ptr, len);
                if let Ok(path) = String::from_utf16(slice) {
                    paths.push(path);
                }
                CoTaskMemFree(name_ptr as *mut _);
            }
            (item_vtbl.release)(p_item);
        }
    }

    (results_vtbl.release)(p_results);
    (vtbl.release)(p_dialog);

    Ok(paths)
}

/// Pick folder (single folder selection)
unsafe fn pick_folder() -> Result<String, HRESULT> {
    let mut p_dialog: *mut c_void = std::ptr::null_mut();
    let hr = CoCreateInstance(&CLSID_FILE_OPEN_DIALOG, std::ptr::null_mut(), CLSCTX_ALL, &IID_IFILE_OPEN_DIALOG, &mut p_dialog);
    if hr != 0 { return Err(hr); }

    let dialog = p_dialog as *mut *mut IFileOpenDialogVtbl;
    let vtbl = (*dialog).as_ref().unwrap();

    let mut options = 0;
    (vtbl.get_options)(p_dialog, &mut options);
    (vtbl.set_options)(p_dialog, options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM);
    
    let hr = (vtbl.show)(p_dialog, std::ptr::null_mut()); // HWND owner = null
    if hr != 0 {
        (vtbl.release)(p_dialog);
        return Err(hr);
    }

    let mut p_item: *mut c_void = std::ptr::null_mut();
    let hr = (vtbl.get_result)(p_dialog, &mut p_item);
    if hr != 0 {
        (vtbl.release)(p_dialog);
        return Err(hr);
    }

    let item = p_item as *mut *mut IShellItemVtbl;
    let item_vtbl = (*item).as_ref().unwrap();
    
    let mut name_ptr: PCWSTR = std::ptr::null();
    let mut path = String::new();
    
    if (item_vtbl.get_display_name)(p_item, SIGDN_FILESYSPATH, &mut name_ptr) == 0 && !name_ptr.is_null() {
        let len = (0..).take_while(|&i| *name_ptr.offset(i) != 0).count();
        let slice = std::slice::from_raw_parts(name_ptr, len);
        if let Ok(s) = String::from_utf16(slice) {
            path = s;
        }
        CoTaskMemFree(name_ptr as *mut _);
    }

    (item_vtbl.release)(p_item);
    (vtbl.release)(p_dialog);

    Ok(path)
}

/// Helper function to process HDROP handle (extract paths and add to batch)
/// Used by both WM_DROPFILES and Ctrl+V (Clipboard)
unsafe fn process_hdrop(_hwnd: HWND, hdrop: HDROP, st: &mut AppState) {
    let count = DragQueryFileW(hdrop, 0xFFFFFFFF, std::ptr::null_mut(), 0);
    let mut paths = Vec::new();
    let mut buffer = [0u16; 1024];
    
    for i in 0..count {
        let len = DragQueryFileW(hdrop, i, buffer.as_mut_ptr(), 1024);
        if len > 0 {
            let s = String::from_utf16_lossy(&buffer[..len as usize]);
            paths.push(s);
        }
    }
    
    if let Some(ctrls) = &st.controls {
        let w_msg = to_wstring("Analyzing dropped files...");
        SetWindowTextW(ctrls.status_bar.label_hwnd(), w_msg.as_ptr());
    }
    let mut items_to_analyze = Vec::new();
    for path in paths {
        if !st.batch_items.iter().any(|item| item.path == path) {
            let id = st.add_batch_item(path.clone());
            if let Some(ctrls) = &st.controls {
                 if let Some(batch_item) = st.batch_items.iter().find(|i| i.id == id) {
                     ctrls.file_list.add_item(id, batch_item, to_wstring("Calculating..."), to_wstring("Calculating..."), CompressionState::None);
                 }
            }
            items_to_analyze.push((id, path));
        }
    }
    
    if items_to_analyze.is_empty() { return; }

    let tx = st.tx.clone();
    thread::spawn(move || {
        for (id, path) in items_to_analyze {
             let logical = calculate_path_logical_size(&path);
             let disk = calculate_path_disk_size(&path);
             let algo = detect_path_algorithm(&path);
             let _ = tx.send(UiMessage::BatchItemAnalyzed(id, logical, disk, algo));
        }
        let _ = tx.send(UiMessage::Status(to_wstring("Ready.")));
    });
}
