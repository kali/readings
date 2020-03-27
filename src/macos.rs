
use libc::*;
#[repr(C)]
#[derive(Default)]
pub struct BasicTaskInfo {
    pub virtual_size: u64,
    pub resident_size: u64,
    pub resident_size_max: u64,
    pub user_time: timeval,
    pub system_time: timeval,
    pub policy: c_int,
    pub suspend_count: c_uint,
}

mod ffi {
    use libc::*;
    extern "C" {
        pub fn mach_task_self() -> c_uint;
        pub fn task_info(
            task: c_uint,
            flavor: c_int,
            task_info: *mut super::BasicTaskInfo,
            count: *mut c_uint,
        ) -> c_uint;
    }
}
pub fn task_self() -> c_uint {
    unsafe { ffi::mach_task_self() }
}
pub fn task_info() -> BasicTaskInfo {
    let mut info = BasicTaskInfo::default();
    let mut count: c_uint =
        (::std::mem::size_of::<BasicTaskInfo>() / ::std::mem::size_of::<c_uint>()) as c_uint;
    unsafe {
        ffi::task_info(task_self(), 20, &mut info, &mut count);
    }
    info
}

#[cfg(target_os = "macos")]
pub fn get_memory_usage() -> Result<ResourceUsage> {
    let info = darwin::task_info();
    let rusage = get_rusage();
    Ok(ResourceUsage {
        virtual_size: info.virtual_size,
        resident_size: info.resident_size,
        resident_size_max: info.resident_size_max,
        user_time: rusage.ru_utime.tv_sec as f64 + rusage.ru_utime.tv_usec as f64 / 1_000_000f64,
        system_time: rusage.ru_stime.tv_sec as f64 + rusage.ru_stime.tv_usec as f64 / 1_000_000f64,
        minor_fault: rusage.ru_minflt as u64,
        major_fault: rusage.ru_majflt as u64,
    })
}
