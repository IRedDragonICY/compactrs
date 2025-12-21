/* --- src/ui/layout.rs --- */

/// A simple helper for vertical stack layout.
/// Handles Y-positioning and spacing automatically.
pub struct LayoutContext {
    current_x: i32,
    current_y: i32,
    width: i32,
    padding: i32,
}

impl LayoutContext {
    /// Creates a new layout context.
    pub fn new(x: i32, y: i32, width: i32, padding: i32) -> Self {
        Self {
            current_x: x,
            current_y: y,
            width,
            padding,
        }
    }

    /// Adds a vertical space.
    pub fn add_space(&mut self, amount: i32) {
        self.current_y += amount;
    }

    /// Returns rect for a new row with specified height, then advances Y.
    /// Returns (x, y, w, h).
    pub fn row(&mut self, height: i32) -> (i32, i32, i32, i32) {
        let r = (self.current_x, self.current_y, self.width, height);
        self.current_y += height + self.padding;
        r
    }

    /// Returns rect for a new row with specified width and height.
    /// Does NOT advance Y automatically unless you want it to?
    /// Actually, normally we want to advance Y after a "row" is done.
    /// If we have multiple columns, we might need more complex logic.
    /// For now, settings.rs has some indentation but mostly vertical.
    
    /// Returns available width.
    pub fn get_width(&self) -> i32 {
        self.width
    }
    
    /// Indents the layout (increases X, decreases Width).
    pub fn indent(&mut self, amount: i32) {
        self.current_x += amount;
        self.width -= amount;
    }
    
    /// Outdents the layout (decreases X, increases Width).
    pub fn outdent(&mut self, amount: i32) {
        self.current_x -= amount;
        self.width += amount;
    }
    
    /// Current Y cursor.
    pub fn cursor_y(&self) -> i32 {
        self.current_y
    }
}
