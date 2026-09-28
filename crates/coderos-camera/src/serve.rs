//! The daemon: the camera in, three outputs out, and the control socket.
//!
//! One thread reads the camera and hands each frame to the fan-out; the
//! loopback, the recorder, and the tracker each run on the thread the
//! fan-out gave them; the control socket answers on threads of its own
//! and reaches the outputs through the state they share.

use crate::capture::Capture;
use crate::config::Grant;
use crate::control::{self, Handler};
use crate::fanout::{Fanout, LaneCount, Sink};
use crate::frame::Frame;
use crate::hands::{HandsSink, HandsState};
use crate::loopback::{LoopbackSink, LoopbackState};
use crate::paths;
use crate::protocol::{Output, Receipt, Refusal, Reply, StatusReport, Verb, refusal};
use crate::publisher::Publisher;
use crate::record::{Recorder, default_path};
use std::collections::VecDeque;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// What the camera thread reports and the control socket reads.
#[derive(Default)]
struct CameraState {
    format: String,
    width: u32,
    height: u32,
    frames: u64,
    /// The moments of the frames read in the last second.
    recent: VecDeque<Instant>,
}

impl CameraState {
    fn fps(&self) -> f64 {
        let now = Instant::now();
        self.recent
            .iter()
            .filter(|at| now.duration_since(**at) <= Duration::from_secs(1))
            .count() as f64
    }
}

/// The recording slot the control socket and the sink share.
#[derive(Default)]
struct RecordSlot {
    recorder: Option<Recorder>,
    last_error: Option<String>,
    last: Option<Receipt>,
}

struct RecordSink {
    slot: Arc<Mutex<RecordSlot>>,
}

impl Sink for RecordSink {
    fn name(&self) -> &'static str {
        "record"
    }

    fn accept(&mut self, frame: &Arc<Frame>, _dropped: u64) {
        let Ok(mut slot) = self.slot.lock() else {
            return;
        };
        let Some(recorder) = slot.recorder.as_mut() else {
            return;
        };
        if let Err(err) = recorder.write(frame) {
            eprintln!("coderos-camera: the recording stopped: {err}");
            slot.last_error = Some(err);
            if let Some(recorder) = slot.recorder.take() {
                match recorder.stop() {
                    Ok(receipt) => {
                        eprintln!("Recording saved: {}", receipt.path);
                        slot.last = Some(receipt);
                    }
                    Err(err) => eprintln!("coderos-camera: {err}"),
                }
            }
        }
    }
}

/// Everything the control socket answers from.
struct Shared {
    grant: Grant,
    camera: Mutex<CameraState>,
    camera_status: Mutex<String>,
    loopback: Option<Arc<Mutex<LoopbackState>>>,
    record: Arc<Mutex<RecordSlot>>,
    hands: Arc<HandsState>,
    hands_socket: PathBuf,
    readers: Arc<Mutex<Vec<UnixStream>>>,
    fanout: Arc<Mutex<Fanout>>,
    /// A signal or the `stop` verb asked the daemon to end.
    stopping: Arc<AtomicBool>,
}

impl Shared {
    /// One lane's counts, or zeros for a lane the fan-out does not have.
    fn lane(&self, name: &str) -> LaneCount {
        self.fanout
            .lock()
            .ok()
            .and_then(|fanout| fanout.counts().into_iter().find(|c| c.name == name))
            .unwrap_or(LaneCount {
                name: "",
                delivered: 0,
                dropped: 0,
            })
    }

