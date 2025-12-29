#![allow(unsafe_op_in_unsafe_fn)]

use crate::types::*;
use crate::ui::state::AppState;
use crate::ui::handlers;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum InputAction {
    None,
    SelectAll,      // Ctrl + A
    Copy,           // Ctrl + C (Not used yet, but good to have)
    Paste,          // Ctrl + V
    Delete,         // Delete
    OpenFiles,      // Ctrl + O
    OpenFolder,     // Ctrl + Shift + O
}

pub unsafe fn resolve_key_action(vk: i32) -> InputAction {
    unsafe {
        let ctrl = (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
        let shift = (GetKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
        
        match vk {
            0x41 if ctrl => InputAction::SelectAll, // 'A'
            0x43 if ctrl => InputAction::Copy,      // 'C'
            0x56 if ctrl => InputAction::Paste,     // 'V'
            0x4F if ctrl && shift => InputAction::OpenFolder, // Ctrl+Shift+O
            0x4F if ctrl => InputAction::OpenFiles, // Ctrl+O
            start if start == VK_DELETE as i32 => InputAction::Delete,
            _ => InputAction::None,
        }
    }
}

/// Handles global shortcuts specific to the main window state.
/// Returns true if the action was handled.
pub unsafe fn handle_global_shortcut(hwnd: HWND, st: &mut AppState, action: InputAction, source_hwnd: HWND) -> bool {
    let _ = hwnd; // Unused for now, but keeping for API consistency
    unsafe {
        match action {
            InputAction::Paste => {
                // If source is Search Input, let it handle natively?
                // Standard Edit controls handle Ctrl+V natively.
                // But we are here because subclass forwarded it.
                // If we forward back, we might loop or need to call CallWindowProc.
                
                // However, our Paste action is "Add from Clipboard" (Global Feature).
                // Does the user want Ctrl+V in Search Box to paste text or add files?
                // Convention: Focused input -> Paste Text. Focused List/Window -> Global Paste.
                
                if let Some(ctrls) = &st.controls {
                    if source_hwnd == ctrls.search_panel.search_hwnd() {
                        SendMessageW(source_hwnd, 0x0302, 0, 0 as isize); // WM_PASTE
                        return true;
                    }
                }
                
                // Default global action
                handlers::process_clipboard(hwnd, st);
                true
            },
            InputAction::OpenFiles => {
                handlers::on_add_files(st);
                true
            },
            InputAction::OpenFolder => {
                handlers::on_add_folder(st);
                true
            },
            InputAction::Delete => {
                 // Check context?
                 // Delete in Search Box -> Delete character?
                 // Handled natively usually. Subclass might have swallowed it?
                 
                 if let Some(ctrls) = &st.controls {
                    if source_hwnd == ctrls.search_panel.search_hwnd() {
                        // Let the edit control handle Delete natively.
                        // How? We shouldn't have consumed it if it was for Edit.
                        // But input::resolve_key_action maps VK_DELETE -> Delete.
                        // If we return FALSE, does caller forward to DefWindowProc?
                        
                        // Current architecture: Subclass calls forward_shortcut... if returns true -> consume(return 0).
                        // If we return TRUE here, subclass consumes it.
                        // So for Edit control, we want to return FALSE here so subclass proceeds to DefSubclassProc?
                        // BUT subclass.rs: `if matches!(action...SelectAll|Paste|Delete) { forward... return 0 }`
                        // It unconditionally consumes if we mapped it.
                        
                        // Fix: We must execute the native action here manually or tell subclass to not consume.
                        // Executing native Delete: WM_KEYDOWN/WM_CHAR?
                        // Better: Send WM_CLEAR (0x0303) or simulate key?
                        // WM_CLEAR deletes selection.
                        
                        // Ideally: For Delete/Copy/Paste in Edit controls, we should probably NOT forward them as Global Actions,
                        // unless we want to override them.
                        
                        // User wants "Global Paste" (Add Files) vs "Local Paste" (Text).
                        
                        // Let's implement Local Paste (done above).
                        
                        // Local Delete:
                        SendMessageW(source_hwnd, 0x0100, 0x2E, 0); // VK_DELETE - Re-injecting might loop?
                        // No, direct SendMessage to control Proc.
                        // But wait, standard Edit control handles VK_DELETE in WM_KEYDOWN.
                        
                        // Simply returning FALSE here isn't enough because subclass consumes.
                        // We must change logic:
                        // If it's an Edit control and action is Delete, we should perhaps return TRUE but do nothing? 
                        // No, then nothing happens.
                        
                        // We need the Subclass to NOT consume it.
                        // But Subclass doesn't know context. Main Window does.
                        
                        // Alternative: We manually perform the Edit action.
                         SendMessageW(source_hwnd, 0x0303, 0, 0 as isize); // WM_CLEAR
                         return true;
                    }
                 }
                 
                handlers::on_remove_selected(st);
                true
            },
            InputAction::SelectAll => {
                 if let Some(ctrls) = &st.controls {
                     // Context: Search Input
                     if source_hwnd == ctrls.search_panel.search_hwnd() {
                         SendMessageW(source_hwnd, 0x00B1, 0, -1 as isize); // EM_SETSEL
                         return true;
                     }
                     
                     // Helper to check for Edit class
                     unsafe fn is_edit_control(hwnd: HWND) -> bool {
                         let mut class_name = [0u16; 16];
                         let len = GetClassNameW(hwnd, class_name.as_mut_ptr(), 16);
                         if len > 0 {
                             let name = String::from_utf16_lossy(&class_name[..len as usize]);
                             // "Edit" is standard. "CompactRsSearchInput"? No, using standard Edit.
                             name.eq_ignore_ascii_case("Edit")
                         } else {
                             false
                         }
                     }

                     if is_edit_control(source_hwnd) {
                         SendMessageW(source_hwnd, 0x00B1, 0, -1 as isize); // EM_SETSEL
                         return true;
                     }

                     // Context: File List (default or specific)
                     // If source is file list OR main window (global)
                     if source_hwnd == ctrls.file_list.hwnd() || source_hwnd == hwnd {
                         let count = ctrls.file_list.get_item_count();
                         for i in 0..count {
                             ctrls.file_list.set_selected(i, true);
                         }
                         return true;
                     }
                 }
                 true
            },
            _ => false,
        }
    }
}

/// Forwards the shortcut key to the parent window using WM_APP_SHORTCUT.
/// Returns true if successful (parent exists), false otherwise.
pub unsafe fn forward_shortcut_to_parent(hwnd: HWND, wparam: usize) -> bool {
    let parent = GetParent(hwnd);
    if !parent.is_null() {
        SendMessageW(parent, WM_APP_SHORTCUT, wparam, hwnd as isize);
        return true;
    }
    false
}

/// Centralized handler for subclass switching logic.
/// Checks if the message is a shortcut key and forwards it to the parent if so.
/// Returns true if the message was consumed/handled.
pub unsafe fn handle_subclass_dispatch(hwnd: HWND, msg: u32, wparam: usize) -> bool {
    if msg == crate::types::WM_KEYDOWN {
        let action = resolve_key_action(wparam as i32);
        match action {
            InputAction::SelectAll | InputAction::Paste | InputAction::Delete | InputAction::OpenFiles | InputAction::OpenFolder => {
                return forward_shortcut_to_parent(hwnd, wparam);
            },
            _ => {}
        }
    }
    false
}
