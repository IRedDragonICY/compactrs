#![allow(unsafe_op_in_unsafe_fn)]
use crate::ui::state::AppTheme;
use crate::ui::builder::ControlBuilder;
use crate::utils::to_wstring;
use crate::ui::framework::WindowHandler;
use crate::types::*;
use crate::ui::wrappers::{Button, Label, Trackbar, ComboBox};
use crate::ui::declarative::{DeclarativeContext, ContainerBuilder};
use crate::ui::layout::{LayoutNode, SizePolicy};

#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(hwnd: HWND, lpOperation: LPCWSTR, lpFile: LPCWSTR, lpParameters: LPCWSTR, lpDirectory: LPCWSTR, nShowCmd: i32) -> HINSTANCE;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumThreadWindows(dwThreadId: u32, lpfn: Option<unsafe extern "system" fn(HWND, LPARAM) -> BOOL>, lParam: LPARAM) -> BOOL;
    fn EnumChildWindows(hWndParent: HWND, lpEnumFunc: Option<unsafe extern "system" fn(HWND, LPARAM) -> BOOL>, lParam: LPARAM) -> BOOL;
    fn GetDC(hWnd: HWND) -> HDC;
    fn ReleaseDC(hWnd: HWND, hDC: HDC) -> i32;
}

const SETTINGS_TITLE: &str = "Settings";

// Control IDs
const IDC_COMBO_THEME: u16 = 2001;
const IDC_BTN_CANCEL: u16 = 2006;
const IDC_CHK_FORCE_STOP: u16 = 2007;
const IDC_CHK_CONTEXT_MENU: u16 = 2008;
const IDC_CHK_SYSTEM_GUARD: u16 = 2009;
const IDC_CHK_LOW_POWER: u16 = 2013;
const IDC_SLIDER_THREADS: u16 = 2014;
const IDC_CHK_CTX_DIALOG: u16 = 2017;

const IDC_CHK_LOG_ENABLED: u16 = 2021;
const IDC_CHK_LOG_ERRORS: u16 = 2022;
const IDC_CHK_LOG_WARNS: u16 = 2023;
const IDC_CHK_LOG_INFO: u16 = 2024;
const IDC_CHK_LOG_TRACE: u16 = 2025;

const IDC_EDIT_CONCURRENT: u16 = 2031;

const IDC_CHK_SKIP_EXT: u16 = 2041;
const IDC_EDIT_EXTENSIONS: u16 = 2042;
const IDC_BTN_RESET_EXT: u16 = 2043;
const IDC_CHK_SET_ATTR: u16 = 2044;

const IDC_COMBO_UI_SCALE: u16 = 2045;

const IDC_BTN_CHECK_UPDATE: u16 = 2010;
const IDC_LBL_UPDATE_STATUS: u16 = 2011;
const IDC_BTN_RESTART_TI: u16 = 2012;
const IDC_BTN_RESET_ALL: u16 = 2016;
const WM_APP_UPDATE_CHECK_RESULT: u32 = 0x8000 + 10;
const WM_GETFONT: u32 = 0x0031;

// Tab System IDs
const IDC_TAB_START: u16 = 2100;
const IDC_SEARCH_SETTINGS: u16 = 2050;
const IDC_LIST_SEARCH_RESULTS: u16 = 2051;

struct SettingsState {
    theme: AppTheme,
    result: Option<AppTheme>,
    is_dark: bool,
    enable_force_stop: bool,
    enable_context_menu: bool,
    enable_system_guard: bool,
    low_power_mode: bool,
    max_threads: u32,
    max_concurrent_items: u32,
    log_enabled: bool,
    log_level_mask: u8,
    enable_skip_heuristics: bool,
    skip_extensions: String,
    set_compressed_attr: bool,
    ui_scale_multiplier: f32,
    context_menu_dialog_only: bool,

    update_status: UpdateStatus,
    pending_update: Option<crate::updater::UpdateInfo>,
    h_font_bold: HFONT,
    h_font_icon: HFONT,
    
    // Tab System
    tab_buttons: Vec<HWND>,
    tab_panels: Vec<HWND>,
    tab_layouts: Vec<LayoutNode>,
    active_tab: usize,
    sidebar_layout: Option<LayoutNode>,
    top_layout: Option<LayoutNode>,
    
    // Highlight & Search
    highlighted_control_id: Option<u16>,
    hwnd_search_list: HWND,
    search_results: Vec<SearchTarget>,
}

#[derive(Clone, Debug, PartialEq)]
enum UpdateStatus {
    Idle,
    Checking,
    Updating,
}

#[derive(Clone)]
struct SearchTarget {
    tab_idx: usize,
    ctrl_id: u16,
    title: &'static str,
    keywords: &'static [&'static str],
}

fn get_search_targets() -> Vec<SearchTarget> {
    vec![
        // Tab 0: General
        SearchTarget { tab_idx: 0, ctrl_id: IDC_CHK_FORCE_STOP, title: "Force Kill Processes", keywords: &["force", "kill", "process", "terminate", "lock", "automatically"] },
        SearchTarget { tab_idx: 0, ctrl_id: IDC_CHK_CONTEXT_MENU, title: "Explorer Context Menu", keywords: &["explorer", "context", "menu", "right", "click", "add"] },
        SearchTarget { tab_idx: 0, ctrl_id: IDC_CHK_SYSTEM_GUARD, title: "System Safety Guard", keywords: &["system", "guard", "safety", "critical", "prevent", "file"] },
        
        // Tab 1: Interface
        SearchTarget { tab_idx: 1, ctrl_id: IDC_COMBO_THEME, title: "Application Theme", keywords: &["theme", "light", "dark", "appearance", "color", "system", "default"] },
        SearchTarget { tab_idx: 1, ctrl_id: IDC_COMBO_UI_SCALE, title: "UI Scaling", keywords: &["scale", "scaling", "size", "zoom", "interface", "ui", "adjust"] },
        SearchTarget { tab_idx: 1, ctrl_id: IDC_CHK_SET_ATTR, title: "Show Compressed Color", keywords: &["compress", "color", "blue", "attribute", "mark", "show"] },
        
        // Tab 2: Performance
        SearchTarget { tab_idx: 2, ctrl_id: IDC_CHK_LOW_POWER, title: "Efficiency Mode", keywords: &["efficiency", "mode", "low", "power", "background", "resource", "reduce"] },
        SearchTarget { tab_idx: 2, ctrl_id: IDC_SLIDER_THREADS, title: "CPU Thread Limit", keywords: &["cpu", "thread", "limit", "worker", "core", "max", "maximum"] },
        SearchTarget { tab_idx: 2, ctrl_id: IDC_EDIT_CONCURRENT, title: "Concurrent File Queue", keywords: &["concurrent", "file", "queue", "simultaneous", "unlimited"] },
        
        // Tab 3: File Handling
        SearchTarget { tab_idx: 3, ctrl_id: IDC_CHK_SKIP_EXT, title: "Smart Compression Skip", keywords: &["smart", "skip", "unlikely", "filter", "compress", "further"] },
        SearchTarget { tab_idx: 3, ctrl_id: IDC_EDIT_EXTENSIONS, title: "Excluded Extensions", keywords: &["exclude", "extension", "format", "zip", "rar", "default"] },
        
        // Tab 4: Diagnostics
        SearchTarget { tab_idx: 4, ctrl_id: IDC_CHK_LOG_ENABLED, title: "Enable Diagnostic Logging", keywords: &["diagnostic", "log", "console", "real-time", "enable", "show"] },
        SearchTarget { tab_idx: 4, ctrl_id: IDC_CHK_LOG_ERRORS, title: "Log Levels (Errors, Warn, Info)", keywords: &["level", "error", "warning", "info", "trace", "log"] },
        
        // Tab 5: Updates
        SearchTarget { tab_idx: 5, ctrl_id: IDC_BTN_CHECK_UPDATE, title: "Check for Updates", keywords: &["compactrs", "update", "version", "latest", "check"] },
        SearchTarget { tab_idx: 5, ctrl_id: IDC_BTN_RESTART_TI, title: "Advanced Startup (TI)", keywords: &["advanced", "startup", "trustedinstaller", "restart", "privilege", "ti"] },
        SearchTarget { tab_idx: 5, ctrl_id: IDC_BTN_RESET_ALL, title: "Reset Application Defaults", keywords: &["reset", "default", "application"] },
    ]
}

