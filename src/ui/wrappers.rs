#![allow(dead_code)]
use crate::types::*;
use crate::utils::to_wstring;


/// Safe wrapper for Button controls (including Checkboxes and RadioButtons)
#[derive(Clone, Copy)]
pub struct Button { hwnd: HWND }
impl Button {
    pub fn new(hwnd: HWND) -> Self { Self { hwnd } }
    
    pub fn is_checked(&self) -> bool { 
        unsafe { SendMessageW(self.hwnd, BM_GETCHECK, 0, 0) == 1 } 
    }
    
    pub fn set_checked(&self, checked: bool) { 
        unsafe { SendMessageW(self.hwnd, BM_SETCHECK, if checked { 1 } else { 0 }, 0); } 
    }
    
    pub fn set_enabled(&self, enabled: bool) { 
        unsafe { EnableWindow(self.hwnd, if enabled { 1 } else { 0 }); } 
    }
    
    pub fn set_text(&self, text: &str) { 
        let w = to_wstring(text);
        unsafe { SetWindowTextW(self.hwnd, w.as_ptr()); }
    }

    pub fn set_text_w(&self, text: &[u16]) {
        unsafe { SetWindowTextW(self.hwnd, text.as_ptr()); }
    }
}

/// Safe wrapper for ComboBox controls
#[derive(Clone, Copy)]
pub struct ComboBox { hwnd: HWND }
impl ComboBox {
    pub fn new(hwnd: HWND) -> Self { Self { hwnd } }
    
    pub fn get_selected_index(&self) -> i32 { 
        unsafe { SendMessageW(self.hwnd, CB_GETCURSEL, 0, 0) as i32 } 
    }
    
    pub fn set_selected_index(&self, index: i32) { 
        unsafe { SendMessageW(self.hwnd, CB_SETCURSEL, index as usize, 0); } 
    }
    
    pub fn add_string(&self, text: &str) {
        let w = to_wstring(text);
        unsafe { SendMessageW(self.hwnd, CB_ADDSTRING, 0, w.as_ptr() as isize); }
    }
    
    pub fn clear(&self) { 
        unsafe { SendMessageW(self.hwnd, CB_RESETCONTENT, 0, 0); } 
    }
    
    pub fn set_enabled(&self, enabled: bool) { 
        unsafe { EnableWindow(self.hwnd, if enabled { 1 } else { 0 }); } 
    }
}

/// Safe wrapper for Static/Label controls
#[derive(Clone, Copy)]
pub struct Label { hwnd: HWND }
impl Label {
    pub fn new(hwnd: HWND) -> Self { Self { hwnd } }
    
    pub fn set_text(&self, text: &str) {
        let w = to_wstring(text);
        unsafe { SetWindowTextW(self.hwnd, w.as_ptr()); }
    }
}

/// Safe wrapper for ProgressBar controls
#[derive(Clone, Copy)]
pub struct ProgressBar { hwnd: HWND }
impl ProgressBar {
    pub fn new(hwnd: HWND) -> Self { Self { hwnd } }
    
    pub fn set_range(&self, min: i32, max: i32) { 
        unsafe { SendMessageW(self.hwnd, PBM_SETRANGE32, min as usize, max as isize); } 
    }
    
    pub fn set_pos(&self, pos: i32) { 
        unsafe { SendMessageW(self.hwnd, PBM_SETPOS, pos as usize, 0); } 
    }
}

/// Safe wrapper for Trackbar (Slider) controls
#[derive(Clone, Copy)]
pub struct Trackbar { hwnd: HWND }
impl Trackbar {
    pub fn new(hwnd: HWND) -> Self { Self { hwnd } }
    
    pub fn set_range(&self, min: u32, max: u32) {
        // TBM_SETRANGE: WPARAM=Redraw(TRUE), LPARAM=LOWORD(Min)|HIWORD(Max)
        let lparam = (min & 0xFFFF) | ((max << 16) & 0xFFFF0000);
        unsafe { SendMessageW(self.hwnd, TBM_SETRANGE, 1, lparam as isize); }
    }
    
    pub fn set_pos(&self, pos: u32) {
        unsafe { SendMessageW(self.hwnd, TBM_SETPOS, 1, pos as isize); }
    }
    
    pub fn get_pos(&self) -> u32 {
        unsafe { SendMessageW(self.hwnd, TBM_GETPOS, 0, 0) as u32 }
    }
}
// Helper to get text from a window
pub fn get_window_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len > 0 {
            let mut buf = vec![0u16; (len + 1) as usize];
            let copied = GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
            if copied > 0 {
                return String::from_utf16_lossy(&buf[..copied as usize]);
            }
        }
        String::new()
    }

}
/// Safe wrapper for ListView controls
#[derive(Clone, Copy)]
pub struct ListView { hwnd: HWND }
impl ListView {
    pub fn new(hwnd: HWND) -> Self { Self { hwnd } }
    
    /// Clear all columns from the ListView
    pub fn clear_columns(&self) {
        const LVM_DELETECOLUMN: u32 = 0x101C;
        // Delete columns from the end to avoid index shifting issues
        loop {
            let result = unsafe { SendMessageW(self.hwnd, LVM_DELETECOLUMN, 0, 0) };
            if result == 0 { break; } // No more columns
        }
    }
    
    pub fn add_column(&self, index: i32, text: &str, width: i32) {
        let w_text = to_wstring(text);
        let mut col = unsafe { std::mem::zeroed::<LVCOLUMNW>() };
        col.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM | LVCF_FMT;
        col.fmt = LVCFMT_LEFT as i32;
        col.cx = width;
        col.pszText = w_text.as_ptr() as *mut _;
        col.iSubItem = index;
        
        unsafe { SendMessageW(self.hwnd, LVM_INSERTCOLUMNW, index as usize, &col as *const _ as isize); }
    }
    
