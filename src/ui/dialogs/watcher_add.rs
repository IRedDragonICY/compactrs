#![allow(unsafe_op_in_unsafe_fn)]

use crate::ui::builder::ControlBuilder;
use crate::ui::wrappers::{Button, ComboBox};
use crate::watcher_config::{WatcherTask, WatcherConfig};
use crate::engine::wof::WofAlgorithm;
use crate::w;
use crate::utils::to_wstring;
use crate::ui::framework::WindowHandler;
use crate::types::*;
use std::sync::{Arc, Mutex};

const TITLE_ADD: &str = "Add Watcher Task";
const TITLE_EDIT: &str = "Edit Watcher Task";
const WM_COPYGLOBALDATA: u32 = 0x0049;

// Control IDs (Reuse from watcher.rs or define new)
const IDC_EDIT_PATH: u16 = 3005;
const IDC_BTN_BROWSE: u16 = 3004;
const IDC_BTN_BROWSE_FILE: u16 = 3009;
const IDC_COMBO_ALGO: u16 = 3006;
const IDC_EDIT_HOUR: u16 = 3007;
const IDC_EDIT_MIN: u16 = 3008;
const IDC_CHK_MON: u16 = 3010;
const IDC_CHK_TUE: u16 = 3011;
const IDC_CHK_WED: u16 = 3012;
const IDC_CHK_THU: u16 = 3013;
const IDC_CHK_FRI: u16 = 3014;
const IDC_CHK_SAT: u16 = 3015;
const IDC_CHK_SUN: u16 = 3016;
const IDC_CHK_EVERYDAY: u16 = 3017;
const IDC_BTN_SAVE: u16 = 4001;
const IDC_BTN_CANCEL: u16 = 4002;

struct WatcherAddState {
    tasks: Arc<Mutex<Vec<WatcherTask>>>,
    is_dark: bool,
    edit_index: Option<usize>,
}

pub unsafe fn show_watcher_add_modal(
    parent: HWND,
    tasks_arc: Arc<Mutex<Vec<WatcherTask>>>,
    is_dark: bool,
    edit_index: Option<usize>,
) {
    let mut state = WatcherAddState {
        tasks: tasks_arc,
        is_dark,
        edit_index,
    };

    let title = if edit_index.is_some() { TITLE_EDIT } else { TITLE_ADD };

    crate::ui::dialogs::base::show_modal_singleton(
        parent,
        &mut state,
        "CompactRS_WatcherAdd",
        title,
        600,
        200,
        is_dark
    );
}

impl WindowHandler for WatcherAddState {
    fn is_dark_mode(&self) -> bool {
        self.is_dark
    }

