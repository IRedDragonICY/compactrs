use windows_sys::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_QUERY_STATUS_CODE, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_LOCATION,
};
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Storage::FileSystem::{
    DeleteFileW, MoveFileExW, MOVEFILE_REPLACE_EXISTING,
};
use std::ptr;
use std::ffi::c_void;
use crate::utils::to_wstring;
use crate::w;

// Pointer Constants (Not always available in windows-sys as pointers)
const WINHTTP_NO_PROXY_NAME: *const u16 = ptr::null();
const WINHTTP_NO_PROXY_BYPASS: *const u16 = ptr::null();
const WINHTTP_NO_REFERER: *const u16 = ptr::null();
const WINHTTP_DEFAULT_ACCEPT_TYPES: *const *const u16 = ptr::null();
const WINHTTP_NO_ADDITIONAL_HEADERS: *const u16 = ptr::null();

// Constants

const GITHUB_API_HOST: &str = "api.github.com";
const REPO_OWNER: &str = "IRedDragonICY";
const REPO_NAME: &str = "compactrs";

// RAII Wrapper for HINTERNET
struct WinHttpHandle(pub *mut c_void);

impl WinHttpHandle {
    fn new(handle: *mut c_void) -> Option<Self> {
        if handle.is_null() { None } else { Some(Self(handle)) }
    }
}

impl Drop for WinHttpHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
}

// Custom struct to keep handle chain alive
struct WinHttpRequest {
    _session: WinHttpHandle,
    _connect: WinHttpHandle,
    request: WinHttpHandle,
}

// --- Helpers ---

// Helper to parse "https://host/path" -> ("host", "/path")
fn parse_url(url: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = url.splitn(4, '/').collect();
    if parts.len() < 3 {
        return Err(format!("Invalid URL format: {}", url));
    }
    let host = parts[2].to_string();
    let path = if parts.len() > 3 {
        format!("/{}", parts[3])
    } else {
        "/".to_string()
    };
    Ok((host, path))
}

// Core HTTP GET logic with redirect handling
fn perform_http_get(url: &str) -> Result<WinHttpRequest, String> {
    const MAX_REDIRECTS: u32 = 5;
    let mut current_url = url.to_string();
    
    // 1. Initialize Session (Once per operation)
    let session_raw = unsafe {
        WinHttpOpen(
            w!("compactrs/updater").as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            WINHTTP_NO_PROXY_NAME,
            WINHTTP_NO_PROXY_BYPASS,
            0,
        )
    };
    let session = WinHttpHandle::new(session_raw).ok_or_else(|| format!("WinHttpOpen failed: {}", unsafe { GetLastError() }))?;

    for _ in 0..=MAX_REDIRECTS {
        let (host, path) = parse_url(&current_url)?;

        // 2. Connect
        let connect_raw = unsafe {
            WinHttpConnect(session.0, to_wstring(&host).as_ptr(), 443, 0)
        };
        let connect = WinHttpHandle::new(connect_raw).ok_or_else(|| format!("WinHttpConnect failed: {}", unsafe { GetLastError() }))?;

        // 3. Open Request
        let request_raw = unsafe {
            WinHttpOpenRequest(
                connect.0,
                w!("GET").as_ptr(),
                to_wstring(&path).as_ptr(),
                ptr::null(),
                WINHTTP_NO_REFERER,
                WINHTTP_DEFAULT_ACCEPT_TYPES,
                WINHTTP_FLAG_SECURE,
            )
        };
        let request = WinHttpHandle::new(request_raw).ok_or_else(|| format!("WinHttpOpenRequest failed: {}", unsafe { GetLastError() }))?;

        // 4. Send Request
        if unsafe { WinHttpSendRequest(request.0, WINHTTP_NO_ADDITIONAL_HEADERS, 0, ptr::null(), 0, 0, 0) } == 0 {
            return Err(format!("WinHttpSendRequest failed: {}", unsafe { GetLastError() }));
        }

        // 5. Receive Response
        if unsafe { WinHttpReceiveResponse(request.0, ptr::null_mut()) } == 0 {
            return Err(format!("WinHttpReceiveResponse failed: {}", unsafe { GetLastError() }));
        }

        // 6. Check Status
        let mut status_code: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        unsafe {
            WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                ptr::null(),
                &mut status_code as *mut _ as *mut c_void,
                &mut size,
                ptr::null_mut()
            );
        }

        match status_code {
            200 => {
                // Return everything to keep handles alive
                return Ok(WinHttpRequest {
                    _session: session,
                    _connect: connect,
                    request,
                });
            },
            301 | 302 | 307 | 308 => {
                // Handle Redirect
                let mut size: u32 = 0;
                unsafe {
                    WinHttpQueryHeaders(request.0, WINHTTP_QUERY_LOCATION, ptr::null(), ptr::null_mut(), &mut size, ptr::null_mut());
                }
                if size == 0 {
                    return Err(format!("Redirect {} missing Location header", status_code));
                }

                let mut buffer = vec![0u8; size as usize];
                if unsafe { WinHttpQueryHeaders(request.0, WINHTTP_QUERY_LOCATION, ptr::null(), buffer.as_mut_ptr() as *mut c_void, &mut size, ptr::null_mut()) } == 0 {
                     return Err("Failed to read Location header".into());
                }
                
                // Parse Unicode Location
                let location_w: Vec<u16> = buffer
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .take((size / 2) as usize)
                    .collect();
                    
                let new_url = String::from_utf16_lossy(&location_w).trim_matches(char::from(0)).to_string();
                current_url = new_url;
                // session is reused, but connect/request checks drop at end of scope.
                // We need to NOT drop session. 
                // But `session` var is moved into loop? No, `session` is outside.
                // Use session.0 for connect.
                continue; 
            },
            _ => return Err(format!("HTTP Request failed with status: {}", status_code)),
        }
    }
    
    Err("Too many redirects".into())
}

