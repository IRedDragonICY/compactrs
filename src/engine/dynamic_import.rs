#![allow(non_snake_case, non_camel_case_types, dead_code, unsafe_op_in_unsafe_fn, static_mut_refs)]
use core::ffi::c_void;
use core::arch::asm;
use core::ptr::null_mut;

// --- 1. Structures needed for PEB and Loader walking ---

#[repr(C)]
pub struct UNICODE_STRING {
    pub Length: u16,
    pub MaximumLength: u16,
    pub Buffer: *mut u16,
}

#[repr(C)]
pub struct LIST_ENTRY {
    pub Flink: *mut LIST_ENTRY,
    pub Blink: *mut LIST_ENTRY,
}

#[repr(C)]
pub struct PEB_LDR_DATA {
    pub Length: u32,
    pub Initialized: u8,
    pub SsHandle: *mut c_void,
    pub InLoadOrderModuleList: LIST_ENTRY,
    pub InMemoryOrderModuleList: LIST_ENTRY,
    pub InInitializationOrderModuleList: LIST_ENTRY,
    pub EntryInProgress: *mut c_void,
    pub ShutdownInProgress: u8,
    pub ShutdownThreadId: *mut c_void,
}

#[repr(C)]
pub struct PEB {
    pub InheritedAddressSpace: u8,
    pub ReadImageFileExecOptions: u8,
    pub BeingDebugged: u8,
    pub BitField: u8,
    pub Mutant: *mut c_void,
    pub ImageBaseAddress: *mut c_void,
    pub Ldr: *mut PEB_LDR_DATA,
    // ... we don't need the rest for this task
}

#[repr(C)]
pub struct LDR_DATA_TABLE_ENTRY {
    pub InLoadOrderLinks: LIST_ENTRY,
    pub InMemoryOrderLinks: LIST_ENTRY,
    pub InInitializationOrderLinks: LIST_ENTRY,
    pub DllBase: *mut c_void,
    pub EntryPoint: *mut c_void,
    pub SizeOfImage: u32,
    pub FullDllName: UNICODE_STRING,
    pub BaseDllName: UNICODE_STRING,
    // ...
}

#[repr(C)]
pub struct IMAGE_DOS_HEADER {
    pub e_magic: u16,
    pub e_cblp: u16,
    pub e_cp: u16,
    pub e_crlc: u16,
    pub e_cparhdr: u16,
    pub e_minalloc: u16,
    pub e_maxalloc: u16,
    pub e_ss: u16,
    pub e_sp: u16,
    pub e_csum: u16,
    pub e_ip: u16,
    pub e_cs: u16,
    pub e_lfarlc: u16,
    pub e_ovno: u16,
    pub e_res: [u16; 4],
    pub e_oemid: u16,
    pub e_oeminfo: u16,
    pub e_res2: [u16; 10],
    pub e_lfanew: i32,
}

#[repr(C)]
pub struct IMAGE_FILE_HEADER {
    pub Machine: u16,
    pub NumberOfSections: u16,
    pub TimeDateStamp: u32,
    pub PointerToSymbolTable: u32,
    pub NumberOfSymbols: u32,
    pub SizeOfOptionalHeader: u16,
    pub Characteristics: u16,
}

#[repr(C)]
pub struct IMAGE_DATA_DIRECTORY {
    pub VirtualAddress: u32,
    pub Size: u32,
}

