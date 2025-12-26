/* --- src/ui/handlers.rs --- */
#![allow(unsafe_op_in_unsafe_fn)]

use crate::types::*;
use std::thread;
use std::sync::atomic::Ordering;
use std::cmp::Ordering as CmpOrdering;

use crate::ui::state::{AppState, BatchAction, ProcessingState, BatchStatus};
use crate::ui::taskbar::TaskbarState;
use crate::ui::controls::*;
use crate::ui::wrappers::{Button, ComboBox, Label};
use crate::ui::theme;
use crate::engine::wof::WofAlgorithm;
use crate::engine::worker::batch_process_worker;
use crate::utils::{to_wstring, u64_to_wstring, concat_wstrings, reveal_path_in_explorer};
use crate::ui::file_dialog::{pick_files, pick_folder};

// --- Command Handlers ---

pub unsafe fn on_add_files(st: &mut AppState) {
    if let Ok(files) = pick_files() {
        st.ingest_paths(files);
        update_process_button_state(st);
    }
}

pub unsafe fn on_add_folder(st: &mut AppState) {
    if let Ok(folder) = pick_folder() {
        st.ingest_paths(vec![folder]);
        update_process_button_state(st);
    }
}

pub unsafe fn on_remove_selected(st: &mut AppState) {
    let mut selected_indices = if let Some(ctrls) = &st.controls {
        ctrls.file_list.get_selected_indices()
    } else { Vec::new() };
    
    // Sort reverse to remove correctly
    selected_indices.sort_by(|a, b| b.cmp(a));
    
    let ids_to_remove: Vec<u32> = selected_indices.iter()
        .filter_map(|&idx| st.batch_items.get(idx).map(|item| item.id))
        .collect();
    
    for id in ids_to_remove { 
        st.remove_batch_item(id); 
    }
    
    if let Some(ctrls) = &st.controls {
        for idx in selected_indices { 
            ctrls.file_list.remove_item(idx as i32); 
        }
    }
    
    update_process_button_state(st);
}

pub unsafe fn on_clear_all(st: &mut AppState) {
    // Note: Do not early return if batch_items is empty, 
    // because user might just want to clear logs/lock state.

    
    st.clear_batch();
    st.next_item_id = 1;

    if let Some(ctrls) = &st.controls {
        ctrls.file_list.clear_all();
    }
    
    st.global_progress_current.store(0, std::sync::atomic::Ordering::Relaxed);
    st.global_progress_total.store(0, std::sync::atomic::Ordering::Relaxed);
    
    // Reset Lock Dialog State
    st.active_lock_dialog = None;
    st.ignored_lock_processes.clear();
    
    // Clear Global Logs
    st.logs.clear();
    
    // Clear Console Window if open (Send IDC_BTN_CLEAR = 1003)
    if let Some(hwnd) = st.console_hwnd {
        SendMessageW(hwnd, WM_COMMAND, 1003, 0); 
    }
    
    update_process_button_state(st);
}

// Old update_process_button_state replaced by newer implementation below


pub unsafe fn on_process_all(st: &mut AppState, hwnd: HWND, is_auto_start: bool) {
    if st.batch_items.is_empty() {
        let w_info = to_wstring("Info");
        let w_msg = to_wstring("Add folders first!");
        MessageBoxW(hwnd, w_msg.as_ptr(), w_info.as_ptr(), MB_OK | MB_ICONINFORMATION);
        return;
    }
    
    if let Some(ctrls) = &st.controls {
        let mut indices = ctrls.file_list.get_selected_indices();
        
        // Auto-start logic implies processing "all" (or conceptually "all pending" which is usually everything at startup).
        // If user manually clicks "Process All" (or "Process Pending"), selection might be empty.
        
        if indices.is_empty() {
            // No selection: Filter for Pending/Error items
           indices = st.batch_items.iter().enumerate()
                .filter(|(_, item)| item.status == BatchStatus::Pending || matches!(item.status, BatchStatus::Error(_)))
                .map(|(i, _)| i)
                .collect();
                
            if indices.is_empty() && !is_auto_start {
                 // Nothing selected and nothing pending -> Show message or just return
                 // The button should be disabled ideally, but if forced:
                 let w_info = to_wstring("Info");
                 // Use a more generic message if we are in a weird state
                 let w_msg = to_wstring("No pending items to process.");
                 MessageBoxW(hwnd, w_msg.as_ptr(), w_info.as_ptr(), MB_OK | MB_ICONINFORMATION);
                 return;
            }
        }
        
        if indices.is_empty() { return; }

        start_processing(st, hwnd, indices);
    }
}

    pub unsafe fn start_processing(st: &mut AppState, hwnd: HWND, mut indices_to_process: Vec<usize>) {
    if indices_to_process.is_empty() { return; }

    // Reset Lock Dialog State for new run
    st.active_lock_dialog = None;
    st.ignored_lock_processes.clear();

    // Queue Logic
    let max = st.config.max_concurrent_items as usize;
    
    if max > 0 && indices_to_process.len() > max {
        let queued = indices_to_process.split_off(max);
        st.processing_queue = queued;
        // status bar update ("Queued X items") happens in internal or via internal's status msg
    } else {
        st.processing_queue.clear();
    }
    
    start_processing_internal(st, hwnd, indices_to_process);
}

