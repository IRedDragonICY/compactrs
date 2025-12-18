use windows_sys::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
    WINHTTP_FLAG_SECURE,
};
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Storage::FileSystem::{
    MoveFileExW, MOVEFILE_REPLACE_EXISTING,
};
use std::ptr;
use std::ffi::c_void;
use crate::utils::to_wstring;

type HINTERNET = *mut c_void;
const WINHTTP_NO_PROXY_NAME: *const u16 = ptr::null();
const WINHTTP_NO_PROXY_BYPASS: *const u16 = ptr::null();
const WINHTTP_NO_REFERER: *const u16 = ptr::null();
const WINHTTP_DEFAULT_ACCEPT_TYPES: *const *const u16 = ptr::null();
const WINHTTP_NO_ADDITIONAL_HEADERS: *const u16 = ptr::null();

const USER_AGENT: &str = "compactrs/updater";
const GITHUB_API_HOST: &str = "api.github.com";
const REPO_OWNER: &str = "IRedDragonICY";
const REPO_NAME: &str = "compactrs";

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
}

struct WinHttpHandle(HINTERNET);

impl WinHttpHandle {
    fn new(handle: HINTERNET) -> Option<Self> {
        if handle.is_null() {
            None
        } else {
            Some(Self(handle))
        }
    }
}

impl Drop for WinHttpHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

pub fn check_for_updates() -> Result<Option<UpdateInfo>, String> {
    // 1. Initialize WinHttp Session
    let session = unsafe {
        WinHttpOpen(
            to_wstring(USER_AGENT).as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            WINHTTP_NO_PROXY_NAME,
            WINHTTP_NO_PROXY_BYPASS,
            0,
        )
    };
    let session = WinHttpHandle::new(session).ok_or_else(|| format!("WinHttpOpen failed: {}", unsafe { GetLastError() }))?;

    // 2. Connect to GitHub API
    let connect = unsafe {
        WinHttpConnect(
            session.0,
            to_wstring(GITHUB_API_HOST).as_ptr(),
            443, // HTTPS port
            0,
        )
    };
    let connect = WinHttpHandle::new(connect).ok_or_else(|| format!("WinHttpConnect failed: {}", unsafe { GetLastError() }))?;

    // 3. Open Request
    let path = format!("/repos/{}/{}/releases/latest", REPO_OWNER, REPO_NAME);
    let request = unsafe {
        WinHttpOpenRequest(
            connect.0,
            to_wstring("GET").as_ptr(),
            to_wstring(&path).as_ptr(),
            ptr::null(), // Version
            WINHTTP_NO_REFERER,
            WINHTTP_DEFAULT_ACCEPT_TYPES,
            WINHTTP_FLAG_SECURE,
        )
    };
    let request = WinHttpHandle::new(request).ok_or_else(|| format!("WinHttpOpenRequest failed: {}", unsafe { GetLastError() }))?;

    // 4. Send Request
    let success = unsafe {
        WinHttpSendRequest(
            request.0,
            WINHTTP_NO_ADDITIONAL_HEADERS,
            0,
            ptr::null(),
            0,
            0,
            0,
        )
    };
    if success == 0 {
        return Err(format!("WinHttpSendRequest failed: {}", unsafe { GetLastError() }));
    }

    // 5. Receive Response
    let success = unsafe { WinHttpReceiveResponse(request.0, ptr::null_mut()) };
    if success == 0 {
        return Err(format!("WinHttpReceiveResponse failed: {}", unsafe { GetLastError() }));
    }

    // 6. Read Response Body
    let mut body = Vec::new();
    loop {
        let mut size: u32 = 0;
        unsafe {
            if WinHttpQueryDataAvailable(request.0, &mut size) == 0 {
                break;
            }
        }
        if size == 0 {
            break;
        }

        let mut buffer = vec![0u8; size as usize];
        let mut downloaded: u32 = 0;
        let success = unsafe {
            WinHttpReadData(
                request.0,
                buffer.as_mut_ptr() as *mut c_void,
                size,
                &mut downloaded,
            )
        };
        if success == 0 {
            return Err(format!("WinHttpReadData failed: {}", unsafe { GetLastError() }));
        }
        buffer.truncate(downloaded as usize);
        body.extend(buffer);
    }

    let json_str = String::from_utf8(body).map_err(|e| format!("Invalid UTF-8: {}", e))?;
    
    // 7. Parse JSON
    let json = crate::json::parse(&json_str).map_err(|e| format!("JSON parse error: {:?}", e))?;
    
    let tag_name = json["tag_name"].as_str().ok_or("Missing tag_name")?.to_string();
    
    // Find asset
    let assets = json["assets"].as_array().ok_or("Missing assets")?;
    let download_url = assets.iter()
        .find(|asset| {
             asset["name"].as_str().map(|n| n == "compactrs.exe").unwrap_or(false)
        })
        .and_then(|asset| asset["browser_download_url"].as_str().map(|s| s.to_string()))
        .ok_or("No compactrs.exe asset found")?;

    let current_version = env!("APP_VERSION");
    
    // Simple comparison logic for now
    // If tag_name starts with 'v', strip it.
    let remote_ver_str = tag_name.trim_start_matches('v');
    let current_ver_str = current_version.trim_start_matches('v');
    
    // If different, assume update. Improving semantic versioning check is better but for now diff check is minimal.
    // Also, usually remote > current, but here != is enough to show update is available.
    if remote_ver_str != current_ver_str {
         Ok(Some(UpdateInfo {
             version: tag_name.clone(),
             download_url: download_url.clone(),
         }))
    } else {
        Ok(None)
    }
}

