use windows::Win32::Foundation::HWND;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, atomic::AtomicBool};
use crate::engine::wof::WofAlgorithm;
use crate::config::AppConfig;
use crate::gui::components::FileListView;

/// App Theme Preference
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AppTheme {
    System,
    Dark,
    Light,
}

impl Default for AppTheme {
    fn default() -> Self {
        AppTheme::System
    }
}

/// Message types for UI updates
pub enum UiMessage {
    Progress(u64, u64), // current, total (global progress)
    BatchItemStatus(u32, BatchStatus),   // Individual item status update
    BatchItemProgress(u32, u64, u64),    // Individual item progress (id, current, total)
    /// Row update for ListView: (row_index, progress_str, status_str, size_after_str)
    RowUpdate(i32, String, String, String),
    Log(String),
    Status(String),
    Finished,
    /// Single item finished: (row_index, status, size_after)
    ItemFinished(i32, String, String),
    /// Item analyzed (id, logical_size, disk_size, algorithm)
    BatchItemAnalyzed(u32, u64, u64, Option<WofAlgorithm>),
    Error(String),
}

/// Action to perform on a batch item
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BatchAction {
    Compress,
    Decompress,
}

impl Default for BatchAction {
    fn default() -> Self {
        BatchAction::Compress
    }
}

/// Status of a batch item
#[derive(Clone, Debug, PartialEq)]
pub enum BatchStatus {
    Pending,
    Processing,
    Complete,
    Error(String),
}

impl Default for BatchStatus {
    fn default() -> Self {
        BatchStatus::Pending
    }
}

/// Represents an item in the batch processing queue
#[derive(Clone, Debug)]
pub struct BatchItem {
    pub id: u32,                    // Unique identifier
    pub path: String,               // Folder path
    pub algorithm: WofAlgorithm,    // Selected compression algorithm
    pub action: BatchAction,        // Compress or Decompress
    pub status: BatchStatus,        // Pending, Processing, Complete, Error
    pub progress: (u64, u64),       // (current, total) files
    pub cancel_token: Option<Arc<AtomicBool>>, // Cancellation token for this item
}

impl BatchItem {
    pub fn new(id: u32, path: String) -> Self {
        Self {
            id,
            path,
            algorithm: WofAlgorithm::Xpress8K, // Default
            action: BatchAction::Compress,
            status: BatchStatus::Pending,
            progress: (0, 0),
            cancel_token: None,
        }
    }
}

/// UI Control handles
pub struct Controls {
    pub file_list: FileListView,
    pub btn_scan: HWND,
    pub btn_compress: HWND,
    pub btn_decompress: HWND,
    pub combo_algo: HWND,
    pub static_text: HWND,
    pub progress_bar: HWND,
    pub btn_cancel: HWND,
    pub btn_settings: HWND,
    pub btn_about: HWND,
    pub btn_console: HWND,
    pub btn_force: HWND,
}

/// Application state
pub struct AppState {
    // Legacy - single folder (will be phased out)
    pub current_folder: Option<String>,
    
    // New batch processing state
    pub batch_items: Vec<BatchItem>,
    pub next_item_id: u32,
    
    // UI and communication
    pub controls: Option<Controls>,
    pub tx: Sender<UiMessage>,
    pub rx: Receiver<UiMessage>,
    pub cancel_flag: Arc<AtomicBool>,
    
    // Settings
    pub config: AppConfig,
    pub theme: AppTheme,
    
    // Console
    pub logs: Vec<String>,
    pub console_hwnd: Option<HWND>,
    pub force_compress: bool,
    pub enable_force_stop: bool,
    pub taskbar: Option<super::taskbar::TaskbarProgress>,
}

impl AppState {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let config = AppConfig::load();
        Self {
            current_folder: None,
            batch_items: Vec::new(),
            next_item_id: 1,
            controls: None,
            tx,
            rx,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            config,
            theme: config.theme,
            logs: Vec::new(),
            console_hwnd: None,
            force_compress: config.force_compress,
            enable_force_stop: config.enable_force_stop,
            taskbar: None,
        }
    }
    
    /// Add a new batch item and return its ID
    pub fn add_batch_item(&mut self, path: String) -> u32 {
        let id = self.next_item_id;
        self.next_item_id += 1;
        self.batch_items.push(BatchItem::new(id, path));
        id
    }
    
    /// Remove a batch item by ID
    pub fn remove_batch_item(&mut self, id: u32) -> bool {
        if let Some(pos) = self.batch_items.iter().position(|item| item.id == id) {
            self.batch_items.remove(pos);
            true
        } else {
            false
        }
    }
    
    /// Get a mutable reference to a batch item by ID
    pub fn get_batch_item_mut(&mut self, id: u32) -> Option<&mut BatchItem> {
        self.batch_items.iter_mut().find(|item| item.id == id)
    }
    
    /// Update the algorithm for a specific batch item
    pub fn set_item_algorithm(&mut self, id: u32, algorithm: WofAlgorithm) {
        if let Some(item) = self.get_batch_item_mut(id) {
            item.algorithm = algorithm;
        }
    }
    
    /// Update the action for a specific batch item
    pub fn set_item_action(&mut self, id: u32, action: BatchAction) {
        if let Some(item) = self.get_batch_item_mut(id) {
            item.action = action;
        }
    }
    
    /// Clear all batch items
    pub fn clear_batch(&mut self) {
        self.batch_items.clear();
    }
}
