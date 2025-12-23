#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

use crate::types::*;


use std::ffi::c_void;

// --- COM Interface Definitions (VTables) ---
// Kept minimal and private to this module to reduce global namespace pollution.

#[repr(C)]
struct IFileOpenDialogVtbl {
    pub query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    pub release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub show: unsafe extern "system" fn(*mut c_void, isize) -> HRESULT, // HWND is isize
    pub set_file_types: unsafe extern "system" fn(*mut c_void, u32, *const c_void) -> HRESULT,
    pub set_file_type_index: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
    pub get_file_type_index: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    pub advise: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut u32) -> HRESULT,
    pub unadvise: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
    pub set_options: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
    pub get_options: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    pub set_default_folder: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    pub set_folder: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    pub get_folder: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub get_current_selection: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub set_file_name: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub get_file_name: unsafe extern "system" fn(*mut c_void, *mut PCWSTR) -> HRESULT,
    pub set_title: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub set_ok_button_label: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub set_file_name_label: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub get_result: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT, 
    pub add_place: unsafe extern "system" fn(*mut c_void, *mut c_void, u32) -> HRESULT,
    pub set_default_extension: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub close: unsafe extern "system" fn(*mut c_void, HRESULT) -> HRESULT,
    pub set_client_guid: unsafe extern "system" fn(*mut c_void, *const GUID) -> HRESULT,
    pub clear_client_data: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    pub set_filter: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    pub get_results: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub get_selected_items: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}

#[repr(C)]
struct IShellItemVtbl {
    pub query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    pub release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub bind_to_handler: unsafe extern "system" fn(*mut c_void, *mut c_void, *const GUID, *const GUID, *mut *mut c_void) -> HRESULT,
    pub get_parent: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub get_display_name: unsafe extern "system" fn(*mut c_void, u32, *mut PCWSTR) -> HRESULT,
    pub get_attributes: unsafe extern "system" fn(*mut c_void, u32, *mut u32) -> HRESULT,
    pub compare: unsafe extern "system" fn(*mut c_void, *mut c_void, u32, *mut i32) -> HRESULT,
}

#[repr(C)]
struct IShellItemArrayVtbl {
    pub query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    pub release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub bind_to_handler: unsafe extern "system" fn(*mut c_void, *mut c_void, *const GUID, *const GUID, *mut *mut c_void) -> HRESULT,
    pub get_property_store: unsafe extern "system" fn(*mut c_void, u32, *const GUID, *mut *mut c_void) -> HRESULT,
    pub get_property_description_list: unsafe extern "system" fn(*mut c_void, *const GUID, *const GUID, *mut *mut c_void) -> HRESULT,
    pub get_attributes: unsafe extern "system" fn(*mut c_void, u32, u32, *mut c_void) -> HRESULT,
    pub get_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    pub get_item_at: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
    pub enum_items: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}

const CLSID_FILE_OPEN_DIALOG: GUID = GUID { data1: 0xDC1C5A9C, data2: 0xE88A, data3: 0x4DDE, data4: [0xA5, 0xA1, 0x60, 0xF8, 0x2A, 0x20, 0xAE, 0xF7] };
const IID_IFILE_OPEN_DIALOG: GUID = GUID { data1: 0xd57c7288, data2: 0xd4ad, data3: 0x4768, data4: [0xbe, 0x02, 0x9d, 0x96, 0x95, 0x32, 0xd9, 0x60] };
const FOS_PICKFOLDERS: u32 = 0x20;
const FOS_FORCEFILESYSTEM: u32 = 0x40;
const FOS_ALLOWMULTISELECT: u32 = 0x200;
const SIGDN_FILESYSPATH: u32 = 0x80058000;

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