#![allow(non_snake_case, non_camel_case_types)]
use std::ffi::c_void;
use std::fs::File;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::fs::OpenOptionsExt; 
use crate::types::*;

// --- Manual Bindings & Constants ---

// IOCTL Codes
const FSCTL_SET_COMPRESSION: u32 = 0x9C040;
const FSCTL_SET_EXTERNAL_BACKING: u32 = 0x9030C;
const FSCTL_GET_EXTERNAL_BACKING: u32 = 0x90310;
const FSCTL_DELETE_EXTERNAL_BACKING: u32 = 0x90314;

// Security Constants
const SE_PRIVILEGE_ENABLED: u32 = 0x00000002;
const TOKEN_ADJUST_PRIVILEGES: u32 = 0x0020;
const TOKEN_QUERY: u32 = 0x0008;

#[repr(C)]
struct LUID_AND_ATTRIBUTES {
    Luid: LUID,
    Attributes: u32,
}

#[repr(C)]
struct TOKEN_PRIVILEGES {
    PrivilegeCount: u32,
    Privileges: [LUID_AND_ATTRIBUTES; 1],
}



use crate::utils::{to_wstring, PathBuffer};

pub fn get_real_file_size(path: &str) -> u64 {
    unsafe {
        let wide = PathBuffer::from(path);
        let mut high: u32 = 0;
        let win_api = crate::engine::dynamic_import::WinApi::get();
        let low = (win_api.GetCompressedFileSizeW.unwrap())(wide.as_ptr(), &mut high);
        
        if low == u32::MAX && GetLastError() != 0 {
            // If error, fall back to logical size or 0
            std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
        } else {
            ((high as u64) << 32) | (low as u64)
        }
    }
}

/// Get the WOF compression algorithm used for a file
/// Returns None if file is not WOF-compressed, Some(algorithm) if it is
pub fn get_wof_algorithm(path: &str) -> Option<WofAlgorithm> {
    unsafe {
        let wide = PathBuffer::from(path);
        let win_api = crate::engine::dynamic_import::WinApi::get();
        let handle = (win_api.CreateFileW.unwrap())(
            wide.as_ptr(),
            0x80000000, // GENERIC_READ
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),

        );

        if handle == INVALID_HANDLE_VALUE {
            return None;
        }

        let result = get_wof_algorithm_from_handle(handle);
        (win_api.CloseHandle.unwrap())(handle);
        result
    }
}

/// Get the WOF compression algorithm from an already-opened file handle.
/// Returns None if file is not WOF-compressed.
/// 
/// # Safety
/// The handle must be a valid, open file handle with at least read access.
pub fn get_wof_algorithm_from_handle(handle: HANDLE) -> Option<WofAlgorithm> {
    unsafe {
        // Buffer for WOF_EXTERNAL_INFO + FILE_PROVIDER_EXTERNAL_INFO_V1
        let mut out_buffer = [0u8; 1024];
        let mut bytes_returned = 0u32;
        
        let win_api = crate::engine::dynamic_import::WinApi::get();
        let result = (win_api.DeviceIoControl.unwrap())(
            handle,
            FSCTL_GET_EXTERNAL_BACKING,
            std::ptr::null(),
            0,
            out_buffer.as_mut_ptr() as *mut _,
            out_buffer.len() as u32,
            &mut bytes_returned,
            std::ptr::null_mut(),
        );
        
        if result == 0 {
            return None;
        }
        
        // Parse the output buffer
        // First comes WOF_EXTERNAL_INFO (8 bytes), then FILE_PROVIDER_EXTERNAL_INFO_V1 (12 bytes)
        if bytes_returned < 20 {
            return None;
        }
        
        let wof_info = &out_buffer[0..8];
        // provider is at offset 4 (u32)
        let provider = u32::from_le_bytes([wof_info[4], wof_info[5], wof_info[6], wof_info[7]]);
        
        // Check if it's WOF_PROVIDER_FILE (2)
        if provider != 2 {
            return None;
        }
        
        let file_info = &out_buffer[8..20];
        // algorithm is at offset 4 (u32)
        let algorithm = u32::from_le_bytes([file_info[4], file_info[5], file_info[6], file_info[7]]);
        
        match algorithm {
            0 => Some(WofAlgorithm::Xpress4K),
            1 => Some(WofAlgorithm::Lzx),
            2 => Some(WofAlgorithm::Xpress8K),
            3 => Some(WofAlgorithm::Xpress16K),
            _ => None,
        }
    }
}

