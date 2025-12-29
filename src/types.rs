#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code)]


// Basic Types
pub type BOOL = i32;
pub type BYTE = u8;
pub type DWORD = u32;
pub type HANDLE = *mut c_void;
pub type HMODULE = HANDLE;
pub type HINSTANCE = HANDLE;
pub type HWND = HANDLE;
pub type HMENU = HANDLE;
pub type HICON = HANDLE;
pub type HBRUSH = HANDLE;
pub type HCURSOR = HANDLE;
pub type HFONT = HANDLE;
pub type HDC = HANDLE;
pub type HGDIOBJ = HANDLE;
pub type HBITMAP = HANDLE;
pub type HKEY = HANDLE;
pub type HDROP = HANDLE;
pub type HGLOBAL = HANDLE;
pub type HRESULT = i32;
pub type LPARAM = isize;
pub type WPARAM = usize;
pub type LRESULT = isize;
pub type LPCSTR = *const u8;
pub type LPCWSTR = *const u16;
pub type LPWSTR = *mut u16;
pub type LPVOID = *mut std::ffi::c_void;
pub type LPDWORD = *mut u32;
pub type COLORREF = u32;
pub type ATOM = u16;
pub use std::ffi::c_void;

pub type WNDPROC = Option<unsafe extern "system" fn(hwnd: HWND, uMsg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT>;


#[repr(C)]
#[derive(Clone, Copy)]
pub struct NMLVCUSTOMDRAW {
    pub nmcd: NMCUSTOMDRAW,
    pub clr_text: u32,
    pub clr_text_bk: u32,
    pub i_sub_item: i32,
    pub dw_item_type: u32,
    pub clr_face: u32,
    pub i_icon_effect: i32,
    pub i_icon_phase: i32,
    pub i_part_id: i32,
    pub i_state_id: i32,
    pub rc_text: RECT,
    pub u_align: u32,
}

#[repr(C)]
pub struct LOGFONTW {
    pub lfHeight: i32,
    pub lfWidth: i32,
    pub lfEscapement: i32,
    pub lfOrientation: i32,
    pub lfWeight: i32,
    pub lfItalic: u8,
    pub lfUnderline: u8,
    pub lfStrikeOut: u8,
    pub lfCharSet: u8,
    pub lfOutPrecision: u8,
    pub lfClipPrecision: u8,
    pub lfQuality: u8,
    pub lfPitchAndFamily: u8,
    pub lfFaceName: [u16; 32],
}

// Constants
pub const FALSE: BOOL = 0;
pub const TRUE: BOOL = 1;

pub const S_OK: HRESULT = 0;
pub const ERROR_MORE_DATA: u32 = 234;
pub const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

// Structs

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SIZE {
    pub cx: i32,
    pub cy: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: u32,
    pub pt: POINT,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WNDCLASSW {
    pub style: u32,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: LPCWSTR,
    pub lpszClassName: LPCWSTR,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FILETIME {
    pub dwLowDateTime: u32,
    pub dwHighDateTime: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SYSTEMTIME {
    pub wYear: u16,
    pub wMonth: u16,
    pub wDayOfWeek: u16,
    pub wDay: u16,
    pub wHour: u16,
    pub wMinute: u16,
    pub wSecond: u16,
    pub wMilliseconds: u16,
}

// Common Constants needed for UI
pub const WS_EX_LAYERED: u32 = 0x00080000;
pub const WS_EX_CONTROLPARENT: u32 = 0x00010000;
pub const WS_EX_TOOLWINDOW: u32 = 0x00000080;
pub const WS_OVERLAPPED: u32 = 0x00000000;
pub const WS_CAPTION: u32 = 0x00C00000;
pub const WS_SYSMENU: u32 = 0x00080000;
pub const WS_THICKFRAME: u32 = 0x00040000;
pub const WS_MINIMIZEBOX: u32 = 0x00020000;
pub const WS_MAXIMIZEBOX: u32 = 0x00010000;
pub const WS_OVERLAPPEDWINDOW: u32 = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
pub const WS_VISIBLE: u32 = 0x10000000;
pub const WS_CHILD: u32 = 0x40000000;
pub const WS_BORDER: u32 = 0x00800000;
pub const WS_TABSTOP: u32 = 0x00010000;
pub const BS_DEFPUSHBUTTON: u32 = 0x00000001;
pub const BS_AUTOCHECKBOX: u32 = 0x00000003;

pub const CW_USEDEFAULT: i32 = -2147483648; // 0x80000000 as i32

pub const SW_SHOW: i32 = 5;
pub const SW_HIDE: i32 = 0;

pub const WM_CREATE: u32 = 0x0001;
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_SIZE: u32 = 0x0005;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_CHAR: u32 = 0x0102;
pub const WM_COMMAND: u32 = 0x0111;
pub const WM_CTLCOLORSTATIC: u32 = 0x0138;
pub const WM_MOUSEMOVE: u32 = 0x0200;
pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_LBUTTONUP: u32 = 0x0202;
pub const WM_NOTIFY: u32 = 0x004E;

pub const GWL_STYLE: i32 = -16;
pub const GWL_EXSTYLE: i32 = -20;
pub const GWLP_USERDATA: i32 = -21;

#[link(name = "uxtheme")]
unsafe extern "system" {
    pub fn SetWindowTheme(hwnd: HWND, pszSubAppName: LPCWSTR, pszSubIdList: LPCWSTR) -> HRESULT;
}

#[link(name = "dwmapi")]
unsafe extern "system" {
    pub fn DwmSetWindowAttribute(hwnd: HWND, dwAttribute: u32, pvAttribute: *const c_void, cbAttribute: u32) -> HRESULT;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    pub fn RegOpenKeyExW(hKey: HKEY, lpSubKey: LPCWSTR, ulOptions: u32, samDesired: u32, phkResult: *mut HKEY) -> i32;
    pub fn RegQueryValueExW(hKey: HKEY, lpValueName: LPCWSTR, lpReserved: *mut u32, lpType: *mut u32, lpData: *mut u8, lpcbData: *mut u32) -> i32;
    pub fn RegCloseKey(hKey: HKEY) -> i32;
}

#[repr(C)]
pub struct CREATESTRUCTW {
    pub lpCreateParams: LPVOID,
    pub hInstance: HINSTANCE,
    pub hMenu: HMENU,
    pub hwndParent: HWND,
    pub cy: i32,
    pub cx: i32,
    pub y: i32,
    pub x: i32,
    pub style: i32,
    pub lpszName: LPCWSTR,
    pub lpszClass: LPCWSTR,
    pub dwExStyle: u32,
}

// Menus
pub const TPM_LEFTALIGN: u32 = 0x0000;
pub const TPM_RETURNCMD: u32 = 0x0100;
pub const TPM_RIGHTBUTTON: u32 = 0x0002;
pub const MF_STRING: u32 = 0x00000000;
pub const MF_CHECKED: u32 = 0x00000008;
pub const MF_BYCOMMAND: u32 = 0x00000000;

// Clipboard
pub const CF_HDROP: u32 = 15;

#[repr(C)]
pub struct NMLISTVIEW {
    pub hdr: NMHDR,
    pub iItem: i32,
    pub iSubItem: i32,
    pub uNewState: u32,
    pub uOldState: u32,
    pub uChanged: u32,
    pub ptAction: POINT,
    pub lParam: LPARAM,
}

pub const EN_CHANGE: u32 = 0x0300;

// Button Messages
pub const BM_GETCHECK: u32 = 0x00F0;
pub const BM_SETCHECK: u32 = 0x00F1;

// ComboBox Messages
pub const CB_GETCURSEL: u32 = 0x0147;
pub const CB_SETCURSEL: u32 = 0x014E;
pub const CB_ADDSTRING: u32 = 0x0143;
pub const CB_RESETCONTENT: u32 = 0x014B;

// ProgressBar Messages
pub const PBM_SETRANGE32: u32 = 0x0406;
pub const PBM_SETPOS: u32 = 0x0402;

// Trackbar Messages
pub const TBM_GETPOS: u32 = 0x0400;
pub const TBM_SETPOS: u32 = 0x0405;
pub const TBM_SETRANGE: u32 = 0x0406;

// Custom Draw
pub const CDDS_PREPAINT: u32 = 0x00000001;
pub const CDDS_ITEM: u32 = 0x00010000;
pub const CDDS_ITEMPREPAINT: u32 = CDDS_ITEM | CDDS_PREPAINT;
pub const CDRF_NEWFONT: u32 = 0x00000002;
pub const CDRF_NOTIFYITEMDRAW: u32 = 0x00000020;
pub const CDDS_SUBITEM: u32 = 0x00020000;

// ListView Messages & Structs
pub const LVM_DELETECOLUMN: u32 = 0x101C;
pub const LVM_GETSELECTIONMARK: u32 = 0x1042;

// ListView Flags
pub const LVCF_FMT: u32 = 0x0001;
pub const LVCF_WIDTH: u32 = 0x0002;
pub const LVCF_TEXT: u32 = 0x0004;
pub const LVCF_SUBITEM: u32 = 0x0008;

pub const LVCFMT_LEFT: i32 = 0x0000;
pub const LVCFMT_RIGHT: i32 = 0x0001;
pub const LVCFMT_CENTER: i32 = 0x0002;
pub const LVCFMT_JUSTIFYMASK: i32 = 0x0003;

pub const LVIF_TEXT: u32 = 0x0001;
pub const LVIF_IMAGE: u32 = 0x0002;
pub const LVIF_PARAM: u32 = 0x0004;
pub const LVIF_STATE: u32 = 0x0008;

// GDI
pub const TRANSPARENT: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NMCUSTOMDRAW {
    pub hdr: NMHDR,
    pub dwDrawStage: u32,
    pub hdc: HDC,
    pub rc: RECT,
    pub dwItemSpec: usize,
    pub uItemState: u32,
    pub lItemlParam: LPARAM,
}



#[repr(C)]
pub struct LVCOLUMNW {
    pub mask: u32,
    pub fmt: i32,
    pub cx: i32,
    pub pszText: LPWSTR,
    pub cchTextMax: i32,
    pub iSubItem: i32,
    pub iImage: i32,
    pub iOrder: i32,
    pub cxMin: i32,
    pub cxDefault: i32,
    pub cxIdeal: i32,
}

#[repr(C)]
pub struct LVITEMW {
    pub mask: u32,
    pub iItem: i32,
    pub iSubItem: i32,
    pub state: u32,
    pub stateMask: u32,
    pub pszText: LPWSTR,
    pub cchTextMax: i32,
    pub iImage: i32,
    pub lParam: LPARAM,
    pub iIndent: i32,
    pub iGroupId: i32,
    pub cColumns: u32,
    pub puColumns: *mut u32,
    pub piColFmt: *mut i32,
    pub iGroup: i32,
}


#[link(name = "gdi32")]
unsafe extern "system" {
    pub fn SetTextColor(hdc: HDC, color: u32) -> u32;

    pub fn SetBkMode(hdc: HDC, mode: i32) -> i32;
    pub fn SetBkColor(hdc: HDC, color: u32) -> u32;
    pub fn CreateSolidBrush(color: u32) -> HBRUSH;
    pub fn GetStockObject(i: i32) -> HGDIOBJ;
    pub fn FillRect(hDC: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    pub fn CreateFontW(cHeight: i32, cWidth: i32, cEscapement: i32, cOrientation: i32, cWeight: i32, bItalic: u32, bUnderline: u32, bStrikeOut: u32, iCharSet: u32, iOutPrecision: u32, iClipPrecision: u32, iQuality: u32, iPitchAndFamily: u32, pszFaceName: LPCWSTR) -> HFONT;
    pub fn DeleteObject(ho: HGDIOBJ) -> BOOL;
    pub fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
    pub fn CreatePen(iStyle: i32, cWidth: i32, color: u32) -> HGDIOBJ;
    pub fn RoundRect(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32, width: i32, height: i32) -> BOOL;
}

// File Operations
pub const MOVEFILE_REPLACE_EXISTING: u32 = 0x00000001;

pub const SW_SHOWNORMAL: i32 = 1;

// Window Positioning
pub const SWP_NOSIZE: u32 = 0x0001;
pub const SWP_NOMOVE: u32 = 0x0002;
pub const SWP_NOZORDER: u32 = 0x0004;
pub const SWP_NOACTIVATE: u32 = 0x0010;
pub const SWP_SHOWWINDOW: u32 = 0x0040;
pub const HWND_TOP: HWND = 0 as HWND;

// Window Class Styles
pub const CS_VREDRAW: u32 = 0x0001;
pub const CS_HREDRAW: u32 = 0x0002;
pub const CS_DBLCLKS: u32 = 0x0008;

// Cursor / Icon
pub const IDC_ARROW: LPCWSTR = 32512 as LPCWSTR;
pub const IDI_APPLICATION: LPCWSTR = 32512 as LPCWSTR;

// Window Creation
pub const WM_NCCREATE: u32 = 0x0081;


// Image Loading
pub const IMAGE_ICON: u32 = 1;
pub const LR_DEFAULTSIZE: u32 = 0x0040;
pub const LR_SHARED: u32 = 0x8000;

// System Metrics
pub const SM_CXSCREEN: i32 = 0;
pub const SM_CYSCREEN: i32 = 1;

// Registry
pub const HKEY_CURRENT_USER: HKEY = -2147483647i32 as isize as HANDLE; // 0x80000001
pub const KEY_READ: u32 = 0x20019;

// DWM
pub const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
pub const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;

// GDI
pub const WHITE_BRUSH: i32 = 0;
pub const BS_SOLID: i32 = 0;
pub const PS_SOLID: i32 = 0;
pub const FW_NORMAL: i32 = 400;
pub const DEFAULT_CHARSET: u32 = 1;
pub const OUT_DEFAULT_PRECIS: u32 = 0;
pub const CLIP_DEFAULT_PRECIS: u32 = 0;
pub const DEFAULT_PITCH: u32 = 0;
pub const FF_DONTCARE: u32 = 0;
pub const CLEARTYPE_QUALITY: u32 = 5;
pub const OPAQUE: i32 = 2;

// Window Styles (Button)
pub const BS_GROUPBOX: u32 = 0x00000007;
pub const BS_CHECKBOX: u32 = 0x00000002;

pub const BS_RADIOBUTTON: u32 = 0x00000004;
pub const BS_3STATE: u32 = 0x00000005;
pub const BS_AUTO3STATE: u32 = 0x00000006;
pub const BS_AUTORADIOBUTTON: u32 = 0x00000009;
pub const BS_OWNERDRAW: u32 = 0x0000000B;

// Window Messages
pub const WM_SETFONT: u32 = 0x0030;
pub const WM_CTLCOLORBTN: u32 = 0x0135;
pub const WM_CTLCOLORDLG: u32 = 0x0136;
pub const WM_CTLCOLOREDIT: u32 = 0x0133;
pub const WM_CTLCOLORLISTBOX: u32 = 0x0134;
pub const WM_ERASEBKGND: u32 = 0x0014;
pub const WM_THEMECHANGED: u32 = 0x031A;

// GetWindow Constants
pub const GW_CHILD: u32 = 5;
pub const GW_HWNDNEXT: u32 = 2;

// COM
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}
pub type PCWSTR = *const u16;

pub const CLSCTX_INPROC_SERVER: u32 = 0x1;
pub const CLSCTX_INPROC_HANDLER: u32 = 0x2;
pub const CLSCTX_LOCAL_SERVER: u32 = 0x4;
pub const CLSCTX_REMOTE_SERVER: u32 = 0x10;
pub const CLSCTX_ALL: u32 = CLSCTX_INPROC_SERVER | CLSCTX_INPROC_HANDLER | CLSCTX_LOCAL_SERVER | CLSCTX_REMOTE_SERVER;

pub const CLSID_TaskbarList: GUID = GUID { data1: 0x56FDF344, data2: 0xFD6D, data3: 0x11d0, data4: [0x95, 0x8A, 0x00, 0x60, 0x97, 0xC9, 0xA0, 0x90] };

// Taskbar
pub type TBPFLAG = u32;
pub const TBPF_NOPROGRESS: u32 = 0;
pub const TBPF_INDETERMINATE: u32 = 0x1;
pub const TBPF_NORMAL: u32 = 0x2;
pub const TBPF_ERROR: u32 = 0x4;
pub const TBPF_PAUSED: u32 = 0x8;

// Text drawing
pub const DT_CENTER: u32 = 0x00000001;
pub const DT_VCENTER: u32 = 0x00000004;
pub const DT_SINGLELINE: u32 = 0x00000020;

// Owner Draw
pub const ODS_SELECTED: u32 = 0x0001;
pub const ODS_DISABLED: u32 = 0x0004;

// Static Control Styles
pub const SS_CENTER: u32 = 0x00000001;

// Edit Control Styles
pub const ES_CENTER: u32 = 0x0001;
pub const ES_MULTILINE: u32 = 0x0004;
pub const ES_AUTOVSCROLL: u32 = 0x0040;
pub const ES_NUMBER: u32 = 0x2000;

// ListView Styles
pub const LVS_REPORT: u32 = 0x0001;
pub const LVS_SINGLESEL: u32 = 0x0004;
pub const LVS_SHOWSELALWAYS: u32 = 0x0008;

// ListView Extended Styles
pub const LVS_EX_FULLROWSELECT: u32 = 0x00000020;
pub const LVS_EX_DOUBLEBUFFER: u32 = 0x00010000;

// ListView Messages
pub const LVM_GETITEMSTATE: u32 = 0x102C;
pub const LVIS_SELECTED: u32 = 0x0002;

#[repr(C)]
pub struct MINMAXINFO {
    pub ptReserved: POINT,
    pub ptMaxSize: POINT,
    pub ptMaxPosition: POINT,
    pub ptMinTrackSize: POINT,
    pub ptMaxTrackSize: POINT,
}

pub const EM_GETSEL: u32 = 0x00B0;
pub const EM_SETSEL: u32 = 0x00B1;
pub const EM_REPLACESEL: u32 = 0x00C2;
pub const WM_PASTE: u32 = 0x0302;

pub const GWLP_WNDPROC: i32 = -4;

// ProgressBar
pub const PROGRESS_CLASSW: *const u16 = crate::w!("msctls_progress32").as_ptr();
pub const PBS_SMOOTH: u32 = 0x01;
pub const PBM_SETBARCOLOR: u32 = 0x0409;
pub const PBM_SETBKCOLOR: u32 = 0x2001;

pub const HDN_FIRST: u32 = (-300i32) as u32;
pub const HDN_BEGINTRACKW: u32 = ((-300i32) - 26) as u32;
pub const HDN_BEGINTRACKA: u32 = ((-300i32) - 6) as u32;
pub const HDN_DIVIDERDBLCLICKW: u32 = ((-300i32) - 25) as u32;
pub const HDN_DIVIDERDBLCLICKA: u32 = ((-300i32) - 5) as u32;

// ListView & Header Constants
pub const LVM_FIRST: u32 = 0x1000;
pub const LVM_GETITEMCOUNT: u32 = LVM_FIRST + 4;
pub const LVM_SETITEMSTATE: u32 = LVM_FIRST + 43;
pub const LVM_DELETEITEM: u32 = LVM_FIRST + 8;
pub const LVM_DELETEALLITEMS: u32 = LVM_FIRST + 9;
pub const LVM_GETHEADER: u32 = LVM_FIRST + 31;
pub const LVM_GETNEXTITEM: u32 = LVM_FIRST + 12;
pub const LVM_INSERTCOLUMNW: u32 = LVM_FIRST + 97;
pub const LVM_INSERTITEMW: u32 = LVM_FIRST + 77;
pub const LVM_SETBKCOLOR: u32 = LVM_FIRST + 1;
pub const LVM_SETEXTENDEDLISTVIEWSTYLE: u32 = LVM_FIRST + 54;
pub const LVM_SETITEMW: u32 = LVM_FIRST + 76;
pub const LVM_SETTEXTBKCOLOR: u32 = LVM_FIRST + 38;
pub const LVM_SETTEXTCOLOR: u32 = LVM_FIRST + 36;
pub const LVM_GETCOLUMNWIDTH: u32 = 0x101D;
pub const LVM_SETCOLUMNWIDTH: u32 = 0x101E;
pub const LVM_SORTITEMS: u32 = LVM_FIRST + 48;
pub const LVM_GETITEMTEXTW: u32 = LVM_FIRST + 115;
pub const LVM_FINDITEMW: u32 = LVM_FIRST + 83;
pub const LVM_GETSUBITEMRECT: u32 = LVM_FIRST + 56;

pub const LVNI_SELECTED: u32 = 0x0002;
pub const LVFI_PARAM: u32 = 0x0001;
pub const LVIR_BOUNDS: u32 = 0;

// Custom Draw (Merged above)
pub const CDRF_NOTIFYSUBITEMDRAW: u32 = 0x00000020;

// Header
pub const HDI_FORMAT: u32 = 0x0004;
pub const HDF_SORTUP: i32 = 0x0400;
pub const HDF_SORTDOWN: i32 = 0x0200;
pub const HDM_GETITEMW: u32 = 0x1200 + 11;
pub const HDM_SETITEMW: u32 = 0x1200 + 12;

// Window Messages

pub const WM_NCPAINT: u32 = 0x0085;
pub const WM_NCCALCSIZE: u32 = 0x0083;

pub const WS_CLIPSIBLINGS: u32 = 0x04000000;

// Styles
pub const WS_DLGFRAME: u32 = 0x00400000;
pub const WS_EX_CLIENTEDGE: u32 = 0x00000200;
pub const WS_EX_STATICEDGE: u32 = 0x00020000;
pub const SWP_FRAMECHANGED: u32 = 0x0020;

pub const COLOR_WINDOW: u32 = 5;

// MessageBox
pub const MB_OK: u32 = 0x00000000;
pub const MB_YESNO: u32 = 0x00000004;
pub const MB_ICONERROR: u32 = 0x00000010;
pub const MB_ICONWARNING: u32 = 0x00000030;
pub const MB_ICONINFORMATION: u32 = 0x00000040;
pub const IDNO: i32 = 7;

// Window Messages
pub const WM_COPYDATA: u32 = 0x004A;
pub const WM_SETTINGCHANGE: u32 = 0x001A;
pub const WM_DROPFILES: u32 = 0x0233;
pub const WM_CONTEXTMENU: u32 = 0x007B;
pub const WM_TIMER: u32 = 0x0113;
pub const WM_DRAWITEM: u32 = 0x002B;
pub const WM_LBUTTONDBLCLK: u32 = 0x0203;
pub const WM_HSCROLL: u32 = 0x0114;
pub const WM_GETMINMAXINFO: u32 = 0x0024;
pub const WM_INITDIALOG: u32 = 0x0110;
pub const WM_APP_UPDATE_CHECK_RESULT: u32 = WM_USER + 1; // Assuming WM_USER is defined or I need to define it.
pub const WM_APP_SHORTCUT: u32 = WM_USER + 900;
pub const WM_USER: u32 = 0x0400;

// Button, ComboBox, Edit Notifications & Styles
pub const BN_CLICKED: u32 = 0;
pub const CBN_SELCHANGE: u32 = 1;
pub const ES_READONLY: u32 = 0x0800;
pub const BS_PUSHBUTTON: u32 = 0x00000000;
pub const CBS_DROPDOWNLIST: u32 = 0x0003;
pub const CBS_HASSTRINGS: u32 = 0x0200;
pub const EM_SETLIMITTEXT: u32 = 0x00C5;

// GDI & Fonts
pub const DEFAULT_GUI_FONT: i32 = 17;
pub const FW_LIGHT: i32 = 300;
pub const FW_BOLD: i32 = 700;
pub const LR_DEFAULTCOLOR: u32 = 0x0000;
pub const STM_SETICON: u32 = 0x0170;

// Window Styles & Commands
pub const WS_POPUP: u32 = 0x80000000;
pub const WS_VSCROLL: u32 = 0x00200000;
pub const SW_RESTORE: i32 = 9;
pub const IDYES: i32 = 6;
pub const MSGFLT_ADD: u32 = 1;
pub const VK_CONTROL: u16 = 0x11;
pub const VK_SHIFT: u16 = 0x10;
pub const VK_DELETE: u16 = 0x2E;

// Common Controls & Notifications
pub const ICC_WIN95_CLASSES: u32 = 0x000000FF;
pub const ICC_STANDARD_CLASSES: u32 = 0x00004000;

pub const NM_CLICK: u32 = 0xFFFFFFFE; // -2
pub const NM_DBLCLK: u32 = 0xFFFFFFFD; // -3
pub const NM_RCLICK: u32 = 0xFFFFFFFB; // -5
pub const NM_CUSTOMDRAW: u32 = 0xFFFFFFF4; // -12

pub const LVN_FIRST: u32 = 0xFFFFFF00; // -100 (approx, casting issues usually)
pub const LVN_ITEMCHANGED: u32 = 0xFFFFFF95; // -101
pub const LVN_KEYDOWN: u32 = 0xFFFFFF9A; // -156
pub const LVN_COLUMNCLICK: u32 = 0xFFFFFF94; // -108

// Message Filter
pub const MSGFLT_ALLOW: u32 = 1;

// Flash Window
pub const FLASHW_STOP: u32 = 0;
pub const FLASHW_CAPTION: u32 = 0x00000001;
pub const FLASHW_TRAY: u32 = 0x00000002;
pub const FLASHW_ALL: u32 = FLASHW_CAPTION | FLASHW_TRAY;
pub const FLASHW_TIMER: u32 = 0x00000004;
pub const FLASHW_TIMERNOFG: u32 = 0x0000000C;

// Memory
pub const GMEM_MOVEABLE: u32 = 0x0002;

// Removed duplicates (LVCOLUMNW, LVITEMW) logic...
// Re-adding impl Default matching earlier structs:

impl Default for LVCOLUMNW {
    fn default() -> Self { unsafe { std::mem::zeroed() } }
}

impl Default for LVITEMW {
    fn default() -> Self { unsafe { std::mem::zeroed() } }
}

#[repr(C)]
pub struct LVFINDINFOW {
    pub flags: u32,
    pub psz: LPCWSTR,
    pub lParam: isize,
    pub pt: POINT,
    pub vkDirection: u32,
}

// Shell Functions
pub type SUBCLASSPROC = Option<unsafe extern "system" fn(hWnd: HWND, uMsg: u32, wParam: WPARAM, lParam: LPARAM, uIdSubclass: usize, dwRefData: usize) -> LRESULT>;

#[link(name = "comctl32")]
unsafe extern "system" {
    pub fn SetWindowSubclass(hWnd: HWND, pfnSubclass: SUBCLASSPROC, uIdSubclass: usize, dwRefData: usize) -> BOOL;
    pub fn DefSubclassProc(hWnd: HWND, uMsg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn RemoveWindowSubclass(hWnd: HWND, pfnSubclass: SUBCLASSPROC, uIdSubclass: usize) -> BOOL;
    pub fn InitCommonControlsEx(picce: *const INITCOMMONCONTROLSEX) -> BOOL;
}

#[repr(C)]
pub struct INITCOMMONCONTROLSEX {
    pub dwSize: u32,
    pub dwICC: u32,
}

#[repr(C)]
pub struct FLASHWINFO {
    pub cbSize: u32,
    pub hwnd: HWND,
    pub dwFlags: u32,
    pub uCount: u32,
    pub dwTimeout: u32,
}

#[repr(C)]
pub struct DRAWITEMSTRUCT {
    pub CtlType: u32,
    pub CtlID: u32,
    pub itemID: u32,
    pub itemAction: u32,
    pub itemState: u32,
    pub hwndItem: HWND,
    pub hDC: HDC,
    pub rcItem: RECT,
    pub itemData: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NMHDR {
    pub hwndFrom: HWND,
    pub idFrom: usize,
    pub code: u32, 
}

#[repr(C)]
pub struct NMITEMACTIVATE {
    pub hdr: NMHDR,
    pub iItem: i32,
    pub iSubItem: i32,
    pub uNewState: u32,
    pub uOldState: u32,
    pub uChanged: u32,
    pub ptAction: POINT,
    pub lParam: LPARAM,
    pub iKeyFlags: u32,
}

#[repr(C)]
pub struct NMLVKEYDOWN {
    pub hdr: NMHDR,
    pub wVKey: u16,
    pub flags: u32,
}

pub const COINIT_APARTMENTTHREADED: u32 = 0x2;

#[repr(C)]
pub struct COPYDATASTRUCT {
    pub dwData: usize,
    pub cbData: u32,
    pub lpData: *mut c_void,
}

// --- PE / Loader Definitions ---

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
    pub AddressOfFunctions: u32,
    pub AddressOfNames: u32,
    pub AddressOfNameOrdinals: u32,
}

// --- COM VTable Definitions ---

#[repr(C)]
pub struct IFileOpenDialogVtbl {
    pub query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    pub release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub show: unsafe extern "system" fn(*mut c_void, isize) -> HRESULT,
    pub set_file_types: unsafe extern "system" fn(*mut c_void, u32, *const c_void) -> HRESULT,
    pub set_file_type_index: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
    pub get_file_type_index: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    pub advise: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut u32) -> HRESULT,
    pub unadvise: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
    pub set_options: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
    pub get_options: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    pub set_default_folder: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    pub set_folder: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    pub get_folder: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub get_current_selection: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub set_file_name: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub get_file_name: unsafe extern "system" fn(*mut c_void, *mut PCWSTR) -> HRESULT,
    pub set_title: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub set_ok_button_label: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub set_file_name_label: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub get_result: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT, 
    pub add_place: unsafe extern "system" fn(*mut c_void, *mut c_void, u32) -> HRESULT,
    pub set_default_extension: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    pub close: unsafe extern "system" fn(*mut c_void, HRESULT) -> HRESULT,
    pub set_client_guid: unsafe extern "system" fn(*mut c_void, *const GUID) -> HRESULT,
    pub clear_client_data: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    pub set_filter: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    pub get_results: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub get_selected_items: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}

#[repr(C)]
pub struct IShellItemVtbl {
    pub query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    pub release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub bind_to_handler: unsafe extern "system" fn(*mut c_void, *mut c_void, *const GUID, *const GUID, *mut *mut c_void) -> HRESULT,
    pub get_parent: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub get_display_name: unsafe extern "system" fn(*mut c_void, u32, *mut PCWSTR) -> HRESULT,
    pub get_attributes: unsafe extern "system" fn(*mut c_void, u32, *mut u32) -> HRESULT,
    pub compare: unsafe extern "system" fn(*mut c_void, *mut c_void, u32, *mut i32) -> HRESULT,
}

#[repr(C)]
pub struct IShellItemArrayVtbl {
    pub query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    pub release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub bind_to_handler: unsafe extern "system" fn(*mut c_void, *mut c_void, *const GUID, *const GUID, *mut *mut c_void) -> HRESULT,
    pub get_property_store: unsafe extern "system" fn(*mut c_void, u32, *const GUID, *mut *mut c_void) -> HRESULT,
    pub get_property_description_list: unsafe extern "system" fn(*mut c_void, *const GUID, *const GUID, *mut *mut c_void) -> HRESULT,
    pub get_attributes: unsafe extern "system" fn(*mut c_void, u32, u32, *mut c_void) -> HRESULT,
    pub get_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    pub get_item_at: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
    pub enum_items: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}

#[repr(C)]
pub struct ITaskbarList3 {
    pub lp_vtbl: *const ITaskbarList3Vtbl,
}

#[repr(C)]
pub struct ITaskbarList3Vtbl {
    pub query_interface: unsafe extern "system" fn(*mut ITaskbarList3, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut ITaskbarList3) -> u32,
    pub release: unsafe extern "system" fn(*mut ITaskbarList3) -> u32,
    pub hr_init: unsafe extern "system" fn(*mut ITaskbarList3) -> HRESULT,
    pub add_tab: unsafe extern "system" fn(*mut ITaskbarList3, HWND) -> HRESULT,
    pub delete_tab: unsafe extern "system" fn(*mut ITaskbarList3, HWND) -> HRESULT,
    pub activate_tab: unsafe extern "system" fn(*mut ITaskbarList3, HWND) -> HRESULT,
    pub set_active_alt: unsafe extern "system" fn(*mut ITaskbarList3, HWND) -> HRESULT,
    pub mark_fullscreen_window: unsafe extern "system" fn(*mut ITaskbarList3, HWND, BOOL) -> HRESULT,
    pub set_progress_value: unsafe extern "system" fn(*mut ITaskbarList3, HWND, u64, u64) -> HRESULT,
    pub set_progress_state: unsafe extern "system" fn(*mut ITaskbarList3, HWND, TBPFLAG) -> HRESULT,
}

#[repr(C)]
pub struct IShellLinkWVtbl {
    pub QueryInterface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(*mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub GetPath: unsafe extern "system" fn(*mut c_void, LPCWSTR, i32, *mut c_void, u32) -> HRESULT,
}

#[repr(C)]
pub struct IPersistFileVtbl {
    pub QueryInterface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(*mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub GetClassID: unsafe extern "system" fn(*mut c_void, *mut GUID) -> HRESULT,
    pub IsDirty: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    pub Load: unsafe extern "system" fn(*mut c_void, LPCWSTR, u32) -> HRESULT,
}

// --- System & Service Definitions ---

#[repr(C)]
pub struct SERVICE_STATUS_PROCESS {
    pub dwServiceType: u32,
    pub dwCurrentState: u32,
    pub dwControlsAccepted: u32,
    pub dwWin32ExitCode: u32,
    pub dwServiceSpecificExitCode: u32,
    pub dwCheckPoint: u32,
    pub dwWaitHint: u32,
    pub dwProcessId: u32,
    pub dwServiceFlags: u32,
}



#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct WOF_EXTERNAL_INFO {
    pub version: u32,
    pub provider: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FILE_PROVIDER_EXTERNAL_INFO_V1 {
    pub version: u32,
    pub algorithm: u32,
    pub flags: u32,
}



// --- Constants & GUIDs for COM ---
pub const CLSID_FILE_OPEN_DIALOG: GUID = GUID { data1: 0xDC1C5A9C, data2: 0xE88A, data3: 0x4DDE, data4: [0xA5, 0xA1, 0x60, 0xF8, 0x2A, 0x20, 0xAE, 0xF7] };
pub const IID_IFILE_OPEN_DIALOG: GUID = GUID { data1: 0xd57c7288, data2: 0xd4ad, data3: 0x4768, data4: [0xbe, 0x02, 0x9d, 0x96, 0x95, 0x32, 0xd9, 0x60] };
pub const CLSID_SHELL_LINK: GUID = GUID { data1: 0x00021401, data2: 0x0000, data3: 0x0000, data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46] };
pub const IID_ISHELL_LINK_W: GUID = GUID { data1: 0x000214F9, data2: 0x0000, data3: 0x0000, data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46] };
pub const IID_IPERSIST_FILE: GUID = GUID { data1: 0x0000010b, data2: 0x0000, data3: 0x0000, data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46] };
pub const IID_ITASKBAR_LIST3: GUID = GUID { data1: 0xea1afb91, data2: 0x9e28, data3: 0x4b86, data4: [0x90, 0xe9, 0x9e, 0x9f, 0x8a, 0x5e, 0xef, 0xaf] };


pub const FOS_PICKFOLDERS: u32 = 0x20;
pub const FOS_FORCEFILESYSTEM: u32 = 0x40;
pub const FOS_ALLOWMULTISELECT: u32 = 0x200;
pub const SIGDN_FILESYSPATH: u32 = 0x80058000;





// External Functions
#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn GetModuleHandleW(lpModuleName: LPCWSTR) -> HMODULE;
    pub fn GetModuleFileNameW(hModule: HMODULE, lpFilename: LPWSTR, nSize: u32) -> u32;
    pub fn GetLastError() -> u32;
    pub fn DeleteFileW(lpFileName: LPCWSTR) -> BOOL;
    pub fn MoveFileExW(lpExistingFileName: LPCWSTR, lpNewFileName: LPCWSTR, dwFlags: u32) -> BOOL;
}

#[link(name = "user32")]
unsafe extern "system" {
    pub fn MessageBoxW(hWnd: HWND, lpText: LPCWSTR, lpCaption: LPCWSTR, uType: u32) -> i32;
    pub fn GetDlgItem(hDlg: HWND, nIDDlgItem: i32) -> HWND;
    pub fn FindWindowW(lpClassName: LPCWSTR, lpWindowName: LPCWSTR) -> HWND;
    pub fn SendMessageW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn SetWindowTextW(hWnd: HWND, lpString: LPCWSTR) -> BOOL;
    pub fn GetWindowTextW(hWnd: HWND, lpString: LPWSTR, nMaxCount: i32) -> i32;
    pub fn GetWindowTextLengthW(hWnd: HWND) -> i32;
    pub fn GetParent(hWnd: HWND) -> HWND;
    pub fn DestroyWindow(hWnd: HWND) -> BOOL;
    pub fn SetPropW(hWnd: HWND, lpString: LPCWSTR, hData: HANDLE) -> BOOL;
    pub fn GetPropW(hWnd: HWND, lpString: LPCWSTR) -> HANDLE;
    pub fn RemovePropW(hWnd: HWND, lpString: LPCWSTR) -> HANDLE;
    pub fn EnableWindow(hWnd: HWND, bEnable: BOOL) -> BOOL;
    pub fn InvalidateRect(hWnd: HWND, lpRect: *const RECT, bErase: BOOL) -> BOOL;
    pub fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
    pub fn GetObjectW(h: HANDLE, c: i32, pv: LPVOID) -> i32;
    pub fn CreateFontIndirectW(lplf: *const LOGFONTW) -> HFONT;
    pub fn GetWindowRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
    pub fn AdjustWindowRect(lpRect: *mut RECT, dwStyle: u32, bMenu: BOOL) -> BOOL;
    pub fn SetForegroundWindow(hWnd: HWND) -> BOOL;
    pub fn GetForegroundWindow() -> HWND;
    pub fn BringWindowToTop(hWnd: HWND) -> BOOL;
    pub fn MoveWindow(hWnd: HWND, X: i32, Y: i32, nWidth: i32, nHeight: i32, bRepaint: BOOL) -> BOOL;
    pub fn ChangeWindowMessageFilter(message: u32, dwFlag: u32) -> BOOL;
    pub fn SetTimer(hWnd: HWND, nIDEvent: usize, uElapse: u32, lpTimerFunc: Option<unsafe extern "system" fn(HWND, u32, usize, u32)>) -> usize;
    pub fn KillTimer(hWnd: HWND, uIDEvent: usize) -> BOOL;
    pub fn ChangeWindowMessageFilterEx(hwnd: HWND, message: u32, action: u32, pChangeFilterStruct: *mut c_void) -> BOOL;
    pub fn GetKeyState(nVirtKey: i32) -> i16;
    pub fn GetSystemMetrics(nIndex: i32) -> i32;
    pub fn SetWindowPos(hWnd: HWND, hWndInsertAfter: HWND, X: i32, Y: i32, cx: i32, cy: i32, uFlags: u32) -> BOOL;
    pub fn GetMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: u32, wMsgFilterMax: u32) -> BOOL;
    pub fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    pub fn DefWindowProcW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn CallWindowProcW(lpPrevWndFunc: WNDPROC, hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn PostQuitMessage(nExitCode: i32);
    pub fn RegisterClassW(lpWndClass: *const WNDCLASSW) -> ATOM;
    pub fn CreateWindowExW(dwExStyle: u32, lpClassName: LPCWSTR, lpWindowName: LPCWSTR, dwStyle: u32, X: i32, Y: i32, nWidth: i32, nHeight: i32, hWndParent: HWND, hMenu: HMENU, hInstance: HINSTANCE, lpParam: LPVOID) -> HWND;
    pub fn IsDialogMessageW(hDlg: HWND, lpMsg: *const MSG) -> BOOL;
    pub fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: LPCWSTR) -> HCURSOR;
    pub fn LoadIconW(hInstance: HINSTANCE, lpIconName: LPCWSTR) -> HICON;
    pub fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
    pub fn UpdateWindow(hWnd: HWND) -> BOOL;
    pub fn PeekMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: u32, wMsgFilterMax: u32, wRemoveMsg: u32) -> BOOL;
    pub fn wsprintfW(output: LPWSTR, format: LPCWSTR, ...) -> i32;
    
    pub fn SetWindowLongPtrW(hWnd: HWND, nIndex: i32, dwNewLong: isize) -> isize;
    pub fn GetWindowLongPtrW(hWnd: HWND, nIndex: i32) -> isize;
    pub fn GetWindow(hWnd: HWND, uCmd: u32) -> HWND;
    pub fn GetClassNameW(hWnd: HWND, lpClassName: LPWSTR, nMaxCount: i32) -> i32;
    pub fn GetWindowLongW(hWnd: HWND, nIndex: i32) -> i32;
    pub fn SetWindowLongW(hWnd: HWND, nIndex: i32, dwNewLong: i32) -> i32;
    pub fn DrawTextW(hdc: HDC, lpchText: LPCWSTR, cchText: i32, lprc: *mut RECT, format: u32) -> i32;
    pub fn LoadLibraryW(lpLibFileName: LPCWSTR) -> HMODULE;
    pub fn GetProcAddress(hModule: HMODULE, lpProcName: *const u8) -> Option<unsafe extern "system" fn() -> isize>; 
    
    pub fn LoadImageW(hInst: HINSTANCE, name: LPCWSTR, type_: u32, cx: i32, cy: i32, fuLoad: u32) -> HANDLE;
    pub fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> HGLOBAL;
    pub fn GlobalLock(hMem: HGLOBAL) -> LPVOID;
    pub fn GlobalUnlock(hMem: HGLOBAL) -> BOOL;
    
    // Menus
    pub fn CreatePopupMenu() -> HMENU;
    pub fn DestroyMenu(hMenu: HMENU) -> BOOL;
    pub fn AppendMenuW(hMenu: HMENU, uFlags: u32, uIDNewItem: usize, lpNewItem: LPCWSTR) -> BOOL;
    pub fn TrackPopupMenu(hMenu: HMENU, uFlags: u32, x: i32, y: i32, nReserved: i32, hWnd: HWND, prcRect: *const RECT) -> BOOL;
    pub fn CheckMenuItem(hMenu: HMENU, uIDCheckItem: u32, uCheck: u32) -> u32;
    
    // Clipboard
    pub fn OpenClipboard(hWnd: HWND) -> BOOL;
    pub fn CloseClipboard() -> BOOL;
    pub fn EmptyClipboard() -> BOOL;
    pub fn SetClipboardData(uFormat: u32, hMem: HANDLE) -> HANDLE;
    pub fn GetClipboardData(uFormat: u32) -> HANDLE;
    pub fn IsClipboardFormatAvailable(format: u32) -> BOOL;
    
    // Misc
    pub fn GetCursorPos(lpPoint: *mut POINT) -> BOOL;
    pub fn ScreenToClient(hWnd: HWND, lpPoint: *mut POINT) -> BOOL;
    
    pub fn GetWindowThreadProcessId(hWnd: HWND, lpdwProcessId: *mut u32) -> u32;
    pub fn AttachThreadInput(idAttach: u32, idAttachTo: u32, fAttach: BOOL) -> BOOL;
    pub fn FlashWindowEx(pfwi: *const FLASHWINFO) -> BOOL;
}

