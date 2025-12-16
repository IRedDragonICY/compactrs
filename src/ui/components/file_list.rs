//! FileListView - A Facade for Win32 ListView control.
//!
//! Encapsulates raw Win32 API calls (`SendMessageW`, `LVM_*`) behind a clean,
//! high-level Rust interface. All unsafe Win32 operations are contained within
//! this module.

use windows::core::{w, Result, PWSTR};
use windows::Win32::Foundation::{HWND, HINSTANCE, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::SetWindowPos;

use super::base::Component;
use windows::Win32::Graphics::Gdi::{InvalidateRect, SetBkMode, SetTextColor, TRANSPARENT};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Controls::{
    LVM_DELETEITEM, LVM_GETHEADER, LVM_GETNEXTITEM, LVM_INSERTCOLUMNW,
    LVM_INSERTITEMW, LVM_SETBKCOLOR, LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETITEMW,
    LVM_SETTEXTBKCOLOR, LVM_SETTEXTCOLOR, LVCFMT_LEFT, LVCF_FMT, LVCF_TEXT, LVCF_WIDTH,
    LVCOLUMNW, LVIF_PARAM, LVIF_TEXT, LVITEMW, LVNI_SELECTED, LVS_EX_DOUBLEBUFFER,
    LVS_EX_FULLROWSELECT, LVS_REPORT, LVS_SHOWSELALWAYS, NM_CUSTOMDRAW, NMCUSTOMDRAW,
    NMHDR, SetWindowTheme, CDDS_ITEMPREPAINT, CDDS_PREPAINT,
    CDRF_NEWFONT, CDRF_NOTIFYITEMDRAW,
};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, HMENU, SendMessageW, WM_NOTIFY, WS_BORDER, WS_CHILD, WS_VISIBLE,
};

