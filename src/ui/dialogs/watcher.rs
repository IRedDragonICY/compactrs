#![allow(unsafe_op_in_unsafe_fn, non_snake_case)]

use crate::ui::builder::ControlBuilder;
use crate::ui::wrappers::ListView;
use crate::watcher_config::{WatcherTask, WatcherConfig};
use crate::engine::wof::WofAlgorithm;
use crate::engine::scanner::scan_path_metrics;
use crate::w;
use crate::utils::format_size;
use crate::ui::framework::WindowHandler;
use crate::types::*;

// Imported from crate::types::*;


#[link(name = "kernel32")]
unsafe extern "system" {
    fn FileTimeToLocalFileTime(lpfiletime: *const FILETIME, lplocalfiletime: *mut FILETIME) -> i32;
    fn FileTimeToSystemTime(lpfiletime: *const FILETIME, lpsystemtime: *mut SYSTEMTIME) -> i32;
}

use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use crate::ui::state::UiMessage;
use crate::ui::dialogs::watcher_add::show_watcher_add_modal;

const WATCHER_TITLE: &str = "Watcher Manager";

// Control IDs
const IDC_LIST_WATCHERS: u16 = 3001;
const IDC_BTN_ADD: u16 = 3002;
const IDC_BTN_REMOVE: u16 = 3003;
const IDC_BTN_REFRESH: u16 = 3022;
const IDC_BTN_CLOSE: u16 = 3021;
// Removed other IDs as they are now in watcher_add

struct WatcherState {
    tasks: Arc<Mutex<Vec<WatcherTask>>>,
    tx: Sender<UiMessage>,
    is_dark: bool,
}

pub unsafe fn show_watcher_modal(
    parent: HWND,
    tasks_arc: Arc<Mutex<Vec<WatcherTask>>>,
    tx: Sender<UiMessage>,
    is_dark: bool
) {
    let mut state = WatcherState {
        tasks: tasks_arc,
        tx,
        is_dark,
    };

    let bg_brush = crate::ui::theme::get_background_brush(is_dark);
    
    // Check for existing window
    

    let class_name = "CompactRS_Watcher";
    let class_name_w = crate::utils::to_wstring(class_name);
    let existing_hwnd = unsafe { FindWindowW(class_name_w.as_ptr(), std::ptr::null()) };
    
    if existing_hwnd != std::ptr::null_mut() {
        unsafe {
            ShowWindow(existing_hwnd, SW_RESTORE);
            SetForegroundWindow(existing_hwnd);
        }
        return;
    }

    use crate::ui::framework::{WindowBuilder, WindowAlignment, show_modal};
    show_modal(
        WindowBuilder::new(&mut state, class_name, WATCHER_TITLE)
            // Use WS_OVERLAPPEDWINDOW for a normal resizable window, or mix styles
            .style(WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE | WS_THICKFRAME | WS_MAXIMIZEBOX)
            .size(700, 400)
            .align(WindowAlignment::CenterOnParent)
            .background(bg_brush),
        parent
    );
}

impl WindowHandler for WatcherState {
    fn is_dark_mode(&self) -> bool {
        self.is_dark
    }

