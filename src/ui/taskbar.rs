use windows_sys::Win32::Foundation::HWND;
use windows_sys::core::{GUID, HRESULT};
use windows_sys::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows_sys::Win32::UI::Shell::{
    TaskbarList, TBPFLAG, TBPF_NOPROGRESS, TBPF_INDETERMINATE, TBPF_NORMAL, TBPF_ERROR, TBPF_PAUSED
};

type BOOL = i32;

// Manual definition of ITaskbarList3 since it's missing in windows-sys
#[repr(C)]
pub struct ITaskbarList3 {
    pub lpVtbl: *const ITaskbarList3Vtbl,
}

#[repr(C)]
pub struct ITaskbarList3Vtbl {
    pub QueryInterface: unsafe extern "system" fn(*mut ITaskbarList3, *const GUID, *mut *mut std::ffi::c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(*mut ITaskbarList3) -> u32,
    pub Release: unsafe extern "system" fn(*mut ITaskbarList3) -> u32,
    pub HrInit: unsafe extern "system" fn(*mut ITaskbarList3) -> HRESULT,
    pub AddTab: unsafe extern "system" fn(*mut ITaskbarList3, HWND) -> HRESULT,
    pub DeleteTab: unsafe extern "system" fn(*mut ITaskbarList3, HWND) -> HRESULT,
    pub ActivateTab: unsafe extern "system" fn(*mut ITaskbarList3, HWND) -> HRESULT,
    pub SetActiveAlt: unsafe extern "system" fn(*mut ITaskbarList3, HWND) -> HRESULT,
    pub MarkFullscreenWindow: unsafe extern "system" fn(*mut ITaskbarList3, HWND, BOOL) -> HRESULT,
    pub SetProgressValue: unsafe extern "system" fn(*mut ITaskbarList3, HWND, u64, u64) -> HRESULT,
    pub SetProgressState: unsafe extern "system" fn(*mut ITaskbarList3, HWND, TBPFLAG) -> HRESULT,
    // We don't need the rest of the methods for this app
}

const IID_ITASKBAR_LIST3: GUID = GUID {
    data1: 0xea1afb91,
    data2: 0x9e28,
    data3: 0x4b86,
    data4: [0x90, 0xe9, 0x9e, 0x9f, 0x8a, 0x5e, 0xef, 0xaf],
};

pub enum TaskbarState {
    NoProgress,
    Indeterminate,
    Normal,
    Error,
    Paused,
}

pub struct TaskbarProgress {
    hwnd: HWND,
    taskbar_list: *mut ITaskbarList3,
}

impl TaskbarProgress {
    pub fn new(hwnd: HWND) -> Self {
        let mut taskbar_list: *mut ITaskbarList3 = std::ptr::null_mut();
        
        unsafe {
            // Ensure COM is initialized (though main thread should have done it)
            // We don't want to re-init if already done, but CoCreateInstance requires it.
            // Main.rs does CoInitializeEx.
            
            let hr = CoCreateInstance(
                &TaskbarList, 
                std::ptr::null_mut(), 
                CLSCTX_ALL, 
                &IID_ITASKBAR_LIST3, 
                &mut taskbar_list as *mut _ as *mut _
            );
            
            if hr != 0 {
                // Failed
                taskbar_list = std::ptr::null_mut();
            }
        }

        Self {
            hwnd,
            taskbar_list,
        }
    }

    pub fn set_state(&self, state: TaskbarState) {
        if !self.taskbar_list.is_null() {
            let flags: TBPFLAG = match state {
                TaskbarState::NoProgress => TBPF_NOPROGRESS,
                TaskbarState::Indeterminate => TBPF_INDETERMINATE,
                TaskbarState::Normal => TBPF_NORMAL,
                TaskbarState::Error => TBPF_ERROR,
                TaskbarState::Paused => TBPF_PAUSED,
            };
            unsafe {
                let vtbl = (*self.taskbar_list).lpVtbl;
                // SetProgressState is the 10th method in ITaskbarList3?
                // ITaskbarList (3) -> ITaskbarList2 (1) -> ITaskbarList3 (SetProgressState is offset?)
                // Actually windows-sys defines the Vtbl struct:
                // IUnknown: QueryInterface, AddRef, Release
                // ITaskbarList: HrInit, AddTab, DeleteTab, ActivateTab, SetActiveAlt
                // ITaskbarList2: MarkFullscreenWindow
                // ITaskbarList3: SetProgressValue, SetProgressState, RegisterTab, UnregisterTab, SetTabOrder, SetTabActive, ThumbBarAddButtons, ThumbBarUpdateButtons, ThumbBarSetImageList, SetOverlayIcon, SetThumbnailTooltip, SetThumbnailClip
                // layout:
                // 0: QueryInterface
                // 1: AddRef
                // 2: Release
                // 3: HrInit
                // 4: AddTab
                // 5: DeleteTab
                // 6: ActivateTab
                // 7: SetActiveAlt
                // 8: MarkFullscreenWindow
                // 9: SetProgressValue
                // 10: SetProgressState
                
                // Call SetProgressState(hwnd, flags)
                ((*vtbl).SetProgressState)(self.taskbar_list, self.hwnd, flags);
            }
        }
    }

    pub fn set_value(&self, completed: u64, total: u64) {
        if !self.taskbar_list.is_null() {
            unsafe {
                let vtbl = (*self.taskbar_list).lpVtbl;
                // Call SetProgressValue(hwnd, completed, total)
                ((*vtbl).SetProgressValue)(self.taskbar_list, self.hwnd, completed, total);
            }
        }
    }
}

impl Drop for TaskbarProgress {
    fn drop(&mut self) {
        if !self.taskbar_list.is_null() {
            unsafe {
                let vtbl = (*self.taskbar_list).lpVtbl;
                ((*vtbl).Release)(self.taskbar_list);
            }
        }
    }
}