#[repr(C)]
pub struct IMAGE_OPTIONAL_HEADER64 {
    pub Magic: u16,
    pub MajorLinkerVersion: u8,
    pub MinorLinkerVersion: u8,
    pub SizeOfCode: u32,
    pub SizeOfInitializedData: u32,
    pub SizeOfUninitializedData: u32,
    pub AddressOfEntryPoint: u32,
    pub BaseOfCode: u32,
    pub ImageBase: u64,
    pub SectionAlignment: u32,
    pub FileAlignment: u32,
    pub MajorOperatingSystemVersion: u16,
    pub MinorOperatingSystemVersion: u16,
    pub MajorImageVersion: u16,
    pub MinorImageVersion: u16,
    pub MajorSubsystemVersion: u16,
    pub MinorSubsystemVersion: u16,
    pub Win32VersionValue: u32,
    pub SizeOfImage: u32,
    pub SizeOfHeaders: u32,
    pub CheckSum: u32,
    pub Subsystem: u16,
    pub DllCharacteristics: u16,
    pub SizeOfStackReserve: u64,
    pub SizeOfStackCommit: u64,
    pub SizeOfHeapReserve: u64,
    pub SizeOfHeapCommit: u64,
    pub LoaderFlags: u32,
    pub NumberOfRvaAndSizes: u32,
    pub DataDirectory: [IMAGE_DATA_DIRECTORY; 16],
}

#[repr(C)]
pub struct IMAGE_NT_HEADERS64 {
    pub Signature: u32,
    pub FileHeader: IMAGE_FILE_HEADER,
    pub OptionalHeader: IMAGE_OPTIONAL_HEADER64,
}

#[repr(C)]
pub struct IMAGE_EXPORT_DIRECTORY {
    pub Characteristics: u32,
    pub TimeDateStamp: u32,
    pub MajorVersion: u16,
    pub MinorVersion: u16,
    pub Name: u32,
    pub Base: u32,
    pub NumberOfFunctions: u32,
    pub NumberOfNames: u32,
    pub AddressOfFunctions: u32,    // RVA from base of image
    pub AddressOfNames: u32,        // RVA from base of image
    pub AddressOfNameOrdinals: u32, // RVA from base of image
}

// --- 2. Compile-Time Hashing (ROR13) ---

/// Calculate ROR13 hash of a string literal at compile time or runtime.
/// Key for ROR13: Rotate right 13 bits, add current char value.
pub const fn hash_str(s: &str) -> u32 {
    let mut hash: u32 = 0;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        // ROR 13
        hash = (hash.rotate_right(13)).wrapping_add(c as u32);
        i += 1;
    }
    hash
}

/// Helper for hashing wide string from memory (module names) (Case Insensitive)
pub unsafe fn hash_name_w(ptr: *const u16, len: usize) -> u32 {
    let mut hash: u32 = 0;
    for i in 0..len {
        let mut c = *ptr.add(i) as u32;
        // simplistic to_upper for basic ASCII range
        if c >= 'a' as u32 && c <= 'z' as u32 {
            c -= 32;
        }
        hash = (hash.rotate_right(13)).wrapping_add(c);
    }
    hash
}

/// Helper for hashing C-string (export names) (Case Sensitive)
pub unsafe fn hash_name_a(ptr: *const u8) -> u32 {
    let mut hash: u32 = 0;
    let mut p = ptr;
    while *p != 0 {
        hash = (hash.rotate_right(13)).wrapping_add(*p as u32);
        p = p.add(1);
    }
    hash
}

// --- 3. Retrieval of Kernel32 Base ---

