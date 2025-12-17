//! Windows Registry operations for Explorer Context Menu integration.
//!
//! Uses the SubCommands pattern to create cascading context menus without
//! requiring a COM shell extension DLL.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::core::{Result, PCWSTR};
use windows::Win32::System::Registry::{
    RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, RegCloseKey, RegOpenKeyExW,
    HKEY, HKEY_CLASSES_ROOT, KEY_WRITE, KEY_READ, REG_SZ, REG_OPTION_NON_VOLATILE,
    REG_CREATE_KEY_DISPOSITION,
};
use windows::Win32::Foundation::ERROR_SUCCESS;

/// Convert a Rust string to a null-terminated wide string (UTF-16)
fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// Get the current executable path as a string
fn get_exe_path() -> Option<String> {
    std::env::current_exe().ok()?.to_str().map(|s| s.to_string())
}

/// Create a registry key and return its handle
unsafe fn create_key(parent: HKEY, subkey: &str) -> Result<HKEY> {
    let wide_subkey = to_wide(subkey);
    let mut hkey = HKEY::default();
    let mut disposition = REG_CREATE_KEY_DISPOSITION::default();
    
    unsafe {
        let result = RegCreateKeyExW(
            parent,
            PCWSTR(wide_subkey.as_ptr()),
            Some(0),
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            Some(&mut disposition),
        );
        
        if result != ERROR_SUCCESS {
            return Err(windows::core::Error::from_thread());
        }
    }
    
    Ok(hkey)
}

/// Set a string value in a registry key
unsafe fn set_value(hkey: HKEY, name: Option<&str>, value: &str) -> Result<()> {
    let wide_value = to_wide(value);
    let value_bytes = unsafe {
        std::slice::from_raw_parts(
            wide_value.as_ptr() as *const u8,
            wide_value.len() * 2,
        )
    };
    
    unsafe {
        if let Some(name) = name {
            let wide_name = to_wide(name);
            let result = RegSetValueExW(
                hkey,
                PCWSTR(wide_name.as_ptr()),
                Some(0),
                REG_SZ,
                Some(value_bytes),
            );
            if result != ERROR_SUCCESS {
                return Err(windows::core::Error::from_thread());
            }
        } else {
            // Set default value (empty name)
            let result = RegSetValueExW(
                hkey,
                PCWSTR::null(),
                Some(0),
                REG_SZ,
                Some(value_bytes),
            );
            if result != ERROR_SUCCESS {
                return Err(windows::core::Error::from_thread());
            }
        }
    }
    
    Ok(())
}

/// Close a registry key handle
unsafe fn close_key(hkey: HKEY) {
    unsafe {
        let _ = RegCloseKey(hkey);
    }
}

/// Create the context menu entries for a given root key path (e.g., "*" or "Directory")
unsafe fn create_menu_for_root(root_path: &str, exe_path: &str) -> Result<()> {
    unsafe {
        // Create: HKCR\{root_path}\shell\CompactRS
        let shell_key_path = format!("{}\\shell\\CompactRS", root_path);
        let main_key = create_key(HKEY_CLASSES_ROOT, &shell_key_path)?;
        
        // Set MUIVerb for display name
        set_value(main_key, Some("MUIVerb"), "CompactRS")?;
        
        // Set Icon (use the exe as icon source)
        set_value(main_key, Some("Icon"), exe_path)?;
        
        // Set SubCommands to empty string to enable cascading menu
        set_value(main_key, Some("SubCommands"), "")?;
        
        close_key(main_key);
        
        // Create subcommands under: HKCR\{root_path}\shell\CompactRS\shell
        let submenu_base = format!("{}\\shell\\CompactRS\\shell", root_path);
        
        // Define all menu items
        let menu_items = [
            ("01_xpress4k", "Compress as XPRESS4K", "--algo xpress4k"),
            ("02_xpress8k", "Compress as XPRESS8K", "--algo xpress8k"),
            ("03_xpress16k", "Compress as XPRESS16K", "--algo xpress16k"),
            ("04_lzx", "Compress as LZX", "--algo lzx"),
            ("05_decompress", "Decompress", "--action decompress"),
        ];
        
        for (id, label, args) in menu_items {
            // Create the subcommand key
            let item_path = format!("{}\\{}", submenu_base, id);
            let item_key = create_key(HKEY_CLASSES_ROOT, &item_path)?;
            set_value(item_key, None, label)?;
            close_key(item_key);
            
            // Create the command key
            let cmd_path = format!("{}\\{}\\command", submenu_base, id);
            let cmd_key = create_key(HKEY_CLASSES_ROOT, &cmd_path)?;
            
            // Command: "path\to\compactrs.exe" --path "%1" {args}
            let command = format!("\"{}\" --path \"%1\" {}", exe_path, args);
            set_value(cmd_key, None, &command)?;
            close_key(cmd_key);
        }
        
        Ok(())
    }
}

/// Delete the context menu entries for a given root key path
unsafe fn delete_menu_for_root(root_path: &str) -> Result<()> {
    let key_path = format!("{}\\shell\\CompactRS", root_path);
    let wide_path = to_wide(&key_path);
    
    unsafe {
        // RegDeleteTreeW deletes a key and all its subkeys
        // It returns an error if the key doesn't exist, which we can ignore
        let _ = RegDeleteTreeW(HKEY_CLASSES_ROOT, PCWSTR(wide_path.as_ptr()));
    }
    
    Ok(())
}

/// Register the CompactRS context menu for files and directories.
///
/// Creates cascading context menu entries under:
/// - HKEY_CLASSES_ROOT\*\shell\CompactRS (for files)
/// - HKEY_CLASSES_ROOT\Directory\shell\CompactRS (for directories)
///
/// # Returns
/// - `Ok(())` on success
/// - `Err(...)` if registry operations fail (e.g., permission denied)
pub fn register_context_menu() -> Result<()> {
    let exe_path = get_exe_path().ok_or_else(|| {
        windows::core::Error::from_thread()
    })?;
    
    unsafe {
        // Register for files (*)
        create_menu_for_root("*", &exe_path)?;
        
        // Register for directories
        create_menu_for_root("Directory", &exe_path)?;
    }
    
    Ok(())
}

/// Unregister the CompactRS context menu.
///
/// Removes all registry entries created by `register_context_menu()`.
///
/// # Returns
/// - `Ok(())` on success (also returns Ok if entries don't exist)
pub fn unregister_context_menu() -> Result<()> {
    unsafe {
        // Remove from files (*)
        delete_menu_for_root("*")?;
        
        // Remove from directories
        delete_menu_for_root("Directory")?;
    }
    
    Ok(())
}

/// Check if the context menu is currently registered.
///
/// # Returns
/// - `true` if the registry key exists
/// - `false` otherwise
pub fn is_context_menu_registered() -> bool {
    let key_path = to_wide("*\\shell\\CompactRS");
    let mut hkey = HKEY::default();
    
    unsafe {
        let result = RegOpenKeyExW(
            HKEY_CLASSES_ROOT,
            PCWSTR(key_path.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        );
        
        if result == ERROR_SUCCESS {
            let _ = RegCloseKey(hkey);
            true
        } else {
            false
        }
    }
}
