use std::time::Duration;

readings::instrumented_allocator!();

fn main() -> readings::ReadingsResult<()> {
    let mut monitor = readings::Monitor::new(std::io::stdout());
    let progress = monitor.register_i64("done".to_string())?;
    monitor.spawn_heartbeat(Duration::from_millis(10))?;
    let mut vec = vec!();
    for i in 0..5 {
        std::thread::sleep(Duration::from_millis(30));
        vec.push(vec!(i; 100000));
        progress.store(i, std::sync::atomic::Ordering::Relaxed);
    }
    monitor.log_event("about to drop buffers")?;
    std::mem::drop(vec);
    monitor.log_event("done")?;
    Ok(())
}
