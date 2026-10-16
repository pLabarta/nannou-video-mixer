use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::client::WledClient;
use crate::model::Pin;

#[derive(Debug, Clone, Default)]
pub struct DeviceReading {
    pub pins: Vec<Pin>,
    pub last_success: Option<Instant>,
    pub last_error: Option<String>,
}

// How often the sleep-between-polls loop re-checks the shared interval and
// the stop flag. Bounds how long `set_poll_interval` takes to kick in, and
// how quickly a dropped poller actually stops.
const TICK: Duration = Duration::from_millis(10);

/// Owns a background thread that repeatedly polls one WLED device's
/// `/json/pins`. The poll interval is shared (via `Arc<Mutex<Duration>>`)
/// with the owning `WledManager`, and re-checked every `TICK` while
/// sleeping so `set_poll_interval` takes effect promptly without restarting
/// the thread.
pub struct DevicePoller {
    reading: Arc<Mutex<DeviceReading>>,
    stop: Arc<AtomicBool>,
}

impl DevicePoller {
    pub fn spawn(client: WledClient, interval: Arc<Mutex<Duration>>) -> Self {
        let reading = Arc::new(Mutex::new(DeviceReading::default()));
        let stop = Arc::new(AtomicBool::new(false));

        let reading_handle = reading.clone();
        let stop_handle = stop.clone();
        thread::spawn(move || {
            while !stop_handle.load(Ordering::Relaxed) {
                match client.fetch_pins() {
                    Ok(resp) => {
                        let mut r = reading_handle.lock().unwrap();
                        r.pins = resp.pins;
                        r.last_success = Some(Instant::now());
                        r.last_error = None;
                    }
                    Err(err) => {
                        // Network/parse failures keep the last-known-good
                        // reading rather than crashing the poll thread.
                        reading_handle.lock().unwrap().last_error = Some(err.to_string());
                    }
                }

                let mut slept = Duration::ZERO;
                loop {
                    if stop_handle.load(Ordering::Relaxed) {
                        return;
                    }
                    let target = *interval.lock().unwrap();
                    if slept >= target {
                        break;
                    }
                    let chunk = TICK.min(target - slept);
                    thread::sleep(chunk);
                    slept += chunk;
                }
            }
        });

        DevicePoller { reading, stop }
    }

    pub fn reading(&self) -> DeviceReading {
        self.reading.lock().unwrap().clone()
    }
}

impl Drop for DevicePoller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