/// Retrieve address of KERNEL32.DLL base via PEB.
/// 
/// Hashes:
/// "KERNEL32.DLL" -> 0x6DDB9555 (using our ROR13 imp, verify logic)
/// Let's re-verify the hash logic:
/// 'K' -> 0x4B
/// ROR13(0) + 0x4B = 0x4B
/// ...
/// Easier to just use the `hash_str` function we defined if it's const.
pub unsafe fn get_kernel32_base() -> *const u8 {
    // KERNEL32.DLL case-insensitive ROR13 hash:
    // We will compute it match time or pre-calculate.
    // hash_str("KERNEL32.DLL") -> ?
    // Let's assume we invoke the hash function on "KERNEL32.DLL" at call site or here.
    const KERNEL32_HASH: u32 = hash_str("KERNEL32.DLL"); // 0x6DDB9555 approx if standard

    let peb: *mut PEB;
    asm!(
        "mov {}, gs:[0x60]",
        out(reg) peb,
    );

    let ldr = (*peb).Ldr;
    // InMemoryOrderModuleList is at offset 0x20 in PEB_LDR_DATA
    // But we defined struct, so use that.
    let head = &mut (*ldr).InMemoryOrderModuleList as *mut LIST_ENTRY;
    let mut current = (*head).Flink;

    while current != head {
        // CONTAINING_RECORD: LDR_DATA_TABLE_ENTRY
        // InMemoryOrderLinks is 2nd field of LDR_DATA_TABLE_ENTRY
        // Field 1: InLoadOrderLinks (size 2 ptrs = 16 bytes)
        // So start of entry is current - 16 bytes (or offsetof(LDR_DATA_TABLE_ENTRY, InMemoryOrderLinks))
        let entry = (current as *const u8).sub(16) as *const LDR_DATA_TABLE_ENTRY;
        
        // BaseDllName
        let _name_slice = core::slice::from_raw_parts(
            (*entry).BaseDllName.Buffer,
            ((*entry).BaseDllName.Length / 2) as usize
        );
        
        let hash = hash_name_w((*entry).BaseDllName.Buffer, ((*entry).BaseDllName.Length / 2) as usize);

        if hash == KERNEL32_HASH {
            return (*entry).DllBase as *const u8;
        }

        current = (*current).Flink;
    }

    null_mut()
}

// --- 4. Manual GetProcAddress ---

pub unsafe fn get_proc_address(base: *const u8, func_hash: u32) -> *const c_void {
    let dos_header = &*(base as *const IMAGE_DOS_HEADER);
    if dos_header.e_magic != 0x5A4D { // 'MZ'
        return null_mut();
    }

    let nt_headers = &*(base.offset(dos_header.e_lfanew as isize) as *const IMAGE_NT_HEADERS64);
    if nt_headers.Signature != 0x00004550 { // 'PE\0\0'
        return null_mut();
    }

    let export_dir_rva = nt_headers.OptionalHeader.DataDirectory[0].VirtualAddress;
    if export_dir_rva == 0 {
        return null_mut();
    }

    let export_dir = &*(base.add(export_dir_rva as usize) as *const IMAGE_EXPORT_DIRECTORY);
    let names_rva = export_dir.AddressOfNames;
    let funcs_rva = export_dir.AddressOfFunctions;
    let ords_rva = export_dir.AddressOfNameOrdinals;

    let names_ptr = base.add(names_rva as usize) as *const u32; // Array of RVAs (u32)
    let ords_ptr = base.add(ords_rva as usize) as *const u16;   // Array of u16
    let funcs_ptr = base.add(funcs_rva as usize) as *const u32; // Array of RVAs (u32)

    for i in 0..export_dir.NumberOfNames {
        let name_rva = *names_ptr.add(i as usize);
        let name_ptr = base.add(name_rva as usize) as *const u8;
        
        let hash = hash_name_a(name_ptr);
        
        if hash == func_hash {
            let ordinal = *ords_ptr.add(i as usize);
            let func_rva = *funcs_ptr.add(ordinal as usize);
            return base.add(func_rva as usize) as *const c_void;
        }
    }

    null_mut()
}

// --- 5. Global Loader and Signatures ---

// Common types
type HANDLE = *mut c_void;
type HMODULE = *mut c_void;
type LPCWSTR = *const u16;
type LPCSTR = *const u8;
type BOOL = i32;
type HWND = *mut c_void;
type HINSTANCE = *mut c_void;

