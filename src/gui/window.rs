use windows::core::{Result, w, PCWSTR, PWSTR};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{HBRUSH, COLOR_WINDOW};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE, DWM_SYSTEMBACKDROP_TYPE, DWMWINDOWATTRIBUTE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, PostQuitMessage, RegisterClassW, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, SW_SHOW, WM_DESTROY, WNDCLASSW,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE, WM_CREATE, WM_SIZE, WM_COMMAND, SetWindowPos, SWP_NOZORDER,
    GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA, GetDlgItem, WM_DROPFILES, MessageBoxW, MB_OK,
    SendMessageW, CB_ADDSTRING, CB_SETCURSEL, CB_GETCURSEL, SetWindowTextW, WS_CHILD, HMENU, WM_TIMER, SetTimer,
    MB_ICONINFORMATION, WM_NOTIFY,
};
use windows::Win32::UI::Shell::{DragQueryFileW, DragFinish, HDROP, FileOpenDialog, IFileOpenDialog, FOS_PICKFOLDERS, FOS_FORCEFILESYSTEM, SIGDN_FILESYSPATH, DragAcceptFiles};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, CoTaskMemFree};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, HKEY};
use crate::gui::controls::{
    create_button, create_listview, create_combobox, create_progress_bar, 
    IDC_LISTVIEW, IDC_BTN_SCAN, IDC_BTN_COMPRESS, IDC_COMBO_ALGO, IDC_BTN_DECOMPRESS, 
    IDC_STATIC_TEXT, IDC_PROGRESS_BAR, IDC_BTN_CANCEL, IDC_BATCH_LIST, IDC_BTN_ADD_FOLDER,
    IDC_BTN_REMOVE, IDC_BTN_PROCESS_ALL,
};
use crate::gui::state::{AppState, Controls, UiMessage, BatchAction};
use std::thread;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use crossbeam_channel::Sender;
use windows::Win32::UI::Controls::{
    PBM_SETRANGE32, PBM_SETPOS, LVM_INSERTCOLUMNW, LVM_INSERTITEMW, LVM_SETITEMW,
    LVM_DELETEITEM, LVM_DELETEALLITEMS, LVM_GETSELECTEDCOUNT, LVM_GETNEXTITEM,
    LVCOLUMNW, LVITEMW, LVCF_WIDTH, LVCF_TEXT, LVCF_FMT, LVCFMT_LEFT, LVIF_TEXT,
    LVNI_SELECTED, LVIF_PARAM, NM_DBLCLK, NMITEMACTIVATE,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use crate::engine::wof::{compress_file, uncompress_file, WofAlgorithm, get_real_file_size, is_wof_compressed, get_wof_algorithm};
use crate::engine::compresstimate::estimate_size;
use ignore::WalkBuilder;
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
const WINDOW_TITLE: PCWSTR = w!("CompactRS - Batch Compressor");

/// Calculate total LOGICAL size of all files in a folder (uncompressed content size)
/// This counts ALL files including hidden and .gitignored files
fn calculate_folder_logical_size(path: &str) -> u64 {
    WalkBuilder::new(path)
        .hidden(false)          // Include hidden files
        .git_ignore(false)      // Don't respect .gitignore
        .git_global(false)      // Don't respect global gitignore
        .git_exclude(false)     // Don't respect .git/info/exclude
        .ignore(false)          // Don't respect .ignore files
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .filter_map(|e| std::fs::metadata(e.path()).ok())
        .map(|m| m.len())
        .sum()
}

/// Calculate total DISK size of all files in a folder (actual space used, respects compression)
/// Uses GetCompressedFileSizeW to get real disk usage for WOF-compressed files
fn calculate_folder_disk_size(path: &str) -> u64 {
    WalkBuilder::new(path)
        .hidden(false)          // Include hidden files
        .git_ignore(false)      // Don't respect .gitignore
        .git_global(false)      // Don't respect global gitignore
        .git_exclude(false)     // Don't respect .git/info/exclude
        .ignore(false)          // Don't respect .ignore files
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .map(|e| get_real_file_size(&e.path().to_string_lossy()))
        .sum()
}

/// Check if a folder has any WOF-compressed files
/// Returns true if disk size < logical size (meaning compression is active)
fn is_folder_compressed(logical_size: u64, disk_size: u64) -> bool {
    // If disk size is noticeably smaller, folder has compressed files
    // Use a small threshold to account for rounding
    disk_size < logical_size && (logical_size - disk_size) > 1024
}

/// Detect the predominant WOF algorithm used in a folder
/// Samples up to 10 files to determine the algorithm
fn detect_folder_algorithm(path: &str) -> Option<WofAlgorithm> {
    let mut algo_counts = [0u32; 4]; // Xpress4K, Lzx, Xpress8K, Xpress16K
    let mut sampled = 0;
    
    for result in WalkBuilder::new(path)
        .hidden(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .ignore(false)
        .build()
    {
        if sampled >= 20 { break; } // Sample enough files
        
        if let Ok(entry) = result {
            if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                let file_path = entry.path().to_string_lossy().to_string();
                if let Some(algo) = get_wof_algorithm(&file_path) {
                    algo_counts[algo as usize] += 1;
                    sampled += 1;
                }
            }
        }
    }
    
    // Find the most common algorithm
    let max_idx = algo_counts.iter().enumerate().max_by_key(|(_, v)| *v).map(|(i, _)| i);
    
    if let Some(idx) = max_idx {
        if algo_counts[idx] > 0 {
            return match idx {
                0 => Some(WofAlgorithm::Xpress4K),
                1 => Some(WofAlgorithm::Lzx),
                2 => Some(WofAlgorithm::Xpress8K),
                3 => Some(WofAlgorithm::Xpress16K),
                _ => None,
            };
        }
    }
    None
}

pub unsafe fn create_main_window(instance: HINSTANCE) -> Result<HWND> {
    unsafe {
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize as *mut _),
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
                
                // Create header label
                let h_label = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    w!("Drag and drop folders here, or use 'Add Folder' button. Then click 'Process All' to start."),
                    WS_CHILD | WS_VISIBLE,
                    10, 10, 860, 25,
                    Some(hwnd),
                    Some(HMENU(IDC_STATIC_TEXT as isize as *mut _)),
                    Some(HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0)),
                    None,
                ).unwrap_or_default();
                
                // Create batch ListView
                let h_listview = create_listview(hwnd, 10, 40, 860, 420, IDC_BATCH_LIST);
                setup_batch_listview_columns(h_listview);
                
                // Create progress bar
                let h_progress = create_progress_bar(hwnd, 10, 470, 860, 25, IDC_PROGRESS_BAR);
                
                // Create buttons at bottom
                let btn_y = 505;
                let h_add = create_button(hwnd, w!("Add Folder"), 10, btn_y, 100, 30, IDC_BTN_ADD_FOLDER);
                let h_remove = create_button(hwnd, w!("Remove"), 120, btn_y, 80, 30, IDC_BTN_REMOVE);
                let h_combo = create_combobox(hwnd, 210, btn_y, 120, 200, IDC_COMBO_ALGO);
                let h_process = create_button(hwnd, w!("Process All"), 340, btn_y, 100, 30, IDC_BTN_PROCESS_ALL);
                let h_cancel = create_button(hwnd, w!("Cancel"), 450, btn_y, 80, 30, IDC_BTN_CANCEL);
                EnableWindow(h_cancel, false);

                // Populate algorithm combo
                let algos = [w!("XPRESS4K"), w!("XPRESS8K"), w!("XPRESS16K"), w!("LZX")];
                for alg in algos {
                    SendMessageW(h_combo, CB_ADDSTRING, Some(WPARAM(0)), Some(LPARAM(alg.as_ptr() as isize)));
                }
                SendMessageW(h_combo, CB_SETCURSEL, Some(WPARAM(1)), Some(LPARAM(0))); // Default XPRESS8K

                state.controls = Some(Controls {
                    list_view: h_listview,
                    btn_scan: h_add,  // Reusing for Add Folder
                    btn_compress: h_process,  // Reusing for Process All
                    btn_decompress: h_remove,  // Reusing for Remove
                    combo_algo: h_combo,
                    static_text: h_label,
                    progress_bar: h_progress,
                    btn_cancel: h_cancel,
                });

                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
                SetTimer(Some(hwnd), 1, 100, None);
                DragAcceptFiles(hwnd, true);

                LRESULT(0)
            }
            
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as u16;

                match id {
                    IDC_BTN_ADD_FOLDER => {
                        if let Ok(folder) = pick_folder() {
                            if let Some(st) = get_state() {
                                let item_id = st.add_batch_item(folder.clone());
                                let logical_size = calculate_folder_logical_size(&folder);
                                let disk_size = calculate_folder_disk_size(&folder);
                                let detected_algo = detect_folder_algorithm(&folder);
                                let logical_str = format_size(logical_size, BINARY);
                                let disk_str = format_size(disk_size, BINARY);
                                if let Some(ctrls) = &st.controls {
                                    add_listview_item(ctrls.list_view, item_id, &folder, "XPRESS8K", "Compress", &logical_str, &disk_str, detected_algo);
                                }
                            }
                        }
                    },
                    
                    IDC_BTN_REMOVE => {
                        if let Some(st) = get_state() {
                            // First get the list_view handle and selected index
                            let list_view = st.controls.as_ref().map(|c| c.list_view);
                            if let Some(lv) = list_view {
                                let sel_idx = SendMessageW(lv, LVM_GETNEXTITEM, Some(WPARAM(usize::MAX)), Some(LPARAM(LVNI_SELECTED as isize)));
                                if sel_idx.0 >= 0 {
                                    let idx = sel_idx.0 as usize;
                                    // Get item ID before removing
                                    let item_id = st.batch_items.get(idx).map(|i| i.id);
                                    if let Some(id) = item_id {
                                        st.remove_batch_item(id);
                                    }
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
                                    // Get selected algorithm
                                    let idx = SendMessageW(ctrls.combo_algo, CB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));
                                    let algo = match idx.0 {
                                        0 => WofAlgorithm::Xpress4K,
                                        2 => WofAlgorithm::Xpress16K,
                                        3 => WofAlgorithm::Lzx,
                                        _ => WofAlgorithm::Xpress8K,
                                    };
                                    
                                    EnableWindow(ctrls.btn_cancel, true);
                                    SetWindowTextW(ctrls.static_text, w!("Processing batch..."));
                                    
                                    let tx = st.tx.clone();
                                    let cancel = st.cancel_flag.clone();
                                    cancel.store(false, Ordering::Relaxed);
                                    
                                    // Clone items for worker thread
                                    let items: Vec<_> = st.batch_items.iter().map(|i| (i.path.clone(), i.action)).collect();
                                    
                                    thread::spawn(move || {
                                        batch_process_worker(items, algo, tx, cancel);
                                    });
                                }
                            }
                        }
                    },
                    
                    IDC_BTN_CANCEL => {
                        if let Some(st) = get_state() {
                            st.cancel_flag.store(true, Ordering::Relaxed);
                            if let Some(ctrls) = &st.controls {
                                EnableWindow(ctrls.btn_cancel, false);
                                SetWindowTextW(ctrls.static_text, w!("Cancelling..."));
                            }
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
                                    },
                                    UiMessage::Status(text) | UiMessage::Log(text) | UiMessage::Error(text) => {
                                        if let Some(ctrls) = &st.controls {
                                            let wstr = windows::core::HSTRING::from(&text);
                                            SetWindowTextW(ctrls.static_text, PCWSTR::from_raw(wstr.as_ptr()));
                                        }
                                    },
                                    UiMessage::Finished => {
                                        if let Some(ctrls) = &st.controls {
                                            EnableWindow(ctrls.btn_cancel, false);
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
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_STATIC_TEXT.into()) {
                    SetWindowPos(h, None, padding, padding, width - padding * 2, header_height, SWP_NOZORDER);
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
                
                // Position buttons at bottom
                let btn_y = height - btn_height - padding;
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BTN_ADD_FOLDER.into()) {
                    SetWindowPos(h, None, padding, btn_y, 100, btn_height, SWP_NOZORDER);
                }
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BTN_REMOVE.into()) {
                    SetWindowPos(h, None, padding + 110, btn_y, 80, btn_height, SWP_NOZORDER);
                }
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_COMBO_ALGO.into()) {
                    SetWindowPos(h, None, padding + 200, btn_y, 120, btn_height, SWP_NOZORDER);
                }
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BTN_PROCESS_ALL.into()) {
                    SetWindowPos(h, None, padding + 330, btn_y, 100, btn_height, SWP_NOZORDER);
                }
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BTN_CANCEL.into()) {
                    SetWindowPos(h, None, padding + 440, btn_y, 80, btn_height, SWP_NOZORDER);
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
                
                for i in 0..count {
                    let len = DragQueryFileW(hdrop, i, Some(&mut buffer));
                    if len > 0 {
                        let path_string = String::from_utf16_lossy(&buffer[..len as usize]);
                        let path = std::path::Path::new(&path_string);
                        
                        let target_folder = if path.is_dir() {
                            Some(path_string.clone())
                        } else {
                            path.parent().map(|p| p.to_string_lossy().to_string())
                        };

                        if let Some(folder) = target_folder {
                            if let Some(st) = get_state() {
                                // Check if already added
                                let already_exists = st.batch_items.iter().any(|item| item.path == folder);
                                if !already_exists {
                                    let item_id = st.add_batch_item(folder.clone());
                                    let logical_size = calculate_folder_logical_size(&folder);
                                    let disk_size = calculate_folder_disk_size(&folder);
                                    let detected_algo = detect_folder_algorithm(&folder);
                                    let logical_str = format_size(logical_size, BINARY);
                                    let disk_str = format_size(disk_size, BINARY);
                                    if let Some(ctrls) = &st.controls {
                                        add_listview_item(ctrls.list_view, item_id, &folder, "XPRESS8K", "Compress", &logical_str, &disk_str, detected_algo);
                                    }
                                }
                            }
                        }
                    }
                }
                
                if let Some(st) = get_state() {
                    if let Some(ctrls) = &st.controls {
                        let count = st.batch_items.len();
                        let msg = format!("{} folder(s) in batch queue. Select algorithm and click 'Process All'.", count);
                        let wstr = windows::core::HSTRING::from(&msg);
                        SetWindowTextW(ctrls.static_text, PCWSTR::from_raw(wstr.as_ptr()));
                    }
                }
                
                DragFinish(hdrop);
                LRESULT(0)
            }

            0x001A => { // WM_SETTINGCHANGE
                update_theme(hwnd);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            
            WM_NOTIFY => {
                let nmhdr = &*(lparam.0 as *const windows::Win32::UI::Controls::NMHDR);
                
                // Check if it's from our ListView and is a double-click
                if nmhdr.idFrom == IDC_BATCH_LIST as usize && nmhdr.code == NM_DBLCLK {
                    let nmia = &*(lparam.0 as *const NMITEMACTIVATE);
                    let row = nmia.iItem;
                    let col = nmia.iSubItem;
                    
                    if row >= 0 {
                        if let Some(st) = get_state() {
                            let row_idx = row as usize;
                            
                            // Column 1 = Algorithm, Column 2 = Action
                            if col == 1 {
                                // Cycle Algorithm: XPRESS4K -> XPRESS8K -> XPRESS16K -> LZX -> XPRESS4K
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
                                    // Update ListView
                                    if let Some(ctrls) = &st.controls {
                                        update_listview_item(ctrls.list_view, row, 1, algo_str);
                                    }
                                }
                            } else if col == 2 {
                                // Toggle Action: Compress <-> Decompress
                                if let Some(item) = st.batch_items.get_mut(row_idx) {
                                    item.action = match item.action {
                                        BatchAction::Compress => BatchAction::Decompress,
                                        BatchAction::Decompress => BatchAction::Compress,
                                    };
                                    let action_str = match item.action {
                                        BatchAction::Compress => "Compress",
                                        BatchAction::Decompress => "Decompress",
                                    };
                                    // Update ListView
                                    if let Some(ctrls) = &st.controls {
                                        update_listview_item(ctrls.list_view, row, 2, action_str);
                                    }
                                }
                            } else if col == 7 {
                                // Start button clicked - process this single item
                                if let Some(item) = st.batch_items.get(row_idx) {
                                    let path = item.path.clone();
                                    let algo = item.algorithm;
                                    let action = item.action;
                                    let tx = st.tx.clone();
                                    let cancel = st.cancel_flag.clone();
                                    cancel.store(false, Ordering::Relaxed);
                                    
                                    // Update status to Processing (col 6)
                                    if let Some(ctrls) = &st.controls {
                                        update_listview_item(ctrls.list_view, row, 6, "Running");
                                        EnableWindow(ctrls.btn_cancel, true);
                                    }
                                    
                                    let row_for_thread = row;
                                    thread::spawn(move || {
                                        single_item_worker(path, algo, action, row_for_thread, tx, cancel);
                                    });
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

fn batch_process_worker(items: Vec<(String, BatchAction)>, algo: WofAlgorithm, tx: Sender<UiMessage>, cancel: Arc<AtomicBool>) {
    let mut total_files = 0u64;
    let mut processed = 0u64;
    let mut success = 0u64;
    let mut failed = 0u64;
    
    // Count total files first
    for (path, _) in &items {
        for result in WalkBuilder::new(path).build() {
            if let Ok(entry) = result {
                if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    total_files += 1;
                }
            }
        }
    }
    
    let _ = tx.send(UiMessage::Progress(0, total_files));
    let _ = tx.send(UiMessage::Status(format!("Processing {} files across {} folders...", total_files, items.len())));
    
    for (folder_idx, (path, action)) in items.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            let _ = tx.send(UiMessage::Status("Batch processing cancelled.".to_string()));
            let _ = tx.send(UiMessage::Finished);
            return;
        }
        
        let _ = tx.send(UiMessage::Status(format!("Processing folder {}/{}: {}", folder_idx + 1, items.len(), path)));
        
        for result in WalkBuilder::new(path).build() {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            
            if let Ok(entry) = result {
                if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    let file_path = entry.path().to_string_lossy().to_string();
                    
                    let result = match action {
                        BatchAction::Compress => compress_file(&file_path, algo).is_ok(),
                        BatchAction::Decompress => uncompress_file(&file_path).is_ok(),
                    };
                    
                    if result {
                        success += 1;
                    } else {
                        failed += 1;
                    }
                    
                    processed += 1;
                    
                    if processed % 20 == 0 {
                        let _ = tx.send(UiMessage::Progress(processed, total_files));
                    }
                }
            }
        }
    }
    
    use humansize::{format_size, BINARY};
    let report = format!("Batch complete! Processed: {} files | Success: {} | Failed: {}", 
        processed, success, failed);
    
    let _ = tx.send(UiMessage::Log(report));
    let _ = tx.send(UiMessage::Progress(total_files, total_files));
    let _ = tx.send(UiMessage::Finished);
}

/// Worker to process a single folder with its own algorithm setting
fn single_item_worker(path: String, algo: WofAlgorithm, action: BatchAction, row: i32, tx: Sender<UiMessage>, cancel: Arc<AtomicBool>) {
    let mut total_files = 0u64;
    let mut processed = 0u64;
    let mut success = 0u64;
    let mut failed = 0u64;
    
    // Count files first
    for result in WalkBuilder::new(&path).build() {
        if let Ok(entry) = result {
            if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                total_files += 1;
            }
        }
    }
    
    let _ = tx.send(UiMessage::Progress(0, total_files));
    let action_str = match action {
        BatchAction::Compress => "Compressing",
        BatchAction::Decompress => "Decompressing",
    };
    let _ = tx.send(UiMessage::Status(format!("{} {} ({} files)...", action_str, path, total_files)));
    
    // Send initial row update
    let _ = tx.send(UiMessage::RowUpdate(row, format!("0/{}", total_files), "Running".to_string(), "".to_string()));
    
    for result in WalkBuilder::new(&path).build() {
        if cancel.load(Ordering::Relaxed) {
            let _ = tx.send(UiMessage::ItemFinished(row, "Cancelled".to_string(), "".to_string()));
            let _ = tx.send(UiMessage::Status("Cancelled.".to_string()));
            let _ = tx.send(UiMessage::Finished);
            return;
        }
        
        if let Ok(entry) = result {
            if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                let file_path = entry.path().to_string_lossy().to_string();
                
                let ok = match action {
                    BatchAction::Compress => compress_file(&file_path, algo).is_ok(),
                    BatchAction::Decompress => uncompress_file(&file_path).is_ok(),
                };
                
                if ok {
                    success += 1;
                } else {
                    failed += 1;
                }
                
                processed += 1;
                
                // Send per-item progress updates every 5 files or when done
                if processed % 5 == 0 || processed == total_files {
                    let progress_str = format!("{}/{}", processed, total_files);
                    let _ = tx.send(UiMessage::RowUpdate(row, progress_str, "Running".to_string(), "".to_string()));
                    let _ = tx.send(UiMessage::Progress(processed, total_files));
                }
            }
        }
    }
    
    // Calculate size after
    let size_after = calculate_folder_disk_size(&path);
    let size_after_str = format_size(size_after, BINARY);
    
    // Send final status with disk size for On Disk column
    let status = if failed > 0 { format!("Done+{} err", failed) } else { "Done".to_string() };
    let _ = tx.send(UiMessage::ItemFinished(row, status, size_after_str));
    
    let report = format!("Done! {} files | Success: {} | Failed: {}", 
        processed, success, failed);
    
    let _ = tx.send(UiMessage::Status(report));
    let _ = tx.send(UiMessage::Progress(total_files, total_files));
    let _ = tx.send(UiMessage::Finished);
}

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
        let system_backdrop_type = DWMWA_SYSTEMBACKDROP_TYPE;
        let mica = DWM_SYSTEMBACKDROP_TYPE(2);
        let _ = DwmSetWindowAttribute(hwnd, system_backdrop_type, &mica as *const _ as _, 4);
    }
}

unsafe fn is_system_dark_mode() -> bool {
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

fn update_theme(hwnd: HWND) {
    unsafe {
        let dark = is_system_dark_mode();
        let attr = 20; // DWMWA_USE_IMMERSIVE_DARK_MODE
        let val = if dark { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(attr), &val as *const _ as _, std::mem::size_of::<i32>() as u32);
    }
}
