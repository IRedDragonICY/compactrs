#![allow(unsafe_op_in_unsafe_fn)]

//! ActionPanel component - groups all action buttons and controls.
//!
//! This component manages the action bar at the bottom of the main window,
//! containing buttons for file operations, algorithm selection, and process control.

use crate::types::*;

use super::base::Component;
use crate::ui::builder::ControlBuilder;
// Duplicate import removed here
use crate::ui::controls::{apply_button_theme, apply_combobox_theme, apply_accent_button_theme};
use crate::ui::layout::{LayoutNode, SizePolicy};

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
    pub lbl_accuracy: u16,
    pub btn_process: u16,
    pub btn_cancel: u16,
    pub btn_pause: u16,
}

pub struct ActionPanel {
    hwnd_panel: HWND,
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
    hwnd_lbl_accuracy: HWND,
    
    hwnd_process: HWND,
    hwnd_cancel: HWND,
    hwnd_pause: HWND,
    
    // Layout
    ids: ActionPanelIds,
}

impl ActionPanel {
    pub fn new(ids: ActionPanelIds) -> Self {
        Self {
            hwnd_panel: std::ptr::null_mut(),
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
            hwnd_lbl_accuracy: std::ptr::null_mut(),
            
            hwnd_process: std::ptr::null_mut(),
            hwnd_cancel: std::ptr::null_mut(),
            hwnd_pause: std::ptr::null_mut(),
            
            ids,
        }
    }

    // Accessors
    pub fn hwnd(&self) -> HWND { self.hwnd_panel }
    pub fn files_hwnd(&self) -> HWND { self.hwnd_files }
    pub fn folder_hwnd(&self) -> HWND { self.hwnd_folder }
    pub fn remove_hwnd(&self) -> HWND { self.hwnd_remove }
    pub fn clear_hwnd(&self) -> HWND { self.hwnd_clear }
    pub fn action_mode_hwnd(&self) -> HWND { self.hwnd_action_mode }
    pub fn combo_hwnd(&self) -> HWND { self.hwnd_combo_algo }
    pub fn force_hwnd(&self) -> HWND { self.hwnd_force }
    pub fn accuracy_hwnd(&self) -> HWND { self.hwnd_lbl_accuracy }
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
        // Use centralized Panel creation
        self.hwnd_panel = crate::ui::components::panel::Panel::create(
            parent,
            "CompactRsActionPanel",
            0, 0, 100, 60
        )?;

        let is_dark = crate::ui::theme::is_system_dark_mode();
        let font = crate::ui::theme::get_app_font();
        let parent_hwnd = self.hwnd_panel;

        // Wait! We need to ensure we call the helper function to build controls with correct parent
        
