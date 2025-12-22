//! GUI Components module.
//!
//! Contains modular, reusable UI components following the Component pattern.
//! Each component encapsulates its own creation, layout, and theming logic.

pub mod base;
pub mod file_list;
pub mod status_bar;
pub mod action_panel;
pub mod header_panel;
pub mod search_panel; // New

pub use base::Component;
pub use file_list::FileListView;
pub use status_bar::{StatusBar, StatusBarIds};
pub use action_panel::{ActionPanel, ActionPanelIds};
pub use header_panel::{HeaderPanel, HeaderPanelIds};
pub use search_panel::{SearchPanel, SearchPanelIds};
