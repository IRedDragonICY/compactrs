#![allow(non_snake_case)]
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use crate::watcher_config::{WatcherTask, WatcherConfig};
use crate::ui::state::UiMessage;
use std::sync::mpsc::Sender;

/// Starts the background watcher thread
pub fn start_watcher_thread(tasks: Arc<Mutex<Vec<WatcherTask>>>, tx: Sender<UiMessage>) {
    thread::spawn(move || {
        loop {
            // Sleep for 60 seconds
            thread::sleep(Duration::from_secs(60));

            let now = SystemTime::now();
            let since_epoch = now.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_secs();
            
            // Get current local time (simple approximation or using system calls)
            // Ideally we need a chrono-like functionality, but we want zero-deps.
            // We'll use Win32 API to get local time.
            // Manual SYSTEMTIME definition
            #[repr(C)]
            struct SYSTEMTIME {
                wYear: u16,
                wMonth: u16,
                wDayOfWeek: u16,
                wDay: u16,
                wHour: u16,
                wMinute: u16,
                wSecond: u16,
                wMilliseconds: u16,
            }

            let mut system_time: SYSTEMTIME = unsafe { std::mem::zeroed() };
            
            // Manual binding for GetLocalTime
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn GetLocalTime(lpsystemtime: *mut SYSTEMTIME);
            }
            unsafe { GetLocalTime(&mut system_time) };
            
            let current_dow = if system_time.wDayOfWeek == 0 { 6 } else { system_time.wDayOfWeek - 1 } as u8; // Mon=0..Sun=6
            let current_hour = system_time.wHour as u8;
            let current_minute = system_time.wMinute as u8;
            
            // Calculate start of today (midnight) in unix timestamp to prevent re-running
            // This is tricky without chrono, so we'll rely on last_run_timestamp comparison
            // If last_run_timestamp is within the last 24 hours AND matches today's date... 
            // Simpler: if last_run_timestamp > (now - 12h) ... 
            
            // Better: Store the last run day/hour/min? No, just timestamp.
            // Strategy: if (now - last_run) > 12 hours AND current time matches schedule. 
            // Ideally we want to run EXACTLY once per day per schedule.
            
            let mut tasks_guard = tasks.lock().unwrap();
            let mut dirty = false;
            
            for task in tasks_guard.iter_mut() {
                // Check Day
                // days_mask: Bit 0=Mon ... 6=Sun, 7=Every Day
                let is_today = (task.days_mask & (1 << current_dow)) != 0 || (task.days_mask & 0x80) != 0;
                
                if is_today {
                    // Check Time (trigger if current time >= schedule time within a small window, or just >= and not run today)
                    // We simply check if we are past the scheduled time.
                    let scheduled_passed = (current_hour > task.time_hour) || 
                                         (current_hour == task.time_hour && current_minute >= task.time_minute);
                    
                    if scheduled_passed {
                        // Check if already run recently (e.g., within last 12 hours)
                        // 12 hours = 43200 seconds
                        let time_since_last = since_epoch.saturating_sub(task.last_run_timestamp);
                        
                        if time_since_last > 43200 {
                            // Trigger!
                            let path = task.get_path();
                            
                            // Send to UI Main Thread to process
                            // We construct a special internal message or just use StatusText for now to debug
                            // Real implementation: We need to trigger batch processing.
                            // We can use `ingest_paths` if we had access to mutable AppState, but we don't here.
                            // We only have the Tx.
                            // We need a new UiMessage type for "WatcherTriggered"
                            
                            // For execution, we can spawn a worker directly? 
                            // No, UI should manage it to show progress.
                            // Let's assume we add `WatcherTrigger(String, WofAlgorithm)` to UiMessage.
                            
                            // Log it
                            let _ = tx.send(UiMessage::StatusText(format!("Watcher executing: {}", path)));
                            
                            // We'll abuse `StatusText` for now if we don't want to change UiMessage enum in a big way yet, 
                            // BUT changing UiMessage is better.
                            // Let's assume we can add `TriggerBatch(String, WofAlgorithm)` to `UiMessage` in `state.rs`.
                            // I will add that modification to state.rs next.
                            
                            // Mark as run
                            task.last_run_timestamp = since_epoch;
                            dirty = true;
                        }
                    }
                }
            }
            
            if dirty {
                let _ = WatcherConfig::save(&tasks_guard);
            }
        }
    });
}
