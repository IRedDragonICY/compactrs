use crate::types::HWND;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, atomic::{AtomicU8, AtomicU64}};
use std::collections::HashMap;
use crate::engine::wof::{WofAlgorithm, CompressionState};
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
    
    /// Raw row progress: (id, current, total, bytes_processed)
    RowProgress(u32, u64, u64, u64),
    
    /// Incremental scan progress: (id, logical_size, disk_size, file_count)
    ScanProgress(u32, u64, u64, u64),
    
    Log(LogEntry),
    
    /// Status text message (UTF-16)
    StatusText(Vec<u16>),
    
    Finished,
    
    /// Single item finished: (id, final_size_bytes, total_count, final_state)
    RowFinished(u32, u64, u64, CompressionState),
    
    /// Item analyzed (id, logical_size, disk_size, compression_state)
    BatchItemAnalyzed(u32, u64, u64, CompressionState),
    
    /// Estimated size update: (id, algorithm, estimated_size)
    UpdateEstimate(u32, WofAlgorithm, u64),

    /// Watcher triggered processing: (Path, Algorithm)
    WatcherTrigger(String, WofAlgorithm),
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
    pub path_lower: String,         // Cached lowercased path for fast sorting/filtering
    pub algorithm: WofAlgorithm,    // Selected compression algorithm
    pub action: BatchAction,        // Compress or Decompress
    pub status: BatchStatus,        // Pending, Processing, Complete, Error
    pub status_override: Option<String>, // UI override text
    pub final_state: Option<CompressionState>, // Final detected state
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
        let path_lower = path.to_lowercase();
        Self {
            id,
            path,
            path_lower,
            algorithm: WofAlgorithm::Xpress8K, // Default
            action: BatchAction::Compress,
            status: BatchStatus::Pending,
            status_override: None,
            final_state: None,
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
    pub filtered_items: Vec<usize>, // maps UI row to batch_items index
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

    pub watcher_tasks: Arc<Mutex<Vec<crate::watcher_config::WatcherTask>>>,
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
            filtered_items: Vec::new(),
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
            watcher_tasks: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    /// Map BatchItem ID to UI row index
    pub fn find_ui_row_by_id(&self, id: u32) -> Option<i32> {
        self.filtered_items.iter().position(|&idx| self.batch_items.get(idx).map_or(false, |i| i.id == id)).map(|p| p as i32)
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
        self.filtered_items.clear();
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
                
                if let Some(item) = self.get_batch_item_mut(id) {
                     item.status_override = Some("Calculating...".to_string());
                }

                items_to_analyze.push((id, path, algo));
            }
        }
        
        if items_to_analyze.is_empty() { return; }

        unsafe { self.refresh_file_list(); }

        let tx = self.tx.clone();
        
        // Spawn analysis thread
        thread::spawn(move || {
            for (id, path, algo) in items_to_analyze {
                 // Single-pass scan with streaming updates
                 let metrics = scan_path_streaming(id, &path, tx.clone(), None);
                 let _ = tx.send(UiMessage::BatchItemAnalyzed(id, metrics.logical_size, metrics.disk_size, metrics.compression_state));
                 
                 // Estimate compressed size
                 let estimated = crate::engine::estimator::estimate_path(&path, algo);
                 let _ = tx.send(UiMessage::UpdateEstimate(id, algo, estimated));
            }
            let _ = tx.send(UiMessage::StatusText(to_wstring("Ready.")));
        });
    }

    pub fn sort_filtered_items(&mut self) {
        if self.sort_column < 0 { return; }
        let sort_col = self.sort_column;
        let asc = self.sort_ascending;
        let items = &self.batch_items;
        
        self.filtered_items.sort_by(|&a, &b| {
            let i1 = &items[a];
            let i2 = &items[b];
            let ord = match sort_col {
                0 => i1.path_lower.cmp(&i2.path_lower), 
                1 => {
                    let s1 = i1.final_state.unwrap_or(CompressionState::None);
                    let s2 = i2.final_state.unwrap_or(CompressionState::None);
                    let v1 = match s1 { CompressionState::None => 0, CompressionState::Specific(_) => 1, CompressionState::Mixed => 2 };
                    let v2 = match s2 { CompressionState::None => 0, CompressionState::Specific(_) => 1, CompressionState::Mixed => 2 };
                    v1.cmp(&v2)
                },
                2 => (i1.algorithm as u32).cmp(&(i2.algorithm as u32)),
                3 => (i1.action as u32).cmp(&(i2.action as u32)),
                4 => i1.logical_size.cmp(&i2.logical_size),
                5 => i1.estimated_size.cmp(&i2.estimated_size),
                6 => i1.disk_size.cmp(&i2.disk_size),
                7 => {
                    let r1 = crate::utils::calculate_saved_percentage(i1.logical_size, i1.disk_size);
                    let r2 = crate::utils::calculate_saved_percentage(i2.logical_size, i2.disk_size);
                    r1.partial_cmp(&r2).unwrap_or(std::cmp::Ordering::Equal)
                },
                8 => i1.progress.0.cmp(&i2.progress.0),
                9 => {
                    let p1 = match &i1.status { BatchStatus::Pending => 0, BatchStatus::Processing => 1, BatchStatus::Complete => 2, BatchStatus::Error(_) => 3 };
                    let p2 = match &i2.status { BatchStatus::Pending => 0, BatchStatus::Processing => 1, BatchStatus::Complete => 2, BatchStatus::Error(_) => 3 };
                    p1.cmp(&p2)
                },
                _ => std::cmp::Ordering::Equal,
            };
            if asc { ord } else { ord.reverse() }
        });
    }

    pub unsafe fn refresh_file_list(&mut self) {
        self.filtered_items.clear();

        // Extract required search state components without maintaining a strict reference 
        // to `self` to avoid borrow-checker conflicts later.
        let filter_text = self.search_state.text.trim().to_lowercase();
        let raw_text = self.search_state.text.clone();
        let use_custom_match = self.search_state.use_regex && !self.search_state.text.trim().is_empty();
        let filter_column = self.search_state.filter_column;
        let case_sensitive = self.search_state.case_sensitive;
        let algorithm_filter = self.search_state.algorithm_filter;
        let size_filter = self.search_state.size_filter;

        for (idx, item) in self.batch_items.iter().enumerate() {
            let text_match = if raw_text.is_empty() {
                true
            } else {
                let status_str;
                let haystack: &str = match filter_column {
                    FilterColumn::Path => item.path_lower.as_str(),
                    FilterColumn::Status => {
                        status_str = match item.status {
                            BatchStatus::Pending => "pending",
                            BatchStatus::Processing => "processing",
                            BatchStatus::Complete => "complete",
                            BatchStatus::Error(_) => "error",
                        };
                        status_str
                    },
                };
                
                if use_custom_match {
                    let pattern = &raw_text;
                    if case_sensitive {
                        let status_str_raw;
                        let haystack_raw: &str = match filter_column {
                            FilterColumn::Path => item.path.as_str(),
                            FilterColumn::Status => {
                                status_str_raw = match &item.status {
                                    BatchStatus::Pending => "Pending".to_string(),
                                    BatchStatus::Processing => "Processing".to_string(),
                                    BatchStatus::Complete => "Complete".to_string(),
                                    BatchStatus::Error(e) => format!("Error({})", e),
                                };
                                &status_str_raw
                            },
                        };
                        crate::utils::matcher::is_match(pattern, haystack_raw)
                    } else {
                       crate::utils::matcher::is_match(&filter_text, haystack)
                    }
                } else if case_sensitive {
                    let status_str_raw;
                    let haystack_raw: &str = match filter_column {
                        FilterColumn::Path => item.path.as_str(),
                        FilterColumn::Status => {
                            status_str_raw = match &item.status {
                                BatchStatus::Pending => "Pending".to_string(),
                                BatchStatus::Processing => "Processing".to_string(),
                                BatchStatus::Complete => "Complete".to_string(),
                                BatchStatus::Error(e) => format!("Error({})", e),
                            };
                            &status_str_raw
                        },
                    };
                    haystack_raw.contains(&raw_text)
                } else {
                    haystack.contains(&filter_text)
                }
            };
            
            if !text_match { continue; }
            
            if let Some(target_algo) = algorithm_filter {
                if item.algorithm != target_algo { continue; }
            }
            
            let size_match = match size_filter {
                1 => item.logical_size < 1_000_000, 
                2 => item.logical_size > 100_000_000, 
                _ => true,
            };
            if !size_match { continue; }

            self.filtered_items.push(idx);
        }
        
        let is_default_state = raw_text.trim().is_empty() 
            && algorithm_filter.is_none() 
            && size_filter == 0
            && !self.search_state.use_regex;

        self.sort_filtered_items();
        
        if let Some(ctrls) = &self.controls {
            let current_count = self.filtered_items.len();
            let total_count = self.batch_items.len();
            
            ctrls.file_list.set_item_count(current_count as i32);

            let msg = if is_default_state {
                if total_count == 0 {
                    crate::utils::to_wstring("Ready.")
                } else {
                    let prefix = crate::w!("Ready. ");
                    let count_str = unsafe { crate::utils::fmt_u32(total_count as u32) };
                    let suffix = crate::w!(" items loaded.");
                    crate::utils::concat_wstrings(&[prefix, &count_str, suffix])
                }
            } else {
                 if current_count == 0 {
                     crate::utils::to_wstring("No matching items found.")
                 } else {
                     let prefix = crate::w!("Found ");
                     let count_str = unsafe { crate::utils::fmt_u32(current_count as u32) };
                     let suffix = crate::w!(" matching items.");
                     crate::utils::concat_wstrings(&[prefix, &count_str, suffix])
                 }
            };
            
            crate::ui::wrappers::Label::new(ctrls.search_panel.results_hwnd()).set_text_w(&msg);
        }
    }
}