    /// Set column width. Use width = -2 for LVSCW_AUTOSIZE_USEHEADER (fill remaining space)
    pub fn set_column_width(&self, index: i32, width: i32) {
        const LVM_SETCOLUMNWIDTH: u32 = 0x101E;
        unsafe { SendMessageW(self.hwnd, LVM_SETCOLUMNWIDTH, index as usize, width as isize); }
    }
    
    pub fn insert_item(&self, index: i32, text: &str, image_index: i32) {
        let w_text = to_wstring(text);
        let mut item = unsafe { std::mem::zeroed::<LVITEMW>() };
        item.mask = LVIF_TEXT | LVIF_IMAGE;
        item.iItem = index;
        item.iSubItem = 0;
        item.pszText = w_text.as_ptr() as *mut _;
        item.iImage = image_index;
        
        unsafe { SendMessageW(self.hwnd, LVM_INSERTITEMW, 0, &item as *const _ as isize); }
    }
    
    pub fn set_item_text(&self, index: i32, sub_index: i32, text: &str) {
        let w_text = to_wstring(text);
        let mut item = unsafe { std::mem::zeroed::<LVITEMW>() };
        item.mask = LVIF_TEXT;
        item.iItem = index;
        item.iSubItem = sub_index;
        item.pszText = w_text.as_ptr() as *mut _;
        
        unsafe { SendMessageW(self.hwnd, LVM_SETITEMW, 0, &item as *const _ as isize); }
    }
    
    pub fn clear(&self) {
        unsafe { SendMessageW(self.hwnd, LVM_DELETEALLITEMS, 0, 0); }
    }

    pub fn get_selection_mark(&self) -> i32 {
         unsafe { SendMessageW(self.hwnd, LVM_GETSELECTIONMARK, 0, 0) as i32 }
    }

    pub fn set_extended_style(&self, style: u32) {
        unsafe {
            SendMessageW(self.hwnd, LVM_SETEXTENDEDLISTVIEWSTYLE, 0, style as isize);
        }
    }

    pub fn apply_theme(&self, is_dark: bool) {
         unsafe { 
             crate::ui::theme::allow_dark_mode_for_window(self.hwnd, is_dark);

             // Apply theme (ItemsView/Explorer) matching FileListView logic
             if is_dark {
                 crate::ui::theme::apply_theme(self.hwnd, crate::ui::theme::ControlType::ItemsView, true);
             } else {
                 crate::ui::theme::apply_theme(self.hwnd, crate::ui::theme::ControlType::List, false);
             }
             
              // Apply theme colors
              
              let (bg, text) = if is_dark {
                  (crate::ui::theme::COLOR_LIST_BG_DARK, crate::ui::theme::COLOR_LIST_TEXT_DARK)
              } else {
                  (crate::ui::theme::COLOR_LIST_BG_LIGHT, crate::ui::theme::COLOR_LIST_TEXT_LIGHT)
              };
              
              SendMessageW(self.hwnd, LVM_SETBKCOLOR, 0, bg as isize);
              SendMessageW(self.hwnd, LVM_SETTEXTCOLOR, 0, text as isize);
              SendMessageW(self.hwnd, LVM_SETTEXTBKCOLOR, 0, bg as isize);
 
              // Also theme the header
              let h_header = SendMessageW(self.hwnd, LVM_GETHEADER, 0, 0) as HWND;
              if h_header != std::ptr::null_mut() {
                  crate::ui::theme::allow_dark_mode_for_window(h_header, is_dark);
                  crate::ui::theme::apply_theme(h_header, crate::ui::theme::ControlType::Header, is_dark);
                  InvalidateRect(h_header, std::ptr::null(), 1);
              }
          }
    }

    /// Installs a subclass to handle custom drawing for the header in dark mode.
    /// This is required because standard themes often fail to set white text for headers.
    pub fn fix_header_dark_mode(&self, parent_check_hwnd: HWND) {
        unsafe {
            let _ = SetWindowSubclass(
                self.hwnd,
                Some(header_subclass_proc),
                4242, // Subclass ID
                parent_check_hwnd as usize,
            );
        }
    }
}

/// Subclass procedure to force white text on ListView headers in dark mode.
unsafe extern "system" fn header_subclass_proc(
    hwnd: HWND,
    umsg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    dwrefdata: usize,
) -> LRESULT {
    unsafe {
        if umsg == WM_NOTIFY {
            let nmhdr = &*(lparam as *const NMHDR);
            
            if nmhdr.code == NM_CUSTOMDRAW {
                 // Check if it's from Header
                 let h_header = SendMessageW(hwnd, LVM_GETHEADER, 0, 0) as HWND;
                 if nmhdr.hwndFrom == h_header {
                     let _parent_hwnd = dwrefdata as HWND;
                     let is_dark = crate::ui::theme::is_system_dark_mode(); 
                     
                     if is_dark {
                         let nmcd = &mut *(lparam as *mut NMCUSTOMDRAW);
                         if nmcd.dwDrawStage == CDDS_PREPAINT {
                             return CDRF_NOTIFYITEMDRAW as LRESULT;
                         }
                         if nmcd.dwDrawStage == CDDS_ITEMPREPAINT {
                             SetTextColor(nmcd.hdc, crate::ui::theme::COLOR_HEADER_TEXT_DARK);
                             SetBkMode(nmcd.hdc, TRANSPARENT as i32);
                             return CDRF_NEWFONT as LRESULT;
                         }
                     }
                 }
            }
        }
        DefSubclassProc(hwnd, umsg, wparam, lparam)
    }
}
