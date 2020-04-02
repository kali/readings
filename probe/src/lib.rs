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
        static A: $crate::InstrumentedAllocator = $crate::InstrumentedAllocator;
    };
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
    #[error("Metrics can only be added before first event")]
    LateRegistertingMetricsAttempt,
    #[error("io Error accessing /proc/self/stat")]
    ProcStat(io::Error),
    #[error("Io error writing readings")]
    Io(#[from] io::Error),
    #[error("Poisoned probe")]
    PoisonedProbe,
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
pub struct Probe(sync::Arc<sync::Mutex<ProbeData>>);

pub struct ProbeData {
    cores: usize,
    origin: Option<std::time::Instant>,
    writer: io::BufWriter<Box<dyn io::Write + Send>>,
    metrics_i64: Vec<(String, Arc<AtomicI64>)>,
}

impl ProbeData {
    fn write_line(&mut self, now: time::Instant, reason: &str) -> ReadingsResult<()> {
        if self.origin.is_none() {
            write!(self.writer, "   time cor        vsz        rsz     rszmax")?;
            write!(self.writer, "    utime    stime       minf       majf")?;
            write!(self.writer, "      alloc       free")?;
            for m in &self.metrics_i64 {
                write!(self.writer, " {:>10}", m.0)?;
            }
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
            write!(self.writer, " {:10}", m.1.load(Relaxed),)?;
        }
        writeln!(self.writer, " {}", reason)?;
        self.writer.flush()?;
        Ok(())
    }
}

impl Probe {
    pub fn new<W: Write + Send + 'static>(write: W) -> ReadingsResult<Probe> {
        let mut writer = io::BufWriter::new(Box::new(write) as _);
        writeln!(writer, "#ReadingsV1")?;
        let data = ProbeData {
            cores: num_cpus::get(),
            origin: None,
            writer,
            metrics_i64: vec![],
        };
        Ok(Probe(sync::Arc::new(sync::Mutex::new(data))))
    }

    pub fn register_i64<S: AsRef<str>>(&mut self, name: S) -> ReadingsResult<Arc<AtomicI64>> {
        let mut m = self.0.lock().map_err(|_| ReadingsError::PoisonedProbe)?;
        if m.origin.is_some() {
            return Err(ReadingsError::LateRegistertingMetricsAttempt);
        }
        let it = Arc::new(AtomicI64::new(0));
        m.metrics_i64.push((name.as_ref().replace(" ", "_"), it.clone()));
        Ok(it)
    }

    pub fn spawn_heartbeat(&mut self, interval: time::Duration) -> ReadingsResult<()> {
        let probe = self.clone();
        probe.log_event("spawned_heartbeat")?;
        let origin = probe
            .0
            .lock()
            .map_err(|_| ReadingsError::PoisonedProbe)?
            .origin
            .unwrap();
        std::thread::spawn(move || {
            for step in 1.. {
                let wanted = origin + (step * interval);
                let now = std::time::Instant::now();
                if wanted > now {
                    ::std::thread::sleep(wanted - now);
                }
                if let Err(e) = probe.log_event("") {
                    eprintln!("{:?}", e);
                }
            }
        });
        Ok(())
    }

    pub fn log_event(&self, event: &str) -> ReadingsResult<()> {
        self.write_line(std::time::Instant::now(), &event.replace(" ", "_"))
    }

    pub fn get_i64<S: AsRef<str>>(&self, name: S) -> Option<Arc<AtomicI64>> {
        let name = name.as_ref().replace(" ", "_");
        self.0.lock().ok().and_then(|l| {
            l.metrics_i64
                .iter()
                .find(|m| (m.0 == name))
                .map(|m| m.1.clone())
        })
    }

    fn write_line(&self, now: time::Instant, reason: &str) -> ReadingsResult<()> {
        self.0
            .lock()
            .map_err(|_| ReadingsError::PoisonedProbe)?
            .write_line(now, reason)
    }
}