// WOF Definitions
// Imported from crate::types::*;

// Compression Algorithms
pub const FILE_PROVIDER_COMPRESSION_XPRESS4K: u32 = 0;
pub const FILE_PROVIDER_COMPRESSION_LZX: u32 = 1;
pub const FILE_PROVIDER_COMPRESSION_XPRESS8K: u32 = 2;
pub const FILE_PROVIDER_COMPRESSION_XPRESS16K: u32 = 3;

// NTFS Compression Formats
pub const COMPRESSION_FORMAT_NONE: u16 = 0;
pub const COMPRESSION_FORMAT_DEFAULT: u16 = 1;
pub const COMPRESSION_FORMAT_LZNT1: u16 = 2;

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WofAlgorithm {
    Xpress4K = 0,
    Lzx = 1,
    Xpress8K = 2,
    Xpress16K = 3,
}

impl WofAlgorithm {
    fn to_u32(self) -> u32 {
        self as u32
    }
}

/// Represents the compression state of a file or folder
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CompressionState {
    /// Not compressed (or not WOF compressed)
    None,
    /// Compressed with a specific algorithm (all files if folder)
    Specific(WofAlgorithm),
    /// Contains files with different compression algorithms (folder only)
    Mixed,
}

pub fn compress_file(path: &str, algo: WofAlgorithm, force: bool) -> Result<bool, u32> {
    // First attempt: Normal open with permissive sharing
    let file_result = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(7)
        .open(path);
    
    match file_result {
        Ok(file) => compress_file_handle(&file, algo, force),
        Err(e) => {
            // Check if Access Denied and force is enabled
            if force && e.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32) {
                // Enable backup privileges first
                enable_backup_privileges();
                
                // Remove read-only attribute if set
                force_remove_readonly(path);
                
                // Retry normal open after removing readonly
                if let Ok(file) = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .share_mode(7)
                    .open(path)
                {
                    return compress_file_handle(&file, algo, force);
                }
                
                // Try direct Win32 API with backup semantics
                if let Some(result) = compress_file_with_backup_semantics(path, algo, force) {
                    return result;
                }
            }
            // Return raw OS error as u32
            // Use explicit closure type for correct inference if needed, though this simple map usually works
            Err(e.raw_os_error().unwrap_or(0) as u32)
        }
    }
}

/// Smart compression that opens the file once and reuses the handle.
/// 
/// This eliminates redundant syscalls by:
/// 1. Opening the file once with permissive sharing (Read|Write|Delete = 7)
/// 2. Checking current compression state using the same handle
/// 3. Compressing using the same handle if needed
/// 
/// # Returns
/// - `Ok(true)` if compression succeeded or file was already optimally compressed
/// - `Ok(false)` if compression was not beneficial (OS driver decision)
/// - `Err(error_code)` on failure
pub fn smart_compress(path: &str, target_algo: WofAlgorithm, force: bool) -> Result<bool, u32> {
    // First attempt: Normal open with permissive sharing
    let file_result = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(7) // FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
        .open(path);
    
    match file_result {
        Ok(file) => {
            let handle = file.as_raw_handle() as HANDLE;
            
            // Check current compression state using the SAME handle
            if !force {
                if let Some(current_algo) = get_wof_algorithm_from_handle(handle) {
                    if current_algo == target_algo {
                        // Already compressed with target algorithm, skip
                        return Ok(true);
                    }
                }
            }
            
            // Proceed with compression using the same file handle
            compress_file_handle(&file, target_algo, force)
        }
        Err(e) => {
            // Check if Access Denied and force is enabled
            if force && e.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32) {
                // Try backup semantics path
                smart_compress_with_backup_semantics(path, target_algo, force)
            } else {
                Err(e.raw_os_error().unwrap_or(0) as u32)
            }
        }
    }
}

