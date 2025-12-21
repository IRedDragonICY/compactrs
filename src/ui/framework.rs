#![allow(unsafe_op_in_unsafe_fn)]
use std::ffi::c_void;
use std::marker::PhantomData;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, HINSTANCE, RECT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::Graphics::Gdi::HBRUSH;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetWindowLongPtrW, LoadCursorW, RegisterClassW, SetWindowLongPtrW,
    CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, IDC_ARROW, WM_CREATE, WM_NCCREATE, WNDCLASSW,
    CREATESTRUCTW, HICON, LoadImageW, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED,
    GetWindowRect, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
    WM_CLOSE, WM_DESTROY, DestroyWindow, PostQuitMessage,
    MSG, GetMessageW, TranslateMessage, DispatchMessageW,
    WS_VISIBLE, WS_OVERLAPPEDWINDOW, IsDialogMessageW,
};
use crate::utils::to_wstring;

/// Defines how the window should be positioned.
pub enum WindowAlignment {
    CenterOnParent,
    CenterOnScreen,
    Manual(i32, i32),
}

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
    /// Return None to let the framework handle default behavior (Theme, Close, Destroy) or DefWindowProc.
    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT>;

    /// Indicates if this window is modal.
    /// If true, the default WM_DESTROY handler will call PostQuitMessage(0).
    /// Default: true.
    fn is_modal(&self) -> bool {
        true
    }

    /// Indicates if the window should use dark mode for the default theme handler.
    /// Default: false.
    fn is_dark_mode(&self) -> bool {
        false
    }
}

/// Builder for creating windows.
pub struct WindowBuilder<'a, T: WindowHandler> {
    state: &'a mut T,
    class_name: String,
    title: String,
    style: u32,
    ex_style: u32,
    width: i32,
    height: i32,
    alignment: WindowAlignment,
    icon: Option<HICON>,
    background: Option<HBRUSH>,
}

impl<'a, T: WindowHandler> WindowBuilder<'a, T> {
    pub fn new(state: &'a mut T, class_name: &str, title: &str) -> Self {
        Self {
            state,
            class_name: class_name.to_string(),
            title: title.to_string(),
            style: WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            ex_style: 0,
            width: 800,
            height: 600,
            alignment: WindowAlignment::CenterOnScreen,
            icon: None,
            background: None,
        }
    }

    pub fn style(mut self, style: u32) -> Self {
        self.style = style;
        self
    }

    pub fn ex_style(mut self, ex_style: u32) -> Self {
        self.ex_style = ex_style;
        self
    }

    pub fn size(mut self, width: i32, height: i32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn align(mut self, alignment: WindowAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn icon(mut self, icon: HICON) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn background(mut self, brush: HBRUSH) -> Self {
        self.background = Some(brush);
        self
    }

    /// Builds and creates the window.
    pub unsafe fn build(self, parent: HWND) -> Result<HWND, String> {
        let instance = GetModuleHandleW(std::ptr::null());
        
        // Resolve Icon
        let icon = self.icon.unwrap_or_else(|| load_app_icon(instance));
        
        // Resolve Geometry
        let (x, y) = match self.alignment {
            WindowAlignment::Manual(x, y) => (x, y),
            WindowAlignment::CenterOnScreen => {
                let screen_w = GetSystemMetrics(SM_CXSCREEN);
                let screen_h = GetSystemMetrics(SM_CYSCREEN);
                ((screen_w - self.width) / 2, (screen_h - self.height) / 2)
            },
            WindowAlignment::CenterOnParent => {
                if parent != std::ptr::null_mut() {
                    let mut rect: RECT = std::mem::zeroed();
                    GetWindowRect(parent, &mut rect);
                    let p_width = rect.right - rect.left;
                    let p_height = rect.bottom - rect.top;
                    (rect.left + (p_width - self.width) / 2, rect.top + (p_height - self.height) / 2)
                } else {
                    // Fallback to center screen if no parent
                    let screen_w = GetSystemMetrics(SM_CXSCREEN);
                    let screen_h = GetSystemMetrics(SM_CYSCREEN);
                    ((screen_w - self.width) / 2, (screen_h - self.height) / 2)
                }
            }
        };

        // Resolve Background (if None, use 0/null or default? Let's use 0 to let DefWindowProc or custom paint handle it, unless specified)
        // Actually, existing code used check for is_dark and passed brush. 
        // We will respect what user passed.
        let background_brush = self.background.unwrap_or(std::ptr::null_mut());

        Window::<T>::create_internal(
            self.state,
            &self.class_name,
            &self.title,
            self.style,
            self.ex_style,
            x, y, self.width, self.height,
            parent,
            icon,
            background_brush
        )
    }
}

/// Generic Window wrapper.
pub struct Window<T: WindowHandler> {
    _marker: PhantomData<T>,
}

impl<T: WindowHandler> Window<T> {
    /// Internal create function used by Builder.
    unsafe fn create_internal(
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
        let instance = GetModuleHandleW(std::ptr::null());
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
                        // If handled, return result
                        if let Some(r) = res {
                            return r;
                        }

                        // === Default Framework Handling ===
                        
                        // 1. Theme Handling (WM_CTLCOLOR*)
                        let is_dark = state.is_dark_mode();
                        if let Some(theme_res) = crate::ui::theme::handle_standard_colors(hwnd, msg, wparam, is_dark) {
                            return theme_res;
                        }

                        // 2. Standard Close/Destroy
                        if msg == WM_CLOSE {
                            DestroyWindow(hwnd);
                            return 0;
                        }

                        if msg == WM_DESTROY {
                            // Only PostQuitMessage if modal
                            if state.is_modal() {
                                PostQuitMessage(0);
                            }
                            return 0;
                        }
                    },
                    Err(_) => {
                        eprintln!("Panic in on_message: msg={}", msg);
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
        1 as *const u16, 
        IMAGE_ICON,
        0, 0,
        LR_DEFAULTSIZE | LR_SHARED,
    )
}}

/// Runs the standard Windows message loop.
pub unsafe fn run_message_loop(hwnd: HWND) {
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
        unsafe {
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

/// Helper method to show a modal window.
/// Creates the window using the builder and runs the message loop.
pub unsafe fn show_modal<T: WindowHandler>(
    builder: WindowBuilder<T>,
    parent: HWND
) {
    let hwnd_res = builder.build(parent);
    if let Ok(hwnd) = hwnd_res {
        if hwnd != std::ptr::null_mut() {
            run_message_loop(hwnd);
        }
    }
}
