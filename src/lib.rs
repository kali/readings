use std::io::Write;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicI64, AtomicUsize};
use std::sync::Arc;
use std::{io, sync, time};

use thiserror::Error;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static FREEED: AtomicUsize = AtomicUsize::new(0);

#[macro_export]
macro_rules! instrumented_allocator {
    () => {
        #[global_allocator]
        static A: readings::InstrumentedAllocator = readings::InstrumentedAllocator;
    }
}

pub struct InstrumentedAllocator;

unsafe impl std::alloc::GlobalAlloc for InstrumentedAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let ptr = std::alloc::System.alloc(layout);
        if !ptr.is_null() {
            ALLOCATED.fetch_add(layout.size(), Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        if !ptr.is_null() {
            FREEED.fetch_add(layout.size(), Relaxed);
        }
        std::alloc::System.dealloc(ptr, layout);
    }
}

#[derive(Error, Debug)]
pub enum ReadingsError {
    #[error("io Error accssing /proc/self/stat")]
    ProcStat(io::Error),
    #[error("Io error writing readings")]
    Io(#[from] io::Error),
    #[error("Poisoned monitor")]
    PoisonedMonitor,
}

pub type ReadingsResult<T> = Result<T, ReadingsError>;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
use linux::get_os_readings;

#[derive(Debug)]
pub struct OsReadings {
    pub virtual_size: u64,
    pub resident_size: u64,
    pub resident_size_max: u64,
    pub user_time: f64,
    pub system_time: f64,
    pub minor_fault: u64,
    pub major_fault: u64,
}

#[derive(Clone)]
pub struct Monitor(sync::Arc<sync::Mutex<MonitorData>>);

pub struct MonitorData {
    cores: usize,
    origin: Option<std::time::Instant>,
    writer: io::BufWriter<Box<dyn io::Write + Send>>,
    metrics_i64: Vec<(String, Arc<AtomicI64>)>,
}

impl MonitorData {
    fn write_line(&mut self, now: time::Instant, reason: &str) -> ReadingsResult<()> {
        if self.origin.is_none() {
            write!(self.writer, "   time cor        vsz        rsz     rszmax")?;
            write!(self.writer, "    utime    stime       minf       majf")?;
            write!(self.writer, "      alloc       free       done")?;
            writeln!(self.writer, " event")?;
            self.origin = Some(now)
        }
        let usage = get_os_readings()?;
        write!(
            self.writer,
            "{:7.3} {:3}",
            (now - self.origin.unwrap()).as_secs_f32(),
            self.cores
        )?;
        write!(
            self.writer,
            " {:10} {:10} {:10}",
            usage.virtual_size, usage.resident_size, usage.resident_size_max
        )?;
        write!(
            self.writer,
            " {:8.6} {:8.6} {:10} {:10}",
            usage.user_time, usage.system_time, usage.minor_fault, usage.major_fault
        )?;
        write!(
            self.writer,
            " {:10} {:10}",
            ALLOCATED.load(Relaxed),
            FREEED.load(Relaxed)
        )?;
        for m in &self.metrics_i64 {
            write!(
                self.writer,
                " {:10}",
                m.1.load(Relaxed),
            )?;
        }
        writeln!(self.writer, " {}", reason)?;
        self.writer.flush()?;
        Ok(())
    }
}

impl Monitor {
    pub fn new<W: Write + Send + 'static>(write: W) -> Monitor {
        let data = MonitorData {
            cores: num_cpus::get(),
            origin: None,
            writer: io::BufWriter::new(Box::new(write)),
            metrics_i64: vec![],
        };
        Monitor(sync::Arc::new(sync::Mutex::new(data)))
    }

    pub fn register_i64(&mut self, name: String) -> ReadingsResult<Arc<AtomicI64>> {
        let mut m = self.0.lock().map_err(|_| ReadingsError::PoisonedMonitor)?;
        let it = Arc::new(AtomicI64::new(0));
        m.metrics_i64.push((name, it.clone()));
        Ok(it)
    }

    pub fn spawn_heartbeat(&mut self, interval: time::Duration) -> ReadingsResult<()> {
        let monitor = self.clone();
        monitor.log_event("spawned_heartbeat")?;
        let origin = monitor
            .0
            .lock()
            .map_err(|_| ReadingsError::PoisonedMonitor)?
            .origin
            .unwrap();
        std::thread::spawn(move || {
            for step in 1.. {
                let delay = origin + (step * interval) - std::time::Instant::now();
                ::std::thread::sleep(delay);
                monitor.log_event("").unwrap();
            }
        });
        Ok(())
    }

    pub fn log_event(&self, event: &str) -> ReadingsResult<()> {
        self.write_line(std::time::Instant::now(), event)
    }

    fn write_line(&self, now: time::Instant, reason: &str) -> ReadingsResult<()> {
        self.0
            .lock()
            .map_err(|_| ReadingsError::PoisonedMonitor)?
            .write_line(now, reason)
    }
}
