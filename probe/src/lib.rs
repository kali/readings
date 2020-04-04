//! # Instrumentation Probe for [Readings](http://github.com/kali/readings)
//!
//!
//! Readings goal is to make process vital metrics intrumentation as easy as
//! possible.
//! 
//! This is the instrumentation library that must be embedded in the client
//! code.
//!
//! Please refer to the [Readings](http://github.com/kali/readings)
//! 
//! 
//! ```rust
//! // this is optional but the cost may be worth it. YMMV. It instruments
//! // Rust global allocator.
//! readings_probe::instrumented_allocator!();
//! 
//! fn main() -> readings_probe::ReadingsResult<()> {
//!     // setup the probe
//!     let mut probe =
//!         readings_probe::Probe::new(std::fs::File::create("readings.out").unwrap()).unwrap();
//! 
//!     // We will use an AtomicI64 to communicate a user-defined metrics ("progress") to the probe.
//!     let progress = probe.register_i64("progress".to_string())?;
//! 
//!     // Starts the probe (1sec i a lot. heartbeat can be realistically set as low as a few millis).
//!     probe.spawn_heartbeat(std::time::Duration::from_millis(1000))?;
//! 
//!     // do some stuff, update progress
//!     let percent_done = 12;
//!     progress.store(percent_done, std::sync::atomic::Ordering::Relaxed);
//! 
//!     // do more stuff, log an event
//!     probe.log_event("about to get crazy")?;
//! 
//!     // still more stuff, and another event
//!     probe.log_event("done")?;
//!     Ok(())
//! }
//! ```


use std::io::Write;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicI64, AtomicUsize};
use std::sync::Arc;
use std::{io, sync, time};

use thiserror::Error;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static FREEED: AtomicUsize = AtomicUsize::new(0);

/// Setup global allocator instrumentation, to track rust-managed memory. 
///
/// It is not mandatory to do so, as we also register the RSZ and VSZ as
/// reported by the OS, but it may be interesting. From our experience it may be
/// worth activating it as the cost is relatively modest.
#[macro_export]
macro_rules! instrumented_allocator {
    () => {
        #[global_allocator]
        static A: $crate::InstrumentedAllocator = $crate::InstrumentedAllocator;
    };
}

#[doc(hidden)]
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

/// Reading error enumeration.
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

/// Reading generic Result helper.
pub type ReadingsResult<T> = Result<T, ReadingsError>;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
use linux::get_os_readings;

#[derive(Debug)]
pub(crate) struct OsReadings {
    pub virtual_size: u64,
    pub resident_size: u64,
    pub resident_size_max: u64,
    pub user_time: f64,
    pub system_time: f64,
    pub minor_fault: u64,
    pub major_fault: u64,
}

/// The interface to reading probe.
#[derive(Clone)]
pub struct Probe(sync::Arc<sync::Mutex<ProbeData>>);

struct ProbeData {
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
    /// Creates a probe logging its data to Write implementation (usually a
    /// file).
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

    /// Register an i64 used-defined metric.
    ///
    /// Must be called prior to the first call to `log_event` or `spawn_heartbeat`.
    ///
    /// The result is shared AtomicI64 that can be used by client code to share
    /// communicate updates with the probe.
    ///
    /// TODO: type-enforce this using the Builder pattern
    pub fn register_i64<S: AsRef<str>>(&mut self, name: S) -> ReadingsResult<Arc<AtomicI64>> {
        let mut m = self.0.lock().map_err(|_| ReadingsError::PoisonedProbe)?;
        if m.origin.is_some() {
            return Err(ReadingsError::LateRegistertingMetricsAttempt);
        }
        let it = Arc::new(AtomicI64::new(0));
        m.metrics_i64.push((name.as_ref().replace(" ", "_"), it.clone()));
        Ok(it)
    }

    /// Spawn a thread that will record all vitals at every "interval". 
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

    /// Spawn a thread that will record all vitals at every "interval". 
    pub fn log_event(&self, event: &str) -> ReadingsResult<()> {
        self.write_line(std::time::Instant::now(), &event.replace(" ", "_"))
    }

    /// Recover a pre-registered used-defined metrics from the probe.
    ///
    /// The result is shared AtomicI64 that can be used by client code to share
    /// communicate updates with the probe.
    ///
    /// It is more efficient for the client code to keep the shared AtomicI64 somewhere
    /// handy than calling this at every update. Nevertheless, it allows for
    /// intermediate code to just have to propagate the probe without worrying
    /// about the various metrics that the underlying code may need.
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
