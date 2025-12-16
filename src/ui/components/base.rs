//! Base component trait for Win32 UI widgets.
//!
//! Defines the `Component` interface that all modular UI widgets must implement,
//! enabling consistent lifecycle management for creation, layout, and theming.

use windows::core::Result;
use windows::Win32::Foundation::{HWND, RECT};

/// A trait for modular UI components that can be created, resized, and themed.
///
/// This trait provides a consistent interface for Win32-based UI widgets,
/// separating creation logic, layout calculations, and theme handling from
/// the main window procedure.
///
/// # Safety
///
/// Methods marked as `unsafe` perform raw Win32 API calls that require:
/// - Valid window handles (HWNDs)
/// - Correct thread affinity (must be called from the UI thread)
///
/// # Example
///
/// ```ignore
/// struct MyComponent {
///     hwnd: HWND,
/// }
///
/// impl Component for MyComponent {
///     unsafe fn create(&mut self, parent: HWND) -> Result<()> {
///         // Create child controls
///         Ok(())
///     }
///     
///     fn hwnd(&self) -> Option<HWND> {
///         Some(self.hwnd)
///     }
///     
///     unsafe fn on_resize(&mut self, parent_rect: &RECT) {
///         // Recalculate positions
///     }
///     
///     unsafe fn on_theme_change(&mut self, is_dark: bool) {
///         // Apply theme colors
///     }
/// }
/// ```
pub trait Component {
    /// Creates the child controls for this component.
    ///
    /// # Arguments
    /// * `parent` - The parent window handle to create controls under.
    ///
    /// # Safety
    /// This function calls Win32 APIs that require a valid parent HWND.
    unsafe fn create(&mut self, parent: HWND) -> Result<()>;

    /// Returns the main HWND of this component, if applicable.
    ///
    /// Some components may have multiple HWNDs; this returns the primary one.
    /// Components that are purely logical groupings may return `None`.
    fn hwnd(&self) -> Option<HWND>;

    /// Handles layout recalculations when the parent window is resized.
    ///
    /// # Arguments
    /// * `parent_rect` - The client rectangle of the parent window.
    ///
    /// # Safety
    /// This function calls Win32 APIs (like `SetWindowPos`) that require valid HWNDs.
    unsafe fn on_resize(&mut self, parent_rect: &RECT);

    /// Handles theme updates (System/Dark/Light mode changes).
    ///
    /// # Arguments
    /// * `is_dark` - `true` for dark mode, `false` for light mode.
    ///
    /// # Safety
    /// This function calls Win32 APIs (like `SetWindowTheme`) that require valid HWNDs.
    unsafe fn on_theme_change(&mut self, is_dark: bool);
}
