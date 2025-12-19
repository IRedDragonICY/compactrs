#![allow(non_snake_case, non_camel_case_types)]

use windows_sys::core::{GUID, HRESULT, PCWSTR};
use windows_sys::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows_sys::Win32::System::Com::STGM_READ;
use std::ffi::c_void;
use crate::utils::to_wstring;

// --- GUID Definitions ---
const CLSID_SHELL_LINK: GUID = GUID { data1: 0x00021401, data2: 0x0000, data3: 0x0000, data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46] };
const IID_ISHELL_LINK_W: GUID = GUID { data1: 0x000214F9, data2: 0x0000, data3: 0x0000, data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46] };
const IID_IPERSIST_FILE: GUID = GUID { data1: 0x0000010b, data2: 0x0000, data3: 0x0000, data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46] };

// --- VTable Definitions ---

#[repr(C)]
struct IShellLinkWVtbl {
    pub QueryInterface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(*mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub GetPath: unsafe extern "system" fn(*mut c_void, PCWSTR, i32, *mut c_void, u32) -> HRESULT,
    // We only need GetPath, but we must provide padding for the rest of the VTable if we were to implement more.
    // However, since we are only calling GetPath (index 3) and IUnknown methods (0-2),
    // and we are NOT implementing the interface but CALLING it,
    // we only need to define the struct up to the method we call.
    // Wait, strictly speaking, to map the VTable layout correctly for calling,
    // we just need the function pointers in the correct order.
}

#[repr(C)]
struct IPersistFileVtbl {
    pub QueryInterface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(*mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub GetClassID: unsafe extern "system" fn(*mut c_void, *mut GUID) -> HRESULT, // IPersist
    pub IsDirty: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    pub Load: unsafe extern "system" fn(*mut c_void, PCWSTR, u32) -> HRESULT,
    // Methods after Load are not used, so we can omit them in this definition.
}

/// Resolves a shortcut (.lnk) file to its target path.
/// Returns None if resolution fails or if the path is not a shortcut.
pub fn resolve_shortcut(path: &str) -> Option<String> {
    if !path.to_lowercase().ends_with(".lnk") {
        return None;
    }

    unsafe {
        let mut p_shell_link: *mut c_void = std::ptr::null_mut();
        
        // 1. CoCreateInstance to get IShellLinkW
        let hr = CoCreateInstance(
            &CLSID_SHELL_LINK, 
            std::ptr::null_mut(), 
            CLSCTX_INPROC_SERVER, 
            &IID_ISHELL_LINK_W, 
            &mut p_shell_link
        );

        if hr != 0 {
            return None;
        }

        let shell_link = p_shell_link as *mut *mut IShellLinkWVtbl;
        let shell_link_vtbl = (*shell_link).as_ref().unwrap();

        // 2. QueryInterface for IPersistFile (to load the file)
        let mut p_persist_file: *mut c_void = std::ptr::null_mut();
        let hr = (shell_link_vtbl.QueryInterface)(p_shell_link, &IID_IPERSIST_FILE, &mut p_persist_file);

        if hr != 0 {
            (shell_link_vtbl.Release)(p_shell_link);
            return None;
        }

        let persist_file = p_persist_file as *mut *mut IPersistFileVtbl;
        let persist_file_vtbl = (*persist_file).as_ref().unwrap();

        // 3. Load the .lnk file
        let path_w = to_wstring(path);
        let hr = (persist_file_vtbl.Load)(p_persist_file, path_w.as_ptr(), STGM_READ);

        if hr != 0 {
            (persist_file_vtbl.Release)(p_persist_file);
            (shell_link_vtbl.Release)(p_shell_link);
            return None;
        }

        // 4. Get the target path using IShellLinkW
        // MAX_PATH is usually 260, but let's use a larger buffer to be safe (e.g. 32768 for long paths support in some contexts, though IShellLink might limited)
        // Standard MAX_PATH is 260.
        let mut target_path = [0u16; 32768]; 
        // WIN32_FIND_DATAW is needed as the 4th argument, but can be NULL if we don't care about it?
        // documentation says: pfd [out, optional] in newer versions, but older docs say it's required.
        // Let's allocate a dummy buffer for WIN32_FIND_DATAW just in case.
        // WIN32_FIND_DATAW is 592 bytes.
        // windows_sys::Win32::Storage::FileSystem::WIN32_FIND_DATAW
        
        let mut fd: windows_sys::Win32::Storage::FileSystem::WIN32_FIND_DATAW = std::mem::zeroed();

        // flags: SLGP_RAWPATH (0x1) or SLGP_UNCPRIORITY (0x2). 0 is standard.
        // We use 0.
        let hr = (shell_link_vtbl.GetPath)(
            p_shell_link, 
            target_path.as_mut_ptr(), 
            target_path.len() as i32, 
            &mut fd as *mut _ as *mut c_void, 
            0
        );

        // Cleanup
        (persist_file_vtbl.Release)(p_persist_file);
        (shell_link_vtbl.Release)(p_shell_link);

        if hr == 0 {
            // Success
            // Find null terminator
            let len = (0..).take_while(|&i| target_path[i] != 0).count();
            let slice = std::slice::from_raw_parts(target_path.as_ptr(), len);
            if let Ok(s) = String::from_utf16(slice) {
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }

    None
}