    fn outputs(&self) -> Vec<Output> {
        let mut outputs = Vec::new();
        if let Some(loopback) = &self.loopback {
            let lane = self.lane("loopback");
            outputs.push(Output {
                name: "loopback".into(),
                target: self.grant.loopback.clone(),
                state: loopback.lock().map(|s| s.word()).unwrap_or_default(),
                frames: lane.delivered,
                dropped: lane.dropped,
            });
        }
        let (target, state) = match self.record.lock() {
            Ok(slot) => match (&slot.recorder, &slot.last_error) {
                (Some(recorder), _) => (
                    Some(recorder.path().to_string_lossy().into_owned()),
                    format!("recording, {} frames", recorder.frames()),
                ),
                (None, Some(err)) => (None, format!("off; the last recording ended: {err}")),
                (None, None) => (None, "off".into()),
            },
            Err(_) => (None, "off".into()),
        };
        let lane = self.lane("record");
        outputs.push(Output {
            name: "record".into(),
            target,
            state,
            frames: lane.delivered,
            dropped: lane.dropped,
        });
        let readers = self.readers.lock().map(|r| r.len()).unwrap_or(0);
        let lane = self.lane("hands");
        outputs.push(Output {
            name: "hands".into(),
            target: Some(self.hands_socket.to_string_lossy().into_owned()),
            state: if self.hands.is_on() {
                format!("on, {readers} reader(s)")
            } else {
                format!("off, {readers} reader(s)")
            },
            frames: lane.delivered,
            dropped: lane.dropped,
        });
        outputs
    }

    fn status(&self) -> StatusReport {
        let camera = self.camera.lock().ok();
        let (format, width, height, frames, fps) = match camera.as_deref() {
            Some(c) if c.frames > 0 => (c.format.clone(), c.width, c.height, c.frames, c.fps()),
            _ => (
                self.camera_status
                    .lock()
                    .map(|s| s.clone())
                    .unwrap_or_default(),
                self.grant.width,
                self.grant.height,
                0,
                0.0,
            ),
        };
        StatusReport {
            device: self.grant.device.clone(),
            format,
            width,
            height,
            frames,
            fps,
            hands: self.hands.status(),
            recording: self.record.lock().ok().and_then(|slot| {
                slot.recorder
                    .as_ref()
                    .map(|r| r.path().to_string_lossy().into_owned())
            }),
            outputs: self.outputs(),
        }
    }

    fn record_start(&self, path: Option<String>) -> Reply {
        let Ok(mut slot) = self.record.lock() else {
            return Reply::Refused(Refusal::new(
                refusal::CANNOT_RECORD,
                "the recording slot is poisoned",
            ));
        };
        if let Some(recorder) = &slot.recorder {
            return Reply::Refused(Refusal::new(
                refusal::ALREADY_RECORDING,
                format!(
                    "a recording is already writing {}",
                    recorder.path().display()
                ),
            ));
        }
        let path = match path {
            Some(path) => PathBuf::from(path),
            None => {
                let home = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("/tmp"));
                default_path(&home, crate::frame::now() as u64)
            }
        };
        let (width, height) = self
            .camera
            .lock()
            .ok()
            .filter(|c| c.frames > 0)
            .map(|c| (c.width, c.height))
            .unwrap_or((self.grant.width, self.grant.height));
        match Recorder::start(&path, width, height, self.grant.framerate) {
            Ok(recorder) => {
                eprintln!("coderos-camera: recording to {}", path.display());
                slot.recorder = Some(recorder);
                slot.last_error = None;
                Reply::Done
            }
            Err(err) => Reply::Refused(Refusal::new(refusal::CANNOT_RECORD, err)),
        }
    }

    fn record_stop(&self) -> Reply {
        let recorder = match self.record.lock() {
            Ok(mut slot) => slot.recorder.take(),
            Err(_) => None,
        };
        let Some(recorder) = recorder else {
            return Reply::Refused(Refusal::new(
                refusal::NO_RECORDING,
                "no recording is running",
            ));
        };
        match recorder.stop() {
            Ok(receipt) => {
                eprintln!("Recording saved: {}", receipt.path);
                if let Ok(mut slot) = self.record.lock() {
                    slot.last = Some(receipt.clone());
                }
                Reply::Recorded(receipt)
            }
            Err(err) => {
                if let Ok(mut slot) = self.record.lock() {
                    slot.last_error = Some(err.clone());
                }
                Reply::Refused(Refusal::new(refusal::CANNOT_RECORD, err))
            }
        }
    }

    fn answer(&self, verb: Verb) -> Reply {
        match verb {
            Verb::Status => Reply::Status(self.status()),
            Verb::Outputs => Reply::Outputs {
                outputs: self.outputs(),
            },
            Verb::RecordStart { path } => self.record_start(path),
            Verb::RecordStop => self.record_stop(),
            Verb::HandsOn => {
                self.hands.turn(true);
                Reply::Done
            }
            Verb::HandsOff => {
                self.hands.turn(false);
                Reply::Done
            }
            Verb::Stop => {
                self.stopping.store(true, Ordering::SeqCst);
                Reply::Done
            }
            Verb::Unknown => Reply::Refused(Refusal::new(
                refusal::UNKNOWN_VERB,
                "this daemon does not know that verb",
            )),
        }
    }
}

