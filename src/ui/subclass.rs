use crate::types::*;
// use crate::ui::state::AppTheme;

// Subclass ID for our centralized theme handler
const SUBCLASS_ID_THEME: usize = 4242;

/// Installs the theme subclass procedure on the window.
pub unsafe fn subclass_control(hwnd: HWND) {
    unsafe { SetWindowSubclass(hwnd, Some(theme_subclass_proc), SUBCLASS_ID_THEME, 0); }
}

/// Removes the theme subclass procedure.
pub unsafe fn unsubclass_control(hwnd: HWND) {
    unsafe { RemoveWindowSubclass(hwnd, Some(theme_subclass_proc), SUBCLASS_ID_THEME); }
}

unsafe extern "system" fn theme_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id_subclass: usize,
    _ref_data: usize,
) -> LRESULT {
    match msg {
        WM_DESTROY => {
            unsafe { unsubclass_control(hwnd); }
            // Pass to next purely to behave nicely, though we just removed ourselves.
            // DefSubclassProc might be invalid if we removed? 
            // Docs say: "You don't need to call RemoveWindowSubclass if you handle WM_NCDESTROY"
            // But for WM_DESTROY it's good practice.
            // Let's safe-call DefSubclassProc
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        WM_ERASEBKGND => {
            // Let specific controls handle this or propagate
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        WM_PAINT => {
            // We can intercept paint to enforce dark mode backgrounds on certain standard controls
            // that stubbornly stay light (like some statics or checkboxes in older styles).
            // However, usually WM_CTLCOLOR* handles this.
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        // Intercept Theme/Color messages?
        // Actually, the PARENT receives WM_CTLCOLORBTN, etc.
        // This subclass is on the CHILD.
        // So this is useful for OWM_THEMECHANGED or custom painting.
        // For standard controls, theming is often done by the parent responding to WM_CTLCOLOR*.
        // BUT, if we use "Visual Styles" (SetWindowTheme(hwnd, mode)), that's on the child.
        
        _ => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
    }
}

/// Applies the visual theme to a control.
pub unsafe fn apply_theme_to_control(hwnd: HWND, is_dark: bool) {
    let mode = if is_dark { crate::w!("DarkMode_Explorer") } else { crate::w!("Explorer") }; // or just NULL for default
    let null_str = crate::w!("");
    
    // For many controls, SetWindowTheme enables the modern look.
    unsafe { crate::types::SetWindowTheme(hwnd, mode.as_ptr(), null_str.as_ptr()); }
    
    // Send a repaint
    unsafe { InvalidateRect(hwnd, std::ptr::null(), 1); }
}