pub unsafe fn start_processing_internal(st: &mut AppState, hwnd: HWND, indices_to_process: Vec<usize>) {
    if indices_to_process.is_empty() { return; }

    if let Some(ctrls) = &st.controls {
        // Read Global Settings
        let combo = ComboBox::new(ctrls.action_panel.combo_hwnd());
        let idx = combo.get_selected_index();
        let use_as_listed = idx == 0;
        let global_algo = match idx {
            1 => WofAlgorithm::Xpress4K,
            3 => WofAlgorithm::Xpress16K,
            4 => WofAlgorithm::Lzx,
            _ => WofAlgorithm::Xpress8K,
        };
        
        // Update UI if overriding per-item settings
        if !use_as_listed {
            for &row in &indices_to_process {
                ctrls.file_list.update_algorithm(row as i32, global_algo);
            }
        }
        
        // Prepare UI state
        if let Some(tb) = &st.taskbar { tb.set_state(TaskbarState::Normal); }
        Button::new(ctrls.action_panel.cancel_hwnd()).set_enabled(true);
        
        let count_w = u64_to_wstring(indices_to_process.len() as u64);
        let status_msg = concat_wstrings(&[&to_wstring("Processing "), &count_w, &to_wstring(" items...")]);
        Label::new(ctrls.status_bar.label_hwnd()).set_text(&String::from_utf16_lossy(&status_msg));
        
        // Prepare Data for Worker
        let tx = st.tx.clone();
        let state_global = st.global_state.clone();
        state_global.store(ProcessingState::Running as u8, Ordering::Relaxed);
        
        // Update button states now that we are running
        update_process_button_state(st);
        
        // Update file list icons for items starting processing
        for &idx in &indices_to_process {
            ctrls.file_list.update_playback_controls(idx as i32, ProcessingState::Running, false);
        }
        
        let action_mode_idx = ComboBox::new(ctrls.action_panel.action_mode_hwnd()).get_selected_index();
        
        let items: Vec<_> = indices_to_process.into_iter().filter_map(|idx| {
            st.batch_items.get(idx).map(|item| {
                let effective_action = match action_mode_idx {
                    1 => BatchAction::Compress, 2 => BatchAction::Decompress, _ => item.action,
                };
                let effective_algo = if use_as_listed { item.algorithm } else { global_algo };
                (item.path.clone(), effective_action, idx, effective_algo)
            })
        }).collect();
        
        // Prepare settings
        let force = st.force_compress;
        let guard = st.config.enable_system_guard;
        let low_power = st.low_power_mode;
        let max_threads = st.config.max_threads;
        let main_hwnd_usize = hwnd as usize;

        let enable_skip = st.config.enable_skip_heuristics;
        let skip_list = String::from_utf16_lossy(&st.config.skip_extensions_buf)
            .trim_matches(char::from(0))
            .to_string();
        
        let set_attr = st.config.set_compressed_attr;
        
        // Launch Worker Thread
        let global_cur = st.global_progress_current.clone();
        let global_tot = st.global_progress_total.clone();

        thread::spawn(move || {
            batch_process_worker(items, tx, state_global, force, main_hwnd_usize, guard, low_power, max_threads, global_cur, global_tot, enable_skip, skip_list, set_attr);
        });
    }
}

pub unsafe fn on_stop_processing(st: &mut AppState) {
    st.global_state.store(ProcessingState::Stopped as u8, Ordering::Relaxed);
    st.processing_queue.clear();
    
    for item in &st.batch_items {
        if let Some(flag) = &item.state_flag { flag.store(ProcessingState::Stopped as u8, Ordering::Relaxed); }
    }
    if let Some(tb) = &st.taskbar { tb.set_state(TaskbarState::Paused); }
    if let Some(ctrls) = &st.controls {
        Button::new(ctrls.action_panel.cancel_hwnd()).set_enabled(false);
        let btn_pause = Button::new(ctrls.action_panel.pause_hwnd());
        btn_pause.set_enabled(false);
        // Reset to Pause icon
        let icon_pause = [0x23F8, 0];
        btn_pause.set_text_w(&icon_pause);
        
        Label::new(ctrls.status_bar.label_hwnd()).set_text("Stopping...");
        
        // Reset all items' visuals
        for (i, item) in st.batch_items.iter().enumerate() {
             ctrls.file_list.update_playback_controls(i as i32, ProcessingState::Stopped, item.status == BatchStatus::Complete);
             if item.status != BatchStatus::Complete {
                 ctrls.file_list.update_status_text(i as i32, "Cancelled");
             }
        }
    }
}

