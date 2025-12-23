#![allow(unsafe_op_in_unsafe_fn, non_snake_case)]

use crate::ui::builder::ControlBuilder;
use crate::ui::wrappers::ListView;
use crate::watcher_config::{WatcherTask, WatcherConfig};
use crate::engine::wof::WofAlgorithm;
use crate::engine::scanner::scan_path_metrics;
use crate::w;
use crate::utils::format_size;
use crate::ui::framework::WindowHandler;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, FILETIME};

#[repr(C)]
struct SYSTEMTIME {
    wYear: u16,
    wMonth: u16,
    wDayOfWeek: u16,
    wDay: u16,
    wHour: u16,
    wMinute: u16,
    wSecond: u16,
    wMilliseconds: u16,
}
// use windows_sys::Win32::Storage::FileSystem::FileTimeToLocalFileTime; // Removed
// use windows_sys::Win32::System::Time::FileTimeToSystemTime; // Removed

#[link(name = "kernel32")]
unsafe extern "system" {
    fn FileTimeToLocalFileTime(lpfiletime: *const FILETIME, lplocalfiletime: *mut FILETIME) -> i32;
    fn FileTimeToSystemTime(lpfiletime: *const FILETIME, lpsystemtime: *mut SYSTEMTIME) -> i32;
}

