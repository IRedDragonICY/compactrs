use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOZORDER};

/// A zero-cost immediate mode layout helper.
///
/// Designed to eliminate manual pixel calculations in `on_resize`.
/// Optimized for horizontal bars (like ActionPanel).
pub struct LayoutRow {
    y: i32,
    height: i32,
    padding: i32,
    current_x: i32,
}

impl LayoutRow {
    /// Starts a new horizontal layout row.
    pub fn new(x: i32, y: i32, height: i32, padding: i32) -> Self {
        Self {
            y,
            height,
            padding,
            current_x: x,
        }
    }

    /// Extends the layout from the right side (Right-to-Left), useful for "Cancel/Ok" buttons.
    pub fn new_rtl(right: i32, y: i32, height: i32, padding: i32) -> Self {
        Self {
            y,
            height,
            padding,
            current_x: right,
        }
    }

    /// Adds a fixed-width control to the layout (Left-to-Right).
    pub unsafe fn add_fixed(&mut self, hwnd: HWND, width: i32) {
        if hwnd.is_null() { return; }
        unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                self.current_x,
                self.y,
                width,
                self.height,
                SWP_NOZORDER,
            );
        }
        self.current_x += width + self.padding;
    }

    /// Adds a fixed-width control to the layout (Right-to-Left).
    /// `self.current_x` acts as the right edge.
    pub unsafe fn add_fixed_rtl(&mut self, hwnd: HWND, width: i32) {
        if hwnd.is_null() { return; }
        let left = self.current_x - width;
        unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                left,
                self.y,
                width,
                self.height,
                SWP_NOZORDER,
            );
        }
        self.current_x = left - self.padding;
    }

    /// Positions a label above the current slot (helper for ActionPanel labels).
    pub unsafe fn add_label_above(&self, hwnd: HWND, width: i32, label_height: i32, offset_y: i32) {
        if hwnd.is_null() { return; }
        // Center-ish or left aligned to the current slot?
        // ActionPanel uses: label is at the same X as the control below it.
        unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                self.current_x,
                self.y + offset_y,
                width,
                label_height,
                SWP_NOZORDER,
            );
        }
    }
    
    /// Returns the current X position.
    pub fn cursor(&self) -> i32 {
        self.current_x
    }
}

/// A zero-cost immediate mode vertical layout helper.
///
/// Designed to satisfy `about.rs` vertical stacking needs.
pub struct LayoutColumn {
    x: i32,
    y: i32,
    width: i32,
    padding: i32,
}

impl LayoutColumn {
    pub fn new(x: i32, y: i32, width: i32, padding: i32) -> Self {
        Self { x, y, width, padding }
    }

    /// Allocates a new row of the given height.
    /// Returns (x, y, width, height).
    pub fn row(&mut self, height: i32) -> (i32, i32, i32, i32) {
        let y = self.y;
        self.y += height + self.padding;
        (self.x, y, self.width, height)
    }

    /// Adds vertical spacing.
    pub fn add_space(&mut self, space: i32) {
        self.y += space;
    }
}
