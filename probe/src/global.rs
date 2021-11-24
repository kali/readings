use crate::{Probe, ReadingsResult};
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref PROBE: Mutex<Option<Probe>> = Mutex::new(None);
}

pub fn set(probe: Probe) {
    if let Ok(mut g) = PROBE.lock() {
        g.replace(probe);
    }
}

pub fn log_event(event: &str) -> ReadingsResult<()> {
    if let Ok(mut lock) = PROBE.lock() {
        if let Some(probe) = lock.as_mut() {
            return probe.log_event(event)
        }
    }
    Ok(())
}
