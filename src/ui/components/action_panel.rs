#![allow(unsafe_op_in_unsafe_fn)]

//! ActionPanel component - groups all action buttons and controls.
//!
//! This component manages the action bar at the bottom of the main window,
//! containing buttons for file operations, algorithm selection, and process control.

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, SWP_NOZORDER, SendMessageW, WM_SETFONT,
};
use windows_sys::Win32::Graphics::Gdi::HFONT;

use super::base::Component;
use crate::ui::builder::ControlBuilder;
use crate::ui::controls::{apply_button_theme, apply_combobox_theme};

/// Configuration for ActionPanel control IDs.
pub struct ActionPanelIds {
    pub btn_files: u16,
    pub btn_folder: u16,
    pub btn_remove: u16,
    pub combo_action_mode: u16,
    pub lbl_action_mode: u16,
    pub combo_algo: u16,
    pub lbl_algo: u16,
    pub chk_force: u16,
    pub btn_process: u16,
    pub btn_cancel: u16,
}

/// ActionPanel component containing all action buttons and controls.
///
/// # Layout
/// Positioned at the very bottom of the window with horizontal button arrangement:
/// [Files] [Folder] [Remove] [Action Mode ▼] [Algorithm ▼] [☐ Force] [Process All] [Cancel]
pub struct ActionPanel {
    hwnd_files: HWND,
    hwnd_folder: HWND,
    hwnd_remove: HWND,
    hwnd_lbl_action_mode: HWND,
    hwnd_action_mode: HWND,
    hwnd_lbl_algo: HWND,
    hwnd_combo: HWND,
    hwnd_force: HWND,
    hwnd_process: HWND,
    hwnd_cancel: HWND,
    ids: ActionPanelIds,
}

impl ActionPanel {
    /// Creates a new ActionPanel with uninitialized handles.
    ///
    /// Call `create()` to actually create the Win32 controls.
    pub fn new(ids: ActionPanelIds) -> Self {
        Self {
            hwnd_files: std::ptr::null_mut(),
            hwnd_folder: std::ptr::null_mut(),
            hwnd_remove: std::ptr::null_mut(),
            hwnd_lbl_action_mode: std::ptr::null_mut(),
            hwnd_action_mode: std::ptr::null_mut(),
            hwnd_lbl_algo: std::ptr::null_mut(),
            hwnd_combo: std::ptr::null_mut(),
            hwnd_force: std::ptr::null_mut(),
            hwnd_process: std::ptr::null_mut(),
            hwnd_cancel: std::ptr::null_mut(),
            ids,
        }
    }

    /// Returns the Files button HWND.
    #[inline]
    pub fn files_hwnd(&self) -> HWND {
        self.hwnd_files
    }

    /// Returns the Folder button HWND.
    #[inline]
    pub fn folder_hwnd(&self) -> HWND {
        self.hwnd_folder
    }

    /// Returns the Remove button HWND.
    #[inline]
    pub fn remove_hwnd(&self) -> HWND {
        self.hwnd_remove
    }

    /// Returns the Action Mode ComboBox HWND.
    #[inline]
    pub fn action_mode_hwnd(&self) -> HWND {
        self.hwnd_action_mode
    }

    /// Returns the Algorithm ComboBox HWND.
    #[inline]
    pub fn combo_hwnd(&self) -> HWND {
        self.hwnd_combo
    }

    /// Returns the Force checkbox HWND.
    #[inline]
    pub fn force_hwnd(&self) -> HWND {
        self.hwnd_force
    }

    /// Returns the Process button HWND.
    #[inline]
    pub fn process_hwnd(&self) -> HWND {
        self.hwnd_process
    }

    /// Returns the Cancel button HWND.
    #[inline]
    pub fn cancel_hwnd(&self) -> HWND {
        self.hwnd_cancel
    }

