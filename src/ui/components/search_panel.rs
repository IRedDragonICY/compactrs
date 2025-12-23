#![allow(unsafe_op_in_unsafe_fn)]

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::HFONT;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    WS_CHILD, WS_VISIBLE, WS_CLIPSIBLINGS, CreateWindowExW, SetWindowPos, SWP_NOZORDER,
    SendMessageW, WM_SETFONT, WM_CTLCOLORSTATIC, WM_CTLCOLORBTN, WM_CTLCOLOREDIT, DefWindowProcW,
    RegisterClassW, WNDCLASSW, CS_HREDRAW, CS_VREDRAW, WM_ERASEBKGND,
    SetPropW, GetPropW, RemovePropW, WM_DESTROY, WM_COMMAND,
    GetParent,
};
use windows_sys::Win32::Graphics::Gdi::{HBRUSH, COLOR_WINDOW};

use super::base::Component;
use crate::ui::builder::ControlBuilder;
use crate::ui::controls::{apply_combobox_theme, apply_checkbox_theme, apply_edit_theme};
use crate::ui::layout::{layout_horizontal, LayoutItem, SizePolicy};
use crate::w;

/// Configuration for SearchPanel control IDs.
pub struct SearchPanelIds {
    pub edit_search: u16,
    pub combo_filter_col: u16,
    pub combo_algo: u16,
    pub combo_size: u16,
    pub chk_case: u16,
    pub chk_regex: u16,
    pub lbl_results: u16,
    // Labels for "Filter By:", etc.
    pub lbl_filter_by: u16,
    pub lbl_algo: u16,
    pub lbl_size: u16,
}

pub struct SearchPanel {
    hwnd_panel: HWND, // Container window
    
    // Controls
    hwnd_search: HWND,
    hwnd_combo_filter_col: HWND,
    hwnd_combo_algo: HWND,
    hwnd_combo_size: HWND,
    
    hwnd_chk_case: HWND,
    hwnd_chk_regex: HWND,
    
    hwnd_lbl_results: HWND,
    hwnd_lbl_filter_by: HWND,
    hwnd_lbl_algo: HWND,
    hwnd_lbl_size: HWND,
    
    ids: SearchPanelIds,
}

impl SearchPanel {
    pub fn new(ids: SearchPanelIds) -> Self {
        Self {
            hwnd_panel: std::ptr::null_mut(),
            hwnd_search: std::ptr::null_mut(),
            hwnd_combo_filter_col: std::ptr::null_mut(),
            hwnd_combo_algo: std::ptr::null_mut(),
            hwnd_combo_size: std::ptr::null_mut(),
            hwnd_chk_case: std::ptr::null_mut(),
            hwnd_chk_regex: std::ptr::null_mut(),
            hwnd_lbl_results: std::ptr::null_mut(),
            hwnd_lbl_filter_by: std::ptr::null_mut(),
            hwnd_lbl_algo: std::ptr::null_mut(),
            hwnd_lbl_size: std::ptr::null_mut(),
            ids,
        }
    }
    
    // Accessors
    pub fn panel_hwnd(&self) -> HWND { self.hwnd_panel }
    pub fn search_hwnd(&self) -> HWND { self.hwnd_search }
    pub fn filter_col_hwnd(&self) -> HWND { self.hwnd_combo_filter_col }
    pub fn algo_hwnd(&self) -> HWND { self.hwnd_combo_algo }
    pub fn size_hwnd(&self) -> HWND { self.hwnd_combo_size }
    pub fn case_hwnd(&self) -> HWND { self.hwnd_chk_case }
    pub fn regex_hwnd(&self) -> HWND { self.hwnd_chk_regex }
    pub fn results_hwnd(&self) -> HWND { self.hwnd_lbl_results }

