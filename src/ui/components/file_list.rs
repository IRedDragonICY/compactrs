//! FileListView - A Facade for Win32 ListView control.
//!
//! Encapsulates raw Win32 API calls (`SendMessageW`, `LVM_*`) behind a clean,
//! high-level Rust interface. All unsafe Win32 operations are contained within
//! this module.

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, SendMessageW, CreateWindowExW, WS_VISIBLE, WS_CHILD, WS_BORDER, WM_NOTIFY,
    SWP_NOZORDER, HMENU,
};
use windows_sys::Win32::Graphics::Gdi::{InvalidateRect, SetBkMode, SetTextColor, TRANSPARENT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{
    LVM_DELETEITEM, LVM_GETHEADER, LVM_GETNEXTITEM, LVM_INSERTCOLUMNW,
    LVM_INSERTITEMW, LVM_SETBKCOLOR, LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETITEMW,
    LVM_SETTEXTBKCOLOR, LVM_SETTEXTCOLOR, LVCFMT_LEFT, LVCF_FMT, LVCF_TEXT, LVCF_WIDTH,
    LVCOLUMNW, LVIF_PARAM, LVIF_TEXT, LVITEMW, LVNI_SELECTED, LVS_EX_DOUBLEBUFFER,
    LVS_EX_FULLROWSELECT, LVS_REPORT, LVS_SHOWSELALWAYS, NM_CUSTOMDRAW, NMCUSTOMDRAW,
    CDRF_NEWFONT, CDRF_NOTIFYITEMDRAW, CDDS_PREPAINT, CDDS_ITEMPREPAINT, NMHDR,
    LVM_GETITEMCOUNT, LVM_SETITEMSTATE, LVIS_SELECTED, LVM_SORTITEMS,
};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};

use super::base::Component;
use crate::engine::wof::{WofAlgorithm, CompressionState};
use crate::ui::state::BatchItem;
use crate::utils::to_wstring;

/// Column indices for the FileListView
pub mod columns {
    pub const PATH: i32 = 0;
    pub const CURRENT: i32 = 1;
    pub const ALGORITHM: i32 = 2;
    pub const ACTION: i32 = 3;
    pub const SIZE: i32 = 4;
    pub const ON_DISK: i32 = 5;
    pub const PROGRESS: i32 = 6;
    pub const STATUS: i32 = 7;
    pub const START: i32 = 8;
}

/// A high-level facade for the Win32 ListView control used to display batch items.
///
/// This struct encapsulates all raw Win32 API calls, providing a clean Rust interface.
/// The caller never needs to deal with `WPARAM`, `LPARAM`, or `SendMessageW` directly.
pub struct FileListView {
    hwnd: HWND,
}

impl FileListView {
    /// Creates a new FileListView control.
    ///
    /// # Arguments
    /// * `parent` - Parent window handle
    /// * `x`, `y`, `w`, `h` - Position and size
    /// * `id` - Control ID
    ///
    /// # Safety
    /// This function is unsafe because it calls Win32 APIs that require valid window handles.
    pub unsafe fn new(parent: HWND, x: i32, y: i32, w: i32, h: i32, id: u16) -> Self { unsafe {
        // SAFETY: GetModuleHandleW with null returns the current module handle.
        let instance = GetModuleHandleW(std::ptr::null());

        let class_name = to_wstring("SysListView32");
        let empty_str = to_wstring("");

        // SAFETY: CreateWindowExW is called with valid parameters.
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            empty_str.as_ptr(),
            WS_VISIBLE | WS_CHILD | WS_BORDER | LVS_REPORT | LVS_SHOWSELALWAYS,
            x,
            y,
            w,
            h,
            parent,
            id as usize as HMENU,
            instance,
            std::ptr::null(),
        );

        if hwnd == std::ptr::null_mut() {
            // In a real app we might want to handle this better, but panic is consistent with previous code
            // wrapped in Result in main builder sometimes, but here we return Self.
            // Using default/placeholder if failed? struct has just hwnd.
        }

        // SAFETY: SendMessageW with valid HWND and LVM_SETEXTENDEDLISTVIEWSTYLE message.
        SendMessageW(
            hwnd,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            0,
            (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize,
        );

        let file_list = Self { hwnd };

        // Setup columns
        file_list.setup_columns();

        file_list
    }}

