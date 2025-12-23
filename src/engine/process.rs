#![allow(non_snake_case, non_camel_case_types)]
use crate::utils::to_wstring;
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_MORE_DATA, FILETIME, HANDLE};

// --- Manual Bindings for RestartManager & Threading ---

const PROCESS_TERMINATE: u32 = 0x0001;
const CCH_RM_SESSION_KEY: u32 = 32;
// const RmRebootReasonNone: u32 = 0; // Not strictly needed if passed as 0/none

#[repr(C)]
#[derive(Clone, Copy)]
struct RM_UNIQUE_PROCESS {
    dwProcessId: u32,
    ProcessStartTime: FILETIME,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RM_PROCESS_INFO {
    Process: RM_UNIQUE_PROCESS,
    strAppName: [u16; 256],
    strServiceShortName: [u16; 64],
    ApplicationType: u32, // RM_APP_TYPE enum
    AppStatus: u32,
    TSSessionId: u32,
    bRestartable: i32,
}

#[link(name = "rstrtmgr")]
unsafe extern "system" {
    fn RmStartSession(pSessionHandle: *mut u32, dwSessionFlags: u32, strSessionKey: *mut u16) -> u32;
    fn RmRegisterResources(dwSessionHandle: u32, nFiles: u32, rgsFileNames: *const *const u16, nApplications: u32, rgApplications: *const std::ffi::c_void, nServices: u32, rgsServiceNames: *const *const u16) -> u32;
    fn RmGetList(dwSessionHandle: u32, pnProcInfoNeeded: *mut u32, pnProcInfo: *mut u32, rgAffectedApps: *mut RM_PROCESS_INFO, lpdwRebootReasons: *mut u32) -> u32;
    fn RmEndSession(dwSessionHandle: u32) -> u32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> HANDLE;
    fn TerminateProcess(hProcess: HANDLE, uExitCode: u32) -> i32;
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
}

/// Get a list of processes locking the specified file path
pub fn get_file_blockers(path: &str) -> Vec<ProcessInfo> {
    unsafe {
        let mut session_handle: u32 = 0;
        let mut session_key = [0u16; CCH_RM_SESSION_KEY as usize + 1]; // +1 for safety
        
        let res = RmStartSession(&mut session_handle, 0, session_key.as_mut_ptr());
        if res != 0 {
            return Vec::new();
        }

        let path_wide = to_wstring(path);
        let resources = [path_wide.as_ptr()];
        
        // Register connection to the file
        let res = RmRegisterResources(
             session_handle, 
             1, 
             resources.as_ptr(), 
             0, 
             std::ptr::null(), 
             0, 
             std::ptr::null()
        );
        
        if res != 0 {
            let _ = RmEndSession(session_handle);
            return Vec::new();
        }

        // Get list
        let mut n_proc_info_needed = 0u32;
        let mut n_proc_info = 0u32;
        let mut reason = 0u32; // RmRebootReasonNone
        
        // First call to get count
        let res = RmGetList(
            session_handle,
            &mut n_proc_info_needed,
            &mut n_proc_info,
            std::ptr::null_mut(),
            &mut reason
        );
        
        if res != ERROR_MORE_DATA && res != 0 {
             let _ = RmEndSession(session_handle);
             return Vec::new();
        }
        
        if n_proc_info_needed == 0 {
            let _ = RmEndSession(session_handle);
            return Vec::new();
        }

        // Allocate buffer
        let mut process_info = vec![std::mem::zeroed::<RM_PROCESS_INFO>(); n_proc_info_needed as usize];
        n_proc_info = n_proc_info_needed;
        
        let res = RmGetList(
            session_handle,
            &mut n_proc_info_needed,
            &mut n_proc_info,
            process_info.as_mut_ptr(),
            &mut reason
        );
        
        let _ = RmEndSession(session_handle);

        if res != 0 {
            return Vec::new();
        }

        let mut results = Vec::new();
        for i in 0..n_proc_info as usize {
            let p = &process_info[i];
            
            let pid = p.Process.dwProcessId;
            let name_arr = p.strAppName; // [u16; 256]
            let name_len = name_arr.iter().position(|&c| c == 0).unwrap_or(name_arr.len());
            let name = String::from_utf16_lossy(&name_arr[..name_len]);
            
            results.push(ProcessInfo { pid, name });
        }
        
        results
    }
}

/// Kill a process by PID
pub fn kill_process(pid: u32) -> Result<(), String> {
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        // Correctly handling null pointer check for handle
        if handle != std::ptr::null_mut() {
            let res = TerminateProcess(handle, 1);
            CloseHandle(handle);
            if res != 0 {
                Ok(())
            } else {
                Err("TerminateProcess failed".to_string())
            }
        } else {
            // Check if process basically doesn't exist?
            // GetLastError could tell access denied vs not found
            Ok(()) 
        }
    }
}