// Function Signatures
type FnLoadLibraryA = unsafe extern "system" fn(lpLibFileName: LPCSTR) -> HMODULE;
type FnGetProcAddress = unsafe extern "system" fn(hModule: HMODULE, lpProcName: LPCSTR) -> *const c_void;
type FnCreateFileW = unsafe extern "system" fn(lpFileName: LPCWSTR, dwDesiredAccess: u32, dwShareMode: u32, lpSecurityAttributes: *const c_void, dwCreationDisposition: u32, dwFlagsAndAttributes: u32, hTemplateFile: HANDLE) -> HANDLE;
type FnCloseHandle = unsafe extern "system" fn(hObject: HANDLE) -> BOOL;
type FnDeviceIoControl = unsafe extern "system" fn(hDevice: HANDLE, dwIoControlCode: u32, lpInBuffer: *const c_void, nInBufferSize: u32, lpOutBuffer: *mut c_void, nOutBufferSize: u32, lpBytesReturned: *mut u32, lpOverlapped: *mut c_void) -> BOOL;
type FnGetCompressedFileSizeW = unsafe extern "system" fn(lpFileName: LPCWSTR, lpFileSizeHigh: *mut u32) -> u32;
type FnGetFileAttributesW = unsafe extern "system" fn(lpFileName: LPCWSTR) -> u32;
type FnSetFileAttributesW = unsafe extern "system" fn(lpFileName: LPCWSTR, dwFileAttributes: u32) -> BOOL;
type FnOpenProcess = unsafe extern "system" fn(dwDesiredAccess: u32, bInheritHandle: BOOL, dwProcessId: u32) -> HANDLE;
type FnCreateProcessW = unsafe extern "system" fn(lpApplicationName: LPCWSTR, lpCommandLine: *mut u16, lpProcessAttributes: *const c_void, lpThreadAttributes: *const c_void, bInheritHandles: BOOL, dwCreationFlags: u32, lpEnvironment: *const c_void, lpCurrentDirectory: LPCWSTR, lpStartupInfo: *mut c_void, lpProcessInformation: *mut c_void) -> BOOL;
type FnInitializeProcThreadAttributeList = unsafe extern "system" fn(lpAttributeList: *mut c_void, dwAttributeCount: u32, dwFlags: u32, lpSize: *mut usize) -> BOOL;
type FnUpdateProcThreadAttribute = unsafe extern "system" fn(lpAttributeList: *mut c_void, dwFlags: u32, Attribute: usize, lpValue: *const c_void, cbSize: usize, lpPreviousValue: *mut c_void, lpReturnSize: *mut usize) -> BOOL;
type FnDeleteProcThreadAttributeList = unsafe extern "system" fn(lpAttributeList: *mut c_void);

