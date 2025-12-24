#![windows_subsystem = "windows"]
#![no_main]

// Entry point refactoring complete

use crate::types::*;
use std::ptr;
use std::sync::OnceLock;

pub mod ui;
pub mod engine;
pub mod config;
pub mod registry;
pub mod utils;
pub mod updater;
pub mod watcher_config;
mod logger;
pub mod types;
pub mod com;

use crate::engine::wof::WofAlgorithm;
use crate::ui::state::BatchAction;
use crate::utils::to_wstring;

// Manual binding for ExitProcess since we are bypassing standard main return
#[link(name = "kernel32")]
unsafe extern "system" {
    fn ExitProcess(uExitCode: u32);
}

/// Startup item passed via command line arguments
#[derive(Clone, Debug)]
pub struct StartupItem {
    pub path: String,
    pub algorithm: WofAlgorithm,
    pub action: BatchAction,
}

static STARTUP_ITEMS: OnceLock<Vec<StartupItem>> = OnceLock::new();

pub fn get_startup_items() -> &'static [StartupItem] {
    STARTUP_ITEMS.get().map(|v| v.as_slice()).unwrap_or(&[])
}

fn parse_cli_args() -> Vec<StartupItem> {
    // std::env::args() works even with no_main as it lazily queries GetCommandLineW
    let args: Vec<String> = std::env::args().collect();
    let mut items = Vec::new();
    
    let mut i = 1; 
    while i < args.len() {
        if args[i] == "--path" && i + 1 < args.len() {
            let path = args[i + 1].clone();
            i += 2;
            let mut algorithm = WofAlgorithm::Xpress8K; 
            let mut action = BatchAction::Compress; 
            
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

fn is_admin() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn WinMainCRTStartup() {
    // Initialize Theme System early
    crate::ui::theme::init();
    crate::ui::theme::set_preferred_app_mode(true);

    // 0. Single Instance Check
    let class_name = w!("CompactRS_Class");
    let hwnd_existing = FindWindowW(class_name.as_ptr(), std::ptr::null());
    
    if hwnd_existing != std::ptr::null_mut() {
        let items = parse_cli_args();
        if !items.is_empty() {
            for item in items {
                let algo_str = match item.algorithm {
                        WofAlgorithm::Xpress4K => "xpress4k",
                        WofAlgorithm::Xpress8K => "xpress8k",
                        WofAlgorithm::Xpress16K => "xpress16k",
                        WofAlgorithm::Lzx => "lzx",
                };
                let action_str = match item.action {
                    BatchAction::Compress => "compress",
                    BatchAction::Decompress => "decompress",
                };
                let payload = [&item.path, "|", algo_str, "|", action_str].concat();
                let payload_w = to_wstring(&payload);
                let cds = COPYDATASTRUCT {
                    dwData: 0xB00B,
                    cbData: (payload_w.len() * 2) as u32,
                    lpData: payload_w.as_ptr() as *mut _,
                };
                SendMessageW(hwnd_existing, WM_COPYDATA, 0, &cds as *const _ as isize);
            }
        }
        ExitProcess(0); // Explicit exit
    }

    // 1. Runtime Admin Check
    if !is_admin() {
        // Attempt to relaunch as administrator
        let mut filename = [0u16; 32768];
        let len = GetModuleFileNameW(std::ptr::null_mut(), filename.as_mut_ptr(), filename.len() as u32);
        
        if len > 0 {
            let operation = w!("runas");
            let args: Vec<String> = std::env::args().skip(1).collect();
            let args_str = args.iter()
                .map(|arg| {
                    if arg.contains(' ') || arg.contains('\t') || arg.is_empty() {
                        ["\"", arg, "\""].concat()
                    } else {
                        arg.clone()
                    }
                })
                .collect::<Vec<String>>()
                .join(" ");
            let args_wide = to_wstring(&args_str);

            let res = ShellExecuteW(
                std::ptr::null_mut(), 
                operation.as_ptr(), 
                filename.as_ptr(), 
                if args.is_empty() { ptr::null() } else { args_wide.as_ptr() }, 
                ptr::null(), 
                SW_SHOW
            );

            if res as isize > 32 {
                ExitProcess(0);
            }
        }

        let title = w!("Privilege Error");
        let msg = w!("CompactRS requires Administrator privileges to perform compression operations.\n\nFailed to elevate privileges. Please restart as Administrator.");
        
        MessageBoxW(
            std::ptr::null_mut(), 
            msg.as_ptr(), 
            title.as_ptr(), 
            MB_ICONERROR | MB_OK
        );
        ExitProcess(1);
    }

    // Parse CLI arguments
    let startup_items = parse_cli_args();
    let _ = STARTUP_ITEMS.set(startup_items);

    // Cleanup old executable
    if let Ok(exe) = std::env::current_exe() {
        let old_exe = exe.with_extension("old");
        if old_exe.exists() {
             let _ = std::fs::remove_file(old_exe);
        }
    }
    
    // Initialize COM
    let _ = CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED);

    // Get Instance Handle Manually
    let instance = GetModuleHandleW(ptr::null());
    
    let hwnd_main = match ui::window::create_main_window(instance) {
        Ok(h) => h,
        Err(e) => {
            let msg = to_wstring(&("Failed to create main window: ".to_string() + &e.to_string()));
            MessageBoxW(std::ptr::null_mut(), msg.as_ptr(), w!("Error").as_ptr(), MB_ICONERROR | MB_OK);
            ExitProcess(1);
            std::ptr::null_mut()
        }
    };

    // Message Loop
    ui::framework::run_message_loop(hwnd_main);

    // Clean up
    CoUninitialize();
    
    // Explicit Process Termination
    ExitProcess(0);
}
