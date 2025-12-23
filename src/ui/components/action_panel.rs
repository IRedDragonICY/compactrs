#![allow(unsafe_op_in_unsafe_fn)]

//! ActionPanel component - groups all action buttons and controls.
//!
//! This component manages the action bar at the bottom of the main window,
//! containing buttons for file operations, algorithm selection, and process control.

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::HFONT;

use super::base::Component;
use crate::ui::builder::ControlBuilder;
// Duplicate import removed here
use crate::ui::controls::{apply_button_theme, apply_combobox_theme, apply_accent_button_theme};
use crate::ui::layout::{layout_horizontal, LayoutItem, SizePolicy};

const ICON_FILES: &[u16] = &[0xD83D, 0xDCC4, 0]; // 📄
const ICON_FOLDER: &[u16] = &[0xD83D, 0xDCC1, 0]; // 📂
const ICON_REMOVE: &[u16] = &[0x2796, 0]; // ➖
const ICON_CLEAR: &[u16] = &[0xD83D, 0xDDD1, 0]; // 🗑
const ICON_PROCESS: &[u16] = &[0x25B6, 0]; // ▶
const ICON_PAUSE: &[u16] = &[0x23F8, 0]; // ⏸
const ICON_CANCEL: &[u16] = &[0x23F9, 0]; // ⏹


/// Configuration for ActionPanel control IDs.
pub struct ActionPanelIds {
    pub btn_files: u16,
    pub btn_folder: u16,
    pub btn_remove: u16,
    pub btn_clear: u16,
    pub lbl_input: u16,
    pub combo_action_mode: u16,
    pub lbl_action_mode: u16,
    pub combo_algo: u16,
    pub lbl_algo: u16,
    pub chk_force: u16,
    pub btn_process: u16,
    pub btn_cancel: u16,
    pub btn_pause: u16,
}

pub struct ActionPanel {
    hwnd_files: HWND,
    hwnd_folder: HWND,
    hwnd_remove: HWND,
    hwnd_clear: HWND,
    
    hwnd_lbl_input: HWND,
    
    hwnd_lbl_action_mode: HWND,
    hwnd_action_mode: HWND,
    
    hwnd_lbl_algo: HWND,
    hwnd_combo_algo: HWND,
    
    hwnd_force: HWND,
    
    hwnd_process: HWND,
    hwnd_cancel: HWND,
    hwnd_pause: HWND,
    
    // Layout
    
    ids: ActionPanelIds,
}

impl ActionPanel {
    pub fn new(ids: ActionPanelIds) -> Self {
        Self {
            hwnd_files: std::ptr::null_mut(),
            hwnd_folder: std::ptr::null_mut(),
            hwnd_remove: std::ptr::null_mut(),
            hwnd_clear: std::ptr::null_mut(),
            hwnd_lbl_input: std::ptr::null_mut(),
            
            hwnd_lbl_action_mode: std::ptr::null_mut(),
            hwnd_action_mode: std::ptr::null_mut(),
            
            hwnd_lbl_algo: std::ptr::null_mut(),
            hwnd_combo_algo: std::ptr::null_mut(),
            
            hwnd_force: std::ptr::null_mut(),
            
            hwnd_process: std::ptr::null_mut(),
            hwnd_cancel: std::ptr::null_mut(),
            hwnd_pause: std::ptr::null_mut(),
            
            ids,
        }
    }

    // Accessors
    pub fn files_hwnd(&self) -> HWND { self.hwnd_files }
    pub fn folder_hwnd(&self) -> HWND { self.hwnd_folder }
    pub fn remove_hwnd(&self) -> HWND { self.hwnd_remove }
    pub fn clear_hwnd(&self) -> HWND { self.hwnd_clear }
    pub fn action_mode_hwnd(&self) -> HWND { self.hwnd_action_mode }
    pub fn combo_hwnd(&self) -> HWND { self.hwnd_combo_algo }
    pub fn force_hwnd(&self) -> HWND { self.hwnd_force }
    pub fn process_hwnd(&self) -> HWND { self.hwnd_process }
    pub fn cancel_hwnd(&self) -> HWND { self.hwnd_cancel }
    pub fn pause_hwnd(&self) -> HWND { self.hwnd_pause }