pub fn download_and_start_update(url: &str) -> Result<(), String> {
    // 1. Download File
    // Initialize WinHttp Session (Reuse logic ideally, duplicating for simplicity to keep functions standalone-ish or refactor later)
    let session = unsafe {
        WinHttpOpen(to_wstring(USER_AGENT).as_ptr(), WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0)
    };
    let session = WinHttpHandle::new(session).ok_or("Failed to open session")?;

    // Parse URL is tricky with WinHttp alone for component extraction without WinHttpCrackUrl
    // Assuming simple structure or just use domain/path.
    // GitHub download URLs are usually on github.com or objects.githubusercontent.com
    // For specific URL 'https://github.com/...' we need to parse host/path.
    // Since we are avoiding deps, a simple split:
    
    let url_parts: Vec<&str> = url.splitn(4, '/').collect();
    // https: / / host / path
    if url_parts.len() < 4 { return Err("Invalid URL format".into()); }
    let host = url_parts[2];
    let path_str = format!("/{}", url_parts[3..].join("/"));
    
    let connect = unsafe { WinHttpConnect(session.0, to_wstring(host).as_ptr(), 443, 0) };
    let connect = WinHttpHandle::new(connect).ok_or("Failed to connect")?;

    let request = unsafe {
        WinHttpOpenRequest(
            connect.0, to_wstring("GET").as_ptr(), to_wstring(&path_str).as_ptr(), ptr::null(),
            WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES, WINHTTP_FLAG_SECURE,
        )
    };
    let request = WinHttpHandle::new(request).ok_or("Failed to open request")?;
    
    if unsafe { WinHttpSendRequest(request.0, WINHTTP_NO_ADDITIONAL_HEADERS, 0, ptr::null(), 0, 0, 0) } == 0 {
         return Err("Failed to send request".into());
    }
    if unsafe { WinHttpReceiveResponse(request.0, ptr::null_mut()) } == 0 {
         return Err("Failed to receive response".into());
    }

    // Read to temp file
    let temp_path = std::env::current_exe().map_err(|e| e.to_string())?.with_extension("tmp");
    let mut file = std::fs::File::create(&temp_path).map_err(|e| e.to_string())?;
    
    loop {
         let mut size = 0;
         unsafe { WinHttpQueryDataAvailable(request.0, &mut size) };
         if size == 0 { break; }
         let mut buf = vec![0u8; size as usize];
         let mut read = 0;
         unsafe { WinHttpReadData(request.0, buf.as_mut_ptr() as *mut _, size, &mut read) };
         if read == 0 { break; }
         use std::io::Write;
         file.write_all(&buf[..read as usize]).map_err(|e| e.to_string())?;
    }
    
    // 2. Move and Replace
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let old_exe = current_exe.with_extension("old");
    
    // Rename current -> old
    unsafe {
        if MoveFileExW(to_wstring(current_exe.to_str().unwrap()).as_ptr(), to_wstring(old_exe.to_str().unwrap()).as_ptr(), MOVEFILE_REPLACE_EXISTING) == 0 {
             return Err(format!("Failed to move current exe: {}", GetLastError()));
        }
    }
    
    // Rename new -> current
    unsafe {
        if MoveFileExW(to_wstring(temp_path.to_str().unwrap()).as_ptr(), to_wstring(current_exe.to_str().unwrap()).as_ptr(), MOVEFILE_REPLACE_EXISTING) == 0 {
             // Try to rollback
             let _ = MoveFileExW(to_wstring(old_exe.to_str().unwrap()).as_ptr(), to_wstring(current_exe.to_str().unwrap()).as_ptr(), MOVEFILE_REPLACE_EXISTING);
             return Err(format!("Failed to replace exe: {}", GetLastError()));
        }
    }
    
    Ok(())
}
