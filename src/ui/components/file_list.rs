//! FileListView - A Facade for Win32 ListView control.
//!
//! Encapsulates raw Win32 API calls (`SendMessageW`, `LVM_*`) behind a clean,
//! high-level Rust interface. All unsafe Win32 operations are contained within
//! this module.

use crate::types::*;

use super::base::Component;
use crate::engine::wof::{WofAlgorithm, CompressionState};
use crate::ui::state::BatchItem;
use crate::utils::to_wstring;
use crate::w;





const DEFAULT_PATH_WIDTH: i32 = 250;

/// Define columns: (Name, Width)
const COLUMN_DEFS: &[(&str, i32)] = &[
    ("Path", DEFAULT_PATH_WIDTH),
    ("Current", 70),
    ("Algorithm", 70),
    ("Action", 70),
    ("Size", 75),
    ("Est. Size", 75),
    ("On Disk", 75),
    ("Ratio", 60),
    ("Progress", 70),
    ("Status", 80),
    ("▶ Start", 110),
];

/// Column indices for the FileListView
pub mod columns {
    pub const PATH: i32 = 0;
    pub const CURRENT: i32 = 1;
    pub const ALGORITHM: i32 = 2;
    pub const ACTION: i32 = 3;
    pub const SIZE: i32 = 4;
    pub const EST_SIZE: i32 = 5;
    pub const ON_DISK: i32 = 6;
    pub const RATIO: i32 = 7;
    pub const PROGRESS: i32 = 8;
    pub const STATUS: i32 = 9;
    pub const START: i32 = 10;
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
        let instance = GetModuleHandleW(std::ptr::null_mut());

