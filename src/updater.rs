//! Self-update: extreme optimization (Win32 IO, no std::fs/io, stack paths)
use crate::types::*;
use std::{ffi::c_void, ptr::{null, null_mut}};

const API: &str = "https://api.github.com/repos/IRedDragonICY/compactrs/releases/latest";

#[link(name = "winhttp")]
unsafe extern "system" {
    fn WinHttpOpen(a: LPCWSTR, b: u32, c: LPCWSTR, d: LPCWSTR, e: u32) -> *mut c_void;
    fn WinHttpConnect(a: *mut c_void, b: LPCWSTR, c: u16, d: u32) -> *mut c_void;
    fn WinHttpOpenRequest(a: *mut c_void, b: LPCWSTR, c: LPCWSTR, d: LPCWSTR, e: LPCWSTR, f: *const *const u16, g: u32) -> *mut c_void;
    fn WinHttpSendRequest(a: *mut c_void, b: LPCWSTR, c: u32, d: *const c_void, e: u32, f: u32, g: usize) -> i32;
    fn WinHttpReceiveResponse(a: *mut c_void, b: *mut c_void) -> i32;
    fn WinHttpQueryHeaders(a: *mut c_void, b: u32, c: LPCWSTR, d: *mut c_void, e: *mut u32, f: *mut u32) -> i32;
    fn WinHttpReadData(a: *mut c_void, b: *mut c_void, c: u32, d: *mut u32) -> i32;
    fn WinHttpCloseHandle(a: *mut c_void) -> i32;
}

// Minimal Handle wrapper
struct Handle(*mut c_void);
impl Drop for Handle { fn drop(&mut self) { if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE { unsafe { WinHttpCloseHandle(self.0); } } } }
struct FileHandle(HANDLE);
impl Drop for FileHandle { fn drop(&mut self) { if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE { unsafe { CloseHandle(self.0); } } } }

struct Link { _s: Handle, _c: Handle, req: Handle }

// Helpers
fn val<'a>(s: &'a str, k: &str) -> Option<&'a str> { s.split(k).nth(1)?.split('"').nth(1) }
fn w(s: &str) -> Vec<u16> { crate::utils::to_wstring(s) }
fn w_buf(s: &str, buf: &mut [u16]) { for (i, c) in s.encode_utf16().enumerate() { if i < buf.len() { buf[i] = c; } } buf[s.len().min(buf.len()-1)] = 0; }

fn get(url: &str) -> Result<Link, &'static str> {
    let s = Handle(unsafe { WinHttpOpen(crate::w!("CompactRS/1.0").as_ptr(), 0, null(), null(), 0) });
    if s.0.is_null() { return Err("Open"); }
    
    // Redirect loop with fixed stack buffer
    let mut u_buf = [0u16; 512]; 
    w_buf(url, &mut u_buf);
    
    // Use string parsing to avoid allocations? "https://" is ASCII.
    for _ in 0..5 {
        // Unsafe internal conversion for speed/no-alloc? No, just use simple parsing.
        let u_str = String::from_utf16_lossy(&u_buf).trim_matches('\0').to_string(); // Alloc here is acceptable for few redir
        let v = u_str.strip_prefix("https://").ok_or("Bad URL")?;
        let (h, p) = v.find('/').map_or((v, "/"), |i| (&v[..i], &v[i..]));
        
        // Connect
        let c = Handle(unsafe { WinHttpConnect(s.0, w(h).as_ptr(), 443, 0) });
        if c.0.is_null() { return Err("Connect"); }
        let q = Handle(unsafe { WinHttpOpenRequest(c.0, null(), w(p).as_ptr(), null(), null(), null(), 0x800000) });
        if q.0.is_null() { return Err("Request"); }
        
        let hdr = crate::w!("Accept: application/vnd.github+json");
        if unsafe { WinHttpSendRequest(q.0, hdr.as_ptr(), hdr.len() as u32 - 1, null(), 0, 0, 0) } == 0 { return Err("Send"); }
        if unsafe { WinHttpReceiveResponse(q.0, null_mut()) } == 0 { return Err("Recv"); }
        
        let (mut code, mut sz) = (0u32, 4u32);
        unsafe { WinHttpQueryHeaders(q.0, 536870931, null(), &mut code as *mut _ as _, &mut sz, null_mut()) };
        if code == 200 { return Ok(Link { _s: s, _c: c, req: q }); }
        if ![301, 302, 307, 308].contains(&code) { return Err("HTTP Error"); }
        
        // Reuse buffer for redirect header
        u_buf.fill(0); sz = 1024;
        unsafe { WinHttpQueryHeaders(q.0, 33, null(), u_buf.as_mut_ptr() as _, &mut sz, null_mut()) };
    }
    Err("Redirect Loop")
}