    /// Sets the font for all child controls.
    ///
    /// # Arguments
    /// * `hfont` - The font handle to apply
    ///
    /// # Safety
    /// Calls Win32 SendMessageW API.
    pub unsafe fn set_font(&self, hfont: HFONT) {
        let wparam = hfont as usize;
        let lparam = 1; // Redraw
        
        SendMessageW(self.hwnd_files, WM_SETFONT, wparam, lparam);
        SendMessageW(self.hwnd_folder, WM_SETFONT, wparam, lparam);
        SendMessageW(self.hwnd_remove, WM_SETFONT, wparam, lparam);
        SendMessageW(self.hwnd_lbl_action_mode, WM_SETFONT, wparam, lparam);
        SendMessageW(self.hwnd_action_mode, WM_SETFONT, wparam, lparam);
        SendMessageW(self.hwnd_lbl_algo, WM_SETFONT, wparam, lparam);
        SendMessageW(self.hwnd_combo, WM_SETFONT, wparam, lparam);
        SendMessageW(self.hwnd_force, WM_SETFONT, wparam, lparam);
        SendMessageW(self.hwnd_process, WM_SETFONT, wparam, lparam);
        SendMessageW(self.hwnd_cancel, WM_SETFONT, wparam, lparam);
    }

}

impl Component for ActionPanel {
    unsafe fn create(&mut self, parent: HWND) -> Result<(), String> { unsafe {
        // Initial positions (will be updated in on_resize)
        let btn_h = 32;
        let btn_y = 460;

        // Check system dark mode for initial theme
        let is_dark = crate::ui::theme::is_system_dark_mode();

        // Create Files button
        self.hwnd_files = ControlBuilder::new(parent, self.ids.btn_files)
            .button()
            .text("Files")
            .pos(10, btn_y)
            .size(65, btn_h)
            .dark_mode(is_dark)
            .build();

        // Create Folder button
        self.hwnd_folder = ControlBuilder::new(parent, self.ids.btn_folder)
            .button()
            .text("Folder")
            .pos(85, btn_y)
            .size(65, btn_h)
            .dark_mode(is_dark)
            .build();

        // Create Remove button
        self.hwnd_remove = ControlBuilder::new(parent, self.ids.btn_remove)
            .button()
            .text("Remove")
            .pos(160, btn_y)
            .size(70, btn_h)
            .dark_mode(is_dark)
            .build();

        // Create Action Mode Label (title above dropdown)
        self.hwnd_lbl_action_mode = ControlBuilder::new(parent, self.ids.lbl_action_mode)
            .label(false)
            .text("Action")
            .pos(240, btn_y - 18)
            .size(100, 16)
            .dark_mode(is_dark)
            .build();

        // Create Action Mode ComboBox
        self.hwnd_action_mode = ControlBuilder::new(parent, self.ids.combo_action_mode)
            .combobox()
            .pos(240, btn_y)
            .size(100, 200) // Height is dropdown height
            .dark_mode(is_dark)
            .build();

        // Create Algorithm Label (title above dropdown)
        self.hwnd_lbl_algo = ControlBuilder::new(parent, self.ids.lbl_algo)
            .label(false)
            .text("Algorithm")
            .pos(350, btn_y - 18)
            .size(100, 16)
            .dark_mode(is_dark)
            .build();

        // Create Algorithm ComboBox
        self.hwnd_combo = ControlBuilder::new(parent, self.ids.combo_algo)
            .combobox()
            .pos(350, btn_y)
            .size(100, 200) // Height is dropdown height
            .dark_mode(is_dark)
            .build();

        // Create Force checkbox
        self.hwnd_force = ControlBuilder::new(parent, self.ids.chk_force)
            .checkbox()
            .text("Force")
            .pos(360, btn_y)
            .size(60, btn_h)
            .dark_mode(is_dark)
            .build();

        // Create Process All button
        self.hwnd_process = ControlBuilder::new(parent, self.ids.btn_process)
            .button()
            .text("Process All")
            .pos(430, btn_y)
            .size(100, btn_h)
            .dark_mode(is_dark)
            .build();

        // Create Cancel button
        self.hwnd_cancel = ControlBuilder::new(parent, self.ids.btn_cancel)
            .button()
            .text("Cancel")
            .pos(540, btn_y)
            .size(80, btn_h)
            .dark_mode(is_dark)
            .build();

        Ok(())
    }}

