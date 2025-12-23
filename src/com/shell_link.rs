#![allow(non_snake_case, non_camel_case_types)]

use crate::types::*;
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
    pub GetPath: unsafe extern "system" fn(*mut c_void, LPCWSTR, i32, *mut c_void, u32) -> HRESULT,
}

#[repr(C)]
struct IPersistFileVtbl {
    pub QueryInterface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(*mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub GetClassID: unsafe extern "system" fn(*mut c_void, *mut GUID) -> HRESULT, // IPersist
    pub IsDirty: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    pub Load: unsafe extern "system" fn(*mut c_void, LPCWSTR, u32) -> HRESULT,
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
        let mut target_path = [0u16; 32768]; 
        
        let mut fd: WIN32_FIND_DATAW = std::mem::zeroed();

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
