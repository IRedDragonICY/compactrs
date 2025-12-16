//! ActionPanel component - groups all action buttons and controls.
//!
//! This component manages the action bar at the bottom of the main window,
//! containing buttons for file operations, algorithm selection, and process control.

use windows::core::{w, Result, PCWSTR};
use windows::Win32::Foundation::{HWND, HINSTANCE, RECT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::SetWindowTheme;
use windows::Win32::UI::WindowsAndMessaging::{
    BS_AUTOCHECKBOX, BS_PUSHBUTTON, CBS_DROPDOWNLIST, CBS_HASSTRINGS, CreateWindowExW,
    SetWindowPos, HMENU, SWP_NOZORDER, WINDOW_STYLE, WS_CHILD, WS_TABSTOP,
    WS_VISIBLE, WS_VSCROLL,
};

use super::base::Component;
use crate::ui::controls::{apply_button_theme, apply_combobox_theme};
use crate::ui::utils::ToWide;

/// Configuration for ActionPanel control IDs.
pub struct ActionPanelIds {
    pub btn_files: u16,
    pub btn_folder: u16,
    pub btn_remove: u16,
    pub combo_algo: u16,
    pub chk_force: u16,
    pub btn_process: u16,
    pub btn_cancel: u16,
}

/// ActionPanel component containing all action buttons and controls.
///
/// # Layout
/// Positioned at the very bottom of the window with horizontal button arrangement:
/// [Files] [Folder] [Remove] [Algorithm ▼] [☐ Force] [Process All] [Cancel]
pub struct ActionPanel {
    hwnd_files: HWND,
    hwnd_folder: HWND,
    hwnd_remove: HWND,
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
            hwnd_files: HWND::default(),
            hwnd_folder: HWND::default(),
            hwnd_remove: HWND::default(),
            hwnd_combo: HWND::default(),
            hwnd_force: HWND::default(),
            hwnd_process: HWND::default(),
            hwnd_cancel: HWND::default(),
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
    pub unsafe fn set_font(&self, hfont: windows::Win32::Graphics::Gdi::HFONT) {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_SETFONT};
        
        unsafe {
            let wparam = WPARAM(hfont.0 as usize);
            let lparam = LPARAM(1); // Redraw
            
            SendMessageW(self.hwnd_files, WM_SETFONT, Some(wparam), Some(lparam));
            SendMessageW(self.hwnd_folder, WM_SETFONT, Some(wparam), Some(lparam));
            SendMessageW(self.hwnd_remove, WM_SETFONT, Some(wparam), Some(lparam));
            SendMessageW(self.hwnd_combo, WM_SETFONT, Some(wparam), Some(lparam));
            SendMessageW(self.hwnd_force, WM_SETFONT, Some(wparam), Some(lparam));
            SendMessageW(self.hwnd_process, WM_SETFONT, Some(wparam), Some(lparam));
            SendMessageW(self.hwnd_cancel, WM_SETFONT, Some(wparam), Some(lparam));
        }
    }

    /// Helper to create a button with the builder pattern logic.
    unsafe fn create_button(
        parent: HWND,
        instance: HINSTANCE,
        text: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        id: u16,
        is_dark: bool,
    ) -> Result<HWND> {
        unsafe {
            let text_wide = text.to_wide();
            let hwnd = CreateWindowExW(
                Default::default(),
                w!("BUTTON"),
                PCWSTR::from_raw(text_wide.as_ptr()),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | BS_PUSHBUTTON as u32),
                x,
                y,
                w,
                h,
                Some(parent),
                Some(HMENU(id as isize as *mut _)),
                Some(instance),
                None,
            )?;
            apply_button_theme(hwnd, is_dark);
            Ok(hwnd)
        }
    }
}