pub unsafe fn on_pause_clicked(st: &mut AppState) {
    let current = st.global_state.load(Ordering::Relaxed);
    let new_state = if current == ProcessingState::Running as u8 {
        ProcessingState::Paused
    } else if current == ProcessingState::Paused as u8 {
        ProcessingState::Running
    } else {
        return; // Should be disabled if not running/paused
    };

    st.global_state.store(new_state as u8, Ordering::Relaxed);
    
    // Update Taskbar
    if let Some(tb) = &st.taskbar {
        match new_state {
            ProcessingState::Paused => tb.set_state(TaskbarState::Paused),
            ProcessingState::Running => tb.set_state(TaskbarState::Normal),
            _ => {}
        }
    }
    
    update_process_button_state(st);
    
    // Update all relevant file list items to reflect new state
    if let Some(ctrls) = &st.controls {
        for (i, item) in st.batch_items.iter().enumerate() {
            // Only update items that are active (processing or pending execution)
            if item.status == BatchStatus::Processing || item.status == BatchStatus::Pending {
                 match new_state {
                     ProcessingState::Paused => {
                         ctrls.file_list.update_playback_controls(i as i32, ProcessingState::Paused, false);
                         ctrls.file_list.update_status_text(i as i32, "Paused");
                     },
                     ProcessingState::Running => {
                         ctrls.file_list.update_playback_controls(i as i32, ProcessingState::Running, false);
                         if item.status == BatchStatus::Processing {
                             ctrls.file_list.update_status_text(i as i32, "Processing");
                         } else {
                             ctrls.file_list.update_status_text(i as i32, "Pending");
                         }
                     },
                     _ => {}
                 }
            }
        }
    }
}

pub unsafe fn update_process_button_state(st: &AppState) {
    if let Some(ctrls) = &st.controls {
        let btn_process = Button::new(ctrls.action_panel.process_hwnd());
        let btn_remove = Button::new(ctrls.action_panel.remove_hwnd());
        let btn_clear = Button::new(ctrls.action_panel.clear_hwnd());
        let btn_pause = Button::new(ctrls.action_panel.pause_hwnd());
        
        let selected_count = ctrls.file_list.get_selection_count();
        let total_count = st.batch_items.len();
        let global_state = ProcessingState::from_u8(st.global_state.load(Ordering::Relaxed));
        
        // 1. Process Button Logic
        if selected_count > 0 {
             btn_process.set_enabled(global_state == ProcessingState::Idle || global_state == ProcessingState::Stopped);
        } else {
            let pending_count = st.batch_items.iter()
                .filter(|i| i.status == BatchStatus::Pending || matches!(i.status, BatchStatus::Error(_)))
                .count();
                
            if pending_count > 0 {
                btn_process.set_enabled(global_state == ProcessingState::Idle || global_state == ProcessingState::Stopped);
            } else {
                btn_process.set_enabled(false);
            }
        }

        // 2. Remove/Clear Logic
        btn_remove.set_enabled(selected_count > 0 && global_state != ProcessingState::Running);
        btn_clear.set_enabled(total_count > 0 && global_state != ProcessingState::Running);

        // 3. Pause Button Logic
        let icon_pause = [0x23F8, 0];
        let icon_resume = [0x25B6, 0];

        if global_state == ProcessingState::Running {
             btn_pause.set_enabled(true);
             btn_pause.set_text_w(&icon_pause);
        } else if global_state == ProcessingState::Paused {
             btn_pause.set_enabled(true);
             btn_pause.set_text_w(&icon_resume);
        } else {
             // Idle or Stopped
             btn_pause.set_enabled(false);
             btn_pause.set_text_w(&icon_pause);
        }
    }
}

