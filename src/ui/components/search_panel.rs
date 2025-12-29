#![allow(unsafe_op_in_unsafe_fn)]

use crate::types::*;

use super::base::Component;
use crate::ui::builder::ControlBuilder;
// use crate::ui::controls::{apply_combobox_theme, apply_checkbox_theme, apply_edit_theme};
use crate::ui::controls::*;
use crate::ui::layout::{LayoutNode, SizePolicy};
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
        let _instance = GetModuleHandleW(std::ptr::null_mut());
        
        // Use centralized Panel creation
        self.hwnd_panel = crate::ui::components::panel::Panel::create(
            parent,
            "CompactRsSearchPanel",
            0, 0, 100, 100
        )?;

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
        let (w, h) = (parent_rect.right - parent_rect.left, parent_rect.bottom - parent_rect.top);
        SetWindowPos(self.hwnd_panel, std::ptr::null_mut(), parent_rect.left, parent_rect.top, w, h, SWP_NOZORDER);
        self.refresh_layout();
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
        unsafe {
            crate::ui::components::panel::Panel::update_theme(self.hwnd_panel, is_dark);
        }
    }
}

impl SearchPanel {
    pub unsafe fn refresh_layout(&self) {
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(self.hwnd_panel, &mut rect);
        let w = rect.right - rect.left;
        
        use SizePolicy::{Fixed, Flex};
        
        // Row 1: Search Bar + Results Label
        LayoutNode::row(10, 0)
            .with(self.hwnd_search, Flex(1.0)).spacer(10).with(self.hwnd_lbl_results, Fixed(200))
            .apply_layout(RECT { left: 0, top: 0, right: w, bottom: 48 });

        // Row 2: Filter Controls
        LayoutNode::row(10, 10)
            .with(self.hwnd_lbl_filter_by, Fixed(60))
            .with(self.hwnd_combo_filter_col, Fixed(100))
            .with(self.hwnd_lbl_algo, Fixed(65))
            .with(self.hwnd_combo_algo, Fixed(90))
            .with(self.hwnd_lbl_size, Fixed(35))
            .with(self.hwnd_combo_size, Fixed(90))
            .with(self.hwnd_chk_case, Fixed(110))
            .with(self.hwnd_chk_regex, Fixed(70))
            .apply_layout(RECT { left: 0, top: 35, right: w, bottom: 79 });
    }
}

// Window Procedure for SearchPanel

