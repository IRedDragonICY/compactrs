//! FileListView - A Facade for Win32 ListView control.
//!
//! Encapsulates raw Win32 API calls (`SendMessageW`, `LVM_*`) behind a clean,
//! high-level Rust interface. All unsafe Win32 operations are contained within
//! this module.

use crate::types::*;

use super::base::Component;
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

/// A high-level facade for the Win32 Virtual ListView control used to display batch items.
pub struct FileListView {
    hwnd: HWND,
    /// Handle to the "empty state" label overlay shown when no items are present.
    hwnd_empty_label: HWND,
}

impl FileListView {
    /// Creates a new FileListView control.
    pub unsafe fn new(parent: HWND, x: i32, y: i32, w: i32, h: i32, id: u16) -> Self { unsafe {
        let instance = GetModuleHandleW(std::ptr::null_mut());

        let class_name = w!("SysListView32");
        let empty_str = w!("");

        // Added LVS_OWNERDATA for Virtual List View
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            empty_str.as_ptr(),
            WS_VISIBLE | WS_CHILD | LVS_REPORT | LVS_SHOWSELALWAYS | LVS_OWNERDATA,
            x,
            y,
            w,
            h,
            parent,
            id as usize as HMENU,
            instance,
            std::ptr::null_mut(),
        );

        SendMessageW(
            hwnd,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            0,
            (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize,
        );

        let static_class = w!("Static");
        let empty_label_text = w!("Drop files here or click 'Add Folder' to start");
        let hwnd_empty_label = CreateWindowExW(
            0,
            static_class.as_ptr(),
            empty_label_text.as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_CENTER,
            0, 0, 300, 20,
            parent,
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        );

        if hwnd_empty_label != std::ptr::null_mut() {
            let font = crate::ui::theme::get_app_font();
            SendMessageW(hwnd_empty_label, WM_SETFONT, font as usize, 1);
        }

        let file_list = Self { hwnd, hwnd_empty_label };

        file_list.setup_columns();
        file_list
    }}

