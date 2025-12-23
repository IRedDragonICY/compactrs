//! Self-update module - Downloads and applies updates from GitHub releases.
use windows_sys::Win32::Networking::WinHttp::*;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Storage::FileSystem::{DeleteFileW, MoveFileExW, MOVEFILE_REPLACE_EXISTING};
use std::{ffi::c_void, io::Write, ptr};
use crate::utils::to_wstring;
use crate::w;

const GITHUB_API: &str = "https://api.github.com/repos/IRedDragonICY/compactrs/releases/latest";
const ASSET_NAME: &str = "\"compactrs.exe\"";

// --- RAII Handle ---

struct Handle(*mut c_void);

impl Handle {
    #[inline]
    fn new(h: *mut c_void) -> Option<Self> { (!h.is_null()).then_some(Self(h)) }
}

impl Drop for Handle {
    fn drop(&mut self) { if !self.0.is_null() { unsafe { WinHttpCloseHandle(self.0) }; } }
}

// --- JSON Helpers ---

fn json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let i = json.find(key)? + key.len();
    let s = json[i..].find('"')? + i + 1;
    let e = json[s..].find('"')? + s;
    Some(&json[s..e])
}

fn find_asset_url(json: &str) -> Option<&str> {
    let idx = json.find(ASSET_NAME)?;
    // Find enclosing object braces
    let (mut bal, mut start) = (0, 0);
    for (i, c) in json[..idx].char_indices().rev() {
        match c { '}' => bal += 1, '{' if bal == 0 => { start = i; break }, '{' => bal -= 1, _ => {} }
    }
    let (mut bal, mut end) = (0, json.len());
    for (i, c) in json[idx..].char_indices() {
        match c { '{' => bal += 1, '}' if bal == 0 => { end = idx + i + 1; break }, '}' => bal -= 1, _ => {} }
    }
    json_str(&json[start..end], "\"browser_download_url\"")
}

// --- HTTP ---

struct Request { _ses: Handle, _con: Handle, req: Handle }

fn parse_url(url: &str) -> Result<(&str, &str), &'static str> {
    let s = url.strip_prefix("https://").ok_or("Invalid URL")?;
    Ok(s.find('/').map_or((s, "/"), |i| (&s[..i], &s[i..])))
}

fn http_get(url: &str) -> Result<Request, String> {
    let ses = Handle::new(unsafe {
        WinHttpOpen(w!("compactrs").as_ptr(), WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, ptr::null(), ptr::null(), 0)
    }).ok_or_else(|| format!("WinHttpOpen: {}", unsafe { GetLastError() }))?;

    let mut url = url.to_string();
    for _ in 0..5 {
        let (host, path) = parse_url(&url).map_err(|e| e.to_string())?;
        let host_w = to_wstring(host);
        let path_w = to_wstring(path);

        let con = Handle::new(unsafe { WinHttpConnect(ses.0, host_w.as_ptr(), 443, 0) })
            .ok_or_else(|| format!("Connect: {}", unsafe { GetLastError() }))?;
        let req = Handle::new(unsafe {
            WinHttpOpenRequest(con.0, w!("GET").as_ptr(), path_w.as_ptr(), ptr::null(), ptr::null(), ptr::null(), WINHTTP_FLAG_SECURE)
        }).ok_or_else(|| format!("OpenRequest: {}", unsafe { GetLastError() }))?;

        if unsafe { WinHttpSendRequest(req.0, ptr::null(), 0, ptr::null(), 0, 0, 0) } == 0 {
            return Err(format!("SendRequest: {}", unsafe { GetLastError() }));
        }
        if unsafe { WinHttpReceiveResponse(req.0, ptr::null_mut()) } == 0 {
            return Err(format!("ReceiveResponse: {}", unsafe { GetLastError() }));
        }

        let mut code: u32 = 0;
        let mut sz = 4u32;
        unsafe { WinHttpQueryHeaders(req.0, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER, ptr::null(), &mut code as *mut _ as _, &mut sz, ptr::null_mut()) };

        match code {
            200 => return Ok(Request { _ses: ses, _con: con, req }),
            301 | 302 | 307 | 308 => {
                let mut sz = 0u32;
                unsafe { WinHttpQueryHeaders(req.0, WINHTTP_QUERY_LOCATION, ptr::null(), ptr::null_mut(), &mut sz, ptr::null_mut()) };
                if sz == 0 { return Err("Redirect missing Location".into()); }
                let mut buf = vec![0u8; sz as usize];
                if unsafe { WinHttpQueryHeaders(req.0, WINHTTP_QUERY_LOCATION, ptr::null(), buf.as_mut_ptr() as _, &mut sz, ptr::null_mut()) } == 0 {
                    return Err("Read Location failed".into());
                }
                let loc: Vec<u16> = buf.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                url = String::from_utf16_lossy(&loc).trim_matches('\0').to_string();
            }
            _ => return Err(format!("HTTP {}", code)),
        }
    }
    Err("Too many redirects".into())
}

