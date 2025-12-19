/* --- src/ui/handlers.rs --- */
#![allow(unsafe_op_in_unsafe_fn)]

use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM, POINT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SendMessageW, MessageBoxW, SetWindowTextW, MB_OK, MB_ICONINFORMATION,
    GetCursorPos, TrackPopupMenu, CreatePopupMenu, AppendMenuW, DestroyMenu,
    TPM_RETURNCMD, TPM_LEFTALIGN, MF_STRING, CB_GETCURSEL, 
    WM_COMMAND,
};
use windows_sys::Win32::System::DataExchange::{
    OpenClipboard, CloseClipboard, GetClipboardData, IsClipboardFormatAvailable
};


use windows_sys::Win32::UI::Shell::{DragQueryFileW, DragFinish, HDROP};
use windows_sys::Win32::UI::Controls::{NM_CLICK, NM_DBLCLK, NMLISTVIEW};
use windows_sys::Win32::Graphics::Gdi::InvalidateRect;
use std::thread;
use std::sync::atomic::Ordering;
use std::cmp::Ordering as CmpOrdering;

use crate::ui::state::{AppState, BatchAction, ProcessingState, BatchStatus};
use crate::ui::taskbar::TaskbarState; // Fixed Import
use crate::ui::controls::*;
use crate::ui::theme;
use crate::engine::wof::WofAlgorithm;
use crate::engine::worker::batch_process_worker;
use crate::utils::{to_wstring, u64_to_wstring, concat_wstrings, reveal_path_in_explorer};
use crate::ui::file_dialog::{pick_files, pick_folder};

// --- Command Handlers ---

pub unsafe fn on_add_files(st: &mut AppState) {
    if let Ok(files) = pick_files() {
        st.ingest_paths(files);
    }
}

pub unsafe fn on_add_folder(st: &mut AppState) {
    if let Ok(folder) = pick_folder() {
        st.ingest_paths(vec![folder]);
    }
}

pub unsafe fn on_remove_selected(st: &mut AppState) {
    let mut selected_indices = if let Some(ctrls) = &st.controls {
        ctrls.file_list.get_selected_indices()
    } else { Vec::new() };
    
    // Sort reverse to remove correctly
    selected_indices.sort_by(|a, b| b.cmp(a));
    
    let ids_to_remove: Vec<u32> = selected_indices.iter()
        .filter_map(|&idx| st.batch_items.get(idx).map(|item| item.id))
        .collect();
    
    for id in ids_to_remove { 
        st.remove_batch_item(id); 
    }
    
    if let Some(ctrls) = &st.controls {
        for idx in selected_indices { 
            ctrls.file_list.remove_item(idx as i32); 
        }
    }
}

pub unsafe fn on_process_all(st: &mut AppState, hwnd: HWND, is_auto_start: bool) {
    if st.batch_items.is_empty() {
        let w_info = to_wstring("Info");
        let w_msg = to_wstring("Add folders first!");
        MessageBoxW(hwnd, w_msg.as_ptr(), w_info.as_ptr(), MB_OK | MB_ICONINFORMATION);
        return;
    }
    
    if let Some(ctrls) = &st.controls {
        let mut indices = ctrls.file_list.get_selected_indices();
        if is_auto_start && indices.is_empty() { return; }
        if indices.is_empty() { indices = (0..st.batch_items.len()).collect(); }
        start_processing(st, hwnd, indices);
    }
}

pub unsafe fn start_processing(st: &mut AppState, hwnd: HWND, indices_to_process: Vec<usize>) {
    if indices_to_process.is_empty() { return; }

    if let Some(ctrls) = &st.controls {
        // Read Global Settings
        let idx = SendMessageW(ctrls.action_panel.combo_hwnd(), CB_GETCURSEL, 0, 0);
        let use_as_listed = idx == 0;
        let global_algo = match idx {
            1 => WofAlgorithm::Xpress4K,
            3 => WofAlgorithm::Xpress16K,
            4 => WofAlgorithm::Lzx,
            _ => WofAlgorithm::Xpress8K,
        };
        
        // Update UI if overriding per-item settings
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
        
        // Prepare UI state
        if let Some(tb) = &st.taskbar { tb.set_state(TaskbarState::Normal); }
        windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(ctrls.action_panel.cancel_hwnd(), 1);

        let count_w = u64_to_wstring(indices_to_process.len() as u64);
        let status_msg = concat_wstrings(&[&to_wstring("Processing "), &count_w, &to_wstring(" items...")]);
        SetWindowTextW(ctrls.status_bar.label_hwnd(), status_msg.as_ptr());
        
        // Prepare Data for Worker
        let tx = st.tx.clone();
        let state_global = st.global_state.clone();
        state_global.store(ProcessingState::Running as u8, Ordering::Relaxed);
        
        let action_mode_idx = SendMessageW(ctrls.action_panel.action_mode_hwnd(), CB_GETCURSEL, 0, 0);
        
        let items: Vec<_> = indices_to_process.into_iter().filter_map(|idx| {
            st.batch_items.get(idx).map(|item| {
                let effective_action = match action_mode_idx {
                    1 => BatchAction::Compress, 2 => BatchAction::Decompress, _ => item.action,
                };
                let effective_algo = if use_as_listed { item.algorithm } else { global_algo };
                (item.path.clone(), effective_action, idx, effective_algo)
            })
        }).collect();
        
        let force = st.force_compress;
        let guard = st.config.enable_system_guard;
        let main_hwnd_usize = hwnd as usize;
        
        // Launch Worker Thread
        thread::spawn(move || {
            batch_process_worker(items, tx, state_global, force, main_hwnd_usize, guard);
        });
    }
}