impl Component for ActionPanel {
    unsafe fn create(&mut self, parent: HWND) -> Result<()> {
        unsafe {
            let module = GetModuleHandleW(None)?;
            let instance = HINSTANCE(module.0);

            // Initial positions (will be updated in on_resize)
            let btn_h = 32;
            let btn_y = 460;

            // Check system dark mode for initial theme
            let is_dark = crate::ui::theme::is_system_dark_mode();

            // Create Files button
            self.hwnd_files =
                Self::create_button(parent, instance, "Files", 10, btn_y, 65, btn_h, self.ids.btn_files, is_dark)?;

            // Create Folder button
            self.hwnd_folder =
                Self::create_button(parent, instance, "Folder", 85, btn_y, 65, btn_h, self.ids.btn_folder, is_dark)?;

            // Create Remove button
            self.hwnd_remove =
                Self::create_button(parent, instance, "Remove", 160, btn_y, 70, btn_h, self.ids.btn_remove, is_dark)?;

            // Create Algorithm ComboBox
            self.hwnd_combo = CreateWindowExW(
                Default::default(),
                w!("COMBOBOX"),
                None,
                WINDOW_STYLE(
                    WS_VISIBLE.0
                        | WS_CHILD.0
                        | WS_TABSTOP.0
                        | WS_VSCROLL.0
                        | CBS_DROPDOWNLIST as u32
                        | CBS_HASSTRINGS as u32,
                ),
                240,
                btn_y,
                110,
                200, // Drop-down height
                Some(parent),
                Some(HMENU(self.ids.combo_algo as isize as *mut _)),
                Some(instance),
                None,
            )?;

            // Create Force checkbox
            self.hwnd_force = CreateWindowExW(
                Default::default(),
                w!("BUTTON"),
                w!("Force"),
                WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | WS_TABSTOP.0 | BS_AUTOCHECKBOX as u32),
                360,
                btn_y,
                60,
                btn_h,
                Some(parent),
                Some(HMENU(self.ids.chk_force as isize as *mut _)),
                Some(instance),
                None,
            )?;

            // Create Process All button
            self.hwnd_process = Self::create_button(
                parent,
                instance,
                "Process All",
                430,
                btn_y,
                100,
                btn_h,
                self.ids.btn_process,
                is_dark,
            )?;

            // Create Cancel button
            self.hwnd_cancel =
                Self::create_button(parent, instance, "Cancel", 540, btn_y, 80, btn_h, self.ids.btn_cancel, is_dark)?;

            // Apply initial theme to ComboBox and Checkbox
            if is_dark {
                let _ = SetWindowTheme(self.hwnd_combo, w!("DarkMode_CFD"), None);
                let _ = SetWindowTheme(self.hwnd_force, w!("DarkMode_Explorer"), None);
            }

            Ok(())
        }
    }

    fn hwnd(&self) -> Option<HWND> {
        // Return Process button as the "main" HWND (most important action)
        Some(self.hwnd_process)
    }

    unsafe fn on_resize(&mut self, parent_rect: &RECT) {
        unsafe {
            let height = parent_rect.bottom - parent_rect.top;

            let padding = 10;
            let btn_height = 30;

            // Position buttons at bottom of window
            let btn_y = height - btn_height - padding;

            // Files button
            SetWindowPos(
                self.hwnd_files,
                None,
                padding,
                btn_y,
                55,
                btn_height,
                SWP_NOZORDER,
            );

            // Folder button
            SetWindowPos(
                self.hwnd_folder,
                None,
                padding + 60,
                btn_y,
                55,
                btn_height,
                SWP_NOZORDER,
            );

            // Remove button
            SetWindowPos(
                self.hwnd_remove,
                None,
                padding + 120,
                btn_y,
                65,
                btn_height,
                SWP_NOZORDER,
            );

            // Algorithm combo
            SetWindowPos(
                self.hwnd_combo,
                None,
                padding + 190,
                btn_y,
                110,
                btn_height,
                SWP_NOZORDER,
            );

            // Force checkbox
            SetWindowPos(
                self.hwnd_force,
                None,
                padding + 310,
                btn_y,
                60,
                btn_height,
                SWP_NOZORDER,
            );

            // Process All button
            SetWindowPos(
                self.hwnd_process,
                None,
                padding + 380,
                btn_y,
                90,
                btn_height,
                SWP_NOZORDER,
            );

            // Cancel button
            SetWindowPos(
                self.hwnd_cancel,
                None,
                padding + 480,
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

            // Apply theme to ComboBox
            apply_combobox_theme(self.hwnd_combo, is_dark);
        }
    }
}
