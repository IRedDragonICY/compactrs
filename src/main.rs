#![windows_subsystem = "windows"]

use windows::core::{Result, w};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MessageBoxW, TranslateMessage, MB_ICONERROR, MB_OK, MSG, WM_QUIT,
};

pub mod ui;
pub mod engine; // Make sure the engine module is available to the rest of the app
pub mod config;
pub mod registry;

use std::sync::OnceLock;
use crate::engine::wof::WofAlgorithm;
use crate::ui::state::BatchAction;

/// Startup item passed via command line arguments
#[derive(Clone, Debug)]
pub struct StartupItem {
    pub path: String,
    pub algorithm: WofAlgorithm,
    pub action: BatchAction,
}

/// Global storage for startup items from CLI arguments
static STARTUP_ITEMS: OnceLock<Vec<StartupItem>> = OnceLock::new();

/// Get startup items parsed from command line (if any)
pub fn get_startup_items() -> &'static [StartupItem] {
    STARTUP_ITEMS.get().map(|v| v.as_slice()).unwrap_or(&[])
}

/// Parse command line arguments
/// Supports: --path "C:\file" --algo xpress4k|xpress8k|xpress16k|lzx --action decompress
fn parse_cli_args() -> Vec<StartupItem> {
    let args: Vec<String> = std::env::args().collect();
    let mut items = Vec::new();
    
    let mut i = 1; // Skip executable name
    while i < args.len() {
        if args[i] == "--path" && i + 1 < args.len() {
            let path = args[i + 1].clone();
            i += 2;
            
            // Look for --algo or --action
            let mut algorithm = WofAlgorithm::Xpress8K; // Default
            let mut action = BatchAction::Compress; // Default
            
            while i < args.len() {
                if args[i] == "--algo" && i + 1 < args.len() {
                    algorithm = match args[i + 1].to_lowercase().as_str() {
                        "xpress4k" => WofAlgorithm::Xpress4K,
                        "xpress8k" => WofAlgorithm::Xpress8K,
                        "xpress16k" => WofAlgorithm::Xpress16K,
                        "lzx" => WofAlgorithm::Lzx,
                        _ => WofAlgorithm::Xpress8K,
                    };
                    i += 2;
                } else if args[i] == "--action" && i + 1 < args.len() {
                    action = match args[i + 1].to_lowercase().as_str() {
                        "decompress" => BatchAction::Decompress,
                        _ => BatchAction::Compress,
                    };
                    i += 2;
                } else if args[i] == "--path" {
                    // Next item starts, don't consume
                    break;
                } else {
                    i += 1;
                }
            }
            
            items.push(StartupItem { path, algorithm, action });
        } else {
            i += 1;
        }
    }
    
    items
}

fn main() -> Result<()> {
    // Parse CLI arguments before GUI initialization
    let startup_items = parse_cli_args();
    let _ = STARTUP_ITEMS.set(startup_items);
    
    // Initialize GUI
    unsafe {
        // Initialize COM for IFileOpenDialog
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let instance = GetModuleHandleW(None)?;
        
        if let Err(e) = ui::window::create_main_window(instance.into()) {
            MessageBoxW(None, w!("Failed to create main window!"), w!("Error"), MB_ICONERROR | MB_OK);
            return Err(e);
        }

        // Message Loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_QUIT {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}