    fn on_create(&mut self, hwnd: HWND) -> LRESULT {
        unsafe {

            crate::ui::theme::set_window_frame_theme(hwnd, self.is_dark);
            
            // Helper
            let builder = |id| ControlBuilder::new(hwnd, id).dark_mode(self.is_dark);
            let btn = |text, id| builder(id).button().text_w(&crate::utils::to_wstring(text)).build();
            
            // 1. List View
            let h_list = builder(IDC_LIST_WATCHERS)
                .listview()
                .style(WS_BORDER | LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS)
                .build();
            
            let lv = ListView::new(h_list);
            lv.set_extended_style(LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER);
            lv.fix_header_dark_mode(hwnd);
            
            lv.clear_columns();
            lv.add_column(0, "Path", 180);
            lv.add_column(1, "Size", 55);
            lv.add_column(2, "On Disk", 55);
            lv.add_column(3, "Schedule", 120);
            lv.add_column(4, "Algorithm", 65);
            lv.add_column(5, "Last Run", 115);
            lv.add_column(6, "Action", 55);
            lv.set_column_width(6, -2);
            lv.apply_theme(self.is_dark);
            
            self.refresh_list(h_list);
            
            // 2. Buttons
            let h_btn_add = btn("Add...", IDC_BTN_ADD);
            let h_btn_remove = btn("Remove", IDC_BTN_REMOVE);
            let h_btn_refresh = btn("Refresh", IDC_BTN_REFRESH);
            let h_btn_close = btn("Close", IDC_BTN_CLOSE);

            crate::ui::theme::apply_theme_recursive(hwnd, self.is_dark);
            
            // Initial Layout
            let client_rect = crate::utils::get_client_rect(hwnd);
            self.do_layout(hwnd, client_rect, h_list, h_btn_add, h_btn_remove, h_btn_refresh, h_btn_close);
        }
        0
    }

    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            match msg {
                WM_NOTIFY => {
                    let nmhdr = lparam as *const NMHDR;
                    if (*nmhdr).code == NM_DBLCLK {
                        let nmitem = lparam as *const NMITEMACTIVATE;
                        if (*nmitem).iItem >= 0 {
                            show_watcher_add_modal(hwnd, self.tasks.clone(), self.is_dark, Some((*nmitem).iItem as usize));
                            let h_list = GetDlgItem(hwnd, IDC_LIST_WATCHERS as i32);
                            self.refresh_list(h_list);
                        }
                    } else if (*nmhdr).code == NM_CLICK {
                        let nmitem = lparam as *const NMITEMACTIVATE;
                        if (*nmitem).iItem >= 0 && (*nmitem).iSubItem == 6 {
                             // Run Button Clicked
                             let task_opt = {
                                 let tasks = self.tasks.lock().unwrap();
                                 tasks.get((*nmitem).iItem as usize).cloned()
                             };
                             if let Some(task) = task_opt {
                                 // Trigger Run
                                 let _ = self.tx.send(UiMessage::WatcherTrigger(task.get_path(), task.algorithm));
                                 MessageBoxW(hwnd, w!("Triggered manual run.").as_ptr(), w!("CompactRS").as_ptr(), MB_OK);
                             }
                        }
                    }
                },
                WM_COMMAND => {
                    let id = (wparam & 0xFFFF) as u16;
                    let code = ((wparam >> 16) & 0xFFFF) as u16;
                    
                    match id {
                        IDC_BTN_ADD => {
                            if code == BN_CLICKED as u16 {
                                show_watcher_add_modal(hwnd, self.tasks.clone(), self.is_dark, None);
                                let h_list = GetDlgItem(hwnd, IDC_LIST_WATCHERS as i32);
                                self.refresh_list(h_list);
                            }
                        },
                        IDC_BTN_REMOVE => {
                             if code == BN_CLICKED as u16 {
                                 let h_list = GetDlgItem(hwnd, IDC_LIST_WATCHERS as i32);
                                 let count = SendMessageW(h_list, LVM_GETITEMCOUNT, 0, 0) as i32;
                                 let mut selected_idx = -1;
                                 for i in 0..count {
                                     let state = SendMessageW(h_list, LVM_GETITEMSTATE, i as WPARAM, LVIS_SELECTED as LPARAM);
                                     if (state & LVIS_SELECTED as LRESULT) != 0 {
                                         selected_idx = i;
                                         break;
                                     }
                                 }
                                 
                                 if selected_idx >= 0 {
                                     {
                                         let mut tasks = self.tasks.lock().unwrap();
                                         if selected_idx < tasks.len() as i32 {
                                             tasks.remove(selected_idx as usize);
                                             let _ = WatcherConfig::save(&tasks);
                                         }
                                     }
                                     self.refresh_list(h_list);
                                 }
                             }
                        },
                        IDC_BTN_REFRESH => {
                            if code == BN_CLICKED as u16 {
                                let h_list = GetDlgItem(hwnd, IDC_LIST_WATCHERS as i32);
                                self.refresh_list(h_list);
                            }
                        },
                        IDC_BTN_CLOSE => {
                            DestroyWindow(hwnd);
                        },
                        _ => {}
                    }
                },
                WM_SIZE => {
                    let w = (lparam & 0xFFFF) as i32;
                    let h = ((lparam >> 16) & 0xFFFF) as i32;
                    let rect = RECT { left: 0, top: 0, right: w, bottom: h };
                    
                    let h_list = GetDlgItem(hwnd, IDC_LIST_WATCHERS as i32);
                    let h_btn_add = GetDlgItem(hwnd, IDC_BTN_ADD as i32);
                    let h_btn_remove = GetDlgItem(hwnd, IDC_BTN_REMOVE as i32);
                    let h_btn_refresh = GetDlgItem(hwnd, IDC_BTN_REFRESH as i32);
                    let h_btn_close = GetDlgItem(hwnd, IDC_BTN_CLOSE as i32);
                    
                    self.do_layout(hwnd, rect, h_list, h_btn_add, h_btn_remove, h_btn_refresh, h_btn_close);
                },
                WM_GETMINMAXINFO => {
                    let mmi = lparam as *mut MINMAXINFO;
                    self.on_min_max_info(hwnd, mmi);
                },
                _ => {
                    return None;
                }
            }
        }
        Some(0)
    }
}

