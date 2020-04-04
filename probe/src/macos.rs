use libc::*;

use super::{OsReadings, ReadingsResult};

#[repr(C)]
struct BasicTaskInfo {
    pub virtual_size: u64,
    pub resident_size: u64,
    pub resident_size_max: u64,
    pub user_time: timeval,
    pub system_time: timeval,
    pub policy: c_int,
    pub suspend_count: c_uint,
}

extern "C" {
    fn mach_task_self() -> c_uint;
    fn task_info(
        task: c_uint,
        flavor: c_int,
        task_info: *mut BasicTaskInfo,
        count: *mut c_uint,
    ) -> c_uint;
}

fn basic_task_info() -> BasicTaskInfo {
    unsafe {
        let mut info = std::mem::zeroed();
        let mut count: c_uint =
            (::std::mem::size_of::<BasicTaskInfo>() / ::std::mem::size_of::<c_uint>()) as c_uint;
        let me = mach_task_self();
        task_info(me, 20, &mut info, &mut count);
        info
    }
}

fn get_rusage() -> rusage {
    unsafe {
        let mut usage = std::mem::zeroed();
        getrusage(RUSAGE_SELF, &mut usage);
        usage
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn get_os_readings() -> ReadingsResult<OsReadings> {
    let info = basic_task_info();
    let rusage = get_rusage();
    Ok(OsReadings {
        virtual_size: info.virtual_size,
        resident_size: info.resident_size,
        resident_size_max: info.resident_size_max,
        user_time: Duration::from_secs(rusage.ru_utime.tv_sec as _) + Duration::from_micros(rusage.ru_utime.tv_usec as _),
        system_time: Duration::from_secs(rusage.ru_stime.tv_sec as _) + Duration::from_micros(rusage.ru_stime.tv_usec as _),
        minor_fault: rusage.ru_minflt as u64,
        major_fault: rusage.ru_majflt as u64,
    })
}
