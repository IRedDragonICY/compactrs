use windows::core::{Result, w, PCWSTR}; // s is unused

// ... (skipping to backdrop)


use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM, BOOL};
use windows::Win32::Graphics::Gdi::{HBRUSH, COLOR_WINDOW};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE, DWM_SYSTEMBACKDROP_TYPE, DWMWINDOWATTRIBUTE};
use windows::Win32::UI::WindowsAndMessaging::{

    CreateWindowExW, DefWindowProcW, LoadCursorW, PostQuitMessage, RegisterClassW, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, SW_SHOW, WM_DESTROY, WNDCLASSW,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE, WM_CREATE, WM_SIZE, WM_COMMAND, SetWindowPos, SWP_NOZORDER,
    GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA, GetDlgItem, WM_DROPFILES, MessageBoxW, MB_OK,
    SendMessageW, CB_ADDSTRING, CB_SETCURSEL, CB_GETCURSEL, SetWindowTextW, WS_CHILD, HMENU, WM_TIMER, SetTimer,
};
use windows::Win32::UI::Shell::{DragQueryFileW, DragFinish, HDROP, FileOpenDialog, IFileOpenDialog, FOS_PICKFOLDERS, FOS_FORCEFILESYSTEM, SIGDN_FILESYSPATH, DragAcceptFiles};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, CoTaskMemFree};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_DWORD, HKEY};
use crate::gui::controls::{create_button, create_listview, create_combobox, create_progress_bar, IDC_LISTVIEW, IDC_BTN_SCAN, IDC_BTN_COMPRESS, IDC_COMBO_ALGO, IDC_BTN_DECOMPRESS, IDC_STATIC_TEXT, IDC_PROGRESS_BAR, IDC_BTN_CANCEL};
use crate::gui::state::{AppState, Controls, UiMessage};
use std::thread;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use crossbeam_channel::Sender;
use windows::Win32::UI::Controls::{PBM_SETRANGE32, PBM_SETPOS};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use crate::engine::wof::{compress_file, uncompress_file, WofAlgorithm, get_real_file_size, is_wof_compressed};
use crate::engine::compresstimate::estimate_size;
use ignore::WalkBuilder;
use std::path::Path;
// Use local helpers for macros
const BN_CLICKED: u16 = 0;

fn lo_word(l: u32) -> u16 {
    (l & 0xffff) as u16
}

fn hi_word(l: u32) -> u16 {
    ((l >> 16) & 0xffff) as u16
}

// Internal modules for handling specific messages if needed
// use crate::engine; 

const WINDOW_CLASS_NAME: PCWSTR = w!("CompactRS_Class");
const WINDOW_TITLE: PCWSTR = w!("CompactRS - Native Compressor");

