use windows::core::Interface;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::UI::Shell::{ITaskbarList3, TaskbarList, TBPFLAG, TBPF_NOPROGRESS, TBPF_INDETERMINATE, TBPF_NORMAL, TBPF_ERROR, TBPF_PAUSED};

pub enum TaskbarState {
    NoProgress,
    Indeterminate,
    Normal,
    Error,
    Paused,
}

pub struct TaskbarProgress {
    hwnd: HWND,
    taskbar_list: Option<ITaskbarList3>,
}

impl TaskbarProgress {
    pub fn new(hwnd: HWND) -> Self {
        let taskbar_list = unsafe {
            CoCreateInstance::<_, ITaskbarList3>(&TaskbarList, None, CLSCTX_ALL).ok()
        };

        Self {
            hwnd,
            taskbar_list,
        }
    }

    pub fn set_state(&self, state: TaskbarState) {
        if let Some(tbl) = &self.taskbar_list {
            let flags: TBPFLAG = match state {
                TaskbarState::NoProgress => TBPF_NOPROGRESS,
                TaskbarState::Indeterminate => TBPF_INDETERMINATE,
                TaskbarState::Normal => TBPF_NORMAL,
                TaskbarState::Error => TBPF_ERROR,
                TaskbarState::Paused => TBPF_PAUSED,
            };
            unsafe {
                let _ = tbl.SetProgressState(self.hwnd, flags);
            }
        }
    }

    pub fn set_value(&self, completed: u64, total: u64) {
        if let Some(tbl) = &self.taskbar_list {
            unsafe {
                let _ = tbl.SetProgressValue(self.hwnd, completed, total);
            }
        }
    }
}