    #[inline]
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    fn setup_columns(&self) {
        for (i, (name, width)) in COLUMN_DEFS.iter().enumerate() {
            let name_wide = to_wstring(name);
            let mut fmt = LVCFMT_LEFT;
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

    pub fn set_item_count(&self, count: i32) {
        unsafe {
            SendMessageW(self.hwnd, LVM_SETITEMCOUNT, count as usize, LVSICF_NOINVALIDATEALL as isize);
            InvalidateRect(self.hwnd, std::ptr::null_mut(), 1);
            self.update_empty_state();
        }
    }

    pub fn redraw_item(&self, row: i32) {
        unsafe {
            SendMessageW(self.hwnd, LVM_REDRAWITEMS, row as usize, row as isize);
            UpdateWindow(self.hwnd);
        }
    }

    pub fn redraw_all(&self) {
        let count = self.get_item_count();
        if count > 0 {
            unsafe {
                SendMessageW(self.hwnd, LVM_REDRAWITEMS, 0, (count - 1) as isize);
                UpdateWindow(self.hwnd);
            }
        }
    }

    pub fn get_selected_indices(&self) -> Vec<usize> {
        let mut selected = Vec::new();
        let mut item_idx: i32 = -1;

        unsafe {
            loop {
                let start_param = if item_idx < 0 { -1isize as usize } else { item_idx as usize };
                let next = SendMessageW(
                    self.hwnd,
                    LVM_GETNEXTITEM,
                    start_param,
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

    pub fn set_theme(&self, is_dark: bool) {
        unsafe {
            crate::ui::theme::apply_flat_listview_theme(self.hwnd, is_dark);
            let _ = InvalidateRect(self.hwnd, std::ptr::null_mut(), 1);
        }
    }

    pub fn apply_subclass(&self, main_hwnd: HWND) {
        unsafe {
            let _ = SetWindowSubclass(
                self.hwnd,
                Some(listview_subclass_proc),
                1,
                main_hwnd as usize,
            );
        }
    }

    pub fn get_selection_count(&self) -> usize {
        self.get_selected_indices().len()
    }

    pub fn get_item_count(&self) -> i32 {
        unsafe { SendMessageW(self.hwnd, LVM_GETITEMCOUNT, 0, 0) as i32 }
    }

    pub fn deselect_all(&self) {
        let selected = self.get_selected_indices();
        for idx in selected {
            self.set_selected(idx as i32, false);
        }
    }

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

    pub fn set_sort_indicator(&self, column_index: i32, ascending: bool) {
        unsafe {
            let header = SendMessageW(self.hwnd, LVM_GETHEADER, 0, 0) as HWND;
            if header.is_null() {
                return;
            }

            for i in 0..11 {
                let mut hd_item: HDITEMW = std::mem::zeroed();
                hd_item.mask = HDI_FORMAT;

                let result = SendMessageW(
                    header,
                    HDM_GETITEMW,
                    i as usize,
                    &mut hd_item as *mut _ as isize,
                );

                if result == 0 {
                    continue;
                }

                hd_item.fmt &= !(HDF_SORTUP | HDF_SORTDOWN);

                if i == column_index {
                    hd_item.fmt |= if ascending { HDF_SORTUP } else { HDF_SORTDOWN };
                }

                SendMessageW(
                    header,
                    HDM_SETITEMW,
                    i as usize,
                    &hd_item as *const _ as isize,
                );
            }
        }
    }

    pub fn get_subitem_rect(&self, row: i32, col: i32) -> RECT {
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        rect.top = col; 
        rect.left = LVIR_BOUNDS as i32;
        unsafe {
            SendMessageW(self.hwnd, LVM_GETSUBITEMRECT, row as usize, &mut rect as *mut _ as isize);
        }
        rect
    }

    pub fn set_font(&self, hfont: crate::types::HFONT) {
        unsafe {
            SendMessageW(self.hwnd, WM_SETFONT, hfont as usize, 1);
        }
    }

    pub fn update_columns(&self) {
        unsafe {
             let mut occupied_width = 0;
             let col_count = COLUMN_DEFS.len();
             
             for i in 1..col_count {
                 let w = SendMessageW(self.hwnd, LVM_GETCOLUMNWIDTH, i, 0) as i32;
                 if w > 0 {
                     occupied_width += w;
                 }
             }

             let mut client_rect: RECT = std::mem::zeroed();
             GetClientRect(self.hwnd, &mut client_rect);
             let list_inner_width = client_rect.right - client_rect.left;

             let margin = 0;
             let mut new_path_width = list_inner_width - occupied_width - margin;

             if new_path_width < 100 {
                 new_path_width = 100;
             }
             
             SendMessageW(
                 self.hwnd,
                 LVM_SETCOLUMNWIDTH,
                 0, 
                 new_path_width as isize,
             );

             self.update_empty_label_position();
        }
    }

    fn update_empty_label_position(&self) {
        unsafe {
            let mut list_rect: RECT = std::mem::zeroed();
            GetWindowRect(self.hwnd, &mut list_rect);
            
            let parent = GetParent(self.hwnd);
            if parent.is_null() { return; }
            
            let mut pt_top_left = POINT { x: list_rect.left, y: list_rect.top };
            let mut pt_bottom_right = POINT { x: list_rect.right, y: list_rect.bottom };
            ScreenToClient(parent, &mut pt_top_left);
            ScreenToClient(parent, &mut pt_bottom_right);
            
            let list_width = pt_bottom_right.x - pt_top_left.x;
            let list_height = pt_bottom_right.y - pt_top_left.y;
            
            const LABEL_WIDTH: i32 = 300;
            const LABEL_HEIGHT: i32 = 20;
            let label_x = pt_top_left.x + (list_width - LABEL_WIDTH) / 2;
            let label_y = pt_top_left.y + (list_height - LABEL_HEIGHT) / 2;
            
            SetWindowPos(
                self.hwnd_empty_label,
                std::ptr::null_mut(),
                label_x,
                label_y,
                LABEL_WIDTH,
                LABEL_HEIGHT,
                SWP_NOZORDER,
            );
        }
    }

    pub fn clear_all(&self) {
        self.set_item_count(0);
    }

    #[inline]
    pub fn empty_label_hwnd(&self) -> HWND {
        self.hwnd_empty_label
    }

    fn update_empty_state(&self) {
        let count = self.get_item_count();
        let cmd = if count == 0 { SW_SHOW } else { SW_HIDE };
        unsafe {
            ShowWindow(self.hwnd_empty_label, cmd);
        }
    }
}

impl Component for FileListView {
    unsafe fn create(&mut self, _parent: HWND) -> Result<(), String> {
        Ok(())
    }

    fn hwnd(&self) -> Option<HWND> {
        Some(self.hwnd)
    }

    unsafe fn on_resize(&mut self, rect: &RECT) {
        unsafe {
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;

            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                rect.left,
                rect.top,
                width,
                height,
                SWP_NOZORDER,
            );

            self.update_columns();

            const LABEL_WIDTH: i32 = 300;
            const LABEL_HEIGHT: i32 = 20;
            let label_x = rect.left + (width - LABEL_WIDTH) / 2;
            let label_y = rect.top + (height - LABEL_HEIGHT) / 2;
            SetWindowPos(
                self.hwnd_empty_label,
                std::ptr::null_mut(),
                label_x,
                label_y,
                LABEL_WIDTH,
                LABEL_HEIGHT,
                SWP_NOZORDER,
            );
        }
    }

    unsafe fn on_theme_change(&mut self, is_dark: bool) {
        self.set_theme(is_dark);
    }
}

#[derive(Clone, Copy)]
enum RatioTier {
    Ultra,
    Good,
    Moderate,
    Negligible,
}

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

#[inline]
fn tier_to_color(tier: RatioTier, is_dark: bool) -> COLORREF {
    match (tier, is_dark) {
        (RatioTier::Ultra, false) => 0x0000A000,
        (RatioTier::Ultra, true) => 0x0080FF00,
        (RatioTier::Good, false) => 0x00C86400,
        (RatioTier::Good, true) => 0x00FFC850,
        (RatioTier::Moderate, false) => 0x000064C8,
        (RatioTier::Moderate, true) => 0x0000D7FF,
        (RatioTier::Negligible, false) => 0x00808080,
        (RatioTier::Negligible, true) => 0x00A0A0A0,
    }
}

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

pub unsafe fn handle_listview_customdraw(
    _list_hwnd: HWND, 
    lparam: LPARAM, 
    is_dark: bool,
    items: &[BatchItem],
    filtered_items: &[usize]
) -> Option<LRESULT> {
    unsafe {
        let nmlvcd = &mut *(lparam as *mut NMLVCUSTOMDRAW);
        let draw_stage = nmlvcd.nmcd.dwDrawStage;

        if draw_stage == CDDS_PREPAINT {
            return Some(CDRF_NOTIFYITEMDRAW as LRESULT);
        }

        if draw_stage == CDDS_ITEMPREPAINT {
            return Some(CDRF_NOTIFYSUBITEMDRAW as LRESULT);
        }

        if (draw_stage & (CDDS_ITEMPREPAINT | CDDS_SUBITEM)) == (CDDS_ITEMPREPAINT | CDDS_SUBITEM) {
            let sub_item = nmlvcd.i_sub_item;
            let row = nmlvcd.nmcd.dwItemSpec as usize;

            if sub_item == columns::RATIO {
                if row < filtered_items.len() {
                    if let Some(item) = items.get(filtered_items[row]) {
                        let ratio = crate::utils::calculate_saved_percentage(item.logical_size, item.disk_size) as f32;
                        let tier = get_ratio_tier(ratio);
                        let color = tier_to_color(tier, is_dark);

                        SetTextColor(nmlvcd.nmcd.hdc, color);
                        SetBkMode(nmlvcd.nmcd.hdc, TRANSPARENT as i32);
                        nmlvcd.clr_text = color;

                        return Some(CDRF_NEWFONT as LRESULT);
                    }
                }
            }

            let default_color = if is_dark {
                crate::ui::theme::COLOR_LIST_TEXT_DARK
            } else {
                crate::ui::theme::COLOR_LIST_TEXT_LIGHT
            };
            
            SetTextColor(nmlvcd.nmcd.hdc, default_color);
            nmlvcd.clr_text = default_color;

            return Some(CDRF_NEWFONT as LRESULT);
        }

        None
    }
}

unsafe extern "system" fn listview_subclass_proc(
    hwnd: HWND,
    umsg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    dwrefdata: usize,
) -> LRESULT { unsafe {
    let main_hwnd = dwrefdata as HWND;
    let is_dark = crate::ui::theme::is_app_dark_mode(main_hwnd);

    if crate::ui::input::handle_subclass_dispatch(hwnd, umsg, wparam) {
        return 0;
    }

    if umsg == WM_NOTIFY {
            let nmhdr = &*(lparam as *const NMHDR);

            if nmhdr.code == HDN_BEGINTRACKW || nmhdr.code == HDN_DIVIDERDBLCLICKW {
                return 1;
            }

        if nmhdr.code == NM_CUSTOMDRAW {
            let header_hwnd = SendMessageW(hwnd, 0x101F, 0, 0) as HWND; 
            let is_from_header = nmhdr.hwndFrom == header_hwnd;

            if is_from_header {
                let nmcd = &mut *(lparam as *mut NMCUSTOMDRAW);
                if let Some(result) = crate::ui::theme::handle_flat_header_customdraw(header_hwnd, nmcd, is_dark) {
                    return result;
                }
            } else if nmhdr.hwndFrom == hwnd {
                let nmlvcd = &mut *(lparam as *mut NMLVCUSTOMDRAW);
                let draw_stage = nmlvcd.nmcd.dwDrawStage;

                if draw_stage == CDDS_PREPAINT {
                    return CDRF_NOTIFYITEMDRAW as LRESULT;
                }

                if draw_stage == CDDS_ITEMPREPAINT {
                    return CDRF_NOTIFYSUBITEMDRAW as LRESULT;
                }

                if draw_stage == (CDDS_ITEMPREPAINT | CDDS_SUBITEM) {
                    // Ratio color logic is now handled in handle_listview_customdraw 
                    // which is called from the main window proc to have access to the items vector.
                    // This subclass proc only handles the header styling here now.
                    return CDRF_DODEFAULT as LRESULT;
                }
            }
        }
    }

    DefSubclassProc(hwnd, umsg, wparam, lparam)
}}