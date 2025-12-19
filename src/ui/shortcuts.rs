#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::builder::ControlBuilder;
use crate::ui::utils::get_window_state;
use crate::utils::to_wstring;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, LoadCursorW, RegisterClassW,
    CS_HREDRAW, CS_VREDRAW, IDC_ARROW, WM_DESTROY, WNDCLASSW,
    WS_VISIBLE, WM_CREATE,
    WS_CAPTION, WS_SYSMENU, WS_POPUP,
    PostQuitMessage, WM_CLOSE, DestroyWindow,
    SetWindowLongPtrW, GWLP_USERDATA,
    GetWindowRect, CREATESTRUCTW,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontW, FW_BOLD, FW_NORMAL, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
    DEFAULT_PITCH, FF_DONTCARE, HFONT,
};

const SHORTCUTS_TITLE: &str = "Keyboard Shortcuts";

struct ShortcutsState {
    is_dark: bool,
}

pub unsafe fn show_shortcuts_modal(parent: HWND, is_dark: bool) {
    let instance = GetModuleHandleW(std::ptr::null());
    let class_name = to_wstring("CompactRS_Shortcuts");

    // Check if window already exists
    let existing_hwnd = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(class_name.as_ptr(), std::ptr::null());
    if existing_hwnd != std::ptr::null_mut() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SetForegroundWindow, SW_RESTORE};
        ShowWindow(existing_hwnd, SW_RESTORE);
        SetForegroundWindow(existing_hwnd);
        return;
    }
    let title = to_wstring(SHORTCUTS_TITLE);

    // Load App Icon
    let icon = crate::ui::utils::load_app_icon(instance);

    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(shortcuts_wnd_proc),
        hInstance: instance,
        hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
        lpszClassName: class_name.as_ptr(),
        hIcon: icon,
        hbrBackground: crate::ui::theme::get_background_brush(is_dark),
        cbClsExtra: 0,
        cbWndExtra: 0,
        lpszMenuName: std::ptr::null(),
    };
    RegisterClassW(&wc);

    // Calculate center
    let mut rect: RECT = std::mem::zeroed();
    GetWindowRect(parent, &mut rect);
    let p_width = rect.right - rect.left;
    let p_height = rect.bottom - rect.top;
    let width = 500;
    let height = 320;
    let x = rect.left + (p_width - width) / 2;
    let y = rect.top + (p_height - height) / 2;

    let mut state = ShortcutsState { is_dark };

    let _hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
        x, y, width, height,
        parent,
        std::ptr::null_mut(),
        instance,
        &mut state as *mut _ as *mut std::ffi::c_void,
    );

    crate::ui::utils::run_message_loop();
}

unsafe extern "system" fn shortcuts_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let get_state = || get_window_state::<ShortcutsState>(hwnd);

    if let Some(st) = get_state() {
        if let Some(result) = crate::ui::theme::handle_standard_colors(hwnd, msg, wparam, st.is_dark) {
            return result;
        }
    }

    match msg {
        WM_CREATE => {
            let createstruct = &*(lparam as *const CREATESTRUCTW);
            let state_ptr = createstruct.lpCreateParams as *mut ShortcutsState;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);

            let is_dark_mode = if let Some(st) = state_ptr.as_ref() { st.is_dark } else { false };
            crate::ui::theme::set_window_frame_theme(hwnd, is_dark_mode);

            // Fonts
            let segoe_ui_var = to_wstring("Segoe UI Variable Display");
            let key_font = CreateFontW(
                -16, 0, 0, 0, FW_BOLD as i32, 0, 0, 0, DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32,
                (DEFAULT_PITCH | FF_DONTCARE) as u32, segoe_ui_var.as_ptr()) as HFONT;

            let desc_font = CreateFontW(
                -16, 0, 0, 0, FW_NORMAL as i32, 0, 0, 0, DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32,
                (DEFAULT_PITCH | FF_DONTCARE) as u32, segoe_ui_var.as_ptr()) as HFONT;

            let shortcuts = [
                ("Ctrl + O", "Add Files"),
                ("Ctrl + Shift + O", "Add Folder"),
                ("Ctrl + V", "Paste Files/Paths from Clipboard"),
                ("Del", "Remove Selected Items"),
                ("Ctrl + A", "Select All"),
                ("Double Click (Path)", "Open File Location"),
                ("Double Click (Algo)", "Cycle Algorithm"),
                ("Double Click (Action)", "Toggle Compress/Decompress"),
            ];

            let start_y = 25;
            let row_h = 32;
            let col1_w = 180;
            let margin = 30;
            
            const SS_RIGHT: u32 = 0x2;

            for (i, (key, desc)) in shortcuts.iter().enumerate() {
                let y = start_y + (i as i32 * row_h);

                // Key Column (Right Aligned)
                ControlBuilder::new(hwnd, 0)
                    .label(false)
                    .text(key)
                    .pos(margin, y)
                    .size(col1_w, 25)
                    .font(key_font)
                    .style(SS_RIGHT)
                    .dark_mode(is_dark_mode)
                    .build();

                // Description Column (Left Aligned)
                ControlBuilder::new(hwnd, 0)
                    .label(false)
                    .text(desc)
                    .pos(margin + col1_w + 20, y)
                    .size(250, 25)
                    .font(desc_font)
                    .dark_mode(is_dark_mode)
                    .build();
            }

            0
        },
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        },
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        },
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