    /// Sets the font for all child controls.
    pub unsafe fn set_font(&self, hfont: HFONT) {
        unsafe {
            SendMessageW(self.hwnd_search, WM_SETFONT, hfont as usize, 1);
            SendMessageW(self.hwnd_combo_filter_col, WM_SETFONT, hfont as usize, 1);
            SendMessageW(self.hwnd_combo_algo, WM_SETFONT, hfont as usize, 1);
            SendMessageW(self.hwnd_combo_size, WM_SETFONT, hfont as usize, 1);
            SendMessageW(self.hwnd_chk_case, WM_SETFONT, hfont as usize, 1);
            SendMessageW(self.hwnd_chk_regex, WM_SETFONT, hfont as usize, 1);
            SendMessageW(self.hwnd_lbl_results, WM_SETFONT, hfont as usize, 1);
            SendMessageW(self.hwnd_lbl_filter_by, WM_SETFONT, hfont as usize, 1);
            SendMessageW(self.hwnd_lbl_algo, WM_SETFONT, hfont as usize, 1);
            SendMessageW(self.hwnd_lbl_size, WM_SETFONT, hfont as usize, 1);
        }
    }
}

impl Component for SearchPanel {
    unsafe fn create(&mut self, parent: HWND) -> Result<(), String> { unsafe {
        let instance = windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null());
        
        // Create container window with custom class for message handling
        let class_name = w!("CompactRsSearchPanel");
        
        // Register class
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(search_panel_proc);
        wc.hInstance = instance;
        wc.lpszClassName = class_name.as_ptr();
        wc.style = CS_HREDRAW | CS_VREDRAW;
        wc.hbrBackground = if crate::ui::theme::is_system_dark_mode() {
            crate::ui::theme::get_dark_brush()
        } else {
             (COLOR_WINDOW + 1) as HBRUSH
        };
        
        RegisterClassW(&wc);

        self.hwnd_panel = CreateWindowExW(
            0,
            class_name.as_ptr(), 
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            0, 0, 100, 100, // Size set later in on_resize
            parent,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );

        let is_dark = crate::ui::theme::is_system_dark_mode();
        let font = crate::ui::theme::get_app_font();

        // --- Helper Builders ---
        let create_lbl = |id: u16, text: &'static str| -> HWND {
            ControlBuilder::new(self.hwnd_panel, id)
                .label(false)
                .text(text)
                .size(80, 20)
                .dark_mode(is_dark)
                .font(font)
                .build()
        };

        // --- Search Bar (Top Row) ---
        // Search Edit
        self.hwnd_search = ControlBuilder::new(self.hwnd_panel, self.ids.edit_search)
            .edit()
            // Placeholder handled via EM_SETCUEBANNER below
            .size(300, 28)
            .dark_mode(is_dark)
            .font(font)
            .build();
            
        // Use Cue Banner if possible (modern windows)
        const EM_SETCUEBANNER: u32 = 0x1501;
        let cue = w!("Search files, paths, or status...");
        SendMessageW(self.hwnd_search, EM_SETCUEBANNER, 1, cue.as_ptr() as isize);


        // --- Filter Bar (Bottom Row) ---
        
        // Labels
        self.hwnd_lbl_filter_by = create_lbl(self.ids.lbl_filter_by, "Filter By:");
        self.hwnd_lbl_algo = create_lbl(self.ids.lbl_algo, "Algorithm:");
        self.hwnd_lbl_size = create_lbl(self.ids.lbl_size, "Size:");
        self.hwnd_lbl_results = create_lbl(self.ids.lbl_results, "Ready.");

        // Combos
        self.hwnd_combo_filter_col = ControlBuilder::new(self.hwnd_panel, self.ids.combo_filter_col)
            .combobox()
            .size(100, 200)
            .dark_mode(is_dark)
            .font(font)
            .build();
        // Init Combo Items
        crate::ui::wrappers::ComboBox::new(self.hwnd_combo_filter_col).add_string("Path");
        crate::ui::wrappers::ComboBox::new(self.hwnd_combo_filter_col).add_string("Status");
        crate::ui::wrappers::ComboBox::new(self.hwnd_combo_filter_col).set_selected_index(0); // Default Path