impl WatcherState {
    unsafe fn do_layout(&mut self, _hwnd: HWND, rect: RECT, h_list: HWND, h_add: HWND, h_rem: HWND, h_ref: HWND, h_close: HWND) {
         use crate::ui::layout::{LayoutNode, SizePolicy::{Fixed, Flex}};
         
         LayoutNode::col(10, 10)
             .with(h_list, Flex(1.0))
             .with_child(LayoutNode::row(0, 5)
                 .with_policy(Fixed(28)) // Fix row height to 28px
                 .with(h_add, Fixed(80))
                 .with(h_rem, Fixed(80))
                 .with(h_ref, Fixed(80))
                 .flex_spacer()
                 .with(h_close, Fixed(100))
             )
             .apply_layout(rect);

         // Dynamic Column Resizing
         let list_w = (rect.right - rect.left) - 20; // -20 for padding
         let fixed_w = 55 + 55 + 120 + 65 + 115 + 60; // Sum of other columns
         let scroll_pad = 25; // Space for scrollbar
         let path_w = list_w - fixed_w - scroll_pad;
         
         if path_w > 100 {
             let lv = ListView::new(h_list);
             lv.set_column_width(0, path_w);
             lv.set_column_width(6, 60); // Fix Action column
         }
    }
    
    // on_resize removed (replaced by do_layout called from WM_SIZE)