pub unsafe fn create_main_window(instance: HINSTANCE) -> Result<HWND> {
    unsafe {
        // 1. Register Window Class
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize as *mut _), // Default white background
            lpszClassName: WINDOW_CLASS_NAME,
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        if atom == 0 {
            return Err(windows::core::Error::from_win32());
        }

        // 2. Create Window
        let hwnd = CreateWindowExW(
            Default::default(),
            WINDOW_CLASS_NAME,
            WINDOW_TITLE,
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            800,
            600,
            None,
            None,
            instance,
            None,
        )?; // Propagate error
        
        // 3. Apply Modern Backdrop (Mica)
        apply_backdrop(hwnd);
        
        ShowWindow(hwnd, SW_SHOW);
        
        // 4. Update Theme
        update_theme(hwnd);

        Ok(hwnd)
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        // Helper to get state
        let get_state = || {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr == 0 { None } else { Some(&mut *(ptr as *mut AppState)) }
        };

        match msg {
            WM_CREATE => {
                // Initialize State
                let mut state = Box::new(AppState::new());
                
                // Create Controls (Fonts etc handled implicitly by system or defaults)
                
                // Create Buttons
                let h_scan = create_button(hwnd, windows::core::w!("Scan Folder"), 10, 520, 120, 30, IDC_BTN_SCAN);
                let h_compress = create_button(hwnd, windows::core::w!("Compress"), 140, 520, 120, 30, IDC_BTN_COMPRESS);
                let h_decompress = create_button(hwnd, windows::core::w!("Decompress"), 270, 520, 120, 30, IDC_BTN_DECOMPRESS);
                let h_combo = create_combobox(hwnd, 400, 520, 150, 200, IDC_COMBO_ALGO);
                let h_cancel = create_button(hwnd, windows::core::w!("Cancel"), 560, 520, 80, 30, IDC_BTN_CANCEL);
                EnableWindow(h_cancel, false); // Disabled initially

                // Static Text Dashboard
                let h_static = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    w!("Ready. Select a folder to analyze."),
                    WS_CHILD | WS_VISIBLE,
                    10, 10, 760, 450,
                    hwnd,
                    HMENU(IDC_STATIC_TEXT as isize as *mut _),
                    HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0),
                    None,
                ).unwrap_or_default();
                
                // Progress Bar
                let h_progress = create_progress_bar(hwnd, 10, 470, 760, 30, IDC_PROGRESS_BAR);

                // Populate Combo
                let algos = [w!("XPRESS4K"), w!("XPRESS8K"), w!("XPRESS16K"), w!("LZX")];
                for alg in algos {
                    SendMessageW(h_combo, CB_ADDSTRING, WPARAM(0), LPARAM(alg.as_ptr() as isize));
                }
                SendMessageW(h_combo, CB_SETCURSEL, WPARAM(1), LPARAM(0)); // Default XPRESS8K

                state.controls = Some(Controls {
                    list_view: HWND(std::ptr::null_mut()), // Unused now
                    btn_scan: h_scan,
                    btn_compress: h_compress,
                    btn_decompress: h_decompress,
                    combo_algo: h_combo,
                    static_text: h_static,
                    progress_bar: h_progress,
                    btn_cancel: h_cancel,
                });

                // Store state
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

                // Start Timer
                SetTimer(hwnd, 1, 100, None);

                // Enable Drag and Drop
                DragAcceptFiles(hwnd, true);

                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as u16;
                let code = ((wparam.0 >> 16) & 0xFFFF) as u16;

                match id {
                    IDC_BTN_SCAN => {
                        // 1. Pick Folder
                        if let Ok(folder) = pick_folder() {
                            if let Some(st) = get_state() {
                                st.current_folder = Some(folder.clone());
                                // Update UI
                                if let Some(ctrls) = &st.controls {
                                    SetWindowTextW(ctrls.static_text, w!("Scanning..."));
                                    SendMessageW(ctrls.progress_bar, PBM_SETPOS, WPARAM(0), LPARAM(0));
                                    EnableWindow(ctrls.btn_cancel, true);
                                }
                                
                                // Start Scan
                                let tx = st.tx.clone();
                                let cancel = st.cancel_flag.clone();
                                cancel.store(false, Ordering::Relaxed);
                                
                                start_scan_worker(folder, tx, cancel);
                            }
                        }
                    },
                    IDC_BTN_COMPRESS => {
                            if let Some(st) = get_state() {
                                if let Some(folder) = &st.current_folder {
                                    if let Some(ctrls) = &st.controls {
                                         let idx = SendMessageW(ctrls.combo_algo, CB_GETCURSEL, WPARAM(0), LPARAM(0));
                                         let algo = match idx.0 {
                                             0 => WofAlgorithm::Xpress4K,
                                             2 => WofAlgorithm::Xpress16K,
                                             3 => WofAlgorithm::Lzx,
                                             _ => WofAlgorithm::Xpress8K,
                                         };
                                         
                                         SetWindowTextW(ctrls.static_text, w!("Starting Compression..."));
                                         EnableWindow(ctrls.btn_cancel, true);
                                         
                                         let tx = st.tx.clone();
                                         let cancel = st.cancel_flag.clone();
                                         cancel.store(false, Ordering::Relaxed);
                                         let folder_path = folder.clone();
                                         
                                         thread::spawn(move || {
                                             let mut files = Vec::new();
                                             let _ = tx.send(UiMessage::Status("Collecting files...".to_string()));
                                             for result in WalkBuilder::new(&folder_path).build() {
                                                  if let Ok(entry) = result {
                                                      if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                                                          files.push(entry.path().to_string_lossy().to_string());
                                                      }
                                                  }
                                             }
                                             
                                             let total = files.len() as u64;
                                             let _ = tx.send(UiMessage::Progress(0, total));
                                             
                                             let mut success = 0;
                                             let mut fail = 0;
                                             let mut total_orig: u64 = 0;
                                             let mut total_comp: u64 = 0;
                                             
                                             for (i, file) in files.iter().enumerate() {
                                                  if cancel.load(Ordering::Relaxed) {
                                                        let _ = tx.send(UiMessage::Status("Compression Cancelled.".to_string()));
                                                        let _ = tx.send(UiMessage::Finished);
                                                        return;
                                                  }
                                                  
                                                  // Optional: Send strict progress less frequently
                                                  if i % 10 == 0 {
                                                     let _ = tx.send(UiMessage::Progress(i as u64, total));
                                                     let _ = tx.send(UiMessage::Status(format!("Compressing {}/{}...", i, total)));
                                                  }
                                                  
                                                  // Get logic size before
                                                  let orig_size = std::fs::metadata(file).map(|m| m.len()).unwrap_or(0);

                                                  if let Ok(_) = compress_file(file, algo) {
                                                      success += 1;
                                                      total_orig += orig_size;
                                                      total_comp += get_real_file_size(file);
                                                  } else {
                                                      fail += 1;
                                                  }
                                             }
                                             
                                             use humansize::{format_size, BINARY};
                                             let ratio = if total_orig > 0 {
                                                 100.0 * (1.0 - (total_comp as f64 / total_orig as f64))
                                             } else { 0.0 };

                                             let report = format!("Finished!\nSuccess: {}\nFailed: {}\nOriginal Size: {}\nCompressed Size: {}\nSaved: {:.1}%", 
                                                 success, fail, format_size(total_orig, BINARY), format_size(total_comp, BINARY), ratio);
                                             
                                             let _ = tx.send(UiMessage::Log(report));
                                             let _ = tx.send(UiMessage::Progress(total, total));
                                             let _ = tx.send(UiMessage::Finished);
                                         });
                                    }
                                } else {
                                     MessageBoxW(hwnd, w!("Please select a folder first!"), w!("Error"), MB_OK);
                                }
                            }
                    },
                    IDC_BTN_DECOMPRESS => {
                            if let Some(st) = get_state() {
                                if let Some(folder) = &st.current_folder {
                                     // Similar logic for decompress
                                     if let Some(ctrls) = &st.controls {
                                         SetWindowTextW(ctrls.static_text, w!("Starting Decompression..."));
                                         EnableWindow(ctrls.btn_cancel, true);
                                     }
                                     let tx = st.tx.clone();
                                     let cancel = st.cancel_flag.clone();
                                     cancel.store(false, Ordering::Relaxed);
                                     let folder_path = folder.clone();
                                     
                                     thread::spawn(move || {
                                         let mut files = Vec::new();
                                         let _ = tx.send(UiMessage::Status("Collecting files...".to_string()));
                                          for result in WalkBuilder::new(&folder_path).build() {
                                              if let Ok(entry) = result {
                                                  if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                                                      files.push(entry.path().to_string_lossy().to_string());
                                                  }
                                              }
                                          }
                                          let total = files.len() as u64;
                                          let _ = tx.send(UiMessage::Progress(0, total));
                                          let mut success = 0;
                                          let mut fail = 0;
                                          
                                          for (i, file) in files.iter().enumerate() {
                                               if cancel.load(Ordering::Relaxed) {
                                                    let _ = tx.send(UiMessage::Status("Decompression Cancelled.".to_string()));
                                                    let _ = tx.send(UiMessage::Finished);
                                                    return;
                                               }
                                               if i % 10 == 0 {
                                                  let _ = tx.send(UiMessage::Progress(i as u64, total));
                                                  let _ = tx.send(UiMessage::Status(format!("Decompressing {}/{}...", i, total)));
                                               }
                                               if let Ok(_) = uncompress_file(file) {
                                                   success += 1;
                                               } else {
                                                   fail += 1;
                                               }
                                          }
                                             let report = format!("Decompression Finished!\nSuccess: {}\nFailed: {}", success, fail);
                                             let _ = tx.send(UiMessage::Log(report));
                                             let _ = tx.send(UiMessage::Progress(total, total));
                                             let _ = tx.send(UiMessage::Finished);
                                     });
                                } else {
                                    MessageBoxW(hwnd, w!("Please select a folder first!"), w!("Error"), MB_OK);
                                }
                            }
                    },
                    IDC_BTN_CANCEL => {
                        if let Some(st) = get_state() {
                            st.cancel_flag.store(true, Ordering::Relaxed);
                            if let Some(ctrls) = &st.controls {
                                EnableWindow(ctrls.btn_cancel, false);
                                SetWindowTextW(ctrls.static_text, w!("Stopping..."));
                            }
                        }
                    },
                    _ => {}
                }
                LRESULT(0)
            }
            WM_TIMER => {
                if let Some(st) = get_state() {
                    loop {
                        match st.rx.try_recv() {
                            Ok(msg) => {
                                match msg {
                                    UiMessage::Progress(cur, total) => {
                                        if let Some(ctrls) = &st.controls {
                                            SendMessageW(ctrls.progress_bar, PBM_SETRANGE32, WPARAM(0), LPARAM(total as isize));
                                            SendMessageW(ctrls.progress_bar, PBM_SETPOS, WPARAM(cur as usize), LPARAM(0));
                                        }
                                    },
                                    UiMessage::Status(text) | UiMessage::Log(text) | UiMessage::Error(text) => {
                                        if let Some(ctrls) = &st.controls {
                                            let wstr = windows::core::HSTRING::from(&text);
                                            SetWindowTextW(ctrls.static_text, PCWSTR::from_raw(wstr.as_ptr()));
                                        }
                                    },
                                    UiMessage::Finished => {
                                        if let Some(ctrls) = &st.controls {
                                            EnableWindow(ctrls.btn_cancel, false);
                                        }
                                    },
                                }
                            },
                            Err(_) => break, // Empty
                        }
                    }
                }
                LRESULT(0)
            }
            WM_SIZE => {
                // Retrieve new width and height
                let width = (lparam.0 & 0xFFFF) as i32;
                let height = ((lparam.0 >> 16) & 0xFFFF) as i32;
                
                // Resizing logic (Manual Layout)
                // ListView takes up most space, buttons at bottom
                
                let btn_height = 30;
                let padding = 10;
                let list_height = height - btn_height - (padding * 3);
                
                // Resize ListView
                if let Ok(h_list) = GetDlgItem(hwnd, IDC_LISTVIEW.into()) {
                    if h_list.0 != std::ptr::null_mut() {
                        SetWindowPos(h_list, HWND(std::ptr::null_mut()), padding, padding, width - (padding * 2), list_height, SWP_NOZORDER);
                    }
                }
                // Resize Scan Button (anchored bottom-left)
                if let Ok(h_scan) = GetDlgItem(hwnd, IDC_BTN_SCAN.into()) {
                    if h_scan.0 != std::ptr::null_mut() {
                        SetWindowPos(h_scan, HWND(std::ptr::null_mut()), padding, height - btn_height - padding, 120, btn_height, SWP_NOZORDER);
                    }
                }

                // Resize Compress Button
                if let Ok(h_comp) = GetDlgItem(hwnd, IDC_BTN_COMPRESS.into()) {
                    if h_comp.0 != std::ptr::null_mut() {
                        SetWindowPos(h_comp, HWND(std::ptr::null_mut()), padding + 120 + padding, height - btn_height - padding, 120, btn_height, SWP_NOZORDER);
                    }
                }

                // Resize Decompress Button
                if let Ok(h_decomp) = GetDlgItem(hwnd, IDC_BTN_DECOMPRESS.into()) {
                    if h_decomp.0 != std::ptr::null_mut() {
                        SetWindowPos(h_decomp, HWND(std::ptr::null_mut()), padding + 120 + padding + 120 + padding, height - btn_height - padding, 120, btn_height, SWP_NOZORDER);
                    }
                }
                
                // Resize Combo
                if let Ok(h_combo) = GetDlgItem(hwnd, IDC_COMBO_ALGO.into()) {
                     if h_combo.0 != std::ptr::null_mut() {
                         SetWindowPos(h_combo, HWND(std::ptr::null_mut()), padding + 120 + padding + 120 + padding + 120 + padding, height - btn_height - padding, 150, btn_height, SWP_NOZORDER);
                     }
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                 // Drop state
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                if ptr != 0 {
                    let _ = Box::from_raw(ptr as *mut AppState);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                PostQuitMessage(0);
                LRESULT(0)
            }

            WM_DROPFILES => {
                let hdrop = HDROP(wparam.0 as *mut _);
                let mut buffer = [0u16; 1024];
                // Get the number of files dropped
                let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
                
                if count > 0 {
                    // Just get the first file for now
                    let len = DragQueryFileW(hdrop, 0, Some(&mut buffer));
                    if len > 0 {
                        let path_string = String::from_utf16_lossy(&buffer[..len as usize]);
                        let path = std::path::Path::new(&path_string);
                        
                        let target_folder = if path.is_dir() {
                            Some(path_string.clone())
                        } else {
                            // If it's a file, get parent
                            path.parent().map(|p| p.to_string_lossy().to_string())
                        };

                        if let Some(folder) = target_folder {
                            if let Some(st) = get_state() {
                                st.current_folder = Some(folder.clone());
                                if let Some(ctrls) = &st.controls {
                                    SetWindowTextW(ctrls.static_text, w!("Folder Dropped. Ready to Scan."));
                                }
                                
                                // Auto-trigger scan? Or wait for user?
                                // Let's just update the label for now.
                                // Or simulate click on Scan button?
                                // "saya drag and drop ... memudahkan user"
                                // Let's trigger the scan logic directly or via PostMessage?
                                // Trigger Scan directly
                                let tx = st.tx.clone();
                                let cancel = st.cancel_flag.clone();
                                cancel.store(false, Ordering::Relaxed);
                                
                                if let Some(ctrls) = &st.controls {
                                    SetWindowTextW(ctrls.static_text, w!("Scanning (Drop)..."));
                                    EnableWindow(ctrls.btn_cancel, true);
                                }

                                start_scan_worker(folder, tx, cancel);
                            }
                        }
                    }
                }
                
                DragFinish(hdrop);
                LRESULT(0)
            }

            WM_SETTINGCHANGE => {
                 // Check for theme change
                 update_theme(hwnd);
                 DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

// Win11 Mica / Win10 Acrylic Support


unsafe fn pick_folder() -> Result<String> {
    let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)?;
    let options = dialog.GetOptions()?;
    dialog.SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)?;
    dialog.Show(None)?;
    let item = dialog.GetResult()?;
    let path_ptr = item.GetDisplayName(SIGDN_FILESYSPATH)?;
    let path = path_ptr.to_string()?;
    CoTaskMemFree(Some(path_ptr.as_ptr() as *mut _));
    Ok(path)
}

fn start_scan_worker(path: String, tx: Sender<UiMessage>, cancel: Arc<AtomicBool>) {
    thread::spawn(move || {
         let mut file_count = 0;
         let mut total_size = 0u64;
         let mut est_size = 0u64;
         let mut compressed_count = 0;

         for result in WalkBuilder::new(&path).build() {
             if cancel.load(Ordering::Relaxed) {
                  let _ = tx.send(UiMessage::Status(format!("Scanning Cancelled.")));
                  let _ = tx.send(UiMessage::Finished);
                  return;
             }
             if let Ok(entry) = result {
                  if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                      file_count += 1;
                      if file_count % 50 == 0 {
                          let _ = tx.send(UiMessage::Status(format!("Scanning... Found {} files ({} compressed)", file_count, compressed_count)));
                      }
                      
                      let p = entry.path();
                      let path_str = p.to_string_lossy();
                      
                      if is_wof_compressed(&path_str) {
                          compressed_count += 1;
                      }

                      if let Ok(meta) = p.metadata() {
                          total_size += meta.len();
                      }
                      est_size += estimate_size(p);
                  }
              }
         }
         use humansize::{format_size, BINARY};
         let msg = format!("Folder: {}\nFiles: {} ({} already compressed)\nCurrent Size: {}\nEst. Compressed: {}", 
             path, file_count, compressed_count, format_size(total_size, BINARY), format_size(est_size, BINARY));
         let _ = tx.send(UiMessage::Log(msg));
         let _ = tx.send(UiMessage::Finished);
    });
}

