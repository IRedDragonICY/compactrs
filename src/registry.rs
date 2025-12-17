//! Windows Registry operations for Explorer Context Menu integration.
//!
//! Uses the SubCommands pattern to create cascading context menus without
//! requiring a COM shell extension DLL.

use crate::utils::to_wstring;
use windows_sys::Win32::System::Registry::{
    RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, RegCloseKey, RegOpenKeyExW,
    HKEY, HKEY_CLASSES_ROOT, KEY_WRITE, KEY_READ, REG_SZ, REG_OPTION_NON_VOLATILE,
};
use windows_sys::Win32::Foundation::ERROR_SUCCESS;

/// Get the current executable path as a string
fn get_exe_path() -> Option<String> {
    std::env::current_exe().ok()?.to_str().map(|s| s.to_string())
}

/// Create a registry key and return its handle
unsafe fn create_key(parent: HKEY, subkey: &str) -> Result<HKEY, String> { unsafe {
    let wide_subkey = to_wstring(subkey);
    let mut hkey: HKEY = std::ptr::null_mut();
    let mut disposition = 0u32;
    
    let result = RegCreateKeyExW(
        parent,
        wide_subkey.as_ptr(),
        0,
        std::ptr::null(),
        REG_OPTION_NON_VOLATILE,
        KEY_WRITE,
        std::ptr::null(),
        &mut hkey,
        &mut disposition,
    );
    
    if result != ERROR_SUCCESS {
        return Err(format!("RegCreateKeyExW failed: {}", result));
    }
    
    Ok(hkey)
}}

/// Set a string value in a registry key
unsafe fn set_value(hkey: HKEY, name: Option<&str>, value: &str) -> Result<(), String> { unsafe {
    let wide_value = to_wstring(value);
    
    // Calculate bytes including null terminator (u16 * 2 bytes)
    // to_wstring includes null terminator.
    let value_bytes_ptr = wide_value.as_ptr() as *const u8;
    let value_size = (wide_value.len() * 2) as u32;
    
    if let Some(name) = name {
        let wide_name = to_wstring(name);
        let result = RegSetValueExW(
            hkey,
            wide_name.as_ptr(),
            0,
            REG_SZ,
            value_bytes_ptr,
            value_size,
        );
        if result != ERROR_SUCCESS {
            return Err(format!("RegSetValueExW (named) failed: {}", result));
        }
    } else {
        // Set default value (empty name)
        let result = RegSetValueExW(
            hkey,
            std::ptr::null(),
            0,
            REG_SZ,
            value_bytes_ptr,
            value_size,
        );
        if result != ERROR_SUCCESS {
            return Err(format!("RegSetValueExW (default) failed: {}", result));
        }
    }
    
    Ok(())
}}

/// Close a registry key handle
unsafe fn close_key(hkey: HKEY) { unsafe {
    RegCloseKey(hkey);
}}

/// Create the context menu entries for a given root key path (e.g., "*" or "Directory")
unsafe fn create_menu_for_root(root_path: &str, exe_path: &str) -> Result<(), String> { unsafe {
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
}}

/// Delete the context menu entries for a given root key path
unsafe fn delete_menu_for_root(root_path: &str) -> Result<(), String> { unsafe {
    let key_path = format!("{}\\shell\\CompactRS", root_path);
    let wide_path = to_wstring(&key_path);
    
    // RegDeleteTreeW deletes a key and all its subkeys
    // It returns an error if the key doesn't exist, which we can ignore
    let _ = RegDeleteTreeW(HKEY_CLASSES_ROOT, wide_path.as_ptr());
    
    Ok(())
}}

/// Register the CompactRS context menu for files and directories.
///
/// Creates cascading context menu entries under:
/// - HKEY_CLASSES_ROOT\*\shell\CompactRS (for files)
/// - HKEY_CLASSES_ROOT\Directory\shell\CompactRS (for directories)
///
/// # Returns
/// - `Ok(())` on success
/// - `Err(...)` if registry operations fail (e.g., permission denied)
pub fn register_context_menu() -> Result<(), String> {
    let exe_path = get_exe_path().ok_or_else(|| {
        "Failed to get current executable path".to_string()
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
pub fn unregister_context_menu() -> Result<(), String> {
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
    let key_path = to_wstring("*\\shell\\CompactRS");
    let mut hkey: HKEY = std::ptr::null_mut();
    
    unsafe {
        let result = RegOpenKeyExW(
            HKEY_CLASSES_ROOT,
            key_path.as_ptr(),
            0,
            KEY_READ,
            &mut hkey,
        );
        
        if result == ERROR_SUCCESS {
            RegCloseKey(hkey);
            true
        } else {
            false
        }
    }
}