pub unsafe fn show_settings_modal(
    parent: HWND, 
    current_theme: AppTheme, 
    is_dark: bool, 
    enable_force_stop: bool, 
    enable_context_menu: bool, 
    enable_system_guard: bool, 
    low_power_mode: bool, 
    max_threads: u32, 
    max_concurrent_items: u32, 
    log_enabled: bool, 
    log_level_mask: u8,
    enable_skip_heuristics: bool,
    skip_extensions_buf: [u16; 512],
    set_compressed_attr: bool,
    ui_scale_multiplier: f32,
    context_menu_dialog_only: bool
) -> (Option<AppTheme>, bool, bool, bool, bool, u32, u32, bool, u8, bool, [u16; 512], bool, f32, bool) {

    let skip_string = String::from_utf16_lossy(&skip_extensions_buf)
        .trim_matches(char::from(0))
        .to_string();

    let mut state = SettingsState {
        theme: current_theme,
        result: None,
        is_dark,
        enable_force_stop,
        enable_context_menu,
        enable_system_guard,
        low_power_mode,
        max_threads,
        max_concurrent_items,
        log_enabled,
        log_level_mask,
        enable_skip_heuristics,
        skip_extensions: skip_string,
        set_compressed_attr,
        ui_scale_multiplier,
        context_menu_dialog_only,
        update_status: UpdateStatus::Idle,
        pending_update: None,
        h_font_bold: std::ptr::null_mut(),
        h_font_icon: std::ptr::null_mut(),
        tab_buttons: Vec::new(),
        tab_panels: Vec::new(),
        tab_layouts: Vec::new(),
        active_tab: 0,
        sidebar_layout: None,
        top_layout: None,
        highlighted_control_id: None,
        hwnd_search_list: std::ptr::null_mut(),
        search_results: Vec::new(),
    };
    
    let initial_width = crate::ui::theme::scale(850);
    let initial_height = crate::ui::theme::scale(550);

    let ran_modal = crate::ui::dialogs::base::show_modal_singleton(
        parent, 
        &mut state, 
        "CompactRS_Settings", 
        SETTINGS_TITLE, 
        initial_width,
        initial_height,
        is_dark
    );
    
    let mut final_buf = [0u16; 512];
    if ran_modal {
        let mut i = 0;
        for c in state.skip_extensions.encode_utf16() {
            if i < 511 {
                final_buf[i] = c;
                i += 1;
            }
        }
        (state.result, state.enable_force_stop, state.enable_context_menu, state.enable_system_guard, state.low_power_mode, state.max_threads, state.max_concurrent_items, state.log_enabled, state.log_level_mask, state.enable_skip_heuristics, final_buf, state.set_compressed_attr, state.ui_scale_multiplier, state.context_menu_dialog_only)
    } else {
         (None, enable_force_stop, enable_context_menu, enable_system_guard, low_power_mode, max_threads, max_concurrent_items, log_enabled, log_level_mask, enable_skip_heuristics, skip_extensions_buf, set_compressed_attr, ui_scale_multiplier, context_menu_dialog_only)
    }
}

struct FontMap {
    old_bold: HFONT,
    old_icon: HFONT,
    new_bold: HFONT,
    new_icon: HFONT,
    new_app: HFONT,
}

// Subclass untuk Highlight Control (Menggambar Highlight secara Overlap agar tidak tertutup Label)
unsafe extern "system" fn highlight_subclass_proc(
    hwnd: HWND, umsg: u32, wparam: WPARAM, lparam: LPARAM,
    _uidsubclass: usize, _dwrefdata: usize
) -> LRESULT {
    if umsg == WM_PAINT {
        let res = DefSubclassProc(hwnd, umsg, wparam, lparam);
        let main_hwnd = GetParent(hwnd);
        let state_ptr = GetWindowLongPtrW(main_hwnd, GWLP_USERDATA) as *mut SettingsState;
        
        if !state_ptr.is_null() {
            let state = &*state_ptr;
            if let Some(hl_id) = state.highlighted_control_id {
                let h_ctrl = GetDlgItem(hwnd, hl_id as i32);
                if h_ctrl != std::ptr::null_mut() && GetParent(h_ctrl) == hwnd {
                    let hdc = GetDC(hwnd);
                    let mut rc: RECT = std::mem::zeroed();
                    GetWindowRect(h_ctrl, &mut rc);
                    let mut pt1 = POINT { x: rc.left, y: rc.top };
                    let mut pt2 = POINT { x: rc.right, y: rc.bottom };
                    ScreenToClient(hwnd, &mut pt1);
                    ScreenToClient(hwnd, &mut pt2);
                    
                    let mut panel_rc: RECT = std::mem::zeroed();
                    GetClientRect(hwnd, &mut panel_rc);
                    
                    let is_large_control = hl_id == IDC_EDIT_EXTENSIONS;
                    let row_top = if is_large_control { pt1.y - crate::ui::theme::scale(28) } else { pt1.y };
                    let row_bottom = if is_large_control { pt2.y + crate::ui::theme::scale(4) } else { pt1.y + crate::ui::theme::scale(42) };
                    
                    let draw_rc = RECT {
                        left: crate::ui::theme::scale(6),
                        top: row_top - crate::ui::theme::scale(2),
                        right: panel_rc.right - crate::ui::theme::scale(6),
                        bottom: row_bottom + crate::ui::theme::scale(2),
                    };
                    
                    let ctrl_center_y = row_top + (row_bottom - row_top) / 2;
                    let pen = CreatePen(PS_SOLID as i32, crate::ui::theme::scale(2), 0x00D47800); 
                    let old_pen = SelectObject(hdc, pen as HGDIOBJ);
                    let old_brush = SelectObject(hdc, GetStockObject(5)); 
                    RoundRect(hdc, draw_rc.left, draw_rc.top, draw_rc.right, draw_rc.bottom, crate::ui::theme::scale(8), crate::ui::theme::scale(8));
                    
                    let pill_rc = RECT {
                        left: draw_rc.left,
                        top: ctrl_center_y - crate::ui::theme::scale(10),
                        right: draw_rc.left + crate::ui::theme::scale(4),
                        bottom: ctrl_center_y + crate::ui::theme::scale(10),
                    };
                    let pill_brush = CreateSolidBrush(0x00D47800);
                    FillRect(hdc, &pill_rc, pill_brush);
                    DeleteObject(pill_brush as HGDIOBJ);
                    
                    SelectObject(hdc, old_brush);
                    SelectObject(hdc, old_pen);
                    DeleteObject(pen as HGDIOBJ);
                    ReleaseDC(hwnd, hdc);
                }
            }
        }
        return res;
    }
    DefSubclassProc(hwnd, umsg, wparam, lparam)
}

