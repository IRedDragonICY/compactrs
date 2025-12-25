use crate::types::*;

#[derive(Clone, Copy, Debug)]
pub enum SizePolicy {
    Fixed(i32),      // Exact pixel width/height
    Flex(f32),       // Proportional weight (e.g., 1.0, 2.0) to fill remaining space
    Auto,            // Auto-size based on content (heuristic)
}

#[derive(Clone, Copy, Debug)]
pub enum Direction {
    Horizontal,
    Vertical,
}

pub struct LayoutItem {
    pub hwnd: HWND,
    pub policy: SizePolicy,
}

/// A node in the layout tree. Can be a leaf (Control) or a branch (Container).
pub struct LayoutNode {
    pub hwnd: Option<HWND>,
    pub policy: SizePolicy,
    pub children: Vec<LayoutNode>,
    pub direction: Direction,
    pub padding: i32,
    pub gap: i32,
}

impl LayoutNode {
    pub fn new_container(direction: Direction, padding: i32, gap: i32) -> Self {
        Self {
            hwnd: None,
            policy: SizePolicy::Flex(1.0),
            children: Vec::new(),
            direction,
            padding,
            gap,
        }
    }

    pub fn new_leaf(hwnd: HWND, policy: SizePolicy) -> Self {
        Self {
            hwnd: Some(hwnd),
            policy,
            children: Vec::new(),
            direction: Direction::Horizontal, // Irrelevant for leaf
            padding: 0,
            gap: 0,
        }
    }

    pub fn add_child(&mut self, child: LayoutNode) -> &mut Self {
        self.children.push(child);
        self
    }

    // ========== Fluent Builder API ==========
    
    /// Create a horizontal row container
    pub fn row(padding: i32, gap: i32) -> Self {
        Self::new_container(Direction::Horizontal, padding, gap)
    }
    
    /// Create a vertical column container
    pub fn col(padding: i32, gap: i32) -> Self {
        Self::new_container(Direction::Vertical, padding, gap)
    }
    
    /// Add a child node (chainable)
    pub fn with_child(mut self, child: LayoutNode) -> Self {
        self.children.push(child);
        self
    }
    
    /// Set the size policy for this node (container or leaf)
    pub fn with_policy(mut self, policy: SizePolicy) -> Self {
        self.policy = policy;
        self
    }
    
    /// Add a fixed-size control (chainable)
    pub fn with(mut self, hwnd: HWND, policy: SizePolicy) -> Self {
        self.children.push(Self::new_leaf(hwnd, policy));
        self
    }
    
    /// Add a fixed-size spacer (chainable)
    pub fn spacer(mut self, size: i32) -> Self {
        self.children.push(Self::new_leaf(std::ptr::null_mut(), SizePolicy::Fixed(size)));
        self
    }
    
    /// Add a flexible spacer that fills remaining space (chainable)
    pub fn flex_spacer(mut self) -> Self {
        self.children.push(Self::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
        self
    }
    
    /// Shorthand for Fixed policy
    pub fn fixed(hwnd: HWND, size: i32) -> Self {
        Self::new_leaf(hwnd, SizePolicy::Fixed(size))
    }
    
    /// Shorthand for Flex policy  
    pub fn flex(hwnd: HWND, weight: f32) -> Self {
        Self::new_leaf(hwnd, SizePolicy::Flex(weight))
    }
    
    pub fn calculate_layout(&self, rect: RECT) -> i32 {
        unsafe { self.apply_layout(rect) }
    }

    pub unsafe fn apply_layout(&self, rect: RECT) -> i32 {
        // Validation
        if rect.right <= rect.left || rect.bottom <= rect.top { return 0; }

        let _width = rect.right - rect.left;
        let _height = rect.bottom - rect.top;

        // Apply padding
        let inner_rect = RECT {
            left: rect.left + self.padding,
            top: rect.top + self.padding,
            right: rect.right - self.padding,
            bottom: rect.bottom - self.padding,
        };
        
        let inner_w = inner_rect.right - inner_rect.left;
        let inner_h = inner_rect.bottom - inner_rect.top;

        if inner_w <= 0 || inner_h <= 0 { return 0; }

        let used = match self.direction {
            Direction::Horizontal => unsafe { self.layout_linear(inner_rect, inner_w, inner_h, true) },
            Direction::Vertical => unsafe { self.layout_linear(inner_rect, inner_h, inner_w, false) },
        };
        
        // Return total used including padding
        used + (self.padding * 2)
    }

    unsafe fn layout_linear(&self, rect: RECT, major_size: i32, minor_size: i32, is_horizontal: bool) -> i32 {
        let count = self.children.len();
        if count == 0 { return 0; }

        let total_gaps = if count > 1 { (count as i32 - 1) * self.gap } else { 0 };
        let mut total_fixed = 0;
        let mut total_flex = 0.0;

        // Pass 1: Measure
        for child in &self.children {
            match child.policy {
                SizePolicy::Fixed(s) => total_fixed += s,
                SizePolicy::Flex(w) => total_flex += w,
                SizePolicy::Auto => {
                    // Heuristic: If leaf, try to guess size? For now treat as Fixed(standard)
                    // In a real engine we'd measure. Let's fallback to Fixed(100) or similar context-aware?
                    // Or treat as Flex(0.0) -> minimal? 
                    // Let's implement Auto as a "content-fit" if possible, else Fixed default.
                    // For buttons: ~80-100. For labels: ~text len.
                    // Simplification: Auto = Fixed(0) + Expand? No.
                    // Let's map Auto to Fixed(Size) where Size is estimated.
                    // For this refactor, we'll try to rely on Fixed/Flex mostly.
                    // Fallback to 24px for height, 100px for width?
                    if is_horizontal { total_fixed += 100; } else { total_fixed += 24; }
                }
            }
        }

        let remaining = major_size - total_fixed - total_gaps;
        let flex_unit = if total_flex > 0.0 && remaining > 0 {
            remaining as f32 / total_flex
        } else {
            0.0
        };

        let mut current_pos = if is_horizontal { rect.left } else { rect.top };
        
        // Pass 2: Position
        for child in &self.children {
            let item_major = match child.policy {
                SizePolicy::Fixed(s) => s,
                SizePolicy::Flex(w) => (w * flex_unit) as i32,
                SizePolicy::Auto => if is_horizontal { 100 } else { 24 }, 
            };
            
            // Minor axis: Stretch children to fill unless they have their own size policy?
            // "StackPanel" usually stretches cross-axis.
            let item_minor = minor_size; 

            let child_rect = if is_horizontal {
                RECT {
                    left: current_pos,
                    top: rect.top,
                    right: current_pos + item_major,
                    bottom: rect.top + item_minor,
                }
            } else {
                RECT {
                    left: rect.left,
                    top: current_pos,
                    right: rect.left + item_minor,
                    bottom: current_pos + item_major,
                }
            };
            
            // If it's a leaf with HWND, set window pos
            if let Some(hwnd) = child.hwnd {
                     unsafe {
                         SetWindowPos(
                             hwnd, 
                             std::ptr::null_mut(), 
                             child_rect.left, 
                             child_rect.top, 
                             child_rect.right - child_rect.left, 
                             child_rect.bottom - child_rect.top, 
                             SWP_NOZORDER | SWP_NOACTIVATE
                         );
                     }
            }
            
            // Recurse if it's a container (has children)
            if !child.children.is_empty() {
                unsafe { child.apply_layout(child_rect); }
            }

            current_pos += item_major + self.gap;
        }
        
        // Return used major size (current_pos - start)
        current_pos - if is_horizontal { rect.left } else { rect.top } - self.gap // subtract last gap
    }
}


