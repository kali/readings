use std::time::Duration;

use winapi::shared::minwindef::FILETIME;
use winapi::um::processthreadsapi::{GetCurrentProcess, GetProcessTimes};

use super::{OsReadings, ReadingsResult};

pub(crate) fn get_os_readings() -> ReadingsResult<OsReadings> {
    unsafe {
        let h_process = GetCurrentProcess();
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

        let usage = OsReadings {
            virtual_size: 0,
            resident_size: 0,
            resident_size_max: 0,
            user_time,
            system_time,
            minor_fault: 0,
            major_fault: 0,
        };

        Ok(usage)
    }
}
