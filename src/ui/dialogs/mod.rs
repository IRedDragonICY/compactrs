//! Dialog/modal window modules for CompactRS UI.
//!
//! This module groups all auxiliary window logic (modals/dialogs) into a dedicated namespace.

pub mod settings;
pub mod about;
pub mod shortcuts;
pub mod console;
pub mod force_stop;

// Flatten the API for consumers
pub use settings::show_settings_modal;
pub use about::show_about_modal;
pub use shortcuts::show_shortcuts_modal;
pub use console::{show_console_window, append_log_msg, close_console};
pub use force_stop::show_force_stop_dialog;
