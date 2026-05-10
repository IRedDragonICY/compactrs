#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::controls::apply_button_theme;
use crate::ui::builder::ControlBuilder;
use crate::ui::framework::{get_window_state, WindowHandler, WindowBuilder, WindowAlignment};
use crate::utils::to_wstring;
use crate::logger::LogEntry;
use crate::ui::state::AppState;
use crate::w;
use crate::types::*;
use std::collections::VecDeque;

const CONSOLE_TITLE: &str = "Debug Console";
const IDC_EDIT_CONSOLE: i32 = 1001;
const IDC_BTN_COPY: i32 = 1002;
const IDC_BTN_CLEAR: i32 = 1003;

const TIMER_ID: usize = 1;
const UPDATE_INTERVAL_MS: u32 = 150;
const MAX_HISTORY: usize = 1000;

struct ConsoleState {
    parent_state: *mut AppState, 
    list_hwnd: Option<HWND>,
    btn_copy_hwnd: Option<HWND>,
    btn_clear_hwnd: Option<HWND>,
    is_dark: bool,
    history: VecDeque<LogEntry>,
    pending: Vec<LogEntry>,
}

pub unsafe fn show_console_window(app_state: &mut AppState, parent: HWND, is_dark: bool) {
    if let Some(hwnd) = app_state.console_hwnd {
        // Update theme if window exists
        if let Some(state) = get_window_state::<ConsoleState>(hwnd) {
            state.is_dark = is_dark;
            state.update_theme(hwnd);
        }
        
        // If already exists, just show and focus
        ShowWindow(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd);
        BringWindowToTop(hwnd);
        return;
    }

    let bg_brush = (COLOR_WINDOW + 1) as HBRUSH;
    
    let mut history = VecDeque::with_capacity(MAX_HISTORY);
    history.extend(app_state.logs.iter().cloned());

    let state = Box::new(ConsoleState {
        parent_state: app_state as *mut _,
        list_hwnd: None,
        btn_copy_hwnd: None,
        btn_clear_hwnd: None,
        is_dark,
        history, 
        pending: Vec::new(), 
    });
    
    // Leak to get a 'static mutable reference
    let state_ref = Box::leak(state);

    let hwnd_res = WindowBuilder::new(state_ref, "CompactRS_Console", CONSOLE_TITLE)
        .style(WS_OVERLAPPEDWINDOW | WS_VISIBLE)
        .size(crate::ui::theme::scale(600), crate::ui::theme::scale(400))
        .align(WindowAlignment::Manual(CW_USEDEFAULT, CW_USEDEFAULT))
        .background(bg_brush)
        .build(parent);
        
    if let Ok(hwnd) = hwnd_res {
        if hwnd != std::ptr::null_mut() {
            app_state.console_hwnd = Some(hwnd);
        }
    }
}

impl ConsoleState {
    unsafe fn update_theme(&self, hwnd: HWND) {
        crate::ui::theme::set_window_frame_theme(hwnd, self.is_dark);
        if let Some(list) = self.list_hwnd {
             if self.is_dark {
                 let dark_mode = w!("DarkMode_Explorer");
                 SetWindowTheme(list, dark_mode.as_ptr(), std::ptr::null_mut());
                 SendMessageW(list, LVM_SETBKCOLOR, 0, crate::ui::theme::COLOR_LIST_BG_DARK as isize);
                 SendMessageW(list, LVM_SETTEXTBKCOLOR, 0, crate::ui::theme::COLOR_LIST_BG_DARK as isize);
                 SendMessageW(list, LVM_SETTEXTCOLOR, 0, crate::ui::theme::COLOR_LIST_TEXT_DARK as isize);
             } else {
                 let explorer = w!("Explorer");
                 SetWindowTheme(list, explorer.as_ptr(), std::ptr::null_mut());
                 SendMessageW(list, LVM_SETBKCOLOR, 0, crate::ui::theme::COLOR_LIST_BG_LIGHT as isize);
                 SendMessageW(list, LVM_SETTEXTBKCOLOR, 0, crate::ui::theme::COLOR_LIST_BG_LIGHT as isize);
                 SendMessageW(list, LVM_SETTEXTCOLOR, 0, crate::ui::theme::COLOR_LIST_TEXT_LIGHT as isize);
             }
             InvalidateRect(list, std::ptr::null_mut(), 1);
        }
        
        if let Some(btn) = self.btn_copy_hwnd {
            apply_button_theme(btn, self.is_dark);
        }
        if let Some(btn) = self.btn_clear_hwnd {
            apply_button_theme(btn, self.is_dark);
        }
        
        InvalidateRect(hwnd, std::ptr::null_mut(), 1);
    }

