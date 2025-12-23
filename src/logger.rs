use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;
use std::sync::mpsc::Sender;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::ui::state::UiMessage;

// Bitflags for log levels
pub const LOG_LEVEL_ERROR: u8 = 1;
pub const LOG_LEVEL_WARN: u8 = 2;
pub const LOG_LEVEL_INFO: u8 = 4;
pub const LOG_LEVEL_TRACE: u8 = 8;

#[allow(dead_code)]
pub const LOG_LEVEL_NONE: u8 = 0;
#[allow(dead_code)]
pub const LOG_LEVEL_ALL: u8 = LOG_LEVEL_ERROR | LOG_LEVEL_WARN | LOG_LEVEL_INFO | LOG_LEVEL_TRACE;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error = 1,
    Warning = 2,
    Info = 4,
    Trace = 8,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Warning => "WARN",
            LogLevel::Info => "INFO",
            LogLevel::Trace => "TRACE",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: LogLevel,
    pub message: String,
    pub thread_id: u32,
}

// Global state
pub static GLOBAL_LOG_LEVEL: AtomicU8 = AtomicU8::new(LOG_LEVEL_ERROR | LOG_LEVEL_WARN | LOG_LEVEL_INFO); // Default safe mask
pub static GLOBAL_LOG_SENDER: OnceLock<Sender<UiMessage>> = OnceLock::new();

/// Initialize the global logger with the UI channel sender
pub fn init_logger(tx: Sender<UiMessage>) {
    let _ = GLOBAL_LOG_SENDER.set(tx);
}

/// Set the global log level mask
pub fn set_log_level(mask: u8) {
    GLOBAL_LOG_LEVEL.store(mask, Ordering::Relaxed);
}

/// Internal function to log a message if level is enabled
pub fn log_internal(level: LogLevel, msg: String) {
    // 1. Atomic check (Zero-cost if disabled)
    let current_mask = GLOBAL_LOG_LEVEL.load(Ordering::Relaxed);
    if (current_mask & (level as u8)) == 0 {
        return;
    }

    // 2. Construct entry
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Manual binding for GetCurrentThreadId
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }
    let thread_id = unsafe { GetCurrentThreadId() };

    let entry = LogEntry {
        timestamp,
        level,
        message: msg,
        thread_id,
    };

    // 3. Send to UI
    if let Some(tx) = GLOBAL_LOG_SENDER.get() {
        let _ = tx.send(UiMessage::Log(entry));
    }
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::logger::log_internal($crate::logger::LogLevel::Error, format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::logger::log_internal($crate::logger::LogLevel::Warning, format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logger::log_internal($crate::logger::LogLevel::Info, format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        $crate::logger::log_internal($crate::logger::LogLevel::Trace, format!($($arg)*))
    };
}
