#![allow(unsafe_op_in_unsafe_fn)]
use windows::core::{w, Result, PCWSTR};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::InvalidateRect;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, PostQuitMessage, RegisterClassW, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, SW_SHOW, WM_DESTROY, WNDCLASSW,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE, WM_CREATE, WM_SIZE, WM_COMMAND,
    GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA, WM_DROPFILES, MessageBoxW, MB_OK,
    SendMessageW, CB_ADDSTRING, CB_SETCURSEL, CB_GETCURSEL, SetWindowTextW, WM_TIMER, SetTimer,
    MB_ICONINFORMATION, WM_NOTIFY, BM_GETCHECK, GetClientRect, GetWindowRect,
    BM_SETCHECK, ChangeWindowMessageFilterEx, MSGFLT_ALLOW, WM_COPYDATA,
    WM_CONTEXTMENU, TrackPopupMenu, CreatePopupMenu, AppendMenuW, MF_STRING, TPM_RETURNCMD, TPM_LEFTALIGN,
    DestroyMenu, GetCursorPos,
};
use windows::Win32::UI::Shell::{
    DragQueryFileW, DragFinish, HDROP, FileOpenDialog, IFileOpenDialog,
    FOS_PICKFOLDERS, FOS_FORCEFILESYSTEM, SIGDN_FILESYSPATH, DragAcceptFiles,
    ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, CoTaskMemFree};