/// Runs the daemon until `SIGTERM`, `SIGINT`, or the `stop` verb ends
/// it. All three set the same flag, so the shutdown is one path: the
/// sinks finish the frames they hold and close, the recording gets the
/// same ending `record stop` gives one, and the socket files go with
/// the process.
pub fn run() -> Result<(), String> {
    let grant = Grant::load()?;
    let control_socket = paths::control_socket();
    let hands_socket = paths::hands_socket();

    // The flag the signals and the `stop` verb share. The handlers are
    // registered only outside a test build: one that stayed would catch
    // the harness's own `SIGTERM`, and a test drives the stop verb over
    // the control socket, which is the same path.
    let stopping = Arc::new(AtomicBool::new(false));
    #[cfg(not(test))]
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        signal_hook::flag::register(signal, Arc::clone(&stopping))
            .map_err(|err| format!("the signal handler: {err}"))?;
    }

    let mut fanout = Fanout::new();
    let loopback = match &grant.loopback {
        Some(node) => {
            let (sink, state) = LoopbackSink::new(node, grant.width, grant.height);
            fanout.add(Box::new(sink))?;
            Some(state)
        }
        None => None,
    };
    let record = Arc::new(Mutex::new(RecordSlot::default()));
    fanout.add(Box::new(RecordSink {
        slot: Arc::clone(&record),
    }))?;
    let hands = HandsState::new();
    let publisher = Publisher::bind(&hands_socket)?;
    let readers = publisher.reader_count();
    fanout.add(Box::new(HandsSink::new(Arc::clone(&hands), publisher)))?;
    let fanout = Arc::new(Mutex::new(fanout));

    let shared = Arc::new(Shared {
        grant: grant.clone(),
        camera: Mutex::new(CameraState::default()),
        camera_status: Mutex::new("opening".into()),
        loopback,
        record,
        hands,
        hands_socket: hands_socket.clone(),
        readers,
        fanout: Arc::clone(&fanout),
        stopping: Arc::clone(&stopping),
    });
    let handler: Handler = {
        let shared = Arc::clone(&shared);
        Arc::new(move |verb| shared.answer(verb))
    };
    control::serve(&control_socket, handler)?;
    eprintln!(
        "coderos-camera: reads {} at {}x{} {} fps; control socket {}, hands socket {}, loopback {}",
        grant.device,
        grant.width,
        grant.height,
        grant.framerate,
        control_socket.display(),
        hands_socket.display(),
        grant.loopback.as_deref().unwrap_or("none")
    );

    'camera: loop {
        let mut camera =
            match Capture::open(&grant.device, grant.width, grant.height, grant.framerate) {
                Ok(camera) => camera,
                Err(err) => {
                    set_camera_status(&shared, &err);
                    eprintln!("coderos-camera: {err}; trying again in 2 seconds");
                    if wait(&stopping, Duration::from_secs(2)) {
                        break;
                    }
                    continue;
                }
            };
        eprintln!(
            "coderos-camera: {} streams {} at {}x{}",
            grant.device, camera.fourcc, camera.width, camera.height
        );
        set_camera_status(&shared, "streaming");
        loop {
            if stopping.load(Ordering::SeqCst) {
                break 'camera;
            }
            match camera.next() {
                Ok(frame) => {
                    if let Ok(mut state) = shared.camera.lock() {
                        state.format = camera.fourcc.clone();
                        state.width = frame.width;
                        state.height = frame.height;
                        state.frames += 1;
                        let now = Instant::now();
                        state.recent.push_back(now);
                        while state
                            .recent
                            .front()
                            .is_some_and(|at| now.duration_since(*at) > Duration::from_secs(1))
                        {
                            state.recent.pop_front();
                        }
                    }
                    if let Ok(mut fanout) = fanout.lock() {
                        fanout.publish(Arc::new(frame));
                    }
                }
                Err(err) => {
                    set_camera_status(&shared, &err);
                    eprintln!("coderos-camera: {err}; opening {} again", grant.device);
                    wait(&stopping, Duration::from_millis(500));
                    break;
                }
            }
        }
        if stopping.load(Ordering::SeqCst) {
            break;
        }
    }

    shutdown(&shared, &[&control_socket, &hands_socket]);
    Ok(())
}