pub unsafe fn on_stop_processing(st: &mut AppState) {
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
}

pub unsafe fn on_open_settings(st: &mut AppState, hwnd: HWND) {
    let current_theme = st.theme;
    let is_dark = theme::resolve_mode(st.theme);
    let (new_theme, new_force, new_ctx, new_guard) = crate::ui::settings::show_settings_modal(
        hwnd, current_theme, is_dark, st.enable_force_stop, st.config.enable_context_menu, st.config.enable_system_guard
    );
    
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
}

// --- Notification Handlers (ListView) ---

pub unsafe fn on_list_click(st: &mut AppState, hwnd: HWND, row: i32, col: i32, code: u32) {
    if row < 0 { return; }
    
    if col == 0 && code == NM_DBLCLK { // Open Path
         if let Some(item) = st.batch_items.get(row as usize) {
             reveal_path_in_explorer(&item.path);
         }
    } else if col == 2 && code == NM_DBLCLK { // Cycle Algo
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
    } else if col == 3 && code == NM_DBLCLK { // Toggle Action
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
    } else if col == 8 && code == NM_CLICK { // Start/Pause Single Item
           if let Some(item) = st.batch_items.get_mut(row as usize) {
                // Trigger single item processing
                // Fix unused var warning by using the var
                let _ = item; 
                let indices = vec![row as usize];
                start_processing(st, hwnd, indices);
           }
    }
}

pub unsafe fn on_list_keydown(st: &mut AppState, _hwnd: HWND, key: u16) {
    if key == windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_DELETE as u16 {
        on_remove_selected(st);
    }
}

pub unsafe fn on_column_click(st: &mut AppState, lparam: LPARAM) {
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

// --- Context Menu Handler ---

pub unsafe fn handle_context_menu(st: &mut AppState, hwnd: HWND, wparam: WPARAM) {
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
                        let _ = AppendMenuW(menu, MF_STRING, 1003, to_wstring("Stop").as_ptr());
                    } else if any_pending {
                        let _ = AppendMenuW(menu, MF_STRING, 1005, to_wstring("Start Selected").as_ptr());
                    }
                    let _ = AppendMenuW(menu, MF_STRING, 1004, to_wstring("Remove").as_ptr());
                    let _ = AppendMenuW(menu, MF_STRING, 1006, to_wstring("Open File Location").as_ptr());

                    let _cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_LEFTALIGN, pt.x, pt.y, 0, hwnd, std::ptr::null());
                    DestroyMenu(menu);
                    
                    match _cmd {
                        1003 => { let _ = SendMessageW(hwnd, WM_COMMAND, IDC_BTN_CANCEL as usize, 0); },
                        1004 => { let _ = SendMessageW(hwnd, WM_COMMAND, IDC_BTN_REMOVE as usize, 0); },
                        1005 => start_processing(st, hwnd, selected.clone()),
                        1006 => {
                            if let Some(&first_idx) = selected.first() {
                                if let Some(item) = st.batch_items.get(first_idx as usize) {
                                    reveal_path_in_explorer(&item.path);
                                }
                            }
                        },
                        _ => {}
                    }
                }
            }
        }
    }
}

// --- Drag and Drop / Clipboard Handler ---

pub unsafe fn process_hdrop(_hwnd: HWND, hdrop: HDROP, st: &mut AppState) {
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
    DragFinish(hdrop);
    st.ingest_paths(paths);
}

pub unsafe fn process_clipboard(hwnd: HWND, st: &mut AppState) {
    // CF_HDROP = 15
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

// --- Sorter ---

pub unsafe extern "system" fn compare_items(lparam1: isize, lparam2: isize, lparam_sort: isize) -> i32 {
    let state = &*(lparam_sort as *const AppState);
    let id1 = lparam1 as u32;
    let id2 = lparam2 as u32;
    
    let item1 = state.batch_items.iter().find(|i| i.id == id1);
    let item2 = state.batch_items.iter().find(|i| i.id == id2);
    
    match (item1, item2) {
        (Some(i1), Some(i2)) => {
            let ord = match state.sort_column {
                0 => i1.path.to_lowercase().cmp(&i2.path.to_lowercase()), 
                2 => format!("{:?}", i1.algorithm).cmp(&format!("{:?}", i2.algorithm)),
                3 => format!("{:?}", i1.action).cmp(&format!("{:?}", i2.action)),
                4 => i1.logical_size.cmp(&i2.logical_size),
                5 => i1.disk_size.cmp(&i2.disk_size),
                7 => format!("{:?}", i1.status).cmp(&format!("{:?}", i2.status)),
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