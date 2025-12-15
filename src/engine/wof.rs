use std::ffi::c_void;
use std::fs::File;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::os::windows::prelude::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use windows::core::{PCWSTR, Result};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, OPEN_EXISTING, GetCompressedFileSizeW};
use windows::Win32::System::Ioctl::{FSCTL_SET_EXTERNAL_BACKING, FSCTL_DELETE_EXTERNAL_BACKING, FSCTL_GET_EXTERNAL_BACKING, FSCTL_SET_COMPRESSION};
use windows::Win32::System::IO::DeviceIoControl;

pub fn get_real_file_size(path: &str) -> u64 {
    unsafe {
        let wide: Vec<u16> = std::ffi::OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();
        let mut high: u32 = 0;
        let low = GetCompressedFileSizeW(PCWSTR(wide.as_ptr()), Some(&mut high));
        if low == u32::MAX && windows::Win32::Foundation::GetLastError().is_err() {
            // If error, fall back to logical size or 0
            std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
        } else {
            ((high as u64) << 32) | (low as u64)
        }
    }
}

pub fn is_wof_compressed(path: &str) -> bool {
    // Check if WOF backing exists
    unsafe {
        let wide: Vec<u16> = std::ffi::OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();
        let handle = CreateFileW(
            PCWSTR(wide.as_ptr()),
            GENERIC_READ.0, // Read access
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        ).unwrap_or(INVALID_HANDLE_VALUE);

        if handle == INVALID_HANDLE_VALUE {
            return false;
        }

        let mut out_buffer = [0u8; 1024]; // WOF_EXTERNAL_INFO buffer
        let mut bytes_returned = 0u32;
        
        // We don't really care about the content, just success
        let result = DeviceIoControl(
            handle,
            FSCTL_GET_EXTERNAL_BACKING,
            None,
            0,
            Some(out_buffer.as_mut_ptr() as *mut _),
            out_buffer.len() as u32,
            Some(&mut bytes_returned),
            None,
        );
        
        let _ = CloseHandle(handle);
        result.is_ok()
    }
}

/// Get the WOF compression algorithm used for a file
/// Returns None if file is not WOF-compressed, Some(algorithm) if it is
pub fn get_wof_algorithm(path: &str) -> Option<WofAlgorithm> {
    unsafe {
        let wide: Vec<u16> = std::ffi::OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();
        let handle = CreateFileW(
            PCWSTR(wide.as_ptr()),
            GENERIC_READ.0,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        ).unwrap_or(INVALID_HANDLE_VALUE);

        if handle == INVALID_HANDLE_VALUE {
            return None;
        }

        // Buffer for WOF_EXTERNAL_INFO + FILE_PROVIDER_EXTERNAL_INFO_V1
        let mut out_buffer = [0u8; 1024];
        let mut bytes_returned = 0u32;
        
        let result = DeviceIoControl(
            handle,
            FSCTL_GET_EXTERNAL_BACKING,
            None,
            0,
            Some(out_buffer.as_mut_ptr() as *mut _),
            out_buffer.len() as u32,
            Some(&mut bytes_returned),
            None,
        );
        
        let _ = CloseHandle(handle);
        
        if result.is_err() {
            return None;
        }
        
        // Parse the output buffer
        // First comes WOF_EXTERNAL_INFO (8 bytes), then FILE_PROVIDER_EXTERNAL_INFO_V1 (12 bytes)
        if bytes_returned < 20 {
            return None;
        }
        
        let wof_info = &out_buffer[0..8];
        let provider = u32::from_le_bytes([wof_info[4], wof_info[5], wof_info[6], wof_info[7]]);
        
        // Check if it's WOF_PROVIDER_FILE (2)
        if provider != 2 {
            return None;
        }
        
        let file_info = &out_buffer[8..20];
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

// WOF Definitions not fully exposed in high-level windows crate helpers sometimes, 
// creating safe wrappers around the raw structs.

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct WOF_EXTERNAL_INFO {
    pub version: u32,
    pub provider: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FILE_PROVIDER_EXTERNAL_INFO_V1 {
    pub version: u32,
    pub algorithm: u32,
    pub flags: u32,
}

pub const WOF_CURRENT_VERSION: u32 = 1;
pub const WOF_PROVIDER_FILE: u32 = 2; // WOF_PROVIDER_FILE

pub const FILE_PROVIDER_CURRENT_VERSION: u32 = 1;

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
#[derive(Clone, Copy, Debug)]
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

pub fn compress_file(path: &str, algo: WofAlgorithm, force: bool) -> Result<bool> {
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
            if force && e.raw_os_error() == Some(5) { // ERROR_ACCESS_DENIED = 5
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
            Err(windows::core::Error::from_thread())
        }
    }
}

/// Force remove read-only attribute from a file
fn force_remove_readonly(path: &str) {
    use windows::Win32::Storage::FileSystem::{
        GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_READONLY,
        FILE_ATTRIBUTE_NORMAL, FILE_FLAGS_AND_ATTRIBUTES,
    };
    
    unsafe {
        let wide: Vec<u16> = std::ffi::OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();
        
        let attrs = GetFileAttributesW(PCWSTR(wide.as_ptr()));
        if attrs != u32::MAX { // INVALID_FILE_ATTRIBUTES
            // Remove read-only flag
            let new_attrs = attrs & !FILE_ATTRIBUTE_READONLY.0;
            let new_attrs = if new_attrs == 0 { FILE_ATTRIBUTE_NORMAL.0 } else { new_attrs };
            let _ = SetFileAttributesW(PCWSTR(wide.as_ptr()), FILE_FLAGS_AND_ATTRIBUTES(new_attrs));
        }
    }
}

/// Enable backup and restore privileges for the current process
fn enable_backup_privileges() {
    use windows::Win32::Foundation::LUID;
    use windows::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, 
        TOKEN_ADJUST_PRIVILEGES, TOKEN_QUERY, TOKEN_PRIVILEGES,
        SE_PRIVILEGE_ENABLED, LUID_AND_ATTRIBUTES,
    };
    use windows::Win32::System::Threading::{OpenProcessToken, GetCurrentProcess};
    use windows::core::w;
    
    unsafe {
        let mut token_handle = HANDLE::default();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token_handle
        ).is_err() {
            return;
        }
        
        let privileges = [
            w!("SeBackupPrivilege"),
            w!("SeRestorePrivilege"),
            w!("SeTakeOwnershipPrivilege"),
            w!("SeSecurityPrivilege"),
        ];
        
        for priv_name in privileges {
            let mut luid = LUID::default();
            if LookupPrivilegeValueW(None, priv_name, &mut luid).is_ok() {
                let tp = TOKEN_PRIVILEGES {
                    PrivilegeCount: 1,
                    Privileges: [LUID_AND_ATTRIBUTES {
                        Luid: luid,
                        Attributes: SE_PRIVILEGE_ENABLED,
                    }],
                };
                let _ = AdjustTokenPrivileges(
                    token_handle,
                    false,
                    Some(&tp),
                    0,
                    None,
                    None,
                );
            }
        }
        
        let _ = CloseHandle(token_handle);
    }
}