/// Ends the daemon's outputs and unbinds its sockets. The lanes stop
/// first so a sink mid-frame finishes it, which lets the recording's
/// last delivered frame land before `record_stop` ends it the way the
/// `record stop` verb does.
fn shutdown(shared: &Shared, sockets: &[&Path]) {
    if let Ok(mut fanout) = shared.fanout.lock() {
        fanout.stop();
    }
    if let Reply::Refused(refusal) = shared.record_stop()
        && refusal.code != refusal::NO_RECORDING
    {
        eprintln!(
            "coderos-camera: the recording did not stop cleanly: {}",
            refusal.message
        );
    }
    for socket in sockets {
        let _ = std::fs::remove_file(socket);
    }
}

/// Sleeps up to `duration`, waking early when a stop is asked for, and
/// answers whether one was.
fn wait(stopping: &AtomicBool, duration: Duration) -> bool {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if stopping.load(Ordering::SeqCst) {
            return true;
        }
        thread::sleep(Duration::from_millis(50).min(deadline - Instant::now()));
    }
    stopping.load(Ordering::SeqCst)
}

fn set_camera_status(shared: &Shared, status: &str) {
    if let Ok(mut s) = shared.camera_status.lock() {
        *s = status.to_string();
    }
}

/// The socket the command asks, for `main`.
pub fn control_path() -> PathBuf {
    paths::control_socket()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control;

    /// A daemon with no camera still serves the control socket, and the
    /// `stop` verb drives the same shutdown a signal does: the run ends
    /// and the sockets go with it.
    #[test]
    fn the_stop_verb_ends_the_run_and_removes_the_sockets() {
        let dir = std::env::temp_dir().join(format!("coderos-camera-serve-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        // The daemon's sockets and grant are the environment's. The grant
        // path names no file, so a host's own grant is never read, and the
        // device names no node, so no camera opens.
        //
        // SAFETY: no other test in this binary reads or writes these
        // variables, and they are set before the daemon's thread starts.
        unsafe {
            std::env::set_var("CODEROS_CAMERA_DIR", &dir);
            std::env::set_var("CODEROS_CAMERA_GRANT", dir.join("no-grant.json"));
            std::env::set_var("CODEROS_CAMERA_DEVICE", "/nonexistent/coderos-camera-test");
            std::env::set_var("CODEROS_CAMERA_LOOPBACK", "");
        }
        let control = paths::control_socket();
        let hands = paths::hands_socket();
        let daemon = thread::Builder::new()
            .name("camera-serve-test".into())
            .spawn(run)
            .expect("the daemon's thread");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !control.exists() {
            assert!(
                Instant::now() < deadline,
                "the control socket never came up"
            );
            thread::sleep(Duration::from_millis(10));
        }
        let reply = control::ask(&control, Verb::Stop).expect("the answer");
        assert_eq!(reply, Reply::Done);
        daemon
            .join()
            .expect("the daemon ended")
            .expect("the daemon ended cleanly");
        assert!(!control.exists(), "the control socket went with the daemon");
        assert!(!hands.exists(), "the hands socket went with the daemon");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
