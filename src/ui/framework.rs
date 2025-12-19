use std::ffi::c_void;
use std::marker::PhantomData;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, HINSTANCE};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::Graphics::Gdi::HBRUSH;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetWindowLongPtrW, LoadCursorW, RegisterClassW, SetWindowLongPtrW,
    CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, IDC_ARROW, WM_CREATE, WM_NCCREATE, WNDCLASSW,
    CREATESTRUCTW, HICON, LoadImageW, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED,
};
use crate::utils::to_wstring;

/// Trait to encapsulate window logic.
/// Implement this for the state struct that drives the window.
pub trait WindowHandler: Sized {
    /// Called when the window receives WM_CREATE.
    /// Use this to initialize control creation.
    /// Return 0 to continue creation, -1 to abort.
    fn on_create(&mut self, _hwnd: HWND) -> LRESULT {
        0
    }

    /// Main message handler.
    /// Return Some(result) if you handled the message.
    /// Return None to let the default window procedure (DefWindowProc) handle it.
    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT>;
}

/// Generic Window wrapper.
/// T is the state struct that implements WindowHandler.
pub struct Window<T: WindowHandler> {
    _marker: PhantomData<T>,
}

impl<T: WindowHandler> Window<T> {
    /// Creates a new window.
    /// 
    /// # Arguments
    /// * `state` - Mutable reference to the state object. Must live as long as the window.
    /// * `class_name` - Unique class name for the window.
    /// * `title` - Window title.
    /// * `style` - Window styles (e.g., WS_OVERLAPPEDWINDOW).
    /// * `ex_style` - Extended window styles.
    /// * `x`, `y`, `width`, `height` - Position and size.
    /// * `parent` - Parent window handle (can be 0/null).
    /// * `icon` - Handle to the icon (can be 0).
    /// * `background` - Handle to the background brush (can be 0).
    pub unsafe fn create(
        state: &mut T,
        class_name: &str,
        title: &str,
        style: u32,
        ex_style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: HWND,
        icon: HICON,
        background: HBRUSH,
    ) -> Result<HWND, String> {
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        let class_name_w = to_wstring(class_name);
        let title_w = to_wstring(title);

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc::<T>),
            hInstance: instance,
            hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
            hIcon: icon,
            lpszClassName: class_name_w.as_ptr(),
            hbrBackground: background,
            cbClsExtra: 0,
            cbWndExtra: 0,
            lpszMenuName: std::ptr::null(),
        };

        // Attempt to register class. Ignore error as it might already be registered.
        unsafe { RegisterClassW(&wc) };

        let hwnd = unsafe { CreateWindowExW(
            ex_style,
            class_name_w.as_ptr(),
            title_w.as_ptr(),
            style,
            x, y, width, height,
            parent,
            std::ptr::null_mut(),
            instance,
            state as *mut T as *mut c_void,
        ) };

        if hwnd == std::ptr::null_mut() {
            Err("CreateWindowExW failed".to_string())
        } else {
            Ok(hwnd)
        }
    }
}

/// Generic Static Window Procedure.
/// Handles bootstrapping the generic state T from `GWLP_USERDATA`.
unsafe extern "system" fn wnd_proc<T: WindowHandler>(
    hwnd: HWND, 
    msg: u32, 
    wparam: WPARAM, 
    lparam: LPARAM
) -> LRESULT {
    // 1. Handle WM_NCCREATE to setup state
    if msg == WM_NCCREATE {
        let createstruct = unsafe { &*(lparam as *const CREATESTRUCTW) };
        let state_ptr = createstruct.lpCreateParams as *mut T;
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize) };
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }

    // 2. Retrieve state
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut T };
    
    // 3. Delegate to state if valid
    if !ptr.is_null() {
        let state = unsafe { &mut *ptr };
        match msg {
            WM_CREATE => {
                // Call on_create trait method
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                     state.on_create(hwnd)
                }));
                match result {
                    Ok(res) => {
                        if res == -1 { return -1; }
                        return 0; 
                    },
                    Err(_) => {
                        eprintln!("Panic in on_create!");
                        return -1;
                    }
                }
            },
            _ => {
                // Delegate to on_message
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    state.on_message(hwnd, msg, wparam, lparam)
                }));
                
                match result {
                    Ok(res) => {
                        if let Some(r) = res {
                            return r;
                        }
                    },
                    Err(_) => {
                        eprintln!("Panic in on_message: msg={}", msg);
                        // Fallthrough to DefWindowProc might be dangerous if state is corrupted,
                        // but better than immediate abort.
                    }
                }
            }
        }
    }

    // 4. Default processing
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Safely retrieves a mutable reference to window state from GWLP_USERDATA.
#[inline]
pub unsafe fn get_window_state<'a, T>(hwnd: HWND) -> Option<&'a mut T> { unsafe {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if ptr == 0 {
        None
    } else {
        Some(&mut *(ptr as *mut T))
    }
}}

/// Loads the application icon from resources.
#[inline]
pub unsafe fn load_app_icon(instance: HINSTANCE) -> HICON { unsafe {
    LoadImageW(
        instance,
        // Helper: Convert integer resource ID (1) to *const u16 using MAKEINTRESOURCE logic
        // But since we can't use MAKEINTRESOURCE macro directly easily, we just cast 1 to pointer
        1 as *const u16, 
        IMAGE_ICON,
        0, 0,
        LR_DEFAULTSIZE | LR_SHARED,
    )
}}

/// Runs the standard Windows message loop.
/// 
/// Application modal windows often restart a message loop to block the caller
/// until the window is closed. This helper consolidates that logic.
/// 
/// # Safety
/// This function calls unsafe Win32 APIs.
pub unsafe fn run_message_loop() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetMessageW, TranslateMessage, DispatchMessageW, MSG
    };
    
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    // Crucial: Check strictly > 0. GetMessage returns -1 on error!
    // We can filter for specific messages if we want, but usually 0,0 is all.
    while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
