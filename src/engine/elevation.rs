#![allow(unsafe_op_in_unsafe_fn, non_snake_case, non_camel_case_types, non_upper_case_globals)]
use crate::utils::to_wstring;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use crate::types::*;

// Imported from crate::types::*;

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
    let open_process_token = match win_api.OpenProcessToken { Some(f) => f, None => return false };
    let lookup_privilege_value = match win_api.LookupPrivilegeValueW { Some(f) => f, None => return false };
    let adjust_token_privileges = match win_api.AdjustTokenPrivileges { Some(f) => f, None => return false };
    let close_handle = match win_api.CloseHandle { Some(f) => f, None => return false };

    let mut token: HANDLE = std::ptr::null_mut();
    
    // Using -1 as pseudo handle for current process
    let current_process = -1isize as HANDLE; 

    if open_process_token(current_process, TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
        return false;
    }
    
    let mut luid: LUID = zeroed();
    let name = to_wstring(privilege_name);
    // LookupPrivilegeValueW takes LPCWSTR
    if lookup_privilege_value(null(), name.as_ptr(), &mut luid as *mut _ as *mut c_void) == 0 {
        close_handle(token);
        return false;
    }
    
    let tp = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }],
    };

    let res = adjust_token_privileges(token, FALSE, &tp as *const _ as *const c_void, size_of::<TOKEN_PRIVILEGES>() as u32, null_mut(), null_mut());
    let err = crate::types::GetLastError();
    close_handle(token);
    
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
    let open_sc_manager = win_api.OpenSCManagerW?;
    let open_service = win_api.OpenServiceW?;
    let start_service = win_api.StartServiceW?;
    let close_service = win_api.CloseServiceHandle?;
    let query_service = win_api.QueryServiceStatusEx?;

    let scm = open_sc_manager(null(), null(), SC_MANAGER_CONNECT);
    if scm.is_null() { return None; }
    
    let ti_name = to_wstring("TrustedInstaller");
    let service = open_service(scm, ti_name.as_ptr(), SERVICE_START | SERVICE_QUERY_STATUS);
    
    if service.is_null() { close_service(scm); return None; }
    
    start_service(service, 0, null());
    
    let mut pid = None;
    for _ in 0..20 {
        let mut bytes_needed = 0;
        let mut ssp: SERVICE_STATUS_PROCESS = zeroed();
        let res = query_service(service, SC_STATUS_PROCESS_INFO, &mut ssp as *mut _ as *mut u8, size_of::<SERVICE_STATUS_PROCESS>() as u32, &mut bytes_needed);
        if res != 0 && ssp.dwCurrentState == SERVICE_RUNNING && ssp.dwProcessId != 0 {
            pid = Some(ssp.dwProcessId);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    
    close_service(service);
    close_service(scm);
    pid
}

pub fn restart_as_trusted_installer() -> Result<(), String> {
    unsafe {
        let win_api = crate::engine::dynamic_import::WinApi::get();
        
        // Safe check for required imports first
        let open_process = win_api.OpenProcess.ok_or("Missing Import: OpenProcess")?;
        let init_proc_attr = win_api.InitializeProcThreadAttributeList.ok_or("Missing Import: InitializeProcThreadAttributeList")?;
        let update_proc_attr = win_api.UpdateProcThreadAttribute.ok_or("Missing Import: UpdateProcThreadAttribute")?;
        let delete_proc_attr = win_api.DeleteProcThreadAttributeList.ok_or("Missing Import: DeleteProcThreadAttributeList")?;
        let create_process = win_api.CreateProcessW.ok_or("Missing Import: CreateProcessW")?;
        let close_handle = win_api.CloseHandle.ok_or("Missing Import: CloseHandle")?;

        if !enable_debug_privilege() { 
            return Err("Failed to enable SeDebugPrivilege".to_string());
        }
        
        let pid = get_trusted_installer_pid().ok_or("Failed to start TrustedInstaller service".to_string())?;
        
        let process = open_process(PROCESS_CREATE_PROCESS, FALSE, pid);
        if process.is_null() { return Err("Failed to open TrustedInstaller process (PID: ".to_string() + &pid.to_string() + ")"); }
        
        let mut size: usize = 0;
        let _ = init_proc_attr(std::ptr::null_mut(), 1, 0, &mut size);
        
        let mut buffer = vec![0u8; size];
        let lp_attribute_list = buffer.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
        
        if init_proc_attr(lp_attribute_list, 1, 0, &mut size) == 0 {
            close_handle(process);
            return Err("InitializeProcThreadAttributeList failed".to_string());
        }
        
        let mut parent_handle = process;
        if update_proc_attr(
                lp_attribute_list, 
                0, 
                PROC_THREAD_ATTRIBUTE_PARENT_PROCESS as usize, 
                &mut parent_handle as *mut _ as *mut c_void, 
                size_of::<HANDLE>(), 
                null_mut(), 
                null_mut()
            ) == 0 {
            delete_proc_attr(lp_attribute_list);
            close_handle(process);
            return Err("UpdateProcThreadAttribute failed".to_string());
        }

        let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
        let cmd_string = crate::utils::concat_wstrings(&[crate::w!("\""), &crate::utils::to_wstring(&exe_path.to_string_lossy()), crate::w!("\"")]);
        // CreateProcessW might modify the command line buffer, though usually not with this flag. 
        // It's safer to have a mutable vector.
        let mut cmd_line = cmd_string.to_vec(); // Ensure we have a mutable copy if needed, though we pass pointer
        
        let mut si_ex: STARTUPINFOEXW = zeroed();
        si_ex.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        si_ex.StartupInfo.dwFlags = STARTF_USESHOWWINDOW;
        si_ex.StartupInfo.wShowWindow = SW_SHOW as u16;
        si_ex.lpAttributeList = lp_attribute_list;

        let mut pi: PROCESS_INFORMATION = zeroed();
        
        let create_res = create_process(
            null(), 
            cmd_line.as_mut_ptr(), 
            null(), 
            null(), 
            FALSE, 
            EXTENDED_STARTUPINFO_PRESENT, 
            null(), 
            null(), 
            &mut si_ex.StartupInfo as *mut _ as *mut c_void, 
            &mut pi as *mut _ as *mut c_void
        );
        
        delete_proc_attr(lp_attribute_list);
        close_handle(process);
        
        if create_res == 0 {
            let err = crate::types::GetLastError();
            return Err(format!("CreateProcessW failed (Error: {})", err));
        }
        close_handle(pi.hProcess);
        close_handle(pi.hThread);
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
