use crate::types::*;
use crate::ui::layout::{LayoutNode, Direction, SizePolicy};
use crate::ui::builder::ControlBuilder;
use crate::ui::subclass::apply_theme_to_control;


// A simpler "Tree Builder" that doesn't hold large state
pub struct DeclarativeContext {
    pub parent_hwnd: HWND,
    pub is_dark: bool,
    pub h_font: HFONT,
}

impl DeclarativeContext {
    pub fn new(parent: HWND, is_dark: bool, h_font: HFONT) -> Self {
        Self { parent_hwnd: parent, is_dark, h_font }
    }

    pub fn vertical<F>(&self, padding: i32, gap: i32, content: F) -> LayoutNode 
    where F: FnOnce(&mut ContainerBuilder) {
        let mut node = LayoutNode::new_container(Direction::Vertical, padding, gap);
        let mut builder = ContainerBuilder { 
            ctx: self, 
            node: &mut node 
        };
        content(&mut builder);
        node
    }

    pub fn horizontal<F>(&self, padding: i32, gap: i32, content: F) -> LayoutNode 
    where F: FnOnce(&mut ContainerBuilder) {
        let mut node = LayoutNode::new_container(Direction::Horizontal, padding, gap);
        let mut builder = ContainerBuilder { 
            ctx: self, 
            node: &mut node 
        };
        content(&mut builder);
        node
    }
}

pub struct ContainerBuilder<'a> {
    ctx: &'a DeclarativeContext,
    node: &'a mut LayoutNode,
}

impl<'a> ContainerBuilder<'a> {
    pub fn add_child(&mut self, child: LayoutNode) {
        self.node.add_child(child);
    }

    // --- Helpers for Controls ---

    pub fn label(&mut self, text: &str, policy: SizePolicy) -> HWND {
        unsafe {
            let h = ControlBuilder::new(self.ctx.parent_hwnd, 0)
                .label(false)
                .text(text)
                .font(self.ctx.h_font)
                .dark_mode(self.ctx.is_dark)
                .build();
            apply_theme_to_control(h, self.ctx.is_dark);
            
            // Subclassing not strictly needed for statics usually, but consistent
            // subclass_control(h); 
            
            self.node.add_child(LayoutNode::new_leaf(h, policy));
            h
        }
    }
    
    pub fn label_w(&mut self, text: &[u16], policy: SizePolicy) -> HWND {
        unsafe {
            let h = ControlBuilder::new(self.ctx.parent_hwnd, 0)
                .label(false)
                .text_w(text)
                .font(self.ctx.h_font)
                .dark_mode(self.ctx.is_dark)
                .build();
            apply_theme_to_control(h, self.ctx.is_dark);
            self.node.add_child(LayoutNode::new_leaf(h, policy));
            h
        }
    }

    pub fn button(&mut self, id: u16, text: &str, policy: SizePolicy) -> HWND {
        unsafe {
             let h = ControlBuilder::new(self.ctx.parent_hwnd, id)
                .button()
                .text(text)
                .font(self.ctx.h_font)
                .dark_mode(self.ctx.is_dark)
                .build();
             apply_theme_to_control(h, self.ctx.is_dark);
             self.node.add_child(LayoutNode::new_leaf(h, policy));
             h
        }
    }
    
    pub fn button_w(&mut self, id: u16, text: &[u16], policy: SizePolicy) -> HWND {
        unsafe {
             let h = ControlBuilder::new(self.ctx.parent_hwnd, id)
                .button()
                .text_w(text)
                .font(self.ctx.h_font)
                .dark_mode(self.ctx.is_dark)
                .build();
             apply_theme_to_control(h, self.ctx.is_dark);
             self.node.add_child(LayoutNode::new_leaf(h, policy));
             h
        }
    }

    pub fn checkbox(&mut self, id: u16, text: &str, checked: bool, policy: SizePolicy) -> HWND {
        unsafe {
            let h = ControlBuilder::new(self.ctx.parent_hwnd, id)
               .checkbox()
               .text(text)
               .checked(checked)
               .font(self.ctx.h_font)
               .dark_mode(self.ctx.is_dark)
               .build();
            apply_theme_to_control(h, self.ctx.is_dark);
            self.node.add_child(LayoutNode::new_leaf(h, policy));
            h
        }
    }
    
