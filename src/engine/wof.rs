use std::ffi::c_void;
use std::fs::File;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::os::windows::prelude::OsStrExt;
use std::path::Path;
use windows::core::{PCWSTR, Result};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, OPEN_EXISTING, GetCompressedFileSizeW};
use windows::Win32::System::Ioctl::{FSCTL_SET_EXTERNAL_BACKING, FSCTL_DELETE_EXTERNAL_BACKING, FSCTL_GET_EXTERNAL_BACKING};
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

pub fn compress_file(path: &str, algo: WofAlgorithm) -> Result<bool> {
    let file = File::open(path).map_err(|_| windows::core::Error::from_win32())?; // Map io error to windows error? Or just skip?
    compress_file_handle(&file, algo)
}

pub fn compress_file_handle(file: &File, algo: WofAlgorithm) -> Result<bool> {
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
                 return Ok(false);
             }
             return Err(e);
        }
    }

    Ok(true)
}


pub fn uncompress_file(path: &str) -> Result<()> {
    let file = File::open(path).map_err(|_| windows::core::Error::from_win32())?; 
    uncompress_file_handle(&file)
}

pub fn uncompress_file_handle(file: &File) -> Result<()> {
    let handle = HANDLE(file.as_raw_handle() as *mut c_void);
    let mut bytes_returned = 0u32;

    unsafe {
        DeviceIoControl(
            handle,
            FSCTL_DELETE_EXTERNAL_BACKING,
            None,
            0,
            None,
            0,
            Some(&mut bytes_returned),
            None
        )?;
    }
    Ok(())
}