pub unsafe fn on_open_settings(st: &mut AppState, hwnd: HWND) {
    let current_theme = st.theme;
    let is_dark = theme::resolve_mode(st.theme);
    let (new_theme, new_force, new_ctx, new_guard, new_low_power, new_threads, new_concurrent, new_log_enabled, new_log_mask, new_skip, new_skip_buf, new_set_attr) = crate::ui::dialogs::show_settings_modal(
        hwnd, current_theme, is_dark, st.enable_force_stop, st.config.enable_context_menu, st.config.enable_system_guard, st.low_power_mode, st.config.max_threads,
        st.config.max_concurrent_items, st.config.log_enabled, st.config.log_level_mask,
        st.config.enable_skip_heuristics, st.config.skip_extensions_buf, st.config.set_compressed_attr
    );
    
    if let Some(t) = new_theme {
        st.theme = t;
        st.config.theme = t;
        // Theme application handled by dialog callbacks to parent
    }
    
    st.enable_force_stop = new_force;
    st.config.enable_force_stop = new_force; // Settings matches config field

    st.config.enable_context_menu = new_ctx;
    st.config.enable_system_guard = new_guard;
    st.low_power_mode = new_low_power;
    st.config.low_power_mode = new_low_power; // Sync config
    st.config.max_threads = new_threads;
    st.config.max_concurrent_items = new_concurrent;
    st.config.log_enabled = new_log_enabled;
    st.config.log_level_mask = new_log_mask;
    
    // New fields
    st.config.enable_skip_heuristics = new_skip;
    st.config.skip_extensions_buf = new_skip_buf;
    st.config.set_compressed_attr = new_set_attr;
    
    // Update global logger state
    if st.config.log_enabled {
        crate::logger::set_log_level(st.config.log_level_mask);
    } else {
        crate::logger::set_log_level(0);
    }
    
    // Apply Process Eco Mode immediately
    crate::engine::power::set_process_eco_mode(st.low_power_mode);
    
    // SAVE CONFIG TO DISK
    st.config.save();
}

pub unsafe fn on_open_watcher_manager(st: &mut AppState, hwnd: HWND) {
    let is_dark = theme::resolve_mode(st.theme);
    
    // We pass the Arc<Mutex> directly
    let tasks = st.watcher_tasks.clone();
    
    crate::ui::dialogs::watcher::show_watcher_modal(hwnd, tasks, st.tx.clone(), is_dark);
}

// --- Notification Handlers (ListView) ---

/// Checks if the notification is a header resize event that should be blocked.
pub unsafe fn should_block_header_resize(lparam: LPARAM) -> bool {
    let nmhdr = &*(lparam as *const NMHDR);
    let code = nmhdr.code;
    // Block tracking (resizing) and divider double-clicks (auto-resize)
    code == HDN_BEGINTRACKW || code == HDN_BEGINTRACKA 
    || code == HDN_DIVIDERDBLCLICKW || code == HDN_DIVIDERDBLCLICKA
}