    fn on_min_max_info(&mut self, _hwnd: HWND, mmi: *mut MINMAXINFO) {
        unsafe {
            // Set minimum size to prevent UI breaking
            (*mmi).ptMinTrackSize.x = 600;
            (*mmi).ptMinTrackSize.y = 300;
        }
    }
    unsafe fn refresh_list(&self, h_list: HWND) {
        let lv = ListView::new(h_list);
        lv.clear();
        
        let tasks = self.tasks.lock().unwrap();
        for (i, task) in tasks.iter().enumerate() {
            let path = task.get_path();
            lv.insert_item(i as i32, &path, 0);

            // Calc Size
            // Note: This is synchronous and might block UI for large folders.
            // For a settings dialog, this is acceptable for now.
            let metrics = scan_path_metrics(&path);
            let size_str = String::from_utf16_lossy(&format_size(metrics.logical_size));
            let disk_str = String::from_utf16_lossy(&format_size(metrics.disk_size));
            // Trim nulls if format_size returns them
            let size_str = size_str.trim_matches('\0');
            let disk_str = disk_str.trim_matches('\0');
            
            lv.set_item_text(i as i32, 1, size_str);
            lv.set_item_text(i as i32, 2, disk_str);
            
            // Schedule String
            let schedule = if (task.days_mask & 0x80) != 0 {
                crate::utils::concat_wstrings(&[
                     crate::w!("Every Day at "),
                     &crate::utils::fmt_u32_padded(task.time_hour as u32),
                     crate::w!(":"),
                     &crate::utils::fmt_u32_padded(task.time_minute as u32)
                ])
            } else {
                 let mut days = Vec::new();
                 if task.days_mask & 1 != 0 { days.push("Mon"); }
                 if task.days_mask & 2 != 0 { days.push("Tue"); }
                 if task.days_mask & 4 != 0 { days.push("Wed"); }
                 if task.days_mask & 8 != 0 { days.push("Thu"); }
                 if task.days_mask & 16 != 0 { days.push("Fri"); }
                 if task.days_mask & 32 != 0 { days.push("Sat"); }
                 if task.days_mask & 64 != 0 { days.push("Sun"); }
                 let days_str = days.join(", ");
                 crate::utils::concat_wstrings(&[
                      &crate::utils::to_wstring(&days_str),
                      crate::w!(" at "),
                      &crate::utils::fmt_u32_padded(task.time_hour as u32),
                      crate::w!(":"),
                      &crate::utils::fmt_u32_padded(task.time_minute as u32)
                 ])
            };
            lv.set_item_text_w(i as i32, 3, &schedule);
            
            // Algo
            let algo = match task.algorithm {
                WofAlgorithm::Xpress4K => "XPRESS4K",
                WofAlgorithm::Xpress8K => "XPRESS8K",
                WofAlgorithm::Xpress16K => "XPRESS16K",
                WofAlgorithm::Lzx => "LZX",
            };
            lv.set_item_text(i as i32, 4, algo);

            // Last Run
            let last_run = if task.last_run_timestamp == 0 {
                crate::utils::to_wstring("Never")
            } else {
                // Convert Unix timestamp to Windows FILETIME then to SYSTEMTIME
                // Unix epoch (1970) to Windows epoch (1601) = 11644473600 seconds
                let windows_ticks = (task.last_run_timestamp + 11644473600) * 10_000_000;
                let ft = FILETIME {
                    dwLowDateTime: (windows_ticks & 0xFFFFFFFF) as u32,
                    dwHighDateTime: (windows_ticks >> 32) as u32,
                };
                let mut local_ft = FILETIME {
                    dwLowDateTime: 0,
                    dwHighDateTime: 0,
                };
                let mut st = std::mem::zeroed::<SYSTEMTIME>();
                
                if unsafe { FileTimeToLocalFileTime(&ft, &mut local_ft) } != 0
                    && unsafe { FileTimeToSystemTime(&local_ft, &mut st) } != 0 {
                    crate::utils::concat_wstrings(&[
                        &crate::utils::fmt_u32(st.wYear as u32),
                        crate::w!("-"),
                        &crate::utils::fmt_u32_padded(st.wMonth as u32),
                        crate::w!("-"),
                        &crate::utils::fmt_u32_padded(st.wDay as u32),
                        crate::w!(" "),
                        &crate::utils::fmt_u32_padded(st.wHour as u32),
                        crate::w!(":"),
                        &crate::utils::fmt_u32_padded(st.wMinute as u32)
                    ])
                } else {
                    crate::utils::to_wstring("Error")
                }
            };
            lv.set_item_text_w(i as i32, 5, &last_run);
            
            // Action
            lv.set_item_text(i as i32, 6, "▶ Run");
        }
    }
    // add_task removed, now in watcher_add
}
