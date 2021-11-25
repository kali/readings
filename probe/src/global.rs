//! This module contains helpers to put a Probe in a global "well-known" place, allowing to
//! instrument deep routines without passing the Probe around the entire stack.
//!
//! Probe must be initialized and spawn as usual, typically close to the main then handled
//! to the `set` function. Notice that Probe can be clone() if some instrumentation at the
//! top level is also needed.
//!
//! log_event() and get_i64 will fail silently if the probe has not be `set`, allowing to
//! toggle on or off the instrumentation at the top level.
use crate::{Probe, ReadingsResult};
use std::sync::atomic::AtomicI64;
use std::sync::{Arc, Mutex};

lazy_static::lazy_static! {
    static ref PROBE: Mutex<Option<Probe>> = Mutex::new(None);
}

/// Setup `probe` as the global default probe.
pub fn set(probe: Probe) {
    if let Ok(mut g) = PROBE.lock() {
        g.replace(probe);
    }
}

/// Remove the global probe if any.
pub fn unset() {
    if let Ok(mut g) = PROBE.lock() {
        g.take();
    }
}

/// Log on the default probe an individual event with a label and the current values of metrics.
pub fn log_event(event: &str) -> ReadingsResult<()> {
    if let Ok(mut lock) = PROBE.lock() {
        if let Some(probe) = lock.as_mut() {
            return probe.log_event(event);
        }
    }
    Ok(())
}

/// Recover from the default probe a pre-registered used-defined metrics.
pub fn get_i64<S: AsRef<str>>(name: S) -> Option<Arc<AtomicI64>> {
    if let Ok(mut lock) = PROBE.lock() {
        if let Some(probe) = lock.as_mut() {
            return probe.get_i64(name);
        }
    }
    None
}