    fn hwnd(&self) -> Option<HWND> {
        // Return Process button as the "main" HWND (most important action)
        Some(self.hwnd_process)
    }

    unsafe fn on_resize(&mut self, parent_rect: &RECT) {
        unsafe {
            let height = parent_rect.bottom - parent_rect.top;

            let padding = 10;
            let btn_height = 30;
            let lbl_height = 16;
            let lbl_btn_gap = 2;  // Gap between label and button/dropdown

            // Position buttons at bottom of window with extra space for labels above
            let btn_y = height - btn_height - padding;
            let lbl_y = btn_y - lbl_height - lbl_btn_gap;

            // Files button
            SetWindowPos(
                self.hwnd_files,
                std::ptr::null_mut(),
                padding,
                btn_y,
                55,
                btn_height,
                SWP_NOZORDER,
            );

            // Folder button
            SetWindowPos(
                self.hwnd_folder,
                std::ptr::null_mut(),
                padding + 60,
                btn_y,
                55,
                btn_height,
                SWP_NOZORDER,
            );

            // Remove button
            SetWindowPos(
                self.hwnd_remove,
                std::ptr::null_mut(),
                padding + 120,
                btn_y,
                65,
                btn_height,
                SWP_NOZORDER,
            );

            // Action Mode label (above dropdown)
            SetWindowPos(
                self.hwnd_lbl_action_mode,
                std::ptr::null_mut(),
                padding + 190,
                lbl_y,
                100,
                lbl_height,
                SWP_NOZORDER,
            );

            // Action Mode combo
            SetWindowPos(
                self.hwnd_action_mode,
                std::ptr::null_mut(),
                padding + 190,
                btn_y,
                100,
                btn_height,
                SWP_NOZORDER,
            );

            // Algorithm label (above dropdown)
            SetWindowPos(
                self.hwnd_lbl_algo,
                std::ptr::null_mut(),
                padding + 295,
                lbl_y,
                100,
                lbl_height,
                SWP_NOZORDER,
            );

            // Algorithm combo
            SetWindowPos(
                self.hwnd_combo,
                std::ptr::null_mut(),
                padding + 295,
                btn_y,
                100,
                btn_height,
                SWP_NOZORDER,
            );

            // Force checkbox (after algo combo)
            SetWindowPos(
                self.hwnd_force,
                std::ptr::null_mut(),
                padding + 400,
                btn_y,
                65,
                btn_height,
                SWP_NOZORDER,
            );

            // Process All button
            SetWindowPos(
                self.hwnd_process,
                std::ptr::null_mut(),
                padding + 470,
                btn_y,
                90,
                btn_height,
                SWP_NOZORDER,
            );

            // Cancel button
            SetWindowPos(
                self.hwnd_cancel,
                std::ptr::null_mut(),
                padding + 565,
                btn_y,
                70,
                btn_height,
                SWP_NOZORDER,
            );
        }
    }

    unsafe fn on_theme_change(&mut self, is_dark: bool) {
        unsafe {
            // Apply theme to all buttons
            apply_button_theme(self.hwnd_files, is_dark);
            apply_button_theme(self.hwnd_folder, is_dark);
            apply_button_theme(self.hwnd_remove, is_dark);
            apply_button_theme(self.hwnd_process, is_dark);
            apply_button_theme(self.hwnd_cancel, is_dark);
            apply_button_theme(self.hwnd_force, is_dark); // Checkbox uses button theme

            // Apply theme to ComboBoxes
            apply_combobox_theme(self.hwnd_action_mode, is_dark);
            apply_combobox_theme(self.hwnd_combo, is_dark);

            // Apply theme to labels
            crate::ui::theme::apply_theme(self.hwnd_lbl_action_mode, crate::ui::theme::ControlType::GroupBox, is_dark);
            crate::ui::theme::apply_theme(self.hwnd_lbl_algo, crate::ui::theme::ControlType::GroupBox, is_dark);
        }
    }
}
