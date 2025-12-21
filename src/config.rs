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
    pub log_enabled: bool,
    pub log_level_mask: u8,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            magic: 0x43505253,
            version: 2,
            theme: AppTheme::System,
            default_algo: WofAlgorithm::Xpress8K, // Default to XPRESS8K
            force_compress: false,
            enable_force_stop: false,
            window_width: 900,
            window_height: 600,
            window_x: -1, // -1 indicates let Windows decide (CW_USEDEFAULT)
            window_y: -1,
            enable_context_menu: false,
            enable_system_guard: true,
            low_power_mode: false,
            max_threads: 0, // 0 = Auto
            log_enabled: true,
            log_level_mask: 7, // Error | Warn | Info (1 | 2 | 4)
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
            let mut buffer = [0u8; std::mem::size_of::<AppConfig>()];
            if file.read_exact(&mut buffer).is_ok() {
                // Safety: AppConfig is repr(C) and contains only Copy types.
                unsafe { 
                    let config: AppConfig = std::mem::transmute(buffer);
                    // Validate magic and version
                    if config.magic == 0x43505253 && (config.version >= 1 && config.version <= 3) {
                        if config.version == 1 {
                             // Migrate v1 to v3
                             return AppConfig {
                                 max_threads: 0,
                                 log_enabled: true,
                                 log_level_mask: 7,
                                 version: 3,
                                 ..config
                             };
                        }
                        if config.version == 2 {
                             // Migrate v2 to v3
                             return AppConfig {
                                 log_enabled: true,
                                 log_level_mask: 7,
                                 version: 3,
                                 ..config
                             };
                        }
                        return config;
                    }
                }
            }
        }
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