unsafe extern "system" fn search_edit_subclass_proc(
    hwnd: HWND, umsg: u32, wparam: WPARAM, lparam: LPARAM,
    _uidsubclass: usize, _dwrefdata: usize
) -> LRESULT {
    if umsg == WM_KEYDOWN {
        let vk = wparam as i32;
        if vk == 0x28 || vk == 0x26 || vk == 0x0D { 
            let main_hwnd = GetParent(hwnd);
            let state_ptr = GetWindowLongPtrW(main_hwnd, GWLP_USERDATA) as *mut SettingsState;
            if !state_ptr.is_null() {
                let state = &*state_ptr;
                if state.hwnd_search_list != std::ptr::null_mut() {
                    let is_visible = (GetWindowLongW(state.hwnd_search_list, GWL_STYLE) & WS_VISIBLE as i32) != 0;
                    if is_visible {
                        if vk == 0x0D { 
                            let idx = SendMessageW(state.hwnd_search_list, 0x0188, 0, 0); 
                            if idx >= 0 {
                                let wparam_cmd = ((1 << 16) | IDC_LIST_SEARCH_RESULTS as u32) as usize;
                                SendMessageW(main_hwnd, WM_COMMAND, wparam_cmd, state.hwnd_search_list as isize);
                            }
                            return 0;
                        } else {
                            SendMessageW(state.hwnd_search_list, umsg, wparam, lparam);
                            return 0;
                        }
                    }
                }
            }
        }
    }
    DefSubclassProc(hwnd, umsg, wparam, lparam)
}

impl SettingsState {
    unsafe fn get_control(&self, id: i32) -> HWND {
        for &p in &self.tab_panels {
            let h = GetDlgItem(p, id);
            if h != std::ptr::null_mut() { return h; }
        }
        std::ptr::null_mut()
    }

    unsafe fn switch_tab(&mut self, index: usize) {
        if index >= self.tab_panels.len() { return; }
        self.active_tab = index;
        
        for (i, &p) in self.tab_panels.iter().enumerate() {
            ShowWindow(p, if i == index { SW_SHOW } else { SW_HIDE });
        }
        
        for (i, &b) in self.tab_buttons.iter().enumerate() {
            let checked = if i == index { 1 } else { 0 };
            SendMessageW(b, BM_SETCHECK, checked, 0);
        }
    }

    unsafe fn apply_dynamic_scale(&mut self, hwnd: HWND) {
        crate::ui::theme::update_ui_scale(self.ui_scale_multiplier);

        let old_font_bold = self.h_font_bold;
        let old_font_icon = self.h_font_icon;

        let h_default = GetStockObject(DEFAULT_GUI_FONT);
        let mut lf: LOGFONTW = std::mem::zeroed();
        GetObjectW(h_default, std::mem::size_of::<LOGFONTW>() as i32, &mut lf as *mut _ as *mut _);
        lf.lfWeight = FW_BOLD as i32;
        lf.lfHeight = crate::ui::theme::scale(-12);
        self.h_font_bold = CreateFontIndirectW(&lf);

        let mut lf_icon: LOGFONTW = std::mem::zeroed();
        lf_icon.lfHeight = crate::ui::theme::scale(-16); 
        lf_icon.lfWeight = 400; 
        lf_icon.lfCharSet = 1; 
        let mdl2_name = "Segoe MDL2 Assets";
        for (i, c) in mdl2_name.encode_utf16().enumerate() { if i < 32 { lf_icon.lfFaceName[i] = c; } }
        self.h_font_icon = CreateFontIndirectW(&lf_icon);

        let font_map = FontMap {
            old_bold: old_font_bold,
            old_icon: old_font_icon,
            new_bold: self.h_font_bold,
            new_icon: self.h_font_icon,
            new_app: crate::ui::theme::get_app_font(),
        };

        let thread_id = GetCurrentThreadId();
        
        unsafe extern "system" fn enum_thread_wnd(hwnd: HWND, lparam: LPARAM) -> BOOL {
            EnumChildWindows(hwnd, Some(enum_child_wnd), lparam);
            enum_child_wnd(hwnd, lparam); 
            1
        }
        
        unsafe extern "system" fn enum_child_wnd(child: HWND, lparam: LPARAM) -> BOOL {
            let map = &*(lparam as *const FontMap);
            let h_font = SendMessageW(child, WM_GETFONT, 0, 0) as HFONT;
            
            if h_font == map.old_icon {
                SendMessageW(child, WM_SETFONT, map.new_icon as usize, 1);
            } else if h_font == map.old_bold {
                SendMessageW(child, WM_SETFONT, map.new_bold as usize, 1);
            } else {
                SendMessageW(child, WM_SETFONT, map.new_app as usize, 1);
            }
            1
        }
        
        EnumThreadWindows(thread_id, Some(enum_thread_wnd), &font_map as *const _ as LPARAM);

        if old_font_bold != std::ptr::null_mut() { DeleteObject(old_font_bold as _); }
        if old_font_icon != std::ptr::null_mut() { DeleteObject(old_font_icon as _); }

        let new_width = crate::ui::theme::scale(850);
        let new_height = crate::ui::theme::scale(550);
        
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let mut win_rect = RECT { left: 0, top: 0, right: new_width, bottom: new_height };
        AdjustWindowRect(&mut win_rect, style, 0);
        
        SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, win_rect.right - win_rect.left, win_rect.bottom - win_rect.top, SWP_NOMOVE | SWP_NOZORDER);
        
        let lparam_size = ((new_height & 0xFFFF) << 16) | (new_width & 0xFFFF);
        SendMessageW(hwnd, WM_SIZE, 0, lparam_size as isize);
        
        InvalidateRect(hwnd, std::ptr::null(), 1);

        let parent = GetParent(hwnd);
        if parent != std::ptr::null_mut() {
            let mut client_rc: RECT = std::mem::zeroed();
            GetClientRect(parent, &mut client_rc);
            let lparam_parent = ((client_rc.bottom & 0xFFFF) << 16) | (client_rc.right & 0xFFFF);
            SendMessageW(parent, WM_SIZE, 0, lparam_parent as isize);
            InvalidateRect(parent, std::ptr::null(), 1);
            
            let h_list = GetDlgItem(parent, crate::ui::controls::IDC_BATCH_LIST as i32);
            if h_list != std::ptr::null_mut() {
                InvalidateRect(h_list, std::ptr::null(), 1);
            }
        }
    }
}

impl WindowHandler for SettingsState {
    fn is_dark_mode(&self) -> bool {
        self.is_dark
    }