#[link(name = "shlwapi")]
unsafe extern "system" {
    pub fn StrFormatByteSizeW(qdw: i64, pszBuf: LPWSTR, cchBuf: u32) -> LPWSTR;
}

#[link(name = "shell32")]
unsafe extern "system" {
    pub fn ShellExecuteW(hwnd: HWND, lpOperation: LPCWSTR, lpFile: LPCWSTR, lpParameters: LPCWSTR, lpDirectory: LPCWSTR, nShowCmd: i32) -> HINSTANCE;
    pub fn IsUserAnAdmin() -> BOOL;
    pub fn DragAcceptFiles(hWnd: HWND, fAccept: BOOL);
    pub fn DragQueryFileW(hDrop: HANDLE, iFile: u32, lpszFile: LPWSTR, cch: u32) -> u32;
    pub fn DragFinish(hDrop: HANDLE);
}

#[link(name = "ole32")]
unsafe extern "system" {
    pub fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: u32) -> HRESULT;
    pub fn CoUninitialize();
    pub fn CoCreateInstance(rclsid: *const GUID, pUnkOuter: *mut c_void, dwClsContext: u32, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT;
    pub fn CoTaskMemFree(pv: *mut c_void);
}

// --- File Finding ---
pub const FindExInfoBasic: u32 = 0;
pub const FindExSearchNameMatch: u32 = 0;
pub const FIND_FIRST_EX_LARGE_FETCH: u32 = 2;
pub const FILE_ATTRIBUTE_DIRECTORY: u32 = 16;
pub const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 1024;
pub const FILE_ATTRIBUTE_READONLY: u32 = 1;
pub const FILE_ATTRIBUTE_COMPRESSED: u32 = 0x800; // Added for visual toggle
pub const FILE_ATTRIBUTE_NORMAL: u32 = 128;
pub const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
pub const FILE_SHARE_READ: u32 = 1;
pub const FILE_SHARE_WRITE: u32 = 2;
pub const FILE_SHARE_DELETE: u32 = 4;
pub const OPEN_EXISTING: u32 = 3;
pub const ERROR_ACCESS_DENIED: u32 = 5;
pub const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn FindFirstFileExW(lpFileName: LPCWSTR, fInfoLevelId: u32, lpFindFileData: *mut c_void, fSearchOp: u32, lpSearchFilter: *mut c_void, dwAdditionalFlags: u32) -> HANDLE;
    pub fn FindNextFileW(hFindFile: HANDLE, lpFindFileData: *mut WIN32_FIND_DATAW) -> BOOL;
    pub fn FindClose(hFindFile: HANDLE) -> BOOL;
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct STARTUPINFOW {
    pub cb: u32,
    pub lpReserved: LPWSTR,
    pub lpDesktop: LPWSTR,
    pub lpTitle: LPWSTR,
    pub dwX: u32,
    pub dwY: u32,
    pub dwXSize: u32,
    pub dwYSize: u32,
    pub dwXCountChars: u32,
    pub dwYCountChars: u32,
    pub dwFillAttribute: u32,
    pub dwFlags: u32,
    pub wShowWindow: u16,
    pub cbReserved2: u16,
    pub lpReserved2: *mut u8,
    pub hStdInput: HANDLE,
    pub hStdOutput: HANDLE,
    pub hStdError: HANDLE,
}

