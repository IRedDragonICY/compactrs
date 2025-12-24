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
const BUTTON_HEIGHT: i32 = 30;
const TIMER_ID: usize = 1;
const UPDATE_INTERVAL_MS: u32 = 150;
const MAX_HISTORY: usize = 1000;
// Win32 Edit Control Limit (64KB is default, let's bump to 1MB to be safe, but we truncate manually too)
const EDIT_LIMIT: usize = 1024 * 1024; 

struct ConsoleState {
    parent_state: *mut AppState, // Pointer back to AppState if needed, but risky. For now avoiding it.
    edit_hwnd: Option<HWND>,
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
        edit_hwnd: None,
        btn_copy_hwnd: None,
        btn_clear_hwnd: None,
        is_dark,
        history, 
        pending: Vec::new(), // Initial logs go into history, which we will force render on create
    });
    
    // Leak to get a 'static mutable reference
    let state_ref = Box::leak(state);

    let hwnd_res = WindowBuilder::new(state_ref, "CompactRS_Console", CONSOLE_TITLE)
        .style(WS_OVERLAPPEDWINDOW | WS_VISIBLE)
        .size(600, 400)
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
        if let Some(edit) = self.edit_hwnd {
             if self.is_dark {
                 let dark_mode = w!("DarkMode_Explorer");
                 SetWindowTheme(edit, dark_mode.as_ptr(), std::ptr::null_mut());
             } else {
                 let explorer = w!("Explorer");
                 SetWindowTheme(edit, explorer.as_ptr(), std::ptr::null_mut());
             }
             InvalidateRect(edit, std::ptr::null_mut(), 1);
        }
        
        if let Some(btn) = self.btn_copy_hwnd {
            apply_button_theme(btn, self.is_dark);
        }
        if let Some(btn) = self.btn_clear_hwnd {
            apply_button_theme(btn, self.is_dark);
        }
        
        InvalidateRect(hwnd, std::ptr::null_mut(), 1);
    }

    /// Re-renders the entire history to the edit control
    unsafe fn refresh_text(&self) {
        if let Some(edit) = self.edit_hwnd {
            // Use concat_wstrings or PathBuffer logic. We don't have a giant buffer helper exposed easily,
            // but we can just use a Vec<u16> builder.
            let total_len = self.history.len() * 100; // rough guess
            let mut combined = crate::utils::PathBuffer::with_capacity(total_len);
            
            for entry in &self.history {
                let line = format_log_entry(entry);
                combined.push_u16_slice(&line);
            }
            // PathBuffer pushes nulls and path separators. Not ideal for generic text.
            // We should use a simple Vec<u16> concatenation loop.
            
            let mut buf = Vec::with_capacity(total_len);
            for entry in &self.history {
                 let line = format_log_entry(entry);
                 // line is null terminated? concat_wstrings strips it.
                 // format_log_entry returns a Vec<u16> from concat_wstrings, so it HAS a null terminator.
                 // We should strip it.
                 let len = if line.last() == Some(&0) { line.len() - 1 } else { line.len() };
                 buf.extend_from_slice(&line[..len]);
            }
            buf.push(0);
            
            SetWindowTextW(edit, buf.as_ptr());
            // Scroll to bottom
            let len = GetWindowTextLengthW(edit);
            SendMessageW(edit, EM_SETSEL, len as WPARAM, len as LPARAM);
            SendMessageW(edit, EM_REPLACESEL, 0, w!("").as_ptr() as LPARAM);
        }
    }
    
    unsafe fn flush_pending(&mut self) {
        if self.pending.is_empty() { return; }
        
        if let Some(edit) = self.edit_hwnd {
            let mut chunk_wide = Vec::with_capacity(self.pending.len() * 100);
            
            for entry in &self.pending {
                // Add to history
                if self.history.len() >= MAX_HISTORY {
                    self.history.pop_front();
                }
                self.history.push_back(entry.clone());
                
                // Format for display
                let line = format_log_entry(entry);
                let len = if line.last() == Some(&0) { line.len() - 1 } else { line.len() };
                chunk_wide.extend_from_slice(&line[..len]);
            }
            chunk_wide.push(0);
            self.pending.clear();
            
            // Append to Edit
            let len = GetWindowTextLengthW(edit);
            SendMessageW(edit, EM_SETSEL, len as WPARAM, len as LPARAM);
            
            SendMessageW(edit, EM_REPLACESEL, 0, chunk_wide.as_ptr() as LPARAM);
            
            // Check limit and truncation
            if self.history.len() >= MAX_HISTORY {
                 // Simplistic check: If text length is huge, re-render from history to prune old text
                 if len > (EDIT_LIMIT as i32) - 10000 {
                     self.refresh_text();
                 }
            }
        }
    }
}