fn read_body<F: FnMut(&[u8]) -> Result<(), String>>(req: &Handle, mut f: F) -> Result<u64, String> {
    let mut buf = [0u8; 8192];
    let mut total = 0u64;
    loop {
        let mut n = 0u32;
        if unsafe { WinHttpReadData(req.0, buf.as_mut_ptr() as _, buf.len() as u32, &mut n) } == 0 {
            return Err(format!("ReadData: {}", unsafe { GetLastError() }));
        }
        if n == 0 { break; }
        f(&buf[..n as usize])?;
        total += n as u64;
    }
    Ok(total)
}

// --- Public API ---

#[derive(Debug, Clone)]
pub struct UpdateInfo { pub version: String, pub download_url: String }

pub fn check_for_updates() -> Result<Option<UpdateInfo>, String> {
    let req = http_get(GITHUB_API)?;
    let mut body = Vec::new();
    read_body(&req.req, |c| { body.extend_from_slice(c); Ok(()) })?;
    
    let json = String::from_utf8(body).map_err(|e| e.to_string())?;
    let tag = json_str(&json, "\"tag_name\"").ok_or("Missing tag_name")?;
    let url = find_asset_url(&json).ok_or("No compactrs.exe asset")?.to_string();

    let cur = env!("APP_VERSION").trim_start_matches('v');
    let rem = tag.trim_start_matches('v');
    Ok((rem != cur).then_some(UpdateInfo { version: tag.to_string(), download_url: url }))
}

pub fn download_and_start_update(url: &str) -> Result<(), String> {
    let req = http_get(url)?;
    let tmp = std::env::current_exe().map_err(|e| e.to_string())?.with_extension("tmp");
    let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut first = true;

    let bytes = read_body(&req.req, |chunk| {
        if first {
            if chunk.len() < 2 || chunk[0] != 0x4D || chunk[1] != 0x5A {
                return Err("Invalid executable".into());
            }
            first = false;
        }
        file.write_all(chunk).map_err(|e| e.to_string())
    })?;

    if bytes == 0 { let _ = std::fs::remove_file(&tmp); return Err("Empty download".into()); }
    drop(file);

    let cur = std::env::current_exe().map_err(|e| e.to_string())?;
    let old = cur.with_extension("old");
    let (cur_w, old_w, tmp_w) = (to_wstring(cur.to_str().unwrap()), to_wstring(old.to_str().unwrap()), to_wstring(tmp.to_str().unwrap()));

    unsafe {
        let _ = DeleteFileW(old_w.as_ptr());
        if MoveFileExW(cur_w.as_ptr(), old_w.as_ptr(), MOVEFILE_REPLACE_EXISTING) == 0 {
            return Err(format!("Move current: {}", GetLastError()));
        }
        if MoveFileExW(tmp_w.as_ptr(), cur_w.as_ptr(), MOVEFILE_REPLACE_EXISTING) == 0 {
            let _ = MoveFileExW(old_w.as_ptr(), cur_w.as_ptr(), MOVEFILE_REPLACE_EXISTING);
            return Err(format!("Replace exe: {}", GetLastError()));
        }
    }
    Ok(())
}