    fn on_create(&mut self, hwnd: HWND) -> LRESULT {
        unsafe {
            crate::ui::theme::set_window_frame_theme(hwnd, self.is_dark);

            let h_default = GetStockObject(DEFAULT_GUI_FONT);
            let mut lf: LOGFONTW = std::mem::zeroed();
            GetObjectW(h_default, std::mem::size_of::<LOGFONTW>() as i32, &mut lf as *mut _ as *mut _);
            lf.lfWeight = FW_BOLD as i32;
            lf.lfHeight = crate::ui::theme::scale(-12);
            self.h_font_bold = CreateFontIndirectW(&lf);
            
            let mut lf_icon: LOGFONTW = std::mem::zeroed();
            lf_icon.lfHeight = crate::ui::theme::scale(-16); 
            lf_icon.lfWeight = 400; 
            lf_icon.lfCharSet = 1; 
            let mdl2_name = "Segoe MDL2 Assets";
            for (i, c) in mdl2_name.encode_utf16().enumerate() { if i < 32 { lf_icon.lfFaceName[i] = c; } }
            self.h_font_icon = CreateFontIndirectW(&lf_icon);

            let ctx_main = DeclarativeContext::new(hwnd, self.is_dark, crate::ui::theme::get_app_font());

            // --- 1. Top Bar ---
            let h_search = ControlBuilder::new(hwnd, IDC_SEARCH_SETTINGS)
                .edit()
                .dark_mode(self.is_dark)
                .font(crate::ui::theme::get_app_font())
                .build();
            let cue = crate::w!("Search Preferences");
            SendMessageW(h_search, 0x1501, 1, cue.as_ptr() as isize);
            
            SetWindowSubclass(h_search, Some(search_edit_subclass_proc), 8888, 0);

            let top_layout = ctx_main.horizontal(10, 5, |r| {
                r.add_child(LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                r.add_child(LayoutNode::new_leaf(h_search, SizePolicy::Fixed(250)));
            });
            self.top_layout = Some(top_layout);

            // --- 2. Sidebar Buttons ---
            let tabs = [
                "General", 
                "Interface", 
                "Performance", 
                "File Handling", 
                "Diagnostics", 
                "Updates"
            ];
            
            let mut btn_nodes = Vec::new();
            for (i, name) in tabs.iter().enumerate() {
                let id = IDC_TAB_START + i as u16;
                let h = ControlBuilder::new(hwnd, id)
                    .radio()
                    .style(0x1000) // BS_PUSHLIKE
                    .text(*name)
                    .dark_mode(self.is_dark)
                    .font(crate::ui::theme::get_app_font())
                    .build();
                self.tab_buttons.push(h);
                btn_nodes.push(LayoutNode::new_leaf(h, SizePolicy::Fixed(32))); 
            }

            let mut sidebar_layout = LayoutNode::col(5, 2);
            for node in btn_nodes {
                sidebar_layout.add_child(node);
            }
            self.sidebar_layout = Some(sidebar_layout);

            // --- 3. Macro Helpers ---
            let section_header = |b: &mut ContainerBuilder, text: &str| {
                b.row_with_policy(10, SizePolicy::Fixed(35), |r: &mut ContainerBuilder| {
                    r.label(text, SizePolicy::Flex(1.0));
                });
            };

            let icon_row = |b: &mut ContainerBuilder, panel: HWND, icon: &str, title: &[u16], sub: &[u16], control_fn: &dyn Fn(&mut ContainerBuilder)| {
                b.row_with_policy(15, SizePolicy::Fixed(42), |r: &mut ContainerBuilder| {
                     r.col_with_policy(0, SizePolicy::Fixed(30), |c: &mut ContainerBuilder| {
                             let h = ControlBuilder::new(panel, 0).label(false).text(icon).font(self.h_font_icon).dark_mode(self.is_dark).build();
                             crate::ui::subclass::apply_theme_to_control(h, self.is_dark);
                             c.add_child(crate::ui::layout::LayoutNode::new_leaf(h, SizePolicy::Fixed(24)));
                     });
                     
                     r.col_with_policy(2, SizePolicy::Flex(1.0), |c: &mut ContainerBuilder| {
                         c.label_w(title, SizePolicy::Fixed(18)); 
                         let h = ControlBuilder::new(panel, 0).label(false).text_w(sub).dark_mode(self.is_dark).build();
                         crate::ui::subclass::apply_theme_to_control(h, self.is_dark);
                         c.add_child(crate::ui::layout::LayoutNode::new_leaf(h, SizePolicy::Fixed(16)));
                     });
                     
                     r.col_with_policy(0, SizePolicy::Fixed(190), |c: &mut ContainerBuilder| {
                         control_fn(c);
                     });
                });
            };

            // --- 4. Build Panels ---
            
            // Tab 0: General
            let p0 = crate::ui::components::panel::Panel::create(hwnd, "Tab0", 0, 0, 100, 100).unwrap();
            crate::ui::components::panel::Panel::update_theme(p0, self.is_dark);
            let l0 = {
                let ctx = DeclarativeContext::new(p0, self.is_dark, self.h_font_bold);
                ctx.vertical(15, 6, |v| {
                    section_header(v, "General Behavior");
                    icon_row(v, p0, "\u{E74D}", crate::w!("Force Kill Processes"), crate::w!("Automatically terminate locking processes"), &|c| {
                         c.row_with_policy(0, SizePolicy::Fixed(20), |r| {
                             r.add_child(crate::ui::layout::LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                             r.checkbox(IDC_CHK_FORCE_STOP, "", self.enable_force_stop, SizePolicy::Fixed(20));
                         });
                    });
                    icon_row(v, p0, "\u{E8DE}", crate::w!("Explorer Context Menu"), crate::w!("Add 'CompactRS' to right-click menu"), &|c| {
                         c.row_with_policy(0, SizePolicy::Fixed(20), |r| {
                             r.add_child(crate::ui::layout::LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                             r.checkbox(IDC_CHK_CONTEXT_MENU, "Enable", self.enable_context_menu, SizePolicy::Fixed(70));
                             r.checkbox(IDC_CHK_CTX_DIALOG, "Dialog Only", self.context_menu_dialog_only, SizePolicy::Fixed(110));
                         });
                    });
                    icon_row(v, p0, "\u{EA18}", crate::w!("System Safety Guard"), crate::w!("Prevent compression of critical system files"), &|c| {
                         c.row_with_policy(0, SizePolicy::Fixed(20), |r| {
                             r.add_child(crate::ui::layout::LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                             r.checkbox(IDC_CHK_SYSTEM_GUARD, "", self.enable_system_guard, SizePolicy::Fixed(20));
                         });
                    });
                })
            };
            self.tab_panels.push(p0);
            self.tab_layouts.push(l0);

            // Tab 1: Interface
            let p1 = crate::ui::components::panel::Panel::create(hwnd, "Tab1", 0, 0, 100, 100).unwrap();
            crate::ui::components::panel::Panel::update_theme(p1, self.is_dark);
            let l1 = {
                let ctx = DeclarativeContext::new(p1, self.is_dark, self.h_font_bold);
                ctx.vertical(15, 6, |v| {
                    section_header(v, "Appearance");
                    icon_row(v, p1, "\u{E713}", crate::w!("Application Theme"), crate::w!("Choose between Light, Dark, or System Default"), &|c| {
                         c.combobox(IDC_COMBO_THEME, &["System Default", "Dark Mode", "Light Mode"], 
                             match self.theme { AppTheme::System => 0, AppTheme::Dark => 1, AppTheme::Light => 2 }, 
                             SizePolicy::Fixed(24)); 
                    });
                    icon_row(v, p1, "\u{E8A3}", crate::w!("UI Scaling"), crate::w!("Adjust interface size (Applies instantly)"), &|c| {
                         c.combobox(IDC_COMBO_UI_SCALE, &["25%", "50%", "75%", "100%", "125%", "150%", "175%", "200%"], 
                             match self.ui_scale_multiplier {
                                 x if x >= 2.0 => 7, x if x >= 1.75 => 6, x if x >= 1.5 => 5, x if x >= 1.25 => 4,
                                 x if x >= 1.0 => 3, x if x >= 0.75 => 2, x if x >= 0.50 => 1, _ => 0,
                             }, 
                             SizePolicy::Fixed(24)); 
                    });
                    icon_row(v, p1, "\u{F012}", crate::w!("Show Compressed Color"), crate::w!("Mark compressed items with blue system attribute"), &|c| {
                         c.row_with_policy(0, SizePolicy::Fixed(20), |r| {
                             r.add_child(crate::ui::layout::LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                             r.checkbox(IDC_CHK_SET_ATTR, "", self.set_compressed_attr, SizePolicy::Fixed(20));
                         });
                    });
                })
            };
            self.tab_panels.push(p1);
            self.tab_layouts.push(l1);

            // Tab 2: Performance
            let p2 = crate::ui::components::panel::Panel::create(hwnd, "Tab2", 0, 0, 100, 100).unwrap();
            crate::ui::components::panel::Panel::update_theme(p2, self.is_dark);
            let l2 = {
                let ctx = DeclarativeContext::new(p2, self.is_dark, self.h_font_bold);
                ctx.vertical(15, 6, |v| {
                    section_header(v, "Performance Optimization");
                    
                    icon_row(v, p2, "\u{EC48}", crate::w!("Efficiency Mode"), crate::w!("Reduce background resource usage (Low Power)"), &|c| {
                         c.row_with_policy(0, SizePolicy::Fixed(20), |r| {
                             r.add_child(crate::ui::layout::LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                             r.checkbox(IDC_CHK_LOW_POWER, "", self.low_power_mode, SizePolicy::Fixed(20));
                         });
                    });
                    
                    let cpu_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as u32;
                    let current_threads = if self.max_threads == 0 { cpu_count } else { self.max_threads };
                    let thread_sub = crate::utils::concat_wstrings(&[crate::w!("Maximum worker threads (Current: "), &crate::utils::fmt_u32(current_threads), crate::w!(")")]);
                    icon_row(v, p2, "\u{E9D9}", crate::w!("CPU Thread Limit"), &thread_sub, &|c| {
                         c.slider(IDC_SLIDER_THREADS, 1, cpu_count, current_threads, SizePolicy::Fixed(30));
                    });
                    icon_row(v, p2, "\u{E902}", crate::w!("Concurrent File Queue"), crate::w!("Files compressed simultaneously (0 = Unlimited)"), &|c| {
                         c.input(IDC_EDIT_CONCURRENT, &self.max_concurrent_items.to_string(), ES_NUMBER | ES_CENTER, SizePolicy::Fixed(24));
                    });
                })
            };
            self.tab_panels.push(p2);
            self.tab_layouts.push(l2);

            // Tab 3: File Handling
            let p3 = crate::ui::components::panel::Panel::create(hwnd, "Tab3", 0, 0, 100, 100).unwrap();
            crate::ui::components::panel::Panel::update_theme(p3, self.is_dark);
            let l3 = {
                let ctx = DeclarativeContext::new(p3, self.is_dark, self.h_font_bold);
                ctx.vertical(15, 6, |v| {
                    section_header(v, "File Filtering");
                    icon_row(v, p3, "\u{E71C}", crate::w!("Smart Compression Skip"), crate::w!("Skip files that are unlikely to compress further"), &|c| {
                         c.row_with_policy(0, SizePolicy::Fixed(20), |r| {
                             r.add_child(crate::ui::layout::LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                             r.checkbox(IDC_CHK_SKIP_EXT, "", self.enable_skip_heuristics, SizePolicy::Fixed(20));
                         });
                    });
                    v.row_with_policy(15, SizePolicy::Fixed(24), |r| {
                        r.label("Excluded Extensions:", SizePolicy::Flex(1.0));
                        r.button_w(IDC_BTN_RESET_EXT, crate::w!("Reset Defaults"), SizePolicy::Fixed(100)); 
                    });
                    
                    let mut ext_node = LayoutNode::row(15, 0);
                    let h_edit_ext = ControlBuilder::new(p3, IDC_EDIT_EXTENSIONS)
                        .edit()
                        .text(&self.skip_extensions)
                        .style(ES_AUTOVSCROLL | ES_MULTILINE)
                        .dark_mode(self.is_dark)
                        .font(crate::ui::theme::get_app_font())
                        .build();
                    crate::ui::subclass::apply_theme_to_control(h_edit_ext, self.is_dark);
                    ext_node.add_child(LayoutNode::new_leaf(h_edit_ext, SizePolicy::Flex(1.0)));
                    v.add_child(ext_node.with_policy(SizePolicy::Fixed(80)));
                    
                    if !self.enable_skip_heuristics {
                         let btn = GetDlgItem(p3, IDC_BTN_RESET_EXT as i32);
                         if btn != std::ptr::null_mut() { Button::new(btn).set_enabled(false); }
                         Button::new(h_edit_ext).set_enabled(false);
                    }
                })
            };
            self.tab_panels.push(p3);
            self.tab_layouts.push(l3);

            // Tab 4: Diagnostics
            let p4 = crate::ui::components::panel::Panel::create(hwnd, "Tab4", 0, 0, 100, 100).unwrap();
            crate::ui::components::panel::Panel::update_theme(p4, self.is_dark);
            let l4 = {
                let ctx = DeclarativeContext::new(p4, self.is_dark, self.h_font_bold);
                ctx.vertical(15, 6, |v| {
                    section_header(v, "Diagnostics");
                    icon_row(v, p4, "\u{EBE8}", crate::w!("Enable Diagnostic Logging"), crate::w!("Show real-time logs in a console window"), &|c| {
                         c.row_with_policy(0, SizePolicy::Fixed(20), |r| {
                             r.add_child(crate::ui::layout::LayoutNode::new_leaf(std::ptr::null_mut(), SizePolicy::Flex(1.0)));
                             r.checkbox(IDC_CHK_LOG_ENABLED, "", self.log_enabled, SizePolicy::Fixed(20));
                         });
                    });
                    v.row_with_policy(15, SizePolicy::Fixed(35), |r| {
                         r.label("Levels:", SizePolicy::Fixed(50));
                         r.checkbox(IDC_CHK_LOG_ERRORS, "Errors", self.log_level_mask & crate::logger::LOG_LEVEL_ERROR != 0, SizePolicy::Fixed(70));
                         r.checkbox(IDC_CHK_LOG_WARNS, "Warnings", self.log_level_mask & crate::logger::LOG_LEVEL_WARN != 0, SizePolicy::Fixed(80));
                         r.checkbox(IDC_CHK_LOG_INFO, "Info", self.log_level_mask & crate::logger::LOG_LEVEL_INFO != 0, SizePolicy::Fixed(60));
                         r.checkbox(IDC_CHK_LOG_TRACE, "Trace", self.log_level_mask & crate::logger::LOG_LEVEL_TRACE != 0, SizePolicy::Fixed(70));
                    });
                    if !self.log_enabled {
                        let ids = [IDC_CHK_LOG_ERRORS, IDC_CHK_LOG_WARNS, IDC_CHK_LOG_INFO, IDC_CHK_LOG_TRACE];
                        for &id in &ids { 
                            let btn = GetDlgItem(p4, id as i32);
                            if btn != std::ptr::null_mut() { Button::new(btn).set_enabled(false); }
                        }
                    }
                })
            };
            self.tab_panels.push(p4);
            self.tab_layouts.push(l4);

            // Tab 5: Updates
            let p5 = crate::ui::components::panel::Panel::create(hwnd, "Tab5", 0, 0, 100, 100).unwrap();
            crate::ui::components::panel::Panel::update_theme(p5, self.is_dark);
            let l5 = {
                let ctx = DeclarativeContext::new(p5, self.is_dark, self.h_font_bold);
                ctx.vertical(15, 6, |v| {
                    section_header(v, "About & Updates");
                    let version_str = crate::utils::to_wstring(env!("APP_VERSION"));
                    icon_row(v, p5, "\u{E946}", crate::w!("CompactRS"), &version_str, &|c| {
                         c.button_w(IDC_BTN_CHECK_UPDATE, crate::w!("Check for Updates"), SizePolicy::Fixed(24));
                         c.add_child(crate::ui::layout::LayoutNode::new_leaf(
                             ControlBuilder::new(p5, IDC_LBL_UPDATE_STATUS).label(true).text("").dark_mode(self.is_dark).build(),
                             SizePolicy::Fixed(16)
                         ));
                    });
                    icon_row(v, p5, "\u{E7EF}", crate::w!("Advanced Startup"), crate::w!("Restart with TrustedInstaller privileges"), &|c| {
                         if crate::engine::elevation::is_system_or_ti() {
                             c.label_w(crate::w!("Running as TI"), SizePolicy::Fixed(24));
                         } else {
                             c.button_w(IDC_BTN_RESTART_TI, crate::w!("Restart as TI"), SizePolicy::Fixed(24));
                         }
                    });
                    v.row_with_policy(15, SizePolicy::Fixed(30), |r| {
                         r.label("", SizePolicy::Flex(1.0));
                         r.button_w(IDC_BTN_RESET_ALL, crate::w!("Reset Application Defaults"), SizePolicy::Fixed(180));
                    });
                })
            };
            self.tab_panels.push(p5);
            self.tab_layouts.push(l5);
            
            for &p in &self.tab_panels {
                SetWindowSubclass(p, Some(highlight_subclass_proc), 9999, hwnd as usize);
            }

            // --- 5. Create the Floating Search ListBox (Owner Drawn) ---
            let listbox_class = crate::w!("LISTBOX");
            let h_list = CreateWindowExW(
                0, 
                listbox_class.as_ptr(),
                std::ptr::null(),
                // LBS_NOTIFY | LBS_HASSTRINGS | LBS_OWNERDRAWFIXED
                WS_CHILD | WS_BORDER | WS_VSCROLL | 0x0001 | 0x0040 | 0x0010, 
                0, 0, 250, 150,
                hwnd,
                IDC_LIST_SEARCH_RESULTS as isize as HMENU,
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null_mut()
            );
            self.hwnd_search_list = h_list;
            
            // Set item height
            SendMessageW(h_list, 0x01A0, 0, crate::ui::theme::scale(28) as isize); // LB_SETITEMHEIGHT
            SendMessageW(h_list, WM_SETFONT, crate::ui::theme::get_app_font() as usize, 1);

            crate::ui::theme::apply_theme_recursive(hwnd, self.is_dark);
            
            self.switch_tab(0);
            
            let lparam_size = ((crate::ui::theme::scale(550) & 0xFFFF) << 16) | (crate::ui::theme::scale(850) & 0xFFFF);
            SendMessageW(hwnd, WM_SIZE, 0, lparam_size as isize);
        }
        0
    }
    
    fn on_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            match msg {
                WM_DESTROY => {
                    if self.h_font_bold != std::ptr::null_mut() { DeleteObject(self.h_font_bold); }
                    if self.h_font_icon != std::ptr::null_mut() { DeleteObject(self.h_font_icon); }
                    None
                },
                // Handle owner draw for search listbox
                WM_DRAWITEM => {
                    let dis = &*(lparam as *const DRAWITEMSTRUCT);
                    if dis.CtlID == IDC_LIST_SEARCH_RESULTS as u32 {
                        if dis.itemID != 0xFFFFFFFF {
                            let hdc = dis.hDC;
                            let rc = dis.rcItem;
                            let is_selected = (dis.itemState & ODS_SELECTED) != 0;
                            
                            // Accent Blue for selected item
                            let bg_color = if is_selected { 0x00D47800 } 
                                           else if self.is_dark { crate::ui::theme::COLOR_LIST_BG_DARK } 
                                           else { crate::ui::theme::COLOR_LIST_BG_LIGHT };
                                           
                            let text_color = if is_selected { 0x00FFFFFF } 
                                             else if self.is_dark { crate::ui::theme::COLOR_LIST_TEXT_DARK } 
                                             else { crate::ui::theme::COLOR_LIST_TEXT_LIGHT };
                            
                            let brush = CreateSolidBrush(bg_color);
                            FillRect(hdc, &rc, brush);
                            DeleteObject(brush as HGDIOBJ);
                            
                            let len = SendMessageW(dis.hwndItem, 0x018A, dis.itemID as usize, 0); // LB_GETTEXTLEN
                            if len > 0 {
                                let mut buf = vec![0u16; (len + 1) as usize];
                                SendMessageW(dis.hwndItem, 0x0189, dis.itemID as usize, buf.as_mut_ptr() as isize); // LB_GETTEXT
                                
                                SetBkMode(hdc, TRANSPARENT as i32);
                                SetTextColor(hdc, text_color);
                                
                                let mut text_rc = rc;
                                text_rc.left += crate::ui::theme::scale(12); // Indent padding
                                DrawTextW(hdc, buf.as_ptr(), len as i32, &mut text_rc, DT_SINGLELINE | DT_VCENTER);
                            }
                        }
                        return Some(1);
                    }
                    None
                },
                WM_SIZE => {
                    let w = (lparam & 0xFFFF) as i32;
                    let h = ((lparam >> 16) & 0xFFFF) as i32;
                    
                    let top_bar_h = crate::ui::theme::scale(40);
                    let sidebar_w = crate::ui::theme::scale(180);
                    
                    if let Some(top) = &self.top_layout {
                        top.calculate_layout(RECT { left: 0, top: 0, right: w, bottom: top_bar_h });
                    }
                    
                    if let Some(sb) = &self.sidebar_layout {
                        sb.calculate_layout(RECT { left: 0, top: top_bar_h, right: sidebar_w, bottom: h });
                    }
                    
                    let content_rc = RECT { left: sidebar_w, top: top_bar_h, right: w, bottom: h };
                    let panel_w = content_rc.right - content_rc.left;
                    let panel_h = content_rc.bottom - content_rc.top;
                    
                    for (i, &p) in self.tab_panels.iter().enumerate() {
                        SetWindowPos(p, std::ptr::null_mut(), content_rc.left, content_rc.top, panel_w, panel_h, SWP_NOZORDER);
                        self.tab_layouts[i].calculate_layout(RECT { left: 0, top: 0, right: panel_w, bottom: panel_h });
                    }
                    
                    if self.hwnd_search_list != std::ptr::null_mut() {
                        let is_visible = (GetWindowLongW(self.hwnd_search_list, GWL_STYLE) & WS_VISIBLE as i32) != 0;
                        if is_visible {
                            let h_search = GetDlgItem(hwnd, IDC_SEARCH_SETTINGS as i32);
                            let mut rc: RECT = std::mem::zeroed();
                            GetWindowRect(h_search, &mut rc);
                            let mut pt = POINT { x: rc.left, y: rc.bottom };
                            ScreenToClient(hwnd, &mut pt);
                            
                            let item_height = SendMessageW(self.hwnd_search_list, 0x01A1, 0, 0) as i32;
                            let mut h_list = item_height * self.search_results.len() as i32 + 4; 
                            if h_list > 200 { h_list = 200; }
                            
                            SetWindowPos(self.hwnd_search_list, 0 as HWND, pt.x, pt.y, rc.right - rc.left, h_list, SWP_NOZORDER);
                        }
                    }
                    
                    Some(0)
                },
                WM_HSCROLL => {
                     let h_ctl = lparam as HWND;
                     if h_ctl == self.get_control(IDC_SLIDER_THREADS as i32) {
                         self.max_threads = Trackbar::new(h_ctl).get_pos();
                     }
                     Some(0)
                },
                WM_APP_UPDATE_CHECK_RESULT => {
                    let res_ptr = lparam as *mut Result<Option<crate::updater::UpdateInfo>, String>;
                    let res = Box::from_raw(res_ptr);
                    
                    self.update_status = UpdateStatus::Idle;
                    let h_btn = self.get_control(IDC_BTN_CHECK_UPDATE as i32);
                    let h_lbl = self.get_control(IDC_LBL_UPDATE_STATUS as i32);
                    
                    Button::new(h_btn).set_enabled(true);
                    
                    match *res {
                        Ok(Some(info)) => {
                            self.pending_update = Some(info.clone());
                            Label::new(h_lbl).set_text(&format!("Latest: {}", info.version));
                            Button::new(h_btn).set_text("Update Now");
                        },
                        Ok(None) => {
                            self.pending_update = None;
                            Label::new(h_lbl).set_text("You have the latest version.");
                            Button::new(h_btn).set_text("Check for Updates");
                        },
                        Err(e) => {
                            self.pending_update = None;
                            Label::new(h_lbl).set_text(&format!("Error: {}", e)); 
                            Button::new(h_btn).set_text("Retry Check");
                        }
                    }
                    Some(0)
                },
                WM_COMMAND => {
                     let id = (wparam & 0xFFFF) as u16;
                     let code = ((wparam >> 16) & 0xFFFF) as u16;
                     
                     if id == IDC_LIST_SEARCH_RESULTS {
                         if code == 1 { // LBN_SELCHANGE
                             let idx = SendMessageW(self.hwnd_search_list, 0x0188, 0, 0) as usize; // LB_GETCURSEL
                             if idx < self.search_results.len() {
                                 let target = self.search_results[idx].clone();
                                 
                                 // Bersihkan teks (akan trigger EN_CHANGE untuk menyembunyikan listbox)
                                 let h_search = GetDlgItem(hwnd, IDC_SEARCH_SETTINGS as i32);
                                 SetWindowTextW(h_search, crate::w!("").as_ptr());
                                 
                                 // Pindah tab
                                 if self.active_tab != target.tab_idx {
                                     self.switch_tab(target.tab_idx);
                                 }
                                 
                                 // Trigger highlight
                                 self.highlighted_control_id = Some(target.ctrl_id);
                                 InvalidateRect(self.tab_panels[target.tab_idx], std::ptr::null(), 1);
                             }
                         }
                         return Some(0);
                     }
                     
                     if id == IDC_SEARCH_SETTINGS {
                         if (code as u32) == EN_CHANGE {
                             let h_search = GetDlgItem(hwnd, IDC_SEARCH_SETTINGS as i32);
                             let len = GetWindowTextLengthW(h_search);
                             
                             // Selalu hapus highlight setiap user mengetik
                             self.highlighted_control_id = None;
                             InvalidateRect(self.tab_panels[self.active_tab], std::ptr::null(), 1);
                             
                             if len == 0 {
                                 ShowWindow(self.hwnd_search_list, SW_HIDE);
                             } else {
                                 let mut buf = vec![0u16; (len + 1) as usize];
                                 GetWindowTextW(h_search, buf.as_mut_ptr(), len + 1);
                                 let text = String::from_utf16_lossy(&buf[..len as usize]).to_lowercase();
                                 let text = text.trim();
                                 
                                 if text.is_empty() {
                                     ShowWindow(self.hwnd_search_list, SW_HIDE);
                                 } else {
                                     let targets = get_search_targets();
                                     self.search_results.clear();
                                     SendMessageW(self.hwnd_search_list, 0x0184, 0, 0); // LB_RESETCONTENT
                                     
                                     for t in targets {
                                         let mut is_match = false;
                                         if t.title.to_lowercase().contains(text) {
                                             is_match = true;
                                         } else {
                                             for &kw in t.keywords {
                                                 if kw.contains(text) || text.contains(kw) {
                                                     is_match = true;
                                                     break;
                                                 }
                                             }
                                         }
                                         
                                         if is_match {
                                             self.search_results.push(t.clone());
                                             let title_w = crate::utils::to_wstring(t.title);
                                             SendMessageW(self.hwnd_search_list, 0x0180, 0, title_w.as_ptr() as isize); // LB_ADDSTRING
                                         }
                                     }
                                     
                                     if !self.search_results.is_empty() {
                                         let mut rc: RECT = std::mem::zeroed();
                                         GetWindowRect(h_search, &mut rc);
                                         let mut pt = POINT { x: rc.left, y: rc.bottom };
                                         ScreenToClient(hwnd, &mut pt);
                                         
                                         let item_height = SendMessageW(self.hwnd_search_list, 0x01A1, 0, 0) as i32; // LB_GETITEMHEIGHT
                                         let mut h_list = item_height * self.search_results.len() as i32 + 4;
                                         if h_list > 200 { h_list = 200; }
                                         
                                         SetWindowPos(self.hwnd_search_list, 0 as HWND, pt.x, pt.y, rc.right - rc.left, h_list, SWP_SHOWWINDOW);
                                         BringWindowToTop(self.hwnd_search_list);
                                         
                                         // Otomatis menyeleksi item pertama
                                         SendMessageW(self.hwnd_search_list, 0x0186, 0, 0); // LB_SETCURSEL ke index 0
                                     } else {
                                         ShowWindow(self.hwnd_search_list, SW_HIDE);
                                     }
                                 }
                             }
                         }
                         return Some(0);
                     }
                     
                     if id >= IDC_TAB_START && id <= IDC_TAB_START + 5 {
                         if code == BN_CLICKED as u16 {
                             self.switch_tab((id - IDC_TAB_START) as usize);
                         }
                         return Some(0);
                     }
                     
                     match id {
                         IDC_COMBO_UI_SCALE => {
                            if (code as u32) == CBN_SELCHANGE {
                                let h_combo = self.get_control(IDC_COMBO_UI_SCALE as i32);
                                let idx = ComboBox::new(h_combo).get_selected_index();
                                self.ui_scale_multiplier = match idx {
                                    7 => 2.00, 6 => 1.75, 5 => 1.50, 4 => 1.25,
                                    3 => 1.00, 2 => 0.75, 1 => 0.50, 0 => 0.25, _ => 1.00,
                                };
                                self.apply_dynamic_scale(hwnd); 
                            }
                         },
                         IDC_COMBO_THEME => {
                            if (code as u32) == CBN_SELCHANGE {
                                let h_combo = self.get_control(IDC_COMBO_THEME as i32);
                                let idx = ComboBox::new(h_combo).get_selected_index();
                                let theme = match idx { 0 => AppTheme::System, 1 => AppTheme::Dark, 2 => AppTheme::Light, _ => AppTheme::System };
                                let new_is_dark = match theme { AppTheme::Dark => true, AppTheme::Light => false, AppTheme::System => crate::ui::theme::is_system_dark_mode() };
                                self.theme = theme; self.result = Some(theme); self.is_dark = new_is_dark;
                                crate::ui::theme::set_window_frame_theme(hwnd, new_is_dark);
                                crate::ui::theme::apply_theme_recursive(hwnd, new_is_dark);
                                InvalidateRect(hwnd, std::ptr::null(), 1);
                                
                                use GetParent;
                                let parent = GetParent(hwnd);
                                if parent != std::ptr::null_mut() {
                                    let theme_val = match theme { AppTheme::System => 0, AppTheme::Dark => 1, AppTheme::Light => 2 };
                                    SendMessageW(parent, 0x8000 + 1, theme_val as WPARAM, 0);
                                }
                            }
                         },
                         IDC_BTN_CANCEL => {
                             let h_edit = self.get_control(IDC_EDIT_CONCURRENT as i32);
                             if h_edit != std::ptr::null_mut() {
                                 let len = GetWindowTextLengthW(h_edit);
                                 if len > 0 {
                                     let mut buf = vec![0u16; (len + 1) as usize];
                                     GetWindowTextW(h_edit, buf.as_mut_ptr(), len + 1);
                                     let s = String::from_utf16_lossy(&buf[..len as usize]);
                                     let clean: String = s.chars().take_while(|c| c.is_digit(10)).collect();
                                     if let Ok(val) = clean.parse::<u32>() { self.max_concurrent_items = val; }
                                 } else { self.max_concurrent_items = 0; }
                             }
                             DestroyWindow(hwnd);
                         },
                         IDC_CHK_SKIP_EXT => {
                             if (code as u32) == BN_CLICKED {
                                  let checked = Button::new(self.get_control(IDC_CHK_SKIP_EXT as i32)).is_checked();
                                  self.enable_skip_heuristics = checked;
                                  let h_edit = self.get_control(IDC_EDIT_EXTENSIONS as i32); Button::new(h_edit).set_enabled(checked);
                                  let h_btn = self.get_control(IDC_BTN_RESET_EXT as i32); Button::new(h_btn).set_enabled(checked);
                             }
                         },
                         IDC_BTN_RESET_EXT => {
                              if (code as u32) == BN_CLICKED {
                                   let default_skip = "zip,7z,rar,gz,bz2,xz,zst,lz4,jpg,jpeg,png,gif,webp,avif,heic,mp4,mkv,avi,webm,mov,wmv,mp3,flac,aac,ogg,opus,wma,pdf";
                                   SetWindowTextW(self.get_control(IDC_EDIT_EXTENSIONS as i32), to_wstring(default_skip).as_ptr());
                              }
                         },
                         IDC_EDIT_EXTENSIONS => {
                               if (code as u32) == EN_CHANGE {
                                   let h = self.get_control(IDC_EDIT_EXTENSIONS as i32);
                                   let len = GetWindowTextLengthW(h);
                                   let mut buf = vec![0u16; (len + 1) as usize];
                                   GetWindowTextW(h, buf.as_mut_ptr(), len + 1);
                                   self.skip_extensions = String::from_utf16_lossy(&buf[..len as usize]);
                               }
                         },
                         IDC_CHK_FORCE_STOP => {
                              if (code as u32) == BN_CLICKED {
                                   let checked = Button::new(self.get_control(IDC_CHK_FORCE_STOP as i32)).is_checked();
                                   self.enable_force_stop = checked;
                                   use GetParent;
                                   let parent = GetParent(hwnd);
                                   if parent != std::ptr::null_mut() {
                                       SendMessageW(parent, 0x8000 + 3, if checked { 1 } else { 0 }, 0);
                                   }
                              }
                         },
                         IDC_CHK_CONTEXT_MENU => { if (code as u32) == BN_CLICKED { self.enable_context_menu = Button::new(self.get_control(id as i32)).is_checked(); } },
                         IDC_CHK_CTX_DIALOG => { if (code as u32) == BN_CLICKED { self.context_menu_dialog_only = Button::new(self.get_control(id as i32)).is_checked(); } },
                         IDC_CHK_SYSTEM_GUARD => { if (code as u32) == BN_CLICKED { self.enable_system_guard = Button::new(self.get_control(id as i32)).is_checked(); } },
                         IDC_CHK_LOW_POWER => { if (code as u32) == BN_CLICKED { self.low_power_mode = Button::new(self.get_control(id as i32)).is_checked(); } },
                         IDC_CHK_SET_ATTR => { if (code as u32) == BN_CLICKED { self.set_compressed_attr = Button::new(self.get_control(id as i32)).is_checked(); } },
                         IDC_CHK_LOG_ENABLED => {
                              if (code as u32) == BN_CLICKED {
                                  let checked = Button::new(self.get_control(id as i32)).is_checked();
                                  self.log_enabled = checked;
                                  let ids = [IDC_CHK_LOG_ERRORS, IDC_CHK_LOG_WARNS, IDC_CHK_LOG_INFO, IDC_CHK_LOG_TRACE];
                                  for &sub_id in &ids { Button::new(self.get_control(sub_id as i32)).set_enabled(checked); }
                              }
                         },
                         IDC_CHK_LOG_ERRORS | IDC_CHK_LOG_WARNS | IDC_CHK_LOG_INFO | IDC_CHK_LOG_TRACE => {
                              if (code as u32) == BN_CLICKED {
                                  self.log_level_mask = 0;
                                  if Button::new(self.get_control(IDC_CHK_LOG_ERRORS as i32)).is_checked() { self.log_level_mask |= crate::logger::LOG_LEVEL_ERROR; }
                                  if Button::new(self.get_control(IDC_CHK_LOG_WARNS as i32)).is_checked() { self.log_level_mask |= crate::logger::LOG_LEVEL_WARN; }
                                  if Button::new(self.get_control(IDC_CHK_LOG_INFO as i32)).is_checked() { self.log_level_mask |= crate::logger::LOG_LEVEL_INFO; }
                                  if Button::new(self.get_control(IDC_CHK_LOG_TRACE as i32)).is_checked() { self.log_level_mask |= crate::logger::LOG_LEVEL_TRACE; }
                              }
                         },
                         IDC_BTN_RESET_ALL => {
                             if (code as u32) == BN_CLICKED {
                                 // Reset logic
                             }
                         },
                         IDC_BTN_CHECK_UPDATE => {
                             if (code as u32) == BN_CLICKED {
                                 match &self.pending_update {
                                     Some(info) => {
                                         self.update_status = UpdateStatus::Updating;
                                         Button::new(self.get_control(IDC_BTN_CHECK_UPDATE as i32)).set_enabled(false);
                                         Button::new(self.get_control(IDC_BTN_CHECK_UPDATE as i32)).set_text("Updating...");
                                         Label::new(self.get_control(IDC_LBL_UPDATE_STATUS as i32)).set_text("Downloading...");
                                         
                                         let url = info.download_url.clone();
                                         std::thread::spawn(move || {
                                             match crate::updater::download_and_start_update(&url) {
                                                Ok(_) => {
                                                     if let Ok(exe) = std::env::current_exe() {
                                                         let op = crate::utils::to_wstring("open"); 
                                                         let path = crate::utils::to_wstring(exe.to_str().unwrap());
                                                         ShellExecuteW(
                                                             std::ptr::null_mut(),
                                                             op.as_ptr(),
                                                             path.as_ptr(),
                                                             std::ptr::null(),
                                                             std::ptr::null(),
                                                             SW_SHOWNORMAL
                                                         );
                                                         std::process::exit(0);
                                                     }
                                                },
                                                Err(_) => {
                                                }
                                             }
                                         });
                                     },
                                     None => {
                                         self.update_status = UpdateStatus::Checking;
                                         Button::new(self.get_control(IDC_BTN_CHECK_UPDATE as i32)).set_enabled(false);
                                         Label::new(self.get_control(IDC_LBL_UPDATE_STATUS as i32)).set_text("Checking...");
                                         
                                         let hwnd_target = hwnd as usize; 
                                         std::thread::spawn(move || {
                                             let res = crate::updater::check_for_updates();
                                             let ptr = Box::into_raw(Box::new(res));
                                             SendMessageW(hwnd_target as HWND, WM_APP_UPDATE_CHECK_RESULT, 0, ptr as LPARAM);
                                         });
                                     }
                                 }
                             }
                         },
                         IDC_BTN_RESTART_TI => {
                              if (code as u32) == BN_CLICKED && !crate::engine::elevation::is_system_or_ti() {
                                  let _ = crate::engine::elevation::restart_as_trusted_installer();
                              }
                         },
                         _ => {}
                     }
                     Some(0)
                },
                _ => None
            }
        }
    }
}