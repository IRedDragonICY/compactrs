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
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::Win32::UI::Controls::{
    LVM_DELETEITEM, LVM_GETHEADER, LVM_GETNEXTITEM, LVM_INSERTCOLUMNW,
    LVM_INSERTITEMW, LVM_SETBKCOLOR, LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETITEMW,
    LVM_SETTEXTBKCOLOR, LVM_SETTEXTCOLOR, LVCFMT_LEFT, LVCF_FMT, LVCF_TEXT, LVCF_WIDTH,
    LVCOLUMNW, LVIF_PARAM, LVIF_TEXT, LVITEMW, LVNI_SELECTED, LVS_EX_DOUBLEBUFFER,
    LVS_EX_FULLROWSELECT, LVS_REPORT, LVS_SHOWSELALWAYS, NM_CUSTOMDRAW, NMCUSTOMDRAW,
    NMHDR, SetWindowTheme, CDDS_ITEMPREPAINT, CDDS_PREPAINT,
    CDRF_NEWFONT, CDRF_NOTIFYITEMDRAW,
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
        size_logical: &str,
        size_disk: &str,
        state: CompressionState,
    ) -> i32 {
        let path_wide = to_wstring(&item.path); // Use helper directly
        let algo_str = Self::algo_to_str(item.algorithm);
        let algo_wide = to_wstring(algo_str);
        let action_str = if item.action == crate::ui::state::BatchAction::Compress {
            "Compress"
        } else {
            "Decompress"
        };
        let action_wide = to_wstring(action_str);
        let size_wide = to_wstring(size_logical);
        let disk_wide = to_wstring(size_disk);

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
    pub fn update_item_text(&self, row: i32, col: i32, text: &str) {
        let text_wide = to_wstring(text);

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
            Self::allow_dark_mode_for_window(self.hwnd, is_dark);

            // Apply dark mode explorer theme (affects header, scrollbars, etc.)
            let theme = if is_dark { to_wstring("DarkMode_ItemsView") } else { to_wstring("Explorer") };
            let _ = SetWindowTheme(self.hwnd, theme.as_ptr(), std::ptr::null());

            // Set colors
            let (bg_color, text_color) = if is_dark {
                (0x00202020u32, 0x00FFFFFFu32) // Dark gray bg, white text
            } else {
                (0x00FFFFFFu32, 0x00000000u32) // White bg, black text
            };

            SendMessageW(self.hwnd, LVM_SETBKCOLOR, 0, bg_color as isize);
            SendMessageW(self.hwnd, LVM_SETTEXTBKCOLOR, 0, bg_color as isize);
            SendMessageW(self.hwnd, LVM_SETTEXTCOLOR, 0, text_color as isize);

            // Get and theme the header control
            let header = SendMessageW(self.hwnd, LVM_GETHEADER, 0, 0) as HWND;

            if header != std::ptr::null_mut() {
                Self::allow_dark_mode_for_window(header, is_dark);
                let _ = SetWindowTheme(header, theme.as_ptr(), std::ptr::null());
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

    /// Helper to convert WofAlgorithm to string.
    fn algo_to_str(algo: WofAlgorithm) -> &'static str {
        match algo {
            WofAlgorithm::Xpress4K => "XPRESS4K",
            WofAlgorithm::Xpress8K => "XPRESS8K",
            WofAlgorithm::Xpress16K => "XPRESS16K",
            WofAlgorithm::Lzx => "LZX",
        }
    }

    /// Enables/disables dark mode for a window using undocumented uxtheme API.
    #[allow(non_snake_case)]
    unsafe fn allow_dark_mode_for_window(hwnd: HWND, allow: bool) { unsafe {
        let lib_name = to_wstring("uxtheme.dll");
        let uxtheme = LoadLibraryW(lib_name.as_ptr());
        if uxtheme != std::ptr::null_mut() {
            // Ordinal 133: AllowDarkModeForWindow
            if let Some(func) = GetProcAddress(uxtheme, 133 as *const u8) {
                 let allow_dark: extern "system" fn(HWND, bool) -> bool =
                     std::mem::transmute(func);
                 allow_dark(hwnd, allow);
            }
            // Note: We intentionally leak the library handle as we might need it later/throughout app life
            // or we could FreeLibrary(uxtheme) if we cache the function pointer? 
            // For themes, keeping uxtheme loaded is standard.
        }
    }}
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

            // Calculate list height: total height minus header, progress bar, buttons, and padding
            let list_height = height - header_height - progress_height - btn_height - (padding * 5);

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
                SetTextColor(nmcd.hdc, 0x00FFFFFF);
                SetBkMode(nmcd.hdc, TRANSPARENT as i32);
                return CDRF_NEWFONT as LRESULT;
            }
        }
    }

    // SAFETY: DefSubclassProc is called with valid parameters.
    DefSubclassProc(hwnd, umsg, wparam, lparam)
}}