    /// Sets the font for all child controls.
    pub unsafe fn set_font(&self, hfont: HFONT) {
        let _ = hfont; 
    }
}

impl Component for ActionPanel {
    unsafe fn create(&mut self, parent: HWND) -> Result<(), String> { unsafe {
        let is_dark = crate::ui::theme::is_system_dark_mode();
        let font = crate::ui::theme::get_app_font();

        // Helper for consistent control creation
        let create_btn = |id: u16, text: &'static [u16], w: i32| -> HWND {
            ControlBuilder::new(parent, id)
                .button()
                .text_w(text)
                .size(w, 32)
                .dark_mode(is_dark)
                .font(font) // Apply font immediately
                .build()
        };

        let create_lbl = |id: u16, text: &'static str| -> HWND {
            ControlBuilder::new(parent, id)
                .label(false)
                .text(text)
                .size(100, 16)
                .dark_mode(is_dark)
                .font(font)
                .build()
        };

        // --- Create Controls ---
        
        // Input Group
        self.hwnd_lbl_input = create_lbl(self.ids.lbl_input, "Input");
        self.hwnd_files = create_btn(self.ids.btn_files, ICON_FILES, 32);
        self.hwnd_folder = create_btn(self.ids.btn_folder, ICON_FOLDER, 32);
        self.hwnd_remove = create_btn(self.ids.btn_remove, ICON_REMOVE, 32);
        self.hwnd_clear = create_btn(self.ids.btn_clear, ICON_CLEAR, 32);

        // Action Mode Group
        self.hwnd_lbl_action_mode = create_lbl(self.ids.lbl_action_mode, "Action");
        self.hwnd_action_mode = ControlBuilder::new(parent, self.ids.combo_action_mode)
            .combobox()
            .size(100, 200)
            .dark_mode(is_dark)
            .font(font)
            .build();

        // Algorithm Group
        self.hwnd_lbl_algo = create_lbl(self.ids.lbl_algo, "Algorithm");
        self.hwnd_combo_algo = ControlBuilder::new(parent, self.ids.combo_algo)
            .combobox()
            .size(100, 200)
            .dark_mode(is_dark)
            .font(font)
            .build();
        
        // Force Checkbox
        self.hwnd_force = ControlBuilder::new(parent, self.ids.chk_force)
            .checkbox()
            .text("Force")
            .size(60, 32)
            .dark_mode(is_dark)
            .font(font)
            .build();

        // Control Buttons
        self.hwnd_process = create_btn(self.ids.btn_process, ICON_PROCESS, 32);
        // Apply Accent Theme
        apply_accent_button_theme(self.hwnd_process, is_dark);

        self.hwnd_pause = create_btn(self.ids.btn_pause, ICON_PAUSE, 32);
        self.hwnd_cancel = create_btn(self.ids.btn_cancel, ICON_CANCEL, 32);
        