    fn on_create(&mut self, hwnd: HWND) -> LRESULT {
        unsafe {
            use crate::ui::layout::{LayoutNode, SizePolicy::{Fixed, Flex}};
            crate::ui::theme::set_window_frame_theme(hwnd, self.is_dark);
            
            // Enable Drag and Drop
            ChangeWindowMessageFilter(WM_DROPFILES, MSGFLT_ADD);
            ChangeWindowMessageFilter(WM_COPYGLOBALDATA, MSGFLT_ADD);
            DragAcceptFiles(hwnd, true as i32);
            
            let padding = 10;
            
            // Helper for control creation
            let builder = |id| ControlBuilder::new(hwnd, id).dark_mode(self.is_dark);
            let lbl = |text| builder(0).label(false).text(text).build();
            let btn = |text, id| builder(id).button().text_w(&crate::utils::to_wstring(text)).build();
            
            // 1. Path Controls
            let h_lbl_path = lbl("Path:");
            let h_path = builder(IDC_EDIT_PATH).edit().build();
            let h_btn_browse = btn("Folder", IDC_BTN_BROWSE);
            let h_btn_file = btn("File", IDC_BTN_BROWSE_FILE);
            
            // 2. Algo
            let h_lbl_algo = lbl("Algorithm:");
            let h_combo = builder(IDC_COMBO_ALGO).combobox().build();
            let cb = ComboBox::new(h_combo);
            cb.add_string("XPRESS 4K");
            cb.add_string("XPRESS 8K");
            cb.add_string("XPRESS 16K");
            cb.add_string("LZX");
            cb.set_selected_index(1);
            
            // 3. Time
            let h_lbl_time = lbl("Time (HH:MM):");
            let h_hour = builder(IDC_EDIT_HOUR).edit().style(ES_NUMBER).build();
            let h_sep = lbl(":");
            let h_min = builder(IDC_EDIT_MIN).edit().style(ES_NUMBER).build();
            
            // 4. Days
            let h_lbl_days = lbl("Days:");
            let days = [
                (IDC_CHK_MON, "Mon"), (IDC_CHK_TUE, "Tue"), (IDC_CHK_WED, "Wed"),
                (IDC_CHK_THU, "Thu"), (IDC_CHK_FRI, "Fri"), (IDC_CHK_SAT, "Sat"),
                (IDC_CHK_SUN, "Sun"), (IDC_CHK_EVERYDAY, "Every Day")
            ];
            
            let mut days_node = LayoutNode::row(0, 5);
            for (id, txt) in days {
                 let h = builder(id).checkbox().text(txt).checked(id == IDC_CHK_EVERYDAY).build();
                 days_node = days_node.with(h, Fixed(if id == IDC_CHK_EVERYDAY { 90 } else { 55 }));
            }
            
            // 5. Buttons
            let h_btn_save = btn("Save", IDC_BTN_SAVE);
            let h_btn_cancel = btn("Cancel", IDC_BTN_CANCEL);
            
            // Build Layout
            let client_rect = crate::utils::get_client_rect(hwnd);
            
            LayoutNode::col(padding, 15)
                .with_child(LayoutNode::row(0, 5)
                    .with(h_lbl_path, Fixed(50))
                    .with(h_path, Flex(1.0))
                    .with(h_btn_browse, Fixed(70))
                    .with(h_btn_file, Fixed(70))
                )
                .with_child(LayoutNode::row(0, 5)
                    .with(h_lbl_algo, Fixed(70))
                    .with(h_combo, Fixed(100))
                    .spacer(30)
                    .with(h_lbl_time, Fixed(90))
                    .with(h_hour, Fixed(30))
                    .with(h_sep, Fixed(10))
                    .with(h_min, Fixed(30))
                    .flex_spacer()
                )
                .with_child(LayoutNode::row(0, 5)
                     .with(h_lbl_days, Fixed(40))
                     .with_child(days_node)
                )
                .spacer(10)
                .with_child(LayoutNode::row(0, 0)
                     .with(h_btn_save, Fixed(80))
                     .flex_spacer() // Push buttons to edges or center? Standard is Left/Right or Right
                     // Original was: Save at padding (Left), Cancel at 490 (Rightish)
                     .with(h_btn_cancel, Fixed(80))
                )
                .apply_layout(client_rect);

            // Populate if Edit (Same logic, slightly adapted)
            if let Some(idx) = self.edit_index {
                let tasks = self.tasks.lock().unwrap();
                if let Some(task) = tasks.get(idx) {
                    SetWindowTextW(h_path, to_wstring(&task.get_path()).as_ptr());
                    
                    let algo_idx = match task.algorithm {
                        WofAlgorithm::Xpress4K => 0,
                        WofAlgorithm::Xpress8K => 1,
                        WofAlgorithm::Xpress16K => 2,
                        WofAlgorithm::Lzx => 3,
                    };
                    cb.set_selected_index(algo_idx);
                    
                    SetWindowTextW(h_hour, crate::utils::fmt_u32_padded(task.time_hour as u32).as_ptr());
                    SetWindowTextW(h_min, crate::utils::fmt_u32_padded(task.time_minute as u32).as_ptr());
                    
                    // Reset checks first
                    for (id, _) in days {
                         Button::new(GetDlgItem(hwnd, id as i32)).set_checked(false);
                    }

                    if (task.days_mask & 0x80) != 0 {
                        Button::new(GetDlgItem(hwnd, IDC_CHK_EVERYDAY as i32)).set_checked(true);
                    } else {
                         if task.days_mask & 1 != 0 { Button::new(GetDlgItem(hwnd, IDC_CHK_MON as i32)).set_checked(true); }
                         if task.days_mask & 2 != 0 { Button::new(GetDlgItem(hwnd, IDC_CHK_TUE as i32)).set_checked(true); }
                         if task.days_mask & 4 != 0 { Button::new(GetDlgItem(hwnd, IDC_CHK_WED as i32)).set_checked(true); }
                         if task.days_mask & 8 != 0 { Button::new(GetDlgItem(hwnd, IDC_CHK_THU as i32)).set_checked(true); }
                         if task.days_mask & 16 != 0 { Button::new(GetDlgItem(hwnd, IDC_CHK_FRI as i32)).set_checked(true); }
                         if task.days_mask & 32 != 0 { Button::new(GetDlgItem(hwnd, IDC_CHK_SAT as i32)).set_checked(true); }
                         if task.days_mask & 64 != 0 { Button::new(GetDlgItem(hwnd, IDC_CHK_SUN as i32)).set_checked(true); }
                    }
                }
            }

            crate::ui::theme::apply_theme_recursive(hwnd, self.is_dark);
        }
        0
    }

    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, _lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            match msg {
                WM_DROPFILES => {
                    let hdrop = wparam as HDROP;
                    let mut buffer = [0u16; 1024];
                    let len = DragQueryFileW(hdrop, 0, buffer.as_mut_ptr(), 1024);
                    if len > 0 {
                        let path = String::from_utf16_lossy(&buffer[..len as usize]);
                        let h_edit = GetDlgItem(hwnd, IDC_EDIT_PATH as i32);
                        if h_edit != std::ptr::null_mut() {
                            SetWindowTextW(h_edit, to_wstring(&path).as_ptr());
                        }
                    }
                    DragFinish(hdrop);
                },
                WM_COMMAND => {
                    let id = (wparam & 0xFFFF) as u16;
                    let code = ((wparam >> 16) & 0xFFFF) as u16;
                    
                    match id {
                        IDC_BTN_BROWSE => {
                             if code == BN_CLICKED as u16 {
                                 if let Ok(path) = crate::ui::file_dialog::pick_folder() {
                                     let h_edit = GetDlgItem(hwnd, IDC_EDIT_PATH as i32);
                                     if h_edit != std::ptr::null_mut() {
                                          SetWindowTextW(h_edit, to_wstring(&path).as_ptr());
                                     }
                                 }
                             }
                        },
                        IDC_BTN_BROWSE_FILE => {
                             if code == BN_CLICKED as u16 {
                                 if let Ok(paths) = crate::ui::file_dialog::pick_files() {
                                     if let Some(first) = paths.first() {
                                         let h_edit = GetDlgItem(hwnd, IDC_EDIT_PATH as i32);
                                         if h_edit != std::ptr::null_mut() {
                                              SetWindowTextW(h_edit, to_wstring(first).as_ptr());
                                         }
                                     }
                                 }
                             }
                        },
                        IDC_BTN_SAVE => {
                            if code == BN_CLICKED as u16 {
                                self.save_task(hwnd);
                            }
                        },
                        IDC_BTN_CANCEL => {
                            if code == BN_CLICKED as u16 {
                                DestroyWindow(hwnd);
                            }
                        },
                         IDC_CHK_EVERYDAY => {
                            if code == BN_CLICKED as u16 {
                                // Optional UX: disable other checkboxes
                            }
                        }
                        _ => {}
                    }
                },
                _ => {
                    return None;
                }
            }
        }
        Some(0)
    }
}

