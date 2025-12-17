use crate::utils::to_wstring;
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_MORE_DATA};
use windows_sys::Win32::System::RestartManager::{
    RmStartSession, RmRegisterResources, RmGetList, RmEndSession,
    CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmRebootReasonNone,
};
use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
}

/// Get a list of processes locking the specified file path
pub fn get_file_blockers(path: &str) -> Vec<ProcessInfo> {
    unsafe {
        let mut session_handle: u32 = 0;
        let mut session_key = [0u16; CCH_RM_SESSION_KEY as usize];
        
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
        let mut reason = RmRebootReasonNone;
        
        // First call to get count
        let res = RmGetList(
            session_handle,
            &mut n_proc_info_needed,
            &mut n_proc_info,
            std::ptr::null_mut(),
            &mut reason // windows-sys defines this as *mut u32 sometimes or enum? 
               as *mut _ as *mut u32
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
            &mut reason as *mut _ as *mut u32
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
