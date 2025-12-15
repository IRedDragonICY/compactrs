#![allow(unsafe_op_in_unsafe_fn)]
use windows::core::{Result, w, PCWSTR, PWSTR, PCSTR};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    HBRUSH, COLOR_WINDOW, InvalidateRect, CreateSolidBrush, FillRect, HDC,
    SetBkMode, SetTextColor, TRANSPARENT, CreateFontW, 
    DEFAULT_PITCH, FF_DONTCARE, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, 
    FW_NORMAL, DEFAULT_CHARSET,
};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE, DWM_SYSTEMBACKDROP_TYPE, DWMWINDOWATTRIBUTE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, PostQuitMessage, RegisterClassW, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, SW_SHOW, WM_DESTROY, WNDCLASSW,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE, WM_CREATE, WM_SIZE, WM_COMMAND, SetWindowPos, SWP_NOZORDER,
    GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA, GetDlgItem, WM_DROPFILES, MessageBoxW, MB_OK,
    SendMessageW, CB_ADDSTRING, CB_SETCURSEL, CB_GETCURSEL, SetWindowTextW, WS_CHILD, HMENU, WM_TIMER, SetTimer,
    MB_ICONINFORMATION, WM_NOTIFY, WM_SETFONT, BM_GETCHECK, WM_ERASEBKGND, GetClientRect,
};
use windows::Win32::UI::Shell::{DragQueryFileW, DragFinish, HDROP, FileOpenDialog, IFileOpenDialog, FOS_PICKFOLDERS, FOS_FORCEFILESYSTEM, SIGDN_FILESYSPATH, DragAcceptFiles, SetWindowSubclass, DefSubclassProc};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, CoTaskMemFree};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW, GetProcAddress};
use windows::Win32::System::Registry::{RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, HKEY};
use crate::gui::controls::{
    create_button_themed, create_listview, create_combobox, create_progress_bar, 
    apply_button_theme, apply_combobox_theme,
    IDC_COMBO_ALGO, IDC_STATIC_TEXT, IDC_PROGRESS_BAR, IDC_BTN_CANCEL, IDC_BATCH_LIST, IDC_BTN_ADD_FOLDER,
    IDC_BTN_REMOVE, IDC_BTN_PROCESS_ALL, IDC_BTN_ADD_FILES, IDC_BTN_SETTINGS, IDC_BTN_ABOUT,
    IDC_BTN_CONSOLE, IDC_CHK_FORCE, create_checkbox,
};
use crate::gui::settings::show_settings_modal;
use crate::gui::about::show_about_modal;
use crate::gui::console::{show_console_window, append_log_msg};
use crate::gui::state::{AppState, Controls, UiMessage, BatchAction, BatchStatus, AppTheme};
use crate::gui::taskbar::{TaskbarProgress, TaskbarState};
use std::thread;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use windows::Win32::UI::Controls::{
    PBM_SETRANGE32, PBM_SETPOS, LVM_INSERTCOLUMNW, LVM_INSERTITEMW, LVM_SETITEMW,
    LVM_DELETEITEM, LVM_GETNEXTITEM,
    LVM_SETBKCOLOR, LVM_SETTEXTCOLOR, LVM_SETTEXTBKCOLOR, SetWindowTheme,
    LVCOLUMNW, LVITEMW, LVCF_WIDTH, LVCF_TEXT, LVCF_FMT, LVCFMT_LEFT, LVIF_TEXT,
    LVNI_SELECTED, LVIF_PARAM, NM_DBLCLK, NMITEMACTIVATE,
    InitCommonControlsEx, INITCOMMONCONTROLSEX, ICC_WIN95_CLASSES, ICC_STANDARD_CLASSES, LVM_GETHEADER,
    NM_CUSTOMDRAW, NMCUSTOMDRAW, CDDS_PREPAINT, CDDS_ITEMPREPAINT, CDRF_NOTIFYITEMDRAW, CDRF_NEWFONT, NMHDR,
    LVN_ITEMCHANGED, BST_CHECKED,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use crate::engine::wof::WofAlgorithm;
use crate::engine::worker::{
    batch_process_worker, single_item_worker, 
    calculate_path_logical_size, calculate_path_disk_size, detect_path_algorithm
};
use humansize::{format_size, BINARY};



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

/// ListView subclass procedure to intercept Header's NM_CUSTOMDRAW notifications
/// Header sends NM_CUSTOMDRAW to its parent (ListView), not grandparent (main window)
unsafe extern "system" fn listview_subclass_proc(
    hwnd: HWND,
    umsg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    dwrefdata: usize,
) -> LRESULT {
    unsafe {
        let main_hwnd = HWND(dwrefdata as *mut _);
        if umsg == WM_NOTIFY && is_app_dark_mode(main_hwnd) {
            let nmhdr = &*(lparam.0 as *const NMHDR);
            
            if nmhdr.code == NM_CUSTOMDRAW {
                let nmcd = &mut *(lparam.0 as *mut NMCUSTOMDRAW);
                
                if nmcd.dwDrawStage == CDDS_PREPAINT {
                    // Request item-level notifications
                    return LRESULT(CDRF_NOTIFYITEMDRAW as isize);
                }
                
                if nmcd.dwDrawStage == CDDS_ITEMPREPAINT {
                    // Set text color to white for header items
                    SetTextColor(nmcd.hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF));
                    SetBkMode(nmcd.hdc, TRANSPARENT);
                    return LRESULT(CDRF_NEWFONT as isize);
                }
            }
        }
        
        // Call original window procedure
        DefSubclassProc(hwnd, umsg, wparam, lparam)
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
        let is_dark = is_system_dark_mode_preference();
        let bg_brush = if is_dark {
            // Dark background brush (same as get_dark_brush color 0x001E1E1E)
            unsafe { CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x001E1E1E)) }
        } else {
            HBRUSH((COLOR_WINDOW.0 + 1) as isize as *mut _)
        };

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

        let hwnd = CreateWindowExW(
            Default::default(),
            WINDOW_CLASS_NAME,
            WINDOW_TITLE,
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            900,
            600,
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
        let get_state = || {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr == 0 { None } else { Some(&mut *(ptr as *mut AppState)) }
        };

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
                
                // Create batch ListView - height 380, ends at y=420
                let h_listview = create_listview(hwnd, 10, 40, 860, 380, IDC_BATCH_LIST);
                setup_batch_listview_columns(h_listview);
                
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
                let is_dark_init = is_system_dark_mode_preference();
                
                // Use shared create_button_themed function (same as Console)
                let h_add_files = create_button_themed(hwnd, w!("Files"), 10, btn_y, 65, btn_h, IDC_BTN_ADD_FILES, is_dark_init);
                let h_add_folder = create_button_themed(hwnd, w!("Folder"), 85, btn_y, 65, btn_h, IDC_BTN_ADD_FOLDER, is_dark_init);
                let h_remove = create_button_themed(hwnd, w!("Remove"), 160, btn_y, 70, btn_h, IDC_BTN_REMOVE, is_dark_init);
                let h_combo = create_combobox(hwnd, 240, btn_y, 110, 200, IDC_COMBO_ALGO);
                // Force Checkbox
                let h_force = create_checkbox(hwnd, w!("Force"), 360, btn_y, 60, btn_h, IDC_CHK_FORCE);
                let h_process = create_button_themed(hwnd, w!("Process All"), 430, btn_y, 100, btn_h, IDC_BTN_PROCESS_ALL, is_dark_init);
                let h_cancel = create_button_themed(hwnd, w!("Cancel"), 540, btn_y, 80, btn_h, IDC_BTN_CANCEL, is_dark_init);
                
                
                // Settings/About items
                let h_settings = create_button_themed(hwnd, w!("\u{2699}"), 0, 0, 30, 25, IDC_BTN_SETTINGS, is_dark_init); // Gear icon
                let h_about = create_button_themed(hwnd, w!("?"), 0, 0, 30, 25, IDC_BTN_ABOUT, is_dark_init); // About icon
                // Console button (using a simple ">_" or similar text)
                let h_console = create_button_themed(hwnd, w!(">_"), 0, 0, 30, 25, IDC_BTN_CONSOLE, is_dark_init);

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
                SendMessageW(h_combo, CB_SETCURSEL, Some(WPARAM(1)), Some(LPARAM(0))); // Default XPRESS8K

                state.controls = Some(Controls {
                    list_view: h_listview,
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
                
                // Apply theme (dark mode support for ListView)
                update_theme(hwnd);

                LRESULT(0)
            }
            
            // WM_ERASEBKGND - Paint dark background when in dark mode
            WM_ERASEBKGND => {
                if is_app_dark_mode(hwnd) {
                    let hdc = HDC(wparam.0 as *mut _);
                    let mut rc = windows::Win32::Foundation::RECT::default();
                    GetClientRect(hwnd, &mut rc);
                    
                    let brush = get_dark_brush();
                    FillRect(hdc, &rc, brush);
                    return LRESULT(1);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            
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
                                            add_listview_item(ctrls.list_view, item_id, &file_path, "XPRESS8K", "Compress", &logical_str, &disk_str, detected_algo);
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
                                        add_listview_item(ctrls.list_view, item_id, &folder, "XPRESS8K", "Compress", &logical_str, &disk_str, detected_algo);
                                    }
                                }
                            }
                        }
                    },
                    
                    IDC_BTN_REMOVE => {
                        if let Some(st) = get_state() {
                            // First get the list_view handle
                            let list_view = st.controls.as_ref().map(|c| c.list_view);
                            if let Some(lv) = list_view {
                                // Collect all selected indices first
                                let mut selected_indices = Vec::new();
                                let mut item_idx = -1;
                                loop {
                                    let start_param = if item_idx < 0 { usize::MAX } else { item_idx as usize };
                                    let next = SendMessageW(lv, LVM_GETNEXTITEM, Some(WPARAM(start_param)), Some(LPARAM(LVNI_SELECTED as isize)));
                                    if next.0 < 0 { break; }
                                    item_idx = next.0 as i32;
                                    selected_indices.push(item_idx as usize);
                                }
                                
                                // Sort descending to remove from end first (preserves indices)
                                selected_indices.sort_by(|a, b| b.cmp(a));
                                
                                for idx in selected_indices {
                                    // Remove from State (by ID)
                                    // Use the index directly since batch_items maps 1:1 to ListView rows
                                    if let Some(item) = st.batch_items.get(idx) {
                                        let id = item.id;
                                        st.remove_batch_item(id);
                                    }
                                    
                                    // Remove from ListView
                                    SendMessageW(lv, LVM_DELETEITEM, Some(WPARAM(idx)), None);
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
                                    // Collect indices to process
                                    let mut indices_to_process = Vec::new();
                                    
                                    // Check for selection
                                    let mut item_idx = -1;
                                    loop {
                                        let start_param = if item_idx < 0 { usize::MAX } else { item_idx as usize };
                                        let next = SendMessageW(ctrls.list_view, LVM_GETNEXTITEM, Some(WPARAM(start_param)), Some(LPARAM(LVNI_SELECTED as isize)));
                                        if next.0 < 0 { break; }
                                        item_idx = next.0 as i32;
                                        indices_to_process.push(item_idx as usize);
                                    }
                                    
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
                                    
                                    // Update Algorithm column for processed items
                                    for &row in &indices_to_process {
                                        update_listview_item(ctrls.list_view, row as i32, 1, algo_name);
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

                                    thread::spawn(move || {
                                        batch_process_worker(items, algo, tx, cancel, force);
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
                            if let Some(new_theme) = show_settings_modal(hwnd, theme, is_dark) {
                                st.theme = new_theme;
                                // Apply immediately
                                apply_theme(hwnd, new_theme);
                            }
                        }
                    },
                    
                    IDC_BTN_ABOUT => {
                        let is_dark = is_app_dark_mode(hwnd);
                        show_about_modal(hwnd, is_dark);
                    },

                    IDC_BTN_CONSOLE => {
                        if let Some(app_state) = (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState).as_mut() {
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
                                            update_listview_item(ctrls.list_view, row, 5, &progress);
                                            update_listview_item(ctrls.list_view, row, 6, &status);
                                        }
                                    },
                                    UiMessage::ItemFinished(row, status, disk_size_str) => {
                                        // Update Status (col 6) and On Disk column (col 4) with compressed size
                                        if let Some(ctrls) = &st.controls {
                                            update_listview_item(ctrls.list_view, row, 6, &status);
                                            // Update On Disk column with the new compressed size
                                            if !disk_size_str.is_empty() {
                                                update_listview_item(ctrls.list_view, row, 4, &disk_size_str);
                                            }
                                            // Reset button to "Start"
                                            update_listview_item(ctrls.list_view, row, 7, "▶ Start");
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
                                                // Update ListView columns:
                                                // 1: Algorithm (if detected)
                                                // 3: Size
                                                // 4: On Disk
                                                update_listview_item(ctrls.list_view, pos as i32, 3, &logical_str);
                                                update_listview_item(ctrls.list_view, pos as i32, 4, &disk_str);
                                                
                                                if let Some(a) = algo {
                                                    let algo_str = match a {
                                                        WofAlgorithm::Xpress4K => "XPRESS4K",
                                                        WofAlgorithm::Xpress8K => "XPRESS8K",
                                                        WofAlgorithm::Xpress16K => "XPRESS16K",
                                                        WofAlgorithm::Lzx => "LZX",
                                                    };
                                                    update_listview_item(ctrls.list_view, pos as i32, 1, algo_str);
                                                }
                                                // Reset status to Pending (from Calculating...)
                                                update_listview_item(ctrls.list_view, pos as i32, 6, "Pending");

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
                     let mut items_to_analyze = Vec::new();
                     for path in paths {
                         // Check duplicates (simple O(N) check is fine for drag-drop)
                         if !st.batch_items.iter().any(|item| item.path == path) {
                             let id = st.add_batch_item(path.clone());
                             if let Some(ctrls) = &st.controls {
                                 add_listview_item(ctrls.list_view, id, &path, "XPRESS8K", "Compress", "Calculating...", "Calculating...", None);
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
                                            update_listview_item(ctrls.list_view, row, 1, algo_str);
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
                                            update_listview_item(ctrls.list_view, row, 2, action_str);
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
                                                update_listview_item(ctrls.list_view, row, 7, "Stopping...");
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
                                            
                                            // Update status and Button
                                            if let Some(ctrls) = &st.controls {
                                                update_listview_item(ctrls.list_view, row, 6, "Running");
                                                update_listview_item(ctrls.list_view, row, 7, "■ Stop");
                                            }
                                            
                                            let force = st.force_compress; // Capture force flag
                                            
                                            let row_for_thread = row;
                                            thread::spawn(move || {
                                                single_item_worker(path, algo, action, row_for_thread, tx, token, force);
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    if nmhdr.code == LVN_ITEMCHANGED {
                         // Check selection count
                         if let Some(st) = get_state() {
                              if let Some(ctrls) = &st.controls {
                                  // Count selected items
                                  let mut count = 0;
                                  let mut item_idx = -1;
                                  loop {
                                      let start_param = if item_idx < 0 { usize::MAX } else { item_idx as usize };
                                      let next = SendMessageW(ctrls.list_view, LVM_GETNEXTITEM, Some(WPARAM(start_param)), Some(LPARAM(LVNI_SELECTED as isize)));
                                      if next.0 < 0 { break; }
                                      item_idx = next.0 as i32;
                                      count += 1;
                                  }
                                  
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
            
            // WM_CTLCOLORSTATIC - handle static text colors in dark mode
            0x0138 => { // WM_CTLCOLORSTATIC
                if is_app_dark_mode(hwnd) {
                    let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut _);
                    SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF)); // White text
                    SetBkMode(hdc, TRANSPARENT);
                    return LRESULT(get_dark_brush().0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            
            // WM_ERASEBKGND - paint dark background
            0x0014 => { // WM_ERASEBKGND
                if is_app_dark_mode(hwnd) {
                    let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut _);
                    let mut rect = windows::Win32::Foundation::RECT::default();
                    if windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect).is_ok() {
                        let brush = get_dark_brush();
                        windows::Win32::Graphics::Gdi::FillRect(hdc, &rect, brush);
                        return LRESULT(1);
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe fn setup_batch_listview_columns(hwnd: HWND) {
    // Columns: Path | Algo | Action | Size | On Disk | Progress | Status | ▶ Start
    let columns = [
        (w!("Path"), 250),
        (w!("Algorithm"), 70),
        (w!("Action"), 70),
        (w!("Size"), 75),
        (w!("On Disk"), 75),
        (w!("Progress"), 70),
        (w!("Status"), 80),
        (w!("▶ Start"), 45),
    ];
    
    for (i, (name, width)) in columns.iter().enumerate() {
        let col = LVCOLUMNW {
            mask: LVCF_WIDTH | LVCF_TEXT | LVCF_FMT,
            fmt: LVCFMT_LEFT,
            cx: *width,
            pszText: PWSTR(name.as_ptr() as *mut _),
            ..Default::default()
        };
        SendMessageW(hwnd, LVM_INSERTCOLUMNW, Some(WPARAM(i)), Some(LPARAM(&col as *const _ as isize)));
    }
}

unsafe fn add_listview_item(hwnd: HWND, id: u32, path: &str, algorithm: &str, action: &str, size_logical: &str, size_disk: &str, detected_algo: Option<WofAlgorithm>) {
    // Columns: 0=Path | 1=Algo | 2=Action | 3=Size | 4=OnDisk | 5=Progress | 6=Status | 7=Start
    let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let algo_wide: Vec<u16> = algorithm.encode_utf16().chain(std::iter::once(0)).collect();
    let action_wide: Vec<u16> = action.encode_utf16().chain(std::iter::once(0)).collect();
    let size_wide: Vec<u16> = size_logical.encode_utf16().chain(std::iter::once(0)).collect();
    let disk_wide: Vec<u16> = size_disk.encode_utf16().chain(std::iter::once(0)).collect();
    
    // Show compression status with algorithm name if already compressed
    let status_text = match detected_algo {
        Some(WofAlgorithm::Xpress4K) => "XPRESS4K ✓".to_string(),
        Some(WofAlgorithm::Xpress8K) => "XPRESS8K ✓".to_string(),
        Some(WofAlgorithm::Xpress16K) => "XPRESS16K ✓".to_string(),
        Some(WofAlgorithm::Lzx) => "LZX ✓".to_string(),
        None => "Pending".to_string(),
    };
    let status_wide: Vec<u16> = status_text.encode_utf16().chain(std::iter::once(0)).collect();
    let start_wide: Vec<u16> = "▶".encode_utf16().chain(std::iter::once(0)).collect();
    
    // Insert main item (path column)
    let mut item = LVITEMW {
        mask: LVIF_TEXT | LVIF_PARAM,
        iItem: i32::MAX, // Append at end
        iSubItem: 0,
        pszText: PWSTR(path_wide.as_ptr() as *mut _),
        lParam: LPARAM(id as isize),
        ..Default::default()
    };
    let idx = SendMessageW(hwnd, LVM_INSERTITEMW, Some(WPARAM(0)), Some(LPARAM(&item as *const _ as isize)));
    let row = idx.0 as i32;
    
    // Set subitems
    item.mask = LVIF_TEXT;
    item.iItem = row;
    
    // Col 1 = Algorithm
    item.iSubItem = 1;
    item.pszText = PWSTR(algo_wide.as_ptr() as *mut _);
    SendMessageW(hwnd, LVM_SETITEMW, Some(WPARAM(0)), Some(LPARAM(&item as *const _ as isize)));
    
    // Col 2 = Action
    item.iSubItem = 2;
    item.pszText = PWSTR(action_wide.as_ptr() as *mut _);
    SendMessageW(hwnd, LVM_SETITEMW, Some(WPARAM(0)), Some(LPARAM(&item as *const _ as isize)));
    
    // Col 3 = Size (logical/uncompressed)
    item.iSubItem = 3;
    item.pszText = PWSTR(size_wide.as_ptr() as *mut _);
    SendMessageW(hwnd, LVM_SETITEMW, Some(WPARAM(0)), Some(LPARAM(&item as *const _ as isize)));
    
    // Col 4 = On Disk (compressed size)
    item.iSubItem = 4;
    item.pszText = PWSTR(disk_wide.as_ptr() as *mut _);
    SendMessageW(hwnd, LVM_SETITEMW, Some(WPARAM(0)), Some(LPARAM(&item as *const _ as isize)));
    
    // Col 5 = Progress (empty initially)
    // Left empty
    
    // Col 6 = Status (shows WOF ✓ if already compressed)
    item.iSubItem = 6;
    item.pszText = PWSTR(status_wide.as_ptr() as *mut _);
    SendMessageW(hwnd, LVM_SETITEMW, Some(WPARAM(0)), Some(LPARAM(&item as *const _ as isize)));
    
    // Col 7 = Start button
    item.iSubItem = 7;
    item.pszText = PWSTR(start_wide.as_ptr() as *mut _);
    SendMessageW(hwnd, LVM_SETITEMW, Some(WPARAM(0)), Some(LPARAM(&item as *const _ as isize)));
}

/// Update a specific cell in the ListView
unsafe fn update_listview_item(hwnd: HWND, row: i32, col: i32, text: &str) {
    let text_wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    
    let item = LVITEMW {
        mask: LVIF_TEXT,
        iItem: row,
        iSubItem: col,
        pszText: PWSTR(text_wide.as_ptr() as *mut _),
        ..Default::default()
    };
    SendMessageW(hwnd, LVM_SETITEMW, Some(WPARAM(0)), Some(LPARAM(&item as *const _ as isize)));
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
        let true_val: i32 = 1;
        let false_val: i32 = 0;
        
        // 2. Force Dark Mode Frame (Titlebar & Borders)
        // DWMWA_USE_IMMERSIVE_DARK_MODE = 20
        let dwm_dark_mode = DWMWINDOWATTRIBUTE(20); 
        if is_dark {
            let _ = DwmSetWindowAttribute(hwnd, dwm_dark_mode, &true_val as *const _ as _, 4);
        } else {
             let _ = DwmSetWindowAttribute(hwnd, dwm_dark_mode, &false_val as *const _ as _, 4);
        }

        // 3. Apply Mica Backdrop (Windows 11 Standard)
        // This matches Task Manager and Explorer design
        let system_backdrop_type = DWMWA_SYSTEMBACKDROP_TYPE;
        let mica = DWM_SYSTEMBACKDROP_TYPE(2); // 2 = Mica
        let _ = DwmSetWindowAttribute(hwnd, system_backdrop_type, &mica as *const _ as _, 4);
        
        // Note: We DO NOT call DwmExtendFrameIntoClientArea for Mica. 
        // We want the system to draw the Mica background, not transparency.
    }
}

unsafe fn is_system_dark_mode_preference() -> bool {
    unsafe {
        let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        let val_name = w!("AppsUseLightTheme");
        let mut hkey: HKEY = Default::default();
        
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, Some(0), KEY_READ, &mut hkey).is_ok() {
            let mut data: u32 = 0;
            let mut cb_data = std::mem::size_of::<u32>() as u32;
            let result = RegQueryValueExW(hkey, val_name, None, None, Some(&mut data as *mut _ as _), Some(&mut cb_data));
            let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
            
            if result.is_ok() {
                return data == 0;
            }
        }
        false
    }
}

// Check effective dark mode state (System or Override)
unsafe fn is_app_dark_mode(hwnd: HWND) -> bool {
    // Try to get AppState to check override
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if ptr != 0 {
        let st = &*(ptr as *const AppState);
        match st.theme {
            AppTheme::Dark => return true,
            AppTheme::Light => return false,
            AppTheme::System => return is_system_dark_mode_preference(),
        }
    }
    // Fallback if no state yet (e.g. during creation)
    is_system_dark_mode_preference()
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
        
        // Update ListView with dark mode explorer theme and colors
        if let Ok(list_view) = GetDlgItem(Some(hwnd), IDC_BATCH_LIST as i32) {
            if !list_view.is_invalid() {
                allow_dark_mode_for_window(list_view, dark);
                // Apply dark mode explorer theme (affects header, scrollbars, etc.)
                if dark {
                    let _ = SetWindowTheme(list_view, w!("DarkMode_ItemsView"), None);
                } else {
                    let _ = SetWindowTheme(list_view, w!("Explorer"), None);
                }
                
                if dark {
                    // Dark mode colors: dark background, light text
                    let bg_color: u32 = 0x00202020;   // Dark gray background (BGR format)
                    let text_color: u32 = 0x00FFFFFF; // White text
                    SendMessageW(list_view, LVM_SETBKCOLOR, Some(WPARAM(0)), Some(LPARAM(bg_color as isize)));
                    SendMessageW(list_view, LVM_SETTEXTBKCOLOR, Some(WPARAM(0)), Some(LPARAM(bg_color as isize)));
                    SendMessageW(list_view, LVM_SETTEXTCOLOR, Some(WPARAM(0)), Some(LPARAM(text_color as isize)));
                } else {
                    // Light mode colors: white background, black text
                    let bg_color: u32 = 0x00FFFFFF;   // White background
                    let text_color: u32 = 0x00000000; // Black text
                    SendMessageW(list_view, LVM_SETBKCOLOR, Some(WPARAM(0)), Some(LPARAM(bg_color as isize)));
                    SendMessageW(list_view, LVM_SETTEXTBKCOLOR, Some(WPARAM(0)), Some(LPARAM(bg_color as isize)));
                    SendMessageW(list_view, LVM_SETTEXTCOLOR, Some(WPARAM(0)), Some(LPARAM(text_color as isize)));
                }
                // Force redraw
                let _ = InvalidateRect(Some(list_view), None, true);
            }
        }
        
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
        
        // Update ListView Font and Header Theme
        if let Ok(list_view) = GetDlgItem(Some(hwnd), IDC_BATCH_LIST as i32) {
             SendMessageW(list_view, WM_SETFONT, Some(WPARAM(hfont.0 as usize)), Some(LPARAM(1)));
             allow_dark_mode_for_window(list_view, dark);
             
             // Subclass ListView to intercept Header's NM_CUSTOMDRAW notifications
             let _ = SetWindowSubclass(
                 list_view,
                 Some(listview_subclass_proc),
                 1, // Subclass ID
                 hwnd.0 as usize, // Pass Main Window HWND as RefData
             );
             
             // Get Header Control and skin it
             let header_lresult = SendMessageW(list_view, LVM_GETHEADER, None, None);
             let header = HWND(header_lresult.0 as *mut _);
             
             if !header.is_invalid() {
                 allow_dark_mode_for_window(header, dark);
                 if dark {
                     // DarkMode_ItemsView for dark background (subclass handles white text)
                     let _ = SetWindowTheme(header, w!("DarkMode_ItemsView"), None);
                 } else {
                     let _ = SetWindowTheme(header, w!("Explorer"), None);
                 }
             }
             
             // Re-apply listview colors
             if dark {
                let bg_color: u32 = 0x00202020;
                let text_color: u32 = 0x00FFFFFF;
                SendMessageW(list_view, LVM_SETBKCOLOR, Some(WPARAM(0)), Some(LPARAM(bg_color as isize)));
                SendMessageW(list_view, LVM_SETTEXTBKCOLOR, Some(WPARAM(0)), Some(LPARAM(bg_color as isize)));
                SendMessageW(list_view, LVM_SETTEXTCOLOR, Some(WPARAM(0)), Some(LPARAM(text_color as isize)));
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
        AppTheme::System => is_system_dark_mode_preference(),
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