    pub fn checkbox_w(&mut self, id: u16, text: &[u16], checked: bool, policy: SizePolicy) -> HWND {
        unsafe {
            let h = ControlBuilder::new(self.ctx.parent_hwnd, id)
               .checkbox()
               .text_w(text)
               .checked(checked)
               .font(self.ctx.h_font)
               .dark_mode(self.ctx.is_dark)
               .build();
            apply_theme_to_control(h, self.ctx.is_dark);
            self.node.add_child(LayoutNode::new_leaf(h, policy));
            h
        }
    }

    pub fn slider(&mut self, id: u16, min: u32, max: u32, pos: u32, policy: SizePolicy) -> HWND {
        unsafe {
             let h = ControlBuilder::new(self.ctx.parent_hwnd, id)
                .trackbar()
                .dark_mode(self.ctx.is_dark)
                .build();
             
             use crate::ui::wrappers::Trackbar;
             let tb = Trackbar::new(h);
             tb.set_range(min, max);
             tb.set_pos(pos);
             
             // Trackbars are finicky with theming, strict subclass might be needed?
             apply_theme_to_control(h, self.ctx.is_dark);
             
             self.node.add_child(LayoutNode::new_leaf(h, policy));
             h
        }
    }

    pub fn input(&mut self, id: u16, text: &str, style: u32, policy: SizePolicy) -> HWND {
         unsafe {
             let h = ControlBuilder::new(self.ctx.parent_hwnd, id)
                .edit()
                .text(text)
                .style(style)
                .font(self.ctx.h_font)
                .dark_mode(self.ctx.is_dark)
                .build();
             apply_theme_to_control(h, self.ctx.is_dark);
             self.node.add_child(LayoutNode::new_leaf(h, policy));
             h
         }
    }
    
    pub fn combobox(&mut self, id: u16, items: &[&str], selected_idx: u32, policy: SizePolicy) -> HWND {
        unsafe {
            let h = ControlBuilder::new(self.ctx.parent_hwnd, id)
               .combobox()
               .dark_mode(self.ctx.is_dark)
               .build();
            
            use crate::ui::wrappers::ComboBox;
            let cb = ComboBox::new(h);
            for item in items {
                cb.add_string(item);
            }
            cb.set_selected_index(selected_idx as i32);
            
            apply_theme_to_control(h, self.ctx.is_dark);
            self.node.add_child(LayoutNode::new_leaf(h, policy));
            h
        }
    }
    
    // Nesting
    pub fn row<F>(&mut self, gap: i32, content: F) 
    where F: FnOnce(&mut ContainerBuilder) {
        let mut sub = LayoutNode::new_container(Direction::Horizontal, 0, gap);
        {
            let mut sub_builder = ContainerBuilder { ctx: self.ctx, node: &mut sub };
            content(&mut sub_builder);
        }
        self.node.add_child(sub);
    }
    
    pub fn col<F>(&mut self, gap: i32, content: F) 
    where F: FnOnce(&mut ContainerBuilder) {
        self.col_with_policy(gap, SizePolicy::Flex(1.0), content);
    }

    pub fn row_with_policy<F>(&mut self, gap: i32, policy: SizePolicy, content: F) 
    where F: FnOnce(&mut ContainerBuilder) {
        let mut sub = LayoutNode::new_container(Direction::Horizontal, 0, gap);
        sub.policy = policy;
        {
            let mut sub_builder = ContainerBuilder { ctx: self.ctx, node: &mut sub };
            content(&mut sub_builder);
        }
        self.node.add_child(sub);
    }
    
    pub fn col_with_policy<F>(&mut self, gap: i32, policy: SizePolicy, content: F) 
    where F: FnOnce(&mut ContainerBuilder) {
        let mut sub = LayoutNode::new_container(Direction::Vertical, 0, gap);
        sub.policy = policy;
        {
            let mut sub_builder = ContainerBuilder { ctx: self.ctx, node: &mut sub };
            content(&mut sub_builder);
        }
        self.node.add_child(sub);
    }
}