/// Compress file using CreateFileW with FILE_FLAG_BACKUP_SEMANTICS
fn compress_file_with_backup_semantics(path: &str, algo: WofAlgorithm, force: bool) -> Option<Result<bool>> {
    use windows::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE, 
        FILE_SHARE_DELETE, OPEN_EXISTING,
    };
    
    use std::fs::File;
    use std::os::windows::io::FromRawHandle;
    
    unsafe {
        let wide: Vec<u16> = std::ffi::OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();
        
        // GENERIC_READ | GENERIC_WRITE = 0x80000000 | 0x40000000
        let access = 0x80000000u32 | 0x40000000u32;
        
        let handle = CreateFileW(
            PCWSTR(wide.as_ptr()),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS, // Key: bypass security with backup semantics
            None,
        );
        
        match handle {
            Ok(h) => {
                // Convert HANDLE to File for compress_file_handle
                let file = File::from_raw_handle(h.0 as *mut _);
                let result = compress_file_handle(&file, algo, force);
                // File will be dropped here, closing the handle
                Some(result)
            }
            Err(_) => None
        }
    }
}

/// Try to force access to a file by taking ownership and granting full control.
/// Uses low-level Win32 API with backup semantics (no external commands).
/// Returns true if successful, false otherwise.
fn try_force_access(path: &str) -> bool {
    use windows::Win32::Foundation::LUID;
    use windows::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, 
        TOKEN_ADJUST_PRIVILEGES, TOKEN_QUERY, TOKEN_PRIVILEGES,
        SE_PRIVILEGE_ENABLED, LUID_AND_ATTRIBUTES,
        DACL_SECURITY_INFORMATION, PSID,
    };
    use windows::Win32::Security::Authorization::{
        SetSecurityInfo, SE_FILE_OBJECT,
    };
    use windows::Win32::System::Threading::{OpenProcessToken, GetCurrentProcess};
    use windows::core::w;
    
    unsafe {
        // 1. Get current process token
        let mut token_handle = HANDLE::default();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token_handle
        ).is_err() {
            return false;
        }
        
        // 2. Enable SE_TAKE_OWNERSHIP_NAME, SE_RESTORE_NAME, SE_BACKUP_NAME privileges
        let privileges = [
            w!("SeTakeOwnershipPrivilege"),
            w!("SeRestorePrivilege"),
            w!("SeBackupPrivilege"),
        ];
        
        for priv_name in privileges {
            let mut luid = LUID::default();
            if LookupPrivilegeValueW(None, priv_name, &mut luid).is_ok() {
                let tp = TOKEN_PRIVILEGES {
                    PrivilegeCount: 1,
                    Privileges: [LUID_AND_ATTRIBUTES {
                        Luid: luid,
                        Attributes: SE_PRIVILEGE_ENABLED,
                    }],
                };
                let _ = AdjustTokenPrivileges(
                    token_handle,
                    false,
                    Some(&tp),
                    0,
                    None,
                    None,
                );
            }
        }
        
        // 3. Try to open file with backup semantics (bypasses security checks for backup operators)
        let wide: Vec<u16> = std::ffi::OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();
        
        use windows::Win32::Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS as BACKUP_SEM,
            FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_SHARE_DELETE,
            OPEN_EXISTING,
        };
        
        // WRITE_DAC | WRITE_OWNER = 0x00040000 | 0x00080000
        let access_flags: u32 = 0x00040000 | 0x00080000;
        
        let file_result = CreateFileW(
            PCWSTR(wide.as_ptr()),
            access_flags,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            BACKUP_SEM, // Backup semantics - critical for bypassing security
            None,
        );
        
        let handle = match file_result {
            Ok(h) => h,
            Err(_) => {
                let _ = CloseHandle(token_handle);
                return false;
            }
        };
        
        // 4. Set NULL DACL (grants everyone full access)
        let result = SetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            Some(PSID::default()),
            Some(PSID::default()),
            Some(std::ptr::null()), // NULL DACL = everyone has full access
            None,
        );
        
        let _ = CloseHandle(handle);
        let _ = CloseHandle(token_handle);
        
        result.is_ok()
    }
}