pub unsafe fn on_list_click(st: &mut AppState, hwnd: HWND, row: i32, col: i32, code: u32) {
    if row < 0 {
        // Clicked on background/empty space -> Deselect All
        if let Some(ctrls) = &st.controls {
            ctrls.file_list.deselect_all();
            update_process_button_state(st);
        }
        return; 
    }
    
    if col == 0 && code == NM_DBLCLK { // Open Path
         if let Some(item) = st.batch_items.get(row as usize) {
             reveal_path_in_explorer(&item.path);
         }
    } else if col == 2 && code == NM_DBLCLK { // Cycle Algo
          if let Some(item) = st.batch_items.get_mut(row as usize) {
              item.algorithm = match item.algorithm {
                  WofAlgorithm::Xpress4K => WofAlgorithm::Xpress8K,
                  WofAlgorithm::Xpress8K => WofAlgorithm::Xpress16K,
                  WofAlgorithm::Xpress16K => WofAlgorithm::Lzx,
                  WofAlgorithm::Lzx => WofAlgorithm::Xpress4K,
              };
              let _name = match item.algorithm {
                  WofAlgorithm::Xpress4K => "XPRESS4K", WofAlgorithm::Xpress8K => "XPRESS8K",
                  WofAlgorithm::Xpress16K => "XPRESS16K", WofAlgorithm::Lzx => "LZX",
              };
              let algo = item.algorithm;
              
                  // Check cache first
                  if let Some(cached) = item.get_cached_estimate(algo) {
                      // Use cached value instantly
                      item.estimated_size = cached;
                      let est_str = crate::utils::format_size(cached);
                      if let Some(ctrls) = &st.controls { 
                          ctrls.file_list.update_algorithm(row, algo); 
                          ctrls.file_list.update_item_text(row, 5, &est_str);
                      }
                  } else {
                      // Need to calculate - trigger async estimation
                      let path = item.path.clone();
                      let id = item.id;
                      if let Some(ctrls) = &st.controls { 
                          ctrls.file_list.update_algorithm(row, algo); 
                          ctrls.file_list.update_item_text(row, 5, &to_wstring("Estimating..."));
                      }
                  let tx = st.tx.clone();
                  thread::spawn(move || {
                      let estimated = crate::engine::estimator::estimate_path(&path, algo);
                      let _est_str = crate::utils::format_size(estimated);
                      let _ = tx.send(crate::ui::state::UiMessage::UpdateEstimate(id, algo, estimated));
                  });
              }
          }
    } else if col == 3 && code == NM_DBLCLK { // Toggle Action
          if let Some(item) = st.batch_items.get_mut(row as usize) {
              let new_action = match item.action {
                  BatchAction::Compress => BatchAction::Decompress,
                  BatchAction::Decompress => BatchAction::Compress,
              };
              item.action = new_action;
              
              if let Some(ctrls) = &st.controls { ctrls.file_list.update_action(row, new_action); }
          }
    } else if col == 10 && code == NM_CLICK { // Start/Pause/Stop (Column 10)
            if let Some(ctrls) = &st.controls {
                let rect = ctrls.file_list.get_subitem_rect(row, 10);
                
                let mut pt: POINT = unsafe { std::mem::zeroed() };
                unsafe {
                    GetCursorPos(&mut pt);
                    ScreenToClient(ctrls.file_list.hwnd(), &mut pt);
                }
                
                let width = rect.right - rect.left;
                let rel_x = pt.x - rect.left;
                
                // Click Zones:
                // [ Eye (~35px) ] [ Playback ... ]
                
                // Use a standard split point suitable for the Eye icon + padding
                let split_x = 32; 
                
                if rel_x <= split_x {
                    // --- ZONE 1: WATCH ---
                    if let Some(item) = st.batch_items.get(row as usize) {
                        let path = item.path.clone();
                         // Add to watcher tasks
                         {
                             let mut tasks = st.watcher_tasks.lock().unwrap();
                             // Check existence
                             if !tasks.iter().any(|t| t.get_path() == path) {
                                 let new_id = tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
                                 let task = crate::watcher_config::WatcherTask::new(
                                     new_id,
                                     &path,
                                     item.algorithm,
                                     0b10000000, // Default: Every Day
                                     12, 0 // Default: 12:00
                                 );
                                 tasks.push(task);
                                 // Save immediately
                                 let _ = crate::watcher_config::WatcherConfig::save(&tasks);
                                 
                                 // Optional: Visual Feedback?
                                 let w_info = to_wstring("Info");
                                 let w_msg = to_wstring("Added to File Watcher schedule!");
                                 MessageBoxW(hwnd, w_msg.as_ptr(), w_info.as_ptr(), MB_OK | MB_ICONINFORMATION);
                             } else {
                                 let w_info = to_wstring("Info");
                                 let w_msg = to_wstring("This path is already in the File Watcher.");
                                 MessageBoxW(hwnd, w_msg.as_ptr(), w_info.as_ptr(), MB_OK | MB_ICONINFORMATION);
                             }
                         }
                    }
                } else {
                    // --- ZONE 2: PLAYBACK ---
                    // No valid dead zone - logic must handle everything to the right
                    
                    let playback_width = width - split_x; 
                    if playback_width <= 0 { return; } // Should not happen with min column width
                    
                    let rel_playback_x = rel_x - split_x;
                    
                    // 1. Determine exact click location and needed action using immutable borrow
                    let mut action_to_take = None; // (IsStop, IsSystemStateChange, Row)
                    
                    let current_state_val = st.global_state.load(Ordering::Relaxed);
                    let current_state = ProcessingState::from_u8(current_state_val);
                    
                    match current_state {
                        ProcessingState::Idle | ProcessingState::Stopped => {
                            // Only allow start if not complete (though button should be hidden)
                            if let Some(_) = st.batch_items.get(row as usize) {
                                // Allow start even if complete (Re-run)
                                 action_to_take = Some((false, true, row));
                            }
                        },
                        ProcessingState::Running => {
                            if rel_playback_x > (playback_width / 2) {
                                 // Stop (Right Half of playback zone)
                                 action_to_take = Some((true, false, row));
                            } else {
                                 // Pause (Left Half of playback zone)
                                 action_to_take = Some((false, false, row));
                            }
                        },
                        ProcessingState::Paused => {
                            if rel_playback_x > (playback_width / 2) {
                                 // Stop (Right Half of playback zone)
                                 action_to_take = Some((true, false, row));
                            } else {
                                 // Resume (Left Half of playback zone)
                                 action_to_take = Some((false, true, row));
                            }
                        },
                    }
                    
                    // 2. Execute Action (Mutable Borrow of st)
                    if let Some((is_stop, is_start_resume, r)) = action_to_take {
                        if is_stop {
                            on_stop_processing(st);
                            if let Some(ctrls) = &st.controls {
                                // Re-update to Stopped, but preserve Watch icon (requires full update call)
                                // actually update_playback_controls handles the icon prepending now.
                                 ctrls.file_list.update_playback_controls(r, ProcessingState::Stopped, st.batch_items[r as usize].status == BatchStatus::Complete);
                            }
                        } else if is_start_resume {
                            // ... (Existing start/resume logic) ...
                            let global = ProcessingState::from_u8(st.global_state.load(Ordering::Relaxed));
                            if global == ProcessingState::Paused {
                                 // RESUME
                                 st.global_state.store(ProcessingState::Running as u8, Ordering::Relaxed);
                                 if let Some(tb) = &st.taskbar { tb.set_state(TaskbarState::Normal); }
                                 
                                 if let Some(ctrls) = &st.controls {
                                     ctrls.file_list.update_playback_controls(r, ProcessingState::Running, false);
                                     ctrls.file_list.update_status_text(r, "Processing");
                                     Label::new(ctrls.status_bar.label_hwnd()).set_text("Resumed.");
                                 }
                            } else {
                                 // START
                                 let indices = vec![r as usize];
                                 start_processing(st, hwnd, indices);
                                 if let Some(ctrls) = &st.controls {
                                     ctrls.file_list.update_playback_controls(r, ProcessingState::Running, false);
                                 }
                            }
                        } else {
                             // PAUSE
                             st.global_state.store(ProcessingState::Paused as u8, Ordering::Relaxed);
                             if let Some(tb) = &st.taskbar { tb.set_state(TaskbarState::Paused); }
                             
                             if let Some(ctrls) = &st.controls {
                                 ctrls.file_list.update_playback_controls(r, ProcessingState::Paused, false);
                                 ctrls.file_list.update_status_text(r, "Paused");
                                 Label::new(ctrls.status_bar.label_hwnd()).set_text("Paused.");
                             }
                        }
                    }
                } // End Zone 2
            }
    }
}