    /// Returns the underlying HWND.
    #[inline]
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Sets up the ListView columns.
    fn setup_columns(&self) {
        // Columns: Path | Current | Algo | Action | Size | On Disk | Progress | Status | ▶ Start
        let columns = [
            ("Path", 250),
            ("Current", 70),
            ("Algorithm", 70),
            ("Action", 70),
            ("Size", 75),
            ("On Disk", 75),
            ("Progress", 70),
            ("Status", 80),
            ("▶ Start", 45),
        ];

        for (i, (name, width)) in columns.iter().enumerate() {
            let name_wide = to_wstring(name);
            let col = LVCOLUMNW {
                mask: LVCF_WIDTH | LVCF_TEXT | LVCF_FMT,
                fmt: LVCFMT_LEFT,
                cx: *width,
                pszText: name_wide.as_ptr() as *mut _,
                ..Default::default()
            };
            // SAFETY: SendMessageW with valid HWND and LVM_INSERTCOLUMNW message.
            unsafe {
                SendMessageW(
                    self.hwnd,
                    LVM_INSERTCOLUMNW,
                    i as usize,
                    &col as *const _ as isize,
                );
            }
        }
    }

    /// Adds a new item to the ListView.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the item (stored in lParam)
    /// * `item` - The BatchItem containing path, algorithm, action data
    /// * `size_logical` - Logical size string (e.g., "1.5 GiB")
    /// * `size_disk` - On-disk size string
    /// * `state` - Current compression state
    ///
    /// # Returns
    /// The row index where the item was inserted.
    pub fn add_item(
        &self,
        id: u32,
        item: &BatchItem,
        size_logical: Vec<u16>,
        size_disk: Vec<u16>,
        state: CompressionState,
    ) -> i32 {
        let path_wide = to_wstring(&item.path);
        let algo_str = Self::algo_to_str(item.algorithm);
        let algo_wide = to_wstring(algo_str);
        let action_str = if item.action == crate::ui::state::BatchAction::Compress {
            "Compress"
        } else {
            "Decompress"
        };
        let action_wide = to_wstring(action_str);
        // size_logical and size_disk are already Vec<u16>
        let size_wide = size_logical;
        let disk_wide = size_disk;

        // Format current state string
        let current_text = match state {
            CompressionState::None => "-".to_string(),
            CompressionState::Specific(algo) => Self::algo_to_str(algo).to_string(),
            CompressionState::Mixed => "Mixed".to_string(),
        };
        let current_wide = to_wstring(&current_text);

        // Show pending status initially
        let status_wide = to_wstring("Pending");
        let start_wide = to_wstring("▶");

        // SAFETY: All wide strings are valid null-terminated UTF-16.
        unsafe {
            // Insert main item (path column)
            let mut lvi = LVITEMW {
                mask: LVIF_TEXT | LVIF_PARAM,
                iItem: std::i32::MAX, // Append at end
                iSubItem: 0,
                pszText: path_wide.as_ptr() as *mut _,
                lParam: id as isize,
                ..Default::default()
            };
            let idx = SendMessageW(
                self.hwnd,
                LVM_INSERTITEMW,
                0,
                &lvi as *const _ as isize,
            );
            let row = idx as i32;

            // Set subitems
            lvi.mask = LVIF_TEXT;
            lvi.iItem = row;

            // Helper macro for settings subitems to avoid repetition
            // ... explicit calls are fine for now

            // Col 1 = Current State
            lvi.iSubItem = columns::CURRENT;
            lvi.pszText = current_wide.as_ptr() as *mut _;
            SendMessageW(self.hwnd, LVM_SETITEMW, 0, &lvi as *const _ as isize);

            // Col 2 = Algorithm
            lvi.iSubItem = columns::ALGORITHM;
            lvi.pszText = algo_wide.as_ptr() as *mut _;
            SendMessageW(self.hwnd, LVM_SETITEMW, 0, &lvi as *const _ as isize);

            // Col 3 = Action
            lvi.iSubItem = columns::ACTION;
            lvi.pszText = action_wide.as_ptr() as *mut _;
            SendMessageW(self.hwnd, LVM_SETITEMW, 0, &lvi as *const _ as isize);

            // Col 4 = Size (logical/uncompressed)
            lvi.iSubItem = columns::SIZE;
            lvi.pszText = size_wide.as_ptr() as *mut _;
            SendMessageW(self.hwnd, LVM_SETITEMW, 0, &lvi as *const _ as isize);

            // Col 5 = On Disk (compressed size)
            lvi.iSubItem = columns::ON_DISK;
            lvi.pszText = disk_wide.as_ptr() as *mut _;
            SendMessageW(self.hwnd, LVM_SETITEMW, 0, &lvi as *const _ as isize);

            // Col 7 = Status (shows Pending)
            lvi.iSubItem = columns::STATUS;
            lvi.pszText = status_wide.as_ptr() as *mut _;
            SendMessageW(self.hwnd, LVM_SETITEMW, 0, &lvi as *const _ as isize);

            // Col 8 = Start button
            lvi.iSubItem = columns::START;
            lvi.pszText = start_wide.as_ptr() as *mut _;
            SendMessageW(self.hwnd, LVM_SETITEMW, 0, &lvi as *const _ as isize);

            row
        }
    }

