use crate::{FrameData, ModuleChrome};
use nokhwa::pixel_format::RgbAFormat;
use nokhwa::utils::{ApiBackend, CameraIndex, CameraInfo, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Serializes the actual frame-grab call across every camera thread.
/// Two `nokhwa` `Camera` instances pulling frames concurrently from
/// different threads can end up delivering one camera's data into the
/// other's buffer — this forces frame grabs one at a time so that can't
/// happen, even though each camera otherwise runs on its own thread.
static FRAME_LOCK: Mutex<()> = Mutex::new(());

/// A decoded camera frame handed from the capture thread to the render loop.
struct CamFrame {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    version: u64,
}

/// Owns a background thread that pulls frames from one camera device.
pub struct CameraWorker {
    frame: Arc<Mutex<Option<CamFrame>>>,
    stop: Arc<AtomicBool>,
}

impl CameraWorker {
    pub fn spawn(index: CameraIndex) -> Self {
        let frame = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));

        let frame_handle = frame.clone();
        let stop_handle = stop.clone();
        thread::spawn(move || {
            let requested =
                RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
            let mut camera = match Camera::new(index, requested) {
                Ok(camera) => camera,
                Err(_) => return,
            };
            if camera.open_stream().is_err() {
                return;
            }

            let mut version = 0u64;
            while !stop_handle.load(Ordering::Relaxed) {
                let frame = {
                    let _guard = FRAME_LOCK.lock().unwrap();
                    camera.frame()
                };
                if let Ok(buffer) = frame {
                    if let Ok(decoded) = buffer.decode_image::<RgbAFormat>() {
                        version += 1;
                        let (width, height) = decoded.dimensions();
                        *frame_handle.lock().unwrap() = Some(CamFrame {
                            width,
                            height,
                            rgba: decoded.into_raw(),
                            version,
                        });
                    }
                }
            }
            let _ = camera.stop_stream();
        });

        CameraWorker { frame, stop }
    }

    pub fn latest(&self) -> Option<FrameData> {
        self.frame
            .lock()
            .unwrap()
            .as_ref()
            .map(|f| (f.width, f.height, f.rgba.clone(), f.version))
    }
}

impl Drop for CameraWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// The CAMERA module's own config: which device, and its capture thread.
pub struct CameraState {
    pub chrome: ModuleChrome,
    pub selected: Option<usize>,
    pub worker: Option<CameraWorker>,
    pub dropdown_open: bool,
}

impl CameraState {
    pub fn new() -> Self {
        CameraState {
            chrome: ModuleChrome::new(),
            selected: None,
            worker: None,
            dropdown_open: false,
        }
    }

    pub fn label(&self, available: &[CameraInfo]) -> String {
        match self.selected.and_then(|i| available.get(i)) {
            Some(info) => info.human_name().to_string(),
            None if available.is_empty() => "No cameras found".to_string(),
            None => "Select a camera".to_string(),
        }
    }

    /// Only records the pick; the worker lifecycle pass in `update` starts
    /// or stops the actual capture thread based on whether this module is
    /// in use, so an idle selection never keeps a device streaming.
    pub fn select(&mut self, choice: Option<usize>) {
        self.selected = choice;
        self.worker = None;
        self.chrome.preview_texture = None;
        self.chrome.preview_version = None;
    }
}

/// Enumerates connected cameras, deduplicated to one entry per physical
/// device (see `dedupe_cameras`).
pub fn discover_cameras() -> Vec<CameraInfo> {
    dedupe_cameras(nokhwa::query(ApiBackend::Auto).unwrap_or_default())
}

/// Many webcams register several V4L2/driver nodes for the same physical
/// sensor (this machine's integrated camera shows up as four, e.g. a
/// separate IR node for Windows Hello, or a metadata-only node alongside
/// the real capture node). Only one of them typically produces an actual
/// decodable frame. Probe every candidate concurrently and, per distinct
/// camera name, keep whichever one actually streams video — falling back
/// to the first entry if none of them do, so the camera still shows up.
fn dedupe_cameras(cameras: Vec<CameraInfo>) -> Vec<CameraInfo> {
    let mut groups: Vec<(String, Vec<CameraInfo>)> = Vec::new();
    for info in cameras {
        let name = info.human_name().to_string();
        match groups.iter_mut().find(|(n, _)| *n == name) {
            Some((_, list)) => list.push(info),
            None => groups.push((name, vec![info])),
        }
    }

    // Different physical cameras can be probed in parallel, but a camera's
    // sibling nodes share a USB interface, so probing two of them at once
    // would recreate the exact contention we're trying to detect around.
    // Try each camera's candidates one at a time within its own thread.
    let handles: Vec<_> = groups
        .into_iter()
        .map(|(_, candidates)| thread::spawn(move || pick_working_camera(candidates)))
        .collect();

    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

/// Tries a physical camera's candidate nodes in turn and keeps the first
/// one that actually produces a frame, falling back to the first candidate
/// if none of them do (so it still shows up, even if non-functional).
fn pick_working_camera(candidates: Vec<CameraInfo>) -> CameraInfo {
    for candidate in &candidates {
        if camera_produces_frame(candidate.index().clone()) {
            return candidate.clone();
        }
    }
    candidates.into_iter().next().expect("at least one candidate per group")
}

fn camera_produces_frame(index: CameraIndex) -> bool {
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let requested =
            RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
        let ok = (|| -> Result<(), nokhwa::NokhwaError> {
            let mut camera = Camera::new(index, requested)?;
            camera.open_stream()?;
            // The first frame or two often isn't ready the instant the
            // stream opens (auto-exposure/auto-focus warm-up on real UVC
            // hardware), so a single immediate attempt reads as a false
            // negative on a perfectly working camera. Retry briefly before
            // giving up.
            let mut last_err = None;
            for _ in 0..8 {
                let result = {
                    let _guard = FRAME_LOCK.lock().unwrap();
                    camera.frame()
                };
                match result {
                    Ok(_) => return Ok(()),
                    Err(err) => last_err = Some(err),
                }
                thread::sleep(Duration::from_millis(200));
            }
            Err(last_err.expect("loop ran at least once"))
        })()
        .is_ok();
        let _ = tx.send(ok);
    });
    rx.recv_timeout(Duration::from_millis(2500)).unwrap_or(false)
}