    /// Updates the list view count and scrolls to the bottom
    unsafe fn refresh_text(&self) {
        if let Some(list) = self.list_hwnd {
            SendMessageW(list, LVM_SETITEMCOUNT, self.history.len() as usize, 0);
            if !self.history.is_empty() {
                SendMessageW(list, LVM_ENSUREVISIBLE, (self.history.len() - 1) as usize, 0);
            }
            InvalidateRect(list, std::ptr::null_mut(), 0);
        }
    }
    
    unsafe fn flush_pending(&mut self) {
        if self.pending.is_empty() { return; }
        
        for entry in self.pending.drain(..) {
            if self.history.len() >= MAX_HISTORY {
                self.history.pop_front();
            }
            self.history.push_back(entry);
        }
        
        self.refresh_text();
    }

    unsafe fn do_layout(&mut self, _hwnd: HWND, rc: RECT) {
        if let (Some(list), Some(copy), Some(clear)) = (self.list_hwnd, self.btn_copy_hwnd, self.btn_clear_hwnd) {
             use crate::ui::layout::{LayoutNode, SizePolicy::{Fixed, Flex}};
             
             LayoutNode::col(0, 5) 
                 .with(list, Flex(1.0))
                 .with_child(LayoutNode::row(5, 5) 
                      .with_policy(Fixed(40)) 
                      .with(copy, Fixed(80))
                      .with(clear, Fixed(80))
                      .flex_spacer()
                 )
                 .apply_layout(rc);
        }
    }
}

unsafe fn format_log_entry(entry: &LogEntry) -> Vec<u16> {
    let ts_w = crate::utils::fmt_timestamp(entry.timestamp);
    
    let level_str = match entry.level {
        crate::logger::LogLevel::Error => w!(" [ERROR] "),
        crate::logger::LogLevel::Warning => w!(" [WARN] "),
        crate::logger::LogLevel::Info => w!(" [INFO] "),
        crate::logger::LogLevel::Trace => w!(" [TRACE] "),
    };
    
    let msg_w = to_wstring(&entry.message);
    crate::utils::concat_wstrings(&[&ts_w, level_str, &msg_w])
}

impl WindowHandler for ConsoleState {
    fn is_dark_mode(&self) -> bool {
        self.is_dark
    }

    fn is_modal(&self) -> bool {
        false
    }