    /// Updates the text of a specific cell.
    ///
    /// # Arguments
    /// * `row` - Row index (0-based)
    /// * `col` - Column index (use `columns::*` constants)
    /// * `text` - New text value
    pub fn update_item_text(&self, row: i32, col: i32, text: Vec<u16>) {
        // Ensure null-termination
        let text_wide = if text.last() == Some(&0) {
            text
        } else {
            let mut t = text;
            t.push(0);
            t
        };

        let item = LVITEMW {
            mask: LVIF_TEXT,
            iItem: row,
            iSubItem: col,
            pszText: text_wide.as_ptr() as *mut _,
            ..Default::default()
        };

        unsafe {
            SendMessageW(
                self.hwnd,
                LVM_SETITEMW,
                0,
                &item as *const _ as isize,
            );
        }
    }

    /// Returns the indices of all selected items.
    pub fn get_selected_indices(&self) -> Vec<usize> {
        let mut selected = Vec::new();
        let mut item_idx: i32 = -1;

        unsafe {
            loop {
                let start_param = item_idx as usize;
                let next = SendMessageW(
                    self.hwnd,
                    LVM_GETNEXTITEM,
                    if item_idx < 0 { usize::MAX } else { start_param },
                    LVNI_SELECTED as isize,
                );
                if (next as i32) < 0 {
                    break;
                }
                item_idx = next as i32;
                selected.push(item_idx as usize);
            }
        }

        selected
    }

    /// Removes an item at the specified index.
    ///
    /// # Arguments
    /// * `index` - Row index to remove (0-based)
    pub fn remove_item(&self, index: i32) {
        unsafe {
            SendMessageW(self.hwnd, LVM_DELETEITEM, index as usize, 0);
        }
    }

    /// Applies theme colors and styles to the ListView.
    ///
    /// # Arguments
    /// * `is_dark` - Whether to apply dark mode theme
    pub fn set_theme(&self, is_dark: bool) {
        unsafe {
            crate::ui::theme::allow_dark_mode_for_window(self.hwnd, is_dark);

            // Apply theme (ItemsView/Explorer)
            if is_dark {
                crate::ui::theme::apply_theme(self.hwnd, crate::ui::theme::ControlType::ItemsView, true);
            } else {
                crate::ui::theme::apply_theme(self.hwnd, crate::ui::theme::ControlType::List, false);
            }

            // Set colors
            let (bg_color, text_color) = if is_dark {
                (crate::ui::theme::COLOR_LIST_BG_DARK, crate::ui::theme::COLOR_LIST_TEXT_DARK)
            } else {
                (crate::ui::theme::COLOR_LIST_BG_LIGHT, crate::ui::theme::COLOR_LIST_TEXT_LIGHT)
            };

            SendMessageW(self.hwnd, LVM_SETBKCOLOR, 0, bg_color as isize);
            SendMessageW(self.hwnd, LVM_SETTEXTBKCOLOR, 0, bg_color as isize);
            SendMessageW(self.hwnd, LVM_SETTEXTCOLOR, 0, text_color as isize);

            // Get and theme the header control
            let header = SendMessageW(self.hwnd, LVM_GETHEADER, 0, 0) as HWND;

            if header != std::ptr::null_mut() {
                crate::ui::theme::allow_dark_mode_for_window(header, is_dark);
                crate::ui::theme::apply_theme(header, crate::ui::theme::ControlType::Header, is_dark);
            }

            // Force redraw
            let _ = InvalidateRect(self.hwnd, std::ptr::null(), 1);
        }
    }