fn apply_backdrop(hwnd: HWND) {
    unsafe {
        // Try Mica (DWMSBT_MAINWINDOW = 2, DWMSBT_TRANSIENTWINDOW = 3, DWMSBT_TABBEDWINDOW = 4)
        // Or older DWMWA_MICA_EFFECT = 1029
        // Windows 11 Build 22621+ uses DWMWA_SYSTEMBACKDROP_TYPE (38)
        let system_backdrop_type = DWMWA_SYSTEMBACKDROP_TYPE;
        let mica = DWM_SYSTEMBACKDROP_TYPE(2); // DWMSBT_MAINWINDOW
        
        if let Err(_) = DwmSetWindowAttribute(hwnd, system_backdrop_type, &mica as *const _ as _, 4) {
             // Fallback or older Windows?
             // Not critical
        }
    }
}

unsafe fn is_system_dark_mode() -> bool {
    let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let val_name = w!("AppsUseLightTheme");
    let mut hkey: HKEY = Default::default();
    
    if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, 0, KEY_READ, &mut hkey).is_ok() {
        let mut data: u32 = 0;
        let mut cb_data = std::mem::size_of::<u32>() as u32;
        let result = RegQueryValueExW(hkey, val_name, None, None, Some(&mut data as *mut _ as _), Some(&mut cb_data));
        let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
        
        if result.is_ok() {
            return data == 0; // 0 means Dark, 1 means Light
        }
    }
    false // Default to Light if unknown
}

fn update_theme(hwnd: HWND) {
    unsafe {
        let dark = is_system_dark_mode();
        let attr = 20; // DWMWA_USE_IMMERSIVE_DARK_MODE
        let val = if dark { 1 } else { 0 }; // BOOL
        let _ = DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(attr), &val as *const _ as _, std::mem::size_of::<i32>() as u32);
        
        // Redraw will happen naturally or via resizing
    }
}
