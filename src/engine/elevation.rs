#![allow(unsafe_op_in_unsafe_fn, non_snake_case, non_camel_case_types, non_upper_case_globals)]
use crate::utils::to_wstring;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{CloseHandle, FALSE, HANDLE, LUID, GetLastError, ERROR_NOT_ALL_ASSIGNED};

// --- Manual Bindings & Structs ---

#[repr(C)]
struct STARTUPINFOEXW {
    StartupInfo: STARUPINFOW,
    lpAttributeList: LPPROC_THREAD_ATTRIBUTE_LIST,
}

#[repr(C)]
struct STARUPINFOW {
    cb: u32,
    lpReserved: *mut u16,
    lpDesktop: *mut u16,
    lpTitle: *mut u16,
    dwX: u32,
    dwY: u32,
    dwXSize: u32,
    dwYSize: u32,
    dwXCountChars: u32,
    dwYCountChars: u32,
    dwFillAttribute: u32,
    dwFlags: u32,
    wShowWindow: u16,
    cbReserved2: u16,
    lpReserved2: *mut u8,
    hStdInput: HANDLE,
    hStdOutput: HANDLE,
    hStdError: HANDLE,
}

#[repr(C)]
struct PROCESS_INFORMATION {
    hProcess: HANDLE,
    hThread: HANDLE,
    dwProcessId: u32,
    dwThreadId: u32,
}

#[repr(C)]
struct TOKEN_PRIVILEGES {
    PrivilegeCount: u32,
    Privileges: [LUID_AND_ATTRIBUTES; 1],
}

#[repr(C)]
struct LUID_AND_ATTRIBUTES {
    Luid: LUID,
    Attributes: u32,
}

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

type SC_HANDLE = HANDLE;
type LPPROC_THREAD_ATTRIBUTE_LIST = *mut std::ffi::c_void;

// Constants
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

#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenProcessToken(ProcessHandle: HANDLE, DesiredAccess: u32, TokenHandle: *mut HANDLE) -> i32;
    fn LookupPrivilegeValueW(lpSystemName: *const u16, lpName: *const u16, lpLuid: *mut LUID) -> i32;
    fn AdjustTokenPrivileges(TokenHandle: HANDLE, DisableAllPrivileges: i32, NewState: *const TOKEN_PRIVILEGES, BufferLength: u32, PreviousState: *mut std::ffi::c_void, ReturnLength: *mut u32) -> i32;
    fn OpenSCManagerW(lpMachineName: *const u16, lpDatabaseName: *const u16, dwDesiredAccess: u32) -> SC_HANDLE;
    fn OpenServiceW(hSCManager: SC_HANDLE, lpServiceName: *const u16, dwDesiredAccess: u32) -> SC_HANDLE;
    fn CloseServiceHandle(hSCObject: SC_HANDLE) -> i32;
    fn StartServiceW(hService: SC_HANDLE, dwNumServiceArgs: u32, lpServiceArgVectors: *const *const u16) -> i32;
    fn QueryServiceStatusEx(hService: SC_HANDLE, InfoLevel: u32, lpBuffer: *mut u8, cbBufSize: u32, pcbBytesNeeded: *mut u32) -> i32;
    fn GetUserNameW(lpBuffer: *mut u16, pcbBuffer: *mut u32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> HANDLE;
    fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> HANDLE;
    fn CreateProcessW(lpApplicationName: *const u16, lpCommandLine: *mut u16, lpProcessAttributes: *const std::ffi::c_void, lpThreadAttributes: *const std::ffi::c_void, bInheritHandles: i32, dwCreationFlags: u32, lpEnvironment: *const std::ffi::c_void, lpCurrentDirectory: *const u16, lpStartupInfo: *mut STARUPINFOW, lpProcessInformation: *mut PROCESS_INFORMATION) -> i32;
    fn InitializeProcThreadAttributeList(lpAttributeList: LPPROC_THREAD_ATTRIBUTE_LIST, dwAttributeCount: u32, dwFlags: u32, lpSize: *mut usize) -> i32;
    fn UpdateProcThreadAttribute(lpAttributeList: LPPROC_THREAD_ATTRIBUTE_LIST, dwFlags: u32, Attribute: usize, lpValue: *const std::ffi::c_void, cbSize: usize, lpPreviousValue: *mut std::ffi::c_void, lpReturnSize: *mut usize) -> i32;
    fn DeleteProcThreadAttributeList(lpAttributeList: LPPROC_THREAD_ATTRIBUTE_LIST);
}

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
                null_mut()
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
        let mut buffer = [0u16; 256];
        let mut size = 256;
        if GetUserNameW(buffer.as_mut_ptr(), &mut size) != 0 {
            let name = String::from_utf16_lossy(&buffer[..size as usize - 1]);
            return name.to_uppercase().contains("SYSTEM");
        }
    }
    false
}