    /// Applies subclass to handle header theming for dark mode.
    ///
    /// # Arguments
    /// * `main_hwnd` - Main window handle (passed to subclass proc for theme checks)
    pub fn apply_subclass(&self, main_hwnd: HWND) {
        unsafe {
            let _ = SetWindowSubclass(
                self.hwnd,
                Some(listview_subclass_proc),
                1, // Subclass ID
                main_hwnd as usize,
            );
        }
    }

    /// Gets the selection count.
    pub fn get_selection_count(&self) -> usize {
        self.get_selected_indices().len()
    }

    /// Gets total item count.
    pub fn get_item_count(&self) -> i32 {
        unsafe { SendMessageW(self.hwnd, LVM_GETITEMCOUNT, 0, 0) as i32 }
    }

    /// Sets the selection state for an item.
    ///
    /// # Arguments
    /// * `index` - Row index
    /// * `selected` - True to select, False to deselect
    pub fn set_selected(&self, index: i32, selected: bool) {
        let state = if selected { LVIS_SELECTED } else { 0 };
        let mask = LVIS_SELECTED;
        let mut item = LVITEMW {
            state,
            stateMask: mask,
            ..Default::default()
        };
        unsafe {
            SendMessageW(self.hwnd, LVM_SETITEMSTATE, index as usize, &mut item as *mut _ as isize);
        }
    }

    /// Sorts the items in the ListView.
    ///
    /// # Arguments
    /// * `callback` - The comparison function.
    /// * `context` - User-defined value passed to the callback (pointer to AppState).
    pub fn sort_items(&self, callback: unsafe extern "system" fn(isize, isize, isize) -> i32, context: isize) {
        unsafe {
            SendMessageW(self.hwnd, LVM_SORTITEMS, context as usize, callback as isize);
        }
    }

    /// Sets the sort indicator (up/down arrow) on a column header.
    ///
    /// # Arguments
    /// * `column_index` - The column to show the indicator on
    /// * `ascending` - True for up arrow (HDF_SORTUP), false for down arrow (HDF_SORTDOWN)
    ///
    /// This clears sort indicators from all other columns to ensure only
    /// the active sort column displays an arrow.
    pub fn set_sort_indicator(&self, column_index: i32, ascending: bool) {
        // Win32 Header control constants
        const LVM_GETHEADER_MSG: u32 = 0x1000 + 31;
        const HDM_GETITEMW: u32 = 0x1200 + 11;
        const HDM_SETITEMW: u32 = 0x1200 + 12;
        const HDI_FORMAT: u32 = 0x0004;
        const HDF_SORTUP: i32 = 0x0400;
        const HDF_SORTDOWN: i32 = 0x0200;

        // HDITEMW struct layout for header item manipulation
        #[repr(C)]
        struct HDITEMW {
            mask: u32,
            cxy: i32,
            psz_text: *mut u16,
            hbm: isize,
            cch_text_max: i32,
            fmt: i32,
            l_param: isize,
            i_image: i32,
            i_order: i32,
            type_: u32,
            pv_filter: *mut std::ffi::c_void,
            state: u32,
        }

        const COLUMN_COUNT: i32 = 9; // Total columns defined in setup_columns

        unsafe {
            // Get the header control handle from the ListView
            let header = SendMessageW(self.hwnd, LVM_GETHEADER_MSG, 0, 0) as HWND;
            if header.is_null() {
                return;
            }

            for i in 0..COLUMN_COUNT {
                // Initialize HDITEMW to retrieve current format
                let mut hd_item: HDITEMW = std::mem::zeroed();
                hd_item.mask = HDI_FORMAT;

                // Get current item state
                let result = SendMessageW(
                    header,
                    HDM_GETITEMW,
                    i as usize,
                    &mut hd_item as *mut _ as isize,
                );

                if result == 0 {
                    continue; // Failed to get item, skip
                }

                // Clear existing sort flags
                hd_item.fmt &= !(HDF_SORTUP | HDF_SORTDOWN);

                // Apply sort flag only to the target column
                if i == column_index {
                    hd_item.fmt |= if ascending { HDF_SORTUP } else { HDF_SORTDOWN };
                }

                // Apply the updated format
                SendMessageW(
                    header,
                    HDM_SETITEMW,
                    i as usize,
                    &hd_item as *const _ as isize,
                );
            }
        }
    }

