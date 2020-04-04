use std::time::Duration;

use winapi::shared::minwindef::FILETIME;
use winapi::um::processthreadsapi::{GetCurrentProcess, GetProcessTimes};
use winapi::um::psapi::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};

use super::{OsReadings, ReadingsResult};

pub(crate) fn get_os_readings() -> ReadingsResult<OsReadings> {
    unsafe {
        let h_process = GetCurrentProcess();

        // Query timers information
        let mut creation_time: FILETIME = std::mem::zeroed();
        let mut exit_time: FILETIME = std::mem::zeroed();
        let mut kernel_time: FILETIME = std::mem::zeroed();
        let mut user_time: FILETIME = std::mem::zeroed();
        GetProcessTimes(
            h_process,
            &mut creation_time,
            &mut exit_time,
            &mut kernel_time,
            &mut user_time,
        );
        let system_time =
            (kernel_time.dwHighDateTime as u32 as u64) << 32 + kernel_time.dwLowDateTime as u64;
        let system_time = Duration::from_nanos(system_time * 100);
        let user_time =
            (user_time.dwHighDateTime as u32 as u64) << 32 + user_time.dwLowDateTime as u64;
        let user_time = Duration::from_nanos(user_time * 100);

        // Query memory information
        let mut mem_counters: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
        GetProcessMemoryInfo(
            h_process,
            &mut mem_counters as *mut PROCESS_MEMORY_COUNTERS_EX as *mut _,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        );

        let virtual_size = mem_counters.PrivateUsage as u64;
        let resident_size = mem_counters.WorkingSetSize as u64;
        let resident_size_max = mem_counters.PeakWorkingSetSize as u64;
        let major_fault = mem_counters.PageFaultCount as u64;

        let usage = OsReadings {
            virtual_size,
            resident_size,
            resident_size_max,
            user_time,
            system_time,
            minor_fault: 0,
            major_fault,
        };

        Ok(usage)
    }
}