pub unsafe fn on_list_keydown(st: &mut AppState, _hwnd: HWND, key: u16) {
    if key == VK_DELETE as u16 {
        on_remove_selected(st);
    } else if key == 0x41 { // 'A' key
        let ctrl_pressed = (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
        if ctrl_pressed {
             if let Some(ctrls) = &st.controls {
                 let count = ctrls.file_list.get_item_count();
                 for i in 0..count {
                     ctrls.file_list.set_selected(i, true);
                 }
             }
        }
    }
}

pub unsafe fn on_list_rclick(st: &mut AppState, hwnd: HWND, row: i32, col: i32) -> bool {
    if row < 0 { return false; } 

    // Handle Algorithm Column (2)
    if col == 2 {
        let mut pt: POINT = std::mem::zeroed();
        GetCursorPos(&mut pt);
        
        let menu = CreatePopupMenu();
        if menu != std::ptr::null_mut() {
            let _ = AppendMenuW(menu, MF_STRING, 2001, to_wstring("XPRESS4K").as_ptr());
            let _ = AppendMenuW(menu, MF_STRING, 2002, to_wstring("XPRESS8K").as_ptr());
            let _ = AppendMenuW(menu, MF_STRING, 2003, to_wstring("XPRESS16K").as_ptr());
            let _ = AppendMenuW(menu, MF_STRING, 2004, to_wstring("LZX").as_ptr());

            if let Some(item) = st.batch_items.get(row as usize) {
                let check_id = match item.algorithm {
                    WofAlgorithm::Xpress4K => 2001,
                    WofAlgorithm::Xpress8K => 2002,
                    WofAlgorithm::Xpress16K => 2003,
                    WofAlgorithm::Lzx => 2004,
                };
                CheckMenuItem(menu, check_id, MF_CHECKED);
            }

            let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_LEFTALIGN | TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, std::ptr::null());
            DestroyMenu(menu);

            if cmd >= 2001 && cmd <= 2004 {
                let new_algo = match cmd {
                    2001 => WofAlgorithm::Xpress4K,
                    2002 => WofAlgorithm::Xpress8K,
                    2003 => WofAlgorithm::Xpress16K,
                    2004 => WofAlgorithm::Lzx,
                    _ => WofAlgorithm::Xpress8K,
                };

                if let Some(item) = st.batch_items.get_mut(row as usize) {
                    if item.algorithm != new_algo {
                        item.algorithm = new_algo;
                        
                        if let Some(ctrls) = &st.controls { 
                            ctrls.file_list.update_algorithm(row, item.algorithm);
                        }

                        if let Some(cached) = item.get_cached_estimate(new_algo) {
                            item.estimated_size = cached;
                            let est_str = crate::utils::format_size(cached);
                            if let Some(ctrls) = &st.controls { 
                                ctrls.file_list.update_item_text(row, 5, &est_str);
                            }
                        } else {
                            let path = item.path.clone();
                            let id = item.id;
                            if let Some(ctrls) = &st.controls { 
                                ctrls.file_list.update_item_text(row, 5, &to_wstring("Estimating..."));
                            }
                            let tx = st.tx.clone();
                            thread::spawn(move || {
                                let estimated = crate::engine::estimator::estimate_path(&path, new_algo);
                                let _est_str = crate::utils::format_size(estimated);
                                let _ = tx.send(crate::ui::state::UiMessage::UpdateEstimate(id, new_algo, estimated));
                            });
                        }
                    }
                }
            }
            return true;
        }
    } 
    // Handle Action Column (3)
    else if col == 3 {
        let mut pt: POINT = std::mem::zeroed();
        GetCursorPos(&mut pt);
        
        let menu = CreatePopupMenu();
        if menu != std::ptr::null_mut() {
            let _ = AppendMenuW(menu, MF_STRING, 3001, to_wstring("Compress").as_ptr());
            let _ = AppendMenuW(menu, MF_STRING, 3002, to_wstring("Decompress").as_ptr());

            if let Some(item) = st.batch_items.get(row as usize) {
                let check_id = match item.action {
                    crate::ui::state::BatchAction::Compress => 3001,
                    crate::ui::state::BatchAction::Decompress => 3002,
                };
                CheckMenuItem(menu, check_id, MF_CHECKED);
            }

            let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_LEFTALIGN | TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, std::ptr::null());
            DestroyMenu(menu);

            if cmd >= 3001 && cmd <= 3002 {
                let new_action = match cmd {
                    3001 => crate::ui::state::BatchAction::Compress,
                    3002 => crate::ui::state::BatchAction::Decompress,
                    _ => crate::ui::state::BatchAction::Compress,
                };
                
                if let Some(item) = st.batch_items.get_mut(row as usize) {
                    if item.action != new_action {
                        item.action = new_action;
                         if let Some(ctrls) = &st.controls { 
                            ctrls.file_list.update_action(row, new_action); 
                        }
                    }
                }
            }
            return true;
        }
    }

    false
}