#[derive(Clone, Debug)]
pub struct UpdateInfo { pub version: String, pub download_url: String }

pub fn check_for_updates() -> Result<Option<UpdateInfo>, String> {
    let l = get(API).map_err(|e| e.to_string())?;
    
    // Read to Vec (heap) is fine for JSON
    let (mut buf, mut chunk, mut n) = (Vec::new(), [0u8; 4096], 0);
    while unsafe { WinHttpReadData(l.req.0, chunk.as_mut_ptr() as _, 4096, &mut n) } != 0 && n > 0 { buf.extend_from_slice(&chunk[..n as usize]); }
    let json = String::from_utf8(buf).map_err(|_| "Bad JSON")?;
    
    if let Some(msg) = val(&json, "\"message\"") { return Err(format!("API: {}", msg)); }
    let ver = val(&json, "\"tag_name\"").ok_or("No tag")?;
    let url = json.split('{').find(|s| s.contains("compactrs.exe") && s.contains("browser_download_url"))
        .and_then(|s| val(s, "\"browser_download_url\"")).ok_or("No asset")?;
        
    let cur = env!("APP_VERSION").trim_start_matches('v');
    Ok((ver.trim_start_matches('v') != cur).then(|| UpdateInfo { version: ver.into(), download_url: url.into() }))
}

pub fn download_and_start_update(url: &str) -> Result<(), String> {
    let l = get(url).map_err(|e| e.to_string())?;

    // Path Logic - Stack allocated buffers
    let mut path_exe = [0u16; 300];
    unsafe { GetModuleFileNameW(null_mut(), path_exe.as_mut_ptr(), 300) };
    
    let len = path_exe.iter().position(|&c| c == 0).unwrap_or(300);
    let mut path_tmp = path_exe;
    // Replace .exe with .tmp roughly or append
    // Easiest: copy path_exe, find extension or append
    // Just append .tmp for safety? extension might be tricky if not .exe
    // Let's assume .exe and replace last 3 chars? No, path could be anything.
    // Safe generic: append .tmp
    // But buffer size limited.
    // Check space: 300 - len > 4
    if len + 5 >= 300 { return Err("Path too long".into()); }
    let tmp_ext = crate::w!(".tmp");
    for (i, &c) in tmp_ext.iter().enumerate() { path_tmp[len + i] = c; } // Append .tmp (including null from macro? No macro gives slice with null)
    // Macro gives slice with null. So it works: . t m p \0. Overwrite null of original?
    // Original: [.., 'e', 0, 0]
    // We start writing at len (index of 0).
    // .tmp\0 -> 5 chars.
    
    let mut path_old = path_exe;
     let old_ext = crate::w!(".old");
     for (i, &c) in old_ext.iter().enumerate() { path_old[len + i] = c; }
    
    // Create File
    let h_file = unsafe { 
        CreateFileW(
            path_tmp.as_ptr(), 
            GENERIC_WRITE, 
            0, null_mut(), 
            CREATE_ALWAYS, 
            FILE_ATTRIBUTE_NORMAL, 
            null_mut()
        ) 
    };
    if h_file == INVALID_HANDLE_VALUE { return Err("CreateFile".into()); }
    let _f_guard = FileHandle(h_file); // Ensure close
    
    let (mut buf, mut n, mut w, mut first) = ([0u8; 8192], 0, 0, true);
    while unsafe { WinHttpReadData(l.req.0, buf.as_mut_ptr() as _, 8192, &mut n) } != 0 && n > 0 {
        if first { if n < 2 || buf[0] != 0x4D || buf[1] != 0x5A { return Err("Not MZ".into()); } first = false; }
        if unsafe { WriteFile(h_file, buf.as_ptr() as _, n, &mut w, null_mut()) } == 0 { return Err("Write".into()); }
    }
    drop(_f_guard); // Close explicitly before move

    unsafe {
        DeleteFileW(path_old.as_ptr());
        if MoveFileExW(path_exe.as_ptr(), path_old.as_ptr(), 1) == 0 { return Err("Backup Fail".into()); }
        if MoveFileExW(path_tmp.as_ptr(), path_exe.as_ptr(), 1) == 0 {
            MoveFileExW(path_old.as_ptr(), path_exe.as_ptr(), 1);
            return Err("Update Fail".into());
        }
    }
    Ok(())
}