use crate::ui::controls::{
    IDC_COMBO_ALGO, IDC_STATIC_TEXT, IDC_PROGRESS_BAR, IDC_BTN_CANCEL, IDC_BATCH_LIST,
    IDC_BTN_ADD_FOLDER, IDC_BTN_REMOVE, IDC_BTN_PROCESS_ALL, IDC_BTN_ADD_FILES,
    IDC_BTN_SETTINGS, IDC_BTN_ABOUT, IDC_BTN_CONSOLE, IDC_CHK_FORCE, IDC_COMBO_ACTION_MODE,
};
use crate::ui::components::{
    Component, FileListView, StatusBar, StatusBarIds, ActionPanel, ActionPanelIds,
    HeaderPanel, HeaderPanelIds,
};
use crate::ui::settings::show_settings_modal;
use crate::ui::about::show_about_modal;
use crate::ui::console::{show_console_window, append_log_msg};
use crate::ui::state::{AppState, Controls, UiMessage, BatchAction, BatchStatus, AppTheme, ProcessingState};
use crate::ui::taskbar::{TaskbarProgress, TaskbarState};
use crate::ui::theme; // New theme module import
use std::thread;
use std::sync::{Arc, atomic::{AtomicU8, Ordering}};
use windows::Win32::UI::Controls::{
    PBM_SETRANGE32, PBM_SETPOS, NM_DBLCLK, NMITEMACTIVATE,
    InitCommonControlsEx, INITCOMMONCONTROLSEX, ICC_WIN95_CLASSES, ICC_STANDARD_CLASSES,
    LVN_ITEMCHANGED, BST_CHECKED,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use crate::engine::wof::{WofAlgorithm, CompressionState};
use crate::engine::worker::{
    batch_process_worker, single_item_worker,
    calculate_path_logical_size, calculate_path_disk_size, detect_path_algorithm,
};
use crate::ui::utils::{format_size, ToWide};
use crate::config::AppConfig;
use crate::ui::utils::get_window_state;





const WINDOW_CLASS_NAME: PCWSTR = w!("CompactRS_Class");
const WINDOW_TITLE: PCWSTR = w!("CompactRS");


pub unsafe fn create_main_window(instance: HINSTANCE) -> Result<HWND> {
    unsafe {
        // Enable dark mode for the application
        theme::allow_dark_mode();

        // Initialize Common Controls to ensure Visual Styles are applied
        let iccex = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_WIN95_CLASSES | ICC_STANDARD_CLASSES,
        };
        InitCommonControlsEx(&iccex);

        // Check dark mode for window class background
        let is_dark = theme::is_system_dark_mode();
        let (bg_brush, _, _) = theme::get_theme_colors(is_dark);

        // Load icon using centralized helper
        let icon = crate::ui::utils::load_app_icon(instance);

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            hIcon: icon,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: bg_brush,
            lpszClassName: WINDOW_CLASS_NAME,
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        if atom == 0 {
            return Err(windows::core::Error::from_thread());
        }

        // Load configuration
        let config = AppConfig::load();
        let (win_x, win_y) = if config.window_x < 0 || config.window_y < 0 {
            (CW_USEDEFAULT, CW_USEDEFAULT)
        } else {
            (config.window_x, config.window_y)
        };
        let win_width = if config.window_width > 0 { config.window_width } else { 900 };
        let win_height = if config.window_height > 0 { config.window_height } else { 600 };

        let hwnd = CreateWindowExW(
            Default::default(),
            WINDOW_CLASS_NAME,
            WINDOW_TITLE,
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            win_x,
            win_y,
            win_width,
            win_height,
            None,
            None,
            Some(instance),
            None,
        )?;

        // Apply initial theme
        let is_dark = theme::resolve_mode(config.theme);
        theme::set_window_frame_theme(hwnd, is_dark);

        ShowWindow(hwnd, SW_SHOW);

        Ok(hwnd)
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        // Use the centralized helper for state access
        let get_state = || get_window_state::<AppState>(hwnd);

        // Centralized handler for theme-related messages
        let is_dark = get_state()
            .map(|st| theme::resolve_mode(st.theme))
            .unwrap_or_else(|| theme::is_system_dark_mode());
        if let Some(result) = theme::handle_standard_colors(hwnd, msg, wparam, is_dark) {
            return result;
        }

        match msg {
            WM_CREATE => {
                let mut state = Box::new(AppState::new());
                state.taskbar = Some(TaskbarProgress::new(hwnd));
                
                // Create components using the new architecture
                
                // 1. StatusBar (label + progress bar)
                let mut status_bar = StatusBar::new(StatusBarIds {
                    label_id: IDC_STATIC_TEXT,
                    progress_id: IDC_PROGRESS_BAR,
                });
                let _ = status_bar.create(hwnd);
                
                // 2. FileListView
                let file_list = FileListView::new(hwnd, 10, 40, 860, 380, IDC_BATCH_LIST);
                
                // 3. ActionPanel (all action buttons + combo + checkbox)
                let mut action_panel = ActionPanel::new(ActionPanelIds {
                    btn_files: IDC_BTN_ADD_FILES,
                    btn_folder: IDC_BTN_ADD_FOLDER,
                    btn_remove: IDC_BTN_REMOVE,
                    combo_action_mode: IDC_COMBO_ACTION_MODE,
                    combo_algo: IDC_COMBO_ALGO,
                    chk_force: IDC_CHK_FORCE,
                    btn_process: IDC_BTN_PROCESS_ALL,
                    btn_cancel: IDC_BTN_CANCEL,
                });
                let _ = action_panel.create(hwnd);
                
                // 4. HeaderPanel (settings, about, console buttons)
                let mut header_panel = HeaderPanel::new(HeaderPanelIds {
                    btn_settings: IDC_BTN_SETTINGS,
                    btn_about: IDC_BTN_ABOUT,
                    btn_console: IDC_BTN_CONSOLE,
                });
                let _ = header_panel.create(hwnd);
                
                // Disable cancel button initially
                EnableWindow(action_panel.cancel_hwnd(), false);

                // Populate algorithm combo
                let h_combo = action_panel.combo_hwnd();
                let algos = [w!("XPRESS4K"), w!("XPRESS8K"), w!("XPRESS16K"), w!("LZX")];
                for alg in algos {
                    SendMessageW(h_combo, CB_ADDSTRING, Some(WPARAM(0)), Some(LPARAM(alg.as_ptr() as isize)));
                }
                // Set initial combo selection based on saved config
                let algo_index = match state.config.default_algo {
                    WofAlgorithm::Xpress4K => 0,
                    WofAlgorithm::Xpress8K => 1,
                    WofAlgorithm::Xpress16K => 2,
                    WofAlgorithm::Lzx => 3,
                };
                SendMessageW(h_combo, CB_SETCURSEL, Some(WPARAM(algo_index)), Some(LPARAM(0)));
                
                // Populate action mode combo
                let h_action_mode = action_panel.action_mode_hwnd();
                let action_modes = [w!("As Listed"), w!("Compress All"), w!("Decompress All")];
                for mode in action_modes {
                    SendMessageW(h_action_mode, CB_ADDSTRING, Some(WPARAM(0)), Some(LPARAM(mode.as_ptr() as isize)));
                }
                // Set "As Listed" as default selection
                SendMessageW(h_action_mode, CB_SETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));
                
                // Set initial force checkbox state
                if state.force_compress {
                    SendMessageW(action_panel.force_hwnd(), BM_SETCHECK, Some(WPARAM(BST_CHECKED.0 as usize)), None);
                }

                // Store all component HWNDs in Controls for backwards compatibility
                state.controls = Some(Controls {
                    file_list,
                    status_bar,
                    action_panel,
                    header_panel,
                });

                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
                SetTimer(Some(hwnd), 1, 100, None);
                DragAcceptFiles(hwnd, true);
                
                // Allow Drag and Drop messages to bypass UIPI (User Interface Privilege Isolation)
                // when running as Administrator. This is needed because Explorer runs at Medium IL
                // while our app runs at High IL in Release mode (requireAdministrator).
                let _ = ChangeWindowMessageFilterEx(hwnd, WM_DROPFILES, MSGFLT_ALLOW, None);
                let _ = ChangeWindowMessageFilterEx(hwnd, WM_COPYDATA, MSGFLT_ALLOW, None);
                let _ = ChangeWindowMessageFilterEx(hwnd, 0x0049, MSGFLT_ALLOW, None); // WM_COPYGLOBALDATA
                
                // Apply saved theme using new Controls::update_theme
                if let Some(st) = get_state() {
                    let is_dark = theme::resolve_mode(st.theme);
                    theme::set_window_frame_theme(hwnd, is_dark);
                    if let Some(ctrls) = &mut st.controls {
                        ctrls.update_theme(is_dark, hwnd);
                    }
                    InvalidateRect(Some(hwnd), None, true);
                }
                
                // Process startup items from CLI arguments
                let startup_items = crate::get_startup_items();
                let has_startup_items = !startup_items.is_empty();
                if has_startup_items {
                    if let Some(st) = get_state() {
                        for startup_item in startup_items {
                            // Check if already added
                            let already_exists = st.batch_items.iter().any(|item| item.path == startup_item.path);
                            if !already_exists {
                                let item_id = st.add_batch_item(startup_item.path.clone());
                                
                                // Set algorithm and action
                                if let Some(batch_item) = st.get_batch_item_mut(item_id) {
                                    batch_item.algorithm = startup_item.algorithm;
                                    batch_item.action = startup_item.action;
                                }
                                
                                // Calculate sizes in background
                                let logical_size = calculate_path_logical_size(&startup_item.path);
                                let disk_size = calculate_path_disk_size(&startup_item.path);
                                let detected_algo = detect_path_algorithm(&startup_item.path);
                                let logical_str = format_size(logical_size);
                                let disk_str = format_size(disk_size);
                                
                                if let Some(ctrls) = &st.controls {
                                    if let Some(batch_item) = st.batch_items.iter().find(|i| i.id == item_id) {
                                        ctrls.file_list.add_item(item_id, batch_item, &logical_str, &disk_str, detected_algo);
                                    }
                                }
                            }
                        }
                    }
                }
                
                // Auto-start processing if launched from context menu
                if has_startup_items {
                    if let Some(st) = get_state() {
                        if !st.batch_items.is_empty() {
                            // Get algorithm from first startup item
                            let startup_items = crate::get_startup_items();
                            let algo = startup_items.first().map(|i| i.algorithm).unwrap_or(WofAlgorithm::Xpress8K);
                            
                            if let Some(ctrls) = &st.controls {
                                // Update status
                                let status_msg = format!("Processing {} item(s)...", st.batch_items.len());
                                let wstr = windows::core::HSTRING::from(&status_msg);
                                SetWindowTextW(ctrls.status_bar.label_hwnd(), PCWSTR::from_raw(wstr.as_ptr()));
                                
                                // Enable cancel button
                                EnableWindow(ctrls.action_panel.cancel_hwnd(), true);
                                
                                // Set taskbar progress
                                if let Some(tb) = &st.taskbar {
                                    tb.set_state(TaskbarState::Normal);
                                }
                            }
                            
                            // Start processing
                            let tx = st.tx.clone();
                            let state = st.global_state.clone();
                            state.store(ProcessingState::Running as u8, Ordering::Relaxed);
                            
                            // Clone items with their individual actions and algorithms
                            let items: Vec<_> = st.batch_items.iter().enumerate().map(|(idx, item)| {
                                (item.path.clone(), item.action, idx)
                            }).collect();
                            
                            let force = st.force_compress;
                            let main_hwnd_usize = hwnd.0 as usize;

                            thread::spawn(move || {
                                batch_process_worker(items, algo, tx, state, force, main_hwnd_usize);
                            });
                        }
                    }
                }

                LRESULT(0)
            }
            
            // WM_APP + 3: Set Enable Force Stop
            // WPARAM: 0 = disable, 1 = enable
            0x8003 => {
                if let Some(st) = get_state() {
                    st.enable_force_stop = wparam.0 != 0;
                }
                LRESULT(0)
            },
            
            // WM_APP + 4: Query Force Stop
            // WPARAM: Pointer to null-terminated Utf16 process name
            // Return: 1 (Kill), 0 (Cancel)
            0x8004 => {
                let mut should_kill = false;
                if let Some(st) = get_state() {
                    if st.enable_force_stop {
                        should_kill = true;
                    } else {
                        // Show Dialog
                        let name_ptr = wparam.0 as *const u16;
                        let name = windows::core::PCWSTR(name_ptr).to_string().unwrap_or_default();
                        let is_dark = theme::resolve_mode(st.theme);
                        should_kill = crate::ui::dialogs::show_force_stop_dialog(hwnd, &name, is_dark);
                    }
                }
                LRESULT(if should_kill { 1 } else { 0 })
            },
            
            // WM_THEME_CHANGED (Custom Message)
            0x8001 => {
                if let Some(st) = get_state() {
                    let theme_val = wparam.0;
                    let new_theme = match theme_val {
                        0 => AppTheme::System,
                        1 => AppTheme::Dark,
                        2 => AppTheme::Light,
                        _ => st.theme,
                    };
                    st.theme = new_theme;
                    
                    // Use new theme API
                    let is_dark = theme::resolve_mode(st.theme);
                    theme::set_window_frame_theme(hwnd, is_dark);
                    if let Some(ctrls) = &mut st.controls {
                        ctrls.update_theme(is_dark, hwnd);
                    }
                    InvalidateRect(Some(hwnd), None, true);
                }
                LRESULT(0)
            }
            
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as u16;

                match id {
                    IDC_BTN_ADD_FILES => {
                        if let Ok(files) = pick_files() {
                            if let Some(st) = get_state() {
                                for file_path in files {
                                    // Check if already added
                                    let already_exists = st.batch_items.iter().any(|item| item.path == file_path);
                                    if !already_exists {
                                        let item_id = st.add_batch_item(file_path.clone());
                                        let logical_size = calculate_path_logical_size(&file_path);
                                        let disk_size = calculate_path_disk_size(&file_path);
                                        let detected_algo = detect_path_algorithm(&file_path);
                                        let logical_str = format_size(logical_size);
                                        let disk_str = format_size(disk_size);
                                        if let Some(ctrls) = &st.controls {
                                            // Get the batch item we just added
                                            if let Some(batch_item) = st.batch_items.iter().find(|i| i.id == item_id) {
                                                ctrls.file_list.add_item(item_id, batch_item, &logical_str, &disk_str, detected_algo);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    
                    IDC_BTN_ADD_FOLDER => {
                        if let Ok(folder) = pick_folder() {
                            if let Some(st) = get_state() {
                                // Check if already added
                                let already_exists = st.batch_items.iter().any(|item| item.path == folder);
                                if !already_exists {
                                    let item_id = st.add_batch_item(folder.clone());
                                    let logical_size = calculate_path_logical_size(&folder);
                                    let disk_size = calculate_path_disk_size(&folder);
                                    let detected_algo = detect_path_algorithm(&folder);
                                    let logical_str = format_size(logical_size);
                                    let disk_str = format_size(disk_size);
                                    if let Some(ctrls) = &st.controls {
                                        if let Some(batch_item) = st.batch_items.iter().find(|i| i.id == item_id) {
                                            ctrls.file_list.add_item(item_id, batch_item, &logical_str, &disk_str, detected_algo);
                                        }
                                    }
                                }
                            }
                        }
                    },
                    
                    IDC_BTN_REMOVE => {
                        if let Some(st) = get_state() {
                            // Collect selected indices first using facade
                            let mut selected_indices = if let Some(ctrls) = &st.controls {
                                ctrls.file_list.get_selected_indices()
                            } else {
                                Vec::new()
                            };
                            
                            // Sort descending to remove from end first (preserves indices)
                            selected_indices.sort_by(|a, b| b.cmp(a));
                            
                            // Collect IDs to remove before doing any mutations
                            let ids_to_remove: Vec<u32> = selected_indices.iter()
                                .filter_map(|&idx| st.batch_items.get(idx).map(|item| item.id))
                                .collect();
                            
                            // Remove from state first
                            for id in ids_to_remove {
                                st.remove_batch_item(id);
                            }
                            
                            // Then remove from ListView
                            if let Some(ctrls) = &st.controls {
                                for idx in selected_indices {
                                    ctrls.file_list.remove_item(idx as i32);
                                }
                            }
                        }
                    },
                    
                    IDC_BTN_PROCESS_ALL => {
                        if let Some(st) = get_state() {
                            if st.batch_items.is_empty() {
                                MessageBoxW(Some(hwnd), w!("Add folders first!"), w!("Info"), MB_OK | MB_ICONINFORMATION);
                            } else {
                                if let Some(ctrls) = &st.controls {
                                    // Collect indices to process using facade
                                    let mut indices_to_process = ctrls.file_list.get_selected_indices();
                                    
                                    // If no selection, process all
                                    if indices_to_process.is_empty() {
                                        indices_to_process = (0..st.batch_items.len()).collect();
                                    }
                                    
                                    // Get selected algorithm
                                    let idx = SendMessageW(ctrls.action_panel.combo_hwnd(), CB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));
                                    let algo = match idx.0 {
                                        0 => WofAlgorithm::Xpress4K,
                                        2 => WofAlgorithm::Xpress16K,
                                        3 => WofAlgorithm::Lzx,
                                        _ => WofAlgorithm::Xpress8K,
                                    };
                                    
                                    // Get algorithm name for display
                                    let algo_name = match algo {
                                        WofAlgorithm::Xpress4K => "XPRESS4K",
                                        WofAlgorithm::Xpress8K => "XPRESS8K",
                                        WofAlgorithm::Xpress16K => "XPRESS16K",
                                        WofAlgorithm::Lzx => "LZX",
                                    };
                                    
                                    // Update Algorithm column for processed items using facade (Col 2)
                                    for &row in &indices_to_process {
                                        ctrls.file_list.update_item_text(row as i32, 2, algo_name);
                                    }
                                    
                                    if let Some(tb) = &st.taskbar {
                                        tb.set_state(TaskbarState::Normal);
                                    }
                                    
                                    EnableWindow(ctrls.action_panel.cancel_hwnd(), true);
                                    let status_msg = if indices_to_process.len() == st.batch_items.len() {
                                        "Processing all items...".to_string()
                                    } else {
                                        format!("Processing {} selected items...", indices_to_process.len())
                                    };
                                    let wstr = windows::core::HSTRING::from(&status_msg);
                                    SetWindowTextW(ctrls.status_bar.label_hwnd(), PCWSTR::from_raw(wstr.as_ptr()));
                                    
                                    let tx = st.tx.clone();
                                    let state = st.global_state.clone();
                                    state.store(ProcessingState::Running as u8, Ordering::Relaxed);
                                    
                                    // Get action mode
                                    let action_mode_idx = SendMessageW(ctrls.action_panel.action_mode_hwnd(), CB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));
                                    
                                    // Clone items for worker thread, applying action mode override
                                    let items: Vec<_> = indices_to_process.into_iter().filter_map(|idx| {
                                        st.batch_items.get(idx).map(|item| {
                                            let effective_action = match action_mode_idx.0 {
                                                0 => item.action, // As Listed - use individual action
                                                1 => BatchAction::Compress, // Compress All
                                                2 => BatchAction::Decompress, // Decompress All
                                                _ => item.action, // Fallback to individual
                                            };
                                            (item.path.clone(), effective_action, idx)
                                        })
                                    }).collect();
                                    
                                    let force = st.force_compress; // Capture force flag
                                    let main_hwnd_usize = hwnd.0 as usize; // Cast to usize for thread safety

                                    thread::spawn(move || {
                                        batch_process_worker(items, algo, tx, state, force, main_hwnd_usize);
                                    });
                                }
                            }
                        }
                    },
                    
                    IDC_BTN_CANCEL => {
                        if let Some(st) = get_state() {
                            // Stop all processing globally
                            st.global_state.store(ProcessingState::Stopped as u8, Ordering::Relaxed);
                            
                            // Also stop all individual items
                            for item in &st.batch_items {
                                if let Some(state_flag) = &item.state_flag {
                                    state_flag.store(ProcessingState::Stopped as u8, Ordering::Relaxed);
                                }
                            }
                            
                            if let Some(tb) = &st.taskbar {
                                tb.set_state(TaskbarState::Paused);
                            }
                            if let Some(ctrls) = &st.controls {
                                let _ = EnableWindow(ctrls.action_panel.cancel_hwnd(), false);
                                let _ = SetWindowTextW(ctrls.status_bar.label_hwnd(), w!("Stopping..."));
                            }
                        }
                    },
                    
                    IDC_BTN_SETTINGS => {
                        if let Some(st) = get_state() {
                            let current_theme = st.theme;
                            let is_dark = theme::resolve_mode(st.theme);
                            let enable_ctx = st.config.enable_context_menu;
                            // Modal will block until closed
                            let (new_theme, new_force, new_ctx) = show_settings_modal(hwnd, current_theme, is_dark, st.enable_force_stop, enable_ctx);
                            if let Some(t) = new_theme {
                                st.theme = t;
                                let new_is_dark = theme::resolve_mode(st.theme);
                                theme::set_window_frame_theme(hwnd, new_is_dark);
                                if let Some(ctrls) = &mut st.controls {
                                    ctrls.update_theme(new_is_dark, hwnd);
                                }
                                InvalidateRect(Some(hwnd), None, true);
                            }
                            st.enable_force_stop = new_force;
                            st.config.enable_context_menu = new_ctx;
                        }
                    },
                    
                    IDC_BTN_ABOUT => {
                        if let Some(st) = get_state() {
                            let is_dark = theme::resolve_mode(st.theme);
                            show_about_modal(hwnd, is_dark);
                        }
                    },

                    IDC_BTN_CONSOLE => {
                        if let Some(st) = get_state() {
                             let is_dark = theme::resolve_mode(st.theme);
                             show_console_window(hwnd, &st.logs, is_dark);
                        }
                    },

                    IDC_CHK_FORCE => {
                         if let Some(st) = get_state() {
                             // LPARAM is the HWND of the control in WM_COMMAND
                             let hwnd_ctl = HWND(lparam.0 as *mut _);
                             let state = SendMessageW(hwnd_ctl, BM_GETCHECK, None, None);
                             st.force_compress = state == LRESULT(BST_CHECKED.0 as isize);
                         }
                    },
                    
                    
                    _ => {}
                }
                LRESULT(0)
            }
            
            WM_TIMER => {
                if let Some(st) = get_state() {
                    loop {
                        match st.rx.try_recv() {
                            Ok(msg) => {
                                match msg {
                                    UiMessage::Progress(cur, total) => {
                                        if let Some(ctrls) = &st.controls {
                                            SendMessageW(ctrls.status_bar.progress_hwnd(), PBM_SETRANGE32, Some(WPARAM(0)), Some(LPARAM(total as isize)));
                                            SendMessageW(ctrls.status_bar.progress_hwnd(), PBM_SETPOS, Some(WPARAM(cur as usize)), Some(LPARAM(0)));
                                        }
                                        if let Some(tb) = &st.taskbar {
                                            tb.set_value(cur, total);
                                        }
                                    },
                                    UiMessage::Status(text) => {
                                        if let Some(st) = get_state() {
                                            if let Some(ctrls) = &st.controls {
                                                let wstr = windows::core::HSTRING::from(&text);
                                                SetWindowTextW(ctrls.status_bar.label_hwnd(), PCWSTR::from_raw(wstr.as_ptr()));
                                            }
                                        }
                                    },
                                    UiMessage::Log(text) => {
                                        if let Some(st) = get_state() {
                                            st.logs.push(text.clone());
                                            append_log_msg(&text);
                                            
                                            // Also update status text? Optional.
                                            if let Some(ctrls) = &st.controls {
                                                let wstr = windows::core::HSTRING::from(&text);
                                                SetWindowTextW(ctrls.status_bar.label_hwnd(), PCWSTR::from_raw(wstr.as_ptr()));
                                            }
                                        }
                                    },
                                    UiMessage::Error(text) => {
                                         if let Some(st) = get_state() {
                                             if let Some(tb) = &st.taskbar {
                                                 tb.set_state(TaskbarState::Error);
                                             }
                                             let full_msg = format!("ERROR: {}", text);
                                             st.logs.push(full_msg.clone());
                                             append_log_msg(&full_msg);
                                             
                                             // Update status text
                                             if let Some(ctrls) = &st.controls {
                                                let wstr = windows::core::HSTRING::from(&text);
                                                SetWindowTextW(ctrls.status_bar.label_hwnd(), PCWSTR::from_raw(wstr.as_ptr()));
                                             }
                                         }
                                    },
                                    UiMessage::Finished => {
                                        if let Some(st) = get_state() {
                                            if let Some(tb) = &st.taskbar {
                                                tb.set_state(TaskbarState::NoProgress);
                                            }
                                            if let Some(ctrls) = &st.controls {
                                                EnableWindow(ctrls.action_panel.cancel_hwnd(), false);
                                            }
                                        }
                                    },
                                    UiMessage::RowUpdate(row, progress, status, _size_after) => {
                                        // Update Progress column (col 6) and Status column (col 7)
                                        if let Some(ctrls) = &st.controls {
                                            ctrls.file_list.update_item_text(row, 6, &progress);
                                            ctrls.file_list.update_item_text(row, 7, &status);
                                        }
                                    },
                                    UiMessage::ItemFinished(row, status, disk_size_str, final_state) => {
                                        // Update Status (col 7) and On Disk column (col 5) with compressed size
                                        if let Some(ctrls) = &st.controls {
                                            ctrls.file_list.update_item_text(row, 7, &status);
                                            // Update On Disk column with the new compressed size
                                            if !disk_size_str.is_empty() {
                                                ctrls.file_list.update_item_text(row, 5, &disk_size_str);
                                            }
                                            
                                            // Update Current column (col 1) with final state
                                            let state_str = match final_state {
                                                CompressionState::None => "-",
                                                CompressionState::Specific(algo) => match algo {
                                                    WofAlgorithm::Xpress4K => "XPRESS4K",
                                                    WofAlgorithm::Xpress8K => "XPRESS8K",
                                                    WofAlgorithm::Xpress16K => "XPRESS16K",
                                                    WofAlgorithm::Lzx => "LZX",
                                                },
                                                CompressionState::Mixed => "Mixed",
                                            };
                                            ctrls.file_list.update_item_text(row, 1, state_str);
                                            
                                            // Reset button to "Start"
                                            ctrls.file_list.update_item_text(row, 8, "▶ Start");
                                            // Update item status in AppState
                                            if let Some(item) = st.batch_items.get_mut(row as usize) {
                                                item.status = BatchStatus::Pending;
                                                item.state_flag = None; // Clear the state flag
                                            }
                                        }
                                    },
                                    UiMessage::BatchItemAnalyzed(id, logical_size, disk_size, state) => {
                                        let logical_str = format_size(logical_size);
                                        let disk_str = format_size(disk_size);
                                        
                                        // Find row index by ID
                                        if let Some(pos) = st.batch_items.iter().position(|item| item.id == id) {
                                            if let Some(ctrls) = &st.controls {
                                                // Update ListView columns using facade:
                                                // 1: Current
                                                // 2: Algorithm
                                                // 4: Size
                                                // 5: On Disk
                                                // 7: Status
                                                ctrls.file_list.update_item_text(pos as i32, 4, &logical_str);
                                                ctrls.file_list.update_item_text(pos as i32, 5, &disk_str);
                                                
                                                // Update Current State Column (1)
                                                let state_str = match state {
                                                    CompressionState::None => "-",
                                                    CompressionState::Specific(algo) => match algo {
                                                        WofAlgorithm::Xpress4K => "XPRESS4K",
                                                        WofAlgorithm::Xpress8K => "XPRESS8K",
                                                        WofAlgorithm::Xpress16K => "XPRESS16K",
                                                        WofAlgorithm::Lzx => "LZX",
                                                    },
                                                    CompressionState::Mixed => "Mixed",
                                                };
                                                ctrls.file_list.update_item_text(pos as i32, 1, state_str);
                                                
                                                // If Specific(algo), also update target algo if we want? 
                                                // No, Requirement says "Update the new 'Current' column with this text".
                                                // It does NOT say to auto-update the target algorithm.
                                                // So we leave Algorithm (Col 2) as default (Xpress8K).

                                                // Reset status to Pending (from Calculating...)
                                                ctrls.file_list.update_item_text(pos as i32, 7, "Pending");

                                                let msg = format!("{} item(s) analyzed.", st.batch_items.len());
                                                let wstr = windows::core::HSTRING::from(&msg);
                                                SetWindowTextW(ctrls.status_bar.label_hwnd(), PCWSTR::from_raw(wstr.as_ptr()));
                                            }
                                        }
                                    },
                                    _ => {}
                                }
                            },
                            Err(_) => break,
                        }
                    }
                }
                LRESULT(0)
            }
            
            WM_SIZE => {
                // Get client rect for component layout
                let mut client_rect = windows::Win32::Foundation::RECT::default();
                if GetClientRect(hwnd, &mut client_rect).is_ok() {
                    // Delegate resize to components
                    if let Some(st) = get_state() {
                        if let Some(ctrls) = &mut st.controls {
                            ctrls.status_bar.on_resize(&client_rect);
                            ctrls.file_list.on_resize(&client_rect);
                            ctrls.action_panel.on_resize(&client_rect);
                            ctrls.header_panel.on_resize(&client_rect);
                        }
                    }
                }
                
                LRESULT(0)
            }
            
            WM_DESTROY => {
                if let Some(state) = get_window_state::<AppState>(hwnd) {
                    // Capture window position/size
                    let mut rect = windows::Win32::Foundation::RECT::default();
                    if GetWindowRect(hwnd, &mut rect).is_ok() {
                        state.config.window_x = rect.left;
                        state.config.window_y = rect.top;
                        state.config.window_width = rect.right - rect.left;
                        state.config.window_height = rect.bottom - rect.top;
                    }
                    
                    // Capture current UI states
                    if let Some(ctrls) = &state.controls {
                        // Get selected algorithm
                        let algo_idx = SendMessageW(ctrls.action_panel.combo_hwnd(), CB_GETCURSEL, None, None).0;
                        state.config.default_algo = match algo_idx {
                            0 => WofAlgorithm::Xpress4K,
                            2 => WofAlgorithm::Xpress16K,
                            3 => WofAlgorithm::Lzx,
                            _ => WofAlgorithm::Xpress8K,
                        };
                        
                        // Get force checkbox state
                        let force_state = SendMessageW(ctrls.action_panel.force_hwnd(), BM_GETCHECK, None, None);
                        state.config.force_compress = force_state == LRESULT(BST_CHECKED.0 as isize);
                    }
                    
                    // Save other settings
                    state.config.theme = state.theme;
                    state.config.enable_force_stop = state.enable_force_stop;
                    
                    // Save config to file
                    state.config.save();
                }
                
                // Clean up state allocation
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                if ptr != 0 {
                    let _ = Box::from_raw(ptr as *mut AppState);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                PostQuitMessage(0);
                LRESULT(0)
            }

            WM_DROPFILES => {
                let hdrop = HDROP(wparam.0 as *mut _);
                let mut buffer = [0u16; 1024];
                let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
                
                let mut paths = Vec::new();
                for i in 0..count {
                    let len = DragQueryFileW(hdrop, i, Some(&mut buffer));
                    if len > 0 {
                        let path_string = String::from_utf16_lossy(&buffer[..len as usize]);
                        paths.push(path_string);
                    }
                }
                DragFinish(hdrop);

                if let Some(st) = get_state() {
                     if let Some(ctrls) = &st.controls {
                         SetWindowTextW(ctrls.status_bar.label_hwnd(), w!("Analyzing dropped files..."));
                     }
                     
                     // 1. Add Placeholders immediately to UI
                     let mut items_to_analyze: Vec<(u32, String)> = Vec::new();
                     for path in paths {
                         // Check duplicates (simple O(N) check is fine for drag-drop)
                         if !st.batch_items.iter().any(|item| item.path == path) {
                             let id = st.add_batch_item(path.clone());
                             if let Some(ctrls) = &st.controls {
                                 // Get the batch item we just added and use facade
                                 if let Some(batch_item) = st.batch_items.iter().find(|i| i.id == id) {
                                     ctrls.file_list.add_item(id, batch_item, "Calculating...", "Calculating...", CompressionState::None);
                                 }
                             }
                             items_to_analyze.push((id, path));
                         }
                     }
                     
                     // 2. Spawn thread to analyze
                     let tx = st.tx.clone();
                     thread::spawn(move || {
                         for (id, path) in items_to_analyze {
                             let logical_size = calculate_path_logical_size(&path);
                             let disk_size = calculate_path_disk_size(&path);
                             let detected_algo = detect_path_algorithm(&path);
                             let _ = tx.send(UiMessage::BatchItemAnalyzed(id, logical_size, disk_size, detected_algo));
                         }
                         // Update status when done
                         let _ = tx.send(UiMessage::Status("Ready.".to_string()));
                     });
                }
                
                LRESULT(0)
            }

            0x001A => { // WM_SETTINGCHANGE
                // System theme may have changed - update if using System theme
                if let Some(st) = get_state() {
                    if st.theme == AppTheme::System {
                        let is_dark = theme::resolve_mode(st.theme);
                        theme::set_window_frame_theme(hwnd, is_dark);
                        if let Some(ctrls) = &mut st.controls {
                            ctrls.update_theme(is_dark, hwnd);
                        }
                        InvalidateRect(Some(hwnd), None, true);
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            
            WM_NOTIFY => {
                let nmhdr = &*(lparam.0 as *const windows::Win32::UI::Controls::NMHDR);
                
                // Check if it's from our ListView
                if nmhdr.idFrom == IDC_BATCH_LIST as usize {
                    if nmhdr.code == NM_DBLCLK {
                        let nmia = &*(lparam.0 as *const NMITEMACTIVATE);
                        let row = nmia.iItem;
                        let col = nmia.iSubItem;
                        
                        if row >= 0 {
                            if let Some(st) = get_state() {
                                let row_idx = row as usize;
                                
                                // Column 0 = Path - Open file location in Explorer
                                if col == 0 {
                                    if let Some(item) = st.batch_items.get(row_idx) {
                                        let path = &item.path;
                                        // Format: /select,"C:\Path With Spaces\File.txt"
                                        let args = format!("/select,\"{}\"", path);
                                        let args_wide = args.to_wide();
                                        
                                        ShellExecuteW(
                                            None,
                                            w!("open"),
                                            w!("explorer.exe"),
                                            PCWSTR(args_wide.as_ptr()),
                                            None,
                                            SW_SHOWNORMAL,
                                        );
                                    }
                                } else 
                                // Column 2 = Algorithm, Column 3 = Action, Column 8 = Start
                                if col == 2 {
                                    // Cycle Algorithm
                                    if let Some(item) = st.batch_items.get_mut(row_idx) {
                                        item.algorithm = match item.algorithm {
                                            WofAlgorithm::Xpress4K => WofAlgorithm::Xpress8K,
                                            WofAlgorithm::Xpress8K => WofAlgorithm::Xpress16K,
                                            WofAlgorithm::Xpress16K => WofAlgorithm::Lzx,
                                            WofAlgorithm::Lzx => WofAlgorithm::Xpress4K,
                                        };
                                        let algo_str = match item.algorithm {
                                            WofAlgorithm::Xpress4K => "XPRESS4K",
                                            WofAlgorithm::Xpress8K => "XPRESS8K",
                                            WofAlgorithm::Xpress16K => "XPRESS16K",
                                            WofAlgorithm::Lzx => "LZX",
                                        };
                                        if let Some(ctrls) = &st.controls {
                                            ctrls.file_list.update_item_text(row, 2, algo_str);
                                        }
                                    }
                                } else if col == 3 {
                                    // Toggle Action
                                    if let Some(item) = st.batch_items.get_mut(row_idx) {
                                        item.action = match item.action {
                                            BatchAction::Compress => BatchAction::Decompress,
                                            BatchAction::Decompress => BatchAction::Compress,
                                        };
                                        let action_str = match item.action {
                                            BatchAction::Compress => "Compress",
                                            BatchAction::Decompress => "Decompress",
                                        };
                                        if let Some(ctrls) = &st.controls {
                                            ctrls.file_list.update_item_text(row, 3, action_str);
                                        }
                                    }
                                } else if col == 8 {
                                    // Start/Pause/Resume button clicked
                                    if let Some(item) = st.batch_items.get_mut(row_idx) {
                                        // Check current state
                                        if let Some(state_flag) = &item.state_flag {
                                            let current_state = ProcessingState::from_u8(state_flag.load(Ordering::Relaxed));
                                            match current_state {
                                                ProcessingState::Running => {
                                                    // Pause
                                                    state_flag.store(ProcessingState::Paused as u8, Ordering::Relaxed);
                                                    if let Some(ctrls) = &st.controls {
                                                        ctrls.file_list.update_item_text(row, 8, "▶ Resume");
                                                        ctrls.file_list.update_item_text(row, 7, "Paused");
                                                    }
                                                }
                                                ProcessingState::Paused => {
                                                    // Resume
                                                    state_flag.store(ProcessingState::Running as u8, Ordering::Relaxed);
                                                    if let Some(ctrls) = &st.controls {
                                                        ctrls.file_list.update_item_text(row, 8, "⏸ Pause");
                                                        ctrls.file_list.update_item_text(row, 7, "Running");
                                                    }
                                                }
                                                ProcessingState::Idle | ProcessingState::Stopped => {
                                                    // Restart - start fresh
                                                    let path = item.path.clone();
                                                    let algo = item.algorithm;
                                                    let action = item.action;
                                                    let tx = st.tx.clone();
                                                    
                                                    // Create new state flag
                                                    let new_state = Arc::new(AtomicU8::new(ProcessingState::Running as u8));
                                                    item.state_flag = Some(new_state.clone());
                                                    item.status = BatchStatus::Processing;
                                                    
                                                    if let Some(ctrls) = &st.controls {
                                                        ctrls.file_list.update_item_text(row, 8, "⏸ Pause");
                                                        ctrls.file_list.update_item_text(row, 7, "Running");
                                                    }
                                                    
                                                    let force = st.force_compress;
                                                    let row_for_thread = row;
                                                    let main_hwnd_usize = hwnd.0 as usize;
                                                    thread::spawn(move || {
                                                        single_item_worker(path, algo, action, row_for_thread, tx, new_state, force, main_hwnd_usize);
                                                    });
                                                }
                                            }
                                        } else {
                                            // No state_flag = not running, start processing
                                            let path = item.path.clone();
                                            let algo = item.algorithm;
                                            let action = item.action;
                                            let tx = st.tx.clone();
                                            
                                            // Create per-item state flag
                                            let state_flag = Arc::new(AtomicU8::new(ProcessingState::Running as u8));
                                            item.state_flag = Some(state_flag.clone());
                                            item.status = BatchStatus::Processing;
                                            
                                            // Update status and Button using facade
                                            if let Some(ctrls) = &st.controls {
                                                ctrls.file_list.update_item_text(row, 8, "⏸ Pause");
                                                ctrls.file_list.update_item_text(row, 7, "Running");
                                            }
                                            
                                            let force = st.force_compress;
                                            let row_for_thread = row;
                                            let main_hwnd_usize = hwnd.0 as usize;
                                            thread::spawn(move || {
                                                single_item_worker(path, algo, action, row_for_thread, tx, state_flag, force, main_hwnd_usize);
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    if nmhdr.code == LVN_ITEMCHANGED {
                         // Check selection count using facade
                         if let Some(st) = get_state() {
                              if let Some(ctrls) = &st.controls {
                                  let count = ctrls.file_list.get_selection_count();
                                  
                                  if count > 0 {
                                      SetWindowTextW(ctrls.action_panel.process_hwnd(), w!("Process Selected"));
                                  } else {
                                      SetWindowTextW(ctrls.action_panel.process_hwnd(), w!("Process All"));
                                  }
                              }
                         }
                    }
                }
                
                // Note: Header NM_CUSTOMDRAW is handled by listview_subclass_proc
                // Header sends NM_CUSTOMDRAW to its parent (ListView), not to main window
                
                LRESULT(0)
            }
            
            WM_CONTEXTMENU => {
                // Check if context menu is for our ListView
                let hwnd_from = HWND(wparam.0 as *mut _);
                if let Some(st) = get_state() {
                    if let Some(ctrls) = &st.controls {
                        if hwnd_from == ctrls.file_list.hwnd() {
                            // Get selected item indices
                            let selected = ctrls.file_list.get_selected_indices();
                            
                            if !selected.is_empty() {
                                // Get cursor position for menu
                                let mut pt = windows::Win32::Foundation::POINT::default();
                                GetCursorPos(&mut pt);
                                
                                // Create popup menu
                                let menu = CreatePopupMenu();
                                if let Ok(menu) = menu {
                                    // Menu item IDs
                                    const ID_PAUSE: u32 = 1001;
                                    const ID_RESUME: u32 = 1002;
                                    const ID_STOP: u32 = 1003;
                                    
                                    // Check first selected item's state
                                    let first_idx = selected[0];
                                    if let Some(item) = st.batch_items.get(first_idx) {
                                        if let Some(state_flag) = &item.state_flag {
                                            let current_state = ProcessingState::from_u8(state_flag.load(Ordering::Relaxed));
                                            match current_state {
                                                ProcessingState::Running => {
                                                    let _ = AppendMenuW(menu, MF_STRING, ID_PAUSE as usize, w!("⏸ Pause"));
                                                    let _ = AppendMenuW(menu, MF_STRING, ID_STOP as usize, w!("■ Stop"));
                                                }
                                                ProcessingState::Paused => {
                                                    let _ = AppendMenuW(menu, MF_STRING, ID_RESUME as usize, w!("▶ Resume"));
                                                    let _ = AppendMenuW(menu, MF_STRING, ID_STOP as usize, w!("■ Stop"));
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    
                                    // Show menu and get selection
                                    let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_LEFTALIGN, pt.x, pt.y, Some(0), hwnd, None);
                                    let _ = DestroyMenu(menu);
                                    
                                    // Handle menu selection
                                    if cmd.as_bool() {
                                        let cmd_id = cmd.0 as u32;
                                        for &idx in &selected {
                                            if let Some(item) = st.batch_items.get(idx) {
                                                if let Some(state_flag) = &item.state_flag {
                                                    match cmd_id {
                                                        ID_PAUSE => {
                                                            state_flag.store(ProcessingState::Paused as u8, Ordering::Relaxed);
                                                            ctrls.file_list.update_item_text(idx as i32, 8, "▶ Resume");
                                                            ctrls.file_list.update_item_text(idx as i32, 7, "Paused");
                                                        }
                                                        ID_RESUME => {
                                                            state_flag.store(ProcessingState::Running as u8, Ordering::Relaxed);
                                                            ctrls.file_list.update_item_text(idx as i32, 8, "⏸ Pause");
                                                            ctrls.file_list.update_item_text(idx as i32, 7, "Running");
                                                        }
                                                        ID_STOP => {
                                                            state_flag.store(ProcessingState::Stopped as u8, Ordering::Relaxed);
                                                            ctrls.file_list.update_item_text(idx as i32, 7, "Stopping...");
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                LRESULT(0)
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}


/// Pick files (multi-select)
unsafe fn pick_files() -> Result<Vec<String>> {
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)?;
        let options = dialog.GetOptions()?;
        dialog.SetOptions(options | FOS_FORCEFILESYSTEM | windows::Win32::UI::Shell::FOS_ALLOWMULTISELECT)?;
        dialog.Show(None)?;
        
        let results = dialog.GetResults()?;
        let count = results.GetCount()?;
        let mut paths = Vec::new();
        
        for i in 0..count {
            if let Ok(item) = results.GetItemAt(i) {
                if let Ok(path_ptr) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                    if let Ok(path) = path_ptr.to_string() {
                        paths.push(path);
                    }
                    CoTaskMemFree(Some(path_ptr.as_ptr() as *mut _));
                }
            }
        }
        
        Ok(paths)
    }
}

/// Pick folder (single folder selection)
unsafe fn pick_folder() -> Result<String> {
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)?;
        let options = dialog.GetOptions()?;
        dialog.SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)?;
        dialog.Show(None)?;
        let item = dialog.GetResult()?;
        let path_ptr = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = path_ptr.to_string()?;
        CoTaskMemFree(Some(path_ptr.as_ptr() as *mut _));
        Ok(path)
    }
}