use windows_sys::Win32::UI::WindowsAndMessaging::{
    WM_COMMAND, BN_CLICKED,
    DestroyWindow, MessageBoxW, SendMessageW,
    MB_OK, MoveWindow, GetClientRect,
    WS_POPUP, WS_CAPTION, WS_SYSMENU, WS_VISIBLE, WS_THICKFRAME, WS_MAXIMIZEBOX,
    WM_SIZE, WM_GETMINMAXINFO,
};
use windows_sys::Win32::UI::WindowsAndMessaging::MINMAXINFO;
use windows_sys::Win32::UI::Controls::{LVM_GETITEMCOUNT, NMITEMACTIVATE, NM_CLICK, NM_DBLCLK};
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
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, ShowWindow, SetForegroundWindow, SW_RESTORE};
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
            
            // --- Layout ---
            let padding = 10;
            let mut y = padding;
            
            // 1. List View (Top half)
            let h_list = ControlBuilder::new(hwnd, IDC_LIST_WATCHERS)
                .listview()
                .pos(padding, y)
                .size(660, 200)
                .style(windows_sys::Win32::UI::WindowsAndMessaging::WS_BORDER | windows_sys::Win32::UI::Controls::LVS_REPORT | windows_sys::Win32::UI::Controls::LVS_SINGLESEL | windows_sys::Win32::UI::Controls::LVS_SHOWSELALWAYS)
                .dark_mode(self.is_dark)
                .build();
            
            let lv = ListView::new(h_list);
            // Match FileListView styles for correct theming behavior
            lv.set_extended_style(windows_sys::Win32::UI::Controls::LVS_EX_FULLROWSELECT | windows_sys::Win32::UI::Controls::LVS_EX_DOUBLEBUFFER);
            lv.fix_header_dark_mode(hwnd);
            
            // Clear any existing columns first
            lv.clear_columns();
            
            lv.add_column(0, "Path", 180);
            lv.add_column(1, "Size", 55);
            lv.add_column(2, "On Disk", 55);
            lv.add_column(3, "Schedule", 120);
            lv.add_column(4, "Algorithm", 65);
            lv.add_column(5, "Last Run", 115); // YYYY-MM-DD HH:MM format
            lv.add_column(6, "Action", 55); // Run button column
            
            // Make last column fill remaining space to eliminate empty column appearance
            lv.set_column_width(6, -2); // LVSCW_AUTOSIZE_USEHEADER
            
            lv.apply_theme(self.is_dark);
            
            self.refresh_list(h_list);
            
            y += 210;
            
            // Buttons
            ControlBuilder::new(hwnd, IDC_BTN_ADD).button().text_w(w!("Add...")).pos(padding, y).size(80, 30).dark_mode(self.is_dark).build();
            ControlBuilder::new(hwnd, IDC_BTN_REMOVE).button().text_w(w!("Remove")).pos(padding + 90, y).size(80, 30).dark_mode(self.is_dark).build();
            ControlBuilder::new(hwnd, IDC_BTN_REFRESH).button().text_w(w!("Refresh")).pos(padding + 180, y).size(80, 30).dark_mode(self.is_dark).build();
            
            // Close (Far right)
            ControlBuilder::new(hwnd, IDC_BTN_CLOSE).button().text_w(w!("Close")).pos(580, y).size(100, 30).dark_mode(self.is_dark).build();

            crate::ui::theme::apply_theme_recursive(hwnd, self.is_dark);
        }
        0
    }



    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            match msg {
                windows_sys::Win32::UI::WindowsAndMessaging::WM_NOTIFY => {
                    let nmhdr = lparam as *const windows_sys::Win32::UI::Controls::NMHDR;
                    if (*nmhdr).code == NM_DBLCLK {
                        let nmitem = lparam as *const NMITEMACTIVATE;
                        if (*nmitem).iItem >= 0 {
                            show_watcher_add_modal(hwnd, self.tasks.clone(), self.is_dark, Some((*nmitem).iItem as usize));
                            let h_list = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_LIST_WATCHERS as i32);
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
                                let h_list = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_LIST_WATCHERS as i32);
                                self.refresh_list(h_list);
                            }
                        },
                        IDC_BTN_REMOVE => {
                             if code == BN_CLICKED as u16 {
                                 let h_list = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_LIST_WATCHERS as i32);
                                 // let lv = ListView::new(h_list); // Unused
                                 let count = SendMessageW(h_list, LVM_GETITEMCOUNT, 0, 0) as i32;
                                 let mut selected_idx = -1;
                                 for i in 0..count {
                                     let state = SendMessageW(h_list, windows_sys::Win32::UI::Controls::LVM_GETITEMSTATE, i as WPARAM, windows_sys::Win32::UI::Controls::LVIS_SELECTED as LPARAM);
                                     if (state & windows_sys::Win32::UI::Controls::LVIS_SELECTED as LRESULT) != 0 {
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
                                let h_list = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_LIST_WATCHERS as i32);
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
                    self.on_resize(hwnd, w, h);
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
    fn on_resize(&mut self, hwnd: HWND, _width: i32, _height: i32) {
        unsafe {
            use windows_sys::Win32::Foundation::RECT;
            let mut rc = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetClientRect(hwnd, &mut rc);
            let w = rc.right - rc.left;
            let h = rc.bottom - rc.top;
            
            let padding = 10;
            let btn_height = 30;
            let btn_y = h - btn_height - padding;
            
            // Resize List View: Fill top area up to buttons
            let list_h = if btn_y > padding { btn_y - padding - 10 } else { 100 };
            let h_list = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_LIST_WATCHERS as i32);
            if h_list != std::ptr::null_mut() {
                MoveWindow(h_list, padding, padding, w - (padding * 2), list_h, 1);
            }
            
            // Move Buttons
            // Left aligned: Add, Remove, Refresh
            let h_btn_add = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_BTN_ADD as i32);
            let h_btn_rem = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_BTN_REMOVE as i32);
            let h_btn_ref = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_BTN_REFRESH as i32);
            
            if h_btn_add != std::ptr::null_mut() { MoveWindow(h_btn_add, padding, btn_y, 80, btn_height, 1); }
            if h_btn_rem != std::ptr::null_mut() { MoveWindow(h_btn_rem, padding + 90, btn_y, 80, btn_height, 1); }
            if h_btn_ref != std::ptr::null_mut() { MoveWindow(h_btn_ref, padding + 180, btn_y, 80, btn_height, 1); }
            
            // Right aligned: Close
            let h_btn_close = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_BTN_CLOSE as i32);
            if h_btn_close != std::ptr::null_mut() {
                MoveWindow(h_btn_close, w - 100 - padding, btn_y, 100, btn_height, 1);
            }
        }
    }

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
                format!("Every Day at {:02}:{:02}", task.time_hour, task.time_minute)
            } else {
                 let mut days = Vec::new();
                 if task.days_mask & 1 != 0 { days.push("Mon"); }
                 if task.days_mask & 2 != 0 { days.push("Tue"); }
                 if task.days_mask & 4 != 0 { days.push("Wed"); }
                 if task.days_mask & 8 != 0 { days.push("Thu"); }
                 if task.days_mask & 16 != 0 { days.push("Fri"); }
                 if task.days_mask & 32 != 0 { days.push("Sat"); }
                 if task.days_mask & 64 != 0 { days.push("Sun"); }
                 format!("{} at {:02}:{:02}", days.join(", "), task.time_hour, task.time_minute)
            };
            lv.set_item_text(i as i32, 3, &schedule);
            
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
                "Never".to_string()
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
                    format!("{:04}-{:02}-{:02} {:02}:{:02}", st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute)
                } else {
                    "Error".to_string()
                }
            };
            lv.set_item_text(i as i32, 5, &last_run);
            
            // Action
            lv.set_item_text(i as i32, 6, "▶ Run");
        }
    }
    // add_task removed, now in watcher_add
}