use crate::engine::wof::{WofAlgorithm, CompressionState};
use crate::ui::state::BatchItem;
use crate::ui::utils::ToWide;

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
        // SAFETY: GetModuleHandleW with None returns the current module handle, which is always valid.
        let module = GetModuleHandleW(None).unwrap();
        let instance = HINSTANCE(module.0);

        // SAFETY: CreateWindowExW is called with valid parameters. The parent HWND
        // is provided by the caller and must be valid.
        let hwnd = CreateWindowExW(
            Default::default(),
            w!("SysListView32"),
            None,
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                WS_VISIBLE.0 | WS_CHILD.0 | WS_BORDER.0 | LVS_REPORT as u32 | LVS_SHOWSELALWAYS as u32,
            ),
            x,
            y,
            w,
            h,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            Some(instance),
            None,
        )
        .unwrap_or_default();

        // SAFETY: SendMessageW with valid HWND and LVM_SETEXTENDEDLISTVIEWSTYLE message.
        SendMessageW(
            hwnd,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            Some(WPARAM(0)),
            Some(LPARAM((LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize)),
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
            (w!("Path"), 250),
            (w!("Current"), 70),
            (w!("Algorithm"), 70),
            (w!("Action"), 70),
            (w!("Size"), 75),
            (w!("On Disk"), 75),
            (w!("Progress"), 70),
            (w!("Status"), 80),
            (w!("▶ Start"), 45),
        ];

        for (i, (name, width)) in columns.iter().enumerate() {
            let col = LVCOLUMNW {
                mask: LVCF_WIDTH | LVCF_TEXT | LVCF_FMT,
                fmt: LVCFMT_LEFT,
                cx: *width,
                pszText: PWSTR(name.as_ptr() as *mut _),
                ..Default::default()
            };
            // SAFETY: SendMessageW with valid HWND and LVM_INSERTCOLUMNW message.
            // The column struct is valid for the duration of the call.
            unsafe {
                SendMessageW(
                    self.hwnd,
                    LVM_INSERTCOLUMNW,
                    Some(WPARAM(i)),
                    Some(LPARAM(&col as *const _ as isize)),
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
        let path_wide = item.path.to_wide();
        let algo_str = Self::algo_to_str(item.algorithm);
        let algo_wide = algo_str.to_wide();
        let action_str = if item.action == crate::ui::state::BatchAction::Compress {
            "Compress"
        } else {
            "Decompress"
        };
        let action_wide = action_str.to_wide();
        let size_wide = size_logical.to_wide();
        let disk_wide = size_disk.to_wide();

        // Format current state string
        let current_text = match state {
            CompressionState::None => "-".to_string(),
            CompressionState::Specific(algo) => Self::algo_to_str(algo).to_string(),
            CompressionState::Mixed => "Mixed".to_string(),
        };
        let current_wide = current_text.to_wide();

        // Show pending status initially
        let status_text = "Pending".to_string();
        let status_wide = status_text.to_wide();
        let start_wide = "▶".to_wide();

        // SAFETY: All wide strings are valid null-terminated UTF-16.
        // SendMessageW is called with valid HWND and message parameters.
        unsafe {
            // Insert main item (path column)
            let mut lvi = LVITEMW {
                mask: LVIF_TEXT | LVIF_PARAM,
                iItem: i32::MAX, // Append at end
                iSubItem: 0,
                pszText: PWSTR(path_wide.as_ptr() as *mut _),
                lParam: LPARAM(id as isize),
                ..Default::default()
            };
            let idx = SendMessageW(
                self.hwnd,
                LVM_INSERTITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&lvi as *const _ as isize)),
            );
            let row = idx.0 as i32;

            // Set subitems
            lvi.mask = LVIF_TEXT;
            lvi.iItem = row;

            // Col 1 = Current State
            lvi.iSubItem = columns::CURRENT;
            lvi.pszText = PWSTR(current_wide.as_ptr() as *mut _);
            SendMessageW(
                self.hwnd,
                LVM_SETITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&lvi as *const _ as isize)),
            );

            // Col 2 = Algorithm
            lvi.iSubItem = columns::ALGORITHM;
            lvi.pszText = PWSTR(algo_wide.as_ptr() as *mut _);
            SendMessageW(
                self.hwnd,
                LVM_SETITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&lvi as *const _ as isize)),
            );

            // Col 3 = Action
            lvi.iSubItem = columns::ACTION;
            lvi.pszText = PWSTR(action_wide.as_ptr() as *mut _);
            SendMessageW(
                self.hwnd,
                LVM_SETITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&lvi as *const _ as isize)),
            );

            // Col 4 = Size (logical/uncompressed)
            lvi.iSubItem = columns::SIZE;
            lvi.pszText = PWSTR(size_wide.as_ptr() as *mut _);
            SendMessageW(
                self.hwnd,
                LVM_SETITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&lvi as *const _ as isize)),
            );

            // Col 5 = On Disk (compressed size)
            lvi.iSubItem = columns::ON_DISK;
            lvi.pszText = PWSTR(disk_wide.as_ptr() as *mut _);
            SendMessageW(
                self.hwnd,
                LVM_SETITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&lvi as *const _ as isize)),
            );

            // Col 6 = Progress (empty initially)
            // Left empty

            // Col 7 = Status (shows Pending)
            lvi.iSubItem = columns::STATUS;
            lvi.pszText = PWSTR(status_wide.as_ptr() as *mut _);
            SendMessageW(
                self.hwnd,
                LVM_SETITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&lvi as *const _ as isize)),
            );

            // Col 8 = Start button
            lvi.iSubItem = columns::START;
            lvi.pszText = PWSTR(start_wide.as_ptr() as *mut _);
            SendMessageW(
                self.hwnd,
                LVM_SETITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&lvi as *const _ as isize)),
            );

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
        let text_wide = text.to_wide();

        let item = LVITEMW {
            mask: LVIF_TEXT,
            iItem: row,
            iSubItem: col,
            pszText: PWSTR(text_wide.as_ptr() as *mut _),
            ..Default::default()
        };

        // SAFETY: SendMessageW with valid HWND and LVM_SETITEMW message.
        // The item struct is valid for the duration of the call.
        unsafe {
            SendMessageW(
                self.hwnd,
                LVM_SETITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&item as *const _ as isize)),
            );
        }
    }

    /// Returns the indices of all selected items.
    pub fn get_selected_indices(&self) -> Vec<usize> {
        let mut selected = Vec::new();
        let mut item_idx: i32 = -1;

        // SAFETY: SendMessageW with valid HWND and LVM_GETNEXTITEM message.
        unsafe {
            loop {
                let start_param = if item_idx < 0 {
                    usize::MAX
                } else {
                    item_idx as usize
                };
                let next = SendMessageW(
                    self.hwnd,
                    LVM_GETNEXTITEM,
                    Some(WPARAM(start_param)),
                    Some(LPARAM(LVNI_SELECTED as isize)),
                );
                if next.0 < 0 {
                    break;
                }
                item_idx = next.0 as i32;
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
        // SAFETY: SendMessageW with valid HWND and LVM_DELETEITEM message.
        unsafe {
            SendMessageW(self.hwnd, LVM_DELETEITEM, Some(WPARAM(index as usize)), None);
        }
    }

    /// Applies theme colors and styles to the ListView.
    ///
    /// # Arguments
    /// * `is_dark` - Whether to apply dark mode theme
    pub fn set_theme(&self, is_dark: bool) {
        // SAFETY: All Win32 calls use valid HWND and message parameters.
        unsafe {
            Self::allow_dark_mode_for_window(self.hwnd, is_dark);

            // Apply dark mode explorer theme (affects header, scrollbars, etc.)
            if is_dark {
                let _ = SetWindowTheme(self.hwnd, w!("DarkMode_ItemsView"), None);
            } else {
                let _ = SetWindowTheme(self.hwnd, w!("Explorer"), None);
            }

            // Set colors
            let (bg_color, text_color) = if is_dark {
                (0x00202020u32, 0x00FFFFFFu32) // Dark gray bg, white text
            } else {
                (0x00FFFFFFu32, 0x00000000u32) // White bg, black text
            };

            SendMessageW(
                self.hwnd,
                LVM_SETBKCOLOR,
                Some(WPARAM(0)),
                Some(LPARAM(bg_color as isize)),
            );
            SendMessageW(
                self.hwnd,
                LVM_SETTEXTBKCOLOR,
                Some(WPARAM(0)),
                Some(LPARAM(bg_color as isize)),
            );
            SendMessageW(
                self.hwnd,
                LVM_SETTEXTCOLOR,
                Some(WPARAM(0)),
                Some(LPARAM(text_color as isize)),
            );

            // Get and theme the header control
            let header_result = SendMessageW(self.hwnd, LVM_GETHEADER, None, None);
            let header = HWND(header_result.0 as *mut _);

            if !header.is_invalid() {
                Self::allow_dark_mode_for_window(header, is_dark);
                if is_dark {
                    let _ = SetWindowTheme(header, w!("DarkMode_ItemsView"), None);
                } else {
                    let _ = SetWindowTheme(header, w!("Explorer"), None);
                }
            }

            // Force redraw
            let _ = InvalidateRect(Some(self.hwnd), None, true);
        }
    }

    /// Applies subclass to handle header theming for dark mode.
    ///
    /// # Arguments
    /// * `main_hwnd` - Main window handle (passed to subclass proc for theme checks)
    pub fn apply_subclass(&self, main_hwnd: HWND) {
        // SAFETY: SetWindowSubclass is called with valid HWND and callback.
        // The main_hwnd is passed as reference data for theme checking.
        unsafe {
            let _ = SetWindowSubclass(
                self.hwnd,
                Some(listview_subclass_proc),
                1, // Subclass ID
                main_hwnd.0 as usize,
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
        // SAFETY: LoadLibraryW and GetProcAddress are standard Win32 APIs.
        // Ordinal 133 is the undocumented AllowDarkModeForWindow function.
        if let Ok(uxtheme) = LoadLibraryW(w!("uxtheme.dll")) {
            if let Some(func) = GetProcAddress(uxtheme, windows::core::PCSTR(133 as *const u8)) {
                let allow_dark: extern "system" fn(HWND, bool) -> bool = std::mem::transmute(func);
                allow_dark(hwnd, allow);
            }
        }
    }}
}

impl Component for FileListView {
    /// FileListView is created in its constructor, so this just returns Ok.
    unsafe fn create(&mut self, _parent: HWND) -> Result<()> {
        // Already created in `new()` - nothing to do here
        Ok(())
    }

    fn hwnd(&self) -> Option<HWND> {
        Some(self.hwnd)
    }

    unsafe fn on_resize(&mut self, parent_rect: &RECT) {
        use windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER;
        
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
                None,
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
    let main_hwnd = HWND(dwrefdata as *mut _);

    if umsg == WM_NOTIFY && is_app_dark_mode(main_hwnd) {
        // SAFETY: lparam points to NMHDR struct provided by the system.
        let nmhdr = &*(lparam.0 as *const NMHDR);

        if nmhdr.code == NM_CUSTOMDRAW {
            // SAFETY: For NM_CUSTOMDRAW, lparam points to NMCUSTOMDRAW struct.
            let nmcd = &mut *(lparam.0 as *mut NMCUSTOMDRAW);

            if nmcd.dwDrawStage == CDDS_PREPAINT {
                // Request item-level notifications
                return LRESULT(CDRF_NOTIFYITEMDRAW as isize);
            }

            if nmcd.dwDrawStage == CDDS_ITEMPREPAINT {
                // Set text color to white for header items in dark mode
                SetTextColor(nmcd.hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF));
                SetBkMode(nmcd.hdc, TRANSPARENT);
                return LRESULT(CDRF_NEWFONT as isize);
            }
        }
    }

    // SAFETY: DefSubclassProc is called with valid parameters.
    DefSubclassProc(hwnd, umsg, wparam, lparam)
}}

/// Checks if the app is in dark mode.
///
/// This is a helper that accesses the main window's AppState to determine theme.
fn is_app_dark_mode(hwnd: HWND) -> bool {
    use crate::ui::state::{AppState, AppTheme};
    use crate::ui::utils::get_window_state;

    // SAFETY: get_window_state returns None if pointer is null.
    // The HWND is the main window which stores AppState in GWLP_USERDATA.
    unsafe {
        if let Some(st) = get_window_state::<AppState>(hwnd) {
            match st.theme {
                AppTheme::Dark => true,
                AppTheme::Light => false,
                AppTheme::System => crate::ui::theme::is_system_dark_mode(),
            }
        } else {
            // Fallback during initialization
            crate::ui::theme::is_system_dark_mode()
        }
    }
}

