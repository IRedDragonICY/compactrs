use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle};
use windows::Win32::System::RestartManager::{
    RmStartSession, RmRegisterResources, RmGetList, RmEndSession,
    CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmRebootReasonNone,
};
use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
use crate::gui::utils::ToWide;

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
        
        let res = RmStartSession(&mut session_handle, Some(0), PWSTR(session_key.as_mut_ptr()));
        if res.0 != 0 {
            return Vec::new();
        }

        let path_wide = path.to_wide();
        let resources = [PCWSTR(path_wide.as_ptr())];
        
        // Register connection to the file
        let res = RmRegisterResources(session_handle, Some(&resources), None, None);
        if res.is_err() {
            let _ = RmEndSession(session_handle);
            return Vec::new();
        }

        // Get list
        let mut n_proc_info_needed = 0u32;
        let mut n_proc_info = 0u32;
        let mut reason = RmRebootReasonNone;
        
        // First call to get count
        let _ = RmGetList(session_handle, &mut n_proc_info_needed, &mut n_proc_info, None, &mut reason as *mut _ as *mut u32);
        
        if n_proc_info_needed == 0 {
            let _ = RmEndSession(session_handle);
            return Vec::new();
        }

        let mut process_info = vec![RM_PROCESS_INFO::default(); n_proc_info_needed as usize];
        n_proc_info = n_proc_info_needed;
        
        let res = RmGetList(session_handle, &mut n_proc_info_needed, &mut n_proc_info, Some(process_info.as_mut_ptr()), &mut reason as *mut _ as *mut u32);
        
        let _ = RmEndSession(session_handle);

        if res.is_err() {
            return Vec::new();
        }

        let mut results = Vec::new();
        for i in 0..n_proc_info as usize {
            let p = &process_info[i];
            // ApplicationType 0 = Unknown, 1 = MainWindow, 2 = OtherWindow, 3 = Service, 4 = Explorer
            // We usually care about all of them
            
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
pub fn kill_process(pid: u32) -> Result<(), windows::core::Error> {
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, false, pid)?;
        if !handle.is_invalid() {
            let res = TerminateProcess(handle, 1);
            let _ = CloseHandle(handle);
            res
        } else {
            Ok(()) // Already gone?
        }
    }
}