impl WatcherAddState {
    unsafe fn save_task(&mut self, hwnd: HWND) {
         // Get Path
         let h_path = GetDlgItem(hwnd, IDC_EDIT_PATH as i32);
         let len = GetWindowTextLengthW(h_path);
         if len == 0 {
             MessageBoxW(hwnd, w!("Please select a path.").as_ptr(), w!("Error").as_ptr(), MB_OK | MB_ICONERROR);
             return;
         }
         let mut buf = vec![0u16; (len + 1) as usize];
         GetWindowTextW(h_path, buf.as_mut_ptr(), len + 1);
         let path_str = String::from_utf16_lossy(&buf[..len as usize]);
         
         // Get Algo
         let h_combo = GetDlgItem(hwnd, IDC_COMBO_ALGO as i32);
         let idx = ComboBox::new(h_combo).get_selected_index();
         let algo = match idx {
             0 => WofAlgorithm::Xpress4K,
             1 => WofAlgorithm::Xpress8K,
             2 => WofAlgorithm::Xpress16K,
             3 => WofAlgorithm::Lzx,
             _ => WofAlgorithm::Xpress8K,
         };
         
         // Get Time
         let h_hr = GetDlgItem(hwnd, IDC_EDIT_HOUR as i32);
         let h_mn = GetDlgItem(hwnd, IDC_EDIT_MIN as i32);
         
         let get_val = |h| {
             let len = GetWindowTextLengthW(h);
             if len > 0 {
                 let mut b = vec![0u16; (len+1) as usize];
                 GetWindowTextW(h, b.as_mut_ptr(), len+1);
                 String::from_utf16_lossy(&b[..len as usize]).parse::<u8>().unwrap_or(0)
             } else { 0 }
         };
         
         let hr = get_val(h_hr);
         let mn = get_val(h_mn);
         
         if hr > 23 || mn > 59 {
              MessageBoxW(hwnd, w!("Invalid time.").as_ptr(), w!("Error").as_ptr(), MB_OK | MB_ICONERROR);
              return;
         }
         
         // Get Days
         let mut mask = 0u8;
         let check = |id| {
             let h = GetDlgItem(hwnd, id as i32);
             Button::new(h).is_checked()
         };
         
         if check(IDC_CHK_EVERYDAY) {
             mask = 0x80;
         } else {
             if check(IDC_CHK_MON) { mask |= 1; }
             if check(IDC_CHK_TUE) { mask |= 2; }
             if check(IDC_CHK_WED) { mask |= 4; }
             if check(IDC_CHK_THU) { mask |= 8; }
             if check(IDC_CHK_FRI) { mask |= 16; }
             if check(IDC_CHK_SAT) { mask |= 32; }
             if check(IDC_CHK_SUN) { mask |= 64; }
         }
         
         if mask == 0 {
              MessageBoxW(hwnd, w!("Please select at least one day.").as_ptr(), w!("Error").as_ptr(), MB_OK | MB_ICONERROR);
              return;
         }
         
         // Save
         {
             let mut tasks = self.tasks.lock().unwrap();
             if let Some(idx) = self.edit_index {
                 // Update existing
                 if let Some(task) = tasks.get_mut(idx) {
                     task.set_path(&path_str);
                     task.algorithm = algo;
                     task.days_mask = mask;
                     task.time_hour = hr;
                     task.time_minute = mn;
                 }
             } else {
                 // Add New
                 let new_id = tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
                 let task = WatcherTask::new(new_id, &path_str, algo, mask, hr, mn);
                 tasks.push(task);
             }
             let _ = WatcherConfig::save(&tasks);
         }
         
         DestroyWindow(hwnd);
    }
}