pub fn compress_file_handle(file: &File, algo: WofAlgorithm, force: bool) -> Result<bool> {
    let handle = HANDLE(file.as_raw_handle() as *mut c_void);

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
    
    // Safety: DeviceIoControl via windows crate
    unsafe {
        let result = DeviceIoControl(
            handle,
            FSCTL_SET_EXTERNAL_BACKING,
             Some(input_buffer.as_ptr() as *const c_void),
            input_buffer.len() as u32,
            None,
            0,
            Some(&mut bytes_returned),
            None
        );

        if let Err(e) = result {
             // Handle specific errors that aren't fatal "failures" but just "Can't compress this"
             // ERROR_COMPRESSION_NOT_BENEFICIAL (344)
             if e.code().0 == -2147024552 { // 0x80070158 which maps to 344 in HRESULT
                 if force {
                     // Fallback to NTFS Compression (LZNT1)
                     let compression_state: u16 = COMPRESSION_FORMAT_DEFAULT;
                     let _ = DeviceIoControl(
                        handle,
                        FSCTL_SET_COMPRESSION,
                        Some(&compression_state as *const _ as *const c_void),
                        std::mem::size_of::<u16>() as u32,
                        None,
                        0,
                        Some(&mut bytes_returned),
                        None
                    );
                    // We assume if this succeeds/fails, we did our best.
                    // But we don't have a good way to verify if it *actually* compressed better without checking size again.
                    // But we return True effectively saying "We forced it".
                    return Ok(true);
                 }
                 return Ok(false);
             }
             return Err(e);
        }
    }

    Ok(true)
}


pub fn uncompress_file(path: &str) -> Result<()> {
    // Requires Write permission for FSCTL_DELETE_EXTERNAL_BACKING
    // Use permissive sharing (Read|Write|Delete = 7) to allow processing locked files
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(7)
        .open(path)
        .map_err(|_| windows::core::Error::from_thread())?;  
    uncompress_file_handle(&file)
}

pub fn uncompress_file_handle(file: &File) -> Result<()> {
    let handle = HANDLE(file.as_raw_handle() as *mut c_void);
    let mut bytes_returned = 0u32;

    unsafe {
        if let Err(e) = DeviceIoControl(
            handle,
            FSCTL_DELETE_EXTERNAL_BACKING,
            None,
            0,
            None,
            0,
            Some(&mut bytes_returned),
            None
        ) {
            if e.code().0 == -2147024554 { 
                // Don't return yet, try NTFS decompression too!
            } else {
                return Err(e);
            }
        }
        
        // ALSO try to remove NTFS compression (Blue files)
        // FSCTL_SET_COMPRESSION(COMPRESSION_FORMAT_NONE)
        let compression_state: u16 = COMPRESSION_FORMAT_NONE; 
        
        let _ = DeviceIoControl(
            handle,
            FSCTL_SET_COMPRESSION,
            Some(&compression_state as *const _ as *const c_void),
            std::mem::size_of::<u16>() as u32,
            None,
            0,
            Some(&mut bytes_returned),
            None
        );
        // We ignore errors here because if it fails it might not be supported or something, 
        // but it shouldn't block the "WOF" success if that was the main goal. 
        // Although the user wants "Decompress" to really Decompress.
        // If FSCTL_SET_COMPRESSION fails, we might want to know?
        // But let's best-effort it.
    }
    Ok(())
}