        self.hwnd_combo_algo = ControlBuilder::new(self.hwnd_panel, self.ids.combo_algo)
            .combobox()
            .size(100, 200)
            .dark_mode(is_dark)
            .font(font)
            .build();
        // Init Algo Items
        crate::ui::wrappers::ComboBox::new(self.hwnd_combo_algo).add_string("All");
        crate::ui::wrappers::ComboBox::new(self.hwnd_combo_algo).add_string("XPRESS4K");
        crate::ui::wrappers::ComboBox::new(self.hwnd_combo_algo).add_string("XPRESS8K");
        crate::ui::wrappers::ComboBox::new(self.hwnd_combo_algo).add_string("XPRESS16K");
        crate::ui::wrappers::ComboBox::new(self.hwnd_combo_algo).add_string("LZX");
        crate::ui::wrappers::ComboBox::new(self.hwnd_combo_algo).set_selected_index(0);

        self.hwnd_combo_size = ControlBuilder::new(self.hwnd_panel, self.ids.combo_size)
            .combobox()
            .size(100, 200)
            .dark_mode(is_dark)
            .font(font)
            .build();
        crate::ui::wrappers::ComboBox::new(self.hwnd_combo_size).add_string("All");
        crate::ui::wrappers::ComboBox::new(self.hwnd_combo_size).add_string("Small (<1MB)");
        crate::ui::wrappers::ComboBox::new(self.hwnd_combo_size).add_string("Large (>100MB)");
        crate::ui::wrappers::ComboBox::new(self.hwnd_combo_size).set_selected_index(0);

        // Checkboxes
        self.hwnd_chk_case = ControlBuilder::new(self.hwnd_panel, self.ids.chk_case)
            .checkbox()
            .text("Case Sensitive")
            .size(110, 20)
            .dark_mode(is_dark)
            .font(font)
            .build();

        self.hwnd_chk_regex = ControlBuilder::new(self.hwnd_panel, self.ids.chk_regex)
            .checkbox()
            .text("Regex")
            .size(70, 20)
            .dark_mode(is_dark)
            .font(font)
            .build();
            
        Ok(())
    }}

    fn hwnd(&self) -> Option<HWND> {
        Some(self.hwnd_panel)
    }
    
    // Layout Logic
    unsafe fn on_resize(&mut self, parent_rect: &RECT) {
        unsafe {
            let width = parent_rect.right - parent_rect.left;
            let height = parent_rect.bottom - parent_rect.top;
            
            // Set Panel Size and Pos (handled by parent usually, but here we assume parent handles pos, we handle internal layout)
            SetWindowPos(self.hwnd_panel, std::ptr::null_mut(), parent_rect.left, parent_rect.top, width, height, SWP_NOZORDER);

            let padding = 10;
            let row_h = 30;
            
            // Row 1: Search Bar + Results Label
            // "Found X Items" takes ~150px on the right.
            let label_w = 200; // ample space for text
            let gap = 10;
            
            let row1_rect = RECT {
                left: 0, 
                top: 0, // layout starts at top+padding, so 0+10 = 10 (Correct)
                right: width,
                bottom: 28 + (padding * 2), // 28 content + 20 padding = 48
            };
            
            let row1_items = [
                LayoutItem { hwnd: self.hwnd_search, policy: SizePolicy::Flex(1.0) },
                LayoutItem { hwnd: std::ptr::null_mut(), policy: SizePolicy::Fixed(gap) },
                LayoutItem { hwnd: self.hwnd_lbl_results, policy: SizePolicy::Fixed(label_w) },
            ];
            
            layout_horizontal(&row1_rect, &row1_items, padding, 0);


            // Row 2: Filter Controls
            let row2_y = padding + row_h + 5; 
            
            // Row 2 Rect
            // We want content at row2_y.
            // layout starts at top + padding.
            // so top = row2_y - padding.
            let r2_top = row2_y - padding;
            let r2_height = 24 + (padding * 2); // 24 height + padding
            let r2_rect = RECT { left: 0, top: r2_top, right: width, bottom: r2_top + r2_height };
            
            let row2_items = [
                 LayoutItem { hwnd: self.hwnd_lbl_filter_by, policy: SizePolicy::Fixed(60) },
                 LayoutItem { hwnd: self.hwnd_combo_filter_col, policy: SizePolicy::Fixed(100) },
                 LayoutItem { hwnd: self.hwnd_lbl_algo, policy: SizePolicy::Fixed(65) },
                 LayoutItem { hwnd: self.hwnd_combo_algo, policy: SizePolicy::Fixed(90) },
                 LayoutItem { hwnd: self.hwnd_lbl_size, policy: SizePolicy::Fixed(35) },
                 LayoutItem { hwnd: self.hwnd_combo_size, policy: SizePolicy::Fixed(90) },
                 LayoutItem { hwnd: self.hwnd_chk_case, policy: SizePolicy::Fixed(110) },
                 LayoutItem { hwnd: self.hwnd_chk_regex, policy: SizePolicy::Fixed(70) },
            ];
            
            layout_horizontal(&r2_rect, &row2_items, padding, 10);
        }
    }

    unsafe fn on_theme_change(&mut self, is_dark: bool) {
        // Update container background if needed (Static usually takes parent bg)
        // But we might need to invalidate rect
        
        apply_edit_theme(self.hwnd_search, is_dark);
        apply_combobox_theme(self.hwnd_combo_filter_col, is_dark);
        apply_combobox_theme(self.hwnd_combo_algo, is_dark);
        apply_combobox_theme(self.hwnd_combo_size, is_dark);
        apply_checkbox_theme(self.hwnd_chk_case, is_dark);
        apply_checkbox_theme(self.hwnd_chk_regex, is_dark);
        
        
        // Store theme state in property so WndProc can access it
        // 1 = Light, 2 = Dark. This distinguishes "Explicit Light" from "Not Set" (0).
        let prop_val = if is_dark { 2 } else { 1 };
        SetPropW(self.hwnd_panel, crate::w!("CompactRs_Theme").as_ptr(), prop_val as isize as _);

        // Update: Custom class handles it now. Force repaint.
        windows_sys::Win32::Graphics::Gdi::InvalidateRect(self.hwnd_panel, std::ptr::null(), 1);
    }
}