        Ok(())
    }}

    fn hwnd(&self) -> Option<HWND> {
        Some(self.hwnd_process)
    }

    unsafe fn on_resize(&mut self, parent_rect: &RECT) {
        unsafe {
             // Access width if needed, though layout engine handles it
            
            // Layout Configuration
            let top_padding = 4;
            let bar_padding = 10;
            let gap = 5;
            let btn_height = 30;
            let lbl_height = 16;
            
            // Labels Row (Top)
            let lbl_rect = RECT {
                left: parent_rect.left,
                top: parent_rect.top + top_padding,
                right: parent_rect.right,
                bottom: parent_rect.top + top_padding + lbl_height,
            };
            
            // Buttons Row (Bottom)
            let btn_y = lbl_rect.bottom + 4;
            // StackLayout contracts height by 2*padding, so we must expand the rect.
            // We also want the content to start at `btn_y`, so we offset top by -padding.
            let btn_rect = RECT {
                left: parent_rect.left,
                top: btn_y - bar_padding, 
                right: parent_rect.right,
                bottom: btn_y + btn_height + bar_padding,
            };

            // 1. Labels Layout
            // "Input" label spans Files, Folder, Remove, Clear (32*4 + 5*3 = 143)
            let labels = [
                LayoutItem { hwnd: self.hwnd_lbl_input, policy: SizePolicy::Fixed(143) },
                LayoutItem { hwnd: std::ptr::null_mut(), policy: SizePolicy::Fixed(20) }, // Spacer alignment
                LayoutItem { hwnd: self.hwnd_lbl_action_mode, policy: SizePolicy::Fixed(100) },
                LayoutItem { hwnd: self.hwnd_lbl_algo, policy: SizePolicy::Fixed(100) },
            ];
            
            layout_horizontal(&lbl_rect, &labels, bar_padding, gap);

            // 2. Buttons Layout
            // Left to Right flow with a Flex spacer to push execution controls to the end
            let buttons = [
                // Input Group
                LayoutItem { hwnd: self.hwnd_files, policy: SizePolicy::Fixed(32) },
                LayoutItem { hwnd: self.hwnd_folder, policy: SizePolicy::Fixed(32) },
                LayoutItem { hwnd: self.hwnd_remove, policy: SizePolicy::Fixed(32) },
                LayoutItem { hwnd: self.hwnd_clear, policy: SizePolicy::Fixed(32) },
                
                // Spacer
                LayoutItem { hwnd: std::ptr::null_mut(), policy: SizePolicy::Fixed(20) },
                
                // Config Group
                LayoutItem { hwnd: self.hwnd_action_mode, policy: SizePolicy::Fixed(100) },
                LayoutItem { hwnd: self.hwnd_combo_algo, policy: SizePolicy::Fixed(100) },
                LayoutItem { hwnd: self.hwnd_force, policy: SizePolicy::Fixed(65) },
                
                // Flexible Spacer (pushes remaining items to right)
                LayoutItem { hwnd: std::ptr::null_mut(), policy: SizePolicy::Flex(1.0) },
                
                // Execution Controls (Right Aligned)
                // Order: Pause, Process, Cancel (Visual L->R)
                LayoutItem { hwnd: self.hwnd_pause, policy: SizePolicy::Fixed(32) },
                 // Note: Original code used gap=10 for right side, layout_horizontal uses constant gap=5.
                 // This small visual difference is acceptable for code consistency.
                LayoutItem { hwnd: self.hwnd_process, policy: SizePolicy::Fixed(32) },
                LayoutItem { hwnd: self.hwnd_cancel, policy: SizePolicy::Fixed(32) },
            ];
            
            layout_horizontal(&btn_rect, &buttons, bar_padding, gap);
        }
    }


    unsafe fn on_theme_change(&mut self, is_dark: bool) {
        unsafe {
            // Apply theme to all buttons
            apply_button_theme(self.hwnd_files, is_dark);
            apply_button_theme(self.hwnd_folder, is_dark);
            apply_button_theme(self.hwnd_remove, is_dark);
            apply_button_theme(self.hwnd_clear, is_dark);
            apply_accent_button_theme(self.hwnd_process, is_dark);
            apply_button_theme(self.hwnd_cancel, is_dark);
            apply_button_theme(self.hwnd_pause, is_dark);
            apply_button_theme(self.hwnd_force, is_dark); 

            apply_combobox_theme(self.hwnd_action_mode, is_dark);
            apply_combobox_theme(self.hwnd_combo_algo, is_dark);

            crate::ui::theme::apply_theme(self.hwnd_lbl_input, crate::ui::theme::ControlType::GroupBox, is_dark);
            crate::ui::theme::apply_theme(self.hwnd_lbl_action_mode, crate::ui::theme::ControlType::GroupBox, is_dark);
            crate::ui::theme::apply_theme(self.hwnd_lbl_algo, crate::ui::theme::ControlType::GroupBox, is_dark);
        }
    }
}