        // Helper for consistent control creation
        let create_btn = |id: u16, text: &'static [u16], w: i32| -> HWND {
            ControlBuilder::new(parent_hwnd, id)
                .button()
                .text_w(text)
                .size(w, 32)
                .dark_mode(is_dark)
                .font(font) // Apply font immediately
                .build()
        };

        let create_lbl = |id: u16, text: &'static str| -> HWND {
            ControlBuilder::new(parent_hwnd, id)
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
        self.hwnd_action_mode = ControlBuilder::new(parent_hwnd, self.ids.combo_action_mode)
            .combobox()
            .size(100, 200)
            .dark_mode(is_dark)
            .font(font)
            .build();

        // Algorithm Group
        self.hwnd_lbl_algo = create_lbl(self.ids.lbl_algo, "Algorithm");
        self.hwnd_combo_algo = ControlBuilder::new(parent_hwnd, self.ids.combo_algo)
            .combobox()
            .size(100, 200)
            .dark_mode(is_dark)
            .font(font)
            .build();
        
        // Force Checkbox
        self.hwnd_force = ControlBuilder::new(parent_hwnd, self.ids.chk_force)
            .checkbox() // Use checkbox builder
            .text("Force")
            .size(60, 32)
            .dark_mode(is_dark)
            .font(font)
            .build();
        
        // Accuracy Label (next to Force checkbox)
        self.hwnd_lbl_accuracy = ControlBuilder::new(parent_hwnd, self.ids.lbl_accuracy)
            .label(false)
            .text("Acc: --")
            .size(80, 32)
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
        Some(self.hwnd_panel)
    }

    unsafe fn on_resize(&mut self, parent_rect: &RECT) {
        // Resize container to fill expected area
        let w = parent_rect.right - parent_rect.left;
        let h = parent_rect.bottom - parent_rect.top;
        
        SetWindowPos(self.hwnd_panel, std::ptr::null_mut(), parent_rect.left, parent_rect.top, w, h, SWP_NOZORDER);
        self.refresh_layout();
    }
    
    unsafe fn on_theme_change(&mut self, is_dark: bool) {
        unsafe {
            // Apply theme to controls
            apply_button_theme(self.hwnd_files, is_dark);
            apply_button_theme(self.hwnd_folder, is_dark);
            apply_button_theme(self.hwnd_remove, is_dark);
            apply_button_theme(self.hwnd_clear, is_dark);
            apply_accent_button_theme(self.hwnd_process, is_dark);
            apply_button_theme(self.hwnd_cancel, is_dark);
            apply_button_theme(self.hwnd_pause, is_dark);
            
            // Fix: Use correct checkbox theme for Force button
            crate::ui::controls::apply_checkbox_theme(self.hwnd_force, is_dark);
            crate::ui::theme::apply_theme(self.hwnd_lbl_accuracy, crate::ui::theme::ControlType::Window, is_dark);

            apply_combobox_theme(self.hwnd_action_mode, is_dark);
            apply_combobox_theme(self.hwnd_combo_algo, is_dark);

            // Fix: Do NOT use GroupBox theme for labels (they are Static controls)
            // Just let the parent WndProc (this panel) handle WM_CTLCOLORSTATIC for them.
            // If we really need theming (e.g. for rounded corners?), apply Window theme.
            // But for now, removing the explicit GroupBox theme call solves the white background
            // if the parent paints dark.
            crate::ui::theme::apply_theme(self.hwnd_lbl_input, crate::ui::theme::ControlType::Window, is_dark);
            crate::ui::theme::apply_theme(self.hwnd_lbl_action_mode, crate::ui::theme::ControlType::Window, is_dark);
            crate::ui::theme::apply_theme(self.hwnd_lbl_algo, crate::ui::theme::ControlType::Window, is_dark);
            
            // Store theme prop for WndProc
            crate::ui::components::panel::Panel::update_theme(self.hwnd_panel, is_dark);
        }
    }
}

// Window Procedure for ActionPanel


impl ActionPanel {
    pub unsafe fn refresh_layout(&self) {
        use SizePolicy::Fixed;
        
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(self.hwnd_panel, &mut rect);
        let w = rect.right - rect.left;
        
        let lbl_rect = RECT {
            left: 0,
            top: 4,
            right: w,
            bottom: 20,
        };
        
        let btn_rect = RECT {
            left: 0,
            top: 14,
            right: w,
            bottom: 64,
        };

        // Labels Row
        // Height 16px. Padding must be 0 or small.
        LayoutNode::row(0, 5)
            .with(self.hwnd_lbl_input, Fixed(143))
            .spacer(20)
            .with(self.hwnd_lbl_action_mode, Fixed(100))
            .with(self.hwnd_lbl_algo, Fixed(100))
            .apply_layout(lbl_rect);

        // Buttons Row
        // Rect height 50. Padding 9 gives 32px inner height.
        LayoutNode::row(9, 5)
            .with(self.hwnd_files, Fixed(32))
            .with(self.hwnd_folder, Fixed(32))
            .with(self.hwnd_remove, Fixed(32))
            .with(self.hwnd_clear, Fixed(32))
            .spacer(20)
            .with(self.hwnd_action_mode, Fixed(100))
            .with(self.hwnd_combo_algo, Fixed(100))
            .with(self.hwnd_force, Fixed(65))
            .with(self.hwnd_lbl_accuracy, Fixed(80))
            .flex_spacer()
            .with(self.hwnd_pause, Fixed(32))
            .with(self.hwnd_process, Fixed(32))
            .with(self.hwnd_cancel, Fixed(32))
            .apply_layout(btn_rect);
    }
}
