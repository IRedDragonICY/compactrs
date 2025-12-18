#![windows_subsystem = "windows"]

use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MessageBoxW, TranslateMessage, 
    MB_ICONERROR, MB_OK, MSG, WM_QUIT, SW_SHOW,
};
use windows_sys::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
use std::ptr;
use std::sync::OnceLock;

pub mod ui;
pub mod engine;
pub mod config;
pub mod registry;
pub mod utils;
pub mod updater;
pub mod json;

use crate::engine::wof::WofAlgorithm;
use crate::ui::state::BatchAction;
use crate::utils::to_wstring;

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

// Helper function to check for admin privileges
fn is_admin() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

fn main() {
    // 1. Runtime Admin Check
    if !is_admin() {
        unsafe {
            // Attempt to relaunch as administrator
            let mut filename = [0u16; 32768]; // MAX_PATH is 260 but wide paths can be longer, using safe buffer
            let len = GetModuleFileNameW(std::ptr::null_mut(), filename.as_mut_ptr(), filename.len() as u32);
            
            if len > 0 {
                let operation = to_wstring("runas");
                // Collect existing arguments to pass them along
                // Note: This simple reconstruction might not perfectly preserve quotes/escapes for complex args
                let args: Vec<String> = std::env::args().skip(1).collect();
                let args_str = args.join(" ");
                let args_wide = to_wstring(&args_str);

                let res = ShellExecuteW(
                    std::ptr::null_mut(), 
                    operation.as_ptr(), 
                    filename.as_ptr(), 
                    if args.is_empty() { ptr::null() } else { args_wide.as_ptr() }, 
                    ptr::null(), 
                    SW_SHOW
                );

                // If ShellExecuteW returns > 32, it succeeded
                if res as isize > 32 {
                    std::process::exit(0); // Exit this non-admin instance immediately
                }
            }

            // If elevation failed (user declined UAC, etc.), show error
            let title = to_wstring("Privilege Error");
            let msg = to_wstring("CompactRS requires Administrator privileges to perform compression operations.\n\nFailed to elevate privileges. Please restart as Administrator.");
            
            MessageBoxW(
                std::ptr::null_mut(), 
                msg.as_ptr(), 
                title.as_ptr(), 
                MB_ICONERROR | MB_OK
            );
        }
        std::process::exit(1);
    }

    // Parse CLI arguments before GUI initialization
    let startup_items = parse_cli_args();
    let _ = STARTUP_ITEMS.set(startup_items);

    // Cleanup old executable if it exists (from self-update)
    if let Ok(exe) = std::env::current_exe() {
        let old_exe = exe.with_extension("old");
        if old_exe.exists() {
             // We can just try to delete it. If it fails (still locked?), we ignore.
             // It will be cleaned up next time.
             let _ = std::fs::remove_file(old_exe);
        }
    }
    
    unsafe {
        // Initialize COM for IFileOpenDialog
        // Ignore result, it might already be initialized
        // Note: windows-sys defines COINIT_APARTMENTTHREADED as i32 (0x2), CoInitializeEx expects u32
        let _ = CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED as u32);

        let instance = GetModuleHandleW(ptr::null());
        
        // We'll update create_main_window to accept isize (HINSTANCE)
        if let Err(e) = ui::window::create_main_window(instance) {
            let msg = to_wstring(&("Failed to create main window: ".to_string() + &e.to_string()));
            MessageBoxW(std::ptr::null_mut(), msg.as_ptr(), to_wstring("Error").as_ptr(), MB_ICONERROR | MB_OK);
            std::process::exit(1);
        }

        // Message Loop
        let mut msg: MSG = std::mem::zeroed();
        // GetMessageW returns BOOL (i32). strict > 0 check for success.
        // HWND parameter should be NULL (0/null_mut) to retrieve messages for any window belonging to the current thread
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_QUIT {
                break;
            }

            // TODO: Add TranslateAcceleratorW here if we add an accelerator table later
            
            // Dispatch key events manually if needed or just translate
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Clean up COM
        windows_sys::Win32::System::Com::CoUninitialize();
        
        // Force process exit to ensure no background threads start
        std::process::exit(0);
    }
}
