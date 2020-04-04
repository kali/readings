use libc::{getrusage, rusage, RUSAGE_SELF};

use super::{OsReadings, ReadingsError};

fn get_rusage() -> rusage {
    unsafe {
        let mut usage = std::mem::zeroed();
        getrusage(RUSAGE_SELF, &mut usage);
        usage
    }
}

pub(crate) fn get_os_readings() -> Result<OsReadings, ReadingsError> {
    let proc_stat =
        std::fs::read_to_string("/proc/self/stat").map_err(|e| ReadingsError::ProcStat(e))?;
    let mut tokens = proc_stat.split(" ");
    let rusage = get_rusage();
    Ok(OsReadings {
        virtual_size: tokens.nth(22).unwrap().parse().unwrap_or(0),
        resident_size: 4 * 1024 * tokens.next().unwrap().parse().unwrap_or(0),
        resident_size_max: 1024 * rusage.ru_maxrss as u64,
        user_time: rusage.ru_utime.tv_sec as f64 + rusage.ru_utime.tv_usec as f64 / 1_000_000f64,
        system_time: rusage.ru_stime.tv_sec as f64 + rusage.ru_stime.tv_usec as f64 / 1_000_000f64,
        minor_fault: rusage.ru_minflt as u64,
        major_fault: rusage.ru_majflt as u64,
    })
}
