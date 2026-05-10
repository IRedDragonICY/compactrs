#![windows_subsystem = "windows"]
#![no_main]
#![allow(unsafe_op_in_unsafe_fn)]

use std::alloc::{GlobalAlloc, Layout};
use crate::types::*;
use std::ptr;
use std::sync::OnceLock;

struct Win32Allocator;

unsafe impl GlobalAlloc for Win32Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();
        
        if align <= 16 {
            unsafe { crate::types::HeapAlloc(crate::types::GetProcessHeap(), 0, size) as *mut u8 }
        } else {
            // Over-allocate to ensure alignment and room to store the original pointer
            let offset = align - 1 + std::mem::size_of::<*mut u8>();
            let ptr = unsafe { crate::types::HeapAlloc(crate::types::GetProcessHeap(), 0, size + offset) as *mut u8 };
            if ptr.is_null() { return std::ptr::null_mut(); }
            
            let aligned = ((ptr as usize + offset) & !(align - 1)) as *mut u8;
            // Store the original pointer right before the aligned pointer
            unsafe { *(aligned.sub(std::mem::size_of::<*mut u8>()) as *mut *mut u8) = ptr };
            aligned
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() { return; }
        let align = layout.align();
        let orig_ptr = if align <= 16 {
            ptr
        } else {
            unsafe { *(ptr.sub(std::mem::size_of::<*mut u8>()) as *mut *mut u8) }
        };
        unsafe { crate::types::HeapFree(crate::types::GetProcessHeap(), 0, orig_ptr as *mut _); }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();
        
        if align <= 16 {
            // HEAP_ZERO_MEMORY = 0x00000008
            unsafe { crate::types::HeapAlloc(crate::types::GetProcessHeap(), 0x00000008, size) as *mut u8 }
        } else {
            let ptr = unsafe { self.alloc(layout) };
            if !ptr.is_null() {
                unsafe { std::ptr::write_bytes(ptr, 0, size) };
            }
            ptr
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let align = layout.align();
        
        if align <= 16 {
            unsafe { crate::types::HeapReAlloc(crate::types::GetProcessHeap(), 0, ptr as *mut _, new_size) as *mut u8 }
        } else {
            let new_ptr = unsafe { self.alloc(Layout::from_size_align_unchecked(new_size, align)) };
            if !new_ptr.is_null() {
                let copy_size = std::cmp::min(layout.size(), new_size);
                unsafe { std::ptr::copy_nonoverlapping(ptr, new_ptr, copy_size) };
                unsafe { self.dealloc(ptr, layout) };
            }
            new_ptr
        }
    }
}

#[global_allocator]
static ALLOCATOR: Win32Allocator = Win32Allocator;

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
                        "lznt1" => WofAlgorithm::Lznt1,
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

#[unsafe(no_mangle)]
pub unsafe extern "system" fn WinMainCRTStartup() {
    // Initialize Theme System early
    crate::ui::theme::init();
    crate::ui::theme::set_preferred_app_mode(true);

    let config = crate::config::AppConfig::load();
    crate::ui::theme::update_ui_scale(config.ui_scale_multiplier);

    // Parse CLI arguments
    let startup_items = parse_cli_args();
    let _ = STARTUP_ITEMS.set(startup_items.clone());

    // If context menu dialog is enabled and we have args, bypass single instance and show dialog directly
    if !startup_items.is_empty() && config.context_menu_dialog_only {
        let _ = CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED);
        crate::ui::dialogs::context_dialog::show(startup_items.clone(), config);
        CoUninitialize();
        ExitProcess(0);
    }

    // 0. Single Instance Check
    let class_name = w!("CompactRS_Class");
    let hwnd_existing = FindWindowW(class_name.as_ptr(), std::ptr::null());
    
    if hwnd_existing != std::ptr::null_mut() {
        if !startup_items.is_empty() {
            for item in startup_items {
                let algo_str = match item.algorithm {
                        WofAlgorithm::Xpress4K => "xpress4k",
                        WofAlgorithm::Xpress8K => "xpress8k",
                        WofAlgorithm::Xpress16K => "xpress16k",
                        WofAlgorithm::Lzx => "lzx",
                        WofAlgorithm::Lznt1 => "lznt1",
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

    if config.enable_context_menu {
        let _ = crate::registry::register_context_menu();
    }

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
            let msg_text = ["Failed to create main window: ", &e].concat();
            let msg = to_wstring(&msg_text);
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