        let class_name = w!("SysListView32");
        let empty_str = w!("");

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
            std::ptr::null_mut(),
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
        for (i, (name, width)) in COLUMN_DEFS.iter().enumerate() {
            let name_wide = to_wstring(name); // Names are static, but from tuple. Can we optimize? 
            // COLUMN_DEFS is string literal slices. 
            // We can leave to_wstring here as it runs once on startup.
            let mut fmt = LVCFMT_LEFT;
            // Start column (10) should be LEFT aligned to ensure hit testing works correctly (Eye icon at x=0)
            if i == columns::START as usize {
                fmt = LVCFMT_LEFT;
            }
            let col = LVCOLUMNW {
                mask: LVCF_WIDTH | LVCF_TEXT | LVCF_FMT,
                fmt,
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
    /// * `size_estimated` - Estimated compressed size string
    /// * `state` - Current compression state
    ///
    /// # Returns
    /// The row index where the item was inserted.
    pub fn add_item(
        &self,
        id: u32,
        item: &BatchItem,
        size_logical: &[u16],
        size_disk: &[u16],
        size_estimated: &[u16],
        state: CompressionState,
    ) -> i32 {
        let path_wide = to_wstring(&item.path);

        
        let algo_wide = match item.algorithm {
            WofAlgorithm::Xpress4K => w!("XPRESS4K"),
            WofAlgorithm::Xpress8K => w!("XPRESS8K"),
            WofAlgorithm::Xpress16K => w!("XPRESS16K"),
            WofAlgorithm::Lzx => w!("LZX"),
        };
        
        let action_wide = if item.action == crate::ui::state::BatchAction::Compress {
            w!("Compress")
        } else {
            w!("Decompress")
        };
        // size_logical, size_disk, size_estimated are already &[u16]
        let size_wide = size_logical;
        let disk_wide = size_disk;
        let est_wide = size_estimated;

        // Format current state string

        let current_wide: &[u16] = match state {
            CompressionState::None => w!("-"),
            CompressionState::Specific(algo) => match algo {
                WofAlgorithm::Xpress4K => w!("XPRESS4K"),
                WofAlgorithm::Xpress8K => w!("XPRESS8K"),
                WofAlgorithm::Xpress16K => w!("XPRESS16K"),
                WofAlgorithm::Lzx => w!("LZX"),
            },
            CompressionState::Legacy => w!("LZNT1"),
            CompressionState::Mixed => w!("Mixed"),
        };

        // Show pending status initially
        let status_wide = w!("Pending");
        // Initial State: Watch Icon + Play Button
        let start_wide_vec = crate::utils::to_wstring("\u{1F441}    ▶");
        let start_wide = &start_wide_vec;

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

            // Col 5 = Estimated Size
            lvi.iSubItem = columns::EST_SIZE;
            lvi.pszText = est_wide.as_ptr() as *mut _;
            SendMessageW(self.hwnd, LVM_SETITEMW, 0, &lvi as *const _ as isize);

            // Col 6 = On Disk (compressed size)
            lvi.iSubItem = columns::ON_DISK;
            lvi.pszText = disk_wide.as_ptr() as *mut _;
            SendMessageW(self.hwnd, LVM_SETITEMW, 0, &lvi as *const _ as isize);

            // Col 7 = Ratio
            // Calculate initial ratio (if logic/disk are available, else "-")
            // Note: pass strings usually, but here we might need to calculate if passed? 
            // The caller passes strings, but we can't parse them easily back to u64 here.
            // Let's rely on update or pass a default "-". 
            // BETTER: Use what we have. We'll update add_item signature in next step if needed, 
            // but for now let's just initialize with "-" and let subsequent updates handle it, 
            // OR we can try to use the passed strings if they are non-empty/valid?
            // Actually, the best way is to calculate it right here if we had the numbers. 
            // Since we don't have the raw numbers passed in (only formatted strings), 
            // let's initialize with "-" for now. Wait, `add_item` is called with `size_logical` etc as strings.
            // I'll update it to "-" initially.
            // Wait, the plan said "Update add_item signature OR initialize with -".
            // Let's initialize with "-" for now as it's safer without changing signature widely yet.
            // Actually, I can use the helper if I change the signature.
            // Let's stick to "-" for now to minimize signature churn, 
            // as `ingest_paths` calls this with "Calculating..." strings anyway.
            let ratio_wide = w!("-");
            lvi.iSubItem = columns::RATIO;
            lvi.pszText = ratio_wide.as_ptr() as *mut _;
            SendMessageW(self.hwnd, LVM_SETITEMW, 0, &lvi as *const _ as isize);

            // Col 8 = Progress
            lvi.iSubItem = columns::PROGRESS;
            // ... (rest shifted)
            // Wait, I need to match the previous indices? 
            // Previous: Progress was 7. Now it is 8.
            // So I need to set index to 8.
            
            // Col 8 = Progress (was 7)
            // We need to provide empty progress initially or what was passed?
            // `add_item` doesn't take progress string, it sets up defaults.
            // Actually `add_item` doesn't take progress. It inits with nothing?
            // Existing code:
            // // Col 8 = Status (shows Pending)  <-- This was comment for old col 8? No, old col 8 was STATUS.
            // Let's look at original code:
            // // Col 8 = Status (shows Pending)
            // lvi.iSubItem = columns::STATUS;
            // So Progress col (idx 7 old) was skipped? 
            // Let's check `setup_columns`:
            // 7: Progress, 8: Status.
            // Original `add_item` set `columns::STATUS` (which was 8). 
            // Did it set Progress? No, it seems it didn't set Progress initially (maybe empty default).
            
            // So for new code:
            // Ratio is 7. Progress is 8. Status is 9. Start is 10.
            
            lvi.iSubItem = columns::STATUS; // Now 9
            lvi.pszText = status_wide.as_ptr() as *mut _;
            SendMessageW(self.hwnd, LVM_SETITEMW, 0, &lvi as *const _ as isize);

            // Col 10 = Start button
            lvi.iSubItem = columns::START;
            lvi.pszText = start_wide.as_ptr() as *mut _;
            SendMessageW(self.hwnd, LVM_SETITEMW, 0, &lvi as *const _ as isize);

            row
        }
    }

    /// Finds the visual row index of an item by its ID (lParam).
    /// Returns None if not found (e.g., hidden by filter).
    pub fn find_item_by_id(&self, id: u32) -> Option<i32> {
        let mut find_info = LVFINDINFOW {
            flags: LVFI_PARAM,
            psz: std::ptr::null_mut(),
            lParam: id as isize,
            pt: POINT { x: 0, y: 0 },
            vkDirection: 0,
        };

        unsafe {
            let index = SendMessageW(
                self.hwnd,
                LVM_FINDITEMW,
                -1isize as usize, // Start search from beginning
                &mut find_info as *mut _ as isize,
            );

            if index >= 0 {
                Some(index as i32)
            } else {
                None
            }
        }
    }

    /// Updates the text of a specific cell.
    ///
    /// # Arguments
    /// * `row` - Row index (0-based)
    /// * `col` - Column index (use `columns::*` constants)
    /// * `text` - New text value
    pub fn update_item_text(&self, row: i32, col: i32, text: &[u16]) {
        // Ensure null-termination
        let text_wide = if text.last() == Some(&0) {
            std::borrow::Cow::Borrowed(text)
        } else {
            let mut t = text.to_vec();
            t.push(0);
            std::borrow::Cow::Owned(t)
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
            let _ = InvalidateRect(self.hwnd, std::ptr::null_mut(), 1);
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

    /// Deselects all items in the list.
    pub fn deselect_all(&self) {
        let selected = self.get_selected_indices();
        for idx in selected {
            self.set_selected(idx as i32, false);
        }
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

        const COLUMN_COUNT: i32 = 11; // Total columns defined in setup_columns

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


    // Local allow_dark_mode_for_window removed in favor of theme::allow_dark_mode_for_window

    /// Updates the playback controls (Start/Pause/Stop) for a row based on state.
    pub fn update_playback_controls(&self, row: i32, state: crate::ui::state::ProcessingState, is_complete: bool) {
        // Use to_wstring for combined strings
        // Format: [Watch]   [Playback...]
        // If complete: Only [Watch]
        // If complete: Show Watch + Start (to allow re-run)
        let text_wide = if is_complete {
             crate::utils::to_wstring("\u{1F441}    ▶")
        } else {
             match state {
                 crate::ui::state::ProcessingState::Idle | crate::ui::state::ProcessingState::Stopped => crate::utils::to_wstring("\u{1F441}    ▶"),
                 crate::ui::state::ProcessingState::Running => crate::utils::to_wstring("\u{1F441}    \u{23F8}   \u{23F9}"),
                 crate::ui::state::ProcessingState::Paused => crate::utils::to_wstring("\u{1F441}    \u{25B6}   \u{23F9}"),
             }
        };
        
        self.update_item_text(row, columns::START, &text_wide);
    }

    /// Updates the status text for a row.
    pub fn update_status_text(&self, row: i32, text: &str) {
        self.update_item_text(row, columns::STATUS, &to_wstring(text));
    }
    
    /// Updates the algorithm text for a row.
    pub fn update_algorithm(&self, row: i32, algo: WofAlgorithm) {
         let name = match algo {
            WofAlgorithm::Xpress4K => w!("XPRESS4K"),
            WofAlgorithm::Xpress8K => w!("XPRESS8K"),
            WofAlgorithm::Xpress16K => w!("XPRESS16K"),
            WofAlgorithm::Lzx => w!("LZX"),
        };
        self.update_item_text(row, columns::ALGORITHM, name);
    }

    /// Updates the action text for a row.
    pub fn update_action(&self, row: i32, action: crate::ui::state::BatchAction) {
        let name = match action {
            crate::ui::state::BatchAction::Compress => w!("Compress"),
            crate::ui::state::BatchAction::Decompress => w!("Decompress"),
        };
        self.update_item_text(row, columns::ACTION, name);
    }

    /// Gets the bounding rectangle of a subitem.
    pub fn get_subitem_rect(&self, row: i32, col: i32) -> RECT {
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        rect.top = col; // weird API: top holds the subitem index
        rect.left = LVIR_BOUNDS as i32;
        unsafe {
            SendMessageW(self.hwnd, LVM_GETSUBITEMRECT, row as usize, &mut rect as *mut _ as isize);
        }
        rect
    }

    /// Sets the font for the ListView.
    pub fn set_font(&self, hfont: crate::types::HFONT) {
        unsafe {
            SendMessageW(self.hwnd, WM_SETFONT, hfont as usize, 1);
        }
    }

    /// Updates column widths based on current window size.
    pub fn update_columns(&self) {
        unsafe {
             // 1. Calculate occupied width of fixed columns
             let mut occupied_width = 0;
             // We can assume column count or query it
             let col_count = COLUMN_DEFS.len();
             
             for i in 1..col_count {
                 let w = SendMessageW(self.hwnd, LVM_GETCOLUMNWIDTH, i, 0) as i32;
                 if w > 0 {
                     occupied_width += w;
                 }
             }

             // 2. Get client width
             let mut client_rect: RECT = std::mem::zeroed();
             GetClientRect(self.hwnd, &mut client_rect);
             let list_inner_width = client_rect.right - client_rect.left;

             // 3. Calculate Path width
             let margin = 0; // Exact fit
             let mut new_path_width = list_inner_width - occupied_width - margin;

             if new_path_width < 100 {
                 new_path_width = 100;
             }
             
             // 4. Set Path width
             SendMessageW(
                 self.hwnd,
                 LVM_SETCOLUMNWIDTH,
                 0, 
                 new_path_width as isize,
             );
        }
    }

    /// Removes all items from the ListView.
    pub fn clear_all(&self) {
        unsafe {
            SendMessageW(self.hwnd, LVM_DELETEALLITEMS, 0, 0);
        }
    }
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

    unsafe fn on_resize(&mut self, rect: &RECT) {
        unsafe {
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;

            // Use passed rect directly (Layout managed by parent)
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                rect.left,
                rect.top,
                width,
                height,
                SWP_NOZORDER,
            );

            // Re-use shared column blocking/layout logic
            self.update_columns();
        }
    }

    unsafe fn on_theme_change(&mut self, is_dark: bool) {
        self.set_theme(is_dark);
    }
}

// ============================================================================
// Custom Draw Color Tier System
// ============================================================================

/// Color tier for compression ratio display
#[derive(Clone, Copy)]
enum RatioTier {
    /// > 50% - Excellent compression
    Ultra,
    /// 20% - 50% - Good compression
    Good,
    /// 5% - 20% - Moderate compression
    Moderate,
    /// < 5% or negative - Negligible/failed compression
    Negligible,
}

/// Parses a percentage value from a UTF-16 buffer without heap allocation.
/// 
/// Handles formats like "42.5%", "-5.2%", and returns None for invalid/empty strings.
/// This is designed for hot-path usage in the draw loop.
#[inline]
fn parse_ratio_from_utf16(buffer: &[u16]) -> Option<f32> {
    // Find the end of the string (null terminator or buffer end)
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    if len == 0 {
        return None;
    }
    
    // Convert to ASCII for parsing (percentages are ASCII-compatible)
    // Stack buffer to avoid heap allocation
    let mut ascii_buf = [0u8; 32];
    let copy_len = len.min(31);
    
    for (i, &wc) in buffer[..copy_len].iter().enumerate() {
        // Only handle ASCII range (0-127)
        if wc > 127 {
            return None;
        }
        ascii_buf[i] = wc as u8;
    }
    
    // Parse the string, stripping '%' suffix if present
    let s = std::str::from_utf8(&ascii_buf[..copy_len]).ok()?;
    let s = s.trim();
    
    // Skip placeholder values
    if s == "-" || s.is_empty() || s == "N/A" {
        return None;
    }
    
    // Strip '%' suffix if present
    let s = s.strip_suffix('%').unwrap_or(s);
    
    // Parse as float
    s.trim().parse::<f32>().ok()
}

/// Determines the color tier based on the parsed ratio percentage.
#[inline]
fn get_ratio_tier(ratio: f32) -> RatioTier {
    if ratio > 50.0 {
        RatioTier::Ultra
    } else if ratio >= 20.0 {
        RatioTier::Good
    } else if ratio >= 5.0 {
        RatioTier::Moderate
    } else {
        RatioTier::Negligible
    }
}

/// Returns the appropriate COLORREF for the given tier and theme.
/// 
/// Colors are designed for optimal readability:
/// - Light mode: Darker, saturated colors for good contrast on white
/// - Dark mode: Brighter, vibrant colors for visibility on dark backgrounds
#[inline]
fn tier_to_color(tier: RatioTier, is_dark: bool) -> COLORREF {
    // COLORREF format: 0x00BBGGRR (BGR order)
    match (tier, is_dark) {
        // Ultra (> 50%): Greens
        (RatioTier::Ultra, false) => 0x0000A000,    // Light: Emerald Green RGB(0, 160, 0)
        (RatioTier::Ultra, true) => 0x0080FF00,     // Dark: Neon/Spring Green RGB(0, 255, 128)
        
        // Good (20% - 50%): Blues
        (RatioTier::Good, false) => 0x00C86400,     // Light: Teal/Sea Blue RGB(0, 100, 200)
        (RatioTier::Good, true) => 0x00FFC850,      // Dark: Cyan/Sky Blue RGB(80, 200, 255)
        
        // Moderate (5% - 20%): Oranges/Golds
        (RatioTier::Moderate, false) => 0x000064C8, // Light: Burnt Orange RGB(200, 100, 0)
        (RatioTier::Moderate, true) => 0x0000D7FF,  // Dark: Gold RGB(255, 215, 0)
        
        // Negligible (< 5% or negative): Grays
        (RatioTier::Negligible, false) => 0x00808080, // Light: Gray RGB(128, 128, 128)
        (RatioTier::Negligible, true) => 0x00A0A0A0,  // Dark: Dim Gray RGB(160, 160, 160)
    }
}

/// Retrieves the text of a ListView subitem into a stack-allocated buffer.
/// 
/// Returns the number of characters retrieved (excluding null terminator).
#[inline]
unsafe fn get_listview_item_text(hwnd: HWND, row: i32, col: i32, buffer: &mut [u16]) -> usize {
    let mut lvi = LVITEMW {
        mask: LVIF_TEXT,
        iItem: row,
        iSubItem: col,
        pszText: buffer.as_mut_ptr(),
        cchTextMax: buffer.len() as i32,
        ..Default::default()
    };
    
    // SAFETY: SendMessageW with LVM_GETITEMTEXTW requires valid HWND and LVITEMW pointer.
    // The buffer is valid for the duration of this call.
    let result = unsafe { SendMessageW(hwnd, LVM_GETITEMTEXTW, row as usize, &mut lvi as *mut _ as isize) };
    result as usize
}

/// Handles NM_CUSTOMDRAW for the ListView to color the Ratio column.
/// 
/// This function should be called from the main window procedure when receiving
/// WM_NOTIFY with NM_CUSTOMDRAW from the ListView control.
/// 
/// # Arguments
/// * `list_hwnd` - Handle to the ListView control
/// * `lparam` - The LPARAM from WM_NOTIFY (points to NMLVCUSTOMDRAW)
/// * `is_dark` - Whether dark mode is active
/// * `items` - The source list of BatchItems to derive data from
/// 
/// # Returns
/// * `Some(LRESULT)` - If we handled the draw stage, return this value
/// * `None` - Let default processing continue
/// 
/// # Safety
/// Caller must ensure lparam points to a valid NMLVCUSTOMDRAW structure.
pub unsafe fn handle_listview_customdraw(
    _list_hwnd: HWND, 
    lparam: LPARAM, 
    is_dark: bool,
    items: &[BatchItem]
) -> Option<LRESULT> {
    unsafe {
        // SAFETY: For ListView NM_CUSTOMDRAW, lparam points to NMLVCUSTOMDRAW
        let nmlvcd = &mut *(lparam as *mut NMLVCUSTOMDRAW);
        let draw_stage = nmlvcd.nmcd.dwDrawStage;

        // Stage 1: Pre-paint for entire control
        // Return CDRF_NOTIFYITEMDRAW to receive per-item notifications
        if draw_stage == CDDS_PREPAINT {
            return Some(CDRF_NOTIFYITEMDRAW as LRESULT);
        }

        // Stage 2: Item pre-paint (before drawing each row)
        // Return CDRF_NOTIFYSUBITEMDRAW to receive per-subitem (column) notifications
        if draw_stage == CDDS_ITEMPREPAINT {
            return Some(CDRF_NOTIFYSUBITEMDRAW as LRESULT);
        }

        // Stage 3: Subitem pre-paint (before drawing each cell)
        // This is where we customize the Ratio column color
        // Check if both CDDS_ITEM and CDDS_SUBITEM bits are set, and we are in PREPAINT
        // CDDS_ITEMPREPAINT is (CDDS_ITEM | CDDS_PREPAINT)
        // So we look for (CDDS_ITEM | CDDS_SUBITEM | CDDS_PREPAINT)
        if (draw_stage & (CDDS_ITEMPREPAINT | CDDS_SUBITEM)) == (CDDS_ITEMPREPAINT | CDDS_SUBITEM) {
            let sub_item = nmlvcd.i_sub_item;

            // Only customize the Ratio column (index 7)
            if sub_item == columns::RATIO {
                // Get ID from lParam (stored in add_item)
                // Note: NMCUSTOMDRAW.lItemlParam contains the item's lParam data
                let id = nmlvcd.nmcd.lItemlParam as u32;
                
                // Optimized Lookup: O(log N) using binary search (items are sorted by ID)
                // Fallback to row index only if ID lookup fails (e.g. invalid state)
                let item_opt = items.binary_search_by_key(&id, |i| i.id)
                    .ok()
                    .map(|idx| &items[idx])
                    .or_else(|| items.get(nmlvcd.nmcd.dwItemSpec as usize));

                if let Some(item) = item_opt {
                    // Use shared helper for consistency (DRY) and cast to f32 for tier logic
                    let ratio = crate::utils::calculate_saved_percentage(item.logical_size, item.disk_size) as f32;

                    let tier = get_ratio_tier(ratio);
                    let color = tier_to_color(tier, is_dark);

                    // Apply the color to the device context
                    SetTextColor(nmlvcd.nmcd.hdc, color);
                    SetBkMode(nmlvcd.nmcd.hdc, TRANSPARENT as i32);
                    
                    // Also update the struct fields, as some controls prefer this
                    nmlvcd.clr_text = color;

                    // CDRF_NEWFONT tells the control to use our color settings
                    return Some(CDRF_NEWFONT as LRESULT);
                }
            }


            // For non-Ratio columns or unparseable values, use default drawing
            // Reset text color to default to prevent leaking custom colors to subsequent columns
            let default_color = if is_dark {
                crate::ui::theme::COLOR_LIST_TEXT_DARK
            } else {
                crate::ui::theme::COLOR_LIST_TEXT_LIGHT
            };
            
            SetTextColor(nmlvcd.nmcd.hdc, default_color);
            nmlvcd.clr_text = default_color;

            // Return CDRF_NEWFONT to force the control to use our "reset" color
            return Some(CDRF_NEWFONT as LRESULT);
        }

        // Other draw stages - let default processing handle them
        None
    }
}

/// ListView subclass procedure to intercept NM_CUSTOMDRAW notifications.
///
/// This subclass handles:
/// 1. Header control custom draw for dark mode text coloring
/// 2. ListView item custom draw for Ratio column color tiers
///
/// The procedure distinguishes between notifications from the header control
/// (for column headers) and the ListView itself (for cell content).
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
    let is_dark = crate::ui::theme::is_app_dark_mode(main_hwnd);

    if umsg == WM_NOTIFY {
            // SAFETY: lparam points to NMHDR struct provided by the system.
            let nmhdr = &*(lparam as *const NMHDR);

            // Block manual column resizing (from header control)
            if nmhdr.code == HDN_BEGINTRACKW || nmhdr.code == HDN_DIVIDERDBLCLICKW {
                return 1; // Prevent resizing
            }

        if nmhdr.code == NM_CUSTOMDRAW {
            // Determine if this is from the header control or the ListView itself
            let header_hwnd = SendMessageW(hwnd, 0x101F, 0, 0) as HWND; // LVM_GETHEADER
            let is_from_header = nmhdr.hwndFrom == header_hwnd;

            if is_from_header {
                // ========================================================
                // Header Control Custom Draw (dark mode text)
                // ========================================================
                if is_dark {
                    let nmcd = &mut *(lparam as *mut NMCUSTOMDRAW);

                    if nmcd.dwDrawStage == CDDS_PREPAINT {
                        // Request item-level notifications for header items
                        return CDRF_NOTIFYITEMDRAW as LRESULT;
                    }

                    if nmcd.dwDrawStage == CDDS_ITEMPREPAINT {
                        // Set text color to white for header items in dark mode
                        SetTextColor(nmcd.hdc, crate::ui::theme::COLOR_HEADER_TEXT_DARK);
                        SetBkMode(nmcd.hdc, TRANSPARENT as i32);
                        return CDRF_NEWFONT as LRESULT;
                    }
                }
            } else if nmhdr.hwndFrom == hwnd {
                // ========================================================
                // ListView Custom Draw (Ratio column color tiers)
                // ========================================================
                // SAFETY: For ListView NM_CUSTOMDRAW, lparam points to NMLVCUSTOMDRAW
                let nmlvcd = &mut *(lparam as *mut NMLVCUSTOMDRAW);
                let draw_stage = nmlvcd.nmcd.dwDrawStage;

                if draw_stage == CDDS_PREPAINT {
                    // Request item-level notifications
                    return CDRF_NOTIFYITEMDRAW as LRESULT;
                }

                if draw_stage == CDDS_ITEMPREPAINT {
                    // Request subitem-level notifications for per-column coloring
                    return CDRF_NOTIFYSUBITEMDRAW as LRESULT;
                }

                // Handle subitem prepaint (specific column drawing)
                if draw_stage == (CDDS_ITEMPREPAINT | CDDS_SUBITEM) {
                    let sub_item = nmlvcd.i_sub_item;

                    // 1. Ratio Column (Color Tiers)
                    if sub_item == columns::RATIO {
                        let row = nmlvcd.nmcd.dwItemSpec as i32;

                        // Stack-allocated buffer for ratio text (e.g., "42.5%")
                        let mut text_buffer = [0u16; 16];
                        let _len = get_listview_item_text(hwnd, row, columns::RATIO, &mut text_buffer);

                        // Parse the ratio and determine color tier
                        if let Some(ratio) = parse_ratio_from_utf16(&text_buffer) {
                            let tier = get_ratio_tier(ratio);
                            let color = tier_to_color(tier, is_dark);

                            // Apply the color
                            SetTextColor(nmlvcd.nmcd.hdc, color);
                            SetBkMode(nmlvcd.nmcd.hdc, TRANSPARENT as i32);

                            // Return CDRF_NEWFONT to apply the color change
                            return CDRF_NEWFONT as LRESULT;
                        }
                    }

                    // For non-Custom columns or unparseable values, continue with default drawing
                    // Return CDRF_NEWFONT anyway to ensure proper rendering if we changed state
                    return CDRF_NEWFONT as LRESULT;
                }
            }
        }
    }

    // SAFETY: DefSubclassProc is called with valid parameters.
    DefSubclassProc(hwnd, umsg, wparam, lparam)
}}

