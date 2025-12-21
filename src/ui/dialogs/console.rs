#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::controls::apply_button_theme;
use crate::ui::builder::ButtonBuilder;
use crate::ui::framework::{get_window_state, WindowHandler, WindowBuilder, WindowAlignment};
use crate::utils::to_wstring;
use crate::logger::LogEntry;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::InvalidateRect;

use windows_sys::Win32::UI::WindowsAndMessaging::{
    WM_CTLCOLOREDIT, WM_COMMAND,
    CreateWindowExW, DestroyWindow, 
    ShowWindow, SetForegroundWindow, BringWindowToTop,
    CW_USEDEFAULT, SW_RESTORE, WM_DESTROY,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE, WM_SIZE, SetWindowPos, SWP_NOZORDER,
    WS_CHILD, WS_VSCROLL, ES_MULTILINE, ES_READONLY, ES_AUTOVSCROLL,
    SendMessageW, GetWindowTextLengthW, GetWindowTextW, SetWindowTextW,
    HMENU, WM_TIMER, SetTimer, KillTimer,
};
use windows_sys::Win32::UI::Controls::{EM_SETSEL, EM_REPLACESEL, SetWindowTheme, EM_SETLIMITTEXT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::DataExchange::{OpenClipboard, CloseClipboard, EmptyClipboard, SetClipboardData};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH};
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

// Registry to track singleton instance
static mut CONSOLE_HWND: Option<HWND> = None;

struct ConsoleState {
    edit_hwnd: Option<HWND>,
    btn_copy_hwnd: Option<HWND>,
    btn_clear_hwnd: Option<HWND>,
    is_dark: bool,
    history: VecDeque<LogEntry>,
    pending: Vec<LogEntry>,
}

pub unsafe fn show_console_window(parent: HWND, initial_logs: &VecDeque<LogEntry>, is_dark: bool) {
    if let Some(hwnd) = CONSOLE_HWND {
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
    history.extend(initial_logs.iter().cloned());

    let state = Box::new(ConsoleState {
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
            CONSOLE_HWND = Some(hwnd);
        }
    }
}

impl ConsoleState {
    unsafe fn update_theme(&self, hwnd: HWND) {
        crate::ui::theme::set_window_frame_theme(hwnd, self.is_dark);
        if let Some(edit) = self.edit_hwnd {
             if self.is_dark {
                 let dark_mode = to_wstring("DarkMode_Explorer");
                 SetWindowTheme(edit, dark_mode.as_ptr(), std::ptr::null());
             } else {
                 let explorer = to_wstring("Explorer");
                 SetWindowTheme(edit, explorer.as_ptr(), std::ptr::null());
             }
             InvalidateRect(edit, std::ptr::null(), 1);
        }
        
        if let Some(btn) = self.btn_copy_hwnd {
            apply_button_theme(btn, self.is_dark);
        }
        if let Some(btn) = self.btn_clear_hwnd {
            apply_button_theme(btn, self.is_dark);
        }
        
        InvalidateRect(hwnd, std::ptr::null(), 1);
    }

    /// Re-renders the entire history to the edit control
    unsafe fn refresh_text(&self) {
        if let Some(edit) = self.edit_hwnd {
            let mut combined = String::with_capacity(self.history.len() * 100);
            for entry in &self.history {
                combined.push_str(&format_log_entry(entry));
            }
            let text_wide = to_wstring(&combined);
            SetWindowTextW(edit, text_wide.as_ptr());
            // Scroll to bottom
            let len = GetWindowTextLengthW(edit);
            SendMessageW(edit, EM_SETSEL, len as WPARAM, len as LPARAM);
            SendMessageW(edit, EM_REPLACESEL, 0, to_wstring("").as_ptr() as LPARAM);
        }
    }
    
    unsafe fn flush_pending(&mut self) {
        if self.pending.is_empty() { return; }
        
        if let Some(edit) = self.edit_hwnd {
            let mut chunk = String::with_capacity(self.pending.len() * 100);
            for entry in &self.pending {
                // Add to history
                if self.history.len() >= MAX_HISTORY {
                    self.history.pop_front();
                }
                self.history.push_back(entry.clone());
                
                // Format for display
                chunk.push_str(&format_log_entry(entry));
            }
            self.pending.clear();
            
            // Append to Edit
            let len = GetWindowTextLengthW(edit);
            SendMessageW(edit, EM_SETSEL, len as WPARAM, len as LPARAM);
            
            let chunk_wide = to_wstring(&chunk);
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

fn format_log_entry(entry: &LogEntry) -> String {
    // [HH:MM:SS] [LEVEL] Message
    // Simple UTC extraction from unix timestamp
    let s = entry.timestamp % 86400;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let s = s % 60;
    
    format!("[{:02}:{:02}:{:02}] [{}] {}\r\n", h, m, s, entry.level.as_str(), entry.message)
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
             let instance = GetModuleHandleW(std::ptr::null());
             let edit_cls = to_wstring("EDIT");
             
             // Create Edit Control
             let edit = CreateWindowExW(
                 0,
                 edit_cls.as_ptr(),
                 std::ptr::null(),
                 WS_CHILD | WS_VISIBLE | WS_VSCROLL | 
                 (ES_MULTILINE as u32) | (ES_READONLY as u32) | (ES_AUTOVSCROLL as u32),
                 0, 0, 0, 0,
                 hwnd,
                 IDC_EDIT_CONSOLE as isize as HMENU,
                 instance,
                 std::ptr::null()
             );
             
             SendMessageW(edit, EM_SETLIMITTEXT, EDIT_LIMIT, 0);
             
             self.edit_hwnd = Some(edit);
             
             // Create Buttons
             let btn_copy = ButtonBuilder::new(hwnd, IDC_BTN_COPY as u16)
                 .text("Copy").pos(0, 0).size(80, BUTTON_HEIGHT).dark_mode(self.is_dark).build();
             self.btn_copy_hwnd = Some(btn_copy);
             
             let btn_clear = ButtonBuilder::new(hwnd, IDC_BTN_CLEAR as u16)
                 .text("Clear").pos(90, 0).size(80, BUTTON_HEIGHT).dark_mode(self.is_dark).build();
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
                            if let Some(edit) = self.edit_hwnd {
                                let empty = to_wstring("");
                                SetWindowTextW(edit, empty.as_ptr());
                            }
                        },
                        _ => {}
                    }
                    Some(0)
                },
                WM_DESTROY => {
                    KillTimer(hwnd, TIMER_ID);
                    CONSOLE_HWND = None;
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

pub unsafe fn append_log_entry(entry: LogEntry) {
    if let Some(hwnd) = CONSOLE_HWND {
        if let Some(state) = get_window_state::<ConsoleState>(hwnd) {
            state.pending.push(entry);
        }
    }
}

pub unsafe fn close_console() {
    if let Some(hwnd) = CONSOLE_HWND {
        DestroyWindow(hwnd);
        CONSOLE_HWND = None;
    }
}
