#![allow(unsafe_op_in_unsafe_fn)]
use crate::types::*;
use std::{mem::{size_of as sz, zeroed}, ptr::{null, null_mut}};

#[link(name = "kernel32")] unsafe extern "system" { fn Sleep(ms: u32); }

unsafe fn priv_ok(n: LPCWSTR) -> bool {
    let (mut t, mut l): (HANDLE, LUID) = (null_mut(), zeroed());
    if OpenProcessToken(-1isize as _, 0x28, &mut t) == 0 { return false; }
    if LookupPrivilegeValueW(null(), n, &mut l) == 0 { CloseHandle(t); return false; }
    let p = TOKEN_PRIVILEGES { PrivilegeCount: 1, Privileges: [LUID_AND_ATTRIBUTES { Luid: l, Attributes: 2 }] };
    let r = AdjustTokenPrivileges(t, 0, &p as *const _ as _, sz::<TOKEN_PRIVILEGES>() as u32, null_mut(), null_mut());
    let e = GetLastError(); CloseHandle(t); r != 0 && e != 1300
}

unsafe fn ti_pid() -> Option<u32> {
    let m = OpenSCManagerW(null(), null(), 1);
    if m.is_null() { return None; }
    let s = OpenServiceW(m, crate::w!("TrustedInstaller").as_ptr(), 0x14);
    if s.is_null() { CloseServiceHandle(m); return None; }
    StartServiceW(s, 0, null());
    for _ in 0..20 {
        let (mut p, mut n): (SERVICE_STATUS_PROCESS, u32) = (zeroed(), 0);
        if QueryServiceStatusEx(s, 0, &mut p as *mut _ as _, sz::<SERVICE_STATUS_PROCESS>() as u32, &mut n) != 0 && p.dwCurrentState == 4 && p.dwProcessId != 0 {
            CloseServiceHandle(s); CloseServiceHandle(m); return Some(p.dwProcessId);
        }
        Sleep(100);
    }
    CloseServiceHandle(s); CloseServiceHandle(m); None
}

pub fn restart_as_trusted_installer() -> Result<(), &'static str> {
    unsafe {
        if !priv_ok(crate::w!("SeDebugPrivilege").as_ptr()) { return Err("P"); }
        let h = OpenProcess(0x80, 0, ti_pid().ok_or("T")?);
        if h.is_null() { return Err("O"); }
        
        let (mut b, mut z) = ([0u8; 64], 64usize);
        let a = b.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
        if InitializeProcThreadAttributeList(a, 1, 0, &mut z) == 0 { CloseHandle(h); return Err("I"); }
        let mut p = h;
        if UpdateProcThreadAttribute(a, 0, 0x20000, &mut p as *mut _ as _, sz::<HANDLE>(), null_mut(), null_mut()) == 0 { DeleteProcThreadAttributeList(a); CloseHandle(h); return Err("A"); }

        let mut c = [0u16; 310];
        c[0] = 34; // "
        let l = GetModuleFileNameW(null_mut(), c[1..].as_mut_ptr(), 300) as usize;
        c[l + 1] = 34; // "

        let mut si: STARTUPINFOEXW = zeroed();
        si.StartupInfo.cb = sz::<STARTUPINFOEXW>() as u32;
        si.StartupInfo.dwFlags = 1;
        si.StartupInfo.wShowWindow = 5;
        si.lpAttributeList = a;

        let mut pi: PROCESS_INFORMATION = zeroed();
        let r = CreateProcessW(null(), c.as_mut_ptr(), null(), null(), 0, 0x80000, null(), null(), &mut si.StartupInfo as *mut _ as _, &mut pi as *mut _ as _);
        DeleteProcThreadAttributeList(a); CloseHandle(h);
        if r == 0 { return Err("C"); }
        CloseHandle(pi.hProcess); CloseHandle(pi.hThread);
        std::process::exit(0);
    }
}

pub fn is_system_or_ti() -> bool {
    unsafe {
        let (mut b, mut z) = ([0u16; 8], 8u32);
        GetUserNameW(b.as_mut_ptr(), &mut z) != 0 && z >= 7 && {
            let u = |c: u16| c & !32; // uppercase ASCII
            u(b[0]) == 83 && u(b[1]) == 89 && u(b[2]) == 83 && u(b[3]) == 84 && u(b[4]) == 69 && u(b[5]) == 77
        }
    }
}