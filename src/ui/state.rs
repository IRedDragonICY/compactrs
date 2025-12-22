use windows_sys::Win32::Foundation::HWND;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, atomic::{AtomicU8, AtomicU64}};
use std::collections::HashMap;
use crate::engine::wof::WofAlgorithm;
use crate::config::AppConfig;
use crate::ui::components::{FileListView, Component};
use crate::engine::worker::scan_path_streaming;
use crate::utils::to_wstring;
use crate::logger::LogEntry;
use std::thread;

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
    
    /// Raw row progress: (row_index, current, total, bytes_processed)
    RowProgress(i32, u64, u64, u64),
    
    /// Incremental scan progress: (id, logical_size, disk_size, file_count)
    ScanProgress(u32, u64, u64, u64),
    
    Log(LogEntry),
    
    /// Status text message (UTF-8)
    StatusText(String),
    
    Finished,
    
    /// Single item finished: (row_index, final_size_bytes, total_count, final_state)
    RowFinished(i32, u64, u64, crate::engine::wof::CompressionState),
    
    /// Item analyzed (id, logical_size, disk_size, compression_state)
    BatchItemAnalyzed(u32, u64, u64, crate::engine::wof::CompressionState),
    
    /// Estimated size update: (id, algorithm, estimated_size)
    UpdateEstimate(u32, WofAlgorithm, u64),
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

/// Filter column type
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterColumn {
    Path,
    Status,
}

/// Search State
pub struct SearchState {
    pub text: String,
    pub filter_column: FilterColumn,
    pub algorithm_filter: Option<crate::engine::wof::WofAlgorithm>,
    pub size_filter: i32, // 0 = All, 1 = Small, 2 = Large (placeholder for now)
    pub use_regex: bool,
    pub case_sensitive: bool,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            text: String::new(),
            filter_column: FilterColumn::Path,
            algorithm_filter: None,
            size_filter: 0,
            use_regex: false,
            case_sensitive: false,
        }
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
    pub estimated_size: u64,        // Current estimated compressed size
    /// Cache of estimated sizes per algorithm (avoids re-calculation)
    pub estimation_cache: HashMap<u32, u64>,
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
            estimated_size: 0,
            estimation_cache: HashMap::new(),
        }
    }
    
    /// Get cached estimation for an algorithm, if available
    pub fn get_cached_estimate(&self, algo: WofAlgorithm) -> Option<u64> {
        self.estimation_cache.get(&(algo as u32)).copied()
    }
    
    /// Cache an estimation result for an algorithm
    pub fn cache_estimate(&mut self, algo: WofAlgorithm, size: u64) {
        self.estimation_cache.insert(algo as u32, size);
        self.estimated_size = size;
    }
}

/// UI Control handles organized by component
pub struct Controls {
    pub file_list: FileListView,
    pub status_bar: crate::ui::components::StatusBar,
    pub action_panel: crate::ui::components::ActionPanel,
    pub header_panel: crate::ui::components::HeaderPanel,
    pub search_panel: crate::ui::components::SearchPanel,
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
        

        
        unsafe {
            // Get the cached app font
            let hfont = crate::ui::theme::get_app_font();
            
            // Update theme for each component
            self.status_bar.on_theme_change(is_dark);
            self.action_panel.on_theme_change(is_dark);
            self.header_panel.on_theme_change(is_dark);
            self.search_panel.on_theme_change(is_dark);
            self.file_list.on_theme_change(is_dark);
            
            // Set fonts on all components
            self.status_bar.set_font(hfont);
            self.action_panel.set_font(hfont);
            self.header_panel.set_font(hfont);
            self.search_panel.set_font(hfont);
            
            // Set font on ListView
            // Set font on ListView
            self.file_list.set_font(hfont);
            
            // Apply subclass for header theming
            self.file_list.apply_subclass(main_hwnd);
        }
    }
}

/// Application state
pub struct AppState {
    // New batch processing state
    pub batch_items: Vec<BatchItem>,
    pub processing_queue: Vec<usize>, // Queue of item indices waiting for processing
    
