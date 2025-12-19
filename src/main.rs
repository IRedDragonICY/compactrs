#![windows_subsystem = "windows"]

use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MessageBoxW, TranslateMessage, 
    MB_ICONERROR, MB_OK, MSG, WM_QUIT, SW_SHOW, FindWindowW, SendMessageW,
    WM_COPYDATA, WM_COMMAND, WM_KEYDOWN,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_SHIFT, VK_DELETE};
use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
use windows_sys::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
use std::ptr;
use std::sync::OnceLock;

use crate::ui::controls::{IDC_BTN_ADD_FILES, IDC_BTN_ADD_FOLDER, IDC_BTN_REMOVE};

pub mod ui;
pub mod engine;
pub mod config;
pub mod registry;
pub mod utils;
pub mod updater;
pub mod json;
pub mod com;

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
    // 0. Single Instance Check (Before Admin Check)
    unsafe {
        let class_name = to_wstring("CompactRS_Class");
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
                    // Format: PATH|ALGO|ACTION
                    let payload = format!("{}|{}|{}", item.path, algo_str, action_str);
                    let payload_w = to_wstring(&payload);
                    let cds = COPYDATASTRUCT {
                        dwData: 0xB00B,
                        cbData: (payload_w.len() * 2) as u32,
                        lpData: payload_w.as_ptr() as *mut _,
                    };
                    SendMessageW(hwnd_existing, WM_COPYDATA, 0, &cds as *const _ as isize);
                }
            }
            std::process::exit(0);
        }
    }

    // 1. Runtime Admin Check

    if !is_admin() {
        unsafe {
            // Attempt to relaunch as administrator
            let mut filename = [0u16; 32768]; // MAX_PATH is 260 but wide paths can be longer, using safe buffer
            let len = GetModuleFileNameW(std::ptr::null_mut(), filename.as_mut_ptr(), filename.len() as u32);
            
            if len > 0 {
                let operation = to_wstring("runas");
                // Collect existing arguments and quote them if necessary to preserve spaces during elevation
                let args: Vec<String> = std::env::args().skip(1).collect();
                let args_str = args.iter()
                    .map(|arg| {
                        if arg.contains(' ') || arg.contains('\t') || arg.is_empty() {
                            format!("\"{}\"", arg)
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
        // We'll update create_main_window to accept isize (HINSTANCE)
        let hwnd_main = match ui::window::create_main_window(instance) {
            Ok(h) => h,
            Err(e) => {
                let msg = to_wstring(&("Failed to create main window: ".to_string() + &e.to_string()));
                MessageBoxW(std::ptr::null_mut(), msg.as_ptr(), to_wstring("Error").as_ptr(), MB_ICONERROR | MB_OK);
                std::process::exit(1);
            }
        };

        // Message Loop
        let mut msg: MSG = std::mem::zeroed();
        // GetMessageW returns BOOL (i32). strict > 0 check for success.
        // HWND parameter should be NULL (0/null_mut) to retrieve messages for any window belonging to the current thread
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_QUIT {
                break;
            }

            // Global Shortcuts Interception
            if msg.message == WM_KEYDOWN {
                let vk = msg.wParam as u16;
                let ctrl_pressed = (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
                let shift_pressed = (GetKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
                
                let mut handled = false;
                
                if ctrl_pressed {
                    match vk {
                        0x4F => { // 'O'
                            if shift_pressed {
                                // Ctrl+Shift+O -> Add Folder
                                SendMessageW(hwnd_main, WM_COMMAND, IDC_BTN_ADD_FOLDER as usize, 0);
                                handled = true;
                            } else {
                                // Ctrl+O -> Add Files
                                SendMessageW(hwnd_main, WM_COMMAND, IDC_BTN_ADD_FILES as usize, 0);
                                handled = true;
                            }
                        },
                         0x41 => { // 'A' - Select All
                             // Only if we want to force global Select All. 
                             // But let's only do it if focus is not in an edit control (not present here).
                             // We'll let ListView handle it if focused, or intercept? 
                             // Since user complained about shortcuts, let's force it via Main Window logic
                             // But Main Window logic for Ctrl+A sets ListView selection.
                             // So it is safe to always trigger it.
                             // We need to route it to Main Window wnd_proc logic for 'A' or manually trigger logic.
                             // Window.rs wnd_proc has 0x41 handler. 
                             // We can't SendMessage(WM_KEYDOWN) effectively to parent if child has focus without refocusing?
                             // Actually, we can just invoke logic. But window.rs logic is inside wnd_proc.
                             // Sending WM_KEYDOWN to hwnd_main works but wnd_proc handles logic.
                             // Let's forward the KEYDOWN message to hwnd_main!
                             SendMessageW(hwnd_main, WM_KEYDOWN, vk as usize, 0);
                             handled = true;
                         },
                        _ => {}
                    }
                } else if vk == VK_DELETE as u16 {
                     // Propagate Delete to main window to trigger removal
                     // Send WM_COMMAND directly
                     SendMessageW(hwnd_main, WM_COMMAND, IDC_BTN_REMOVE as usize, 0);
                     handled = true;
                }
                
                if handled {
                    continue;
                }
            }
            
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
