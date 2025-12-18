use windows_sys::Win32::Foundation::HWND;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, atomic::AtomicU8};
use crate::engine::wof::WofAlgorithm;
use crate::config::AppConfig;
use crate::ui::components::{FileListView, Component};

/// Processing state for items (state machine)
/// Used with AtomicU8 for thread-safe state transitions
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessingState {
    /// Not started or finished processing
    Idle = 0,
    /// Actively processing files
    Running = 1,
    /// Paused by user, can be resumed
    Paused = 2,
    /// Cancelled/stopped, cannot be resumed
    Stopped = 3,
}

impl ProcessingState {
    /// Convert from raw u8 value (for AtomicU8 loads)
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => ProcessingState::Running,
            2 => ProcessingState::Paused,
            3 => ProcessingState::Stopped,
            _ => ProcessingState::Idle,
        }
    }
}

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
    /// Row update for ListView: (row_index, progress_wide, status_wide, size_after_wide)
    RowUpdate(i32, Vec<u16>, Vec<u16>, Vec<u16>),
    Log(Vec<u16>),
    Status(Vec<u16>),
    Finished,
    /// Single item finished: (row_index, status_wide, size_after_wide, final_state)
    ItemFinished(i32, Vec<u16>, Vec<u16>, crate::engine::wof::CompressionState),
    /// Item analyzed (id, logical_size, disk_size, compression_state, logical_str_wide, disk_str_wide)
    /// Note: I'm adding pre-formatted strings here to avoid re-formatting in UI thread if possible, 
    /// or just keeping numbers. Actually, let's keep numbers and format in UI? 
    /// User said: "Modify format_size to return Vec<u16> ... Refactor UI Logic ... Identify where we perform UI updates".
    /// If I format in worker, I send Vec<u16>.
    /// The original was: BatchItemAnalyzed(u32, u64, u64, ...).
    /// Let's keep numbers for BatchItemAnalyzed as they are stored in BatchItem? 
    /// Wait, BatchItem has progress(u64,u64). It doesn't store size strings.
    /// UI updates text.
    /// Let's stick to the current definition for BatchItemAnalyzed but remember that window.rs formats them.
    BatchItemAnalyzed(u32, u64, u64, crate::engine::wof::CompressionState),
    Error(Vec<u16>),
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
    pub state_flag: Option<Arc<AtomicU8>>, // Processing state
    // Added for sorting
    pub logical_size: u64,
    pub disk_size: u64,
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
            state_flag: None,
            logical_size: 0,
            disk_size: 0,
        }
    }
}

/// UI Control handles organized by component
pub struct Controls {
    pub file_list: FileListView,
    pub status_bar: crate::ui::components::StatusBar,
    pub action_panel: crate::ui::components::ActionPanel,
    pub header_panel: crate::ui::components::HeaderPanel,
}

impl Controls {
    /// Updates theme for all child components.
    ///
    /// This method coordinates theme updates across all UI components,
    /// including setting fonts and applying dark/light mode styles.
    ///
    /// # Arguments
    /// * `is_dark` - Whether dark mode is active
    /// * `main_hwnd` - Main window handle for ListView subclass
    ///
    /// # Safety
    /// Calls Win32 APIs for theme updates.
    pub unsafe fn update_theme(&mut self, is_dark: bool, main_hwnd: HWND) {
        use windows_sys::Win32::Foundation::{LPARAM, WPARAM};
        use windows_sys::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_SETFONT};
        
        unsafe {
            // Get the cached app font
            let hfont = crate::ui::theme::get_app_font();
            
            // Update theme for each component
            self.status_bar.on_theme_change(is_dark);
            self.action_panel.on_theme_change(is_dark);
            self.header_panel.on_theme_change(is_dark);
            self.file_list.on_theme_change(is_dark);
            
            // Set fonts on all components
            self.status_bar.set_font(hfont);
            self.action_panel.set_font(hfont);
            self.header_panel.set_font(hfont);
            
            // Set font on ListView
            let wparam = hfont as WPARAM;
            let lparam = 1 as LPARAM;
            SendMessageW(self.file_list.hwnd(), WM_SETFONT, wparam, lparam);
            
            // Apply subclass for header theming
            self.file_list.apply_subclass(main_hwnd);
        }
    }
}

/// Application state
pub struct AppState {
    // New batch processing state
    pub batch_items: Vec<BatchItem>,
    pub next_item_id: u32,
    
    // UI and communication
    pub controls: Option<Controls>,
    pub tx: Sender<UiMessage>,
    pub rx: Receiver<UiMessage>,
    pub global_state: Arc<AtomicU8>, // Global processing state (0=Idle, 1=Running, 2=Paused, 3=Stopped)
    
    // Settings
    pub config: AppConfig,
    pub theme: AppTheme,
    
    // Console
    pub logs: Vec<Vec<u16>>,
    pub console_hwnd: Option<HWND>,
    pub force_compress: bool,
    pub enable_force_stop: bool,
    pub taskbar: Option<super::taskbar::TaskbarProgress>,
    
    // Sorting state
    pub sort_column: i32,
    pub sort_ascending: bool,

    // IPC state
    pub ipc_active: bool,
    pub pending_ipc_ids: Vec<u32>,
}

impl AppState {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let config = AppConfig::load();
        Self {
            batch_items: Vec::new(),
            next_item_id: 1,
            controls: None,
            tx,
            rx,
            global_state: Arc::new(AtomicU8::new(ProcessingState::Idle as u8)),
            config,
            theme: config.theme,
            logs: Vec::new(),
            console_hwnd: None,
            force_compress: config.force_compress,
            enable_force_stop: config.enable_force_stop,
            taskbar: None,
            sort_column: -1,
            sort_ascending: true,
            ipc_active: false,
            pending_ipc_ids: Vec::new(),
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