    // Search State
    pub search_state: SearchState,

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
    pub logs: std::collections::VecDeque<LogEntry>,
    pub console_hwnd: Option<HWND>,
    pub force_compress: bool,
    pub enable_force_stop: bool,
    pub low_power_mode: bool,
    pub taskbar: Option<super::taskbar::TaskbarProgress>,
    
    // Sorting state
    pub sort_column: i32,
    pub sort_ascending: bool,

    // IPC state
    pub ipc_active: bool,
    pub pending_ipc_ids: Vec<u32>,

    // Global Progress
    pub global_progress_current: Arc<AtomicU64>,
    pub global_progress_total: Arc<AtomicU64>,
    
    // File Lock Dialog State
    pub active_lock_dialog: Option<String>,
    pub ignored_lock_processes: std::collections::HashSet<String>,
}

impl AppState {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let config = AppConfig::load();
        
        // Apply Eco Mode immediately if saved in config
        if config.low_power_mode {
            crate::engine::power::set_process_eco_mode(true);
        }

        Self {
            batch_items: Vec::new(),
            processing_queue: Vec::new(),
            search_state: SearchState::default(),
            next_item_id: 1,
            controls: None,
            tx,
            rx,
            global_state: Arc::new(AtomicU8::new(ProcessingState::Idle as u8)),
            config,
            theme: config.theme,
            logs: std::collections::VecDeque::with_capacity(1000),
            console_hwnd: None,
            force_compress: config.force_compress,
            enable_force_stop: config.enable_force_stop,
            low_power_mode: config.low_power_mode,
            taskbar: None,
            sort_column: -1,
            sort_ascending: true,
            ipc_active: false,
            pending_ipc_ids: Vec::new(),
            global_progress_current: Arc::new(AtomicU64::new(0)),
            global_progress_total: Arc::new(AtomicU64::new(0)),
            active_lock_dialog: None,
            ignored_lock_processes: std::collections::HashSet::new(),
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

    /// Ingest a list of file paths, adding them to the batch and starting analysis
    pub fn ingest_paths(&mut self, paths: Vec<String>) {
        let mut items_to_analyze = Vec::new();

        for path in paths {
            // Resolve shortcut if applicable
            let path = crate::com::shell_link::resolve_shortcut(&path).unwrap_or(path);

            // Filter duplicates
            if !self.batch_items.iter().any(|item| item.path == path) {
                let id = self.add_batch_item(path.clone());
                
                // Get the default algorithm for estimation
                let algo = self.batch_items.iter().find(|i| i.id == id)
                    .map(|i| i.algorithm)
                    .unwrap_or(WofAlgorithm::Xpress8K);
                
                // Update UI immediately
                if let Some(ctrls) = &self.controls {
                     if let Some(batch_item) = self.batch_items.iter().find(|i| i.id == id) {
                         ctrls.file_list.add_item(
                             id, 
                             batch_item, 
                             &to_wstring("Calculating..."), 
                             &to_wstring("Calculating..."),
                             &to_wstring("Estimating..."),
                             crate::engine::wof::CompressionState::None
                         );
                     }
                }
                items_to_analyze.push((id, path, algo));
            }
        }
        
        if items_to_analyze.is_empty() { return; }

        let tx = self.tx.clone();
        
        // Spawn analysis thread
        thread::spawn(move || {
            for (id, path, algo) in items_to_analyze {
                 // Single-pass scan with streaming updates
                 let metrics = scan_path_streaming(id, &path, tx.clone(), None);
                 let _ = tx.send(UiMessage::BatchItemAnalyzed(id, metrics.logical_size, metrics.disk_size, metrics.compression_state));
                 
                 // Estimate compressed size
                 let estimated = crate::engine::estimator::estimate_path(&path, algo);
                 let _est_str = crate::utils::format_size(estimated);
                 let _ = tx.send(UiMessage::UpdateEstimate(id, algo, estimated));
            }
            let _ = tx.send(UiMessage::StatusText("Ready.".to_string()));
        });
    }
}
