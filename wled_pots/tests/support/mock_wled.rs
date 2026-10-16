use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use tiny_http::{Response, Server};

/// Minimal mock of a WLED device: serves `GET /json/pins` from a mutable
/// body, so tests can simulate a potentiometer's value changing between
/// polls. Binds to an OS-assigned port (127.0.0.1:0) to avoid clashes.
pub struct MockWled {
    pub addr: SocketAddr,
    body: Arc<Mutex<String>>,
    pub request_count: Arc<AtomicUsize>,
    status_500: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

impl MockWled {
    pub fn start(initial_body: &str) -> Self {
        let server = Server::http("127.0.0.1:0").expect("failed to bind mock WLED server");
        let addr = server.server_addr().to_ip().expect("expected a bound IP address");

        let body = Arc::new(Mutex::new(initial_body.to_string()));
        let request_count = Arc::new(AtomicUsize::new(0));
        let status_500 = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));

        let body_handle = body.clone();
        let count_handle = request_count.clone();
        let status_500_handle = status_500.clone();
        let stop_handle = stop.clone();

        thread::spawn(move || {
            while !stop_handle.load(Ordering::Relaxed) {
                let request = match server.recv_timeout(std::time::Duration::from_millis(50)) {
                    Ok(Some(request)) => request,
                    Ok(None) => continue,
                    Err(_) => break,
                };

                count_handle.fetch_add(1, Ordering::Relaxed);

                if status_500_handle.load(Ordering::Relaxed) {
                    let response = Response::from_string("internal error").with_status_code(500);
                    let _ = request.respond(response);
                    continue;
                }

                let body = body_handle.lock().unwrap().clone();
                let response = Response::from_string(body);
                let _ = request.respond(response);
            }
        });

        MockWled {
            addr,
            body,
            request_count,
            status_500,
            stop,
        }
    }

    pub fn set_body(&self, body: &str) {
        *self.body.lock().unwrap() = body.to_string();
    }

    pub fn set_status_500(&self, enabled: bool) {
        self.status_500.store(enabled, Ordering::Relaxed);
    }

    pub fn request_count(&self) -> usize {
        self.request_count.load(Ordering::Relaxed)
    }
}

impl Drop for MockWled {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