/// Internal helper for smart_compress with backup semantics
fn smart_compress_with_backup_semantics(path: &str, algo: WofAlgorithm, force: bool) -> Result<bool, u32> {
    // Remove read-only attribute if set
    force_remove_readonly(path);
    
    // Retry normal open after removing readonly
    if let Ok(file) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(7)
        .open(path)
    {
        let handle = file.as_raw_handle() as HANDLE;
        
        // Check current compression state
        if !force {
            if let Some(current_algo) = get_wof_algorithm_from_handle(handle) {
                if current_algo == algo {
                    return Ok(true);
                }
            }
        }
        
        return compress_file_handle(&file, algo, force);
    }
    
    // Try direct Win32 API with backup semantics
    unsafe {
        let wide = PathBuffer::from(path);
        let access = 0x80000000 | 0x40000000; // GENERIC_READ | GENERIC_WRITE
        let win_api = crate::engine::dynamic_import::WinApi::get();
        
        let handle = (win_api.CreateFileW.unwrap())(
            wide.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        );
        
        if handle == INVALID_HANDLE_VALUE {
            return Err(GetLastError());
        }
        
        // Check current compression state
        if !force {
            if let Some(current_algo) = get_wof_algorithm_from_handle(handle) {
                if current_algo == algo {
                    (win_api.CloseHandle.unwrap())(handle);
                    return Ok(true);
                }
            }
        }
        
        // Convert to File for compress_file_handle
        let file = File::from_raw_handle(handle as *mut _);
        compress_file_handle(&file, algo, force)
        // File is dropped here, closing the handle
    }
}


/// Force remove read-only attribute from a file
fn force_remove_readonly(path: &str) {
    unsafe {
        let wide = PathBuffer::from(path);
        let win_api = crate::engine::dynamic_import::WinApi::get();
        
        let attrs = (win_api.GetFileAttributesW.unwrap())(wide.as_ptr());
        if attrs != u32::MAX { // INVALID_FILE_ATTRIBUTES
            // Remove read-only flag
            let new_attrs = attrs & !FILE_ATTRIBUTE_READONLY;
            let new_attrs = if new_attrs == 0 { FILE_ATTRIBUTE_NORMAL } else { new_attrs };
            (win_api.SetFileAttributesW.unwrap())(wide.as_ptr(), new_attrs);
        }
    }
}

/// Set or unset the compressed file attribute (visual indicator only)
/// Set or unset the compressed file attribute (via FSCTL_SET_COMPRESSION)
pub fn set_compressed_attribute(path: &str, enable: bool) {
    unsafe {
        let wide = crate::utils::PathBuffer::from(path);
        
        // Open handle
        let handle = crate::types::CreateFileW(
            wide.as_ptr(),
            crate::types::GENERIC_READ | crate::types::GENERIC_WRITE,
            crate::types::FILE_SHARE_READ | crate::types::FILE_SHARE_WRITE,
            std::ptr::null_mut(),
            crate::types::OPEN_EXISTING,
            crate::types::FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        );
        
        if handle != crate::types::INVALID_HANDLE_VALUE {
            let format: u16 = if enable { 1 } else { 0 }; // 1 = DEFAULT, 0 = NONE
            let mut bytes_ret = 0u32;
            let res = crate::types::DeviceIoControl(
                handle,
                0x9C040, // FSCTL_SET_COMPRESSION
                &format as *const _ as *mut _,
                std::mem::size_of::<u16>() as u32,
                std::ptr::null_mut(),
                0,
                &mut bytes_ret,
                std::ptr::null_mut(),
            );
            
            if res == 0 {
                 let err = crate::types::GetLastError();
                 crate::log_warn!(&["FSCTL failed on: ", path, " Err: ", &err.to_string()].concat());
            } else {
                 // crate::log_trace!(&["Set Compressed Attr: ", path, " = ", &enable.to_string()].concat());
            }
            crate::types::CloseHandle(handle);
        } else {
            let err = crate::types::GetLastError();
            crate::log_warn!(&["Failed to open handle for FSCTL: ", path, " Err: ", &err.to_string()].concat());
        }
    }
}