// Window Procedure for SearchPanel
unsafe extern "system" fn search_panel_proc(hwnd: HWND, umsg: u32, wparam: usize, lparam: isize) -> isize {
    match umsg {
        WM_COMMAND => {
             // Forward notifications (Edit change, Combo select) to parent (Main Window)
             let parent = GetParent(hwnd);
             if parent != std::ptr::null_mut() {
                 SendMessageW(parent, umsg, wparam, lparam);
             }
             return 0;
        },
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT => {
            // Check property first (1=Light, 2=Dark), fallback to system
            let prop_val = GetPropW(hwnd, crate::w!("CompactRs_Theme").as_ptr()) as usize;
            let is_dark = if prop_val != 0 {
                prop_val == 2
            } else {
                crate::ui::theme::is_system_dark_mode()
            };

            // Use centralized handler
            if let Some(res) = crate::ui::theme::handle_standard_colors(hwnd, umsg, wparam, is_dark) {
                return res as isize;
            }
        },
        WM_ERASEBKGND => {
            // Handle background erasure to prevent white flash
            let hdc = wparam as windows_sys::Win32::Graphics::Gdi::HDC;
            let mut rect: RECT = std::mem::zeroed();
            windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
            
            let prop_val = GetPropW(hwnd, crate::w!("CompactRs_Theme").as_ptr()) as usize;
            let is_dark = if prop_val != 0 {
                prop_val == 2
            } else {
                crate::ui::theme::is_system_dark_mode()
            };

            let brush = if is_dark {
                crate::ui::theme::get_dark_brush()
            } else {
             (COLOR_WINDOW + 1) as HBRUSH
            };
            
            unsafe {
                windows_sys::Win32::Graphics::Gdi::FillRect(hdc, &rect, brush);
            }
            return 1;
        },
        WM_DESTROY => {
            RemovePropW(hwnd, crate::w!("CompactRs_Theme").as_ptr());
        },
        _ => {}
    }
    
    DefWindowProcW(hwnd, umsg, wparam, lparam)
}
