#![windows_subsystem = "windows"]

use windows::core::{Result, w};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MessageBoxW, TranslateMessage, MB_ICONERROR, MB_OK, MSG, WM_QUIT,
};

pub mod ui;
pub mod engine; // Make sure the engine module is available to the rest of the app
pub mod config;

fn main() -> Result<()> {
    // 1. Initialize GUI
    // The implementation details are in ui::window.
    unsafe {
        // Initialize COM for IFileOpenDialog
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let instance = GetModuleHandleW(None)?;
        
        // We'll return the Handle to the main window, though we might not strict need it here
        // as the message loop drives everything.
        if let Err(e) = ui::window::create_main_window(instance.into()) {
            MessageBoxW(None, w!("Failed to create main window!"), w!("Error"), MB_ICONERROR | MB_OK);
            return Err(e);
        }

        // 2. Message Loop
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