unsafe fn format_log_entry(entry: &LogEntry) -> Vec<u16> {
    // [HH:MM:SS] [LEVEL] Message\r\n
    let ts_w = crate::utils::fmt_timestamp(entry.timestamp);
    
    // Level
    // We could optimize this by returning static w-slices or using a small map
    let level_str = match entry.level {
        crate::logger::LogLevel::Error => w!(" [ERROR] "),
        crate::logger::LogLevel::Warning => w!(" [WARN] "),
        crate::logger::LogLevel::Info => w!(" [INFO] "),
        crate::logger::LogLevel::Trace => w!(" [TRACE] "),
    };
    
    let msg_w = to_wstring(&entry.message);
    let newline = w!("\r\n");
    
    crate::utils::concat_wstrings(&[&ts_w, level_str, &msg_w, newline])
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
             let edit_cls = to_wstring("EDIT");
             
             // Create Edit Control
             let edit = CreateWindowExW(
                 0,
                 edit_cls.as_ptr(),
                 std::ptr::null_mut(),
                 WS_CHILD | WS_VISIBLE | WS_VSCROLL | 
                 (ES_MULTILINE as u32) | (ES_READONLY as u32) | (ES_AUTOVSCROLL as u32),
                 0, 0, 0, 0,
                 hwnd,
                 IDC_EDIT_CONSOLE as isize as HMENU,
                 instance,
                 std::ptr::null_mut()
             );
             
             SendMessageW(edit, EM_SETLIMITTEXT, EDIT_LIMIT, 0);
             
             self.edit_hwnd = Some(edit);
             
             // Create Buttons
             let btn_copy = ControlBuilder::new(hwnd, IDC_BTN_COPY as u16)
                 .text_w(w!("Copy")).pos(0, 0).size(80, BUTTON_HEIGHT).dark_mode(self.is_dark).build();
             self.btn_copy_hwnd = Some(btn_copy);
             
             let btn_clear = ControlBuilder::new(hwnd, IDC_BTN_CLEAR as u16)
                 .text_w(w!("Clear")).pos(90, 0).size(80, BUTTON_HEIGHT).dark_mode(self.is_dark).build();
             self.btn_clear_hwnd = Some(btn_clear);
             
             self.update_theme(hwnd);
             
             // Initial render of history
             self.refresh_text();
             
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
                WM_SIZE => {
                    let width = (lparam & 0xFFFF) as i32;
                    let height = ((lparam >> 16) & 0xFFFF) as i32;
                    
                    if let Some(edit) = self.edit_hwnd {
                        SetWindowPos(edit, std::ptr::null_mut(), 0, 0, width, height - BUTTON_HEIGHT - 5, SWP_NOZORDER);
                    }
                    
                    let btn_y = height - BUTTON_HEIGHT;
                    if let Some(btn) = self.btn_copy_hwnd {
                        SetWindowPos(btn, std::ptr::null_mut(), 5, btn_y, 80, BUTTON_HEIGHT - 5, SWP_NOZORDER);
                    }
                    if let Some(btn) = self.btn_clear_hwnd {
                        SetWindowPos(btn, std::ptr::null_mut(), 90, btn_y, 80, BUTTON_HEIGHT - 5, SWP_NOZORDER);
                    }
                    Some(0)
                },
                WM_COMMAND => {
                    let id = (wparam & 0xFFFF) as i32;
                    match id {
                        IDC_BTN_COPY => {
                            if let Some(edit) = self.edit_hwnd {
                                let len = GetWindowTextLengthW(edit);
                                if len > 0 {
                                    let mut buffer: Vec<u16> = vec![0; (len + 1) as usize];
                                    GetWindowTextW(edit, buffer.as_mut_ptr(), len + 1);
                                    
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
                            }
                        },
                        IDC_BTN_CLEAR => {
                            // Clear history and edit
                            self.history.clear();
                            self.pending.clear();
                            
                            // CRITICAL: Clear global app logs too, otherwise reopening console will restore them!
                            let app_state = &mut *self.parent_state;
                            app_state.logs.clear();
                            
                            if let Some(edit) = self.edit_hwnd {
                                let empty = w!("");
                                SetWindowTextW(edit, empty.as_ptr());
                            }
                        },
                        _ => {}
                    }
                    Some(0)
                },
                WM_DESTROY => {
                    KillTimer(hwnd, TIMER_ID);
                    // Safe cleanup: Clear the HWND in parent state
                    let app_state = &mut *self.parent_state;
                    app_state.console_hwnd = None;
                    Some(0)
                },
                WM_CTLCOLOREDIT => None,
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
