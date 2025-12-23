use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::mem;
use crate::engine::wof::WofAlgorithm;

pub const MAX_PATH_LEN: usize = 260; // Standard MAX_PATH

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct WatcherTask {
    pub id: u32,
    pub path: [u16; MAX_PATH_LEN],
    pub algorithm: WofAlgorithm,
    pub days_mask: u8, // Bit 0=Mon, 1=Tue, ... 6=Sun, 7=Every Day
    pub time_hour: u8,
    pub time_minute: u8,
    pub last_run_timestamp: u64, // Unix timestamp
    pub _padding: [u8; 4], // Align to 8 bytes if needed, though u64 is 8-byte aligned. repr(C) will handle basic alignment.
}

impl Default for WatcherTask {
    fn default() -> Self {
        Self {
            id: 0,
            path: [0; MAX_PATH_LEN],
            algorithm: WofAlgorithm::Xpress8K,
            days_mask: 0,
            time_hour: 0,
            time_minute: 0,
            last_run_timestamp: 0,
            _padding: [0; 4],
        }
    }
}

impl WatcherTask {
    pub fn new(id: u32, path_str: &str, algorithm: WofAlgorithm, days_mask: u8, hour: u8, minute: u8) -> Self {
        let mut path = [0u16; MAX_PATH_LEN];
        let mut i = 0;
        for c in path_str.encode_utf16() {
            if i < MAX_PATH_LEN - 1 {
                path[i] = c;
                i += 1;
            }
        }
        
        Self {
            id,
            path,
            algorithm,
            days_mask,
            time_hour: hour,
            time_minute: minute,
            last_run_timestamp: 0,
            _padding: [0; 4],
        }
    }

    pub fn get_path(&self) -> String {
        let end = self.path.iter().position(|&c| c == 0).unwrap_or(MAX_PATH_LEN);
        String::from_utf16_lossy(&self.path[..end])
    }

    pub fn set_path(&mut self, path_str: &str) {
        let mut path = [0u16; MAX_PATH_LEN];
        let mut i = 0;
        for c in path_str.encode_utf16() {
            if i < MAX_PATH_LEN - 1 {
                path[i] = c;
                i += 1;
            }
        }
        self.path = path;
    }
}

pub struct WatcherConfig;

impl WatcherConfig {
    fn get_config_path() -> std::path::PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            exe.with_file_name("watcher.dat")
        } else {
            std::path::PathBuf::from("watcher.dat")
        }
    }

    pub fn load() -> Vec<WatcherTask> {
        let path = Self::get_config_path();
        if !path.exists() {
            return Vec::new();
        }

        let mut tasks = Vec::new();
        if let Ok(mut file) = File::open(path) {
            let struct_size = mem::size_of::<WatcherTask>();
            let mut buffer = vec![0u8; struct_size];
            
            while file.read_exact(&mut buffer).is_ok() {
                let task: WatcherTask = unsafe { std::ptr::read(buffer.as_ptr() as *const _) };
                tasks.push(task);
            }
        }
        tasks
    }

    pub fn save(tasks: &[WatcherTask]) -> std::io::Result<()> {
        let path = Self::get_config_path();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        for task in tasks {
            let ptr = task as *const WatcherTask as *const u8;
            let slice = unsafe { std::slice::from_raw_parts(ptr, mem::size_of::<WatcherTask>()) };
            file.write_all(slice)?;
        }
        Ok(())
    }
}
