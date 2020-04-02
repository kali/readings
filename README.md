# Readings: monitoring your process vitals

Readings is meant to get vital information for the health of a process.

It is made of two crates:
* readings-probe is the instrumentation bit that you must depend on and setup
* readings contains the executable that will make charts from the instrumentation output

# Status

This is alpha, early work bazaar style. For instance, the probe will only compile on Linux and Mac.

# Quickstart

## Instrument

In your main, or just around:

```rust
// this is optional but the cost may be worth it. YMMV
readings_probe::instrumented_allocator!();

fn main() -> readings_probe::ReadingsResult<()> {
    // setup the probe
    let mut probe =
        readings_probe::Probe::new(std::fs::File::create("readings.out").unwrap()).unwrap();

    // We will use an AtomicI64 to communicate a user-defined metrics ("progress") to the probe.
    let progress = probe.register_i64("progress".to_string())?;

    // Starts the probe (1sec i a lot. heartbeat can be realistically set as low as a few millis).
    probe.spawn_heartbeat(std::time::Duration::from_millis(1000))?;

    // do some stuff, update progress
    progress.store(percent_done, std::sync::atomic::Ordering::Relaxed);

    // do more stuff, log an event
    probe.log_event("about to get crazy")?;

    // still more stuff, and another event
    probe.log_event("done")?;
    Ok(())
}
```

## Graph

* Install the command line tool. 

`cargo install readings`

* run it

`readings readings.out`
