#![allow(unsafe_op_in_unsafe_fn)]
use windows::core::{Result, w, PCWSTR, PWSTR, PCSTR};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    HBRUSH, COLOR_WINDOW, InvalidateRect, CreateSolidBrush, GetStockObject, NULL_BRUSH, 
    SetBkMode, SetTextColor, TRANSPARENT, FillRect, HDC, CreateFontW, DEFAULT_QUALITY, 
    DEFAULT_PITCH, FF_DONTCARE, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, 
    FW_NORMAL, DEFAULT_CHARSET, FONT_PITCH, DrawTextW, DT_CENTER, DT_VCENTER, DT_SINGLELINE,
};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DwmExtendFrameIntoClientArea, DWMWA_SYSTEMBACKDROP_TYPE, DWM_SYSTEMBACKDROP_TYPE, DWMWINDOWATTRIBUTE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, PostQuitMessage, RegisterClassW, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, SW_SHOW, WM_DESTROY, WNDCLASSW,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE, WM_CREATE, WM_SIZE, WM_COMMAND, SetWindowPos, SWP_NOZORDER,
    GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA, GetDlgItem, WM_DROPFILES, MessageBoxW, MB_OK,
    SendMessageW, CB_ADDSTRING, CB_SETCURSEL, CB_GETCURSEL, SetWindowTextW, WS_CHILD, HMENU, WM_TIMER, SetTimer,
    MB_ICONINFORMATION, WM_NOTIFY, GetClientRect, WM_SETFONT,
};
use windows::Win32::UI::Shell::{DragQueryFileW, DragFinish, HDROP, FileOpenDialog, IFileOpenDialog, FOS_PICKFOLDERS, FOS_FORCEFILESYSTEM, SIGDN_FILESYSPATH, DragAcceptFiles, SetWindowSubclass, DefSubclassProc, SUBCLASSPROC};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, CoTaskMemFree};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW, GetProcAddress};
use windows::Win32::System::Registry::{RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, HKEY};
use crate::gui::controls::{
    create_button, create_listview, create_combobox, create_progress_bar, 
    IDC_LISTVIEW, IDC_BTN_SCAN, IDC_BTN_COMPRESS, IDC_COMBO_ALGO, IDC_BTN_DECOMPRESS, 
    IDC_STATIC_TEXT, IDC_PROGRESS_BAR, IDC_BTN_CANCEL, IDC_BATCH_LIST, IDC_BTN_ADD_FOLDER,
    IDC_BTN_REMOVE, IDC_BTN_PROCESS_ALL, IDC_BTN_ADD_FILES,
};
use crate::gui::state::{AppState, Controls, UiMessage, BatchAction};
use std::thread;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use crossbeam_channel::Sender;
use windows::Win32::UI::Controls::{
    PBM_SETRANGE32, PBM_SETPOS, LVM_INSERTCOLUMNW, LVM_INSERTITEMW, LVM_SETITEMW,
    LVM_DELETEITEM, LVM_DELETEALLITEMS, LVM_GETSELECTEDCOUNT, LVM_GETNEXTITEM,
    LVM_SETBKCOLOR, LVM_SETTEXTCOLOR, LVM_SETTEXTBKCOLOR, SetWindowTheme,
    LVCOLUMNW, LVITEMW, LVCF_WIDTH, LVCF_TEXT, LVCF_FMT, LVCFMT_LEFT, LVIF_TEXT,
    LVNI_SELECTED, LVIF_PARAM, NM_DBLCLK, NMITEMACTIVATE, MARGINS,
    InitCommonControlsEx, INITCOMMONCONTROLSEX, ICC_WIN95_CLASSES, ICC_STANDARD_CLASSES, LVM_GETHEADER,
    NM_CUSTOMDRAW, NMCUSTOMDRAW, CDDS_PREPAINT, CDDS_ITEMPREPAINT, CDRF_NOTIFYITEMDRAW, CDRF_NEWFONT, NMHDR, CDRF_SKIPDEFAULT,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use crate::engine::wof::{compress_file, uncompress_file, WofAlgorithm, get_real_file_size, is_wof_compressed, get_wof_algorithm};
use crate::engine::compresstimate::estimate_size;
use ignore::WalkBuilder;
use humansize::{format_size, BINARY};
use rayon::prelude::*;
use std::sync::atomic::{AtomicU64};

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

// ===== PATH-AWARE FUNCTIONS (work for both files and folders) =====

/// Calculate logical size for a path (file or folder)
fn calculate_path_logical_size(path: &str) -> u64 {
    let p = std::path::Path::new(path);
    if p.is_file() {
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    } else {
        calculate_folder_logical_size(path)
    }
}

/// Calculate disk size for a path (file or folder)
fn calculate_path_disk_size(path: &str) -> u64 {
    let p = std::path::Path::new(path);
    if p.is_file() {
        get_real_file_size(path)
    } else {
        calculate_folder_disk_size(path)
    }
}

/// Detect WOF algorithm for a path (file or folder)  
fn detect_path_algorithm(path: &str) -> Option<WofAlgorithm> {
    let p = std::path::Path::new(path);
    if p.is_file() {
        get_wof_algorithm(path)
    } else {
        detect_folder_algorithm(path)
    }
}

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
    _dwrefdata: usize,
) -> LRESULT {
    unsafe {
        if umsg == WM_NOTIFY && is_system_dark_mode() {
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
                
                let h_add_files = create_button(hwnd, w!("Files"), 10, btn_y, 65, btn_h, IDC_BTN_ADD_FILES);
                let h_add_folder = create_button(hwnd, w!("Folder"), 85, btn_y, 65, btn_h, IDC_BTN_ADD_FOLDER);
                let h_remove = create_button(hwnd, w!("Remove"), 160, btn_y, 70, btn_h, IDC_BTN_REMOVE);
                let h_combo = create_combobox(hwnd, 240, btn_y, 110, 200, IDC_COMBO_ALGO);
                let h_process = create_button(hwnd, w!("Process All"), 360, btn_y, 100, btn_h, IDC_BTN_PROCESS_ALL);
                let h_cancel = create_button(hwnd, w!("Cancel"), 470, btn_y, 80, btn_h, IDC_BTN_CANCEL);
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
                });

                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
                SetTimer(Some(hwnd), 1, 100, None);
                DragAcceptFiles(hwnd, true);
                
                // Apply theme (dark mode support for ListView)
                update_theme(hwnd);

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
                                    
                                    // Get algorithm name for display
                                    let algo_name = match algo {
                                        WofAlgorithm::Xpress4K => "XPRESS4K",
                                        WofAlgorithm::Xpress8K => "XPRESS8K",
                                        WofAlgorithm::Xpress16K => "XPRESS16K",
                                        WofAlgorithm::Lzx => "LZX",
                                    };
                                    
                                    // Update Algorithm column for all items in ListView
                                    let item_count = st.batch_items.len();
                                    for row in 0..item_count {
                                        update_listview_item(ctrls.list_view, row as i32, 1, algo_name);
                                    }
                                    
                                    EnableWindow(ctrls.btn_cancel, true);
                                    SetWindowTextW(ctrls.static_text, w!("Processing batch..."));
                                    
                                    let tx = st.tx.clone();
                                    let cancel = st.cancel_flag.clone();
                                    cancel.store(false, Ordering::Relaxed);
                                    
                                    // Clone items for worker thread, including their row index
                                    let items: Vec<_> = st.batch_items.iter().enumerate().map(|(idx, i)| (i.path.clone(), i.action, idx)).collect();
                                    
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
                // Process All button
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BTN_PROCESS_ALL.into()) {
                    SetWindowPos(h, None, padding + 310, btn_y, 90, btn_height, SWP_NOZORDER);
                }
                // Cancel button
                if let Ok(h) = GetDlgItem(Some(hwnd), IDC_BTN_CANCEL.into()) {
                    SetWindowPos(h, None, padding + 410, btn_y, 70, btn_height, SWP_NOZORDER);
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
                        
                        // Accept both files and folders directly
                        if let Some(st) = get_state() {
                            // Check if already added
                            let already_exists = st.batch_items.iter().any(|item| item.path == path_string);
                            if !already_exists {
                                let item_id = st.add_batch_item(path_string.clone());
                                let logical_size = calculate_path_logical_size(&path_string);
                                let disk_size = calculate_path_disk_size(&path_string);
                                let detected_algo = detect_path_algorithm(&path_string);
                                let logical_str = format_size(logical_size, BINARY);
                                let disk_str = format_size(disk_size, BINARY);
                                if let Some(ctrls) = &st.controls {
                                    add_listview_item(ctrls.list_view, item_id, &path_string, "XPRESS8K", "Compress", &logical_str, &disk_str, detected_algo);
                                }
                            }
                        }
                    }
                }
                
                if let Some(st) = get_state() {
                    if let Some(ctrls) = &st.controls {
                        let count = st.batch_items.len();
                        let msg = format!("{} item(s) in batch queue. Select algorithm and click 'Process All'.", count);
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
                
                // Note: Header NM_CUSTOMDRAW is handled by listview_subclass_proc
                // Header sends NM_CUSTOMDRAW to its parent (ListView), not to main window
                
                LRESULT(0)
            }
            
            // WM_CTLCOLORSTATIC - handle static text colors in dark mode
            0x0138 => { // WM_CTLCOLORSTATIC
                if is_system_dark_mode() {
                    let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut _);
                    SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF)); // White text
                    SetBkMode(hdc, TRANSPARENT);
                    return LRESULT(get_dark_brush().0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            
            // WM_ERASEBKGND - paint dark background
            0x0014 => { // WM_ERASEBKGND
                if is_system_dark_mode() {
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

fn batch_process_worker(items: Vec<(String, BatchAction, usize)>, algo: WofAlgorithm, tx: Sender<UiMessage>, cancel: Arc<AtomicBool>) {
    // 1. Discovery Phase
    let _ = tx.send(UiMessage::Status("Discovering files...".to_string()));
    
    // Store tasks as (path, action, row_index)
    let mut tasks: Vec<(String, BatchAction, usize)> = Vec::new();
    // Track total files per row (row_index -> count)
    let mut row_totals: std::collections::HashMap<usize, u64> = std::collections::HashMap::new();
    
    for (path, action, row_idx) in &items {
        if cancel.load(Ordering::Relaxed) { break; }
        
        let mut row_count = 0;
        
        // If it's a file, just add it
        if std::path::Path::new(path).is_file() {
            tasks.push((path.clone(), *action, *row_idx));
            row_count = 1;
        } else {
            // If it's a directory, walk it
            for result in WalkBuilder::new(path).build() {
                if let Ok(entry) = result {
                    if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                        tasks.push((entry.path().to_string_lossy().to_string(), *action, *row_idx));
                        row_count += 1;
                    }
                }
            }
        }
        
        row_totals.insert(*row_idx, row_count);
        // Initialize row progress
        let _ = tx.send(UiMessage::RowUpdate(*row_idx as i32, format!("0/{}", row_count), "Running".to_string(), "".to_string()));
    }
    
    let total_files = tasks.len() as u64;
    let _ = tx.send(UiMessage::Progress(0, total_files));
    let _ = tx.send(UiMessage::Status(format!("Processing {} files with {} threads...", total_files, rayon::current_num_threads())));
    
    if total_files == 0 {
        let _ = tx.send(UiMessage::Status("No files found to process.".to_string()));
        let _ = tx.send(UiMessage::Finished);
        return;
    }

    // 2. Parallel Processing Phase
    let processed = AtomicU64::new(0);
    let success = AtomicU64::new(0);
    let failed = AtomicU64::new(0);
    
    // Per-row processed counters. We need thread-safe access.
    // Since row indices are 0..items.len(), we can use a Vec<AtomicU64>.
    // But items might not be contiguous if we support deletion (Wait, we rewrite the list on every frame? No, index comes from enumerate)
    // The row index passed in comes from enumerate() on the current state list, so it is 0..N contiguous.
    let max_row = items.iter().map(|(_, _, r)| *r).max().unwrap_or(0);
    let row_processed_counts: Vec<AtomicU64> = (0..=max_row).map(|_| AtomicU64::new(0)).collect();
    // Wrap in Arc implies we need to share it, but pare_iter takes reference. 
    // Wait, Vec can be shared immutably, and AtomicU64 inside it can be mutated.
    // We don't need Arc for the Vec itself if we just reference it in the closure scope, 
    // BUT par_iter requires Send/Sync. Vec<AtomicU64> is Send/Sync.
    
    // Process in parallel
    tasks.par_iter().for_each(|(file_path, action, row_idx)| {
        if cancel.load(Ordering::Relaxed) {
             return; // Stop processing new items
        }
        
        let result = match action {
            BatchAction::Compress => compress_file(file_path, algo).is_ok(),
            BatchAction::Decompress => uncompress_file(file_path).is_ok(),
        };
        
        if result {
            success.fetch_add(1, Ordering::Relaxed);
        } else {
            failed.fetch_add(1, Ordering::Relaxed);
        }
        
        // Global progress
        let current_global = processed.fetch_add(1, Ordering::Relaxed) + 1;
        
        // Row progress
        // Safety: row_idx is bounded by max_row
        if let Some(counter) = row_processed_counts.get(*row_idx) {
            let current_row = counter.fetch_add(1, Ordering::Relaxed) + 1;
            
            // Update row UI (throttled: every 5 items or when done for that row)
            let total_row = *row_totals.get(row_idx).unwrap_or(&1); // fallback to 1 to avoid div by zero/logic error
            
            if current_row % 5 == 0 || current_row == total_row {
                 let _ = tx.send(UiMessage::RowUpdate(*row_idx as i32, format!("{}/{}", current_row, total_row), "Running".to_string(), "".to_string()));
                 
                 // If row finished, we could calculate size? 
                 // Doing it here might be expensive if many threads hit this.
                 // Maybe do it only at the very end of the row?
                 // But parallel execution means we don't know "who" finishes the row last easily without checking equality
                 if current_row == total_row {
                     // Check if it was single file or folder
                     // We don't have the original path here easily unless we look it up or pass it.
                     // But we can just leave the final size update for later or skip it for now to avoid complexity in hot loop.
                     // Re-reading the size happens in ItemFinished mostly.
                     // Let's just update the status to "Done"
                     // let _ = tx.send(UiMessage::RowUpdate(*row_idx as i32, format!("{}/{}", current_row, total_row), "Done".to_string(), "".to_string()));
                 }
            }
        }
        
        // Throttled Global updates
        if current_global % 20 == 0 || current_global == total_files {
             let _ = tx.send(UiMessage::Progress(current_global, total_files));
             let _ = tx.send(UiMessage::Status(format!("Processed {}/{} files...", current_global, total_files)));
        }
    });
    
    if cancel.load(Ordering::Relaxed) {
        let _ = tx.send(UiMessage::Status("Batch processing cancelled.".to_string()));
         let _ = tx.send(UiMessage::Finished);
        return;
    }

    // 3. Final Report & Cleanup
    // Update all rows to "Done" and calculate sizes
    for (path, _, row_idx) in items {
        let size_after = if std::path::Path::new(&path).is_file() {
            calculate_path_disk_size(&path)
        } else {
            calculate_folder_disk_size(&path)
        };
        let size_str = format_size(size_after, BINARY);
        
        // Check if there were failed items for this row?
        // We tracked global failures, but not per-row failures. 
        // For now just mark as Done.
        let _ = tx.send(UiMessage::ItemFinished(row_idx as i32, "Done".to_string(), size_str));
    }

    let s = success.load(Ordering::Relaxed);
    let f = failed.load(Ordering::Relaxed);
    let p = processed.load(Ordering::Relaxed);
    
    let report = format!("Batch complete! Processed: {} files | Success: {} | Failed: {}", p, s, f);
    
    let _ = tx.send(UiMessage::Log(report.clone()));
    let _ = tx.send(UiMessage::Status(report));
    let _ = tx.send(UiMessage::Progress(total_files, total_files));
    let _ = tx.send(UiMessage::Finished);
}

/// Worker to process a single file or folder with its own algorithm setting
fn single_item_worker(path: String, algo: WofAlgorithm, action: BatchAction, row: i32, tx: Sender<UiMessage>, cancel: Arc<AtomicBool>) {
    let mut total_files = 0u64;
    let mut processed = 0u64;
    let mut success = 0u64;
    let mut failed = 0u64;
    
    let is_single_file = std::path::Path::new(&path).is_file();
    
    // Count files first
    if is_single_file {
        total_files = 1;
    } else {
        for result in WalkBuilder::new(&path).build() {
            if let Ok(entry) = result {
                if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    total_files += 1;
                }
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
    
    if is_single_file {
        // Process single file directly
        if cancel.load(Ordering::Relaxed) {
            let _ = tx.send(UiMessage::ItemFinished(row, "Cancelled".to_string(), "".to_string()));
            let _ = tx.send(UiMessage::Status("Cancelled.".to_string()));
            let _ = tx.send(UiMessage::Finished);
            return;
        }
        
        let ok = match action {
            BatchAction::Compress => compress_file(&path, algo).is_ok(),
            BatchAction::Decompress => uncompress_file(&path).is_ok(),
        };
        
        if ok {
            success += 1;
        } else {
            failed += 1;
        }
        processed = 1;
        let _ = tx.send(UiMessage::RowUpdate(row, "1/1".to_string(), "Running".to_string(), "".to_string()));
        let _ = tx.send(UiMessage::Progress(1, 1));
    } else {
        // Process folder in PARALLEL
        let mut tasks: Vec<String> = Vec::new();
        for result in WalkBuilder::new(&path).build() {
            if let Ok(entry) = result {
                if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                     tasks.push(entry.path().to_string_lossy().to_string());
                }
            }
        }
        
        total_files = tasks.len() as u64;
        
        let processed_atomic = AtomicU64::new(0);
        let success_atomic = AtomicU64::new(0);
        let failed_atomic = AtomicU64::new(0);
        
        tasks.par_iter().for_each(|file_path| {
            if cancel.load(Ordering::Relaxed) {
                 return;
            }
            
            let ok = match action {
                BatchAction::Compress => compress_file(file_path, algo).is_ok(),
                BatchAction::Decompress => uncompress_file(file_path).is_ok(),
            };
            
            if ok {
                success_atomic.fetch_add(1, Ordering::Relaxed);
            } else {
                failed_atomic.fetch_add(1, Ordering::Relaxed);
            }
            
            let current = processed_atomic.fetch_add(1, Ordering::Relaxed) + 1;
            
            // Send per-item progress updates every 5 files or when done
            if current % 5 == 0 || current == total_files {
                let progress_str = format!("{}/{}", current, total_files);
                let _ = tx.send(UiMessage::RowUpdate(row, progress_str, "Running".to_string(), "".to_string()));
                let _ = tx.send(UiMessage::Progress(current, total_files));
            }
        });
        
        if cancel.load(Ordering::Relaxed) {
            let _ = tx.send(UiMessage::ItemFinished(row, "Cancelled".to_string(), "".to_string()));
            let _ = tx.send(UiMessage::Status("Cancelled.".to_string()));
            let _ = tx.send(UiMessage::Finished);
            return;
        }
        
        // Sync back to local variables for the final report
        processed = processed_atomic.load(Ordering::Relaxed);
        success = success_atomic.load(Ordering::Relaxed);
        failed = failed_atomic.load(Ordering::Relaxed);
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
        let is_dark = is_system_dark_mode();
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
        let dark = is_system_dark_mode();
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
        
        // Helper to update button theme, font, and force dark mode
        let update_btn_theme = |id: u16| {
            if let Ok(btn) = GetDlgItem(Some(hwnd), id as i32) {
                if !btn.is_invalid() {
                    // Force dark mode on control itself (Ordinal 133)
                    allow_dark_mode_for_window(btn, dark);
                    
                    let font_height = -12;
                    let hfont = CreateFontW(
                        font_height, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, DEFAULT_CHARSET,
                        OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                        w!("Segoe UI Variable Display"));
                    
                    SendMessageW(btn, WM_SETFONT, Some(WPARAM(hfont.0 as usize)), Some(LPARAM(1)));
                    
                    if dark {
                         // DarkMode_Explorer gives dark buttons (with border)
                        let _ = SetWindowTheme(btn, w!("DarkMode_Explorer"), None);
                    } else {
                         let _ = SetWindowTheme(btn, w!("Explorer"), None);
                    }
                }
            }
        };
        
        // Update all buttons
        update_btn_theme(IDC_BTN_ADD_FILES);
        update_btn_theme(IDC_BTN_ADD_FOLDER);
        update_btn_theme(IDC_BTN_REMOVE);
        update_btn_theme(IDC_BTN_PROCESS_ALL);
        update_btn_theme(IDC_BTN_CANCEL);
        
        // Update ComboBox
        if let Ok(combo) = GetDlgItem(Some(hwnd), IDC_COMBO_ALGO as i32) {
             if !combo.is_invalid() {
                 SendMessageW(combo, WM_SETFONT, Some(WPARAM(hfont.0 as usize)), Some(LPARAM(1)));
                 allow_dark_mode_for_window(combo, dark);
                 if dark {
                     let _ = SetWindowTheme(combo, w!("Explorer"), None); 
                 } else {
                     let _ = SetWindowTheme(combo, w!("Explorer"), None);
                 }
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
                 0, // Reference data
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

