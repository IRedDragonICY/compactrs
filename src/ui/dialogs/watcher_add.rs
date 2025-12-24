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
            crate::ui::theme::set_window_frame_theme(hwnd, self.is_dark);
            
            // Enable Drag and Drop (Allow messages from lower integrity levels)
            ChangeWindowMessageFilter(WM_DROPFILES, MSGFLT_ADD);
            ChangeWindowMessageFilter(WM_COPYGLOBALDATA, MSGFLT_ADD);
            DragAcceptFiles(hwnd, true as i32);
            
            let padding = 10;
            let mut y = padding;
            
            // Path
            ControlBuilder::new(hwnd, 0).label(false).text("Path:").pos(padding, y+3).size(50, 20).dark_mode(self.is_dark).build();
            let h_path = ControlBuilder::new(hwnd, IDC_EDIT_PATH).edit().pos(padding + 50, y).size(350, 25).dark_mode(self.is_dark).build();
            ControlBuilder::new(hwnd, IDC_BTN_BROWSE).button().text_w(w!("Folder")).pos(padding + 410, y).size(70, 25).dark_mode(self.is_dark).build();
            ControlBuilder::new(hwnd, IDC_BTN_BROWSE_FILE).button().text_w(w!("File")).pos(padding + 490, y).size(70, 25).dark_mode(self.is_dark).build();
            
            y += 35;
            
            // Algorithm & Time
            ControlBuilder::new(hwnd, 0).label(false).text("Algorithm:").pos(padding, y+3).size(70, 20).dark_mode(self.is_dark).build();
            let h_combo = ControlBuilder::new(hwnd, IDC_COMBO_ALGO).combobox().pos(padding + 80, y).size(100, 100).dark_mode(self.is_dark).build();
            let cb = ComboBox::new(h_combo);
            cb.add_string("XPRESS 4K");
            cb.add_string("XPRESS 8K");
            cb.add_string("XPRESS 16K");
            cb.add_string("LZX");
            cb.set_selected_index(1); // Default 8K

            ControlBuilder::new(hwnd, 0).label(false).text("Time (HH:MM):").pos(padding + 200, y+3).size(90, 20).dark_mode(self.is_dark).build();
            let h_hour = ControlBuilder::new(hwnd, IDC_EDIT_HOUR).edit().pos(padding + 290, y).size(30, 25).style(ES_NUMBER).dark_mode(self.is_dark).build();
            ControlBuilder::new(hwnd, 0).label(false).text(":").pos(padding + 325, y+3).size(10, 20).dark_mode(self.is_dark).build();
            let h_min = ControlBuilder::new(hwnd, IDC_EDIT_MIN).edit().pos(padding + 335, y).size(30, 25).style(ES_NUMBER).dark_mode(self.is_dark).build();
            
            y += 35;
            
            // Days
            ControlBuilder::new(hwnd, 0).label(false).text("Days:").pos(padding, y+3).size(40, 20).dark_mode(self.is_dark).build();
            
            let days = [
                (IDC_CHK_MON, "Mon"), (IDC_CHK_TUE, "Tue"), (IDC_CHK_WED, "Wed"),
                (IDC_CHK_THU, "Thu"), (IDC_CHK_FRI, "Fri"), (IDC_CHK_SAT, "Sat"),
                (IDC_CHK_SUN, "Sun"), (IDC_CHK_EVERYDAY, "Every Day")
            ];
            
            let mut x_off = padding + 50;
            for (id, txt) in days {
                ControlBuilder::new(hwnd, id).checkbox().text(txt).pos(x_off, y).size(if id == IDC_CHK_EVERYDAY { 90 } else { 55 }, 20).dark_mode(self.is_dark).checked(id == IDC_CHK_EVERYDAY).build();
                x_off += if id == IDC_CHK_EVERYDAY { 100 } else { 60 };
            }
            
            y += 35;
            
            // Buttons
            ControlBuilder::new(hwnd, IDC_BTN_SAVE).button().text_w(w!("Save")).pos(padding, y).size(80, 30).dark_mode(self.is_dark).build();
            ControlBuilder::new(hwnd, IDC_BTN_CANCEL).button().text_w(w!("Cancel")).pos(490, y).size(80, 30).dark_mode(self.is_dark).build();

            // Populate if Edit
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
