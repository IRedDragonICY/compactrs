#![allow(unsafe_op_in_unsafe_fn)]
use windows::core::{Result, w, PCWSTR, PCSTR};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    HBRUSH, InvalidateRect, CreateSolidBrush, FillRect, HDC, CreateFontW, 
    DEFAULT_PITCH, FF_DONTCARE, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, 
    FW_NORMAL, DEFAULT_CHARSET,
};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, PostQuitMessage, RegisterClassW, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, SW_SHOW, WM_DESTROY, WNDCLASSW,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE, WM_CREATE, WM_SIZE, WM_COMMAND, SetWindowPos, SWP_NOZORDER,
    GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA, GetDlgItem, WM_DROPFILES, MessageBoxW, MB_OK,
    SendMessageW, CB_ADDSTRING, CB_SETCURSEL, CB_GETCURSEL, SetWindowTextW, WS_CHILD, HMENU, WM_TIMER, SetTimer,
    MB_ICONINFORMATION, WM_NOTIFY, WM_SETFONT, BM_GETCHECK, WM_ERASEBKGND, GetClientRect, GetWindowRect,
    BM_SETCHECK, ChangeWindowMessageFilterEx, MSGFLT_ALLOW, WM_COPYDATA,
};
use windows::Win32::UI::Shell::{DragQueryFileW, DragFinish, HDROP, FileOpenDialog, IFileOpenDialog, FOS_PICKFOLDERS, FOS_FORCEFILESYSTEM, SIGDN_FILESYSPATH, DragAcceptFiles};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, CoTaskMemFree};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW, GetProcAddress};

