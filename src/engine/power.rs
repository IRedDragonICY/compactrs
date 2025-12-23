#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]
use windows_sys::Win32::Foundation::HANDLE;
use std::mem::zeroed;
use std::ffi::c_void;

// --- Manual Bindings & Structs ---

#[repr(C)]
struct PROCESS_POWER_THROTTLING_STATE {
    Version: u32,
    ControlMask: u32,
    StateMask: u32,
}

#[repr(C)]
struct THREAD_POWER_THROTTLING_STATE {
    Version: u32,
    ControlMask: u32,
    StateMask: u32,
}

#[repr(C)]
struct SYSTEM_INFO {
    wProcessorArchitecture: u16,
    wReserved: u16,
    dwPageSize: u32,
    lpMinimumApplicationAddress: *mut c_void,
    lpMaximumApplicationAddress: *mut c_void,
    dwActiveProcessorMask: usize,
    dwNumberOfProcessors: u32,
    dwProcessorType: u32,
    dwAllocationGranularity: u32,
    wProcessorLevel: u16,
    wProcessorRevision: u16,
}

const ProcessPowerThrottling: u32 = 4;
const ThreadPowerThrottling: u32 = 1;
const THREAD_PRIORITY_IDLE: i32 = -15;
const IDLE_PRIORITY_CLASS: u32 = 64;
const NORMAL_PRIORITY_CLASS: u32 = 32;

// Version 1 is standard for EcoQoS
const PROCESS_POWER_THROTTLING_CURRENT_VERSION: u32 = 1;
const PROCESS_POWER_THROTTLING_EXECUTION_SPEED: u32 = 1;

const THREAD_POWER_THROTTLING_CURRENT_VERSION: u32 = 1;
const THREAD_POWER_THROTTLING_EXECUTION_SPEED: u32 = 1;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThread() -> HANDLE;
    fn GetCurrentProcess() -> HANDLE;
    fn SetThreadPriority(hthread: HANDLE, npriority: i32) -> i32;
    fn SetThreadAffinityMask(hthread: HANDLE, dwthreadaffinitymask: usize) -> usize;
    fn SetThreadInformation(
        hthread: HANDLE,
        threadinformationclass: u32,
        threadinformation: *const c_void,
        threadinformationsize: u32,
    ) -> i32;
    fn SetPriorityClass(hprocess: HANDLE, dwpriorityclass: u32) -> i32;
    fn SetProcessInformation(
        hprocess: HANDLE,
        processinformationclass: u32,
        processinformation: *const c_void,
        processinformationsize: u32,
    ) -> i32;
    fn GetSystemInfo(lpsysteminfo: *mut SYSTEM_INFO);
}

/// Enable "Low Power Mode" (Eco Mode) for the current thread.
/// 
/// This applies three strategies:
/// 1. Sets thread priority to IDLE (background).
/// 2. Enables EcoQoS (Power Throttling) to hint the scheduler to use E-cores.
/// 3. Restricts thread affinity to the upper 50% of logical processors (heuristic for E-cores on hybrid CPUs).
pub fn enable_eco_mode() {
    unsafe {
        let thread_handle = GetCurrentThread();

        // 1. Lower Priority
        // This is the strongest signal to the Windows Scheduler.
        SetThreadPriority(thread_handle, THREAD_PRIORITY_IDLE);

        // 2. Enable EcoQoS (Power Throttling)
        // This tells Windows 10/11 that this thread is non-critical background work.
        let mut throttling_state: THREAD_POWER_THROTTLING_STATE = zeroed();
        throttling_state.Version = THREAD_POWER_THROTTLING_CURRENT_VERSION;
        throttling_state.ControlMask = THREAD_POWER_THROTTLING_EXECUTION_SPEED;
        throttling_state.StateMask = THREAD_POWER_THROTTLING_EXECUTION_SPEED;

        let _ = SetThreadInformation(
            thread_handle,
            ThreadPowerThrottling,
            &throttling_state as *const _ as *const _,
            std::mem::size_of::<THREAD_POWER_THROTTLING_STATE>() as u32,
        );

        // 3. Affinity Mask (Heuristic: Use last 50% of cores)
        // Only apply if we have more than 4 cores, otherwise we might choke too much.
        let mut sys_info: SYSTEM_INFO = zeroed();
        GetSystemInfo(&mut sys_info);
        let num_processors = sys_info.dwNumberOfProcessors as usize;

        if num_processors > 4 {
            // Calculate mask for the upper half of processors.
            // e.g. 8 processors: 0000 0000 ... 1111 0000
            let start_index = num_processors / 2;
            let mut mask: usize = 0;
            
            for i in start_index..num_processors {
                // Ensure we don't overflow usize (64-bit safe)
                if i < 64 {
                    mask |= 1 << i;
                }
            }

            if mask != 0 {
               SetThreadAffinityMask(thread_handle, mask);
            }
        }
    }
}

/// Sets the "Efficiency Mode" (EcoQoS) for the entire process.
///
/// If `enabled` is true:
/// - Sets Process Priority Class to `IDLE_PRIORITY_CLASS`.
/// - Enables `ProcessPowerThrottling` (EcoQoS).
///
/// If `enabled` is false:
/// - Sets Process Priority Class to `NORMAL_PRIORITY_CLASS`.
/// - Disables `ProcessPowerThrottling`.
pub fn set_process_eco_mode(enabled: bool) {
    unsafe {
        let process_handle = GetCurrentProcess();

        if enabled {
            // 1. Set Process Priority to IDLE (background)
            SetPriorityClass(process_handle, IDLE_PRIORITY_CLASS);

            // 2. Enable EcoQoS
            let mut throttling_state: PROCESS_POWER_THROTTLING_STATE = zeroed();
            throttling_state.Version = PROCESS_POWER_THROTTLING_CURRENT_VERSION;
            throttling_state.ControlMask = PROCESS_POWER_THROTTLING_EXECUTION_SPEED;
            throttling_state.StateMask = PROCESS_POWER_THROTTLING_EXECUTION_SPEED;

            let _ = SetProcessInformation(
                process_handle,
                ProcessPowerThrottling,
                &throttling_state as *const _ as *const _,
                std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
            );
        } else {
            // Restore Normal Priority
            SetPriorityClass(process_handle, NORMAL_PRIORITY_CLASS);

            // Disable EcoQoS
            let mut throttling_state: PROCESS_POWER_THROTTLING_STATE = zeroed();
            throttling_state.Version = PROCESS_POWER_THROTTLING_CURRENT_VERSION;
            throttling_state.ControlMask = PROCESS_POWER_THROTTLING_EXECUTION_SPEED;
            throttling_state.StateMask = 0; // Clear the state bit to disable

            let _ = SetProcessInformation(
                process_handle,
                ProcessPowerThrottling,
                &throttling_state as *const _ as *const _,
                std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
            );
        }
    }
}
