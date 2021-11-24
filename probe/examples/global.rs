use std::time::Duration;

fn main() -> readings_probe::ReadingsResult<()> {
    let mut probe =
        readings_probe::Probe::new(std::fs::File::create("readings.out").unwrap()).unwrap();
    probe.spawn_heartbeat(Duration::from_millis(1000))?;
    readings_probe::global::set(probe);
    some_other_code();
    readings_probe::global::log_event("done").unwrap();
    Ok(())
}

fn some_other_code() {
    let mut vec = vec![];
    for i in 0..5 {
        std::thread::sleep(Duration::from_millis(3000));
        vec.push(vec![i; 100000]);
    }
    readings_probe::global::log_event("about to drop buffers").unwrap();
    std::mem::drop(vec);
}