    fn on_create(&mut self, hwnd: HWND) -> LRESULT {
        unsafe {
             let instance = GetModuleHandleW(std::ptr::null_mut());
             let list_cls = to_wstring("SysListView32");
             
             // Create Virtual ListView
             let lvs_nosortheader = 0x8000;
             let list = CreateWindowExW(
                 0,
                 list_cls.as_ptr(),
                 std::ptr::null_mut(),
                 WS_CHILD | WS_VISIBLE | WS_VSCROLL | WS_HSCROLL | 
                 (LVS_REPORT as u32) | (LVS_SINGLESEL as u32) | (LVS_OWNERDATA as u32) | lvs_nosortheader,
                 0, 0, 0, 0,
                 hwnd,
                 IDC_EDIT_CONSOLE as isize as HMENU,
                 instance,
                 std::ptr::null_mut()
             );
             
             SendMessageW(list, LVM_SETEXTENDEDLISTVIEWSTYLE, 0, (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize);
             
             let mut col: LVCOLUMNW = std::mem::zeroed();
             col.mask = LVCF_WIDTH | LVCF_FMT;
             col.fmt = LVCFMT_LEFT;
             col.cx = 2000; // Large width to prevent scrolling text clipping
             SendMessageW(list, LVM_INSERTCOLUMNW, 0, &col as *const _ as isize);

             self.list_hwnd = Some(list);
             
             // Create Buttons using ControlBuilder
             let builder = |id| ControlBuilder::new(hwnd, id).dark_mode(self.is_dark);
             let btn = |text, id| builder(id).button().text_w(&crate::utils::to_wstring(text)).build();
             
             self.btn_copy_hwnd = Some(btn("Copy", IDC_BTN_COPY as u16));
             self.btn_clear_hwnd = Some(btn("Clear", IDC_BTN_CLEAR as u16));
             
             self.update_theme(hwnd);
             
             // Initial render & Layout
             self.refresh_text();
             
             let rc = crate::utils::get_client_rect(hwnd);
             self.do_layout(hwnd, rc);
             
             // Start timer
             SetTimer(hwnd, TIMER_ID, UPDATE_INTERVAL_MS, None);
        }
        0
    }

    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            match msg {
                WM_TIMER => {
                    if wparam == TIMER_ID {
                        self.flush_pending();
                    }
                    Some(0)
                },
                WM_NOTIFY => {
                    let nmhdr = lparam as *const NMHDR;
                    if (*nmhdr).code == LVN_GETDISPINFOW {
                        let pdi = lparam as *mut NMLVDISPINFOW;
                        let row = (*pdi).item.iItem as usize;
                        if row < self.history.len() {
                            let entry = &self.history[row];
                            if ((*pdi).item.mask & LVIF_TEXT) != 0 {
                                let line = format_log_entry(entry);
                                let max_len = (*pdi).item.cchTextMax as usize;
                                if max_len > 0 {
                                    let copy_len = std::cmp::min(line.len(), max_len);
                                    std::ptr::copy_nonoverlapping(line.as_ptr(), (*pdi).item.pszText, copy_len);
                                    if copy_len > 0 {
                                        *((*pdi).item.pszText.add(copy_len - 1)) = 0;
                                    }
                                }
                            }
                        }
                        return Some(0);
                    }
                    None
                },
                WM_SIZE => {
                    let w = (lparam & 0xFFFF) as i32;
                    let h = ((lparam >> 16) & 0xFFFF) as i32;
                    let rc = RECT { left: 0, top: 0, right: w, bottom: h };
                    self.do_layout(hwnd, rc);
                    Some(0)
                },
                WM_COMMAND => {
                    let id = (wparam & 0xFFFF) as i32;
                    match id {
                        IDC_BTN_COPY => {
                            let mut total_len = 0;
                            let mut formatted_entries = Vec::with_capacity(self.history.len());
                            let newline = w!("\r\n");
                            let newline_len = newline.len() - 1; 
                            
                            for entry in &self.history {
                                let mut line = format_log_entry(entry);
                                if line.last() == Some(&0) {
                                    line.pop();
                                }
                                total_len += line.len() + newline_len;
                                formatted_entries.push(line);
                            }
                            total_len += 1; // final null terminator
                            
                            if total_len > 1 {
                                let mut buffer: Vec<u16> = Vec::with_capacity(total_len);
                                for line in formatted_entries {
                                    buffer.extend_from_slice(&line);
                                    buffer.extend_from_slice(&newline[..newline_len]);
                                }
                                buffer.push(0);
                                
                                if OpenClipboard(hwnd) != 0 {
                                    let _ = EmptyClipboard();
                                    let size = (buffer.len() * 2) as usize;
                                    let hmem = GlobalAlloc(GMEM_MOVEABLE, size);
                                    if hmem != std::ptr::null_mut() {
                                        let ptr = GlobalLock(hmem);
                                        if !ptr.is_null() {
                                            std::ptr::copy_nonoverlapping(buffer.as_ptr(), ptr as *mut u16, buffer.len());
                                            GlobalUnlock(hmem);
                                            SetClipboardData(13, hmem);
                                        }
                                    }
                                    CloseClipboard();
                                }
                            }
                        },
                        IDC_BTN_CLEAR => {
                            self.history.clear();
                            self.pending.clear();
                            
                            let app_state = &mut *self.parent_state;
                            app_state.logs.clear();
                            
                            self.refresh_text();
                        },
                        _ => {}
                    }
                    Some(0)
                },
                WM_DESTROY => {
                    KillTimer(hwnd, TIMER_ID);
                    let app_state = &mut *self.parent_state;
                    app_state.console_hwnd = None;
                    Some(0)
                },
                0x8002 => {
                    let new_is_dark = wparam == 1;
                    self.is_dark = new_is_dark;
                    self.update_theme(hwnd);
                    Some(0)
                },
                _ => None,
            }
        }
    }
}

pub unsafe fn append_log_entry(console_hwnd: Option<HWND>, entry: LogEntry) {
    if let Some(hwnd) = console_hwnd {
        if let Some(state) = get_window_state::<ConsoleState>(hwnd) {
            state.pending.push(entry);
        }
    }
}

pub unsafe fn close_console(hwnd: HWND) {
    DestroyWindow(hwnd);
}