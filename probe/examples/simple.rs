use std::time::Duration;

readings_probe::instrumented_allocator!();

fn main() -> readings_probe::ReadingsResult<()> {
    let mut probe =
        readings_probe::Probe::new(std::fs::File::create("readings.out").unwrap()).unwrap();
    let progress = probe.register_i64("done".to_string())?;
    probe.spawn_heartbeat(Duration::from_millis(1000))?;
    let mut vec = vec![];
    for i in 0..5 {
        std::thread::sleep(Duration::from_millis(3000));
        vec.push(vec![i; 100000]);
        progress.store(i, std::sync::atomic::Ordering::Relaxed);
    }
    probe.log_event("about to drop buffers")?;
    std::mem::drop(vec);
    probe.log_event("done")?;
    Ok(())
}