// Generic helper to stream response body
fn read_data_stream<F>(request_handle: &WinHttpHandle, mut writer: F) -> Result<u64, String>
where F: FnMut(&[u8]) -> Result<(), String>
{
    let mut total_bytes = 0;
    loop {
        let mut size: u32 = 0;
        if unsafe { WinHttpQueryDataAvailable(request_handle.0, &mut size) } == 0 {
             // If fails, check if error is expected?
             // Usually returns FALSE on error.
             let err = unsafe { GetLastError() };
             return Err(format!("WinHttpQueryDataAvailable failed: {}", err));
        }
        if size == 0 { break; }
        
        let mut buffer = vec![0u8; size as usize];
        let mut read: u32 = 0;
        if unsafe { WinHttpReadData(request_handle.0, buffer.as_mut_ptr() as *mut c_void, size, &mut read) } == 0 {
             return Err(format!("WinHttpReadData failed: {}", unsafe { GetLastError() }));
        }
        if read == 0 { break; }
        
        writer(&buffer[..read as usize])?;
        total_bytes += read as u64;
    }
    Ok(total_bytes)
}

// --- Public APIs ---

pub fn check_for_updates() -> Result<Option<UpdateInfo>, String> {
    // Construct GitHub API URL
    let url = format!("https://{}/repos/{}/{}/releases/latest", GITHUB_API_HOST, REPO_OWNER, REPO_NAME);
    let req = perform_http_get(&url)?;
    
    // Read Body to memory
    let mut body = Vec::new();
    read_data_stream(&req.request, |chunk| {
        body.extend_from_slice(chunk);
        Ok(())
    })?;
    
    let json_str = String::from_utf8(body).map_err(|e| format!("Invalid UTF-8: {}", e))?;
    let json = crate::json::parse(&json_str).map_err(|e| format!("JSON parse error: {:?}", e))?;
    
    // Parse JSON logic
    let tag_name = json["tag_name"].as_str().ok_or("Missing tag_name")?.to_string();
    let assets = json["assets"].as_array().ok_or("Missing assets")?;
    
    let download_url = assets.iter()
        .find(|asset| asset["name"].as_str() == Some("compactrs.exe"))
        .and_then(|asset| asset["browser_download_url"].as_str())
        .ok_or("No compactrs.exe asset found")?
        .to_string();

    let current_version = env!("APP_VERSION").trim_start_matches('v');
    let remote_version = tag_name.trim_start_matches('v');
    
    if remote_version != current_version {
         Ok(Some(UpdateInfo { version: tag_name, download_url }))
    } else {
        Ok(None)
    }
}

pub fn download_and_start_update(url: &str) -> Result<(), String> {
    let req = perform_http_get(url)?;
    
    let temp_path = std::env::current_exe().map_err(|e| e.to_string())?.with_extension("tmp");
    let mut file = std::fs::File::create(&temp_path).map_err(|e| e.to_string())?;
    
    use std::io::Write;
    let mut first_chunk = true;
    
    // Download and Validate
    let bytes_downloaded = read_data_stream(&req.request, |chunk| {
        if first_chunk {
            if chunk.len() < 2 || chunk[0] != 0x4D || chunk[1] != 0x5A {
                return Err("Invalid executable (missing MZ header)".into());
            }
            first_chunk = false;
        }
        file.write_all(chunk).map_err(|e| e.to_string())
    })?;
    
    if bytes_downloaded == 0 {
        let _ = std::fs::remove_file(&temp_path);
        return Err("Empty download".into());
    }
    // Explicitly drop file to flush and close handle
    drop(file);

    // Replace Logic
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let old_exe = current_exe.with_extension("old");
    
    unsafe {
        // Clean up previous old file if any
        let _ = DeleteFileW(to_wstring(old_exe.to_str().unwrap()).as_ptr());
        
        if MoveFileExW(to_wstring(current_exe.to_str().unwrap()).as_ptr(), to_wstring(old_exe.to_str().unwrap()).as_ptr(), MOVEFILE_REPLACE_EXISTING) == 0 {
             return Err(format!("Failed to move current exe: {}", GetLastError()));
        }
        
        if MoveFileExW(to_wstring(temp_path.to_str().unwrap()).as_ptr(), to_wstring(current_exe.to_str().unwrap()).as_ptr(), MOVEFILE_REPLACE_EXISTING) == 0 {
             // Rollback
             let _ = MoveFileExW(to_wstring(old_exe.to_str().unwrap()).as_ptr(), to_wstring(current_exe.to_str().unwrap()).as_ptr(), MOVEFILE_REPLACE_EXISTING);
             return Err(format!("Failed to replace exe: {}", GetLastError()));
        }
    }
    
    Ok(())
}