use crate::gui::controls::{
    create_button, ButtonOpts, create_combobox, create_progress_bar, 
    apply_button_theme, apply_combobox_theme,
    IDC_COMBO_ALGO, IDC_STATIC_TEXT, IDC_PROGRESS_BAR, IDC_BTN_CANCEL, IDC_BATCH_LIST, IDC_BTN_ADD_FOLDER,
    IDC_BTN_REMOVE, IDC_BTN_PROCESS_ALL, IDC_BTN_ADD_FILES, IDC_BTN_SETTINGS, IDC_BTN_ABOUT,
    IDC_BTN_CONSOLE, IDC_CHK_FORCE, create_checkbox,
};
use crate::gui::components::FileListView;
use crate::gui::settings::show_settings_modal;
use crate::gui::about::show_about_modal;
use crate::gui::console::{show_console_window, append_log_msg};
use crate::gui::state::{AppState, Controls, UiMessage, BatchAction, BatchStatus, AppTheme};
use crate::gui::taskbar::{TaskbarProgress, TaskbarState};
use std::thread;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use windows::Win32::UI::Controls::{
    PBM_SETRANGE32, PBM_SETPOS, SetWindowTheme,
    NM_DBLCLK, NMITEMACTIVATE,
    InitCommonControlsEx, INITCOMMONCONTROLSEX, ICC_WIN95_CLASSES, ICC_STANDARD_CLASSES,
    LVN_ITEMCHANGED, BST_CHECKED,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use crate::engine::wof::WofAlgorithm;
use crate::engine::worker::{
    batch_process_worker, single_item_worker, 
    calculate_path_logical_size, calculate_path_disk_size, detect_path_algorithm
};
use humansize::{format_size, BINARY};
use crate::config::AppConfig;
use crate::gui::utils::get_window_state;



#[allow(dead_code)]
fn lo_word(l: u32) -> u16 {
    (l & 0xffff) as u16
}

#[allow(dead_code)]
fn hi_word(l: u32) -> u16 {
    ((l >> 16) & 0xffff) as u16
}

const WINDOW_CLASS_NAME: PCWSTR = w!("CompactRS_Class");
const WINDOW_TITLE: PCWSTR = w!("CompactRS");

// ===== PATH-AWARE FUNCTIONS moved to engine::worker =====

#[allow(non_snake_case)]
unsafe fn allow_dark_mode() {
    unsafe {
        if let Ok(uxtheme) = LoadLibraryW(w!("uxtheme.dll")) {
            // Ordinal 135: SetPreferredAppMode
            if let Some(set_preferred_app_mode) = GetProcAddress(uxtheme, PCSTR(135 as *const u8)) {
                let set_preferred_app_mode: extern "system" fn(i32) -> i32 = std::mem::transmute(set_preferred_app_mode);
                set_preferred_app_mode(2); // 2 = AllowDark (or ForceDark)
            }
        }
    }
}


pub unsafe fn create_main_window(instance: HINSTANCE) -> Result<HWND> {
    unsafe {
        allow_dark_mode();
        
        // Initialize Common Controls to ensure Visual Styles are applied
        let iccex = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_WIN95_CLASSES | ICC_STANDARD_CLASSES,
        };
        InitCommonControlsEx(&iccex);


        // Check dark mode for window class background
        let is_dark = crate::gui::theme::ThemeManager::is_system_dark_mode();
        let (bg_brush, _, _) = crate::gui::theme::ThemeManager::get_theme_colors(is_dark);

        // Load icon (ID 1)
        let icon_handle = windows::Win32::UI::WindowsAndMessaging::LoadImageW(
            Some(instance),
            PCWSTR(1 as *const u16),
            windows::Win32::UI::WindowsAndMessaging::IMAGE_ICON,
            0, 0, // Default size
            windows::Win32::UI::WindowsAndMessaging::LR_DEFAULTSIZE | windows::Win32::UI::WindowsAndMessaging::LR_SHARED
        ).unwrap_or_default();
        
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            hIcon: windows::Win32::UI::WindowsAndMessaging::HICON(icon_handle.0),
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
        
        apply_backdrop(hwnd);
        ShowWindow(hwnd, SW_SHOW);
        update_theme(hwnd);

        Ok(hwnd)
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        // Use the centralized helper for state access
        let get_state = || get_window_state::<AppState>(hwnd);

        match msg {
            WM_CREATE => {
                let mut state = Box::new(AppState::new());
                state.taskbar = Some(TaskbarProgress::new(hwnd));
                
                // Create header label
                let h_label = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    w!("Drag and drop files or folders, or use 'Files'/'Folder' buttons. Then click 'Process All'."),
                    WS_CHILD | WS_VISIBLE,
                    10, 10, 860, 25,
                    Some(hwnd),
                    Some(HMENU(IDC_STATIC_TEXT as isize as *mut _)),
                    Some(HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0)),
                    None,
                ).unwrap_or_default();
                
                // Create batch ListView - uses FileListView facade
                let file_list = FileListView::new(hwnd, 10, 40, 860, 380, IDC_BATCH_LIST);
                
                // Create progress bar at y=430
                let h_progress = create_progress_bar(hwnd, 10, 430, 860, 20, IDC_PROGRESS_BAR);
                
                // Create ALL buttons at y=460 - Taller buttons (32px) for modern look
                // Files: x=10, width=65
                // Folder: x=85, width=65
                // Remove: x=160, width=70
                // Combo: x=240, width=110
                // Process: x=360, width=100
                // Cancel: x=470, width=80
                let btn_h = 32;
                let btn_y = 460;
                
                // Check if dark mode for initial theme application
                let is_dark_init = crate::gui::theme::ThemeManager::is_system_dark_mode();
                
                // Use ButtonOpts factory pattern (DRY - theme applied inside)
                let h_add_files = create_button(hwnd, ButtonOpts::new(w!("Files"), 10, btn_y, 65, btn_h, IDC_BTN_ADD_FILES, is_dark_init));
                let h_add_folder = create_button(hwnd, ButtonOpts::new(w!("Folder"), 85, btn_y, 65, btn_h, IDC_BTN_ADD_FOLDER, is_dark_init));
                let h_remove = create_button(hwnd, ButtonOpts::new(w!("Remove"), 160, btn_y, 70, btn_h, IDC_BTN_REMOVE, is_dark_init));
                let h_combo = create_combobox(hwnd, 240, btn_y, 110, 200, IDC_COMBO_ALGO);
                // Force Checkbox
                let h_force = create_checkbox(hwnd, w!("Force"), 360, btn_y, 60, btn_h, IDC_CHK_FORCE);
                let h_process = create_button(hwnd, ButtonOpts::new(w!("Process All"), 430, btn_y, 100, btn_h, IDC_BTN_PROCESS_ALL, is_dark_init));
                let h_cancel = create_button(hwnd, ButtonOpts::new(w!("Cancel"), 540, btn_y, 80, btn_h, IDC_BTN_CANCEL, is_dark_init));
                
                
                // Settings/About items
                let h_settings = create_button(hwnd, ButtonOpts::new(w!("\u{2699}"), 0, 0, 30, 25, IDC_BTN_SETTINGS, is_dark_init)); // Gear icon
                let h_about = create_button(hwnd, ButtonOpts::new(w!("?"), 0, 0, 30, 25, IDC_BTN_ABOUT, is_dark_init)); // About icon
                // Console button (using a simple ">_" or similar text)
                let h_console = create_button(hwnd, ButtonOpts::new(w!(">_"), 0, 0, 30, 25, IDC_BTN_CONSOLE, is_dark_init));

                // Apply dark theme to ComboBox and Checkbox
                if is_dark_init {
                    let _ = SetWindowTheme(h_combo, w!("DarkMode_CFD"), None);
                    let _ = SetWindowTheme(h_force, w!("DarkMode_Explorer"), None);
                }

                let _ = h_add_files; // Used via IDC_BTN_ADD_FILES
                EnableWindow(h_cancel, false);

                // Populate algorithm combo
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
                
                // Set initial force checkbox state
                if state.force_compress {
                    SendMessageW(h_force, BM_SETCHECK, Some(WPARAM(BST_CHECKED.0 as usize)), None);
                }

                state.controls = Some(Controls {
                    file_list,
                    btn_scan: h_add_folder,  // Reusing for Add Folder
                    btn_compress: h_process,  // Reusing for Process All
                    btn_decompress: h_remove,  // Reusing for Remove
                    combo_algo: h_combo,
                    static_text: h_label,
                    progress_bar: h_progress,
                    btn_cancel: h_cancel,
                    btn_settings: h_settings,
                    btn_about: h_about,
                    btn_console: h_console,
                    btn_force: h_force,
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
                
                // Apply saved theme (dark mode support for ListView)
                if let Some(st) = get_state() {
                    apply_theme(hwnd, st.theme);
                }

                LRESULT(0)
            }
            
            // WM_ERASEBKGND - Paint dark background when in dark mode
            WM_ERASEBKGND => {
                let hdc = HDC(wparam.0 as *mut _);
                let mut rect = windows::Win32::Foundation::RECT::default();
                if GetClientRect(hwnd, &mut rect).is_ok() {
                    let is_dark = is_app_dark_mode(hwnd);
                    let (brush, _, _) = crate::gui::theme::ThemeManager::get_theme_colors(is_dark);
                    FillRect(hdc, &rect, brush);
                    return LRESULT(1);
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
                        let is_dark = is_app_dark_mode(hwnd);
                        should_kill = crate::gui::dialogs::show_force_stop_dialog(hwnd, &name, is_dark);
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
                    update_theme(hwnd);
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
                                        let logical_str = format_size(logical_size, BINARY);
                                        let disk_str = format_size(disk_size, BINARY);
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
                                    let logical_str = format_size(logical_size, BINARY);
                                    let disk_str = format_size(disk_size, BINARY);
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
                                    let idx = SendMessageW(ctrls.combo_algo, CB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));
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
                                    
                                    // Update Algorithm column for processed items using facade
                                    for &row in &indices_to_process {
                                        ctrls.file_list.update_item_text(row as i32, 1, algo_name);
                                    }
                                    
                                    if let Some(tb) = &st.taskbar {
                                        tb.set_state(TaskbarState::Normal);
                                    }
                                    
                                    EnableWindow(ctrls.btn_cancel, true);
                                    let status_msg = if indices_to_process.len() == st.batch_items.len() {
                                        "Processing all items...".to_string()
                                    } else {
                                        format!("Processing {} selected items...", indices_to_process.len())
                                    };
                                    let wstr = windows::core::HSTRING::from(&status_msg);
                                    SetWindowTextW(ctrls.static_text, PCWSTR::from_raw(wstr.as_ptr()));
                                    
                                    let tx = st.tx.clone();
                                    let cancel = st.cancel_flag.clone();
                                    cancel.store(false, Ordering::Relaxed);
                                    
                                    // Clone items for worker thread
                                    let items: Vec<_> = indices_to_process.into_iter().filter_map(|idx| {
                                        st.batch_items.get(idx).map(|item| (item.path.clone(), item.action, idx))
                                    }).collect();
                                    
                                    let force = st.force_compress; // Capture force flag
                                    let main_hwnd_usize = hwnd.0 as usize; // Cast to usize for thread safety

                                    thread::spawn(move || {
                                        batch_process_worker(items, algo, tx, cancel, force, main_hwnd_usize);
                                    });
                                }
                            }
                        }
                    },
                    
                    IDC_BTN_CANCEL => {
                        if let Some(st) = get_state() {
                            st.cancel_flag.store(true, Ordering::Relaxed);
                            if let Some(tb) = &st.taskbar {
                                tb.set_state(TaskbarState::Paused);
                            }
                            if let Some(ctrls) = &st.controls {
                                let _ = EnableWindow(ctrls.btn_cancel, false);
                                let _ = SetWindowTextW(ctrls.static_text, w!("Cancelling..."));
                            }
                        }
                    },
                    
                    IDC_BTN_SETTINGS => {
                        if let Some(st) = get_state() {
                            let theme = st.theme;
                            let is_dark = is_app_dark_mode(hwnd);
                            // Modal will block until closed
                            let (new_theme, new_force) = show_settings_modal(hwnd, theme, is_dark, st.enable_force_stop);
                            if let Some(t) = new_theme {
                                st.theme = t;
                                apply_theme(hwnd, st.theme);
                            }
                            st.enable_force_stop = new_force;
                        }
                    },
                    
                    IDC_BTN_ABOUT => {
                        let is_dark = is_app_dark_mode(hwnd);
                        show_about_modal(hwnd, is_dark);
                    },

                    IDC_BTN_CONSOLE => {
                        if let Some(app_state) = get_window_state::<AppState>(hwnd) {
                             let is_dark = is_app_dark_mode(hwnd);
                             show_console_window(hwnd, &app_state.logs, is_dark);
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
                                            SendMessageW(ctrls.progress_bar, PBM_SETRANGE32, Some(WPARAM(0)), Some(LPARAM(total as isize)));
                                            SendMessageW(ctrls.progress_bar, PBM_SETPOS, Some(WPARAM(cur as usize)), Some(LPARAM(0)));
                                        }
                                        if let Some(tb) = &st.taskbar {
                                            tb.set_value(cur, total);
                                        }
                                    },
                                    UiMessage::Status(text) => {
                                        if let Some(st) = get_state() {
                                            if let Some(ctrls) = &st.controls {
                                                let wstr = windows::core::HSTRING::from(&text);
                                                SetWindowTextW(ctrls.static_text, PCWSTR::from_raw(wstr.as_ptr()));
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
                                                SetWindowTextW(ctrls.static_text, PCWSTR::from_raw(wstr.as_ptr()));
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
                                                SetWindowTextW(ctrls.static_text, PCWSTR::from_raw(wstr.as_ptr()));
                                             }
                                         }
                                    },
                                    UiMessage::Finished => {
                                        if let Some(st) = get_state() {
                                            if let Some(tb) = &st.taskbar {
                                                tb.set_state(TaskbarState::NoProgress);
                                            }
                                            if let Some(ctrls) = &st.controls {
                                                EnableWindow(ctrls.btn_cancel, false);
                                            }
                                        }
                                    },
                                    UiMessage::RowUpdate(row, progress, status, _size_after) => {
                                        // Update Progress column (col 5) and Status column (col 6)
                                        if let Some(ctrls) = &st.controls {
                                            ctrls.file_list.update_item_text(row, 5, &progress);
                                            ctrls.file_list.update_item_text(row, 6, &status);
                                        }
                                    },
                                    UiMessage::ItemFinished(row, status, disk_size_str) => {
                                        // Update Status (col 6) and On Disk column (col 4) with compressed size
                                        if let Some(ctrls) = &st.controls {
                                            ctrls.file_list.update_item_text(row, 6, &status);
                                            // Update On Disk column with the new compressed size
                                            if !disk_size_str.is_empty() {
                                                ctrls.file_list.update_item_text(row, 4, &disk_size_str);
                                            }
                                            // Reset button to "Start"
                                            ctrls.file_list.update_item_text(row, 7, "▶ Start");
                                            // Update item status in AppState
                                            if let Some(item) = st.batch_items.get_mut(row as usize) {
                                                item.status = BatchStatus::Pending;
                                                item.cancel_token = None; // Clear the token
                                            }
                                        }
                                    },
                                    UiMessage::BatchItemAnalyzed(id, logical_size, disk_size, algo) => {
                                        let logical_str = format_size(logical_size, BINARY);
                                        let disk_str = format_size(disk_size, BINARY);
                                        
                                        // Find row index by ID
                                        if let Some(pos) = st.batch_items.iter().position(|item| item.id == id) {
                                            if let Some(ctrls) = &st.controls {
                                                // Update ListView columns using facade:
                                                // 1: Algorithm (if detected)
                                                // 3: Size
                                                // 4: On Disk
                                                ctrls.file_list.update_item_text(pos as i32, 3, &logical_str);
                                                ctrls.file_list.update_item_text(pos as i32, 4, &disk_str);
                                                
                                                if let Some(a) = algo {
                                                    let algo_str = match a {
                                                        WofAlgorithm::Xpress4K => "XPRESS4K",
                                                        WofAlgorithm::Xpress8K => "XPRESS8K",
                                                        WofAlgorithm::Xpress16K => "XPRESS16K",
                                                        WofAlgorithm::Lzx => "LZX",
                                                    };
                                                    ctrls.file_list.update_item_text(pos as i32, 1, algo_str);
                                                }
                                                // Reset status to Pending (from Calculating...)
                                                ctrls.file_list.update_item_text(pos as i32, 6, "Pending");

                                                let msg = format!("{} item(s) analyzed.", st.batch_items.len());
                                                let wstr = windows::core::HSTRING::from(&msg);
                                                SetWindowTextW(ctrls.static_text, PCWSTR::from_raw(wstr.as_ptr()));
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
                let width = (lparam.0 & 0xFFFF) as i32;
                let height = ((lparam.0 >> 16) & 0xFFFF) as i32;
                
                let padding = 10;
                let btn_height = 30;
                let progress_height = 25;
                let header_height = 25;
                let list_height = height - header_height - progress_height - btn_height - (padding * 5);
                
                // Resize header
                // Resize header - leave space for settings (30), about (30), console (30) + spacing
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_STATIC_TEXT.into()) {
                    // Previous width: width - padding - 80 (was overlapping console)
                    // New width: width - padding - 120 (leaves space for 3 buttons + gaps)
                    SetWindowPos(h, None, padding, padding, width - padding - 120, header_height, SWP_NOZORDER);
                }
                
                // Position Settings button (Rightmost)
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BTN_SETTINGS.into()) {
                     SetWindowPos(h, None, width - padding - 30, padding, 30, header_height, SWP_NOZORDER);
                }

                // Position About button (Left of Settings)
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BTN_ABOUT.into()) {
                     SetWindowPos(h, None, width - padding - 65, padding, 30, header_height, SWP_NOZORDER);
                }

                // Position Console button (Left of About)
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BTN_CONSOLE.into()) {
                     SetWindowPos(h, None, width - padding - 100, padding, 30, header_height, SWP_NOZORDER);
                }
                
                // Resize ListView
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BATCH_LIST.into()) {
                    SetWindowPos(h, None, padding, padding + header_height + padding, width - padding * 2, list_height, SWP_NOZORDER);
                }
                
                // Resize progress bar
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_PROGRESS_BAR.into()) {
                    let y = padding + header_height + padding + list_height + padding;
                    SetWindowPos(h, None, padding, y, width - padding * 2, progress_height, SWP_NOZORDER);
                }
                
                // Position buttons at bottom - all on same row
                let btn_y = height - btn_height - padding;
                
                // Files button
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BTN_ADD_FILES.into()) {
                    SetWindowPos(h, None, padding, btn_y, 55, btn_height, SWP_NOZORDER);
                }
                // Folder button
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BTN_ADD_FOLDER.into()) {
                    SetWindowPos(h, None, padding + 60, btn_y, 55, btn_height, SWP_NOZORDER);
                }
                // Remove button
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BTN_REMOVE.into()) {
                    SetWindowPos(h, None, padding + 120, btn_y, 65, btn_height, SWP_NOZORDER);
                }
                // Algorithm combo
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_COMBO_ALGO.into()) {
                    SetWindowPos(h, None, padding + 190, btn_y, 110, btn_height, SWP_NOZORDER);
                }
                // Force Checkbox
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_CHK_FORCE.into()) {
                    SetWindowPos(h, None, padding + 310, btn_y, 60, btn_height, SWP_NOZORDER);
                }
                // Process All button
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BTN_PROCESS_ALL.into()) {
                    SetWindowPos(h, None, padding + 380, btn_y, 90, btn_height, SWP_NOZORDER);
                }
                // Cancel button
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BTN_CANCEL.into()) {
                    SetWindowPos(h, None, padding + 480, btn_y, 70, btn_height, SWP_NOZORDER);
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
                        let algo_idx = SendMessageW(ctrls.combo_algo, CB_GETCURSEL, None, None).0;
                        state.config.default_algo = match algo_idx {
                            0 => WofAlgorithm::Xpress4K,
                            2 => WofAlgorithm::Xpress16K,
                            3 => WofAlgorithm::Lzx,
                            _ => WofAlgorithm::Xpress8K,
                        };
                        
                        // Get force checkbox state
                        let force_state = SendMessageW(ctrls.btn_force, BM_GETCHECK, None, None);
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
                         SetWindowTextW(ctrls.static_text, w!("Analyzing dropped files..."));
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
                                     ctrls.file_list.add_item(id, batch_item, "Calculating...", "Calculating...", None);
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
                update_theme(hwnd);
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
                                
                                // Column 1 = Algorithm, Column 2 = Action
                                if col == 1 {
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
                                            ctrls.file_list.update_item_text(row, 1, algo_str);
                                        }
                                    }
                                } else if col == 2 {
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
                                            ctrls.file_list.update_item_text(row, 2, action_str);
                                        }
                                    }
                                } else if col == 7 {
                                    // Start/Stop button clicked
                                    if let Some(item) = st.batch_items.get_mut(row_idx) {
                                        // Check if running
                                        if let BatchStatus::Processing = item.status {
                                            // Stop
                                            if let Some(token) = &item.cancel_token {
                                                token.store(true, Ordering::Relaxed);
                                            }
                                            if let Some(ctrls) = &st.controls {
                                                ctrls.file_list.update_item_text(row, 7, "Stopping...");
                                            }
                                        } else {
                                            // Start
                                            let path = item.path.clone();
                                            let algo = item.algorithm;
                                            let action = item.action;
                                            let tx = st.tx.clone();
                                            
                                            // Create per-item cancellation token
                                            let token = Arc::new(AtomicBool::new(false));
                                            item.cancel_token = Some(token.clone());
                                            item.status = BatchStatus::Processing;
                                            
                                            // Update status and Button using facade
                                            if let Some(ctrls) = &st.controls {
                                                ctrls.file_list.update_item_text(row, 6, "Running");
                                                ctrls.file_list.update_item_text(row, 7, "■ Stop");
                                            }
                                            
                                            let force = st.force_compress; // Capture force flag
                                            
                                            let row_for_thread = row;
                                            let main_hwnd_usize = hwnd.0 as usize;
                                            thread::spawn(move || {
                                                single_item_worker(path, algo, action, row_for_thread, tx, token, force, main_hwnd_usize);
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
                                      SetWindowTextW(ctrls.btn_compress, w!("Process Selected"));
                                  } else {
                                      SetWindowTextW(ctrls.btn_compress, w!("Process All"));
                                  }
                              }
                         }
                    }
                }
                
                // Note: Header NM_CUSTOMDRAW is handled by listview_subclass_proc
                // Header sends NM_CUSTOMDRAW to its parent (ListView), not to main window
                
                LRESULT(0)
            }
            
            // WM_CTLCOLORSTATIC - handle static text colors
            0x0138 => { // WM_CTLCOLORSTATIC
                let is_dark = is_app_dark_mode(hwnd);
                if let Some(result) = crate::gui::theme::ThemeManager::handle_ctl_color(hwnd, wparam, is_dark) {
                    return result;
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            },


            
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

fn apply_backdrop(hwnd: HWND) {
    unsafe {
        // 1. Monitor System Dark Mode
        let is_dark = is_app_dark_mode(hwnd);
        crate::gui::theme::ThemeManager::apply_window_theme(hwnd, is_dark);
    }
}



// Check effective dark mode state (System or Override)
unsafe fn is_app_dark_mode(hwnd: HWND) -> bool {
    // Try to get AppState to check override
    if let Some(st) = get_window_state::<AppState>(hwnd) {
        match st.theme {
            AppTheme::Dark => return true,
            AppTheme::Light => return false,
            AppTheme::System => return crate::gui::theme::ThemeManager::is_system_dark_mode(),
        }
    }
    // Fallback if no state yet (e.g. during creation)
    crate::gui::theme::ThemeManager::is_system_dark_mode()
}

#[allow(non_snake_case)]
unsafe fn allow_dark_mode_for_window(hwnd: HWND, allow: bool) {
    unsafe {
        if let Ok(uxtheme) = LoadLibraryW(w!("uxtheme.dll")) {
            if let Some(func) = GetProcAddress(uxtheme, PCSTR(133 as *const u8)) {
                let allow_dark_mode_for_window: extern "system" fn(HWND, bool) -> bool = std::mem::transmute(func);
                allow_dark_mode_for_window(hwnd, allow);
            }
        }
    }
}

fn update_theme(hwnd: HWND) {
    unsafe {
        let dark = is_app_dark_mode(hwnd);
        let attr = 20; // DWMWA_USE_IMMERSIVE_DARK_MODE
        let val = if dark { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(attr), &val as *const _ as _, std::mem::size_of::<i32>() as u32);
        
        // Update progress bar with dark theme
        if let Ok(progress) = GetDlgItem(Some(hwnd), IDC_PROGRESS_BAR as i32) {
            if !progress.is_invalid() {
                allow_dark_mode_for_window(progress, dark);
                if dark {
                    let _ = SetWindowTheme(progress, w!("DarkMode_Explorer"), None);
                } else {
                    let _ = SetWindowTheme(progress, w!("Explorer"), None);
                }
            }
        }
        
        // Create modern font (Segoe UI Variable Display or Segoe UI)
        let font_height = -12; // ~9pt
        let hfont = CreateFontW(
            font_height,
            0, 0, 0,
            FW_NORMAL.0 as i32,
            0, 0, 0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            w!("Segoe UI Variable Display"),
        );
        
        // Helper to update button font and theme using shared function
        let update_btn = |id: u16| {
            if let Ok(btn) = GetDlgItem(Some(hwnd), id as i32) {
                if !btn.is_invalid() {
                    SendMessageW(btn, WM_SETFONT, Some(WPARAM(hfont.0 as usize)), Some(LPARAM(1)));
                    apply_button_theme(btn, dark);
                }
            }
        };
        
        // Update all buttons using shared apply_button_theme
        update_btn(IDC_BTN_ADD_FILES);
        update_btn(IDC_BTN_ADD_FOLDER);
        update_btn(IDC_BTN_REMOVE);
        update_btn(IDC_BTN_PROCESS_ALL);
        update_btn(IDC_BTN_CANCEL);
        update_btn(IDC_BTN_SETTINGS);
        update_btn(IDC_BTN_ABOUT);
        update_btn(IDC_BTN_CONSOLE);
        
        // Update ComboBox using shared apply_combobox_theme
        if let Ok(combo) = GetDlgItem(Some(hwnd), IDC_COMBO_ALGO as i32) {
             if !combo.is_invalid() {
                 SendMessageW(combo, WM_SETFONT, Some(WPARAM(hfont.0 as usize)), Some(LPARAM(1)));
                 apply_combobox_theme(combo, dark);
             }
        }
        
        // Update Force Checkbox using shared apply_button_theme (same theme as buttons)
        if let Ok(chk) = GetDlgItem(Some(hwnd), IDC_CHK_FORCE as i32) {
             if !chk.is_invalid() {
                 SendMessageW(chk, WM_SETFONT, Some(WPARAM(hfont.0 as usize)), Some(LPARAM(1)));
                 apply_button_theme(chk, dark);
             }
        }
        
        // Update Static Text Font
        if let Ok(static_text) = GetDlgItem(Some(hwnd), IDC_STATIC_TEXT as i32) {
            SendMessageW(static_text, WM_SETFONT, Some(WPARAM(hfont.0 as usize)), Some(LPARAM(1)));
        }
        
        // Update ListView using FileListView facade
        if let Some(st) = get_window_state::<AppState>(hwnd) {
            if let Some(ctrls) = &st.controls {
                // Apply theme using facade (handles colors, window theme, header)
                ctrls.file_list.set_theme(dark);
                
                // Apply subclass for header theming
                ctrls.file_list.apply_subclass(hwnd);
                
                // Also update font
                SendMessageW(ctrls.file_list.hwnd(), WM_SETFONT, Some(WPARAM(hfont.0 as usize)), Some(LPARAM(1)));
            }
        }
        
        // Force main window redraw
        let _ = InvalidateRect(Some(hwnd), None, true);
    }
}

// Store dark brush handle as isize for thread safety (HBRUSH is not Sync)
use std::sync::OnceLock;

static DARK_BRUSH_HANDLE: OnceLock<isize> = OnceLock::new();

fn get_dark_brush() -> HBRUSH {
    let handle = *DARK_BRUSH_HANDLE.get_or_init(|| {
        unsafe { 
            let brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00202020));
            brush.0 as isize
        }
    });
    HBRUSH(handle as *mut _)
}

unsafe fn apply_theme(hwnd: HWND, theme: AppTheme) {
    let is_dark = match theme {
        AppTheme::System => crate::gui::theme::ThemeManager::is_system_dark_mode(),
        AppTheme::Dark => true,
        AppTheme::Light => false,
    };
    
    let dark_mode: u32 = if is_dark { 1 } else { 0 };
    let _ = windows::Win32::Graphics::Dwm::DwmSetWindowAttribute(
        hwnd,
        windows::Win32::Graphics::Dwm::DWMWA_USE_IMMERSIVE_DARK_MODE,
        &dark_mode as *const u32 as *const _,
        4
    );
    
    // Force redraw
    windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, true);
}
