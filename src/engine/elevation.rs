#![allow(unsafe_op_in_unsafe_fn)]
use crate::utils::to_wstring;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{CloseHandle, FALSE, HANDLE, LUID, GetLastError, ERROR_NOT_ALL_ASSIGNED};

use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES,
    SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_QUERY, TOKEN_PRIVILEGES,
};
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, StartServiceW,
    SC_MANAGER_CONNECT, SC_STATUS_PROCESS_INFO, SERVICE_QUERY_STATUS, SERVICE_START,
    SERVICE_STATUS_PROCESS, SERVICE_RUNNING,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken,
    STARTF_USESHOWWINDOW,
    PROCESS_INFORMATION, CreateProcessW, EXTENDED_STARTUPINFO_PRESENT, 
    PROCESS_CREATE_PROCESS, InitializeProcThreadAttributeList, UpdateProcThreadAttribute, 
    DeleteProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST, PROC_THREAD_ATTRIBUTE_PARENT_PROCESS,
    STARTUPINFOEXW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW;

unsafe fn enable_privilege(privilege_name: &str) -> bool {
    let mut token: HANDLE = std::ptr::null_mut();
    if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
        eprintln!("OpenProcessToken failed for enable_privilege: {}", GetLastError());
        return false;
    }
    let mut luid: LUID = zeroed();
    let name = to_wstring(privilege_name);
    if LookupPrivilegeValueW(null(), name.as_ptr(), &mut luid) == 0 {
        eprintln!("LookupPrivilegeValueW failed for {}: {}", privilege_name, GetLastError());
        CloseHandle(token);
        return false;
    }
    let tp = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }],
    };

    let res = AdjustTokenPrivileges(token, FALSE, &tp, size_of::<TOKEN_PRIVILEGES>() as u32, null_mut(), null_mut());
    let err = GetLastError();
    CloseHandle(token);
    
    if res == 0 || err == ERROR_NOT_ALL_ASSIGNED {
        eprintln!("AdjustTokenPrivileges failed for {}: {}", privilege_name, err);
        return false;
    }
    true
}

pub unsafe fn enable_debug_privilege() -> bool {
    enable_privilege("SeDebugPrivilege")
}





unsafe fn get_trusted_installer_pid() -> Option<u32> {
    let scm = OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT);
    if scm.is_null() { return None; }
    let ti_name = to_wstring("TrustedInstaller");
    let service = OpenServiceW(scm, ti_name.as_ptr(), SERVICE_START | SERVICE_QUERY_STATUS);
    if service.is_null() { CloseServiceHandle(scm); return None; }
    StartServiceW(service, 0, null());
    
    let mut pid = None;
    for _ in 0..20 {
        let mut bytes_needed = 0;
        let mut ssp: SERVICE_STATUS_PROCESS = zeroed();
        let res = QueryServiceStatusEx(service, SC_STATUS_PROCESS_INFO, &mut ssp as *mut _ as *mut u8, size_of::<SERVICE_STATUS_PROCESS>() as u32, &mut bytes_needed);
        if res != 0 && ssp.dwCurrentState == SERVICE_RUNNING && ssp.dwProcessId != 0 {
            pid = Some(ssp.dwProcessId);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    CloseServiceHandle(service);
    CloseServiceHandle(scm);
    pid
}

pub fn restart_as_trusted_installer() -> Result<(), String> {
    unsafe {
        if !enable_debug_privilege() { 
            return Err(format!("Failed to enable SeDebugPrivilege (Error: {})", GetLastError())); 
        }
        
        let pid = get_trusted_installer_pid().ok_or(format!("Failed to start TrustedInstaller service (Error: {})", GetLastError()))?;
        
        // We need PROCESS_CREATE_PROCESS for parent spoofing
        let process = OpenProcess(PROCESS_CREATE_PROCESS, FALSE, pid);
        if process.is_null() { return Err(format!("Failed to open TrustedInstaller process (PID: {}) (Error: {})", pid, GetLastError())); }
        
        let mut size: usize = 0;
        let _ = InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size);
        
        let mut buffer = vec![0u8; size];
        let lp_attribute_list = buffer.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
        
        if InitializeProcThreadAttributeList(lp_attribute_list, 1, 0, &mut size) == 0 {
            CloseHandle(process);
            return Err(format!("InitializeProcThreadAttributeList failed (Error: {})", GetLastError()));
        }
        
        // PROC_THREAD_ATTRIBUTE_PARENT_PROCESS is a pointer to the parent process HANDLE
        let mut parent_handle = process;
        if UpdateProcThreadAttribute(
                lp_attribute_list, 
                0, 
                PROC_THREAD_ATTRIBUTE_PARENT_PROCESS as usize, 
                &mut parent_handle as *mut _ as *mut _, 
                size_of::<HANDLE>(), 
                null_mut(), 
                null()
            ) == 0 {
            DeleteProcThreadAttributeList(lp_attribute_list);
            CloseHandle(process);
            return Err(format!("UpdateProcThreadAttribute failed (Error: {})", GetLastError()));
        }

        let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
        let cmd_line = to_wstring(&format!("\"{}\"", exe_path.to_string_lossy()));
        
        let mut si_ex: STARTUPINFOEXW = zeroed();
        si_ex.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        si_ex.StartupInfo.dwFlags = STARTF_USESHOWWINDOW;
        si_ex.StartupInfo.wShowWindow = SW_SHOW as u16;
        si_ex.lpAttributeList = lp_attribute_list;

        let mut pi: PROCESS_INFORMATION = zeroed();
        
        let create_res = CreateProcessW(
            null(), 
            cmd_line.as_ptr() as *mut u16, 
            null(), 
            null(), 
            FALSE, 
            EXTENDED_STARTUPINFO_PRESENT, 
            null(), 
            null(), 
            &mut si_ex.StartupInfo, 
            &mut pi
        );
        
        DeleteProcThreadAttributeList(lp_attribute_list);
        CloseHandle(process);
        
        if create_res == 0 {
            use windows_sys::Win32::Foundation::GetLastError;
            return Err(format!("CreateProcessW (Spoof Parent) failed code: {}", GetLastError()));
        }
        CloseHandle(pi.hProcess);
        CloseHandle(pi.hThread);
        std::process::exit(0);
    }
}

pub fn is_system_or_ti() -> bool {
    unsafe {
        use windows_sys::Win32::System::WindowsProgramming::GetUserNameW;
        let mut buffer = [0u16; 256];
        let mut size = 256;
        if GetUserNameW(buffer.as_mut_ptr(), &mut size) != 0 {
            let name = String::from_utf16_lossy(&buffer[..size as usize - 1]);
            return name.to_uppercase().contains("SYSTEM");
        }
    }
    false
}