pub type LPPROC_THREAD_ATTRIBUTE_LIST = *mut c_void;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct STARTUPINFOEXW {
    pub StartupInfo: STARTUPINFOW,
    pub lpAttributeList: LPPROC_THREAD_ATTRIBUTE_LIST,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PROCESS_INFORMATION {
    pub hProcess: HANDLE,
    pub hThread: HANDLE,
    pub dwProcessId: u32,
    pub dwThreadId: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct WIN32_FIND_DATAW {
    pub dwFileAttributes: u32,
    pub ftCreationTime: FILETIME,
    pub ftLastAccessTime: FILETIME,
    pub ftLastWriteTime: FILETIME,
    pub nFileSizeHigh: u32,
    pub nFileSizeLow: u32,
    pub dwReserved0: u32,
    pub dwReserved1: u32,
    pub cFileName: [u16; 260],
    pub cAlternateFileName: [u16; 14],
}

// --- Registry Constants & Types ---
#[allow(overflowing_literals)]
pub const HKEY_CLASSES_ROOT: HKEY = 0x80000000 as u32 as isize as HANDLE;
pub const KEY_WRITE: u32 = 0x20006;
pub const REG_SZ: u32 = 1;
pub const REG_OPTION_NON_VOLATILE: u32 = 0;
pub const ERROR_SUCCESS: i32 = 0;
pub const MAX_PATH: usize = 260;

// --- COM Constants ---
pub const STGM_READ: u32 = 0x00000000;

// --- Structs ---
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct LUID {
    pub LowPart: u32,
    pub HighPart: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct LUID_AND_ATTRIBUTES {
    pub Luid: LUID,
    pub Attributes: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct TOKEN_PRIVILEGES {
    pub PrivilegeCount: u32,
    pub Privileges: [LUID_AND_ATTRIBUTES; 1],
}

// --- Registry Functions (Advapi32) ---
#[link(name = "advapi32")]
unsafe extern "system" {
    pub fn RegCreateKeyExW(hKey: HKEY, lpSubKey: LPCWSTR, Reserved: u32, lpClass: LPCWSTR, dwOptions: u32, samDesired: u32, lpSecurityAttributes: LPVOID, phkResult: *mut HKEY, lpdwDisposition: LPDWORD) -> i32;
    pub fn RegSetValueExW(hKey: HKEY, lpValueName: LPCWSTR, Reserved: u32, dwType: u32, lpData: *const u8, cbData: u32) -> i32;
    pub fn RegDeleteTreeW(hKey: HKEY, lpSubKey: LPCWSTR) -> i32;
    
    // Token/Privilege Functions
    pub fn OpenProcessToken(ProcessHandle: HANDLE, DesiredAccess: u32, TokenHandle: *mut HANDLE) -> BOOL;
    pub fn LookupPrivilegeValueW(lpSystemName: LPCWSTR, lpName: LPCWSTR, lpLuid: *mut LUID) -> BOOL;
    pub fn AdjustTokenPrivileges(TokenHandle: HANDLE, DisableAllPrivileges: BOOL, NewState: *const TOKEN_PRIVILEGES, BufferLength: u32, PreviousState: *mut TOKEN_PRIVILEGES, ReturnLength: *mut u32) -> BOOL;
    
    pub fn CloseServiceHandle(hSCObject: HANDLE) -> BOOL;
    
    pub fn OpenSCManagerW(lpMachineName: LPCWSTR, lpDatabaseName: LPCWSTR, dwDesiredAccess: u32) -> HANDLE;
    pub fn OpenServiceW(hSCManager: HANDLE, lpServiceName: LPCWSTR, dwDesiredAccess: u32) -> HANDLE;
    pub fn StartServiceW(hService: HANDLE, dwNumServiceArgs: u32, lpServiceArgVectors: *const *const u16) -> BOOL;
    pub fn QueryServiceStatusEx(hService: HANDLE, InfoLevel: u32, lpBuffer: *mut u8, cbBufSize: u32, pcbBytesNeeded: *mut u32) -> BOOL;
    pub fn GetUserNameW(lpBuffer: *mut u16, pcbBuffer: *mut u32) -> BOOL;
}

// Power Management
pub const ES_CONTINUOUS: u32 = 0x80000000;
pub const ES_SYSTEM_REQUIRED: u32 = 0x00000001;

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    pub fn SetThreadExecutionState(esFlags: u32) -> u32;
    pub fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> HANDLE;
    pub fn TerminateProcess(hProcess: HANDLE, uExitCode: u32) -> i32;
    pub fn GetCurrentThread() -> HANDLE;
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn SetThreadPriority(hthread: HANDLE, npriority: i32) -> i32;
    pub fn SetThreadAffinityMask(hthread: HANDLE, dwthreadaffinitymask: usize) -> usize;
    pub fn SetThreadInformation(
        hthread: HANDLE,
        threadinformationclass: u32,
        threadinformation: *const std::ffi::c_void,
        threadinformationsize: u32,
    ) -> i32;
    pub fn SetPriorityClass(hprocess: HANDLE, dwpriorityclass: u32) -> i32;
    pub fn SetProcessInformation(
        hprocess: HANDLE,
        processinformationclass: u32,
        processinformation: *const std::ffi::c_void,
        processinformationsize: u32,
    ) -> i32;
    pub fn GetSystemInfo(lpsysteminfo: *mut SYSTEM_INFO);
}

// Restart Manager
pub const PROCESS_TERMINATE: u32 = 0x0001;
pub const CCH_RM_SESSION_KEY: u32 = 32;
pub const RM_REBOOT_REASON_NONE: u32 = 0;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RM_UNIQUE_PROCESS {
    pub dwProcessId: u32,
    pub ProcessStartTime: FILETIME,
}

impl Default for RM_UNIQUE_PROCESS {
    fn default() -> Self { unsafe { std::mem::zeroed() } }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RM_PROCESS_INFO {
    pub Process: RM_UNIQUE_PROCESS,
    pub strAppName: [u16; 256],
    pub strServiceShortName: [u16; 64],
    pub ApplicationType: u32,
    pub AppStatus: u32,
    pub TSSessionId: u32,
    pub bRestartable: i32,
}

impl Default for RM_PROCESS_INFO {
    fn default() -> Self { unsafe { std::mem::zeroed() } }
}

#[link(name = "rstrtmgr")]
unsafe extern "system" {
    pub fn RmStartSession(pSessionHandle: *mut u32, dwSessionFlags: u32, strSessionKey: *mut u16) -> u32;
    pub fn RmRegisterResources(dwSessionHandle: u32, nFiles: u32, rgsFileNames: *const *const u16, nApplications: u32, rgApplications: *const std::ffi::c_void, nServices: u32, rgsServiceNames: *const *const u16) -> u32;
    pub fn RmGetList(dwSessionHandle: u32, pnProcInfoNeeded: *mut u32, pnProcInfo: *mut u32, rgAffectedApps: *mut RM_PROCESS_INFO, lpdwRebootReasons: *mut u32) -> u32;
    pub fn RmEndSession(dwSessionHandle: u32) -> u32;
}

// Power Throttling / Eco Mode
pub const ProcessPowerThrottling: u32 = 4;
pub const ThreadPowerThrottling: u32 = 1;
pub const THREAD_PRIORITY_IDLE: i32 = -15;
pub const IDLE_PRIORITY_CLASS: u32 = 64;
pub const NORMAL_PRIORITY_CLASS: u32 = 32;

pub const PROCESS_POWER_THROTTLING_CURRENT_VERSION: u32 = 1;
pub const PROCESS_POWER_THROTTLING_EXECUTION_SPEED: u32 = 1;

pub const THREAD_POWER_THROTTLING_CURRENT_VERSION: u32 = 1;
pub const THREAD_POWER_THROTTLING_EXECUTION_SPEED: u32 = 1;

#[repr(C)]
pub struct PROCESS_POWER_THROTTLING_STATE {
    pub Version: u32,
    pub ControlMask: u32,
    pub StateMask: u32,
}

#[repr(C)]
pub struct THREAD_POWER_THROTTLING_STATE {
    pub Version: u32,
    pub ControlMask: u32,
    pub StateMask: u32,
}

#[repr(C)]
pub struct SYSTEM_INFO {
    pub wProcessorArchitecture: u16,
    pub wReserved: u16,
    pub dwPageSize: u32,
    pub lpMinimumApplicationAddress: *mut std::ffi::c_void,
    pub lpMaximumApplicationAddress: *mut std::ffi::c_void,
    pub dwActiveProcessorMask: usize,
    pub dwNumberOfProcessors: u32,
    pub dwProcessorType: u32,
    pub dwAllocationGranularity: u32,
    pub wProcessorLevel: u16,
    pub wProcessorRevision: u16,
}

impl Default for SYSTEM_INFO {
    fn default() -> Self { unsafe { std::mem::zeroed() } }
}

// Additional Kernel32 Functions
#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn CreateFileW(
        lpFileName: LPCWSTR,
        dwDesiredAccess: u32,
        dwShareMode: u32,
        lpSecurityAttributes: *mut c_void,
        dwCreationDisposition: u32,
        dwFlagsAndAttributes: u32,
        hTemplateFile: HANDLE
    ) -> HANDLE;
    pub fn WriteFile(
        hFile: HANDLE,
        lpBuffer: *const c_void,
        nNumberOfBytesToWrite: u32,
        lpNumberOfBytesWritten: *mut u32,
        lpOverlapped: *mut c_void
    ) -> BOOL;
}

pub const CREATE_ALWAYS: u32 = 2;
    
#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn DeviceIoControl(
        hDevice: HANDLE,
        dwIoControlCode: u32,
        lpInBuffer: *mut c_void,
        nInBufferSize: u32,
        lpOutBuffer: *mut c_void,
        nOutBufferSize: u32,
        lpBytesReturned: *mut u32,
        lpOverlapped: *mut c_void
    ) -> BOOL;

    pub fn GetCompressedFileSizeW(lpFileName: LPCWSTR, lpFileSizeHigh: *mut u32) -> u32;
    pub fn GetFileAttributesW(lpFileName: LPCWSTR) -> u32;
    pub fn SetFileAttributesW(lpFileName: LPCWSTR, dwFileAttributes: u32) -> BOOL;
    pub fn GetCurrentThreadId() -> u32;
    
    pub fn InitializeProcThreadAttributeList(lpAttributeList: *mut c_void, dwAttributeCount: u32, dwFlags: u32, lpSize: *mut usize) -> BOOL;
    pub fn UpdateProcThreadAttribute(lpAttributeList: *mut c_void, dwFlags: u32, Attribute: usize, lpValue: *const c_void, cbSize: usize, lpPreviousValue: *mut c_void, lpReturnSize: *mut usize) -> BOOL;
    pub fn DeleteProcThreadAttributeList(lpAttributeList: *mut c_void);
    pub fn CreateProcessW(lpApplicationName: LPCWSTR, lpCommandLine: LPWSTR, lpProcessAttributes: *const c_void, lpThreadAttributes: *const c_void, bInheritHandles: BOOL, dwCreationFlags: u32, lpEnvironment: *const c_void, lpCurrentDirectory: LPCWSTR, lpStartupInfo: *mut c_void, lpProcessInformation: *mut c_void) -> BOOL;
}

pub const GENERIC_READ: u32 = 0x80000000;
pub const GENERIC_WRITE: u32 = 0x40000000;

// --- WOF Constants ---
pub const WOF_CURRENT_VERSION: u32 = 1;
pub const WOF_PROVIDER_FILE: u32 = 2;
pub const FILE_PROVIDER_CURRENT_VERSION: u32 = 1;

