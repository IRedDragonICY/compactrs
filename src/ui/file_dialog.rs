#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

use crate::types::*;


use std::ffi::c_void;

// --- COM Interface Definitions (VTables) ---
// Imported from crate::types::*;


unsafe fn run_dialog_internal(options_flags: u32) -> Result<Vec<String>, HRESULT> { unsafe {
    let mut p_dialog: *mut c_void = std::ptr::null_mut();
    let hr = CoCreateInstance(&CLSID_FILE_OPEN_DIALOG, std::ptr::null_mut(), CLSCTX_ALL, &IID_IFILE_OPEN_DIALOG, &mut p_dialog);
    if hr != 0 { return Err(hr); }

    let dialog = p_dialog as *mut *mut IFileOpenDialogVtbl;
    let vtbl = (*dialog).as_ref().unwrap();

    let mut current_options = 0;
    (vtbl.get_options)(p_dialog, &mut current_options);
    (vtbl.set_options)(p_dialog, current_options | options_flags);
    
    let hr = (vtbl.show)(p_dialog, 0); 
    if hr != 0 {
        (vtbl.release)(p_dialog);
        return Err(hr);
    }

    let mut p_results: *mut c_void = std::ptr::null_mut();
    let hr = (vtbl.get_results)(p_dialog, &mut p_results);
    if hr != 0 {
        (vtbl.release)(p_dialog);
        return Err(hr);
    }

    let results = p_results as *mut *mut IShellItemArrayVtbl;
    let results_vtbl = (*results).as_ref().unwrap();

    let mut count = 0;
    (results_vtbl.get_count)(p_results, &mut count);
    
    let mut paths = Vec::with_capacity(count as usize);
    for i in 0..count {
        let mut p_item: *mut c_void = std::ptr::null_mut();
        if (results_vtbl.get_item_at)(p_results, i, &mut p_item) == 0 {
            if let Some(path) = get_path_from_item(p_item) {
                paths.push(path);
            }
            // IShellItem::Release
            let item = p_item as *mut *mut IShellItemVtbl;
            let item_vtbl = (*item).as_ref().unwrap();
            (item_vtbl.release)(p_item);
        }
    }

    // 5. Cleanup
    (results_vtbl.release)(p_results);
    (vtbl.release)(p_dialog);

    Ok(paths)
}}

/// Helper to extract a filesystem path string from an IShellItem pointer.
/// Handles GetDisplayName, string conversion, and CoTaskMemFree.
unsafe fn get_path_from_item(p_item: *mut c_void) -> Option<String> { unsafe {
    let item = p_item as *mut *mut IShellItemVtbl;
    let item_vtbl = (*item).as_ref().unwrap();
    
    let mut name_ptr: PCWSTR = std::ptr::null();
    if (item_vtbl.get_display_name)(p_item, SIGDN_FILESYSPATH, &mut name_ptr) == 0 && !name_ptr.is_null() {
        let len = (0..).take_while(|&i| *name_ptr.offset(i) != 0).count();
        let slice = std::slice::from_raw_parts(name_ptr, len);
        let result = String::from_utf16(slice).ok();
        CoTaskMemFree(name_ptr as *mut _);
        return result;
    }
    None
}}

/// Pick multiple files using the native IFileOpenDialog.
pub unsafe fn pick_files() -> Result<Vec<String>, HRESULT> { unsafe {
    run_dialog_internal(FOS_FORCEFILESYSTEM | FOS_ALLOWMULTISELECT)
}}

/// Pick a single folder using the native IFileOpenDialog.
pub unsafe fn pick_folder() -> Result<String, HRESULT> { unsafe {
    let paths = run_dialog_internal(FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)?;
    paths.into_iter().next().ok_or(-1) // Return first item or generic error if empty (though Cancel handles empty)
}}