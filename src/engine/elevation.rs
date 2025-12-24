#![allow(unsafe_op_in_unsafe_fn, non_snake_case, non_camel_case_types, non_upper_case_globals)]
use crate::utils::to_wstring;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use crate::types::*;

#[repr(C)]
struct SERVICE_STATUS_PROCESS {
    dwServiceType: u32,
    dwCurrentState: u32,
    dwControlsAccepted: u32,
    dwWin32ExitCode: u32,
    dwServiceSpecificExitCode: u32,
    dwCheckPoint: u32,
    dwWaitHint: u32,
    dwProcessId: u32,
    dwServiceFlags: u32,
}

const SE_PRIVILEGE_ENABLED: u32 = 0x00000002;
const TOKEN_ADJUST_PRIVILEGES: u32 = 0x0020;
const TOKEN_QUERY: u32 = 0x0008;

const SC_MANAGER_CONNECT: u32 = 0x0001;
const SERVICE_QUERY_STATUS: u32 = 0x0004;
const SERVICE_START: u32 = 0x0010;
const SC_STATUS_PROCESS_INFO: u32 = 0;
const SERVICE_RUNNING: u32 = 0x00000004;

const STARTF_USESHOWWINDOW: u32 = 0x00000001;
const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x00080000;
const PROCESS_CREATE_PROCESS: u32 = 0x0080;
const PROC_THREAD_ATTRIBUTE_PARENT_PROCESS: usize = 0x00020000;

const ERROR_NOT_ALL_ASSIGNED: u32 = 1300;

unsafe fn enable_privilege(privilege_name: &str) -> bool {
    let win_api = crate::engine::dynamic_import::WinApi::get();
    let mut token: HANDLE = std::ptr::null_mut();
    
    // Using -1 as pseudo handle for current process
    let current_process = -1isize as HANDLE; 

    if (win_api.OpenProcessToken.unwrap())(current_process, TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
        return false;
    }
    
    let mut luid: LUID = zeroed();
    let name = to_wstring(privilege_name);
    // LookupPrivilegeValueW takes LPCWSTR
    if (win_api.LookupPrivilegeValueW.unwrap())(null(), name.as_ptr(), &mut luid as *mut _ as *mut c_void) == 0 {
        (win_api.CloseHandle.unwrap())(token);
        return false;
    }
    
    let tp = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }],
    };

    let res = (win_api.AdjustTokenPrivileges.unwrap())(token, FALSE, &tp as *const _ as *const c_void, size_of::<TOKEN_PRIVILEGES>() as u32, null_mut(), null_mut());
    let err = crate::types::GetLastError();
    (win_api.CloseHandle.unwrap())(token);
    
    if res == 0 || err == ERROR_NOT_ALL_ASSIGNED {
        return false;
    }
    true
}

pub unsafe fn enable_debug_privilege() -> bool {
    enable_privilege("SeDebugPrivilege")
}

