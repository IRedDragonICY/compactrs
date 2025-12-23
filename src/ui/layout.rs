use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOZORDER, HWND_TOP};

#[derive(Clone, Copy)]
pub enum SizePolicy {
    Fixed(i32),      // Exact pixel width/height
    Flex(f32),       // Proportional weight (e.g., 1.0, 2.0) to fill remaining space
}

pub struct LayoutItem {
    pub hwnd: HWND,
    pub policy: SizePolicy,
}

/// Lays out items horizontally within the given parent rectangle.
/// 
/// * `parent_rect`: The bounds to layout within.
/// * `items`: List of items to position.
/// * `padding`: Inner padding from the edges of rect.
/// * `gap`: Horizontal spacing between items.
pub unsafe fn layout_horizontal(parent_rect: &RECT, items: &[LayoutItem], padding: i32, gap: i32) {
    if items.is_empty() { return; }

    let width = parent_rect.right - parent_rect.left;
    let height = parent_rect.bottom - parent_rect.top;
    
    // Content area
    let avail_width = width - (padding * 2);
    let start_x = parent_rect.left + padding;
    let start_y = parent_rect.top + padding;
    let item_height = height - (padding * 2);

    // Pass 1: Calculate usage
    let mut total_fixed = 0;
    let mut total_flex = 0.0;
    
    // Gaps sum: (n-1) * gap
    // Use safe math to avoid underflow if items.len() is 0
    let total_gaps = if items.len() > 1 { (items.len() as i32 - 1) * gap } else { 0 };

    for item in items {
        match item.policy {
            SizePolicy::Fixed(w) => total_fixed += w,
            SizePolicy::Flex(w) => total_flex += w,
        }
    }

    let remaining_space = avail_width - total_fixed - total_gaps;
    let flex_unit = if total_flex > 0.0 {
        remaining_space as f32 / total_flex
    } else {
        0.0
    };

    // Pass 2: Layout
    let mut current_x = start_x;

    for item in items {
        let w = match item.policy {
            SizePolicy::Fixed(val) => val,
            SizePolicy::Flex(weight) => (weight * flex_unit) as i32,
        };

        // If hwnd is valid (non-zero), position it. 
        // 0/null HWNDs are treated as spacers.
        if item.hwnd != std::ptr::null_mut() {
             unsafe {
                SetWindowPos(
                    item.hwnd,
                    HWND_TOP, 
                    current_x,
                    start_y,
                    w,
                    item_height,
                    SWP_NOZORDER,
                );
             }
        }

        current_x += w + gap;
    }
}

/// Lays out items vertically within the given parent rectangle.
pub unsafe fn layout_vertical(parent_rect: &RECT, items: &[LayoutItem], padding: i32, gap: i32) {
    if items.is_empty() { return; }

    let width = parent_rect.right - parent_rect.left;
    let height = parent_rect.bottom - parent_rect.top;
    
    let avail_height = height - (padding * 2);
    let start_x = parent_rect.left + padding;
    let start_y = parent_rect.top + padding;
    let item_width = width - (padding * 2);

    let mut total_fixed = 0;
    let mut total_flex = 0.0;
    let total_gaps = if items.len() > 1 { (items.len() as i32 - 1) * gap } else { 0 };

    for item in items {
        match item.policy {
            SizePolicy::Fixed(h) => total_fixed += h,
            SizePolicy::Flex(h) => total_flex += h,
        }
    }

    let remaining_space = avail_height - total_fixed - total_gaps;
    let flex_unit = if total_flex > 0.0 {
        remaining_space as f32 / total_flex
    } else {
        0.0
    };

    let mut current_y = start_y;

    for item in items {
        let h = match item.policy {
            SizePolicy::Fixed(val) => val,
            SizePolicy::Flex(weight) => (weight * flex_unit) as i32,
        };

        if item.hwnd != std::ptr::null_mut() {
             unsafe {
                SetWindowPos(
                    item.hwnd,
                    HWND_TOP,
                    start_x,
                    current_y,
                    item_width,
                    h,
                    SWP_NOZORDER,
                );
             }
        }
        current_y += h + gap;
    }
}