pub unsafe fn on_column_click(st: &mut AppState, lparam: LPARAM) {
    let nmlv = &*(lparam as *const NMLISTVIEW);
    let column = nmlv.iSubItem;
    
    if st.sort_column == column {
        st.sort_ascending = !st.sort_ascending;
    } else {
        st.sort_column = column;
        st.sort_ascending = true;
    }
    
    if let Some(ctrls) = &st.controls {
        let context = st as *const AppState as isize;
        ctrls.file_list.sort_items(compare_items, context);
        // Update header sort indicator
        ctrls.file_list.set_sort_indicator(st.sort_column, st.sort_ascending);
    }
}

// --- Context Menu Handler ---

pub unsafe fn handle_context_menu(st: &mut AppState, hwnd: HWND, wparam: WPARAM) {
    let hwnd_from = wparam as HWND;
    if let Some(ctrls) = &st.controls {
        if hwnd_from == ctrls.file_list.hwnd() {
            let selected = ctrls.file_list.get_selected_indices();
            if !selected.is_empty() {
                let mut pt: POINT = std::mem::zeroed();
                GetCursorPos(&mut pt);
                let menu = CreatePopupMenu();
                if menu != std::ptr::null_mut() {
                    let mut any_processing = false;
                    let mut any_pending = false;
                    
                    for &idx in &selected {
                        if let Some(item) = st.batch_items.get(idx as usize) {
                            match item.status {
                                BatchStatus::Processing => { any_processing = true; },
                                BatchStatus::Pending => { any_pending = true; },
                                _ => {}
                            }
                        }
                    }

                    if any_processing {
                        let _ = AppendMenuW(menu, MF_STRING, 1003, to_wstring("Stop").as_ptr());
                    } else if any_pending {
                        let _ = AppendMenuW(menu, MF_STRING, 1005, to_wstring("Start Selected").as_ptr());
                    }
                    let _ = AppendMenuW(menu, MF_STRING, 1004, to_wstring("Remove").as_ptr());
                    let _ = AppendMenuW(menu, MF_STRING, 1006, to_wstring("Open File Location").as_ptr());

                    let _cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_LEFTALIGN, pt.x, pt.y, 0, hwnd, std::ptr::null());
                    DestroyMenu(menu);
                    
                    match _cmd {
                        1003 => { Button::new(GetDlgItem(hwnd, IDC_BTN_CANCEL as i32)).set_enabled(false); on_stop_processing(st); },
                        1004 => { on_remove_selected(st); },
                        1005 => {
                             start_processing(st, hwnd, selected.clone());
                        },
                        1006 => {
                            if let Some(&first_idx) = selected.first() {
                                if let Some(item) = st.batch_items.get(first_idx as usize) {
                                    reveal_path_in_explorer(&item.path);
                                }
                            }
                        },
                        _ => {}
                    }
                }
            }
        }
    }
}