    /// Helper to convert WofAlgorithm to string.
    fn algo_to_str(algo: WofAlgorithm) -> &'static str {
        match algo {
            WofAlgorithm::Xpress4K => "XPRESS4K",
            WofAlgorithm::Xpress8K => "XPRESS8K",
            WofAlgorithm::Xpress16K => "XPRESS16K",
            WofAlgorithm::Lzx => "LZX",
        }
    }

    // Local allow_dark_mode_for_window removed in favor of theme::allow_dark_mode_for_window

}

impl Component for FileListView {
    /// FileListView is created in its constructor, so this just returns Ok.
    unsafe fn create(&mut self, _parent: HWND) -> Result<(), String> {
        // Already created in `new()` - nothing to do here
        Ok(())
    }

    fn hwnd(&self) -> Option<HWND> {
        Some(self.hwnd)
    }

    unsafe fn on_resize(&mut self, parent_rect: &RECT) {
        unsafe {
            let width = parent_rect.right - parent_rect.left;
            let height = parent_rect.bottom - parent_rect.top;

            let padding = 10;
            let header_height = 25;
            let progress_height = 25;
            let btn_height = 30;
            let lbl_height = 18;  // Space for labels above action dropdowns

            // Calculate list height: total height minus header, progress bar, buttons, labels, and padding
            let list_height = height - header_height - progress_height - btn_height - lbl_height - (padding * 5);

            // Position ListView below header
            let list_y = padding + header_height + padding;

            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                padding,
                list_y,
                width - padding * 2,
                list_height,
                SWP_NOZORDER,
            );
        }
    }

    unsafe fn on_theme_change(&mut self, is_dark: bool) {
        self.set_theme(is_dark);
    }
}

/// ListView subclass procedure to intercept Header's NM_CUSTOMDRAW notifications.
///
/// Header sends NM_CUSTOMDRAW to its parent (ListView), not grandparent (main window).
/// This subclass handles custom drawing for dark mode header text.
///
/// # Safety
/// This is a Win32 callback. The parameters are provided by the system.
unsafe extern "system" fn listview_subclass_proc(
    hwnd: HWND,
    umsg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    dwrefdata: usize,
) -> LRESULT { unsafe {
    // SAFETY: dwrefdata contains the main window HWND passed during subclass setup.
    let main_hwnd = dwrefdata as HWND;

    if umsg == WM_NOTIFY && crate::ui::theme::is_app_dark_mode(main_hwnd) {
        // SAFETY: lparam points to NMHDR struct provided by the system.
        let nmhdr = &*(lparam as *const NMHDR);

        if nmhdr.code == NM_CUSTOMDRAW {
            // SAFETY: For NM_CUSTOMDRAW, lparam points to NMCUSTOMDRAW struct.
            let nmcd = &mut *(lparam as *mut NMCUSTOMDRAW);

            if nmcd.dwDrawStage == CDDS_PREPAINT {
                // Request item-level notifications
                return CDRF_NOTIFYITEMDRAW as LRESULT;
            }

            if nmcd.dwDrawStage == CDDS_ITEMPREPAINT {
                // Set text color to white for header items in dark mode
                SetTextColor(nmcd.hdc, crate::ui::theme::COLOR_HEADER_TEXT_DARK);
                SetBkMode(nmcd.hdc, TRANSPARENT as i32);
                return CDRF_NEWFONT as LRESULT;
            }
        }
    }

    // SAFETY: DefSubclassProc is called with valid parameters.
    DefSubclassProc(hwnd, umsg, wparam, lparam)
}}
