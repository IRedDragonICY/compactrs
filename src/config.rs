use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use crate::engine::wof::WofAlgorithm;
use crate::ui::state::AppTheme;

// NOTE: We use #[repr(C)] to ensure predictable memory layout for binary dumping.
// WARNING: Changing fields later will invalidate existing config files.
#[repr(C)] 
#[derive(Clone, Copy, Debug)]
pub struct AppConfig {
    pub magic: u32,   // 0x43505253 ("CPRS")
    pub version: u32, // 1
    pub theme: AppTheme,
    pub default_algo: WofAlgorithm,
    pub force_compress: bool,
    pub enable_force_stop: bool,
    pub window_width: i32,
    pub window_height: i32,
    pub window_x: i32,
    pub window_y: i32,
    pub enable_context_menu: bool,
    pub enable_system_guard: bool,
    pub low_power_mode: bool,
    pub max_threads: u32,
    pub max_concurrent_items: u32, // New in v5
    pub log_enabled: bool,
    pub log_level_mask: u8,
    // New in v6
    pub enable_skip_heuristics: bool,
    pub skip_extensions_buf: [u16; 512], // Comma separated list
}


impl Default for AppConfig {
    fn default() -> Self {
        // Default skip list
        let default_skip = "zip,7z,rar,gz,bz2,xz,zst,lz4,jpg,jpeg,png,gif,webp,avif,heic,mp4,mkv,avi,webm,mov,wmv,mp3,flac,aac,ogg,opus,wma,pdf";
        let mut buf = [0u16; 512];
        let mut i = 0;
        for c in default_skip.encode_utf16() {
            if i < 511 {
                buf[i] = c;
                i += 1;
            }
        }

        Self {
            magic: 0x43505253,
            version: 6,
            theme: AppTheme::System,
            default_algo: WofAlgorithm::Xpress8K, // Default to XPRESS8K
            force_compress: false,
            enable_force_stop: false,
            window_width: 900,
            window_height: 600,
            window_x: -1, 
            window_y: -1,
            enable_context_menu: false,
            enable_system_guard: true,
            low_power_mode: false,
            max_threads: 0,
            max_concurrent_items: 0, 
            log_enabled: true,
            log_level_mask: 7,
            enable_skip_heuristics: true,
            skip_extensions_buf: buf,
        }
    }
}

impl AppConfig {
    fn get_path() -> PathBuf {
        let mut path = std::env::current_exe().unwrap_or_default();
        path.set_file_name("compactrs.dat");
        path
    }

    pub fn load() -> Self {
        let path = Self::get_path();
        if let Ok(mut file) = File::open(path) {
            let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
            
            // Strict binary compatibility check: Size must match exactly.
            if file_len == std::mem::size_of::<AppConfig>() as u64 {
                let mut buffer = [0u8; std::mem::size_of::<AppConfig>()];
                if file.read_exact(&mut buffer).is_ok() {
                    // Safe because AppConfig is #[repr(C)] and contains only POD-like types/primitives.
                    // We verify Magic and Version to ensure it's not random garbage.
                    unsafe {
                        let config = std::ptr::read_unaligned(buffer.as_ptr() as *const AppConfig);
                        if config.magic == 0x43505253 && config.version == 6 {
                            return config;
                        }
                    }
                }
            }
        }
        // If file missing, wrong size, or invalid magic/version, return default.
        Self::default()
    }

    pub fn save(&self) {
        if let Ok(mut file) = File::create(Self::get_path()) {
            unsafe {
                let bytes: &[u8] = std::slice::from_raw_parts(
                    self as *const _ as *const u8, 
                    std::mem::size_of::<AppConfig>()
                );
                let _ = file.write_all(bytes);
            }
        }
    }
}