// --- Drag and Drop / Clipboard Handler ---

pub unsafe fn process_hdrop(_hwnd: HWND, hdrop: HDROP, st: &mut AppState) {
    let count = DragQueryFileW(hdrop, 0xFFFFFFFF, std::ptr::null_mut(), 0);
    let mut paths = Vec::new();
    let mut buffer = [0u16; 1024];
    
    for i in 0..count {
        let len = DragQueryFileW(hdrop, i, buffer.as_mut_ptr(), 1024);
        if len > 0 {
            let s = String::from_utf16_lossy(&buffer[..len as usize]);
            paths.push(s);
        }
    }
    DragFinish(hdrop);
    st.ingest_paths(paths);
    update_process_button_state(st);
}

pub unsafe fn process_clipboard(hwnd: HWND, st: &mut AppState) {
    if OpenClipboard(hwnd) == 0 { return; }

    // CF_HDROP = 15
    if IsClipboardFormatAvailable(15) != 0 {
         let hdrop = GetClipboardData(15) as HDROP;
         if !hdrop.is_null() {
             process_hdrop(hwnd, hdrop, st);
         }
    } else if IsClipboardFormatAvailable(13) != 0 {
        // CF_UNICODETEXT = 13
        let h_global = GetClipboardData(13);
        if !h_global.is_null() {
            let ptr = GlobalLock(h_global) as *const u16;
            if !ptr.is_null() {
                // Read string until null terminator
                let mut len = 0;
                while *ptr.add(len) != 0 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(ptr, len);
                let text = String::from_utf16_lossy(slice);
                GlobalUnlock(h_global);
                
                // Clean up text (trim whitespace/quotes) and check existence
                let path_str = text.trim().trim_matches('"').to_string();
                if std::path::Path::new(&path_str).exists() {
                    st.ingest_paths(vec![path_str]);
                    update_process_button_state(st);
                }
            }
        }
    }
    
    CloseClipboard();
}

// --- Sorter ---

pub unsafe extern "system" fn compare_items(lparam1: isize, lparam2: isize, lparam_sort: isize) -> i32 {
    let state = &*(lparam_sort as *const AppState);
    let id1 = lparam1 as u32;
    let id2 = lparam2 as u32;
    
    let item1 = state.batch_items.iter().find(|i| i.id == id1);
    let item2 = state.batch_items.iter().find(|i| i.id == id2);
    
    match (item1, item2) {
        (Some(i1), Some(i2)) => {
            let ord = match state.sort_column {
                0 => i1.path.to_lowercase().cmp(&i2.path.to_lowercase()), 
                2 => {
                    let p1 = match i1.algorithm {
                        WofAlgorithm::Xpress4K => 0, WofAlgorithm::Xpress8K => 1, WofAlgorithm::Xpress16K => 2, WofAlgorithm::Lzx => 3,
                    };
                    let p2 = match i2.algorithm {
                        WofAlgorithm::Xpress4K => 0, WofAlgorithm::Xpress8K => 1, WofAlgorithm::Xpress16K => 2, WofAlgorithm::Lzx => 3,
                    };
                    p1.cmp(&p2)
                },
                3 => {
                    let p1 = match i1.action { BatchAction::Compress => 0, BatchAction::Decompress => 1 };
                    let p2 = match i2.action { BatchAction::Compress => 0, BatchAction::Decompress => 1 };
                    p1.cmp(&p2)
                },
                4 => i1.logical_size.cmp(&i2.logical_size),
                5 => i1.estimated_size.cmp(&i2.estimated_size),
                6 => i1.disk_size.cmp(&i2.disk_size),
                8 => {
                   let p1 = match &i1.status { 
                       BatchStatus::Pending => 0, BatchStatus::Processing => 1, BatchStatus::Complete => 2, BatchStatus::Error(_) => 3 
                   };
                   let p2 = match &i2.status { 
                       BatchStatus::Pending => 0, BatchStatus::Processing => 1, BatchStatus::Complete => 2, BatchStatus::Error(_) => 3 
                   };
                   p1.cmp(&p2)
                },
                _ => CmpOrdering::Equal,
            };
            
            let result = match ord {
                CmpOrdering::Less => -1,
                CmpOrdering::Equal => 0,
                CmpOrdering::Greater => 1,
            };
            
            if state.sort_ascending { result } else { -result }
        },
        _ => 0
    }
}