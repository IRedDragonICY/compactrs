#![allow(unsafe_op_in_unsafe_fn)]

use crate::types::*;
use crate::w;
// use crate::ui::theme::{SetPropW, GetPropW, RemovePropW}; // Removed bad import

// Property name to store the original WndProc
// Property name literal moved to usage to avoid const issues
// const PROP_OLD_PROC: *const u16 = w!("CompactRs_OldEditProc").as_ptr();

/// Subclass an Edit control to handle Ctrl+Backspace
pub unsafe fn subclass_edit(hwnd: HWND) {
    let old_proc = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
    if old_proc != 0 {
        let prop_name = w!("CompactRs_OldEditProc");
        SetPropW(hwnd, prop_name.as_ptr(), old_proc as *mut std::ffi::c_void);
        SetWindowLongPtrW(hwnd, GWLP_WNDPROC, edit_subclass_proc as *const () as isize);
    }
}

unsafe extern "system" fn edit_subclass_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let prop_name = w!("CompactRs_OldEditProc");
    let old_proc = GetPropW(hwnd, prop_name.as_ptr()) as isize;
    
    if msg == WM_DESTROY {
        // Restore original proc (optional, but good hygiene)
        if old_proc != 0 {
            SetWindowLongPtrW(hwnd, GWLP_WNDPROC, old_proc);
            RemovePropW(hwnd, prop_name.as_ptr());
        }
        return CallWindowProcW(Some(std::mem::transmute(old_proc)), hwnd, msg, wparam, lparam);
    }

    if msg == WM_CHAR {
        // Ctrl+Backspace produces 0x7F (127)
        if wparam == 0x7F {
             // Handle Ctrl+Backspace
             if handle_ctrl_backspace(hwnd) {
                 return 0; // Handled
             }
        }
    }

    if old_proc != 0 {
        CallWindowProcW(Some(std::mem::transmute(old_proc)), hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

unsafe fn handle_ctrl_backspace(hwnd: HWND) -> bool {
    let mut start: u32 = 0;
    let mut end: u32 = 0;
    
    // Get current selection
    SendMessageW(hwnd, EM_GETSEL, &mut start as *mut _ as usize, &mut end as *mut _ as isize);
    
    // If selection is empty (caret only), finding previous word
    // If selection exists, standard backspace behavior deletes selection (default handler handles this).
    // Ctrl+Backspace usually deletes selection if exists, OR deletes word left.
    // Let's mimic standard: if selection, delete it. If not, delete word left.
    // Actually, default Edit control DOES handle selection delete on Backspace/Ctrl+Backspace char?
    // Standard Backspace (0x08) deletes selection.
    // Ctrl+Backspace (0x7F) usually does nothing in raw Edit.
    
    if start != end {
        // Selection exists, just replace with empty.
        let empty = w!("");
        SendMessageW(hwnd, EM_REPLACESEL, 1, empty.as_ptr() as isize);
        return true;
    }
    
    if start == 0 {
        return false; // Nothing to delete
    }

    // We need text to find word boundary.
    // This is expensive for large text, but for search/settings inputs it's fine.
    let text = crate::ui::wrappers::get_window_text(hwnd);
    let chars: Vec<u16> = text.encode_utf16().collect();
    
    // Find split point
    let mut pos = start as usize;
    if pos > chars.len() { pos = chars.len(); } // Safety
    
    let _slice = &chars[0..pos];
    
    // Logic: 
    // 1. Skip trailing whitespace
    // 2. Skip non-whitespace (word)
    // 3. That's our new start.
    
    let mut new_pos = pos;
    
    // 1. Go back over potential whitespace (if any)?
    // VSCode: "foo bar  |" -> "foo bar |" (deletes space?) or "foo |"
    // Standard: deletes word left of cursor.
    // "abc def|" -> "abc |"
    // "abc   def|" -> "abc   |"
    // "abc   |" -> "abc |" (deletes spaces)
    
    // Implementation:
    // Iterate backwards.
    // If we are at space, consume spaces until non-space.
    // Then consume non-spaces until space or start.
    
    // Let's treat it as: Delete until next word boundary.
    
    let is_space = |c: u16| c == 0x0020 || c == 0x0009 || c == 0x000A || c == 0x000D;
    
    // If currently at valid char, consume valid chars.
    // If currently at space, consume spaces.
    // Wait, standard Ctrl+Back behavior:
    // "Word|" -> "|"
    // "Word |" -> "Word|" 
    // "Word  |" -> "Word |" (Deletes 1 space?) Or all spaces?
    // Notepad: 
    // "Test Test|" -> "Test |" (Deletes word)
    // "Test   |" -> "Test|" (Deletes spaces)
    // "Test   Test|" -> "Test   |" (Deletes word)
    
    // Let's refine:
    // 1. If we are just after word chars, delete back to start of word.
    // 2. If we are just after whitespace, delete back to end of word.
    
    // Simple algo:
    // If char before is space, delete contiguous spaces.
    // If char before is non-space, delete contiguous non-spaces.
    
    if new_pos > 0 && is_space(chars[new_pos-1]) {
        // Consumption 1: Spaces
        while new_pos > 0 && is_space(chars[new_pos-1]) {
            new_pos -= 1;
        }
    } else {
        // Consumption 2: Word
        // Also handling punctuation? Win32 usually stops at punctuation?
        // Let's assume non-whitespace is "Word".
        while new_pos > 0 && !is_space(chars[new_pos-1]) {
             // Optional: Stop at punctuation? "foo-bar" -> "foo-"? 
             // Let's simpler: consume until whitespace.
             new_pos -= 1;
        }
    }
    
    // Perform Delete
    // Select range [new_pos, start]
    SendMessageW(hwnd, EM_SETSEL, new_pos, start as isize);
    let empty = w!("");
    SendMessageW(hwnd, EM_REPLACESEL, 1, empty.as_ptr() as isize);
    
    true
}