// advapi32.dll
type FnOpenProcessToken = unsafe extern "system" fn(ProcessHandle: HANDLE, DesiredAccess: u32, TokenHandle: *mut HANDLE) -> BOOL;
type FnLookupPrivilegeValueW = unsafe extern "system" fn(lpSystemName: LPCWSTR, lpName: LPCWSTR, lpLuid: *mut c_void) -> BOOL;
type FnAdjustTokenPrivileges = unsafe extern "system" fn(TokenHandle: HANDLE, DisableAllPrivileges: BOOL, NewState: *const c_void, BufferLength: u32, PreviousState: *mut c_void, ReturnLength: *mut u32) -> BOOL;
type FnOpenSCManagerW = unsafe extern "system" fn(lpMachineName: LPCWSTR, lpDatabaseName: LPCWSTR, dwDesiredAccess: u32) -> HANDLE;
type FnOpenServiceW = unsafe extern "system" fn(hSCManager: HANDLE, lpServiceName: LPCWSTR, dwDesiredAccess: u32) -> HANDLE;
type FnStartServiceW = unsafe extern "system" fn(hService: HANDLE, dwNumServiceArgs: u32, lpServiceArgVectors: *const *const u16) -> BOOL;
type FnCloseServiceHandle = unsafe extern "system" fn(hSCObject: HANDLE) -> BOOL;
type FnQueryServiceStatusEx = unsafe extern "system" fn(hService: HANDLE, InfoLevel: u32, lpBuffer: *mut u8, cbBufSize: u32, pcbBytesNeeded: *mut u32) -> BOOL;
type FnGetUserNameW = unsafe extern "system" fn(lpBuffer: *mut u16, pcbBuffer: *mut u32) -> BOOL;

    // user32.dll
    type FnGetForegroundWindow = unsafe extern "system" fn() -> HWND;
    type FnSetForegroundWindow = unsafe extern "system" fn(hWnd: HWND) -> BOOL;
    type FnGetWindowThreadProcessId = unsafe extern "system" fn(hWnd: HWND, lpdwProcessId: *mut u32) -> u32;
    type FnAttachThreadInput = unsafe extern "system" fn(idAttach: u32, idAttachTo: u32, fAttach: BOOL) -> BOOL;
    type FnBringWindowToTop = unsafe extern "system" fn(hWnd: HWND) -> BOOL;
    type FnFlashWindowEx = unsafe extern "system" fn(pfwi: *const c_void) -> BOOL;
    type FnDestroyWindow = unsafe extern "system" fn(hWnd: HWND) -> BOOL;
    type FnMessageBoxW = unsafe extern "system" fn(hWnd: HWND, lpText: LPCWSTR, lpCaption: LPCWSTR, uType: u32) -> i32;
    // kernel32.dll
    type FnGetCurrentThreadId = unsafe extern "system" fn() -> u32;
    // comctl32.dll
    type FnInitCommonControlsEx = unsafe extern "system" fn(picce: *const c_void) -> BOOL;

    pub struct WinApi {
        pub LoadLibraryA: Option<FnLoadLibraryA>,
        pub GetProcAddress: Option<FnGetProcAddress>,
        pub GetForegroundWindow: Option<FnGetForegroundWindow>,
        pub SetForegroundWindow: Option<FnSetForegroundWindow>,
        pub GetWindowThreadProcessId: Option<FnGetWindowThreadProcessId>,
        pub AttachThreadInput: Option<FnAttachThreadInput>,
        pub BringWindowToTop: Option<FnBringWindowToTop>,
        pub GetCurrentThreadId: Option<FnGetCurrentThreadId>,
        pub InitCommonControlsEx: Option<FnInitCommonControlsEx>,
        pub FlashWindowEx: Option<FnFlashWindowEx>,
        pub DestroyWindow: Option<FnDestroyWindow>,
        pub MessageBoxW: Option<FnMessageBoxW>,
        pub CreateFileW: Option<FnCreateFileW>,
        pub CloseHandle: Option<FnCloseHandle>,
        pub DeviceIoControl: Option<FnDeviceIoControl>,
        pub GetCompressedFileSizeW: Option<FnGetCompressedFileSizeW>,
        pub GetFileAttributesW: Option<FnGetFileAttributesW>,
        pub SetFileAttributesW: Option<FnSetFileAttributesW>,
        pub OpenProcess: Option<FnOpenProcess>,
        pub CreateProcessW: Option<FnCreateProcessW>,
        pub InitializeProcThreadAttributeList: Option<FnInitializeProcThreadAttributeList>,
        pub UpdateProcThreadAttribute: Option<FnUpdateProcThreadAttribute>,
        pub DeleteProcThreadAttributeList: Option<FnDeleteProcThreadAttributeList>,
        // Advapi32
        pub OpenProcessToken: Option<FnOpenProcessToken>,
        pub LookupPrivilegeValueW: Option<FnLookupPrivilegeValueW>,
        pub AdjustTokenPrivileges: Option<FnAdjustTokenPrivileges>,
        pub OpenSCManagerW: Option<FnOpenSCManagerW>,
        pub OpenServiceW: Option<FnOpenServiceW>,
        pub StartServiceW: Option<FnStartServiceW>,
        pub CloseServiceHandle: Option<FnCloseServiceHandle>,
        pub QueryServiceStatusEx: Option<FnQueryServiceStatusEx>,
        pub GetUserNameW: Option<FnGetUserNameW>,
    }

    impl WinApi {
        pub unsafe fn get() -> &'static WinApi {
            unsafe { &GLOBAL_API }
        }
    }

    static mut GLOBAL_API: WinApi = WinApi {
        LoadLibraryA: None,
        GetProcAddress: None,
        GetForegroundWindow: None,
        SetForegroundWindow: None,
        GetWindowThreadProcessId: None,
        AttachThreadInput: None,
        BringWindowToTop: None,
        GetCurrentThreadId: None,
        InitCommonControlsEx: None,
        FlashWindowEx: None,
        DestroyWindow: None,
        MessageBoxW: None,
        CreateFileW: None,
        CloseHandle: None,
        DeviceIoControl: None,
        GetCompressedFileSizeW: None,
        GetFileAttributesW: None,
        SetFileAttributesW: None,
        OpenProcess: None,
        CreateProcessW: None,
        InitializeProcThreadAttributeList: None,
        UpdateProcThreadAttribute: None,
        DeleteProcThreadAttributeList: None,
        OpenProcessToken: None,
        LookupPrivilegeValueW: None,
        AdjustTokenPrivileges: None,
        OpenSCManagerW: None,
        OpenServiceW: None,
        StartServiceW: None,
        CloseServiceHandle: None,
        QueryServiceStatusEx: None,
        GetUserNameW: None,
    };


    pub unsafe fn init() -> bool {
        let kernel32 = get_kernel32_base();
        if kernel32.is_null() { return false; }

        // Resolve LoadLibraryA
        let hash_lla = hash_str("LoadLibraryA");
        let addr_lla = get_proc_address(kernel32, hash_lla);
        if addr_lla.is_null() { return false; }
        GLOBAL_API.LoadLibraryA = Some(core::mem::transmute(addr_lla));

        // Resolve GetProcAddress
        let hash_gpa = hash_str("GetProcAddress");
        let addr_gpa = get_proc_address(kernel32, hash_gpa);
        if addr_gpa.is_null() { return false; }
        GLOBAL_API.GetProcAddress = Some(core::mem::transmute(addr_gpa));
        
        // Helper to load other functions
        let load_lib = GLOBAL_API.LoadLibraryA.unwrap();
        let get_addr = GLOBAL_API.GetProcAddress.unwrap();

        // Resolve Kernel32 functions
        GLOBAL_API.GetCurrentThreadId = Some(core::mem::transmute(get_addr(kernel32 as HMODULE, "GetCurrentThreadId\0".as_ptr())));
        GLOBAL_API.CreateFileW = Some(core::mem::transmute(get_addr(kernel32 as HMODULE, "CreateFileW\0".as_ptr())));
        GLOBAL_API.CloseHandle = Some(core::mem::transmute(get_addr(kernel32 as HMODULE, "CloseHandle\0".as_ptr())));
        GLOBAL_API.DeviceIoControl = Some(core::mem::transmute(get_addr(kernel32 as HMODULE, "DeviceIoControl\0".as_ptr())));

        // Load User32
        let user32_name = "USER32.DLL\0";
        let user32 = load_lib(user32_name.as_ptr());
        if !user32.is_null() {
             GLOBAL_API.GetForegroundWindow = Some(core::mem::transmute(get_addr(user32, "GetForegroundWindow\0".as_ptr())));
             GLOBAL_API.SetForegroundWindow = Some(core::mem::transmute(get_addr(user32, "SetForegroundWindow\0".as_ptr())));
             GLOBAL_API.GetWindowThreadProcessId = Some(core::mem::transmute(get_addr(user32, "GetWindowThreadProcessId\0".as_ptr())));
             GLOBAL_API.AttachThreadInput = Some(core::mem::transmute(get_addr(user32, "AttachThreadInput\0".as_ptr())));
             GLOBAL_API.BringWindowToTop = Some(core::mem::transmute(get_addr(user32, "BringWindowToTop\0".as_ptr())));
             GLOBAL_API.FlashWindowEx = Some(core::mem::transmute(get_addr(user32, "FlashWindowEx\0".as_ptr())));
             GLOBAL_API.DestroyWindow = Some(core::mem::transmute(get_addr(user32, "DestroyWindow\0".as_ptr())));
             GLOBAL_API.MessageBoxW = Some(core::mem::transmute(get_addr(user32, "MessageBoxW\0".as_ptr())));
        }

        // Load ComCtl32
        let comctl_name = "COMCTL32.DLL\0";
        let comctl = load_lib(comctl_name.as_ptr());
         if !comctl.is_null() {
             GLOBAL_API.InitCommonControlsEx = Some(core::mem::transmute(get_addr(comctl, "InitCommonControlsEx\0".as_ptr())));
         }


        GLOBAL_API.GetCompressedFileSizeW = Some(core::mem::transmute(get_addr(kernel32 as HMODULE, "GetCompressedFileSizeW\0".as_ptr())));
        GLOBAL_API.GetFileAttributesW = Some(core::mem::transmute(get_addr(kernel32 as HMODULE, "GetFileAttributesW\0".as_ptr())));
        GLOBAL_API.SetFileAttributesW = Some(core::mem::transmute(get_addr(kernel32 as HMODULE, "SetFileAttributesW\0".as_ptr())));
        GLOBAL_API.OpenProcess = Some(core::mem::transmute(get_addr(kernel32 as HMODULE, "OpenProcess\0".as_ptr())));
        GLOBAL_API.CreateProcessW = Some(core::mem::transmute(get_addr(kernel32 as HMODULE, "CreateProcessW\0".as_ptr())));
        GLOBAL_API.InitializeProcThreadAttributeList = Some(core::mem::transmute(get_addr(kernel32 as HMODULE, "InitializeProcThreadAttributeList\0".as_ptr())));
        GLOBAL_API.UpdateProcThreadAttribute = Some(core::mem::transmute(get_addr(kernel32 as HMODULE, "UpdateProcThreadAttribute\0".as_ptr())));
        GLOBAL_API.DeleteProcThreadAttributeList = Some(core::mem::transmute(get_addr(kernel32 as HMODULE, "DeleteProcThreadAttributeList\0".as_ptr())));

        // Load Advapi32
        let advapi_name = "ADVAPI32.DLL\0";
        let advapi = load_lib(advapi_name.as_ptr());
        if !advapi.is_null() {
             GLOBAL_API.OpenProcessToken = Some(core::mem::transmute(get_addr(advapi, "OpenProcessToken\0".as_ptr())));
             GLOBAL_API.LookupPrivilegeValueW = Some(core::mem::transmute(get_addr(advapi, "LookupPrivilegeValueW\0".as_ptr())));
             GLOBAL_API.AdjustTokenPrivileges = Some(core::mem::transmute(get_addr(advapi, "AdjustTokenPrivileges\0".as_ptr())));
             GLOBAL_API.OpenSCManagerW = Some(core::mem::transmute(get_addr(advapi, "OpenSCManagerW\0".as_ptr())));
             GLOBAL_API.OpenServiceW = Some(core::mem::transmute(get_addr(advapi, "OpenServiceW\0".as_ptr())));
             GLOBAL_API.StartServiceW = Some(core::mem::transmute(get_addr(advapi, "StartServiceW\0".as_ptr())));
             GLOBAL_API.CloseServiceHandle = Some(core::mem::transmute(get_addr(advapi, "CloseServiceHandle\0".as_ptr())));
             GLOBAL_API.QueryServiceStatusEx = Some(core::mem::transmute(get_addr(advapi, "QueryServiceStatusEx\0".as_ptr())));
             GLOBAL_API.GetUserNameW = Some(core::mem::transmute(get_addr(advapi, "GetUserNameW\0".as_ptr())));
        }

        true
    }