unsafe fn get_trusted_installer_pid() -> Option<u32> {
    let win_api = crate::engine::dynamic_import::WinApi::get();
    let scm = (win_api.OpenSCManagerW.unwrap())(null(), null(), SC_MANAGER_CONNECT);
    if scm.is_null() { return None; }
    
    let ti_name = to_wstring("TrustedInstaller");
    let service = (win_api.OpenServiceW.unwrap())(scm, ti_name.as_ptr(), SERVICE_START | SERVICE_QUERY_STATUS);
    
    if service.is_null() { (win_api.CloseServiceHandle.unwrap())(scm); return None; }
    
    (win_api.StartServiceW.unwrap())(service, 0, null());
    
    let mut pid = None;
    for _ in 0..20 {
        let mut bytes_needed = 0;
        let mut ssp: SERVICE_STATUS_PROCESS = zeroed();
        let res = (win_api.QueryServiceStatusEx.unwrap())(service, SC_STATUS_PROCESS_INFO, &mut ssp as *mut _ as *mut u8, size_of::<SERVICE_STATUS_PROCESS>() as u32, &mut bytes_needed);
        if res != 0 && ssp.dwCurrentState == SERVICE_RUNNING && ssp.dwProcessId != 0 {
            pid = Some(ssp.dwProcessId);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    
    (win_api.CloseServiceHandle.unwrap())(service);
    (win_api.CloseServiceHandle.unwrap())(scm);
    pid
}

pub fn restart_as_trusted_installer() -> Result<(), String> {
    unsafe {
        let win_api = crate::engine::dynamic_import::WinApi::get();
        if !enable_debug_privilege() { 
            return Err("Failed to enable SeDebugPrivilege".to_string());
        }
        
        let pid = get_trusted_installer_pid().ok_or("Failed to start TrustedInstaller service".to_string())?;
        
        let process = (win_api.OpenProcess.unwrap())(PROCESS_CREATE_PROCESS, FALSE, pid);
        if process.is_null() { return Err("Failed to open TrustedInstaller process (PID: ".to_string() + &pid.to_string() + ")"); }
        
        let mut size: usize = 0;
        let _ = (win_api.InitializeProcThreadAttributeList.unwrap())(std::ptr::null_mut(), 1, 0, &mut size);
        
        let mut buffer = vec![0u8; size];
        let lp_attribute_list = buffer.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
        
        if (win_api.InitializeProcThreadAttributeList.unwrap())(lp_attribute_list, 1, 0, &mut size) == 0 {
            (win_api.CloseHandle.unwrap())(process);
            return Err("InitializeProcThreadAttributeList failed".to_string());
        }
        
        let mut parent_handle = process;
        if (win_api.UpdateProcThreadAttribute.unwrap())(
                lp_attribute_list, 
                0, 
                PROC_THREAD_ATTRIBUTE_PARENT_PROCESS as usize, 
                &mut parent_handle as *mut _ as *mut c_void, 
                size_of::<HANDLE>(), 
                null_mut(), 
                null_mut()
            ) == 0 {
            (win_api.DeleteProcThreadAttributeList.unwrap())(lp_attribute_list);
            (win_api.CloseHandle.unwrap())(process);
            return Err("UpdateProcThreadAttribute failed".to_string());
        }

        let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
        let cmd_line = crate::utils::concat_wstrings(&[crate::w!("\""), &crate::utils::to_wstring(&exe_path.to_string_lossy()), crate::w!("\"")]);
        
        let mut si_ex: STARTUPINFOEXW = zeroed();
        si_ex.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        si_ex.StartupInfo.dwFlags = STARTF_USESHOWWINDOW;
        si_ex.StartupInfo.wShowWindow = SW_SHOW as u16;
        si_ex.lpAttributeList = lp_attribute_list;

        let mut pi: PROCESS_INFORMATION = zeroed();
        
        let create_res = (win_api.CreateProcessW.unwrap())(
            null(), 
            cmd_line.as_ptr() as *mut u16, 
            null(), 
            null(), 
            FALSE, 
            EXTENDED_STARTUPINFO_PRESENT, 
            null(), 
            null(), 
            &mut si_ex.StartupInfo as *mut _ as *mut c_void, 
            &mut pi as *mut _ as *mut c_void
        );
        
        (win_api.DeleteProcThreadAttributeList.unwrap())(lp_attribute_list);
        (win_api.CloseHandle.unwrap())(process);
        
        if create_res == 0 {
            return Err("CreateProcessW (Spoof Parent) failed".to_string());
        }
        (win_api.CloseHandle.unwrap())(pi.hProcess);
        (win_api.CloseHandle.unwrap())(pi.hThread);
        std::process::exit(0);
    }
}

pub fn is_system_or_ti() -> bool {
    unsafe {
        let mut buffer = [0u16; 256];
        let mut size = 256;
        let win_api = crate::engine::dynamic_import::WinApi::get();
        if (win_api.GetUserNameW.unwrap())(buffer.as_mut_ptr(), &mut size) != 0 {
            let name = String::from_utf16_lossy(&buffer[..size as usize - 1]);
            return name.to_uppercase().contains("SYSTEM");
        }
    }
    false
}
