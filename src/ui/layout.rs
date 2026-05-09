#![allow(unsafe_op_in_unsafe_fn)]

use crate::types::*;

#[link(name = "user32")]
unsafe extern "system" {
    pub fn BeginDeferWindowPos(nNumWindows: i32) -> HANDLE;
    pub fn DeferWindowPos(hWinPosInfo: HANDLE, hWnd: HWND, hWndInsertAfter: HWND, x: i32, y: i32, cx: i32, cy: i32, uFlags: u32) -> HANDLE;
    pub fn EndDeferWindowPos(hWinPosInfo: HANDLE) -> BOOL;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SizePolicy {
    Fixed(i32),      // Exact pixel width/height (unscaled)
    Flex(f32),       // Proportional weight to fill remaining space
    Auto,            // Auto-size based on content (intrinsic)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlignItems {
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JustifyContent {
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlignContent {
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
}

pub struct LayoutItem {
    pub hwnd: HWND,
    pub policy: SizePolicy,
}

pub struct LayoutNode {
    pub hwnd: Option<HWND>,
    pub policy: SizePolicy,
    pub cross_policy: Option<SizePolicy>,
    pub children: Vec<LayoutNode>,
    pub direction: Direction,
    pub padding: i32,
    pub gap: i32,
    
    pub align_items: AlignItems,
    pub justify_content: JustifyContent,
    pub flex_wrap: FlexWrap,
    pub align_content: AlignContent,
}

impl LayoutNode {
    pub fn new_container(direction: Direction, padding: i32, gap: i32) -> Self {
        Self {
            hwnd: None,
            policy: SizePolicy::Flex(1.0),
            cross_policy: None,
            children: Vec::new(),
            direction,
            padding,
            gap,
            align_items: AlignItems::Stretch,
            justify_content: JustifyContent::FlexStart,
            flex_wrap: FlexWrap::NoWrap,
            align_content: AlignContent::Stretch,
        }
    }

    pub fn new_leaf(hwnd: HWND, policy: SizePolicy) -> Self {
        Self {
            hwnd: Some(hwnd),
            policy,
            cross_policy: None,
            children: Vec::new(),
            direction: Direction::Horizontal, 
            padding: 0,
            gap: 0,
            align_items: AlignItems::Stretch,
            justify_content: JustifyContent::FlexStart,
            flex_wrap: FlexWrap::NoWrap,
            align_content: AlignContent::Stretch,
        }
    }

    pub fn add_child(&mut self, child: LayoutNode) -> &mut Self {
        self.children.push(child);
        self
    }

    pub fn align_items(mut self, align: AlignItems) -> Self {
        self.align_items = align;
        self
    }

    pub fn justify_content(mut self, justify: JustifyContent) -> Self {
        self.justify_content = justify;
        self
    }

    pub fn flex_wrap(mut self, wrap: FlexWrap) -> Self {
        self.flex_wrap = wrap;
        self
    }

    pub fn align_content(mut self, align: AlignContent) -> Self {
        self.align_content = align;
        self
    }

    pub fn cross_policy(mut self, policy: SizePolicy) -> Self {
        self.cross_policy = Some(policy);
        self
    }

    pub fn row(padding: i32, gap: i32) -> Self {
        Self::new_container(Direction::Horizontal, padding, gap)
    }
    
    pub fn col(padding: i32, gap: i32) -> Self {
        Self::new_container(Direction::Vertical, padding, gap)
    }
    
    pub fn with_child(mut self, child: LayoutNode) -> Self {
        self.children.push(child);
        self
    }
    
    pub fn with_policy(mut self, policy: SizePolicy) -> Self {
        self.policy = policy;
        self
    }
    
    pub fn with(mut self, hwnd: HWND, policy: SizePolicy) -> Self {
        self.children.push(Self::new_leaf(hwnd, policy));
        self
    }
    
    pub fn spacer(mut self, size: i32) -> Self {
        self.children.push(Self::new_leaf(std::ptr::null_mut(), SizePolicy::Fixed(size)));
        self
    }
    
    pub fn flex_spacer(mut self) -> Self {
        self.children.push(Self::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
        self
    }
    
    pub fn fixed(hwnd: HWND, size: i32) -> Self {
        Self::new_leaf(hwnd, SizePolicy::Fixed(size))
    }
    
    pub fn flex(hwnd: HWND, weight: f32) -> Self {
        Self::new_leaf(hwnd, SizePolicy::Flex(weight))
    }
    
    pub fn calculate_layout(&self, rect: RECT) -> i32 {
        unsafe { self.apply_layout(rect) }
    }

    pub unsafe fn apply_layout(&self, rect: RECT) -> i32 {
        let mut count = 0;
        self.count_hwnds(&mut count);
        
        unsafe {
            let hdwp = if count > 0 { BeginDeferWindowPos(count as i32) } else { std::ptr::null_mut() };
            let (used, final_hdwp) = self.apply_layout_internal(rect, hdwp);
            
            if !final_hdwp.is_null() {
                EndDeferWindowPos(final_hdwp);
            }
            used
        }
    }

    fn count_hwnds(&self, count: &mut usize) {
        if let Some(hwnd) = self.hwnd {
            if !hwnd.is_null() {
                *count += 1;
            }
        }
        for child in &self.children {
            child.count_hwnds(count);
        }
    }

    unsafe fn apply_layout_internal(&self, rect: RECT, mut hdwp: HANDLE) -> (i32, HANDLE) {
        if rect.right <= rect.left || rect.bottom <= rect.top { return (0, hdwp); }

        let padding = crate::ui::theme::scale(self.padding);
        let gap = crate::ui::theme::scale(self.gap) as f32;

        let inner_rect = RECT {
            left: rect.left + padding,
            top: rect.top + padding,
            right: rect.right - padding,
            bottom: rect.bottom - padding,
        };
        
        let inner_w = (inner_rect.right - inner_rect.left) as f32;
        let inner_h = (inner_rect.bottom - inner_rect.top) as f32;

        if inner_w <= 0.0 || inner_h <= 0.0 { return (0, hdwp); }

        let is_horiz = self.direction == Direction::Horizontal;
        let (max_main, max_cross) = if is_horiz { (inner_w, inner_h) } else { (inner_h, inner_w) };

        struct FlexItem<'a> {
            node: &'a LayoutNode,
            basis: f32,
            grow: f32,
            shrink: f32,
            main_size: f32,
            cross_size: f32,
            main_offset: f32,
            cross_offset: f32,
        }

        let mut items = Vec::with_capacity(self.children.len());
        for child in &self.children {
            let (basis, grow, shrink) = match child.policy {
                SizePolicy::Fixed(s) => (crate::ui::theme::scale(s) as f32, 0.0, 0.0),
                SizePolicy::Flex(w) => (0.0, w, 1.0),
                SizePolicy::Auto => {
                    let s = if is_horiz { crate::ui::theme::scale(100) } else { crate::ui::theme::scale(24) };
                    (s as f32, 0.0, 0.0)
                }
            };
            items.push(FlexItem {
                node: child,
                basis, grow, shrink,
                main_size: basis,
                cross_size: 0.0,
                main_offset: 0.0,
                cross_offset: 0.0,
            });
        }

        struct FlexLine {
            start: usize,
            end: usize,
            main_size: f32,
            total_grow: f32,
            total_shrink: f32,
            cross_size: f32,
        }

        let mut lines = Vec::new();
        let mut start_idx = 0;
        let mut current_main = 0.0;
        let mut current_grow = 0.0;
        let mut current_shrink = 0.0;

        for i in 0..items.len() {
            let item = &items[i];
            let item_outer = item.basis + if i > start_idx { gap } else { 0.0 };

            if self.flex_wrap == FlexWrap::Wrap && current_main + item_outer > max_main && i > start_idx {
                lines.push(FlexLine {
                    start: start_idx,
                    end: i,
                    main_size: current_main,
                    total_grow: current_grow,
                    total_shrink: current_shrink,
                    cross_size: 0.0,
                });
                start_idx = i;
                current_main = item.basis;
                current_grow = item.grow;
                current_shrink = item.shrink * item.basis;
            } else {
                current_main += item_outer;
                current_grow += item.grow;
                current_shrink += item.shrink * item.basis;
            }
        }
        
        if start_idx < items.len() {
            lines.push(FlexLine {
                start: start_idx,
                end: items.len(),
                main_size: current_main,
                total_grow: current_grow,
                total_shrink: current_shrink,
                cross_size: 0.0,
            });
        }

        for line in &mut lines {
            let available = max_main - line.main_size;
            
            if available > 0.0 && line.total_grow > 0.0 {
                for i in line.start..line.end {
                    let item = &mut items[i];
                    item.main_size = item.basis + available * (item.grow / line.total_grow);
                }
                line.main_size = max_main;
            } else if available < 0.0 && line.total_shrink > 0.0 {
                for i in line.start..line.end {
                    let item = &mut items[i];
                    let shrink_share = (item.shrink * item.basis) / line.total_shrink;
                    item.main_size = (item.basis + available * shrink_share).max(0.0);
                }
                line.main_size = max_main;
            }
        }

        for item in &mut items {
            let c_sz = match item.node.cross_policy {
                Some(SizePolicy::Fixed(s)) => crate::ui::theme::scale(s) as f32,
                Some(SizePolicy::Flex(w)) => max_cross * w,
                _ => max_cross, // Fill available cross space
            };
            item.cross_size = c_sz;
        }

        let mut total_cross = 0.0;
        let line_count = lines.len();
        
        for line in &mut lines {
            let mut max_c = 0.0f32;
            for i in line.start..line.end {
                if items[i].cross_size > max_c {
                    max_c = items[i].cross_size;
                }
            }
            if line_count == 1 && self.align_content == AlignContent::Stretch {
                line.cross_size = max_cross;
            } else {
                line.cross_size = max_c;
            }
            total_cross += line.cross_size;
        }
        
        let cross_gap = gap;
        total_cross += (line_count.saturating_sub(1) as f32) * cross_gap;

        let available_cross = max_cross - total_cross;
        let (mut current_cross, cross_spacing) = match self.align_content {
            AlignContent::FlexStart => (0.0, 0.0),
            AlignContent::FlexEnd => (available_cross, 0.0),
            AlignContent::Center => (available_cross / 2.0, 0.0),
            AlignContent::SpaceBetween => {
                if line_count > 1 { (0.0, available_cross / (line_count - 1) as f32) }
                else { (0.0, 0.0) }
            },
            AlignContent::SpaceAround => {
                if line_count > 0 { let s = available_cross / line_count as f32; (s / 2.0, s) }
                else { (0.0, 0.0) }
            },
            AlignContent::Stretch => {
                if line_count > 0 && available_cross > 0.0 {
                    let extra = available_cross / line_count as f32;
                    for line in &mut lines { line.cross_size += extra; }
                }
                (0.0, 0.0)
            }
        };

        let mut total_used_main = 0.0f32;

        for line in &lines {
            let free_main = max_main - line.main_size;
            let item_count = line.end - line.start;
            
            let (mut main_pos, main_spacing) = match self.justify_content {
                JustifyContent::FlexStart => (0.0, 0.0),
                JustifyContent::FlexEnd => (free_main, 0.0),
                JustifyContent::Center => (free_main / 2.0, 0.0),
                JustifyContent::SpaceBetween => {
                    if item_count > 1 { (0.0, free_main / (item_count - 1) as f32) }
                    else { (0.0, 0.0) }
                },
                JustifyContent::SpaceAround => {
                    if item_count > 0 { let s = free_main / item_count as f32; (s / 2.0, s) }
                    else { (0.0, 0.0) }
                },
                JustifyContent::SpaceEvenly => {
                    if item_count > 0 { let s = free_main / (item_count + 1) as f32; (s, s) }
                    else { (0.0, 0.0) }
                }
            };

            for i in line.start..line.end {
                let item = &mut items[i];
                
                item.main_offset = main_pos;
                main_pos += item.main_size + gap + main_spacing;

                match self.align_items {
                    AlignItems::Stretch => {
                        item.cross_offset = current_cross;
                        item.cross_size = line.cross_size;
                    },
                    AlignItems::FlexStart => {
                        item.cross_offset = current_cross;
                    },
                    AlignItems::FlexEnd => {
                        item.cross_offset = current_cross + line.cross_size - item.cross_size;
                    },
                    AlignItems::Center => {
                        item.cross_offset = current_cross + (line.cross_size - item.cross_size) / 2.0;
                    }
                }
            }
            
            if main_pos - gap - main_spacing > total_used_main {
                total_used_main = main_pos - gap - main_spacing;
            }

            current_cross += line.cross_size + cross_gap + cross_spacing;
        }

        for item in &items {
            let (x, y, w, h) = if is_horiz {
                (inner_rect.left as f32 + item.main_offset, 
                 inner_rect.top as f32 + item.cross_offset, 
                 item.main_size, 
                 item.cross_size)
            } else {
                (inner_rect.left as f32 + item.cross_offset, 
                 inner_rect.top as f32 + item.main_offset, 
                 item.cross_size, 
                 item.main_size)
            };

            let child_rect = RECT {
                left: x as i32,
                top: y as i32,
                right: (x + w) as i32,
                bottom: (y + h) as i32,
            };

            if let Some(hwnd) = item.node.hwnd {
                if !hwnd.is_null() {
                    unsafe {
                        // ComboBox HACK: Dropdown requires huge height (cy).
                        // Visual layout logic uses standard height (e.g. 24px), but Win32 needs e.g. 200px.
                        let mut final_h = child_rect.bottom - child_rect.top;
                        
                        let mut class_name = [0u16; 16];
                        let len = crate::types::GetClassNameW(hwnd, class_name.as_mut_ptr(), 16);
                        if len > 0 {
                            let name = String::from_utf16_lossy(&class_name[..len as usize]);
                            if name.eq_ignore_ascii_case("COMBOBOX") {
                                final_h = crate::ui::theme::scale(200); // Override cy to prevent clipped dropdown
                            }
                        }

                        if !hdwp.is_null() {
                            hdwp = DeferWindowPos(
                                hdwp,
                                hwnd,
                                std::ptr::null_mut(),
                                child_rect.left,
                                child_rect.top,
                                child_rect.right - child_rect.left,
                                final_h,
                                SWP_NOZORDER | SWP_NOACTIVATE
                            );
                        } else {
                            crate::types::SetWindowPos(
                                hwnd,
                                std::ptr::null_mut(),
                                child_rect.left,
                                child_rect.top,
                                child_rect.right - child_rect.left,
                                final_h,
                                SWP_NOZORDER | SWP_NOACTIVATE
                            );
                        }
                    }
                }
            }

            if !item.node.children.is_empty() {
                unsafe {
                    let (_used, new_hdwp) = item.node.apply_layout_internal(child_rect, hdwp);
                    hdwp = new_hdwp;
                }
            }
        }

        ((total_used_main as i32) + (padding * 2), hdwp)
    }
}