/// Enable backup and restore privileges for the current process.
/// Call this once per thread for optimal performance (reduces syscalls).
pub fn enable_backup_privileges() {
    unsafe {
        let mut token_handle: HANDLE = std::ptr::null_mut(); // Initialize with null_mut
        let win_api = crate::engine::dynamic_import::WinApi::get();
        
        // Using -1 as pseudo handle for current process
        let current_process = -1isize as HANDLE;
        
        if (win_api.OpenProcessToken.unwrap())(
            current_process,
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token_handle
        ) == 0 {
            return;
        }
        
        let privileges = [
            to_wstring("SeBackupPrivilege"),
            to_wstring("SeRestorePrivilege"),
            to_wstring("SeTakeOwnershipPrivilege"),
            to_wstring("SeSecurityPrivilege"),
        ];
        
        for priv_name in privileges {
            let mut luid = LUID { LowPart: 0, HighPart: 0 };
            if (win_api.LookupPrivilegeValueW.unwrap())(std::ptr::null(), priv_name.as_ptr(), &mut luid as *mut _ as *mut _) != 0 {
                let tp = TOKEN_PRIVILEGES {
                    PrivilegeCount: 1,
                    Privileges: [LUID_AND_ATTRIBUTES {
                        Luid: luid,
                        Attributes: SE_PRIVILEGE_ENABLED,
                    }],
                };
                (win_api.AdjustTokenPrivileges.unwrap())(
                    token_handle,
                    0, // FALSE
                    &tp as *const _ as *const _,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
            }
        }
        
        (win_api.CloseHandle.unwrap())(token_handle);
    }
}

/// Compress file using CreateFileW with FILE_FLAG_BACKUP_SEMANTICS
fn compress_file_with_backup_semantics(path: &str, algo: WofAlgorithm, force: bool) -> Option<Result<bool, u32>> {
    unsafe {
        let wide = PathBuffer::from(path);
        
        // GENERIC_READ (0x80000000) | GENERIC_WRITE (0x40000000)
        let access = 0x80000000 | 0x40000000;
        
        let handle = (crate::engine::dynamic_import::WinApi::get().CreateFileW.unwrap())(
            wide.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS, // Key: bypass security with backup semantics
            std::ptr::null_mut(),
        );
        
        if handle != INVALID_HANDLE_VALUE {
            // Convert HANDLE to File for compress_file_handle
            let file = File::from_raw_handle(handle as *mut _);
            let result = compress_file_handle(&file, algo, force);
            // File will be dropped here, closing the handle
            Some(result)
        } else {
            None
        }
    }
}

pub fn compress_file_handle(file: &File, algo: WofAlgorithm, force: bool) -> Result<bool, u32> {
    let handle = file.as_raw_handle() as HANDLE;

    // 1. Prepare WOF_EXTERNAL_INFO
    let wof_info = WOF_EXTERNAL_INFO {
        version: WOF_CURRENT_VERSION,
        provider: WOF_PROVIDER_FILE,
    };

    // 2. Prepare FILE_PROVIDER_EXTERNAL_INFO_V1
    let file_info = FILE_PROVIDER_EXTERNAL_INFO_V1 {
        version: FILE_PROVIDER_CURRENT_VERSION,
        algorithm: algo.to_u32(),
        flags: 0,
    };

    // 3. Combine into a single buffer
    // Layout: [WOF_EXTERNAL_INFO] [FILE_PROVIDER_EXTERNAL_INFO_V1]
    let mut input_buffer = Vec::with_capacity(size_of::<WOF_EXTERNAL_INFO>() + size_of::<FILE_PROVIDER_EXTERNAL_INFO_V1>());
    
    // Safety: Simple POD structs
    unsafe {
        let wof_ptr = &wof_info as *const _ as *const u8;
        let wof_slice = std::slice::from_raw_parts(wof_ptr, size_of::<WOF_EXTERNAL_INFO>());
        input_buffer.extend_from_slice(wof_slice);

        let file_ptr = &file_info as *const _ as *const u8;
        let file_slice = std::slice::from_raw_parts(file_ptr, size_of::<FILE_PROVIDER_EXTERNAL_INFO_V1>());
        input_buffer.extend_from_slice(file_slice);
    }

    let mut bytes_returned = 0u32;
    
    unsafe {
        let win_api = crate::engine::dynamic_import::WinApi::get();
        let result = (win_api.DeviceIoControl.unwrap())(
            handle,
            FSCTL_SET_EXTERNAL_BACKING,
            input_buffer.as_ptr() as *const c_void,
            input_buffer.len() as u32,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        );

        if result == 0 {
             let err = GetLastError();
             // Handle specific errors that aren't fatal "failures" but just "Can't compress this"
             // ERROR_COMPRESSION_NOT_BENEFICIAL (344)
             if err == 344 { 
                 if force {
                     // Fallback to NTFS Compression (LZNT1)
                     let compression_state: u16 = COMPRESSION_FORMAT_DEFAULT;
                     let _ = (crate::engine::dynamic_import::WinApi::get().DeviceIoControl.unwrap())(
                        handle,
                        FSCTL_SET_COMPRESSION,
                        &compression_state as *const _ as *const c_void,
                        std::mem::size_of::<u16>() as u32,
                        std::ptr::null_mut(),
                        0,
                        &mut bytes_returned,
                        std::ptr::null_mut(),
                    );
                    // We assume if this succeeds/fails, we did our best.
                    return Ok(true);
                 }
                 return Ok(false);
             }
             return Err(err);
        }
    }

    Ok(true)
}


pub fn uncompress_file(path: &str) -> Result<(), u32> {
    // Requires Write permission for FSCTL_DELETE_EXTERNAL_BACKING
    // Use permissive sharing (Read|Write|Delete = 7) to allow processing locked files
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(7)
        .open(path)
        .map_err(|e| e.raw_os_error().unwrap_or(0) as u32)?;  
    uncompress_file_handle(&file)
}

pub fn uncompress_file_handle(file: &File) -> Result<(), u32> {
    let handle = file.as_raw_handle() as HANDLE;
    let mut bytes_returned = 0u32;

    unsafe {
        let win_api = crate::engine::dynamic_import::WinApi::get();
        if (win_api.DeviceIoControl.unwrap())(
            handle,
            FSCTL_DELETE_EXTERNAL_BACKING,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        ) == 0 {
            let err = GetLastError();
             // ERROR_INVALID_FUNCTION (1) or ERROR_NOT_SUPPORTED (50) might happen if not compressed
             if err == 346 { // ERROR_NOT_CAPABLE? Or some specific WOF error? 
                // Don't return yet, try NTFS decompression too!
            } else {
                 // We might want to just proceed to NTFS decompression anyway
            }
        }
        
        // ALSO try to remove NTFS compression (Blue files)
        // FSCTL_SET_COMPRESSION(COMPRESSION_FORMAT_NONE)
        let compression_state: u16 = COMPRESSION_FORMAT_NONE; 
        
        let _ = (win_api.DeviceIoControl.unwrap())(
            handle,
            FSCTL_SET_COMPRESSION,
            &compression_state as *const _ as *const c_void,
            std::mem::size_of::<u16>() as u32,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        );
    }
    Ok(())
}
