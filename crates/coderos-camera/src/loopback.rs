//! The loopback sink: every frame onto a `v4l2loopback` node, as YUYV.
//!
//! The node is what `mpv` for the camera circle, Zoom, and a browser in a
//! sandbox open when they ask for a camera, so `camera-overlay` and the
//! sandboxes change nothing but the device name. The daemon sets the
//! node's format once and writes each frame with `write`, which the
//! module reads as one picture. A node that is absent, because the
//! module is not loaded, is reported and tried again every few seconds,
//! so a daemon that started before the module still serves it once it
//! is there.

use crate::fanout::Sink;
use crate::frame::{Frame, rgb_to_yuyv};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long to wait before opening a node that failed again.
const RETRY: Duration = Duration::from_secs(5);

/// Where the loopback stands, for `status`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoopbackState {
    /// The node takes frames.
    Up,
    /// The node could not be opened, and why.
    Absent(String),
}

impl LoopbackState {
    pub fn word(&self) -> String {
        match self {
            LoopbackState::Up => "up".into(),
            LoopbackState::Absent(reason) => format!("absent: {reason}"),
        }
    }
}

pub struct LoopbackSink {
    path: String,
    width: u32,
    height: u32,
    state: Arc<Mutex<LoopbackState>>,
    node: Option<Node>,
    last_try: Option<Instant>,
}

impl LoopbackSink {
    pub fn new(path: &str, width: u32, height: u32) -> (LoopbackSink, Arc<Mutex<LoopbackState>>) {
        let state = Arc::new(Mutex::new(LoopbackState::Absent("not opened yet".into())));
        (
            LoopbackSink {
                path: path.to_string(),
                width,
                height,
                state: Arc::clone(&state),
                node: None,
                last_try: None,
            },
            state,
        )
    }

    fn set(&self, state: LoopbackState) {
        if let Ok(mut s) = self.state.lock() {
            *s = state;
        }
    }

    fn open(&mut self) {
        self.last_try = Some(Instant::now());
        match Node::open(&self.path, self.width, self.height) {
            Ok(node) => {
                self.node = Some(node);
                self.set(LoopbackState::Up);
                eprintln!("coderos-camera: the loopback {} takes frames", self.path);
            }
            Err(err) => {
                self.set(LoopbackState::Absent(err));
            }
        }
    }
}

impl Sink for LoopbackSink {
    fn name(&self) -> &'static str {
        "loopback"
    }

    fn accept(&mut self, frame: &Arc<Frame>, _dropped: u64) {
        if self.node.is_none() {
            let due = self.last_try.is_none_or(|at| at.elapsed() > RETRY);
            if due {
                self.open();
            }
        }
        let Some(node) = self.node.as_mut() else {
            return;
        };
        if frame.width != self.width || frame.height != self.height {
            return;
        }
        let yuyv = rgb_to_yuyv(&frame.rgb, frame.width, frame.height);
        if let Err(err) = node.write(&yuyv) {
            eprintln!(
                "coderos-camera: the loopback {} stopped taking frames: {err}",
                self.path
            );
            self.node = None;
            self.set(LoopbackState::Absent(err));
        }
    }
}

/// An open failure that names which half is missing. The module is
/// the one a switch added but the running kernel does not have yet: it
/// loads from the new system with
/// `modprobe -d /run/current-system/kernel-modules` or at the next reboot.
/// The node's permission is the video group or the seat ACL udev writes —
/// a module loaded by hand holds neither until the trigger runs. Only the
/// Linux node opens a real device, so a build for another platform
/// compiles the tests and nothing else.
#[cfg(any(target_os = "linux", test))]
fn describe_open(path: &str, err: &std::io::Error, module_dir: &std::path::Path) -> String {
    if !module_dir.exists() {
        format!(
            "{path}: {err}; the v4l2loopback module is not loaded \
             (modprobe -d /run/current-system/kernel-modules v4l2loopback, or a reboot)"
        )
    } else if err.kind() == std::io::ErrorKind::PermissionDenied {
        format!(
            "{path}: {err}; the node is there but this daemon may not open it \
             (the video group, or the seat ACL udev gives it)"
        )
    } else {
        format!("{path}: {err}")
    }
}

#[cfg(target_os = "linux")]
use linux::Node;

#[cfg(target_os = "linux")]
mod linux {
    use std::io::Write;
    use v4l::video::Output as _;
    use v4l::{Device, FourCC};

    /// An open loopback node with its format set.
    pub struct Node {
        device: Device,
    }

    impl Node {
        pub fn open(path: &str, width: u32, height: u32) -> Result<Node, String> {
            let device = Device::with_path(path).map_err(|err| {
                super::describe_open(path, &err, std::path::Path::new("/sys/module/v4l2loopback"))
            })?;
            let mut fmt = device
                .format()
                .map_err(|err| format!("{path} format: {err}"))?;
            fmt.width = width;
            fmt.height = height;
            fmt.fourcc = FourCC::new(b"YUYV");
            let set = device
                .set_format(&fmt)
                .map_err(|err| format!("{path} format: {err}"))?;
            if set.fourcc != FourCC::new(b"YUYV") || set.width != width || set.height != height {
                return Err(format!(
                    "{path} took {} {}x{} rather than YUYV {width}x{height}",
                    set.fourcc, set.width, set.height
                ));
            }
            Ok(Node { device })
        }

        pub fn write(&mut self, yuyv: &[u8]) -> Result<(), String> {
            self.device.write_all(yuyv).map_err(|err| err.to_string())
        }
    }
}

#[cfg(not(target_os = "linux"))]
use other::Node;

#[cfg(not(target_os = "linux"))]
mod other {
    /// A loopback on a platform with no `v4l2loopback`.
    pub struct Node;

    impl Node {
        pub fn open(path: &str, _width: u32, _height: u32) -> Result<Node, String> {
            Err(format!("{path}: this platform has no v4l2loopback"))
        }

        pub fn write(&mut self, _yuyv: &[u8]) -> Result<(), String> {
            Err("no loopback on this platform".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_node_that_is_not_there_is_reported_and_tried_again_later() {
        let (mut sink, state) = LoopbackSink::new("/nonexistent/coderos-camera/video10", 4, 4);
        assert_eq!(
            state.lock().expect("state").word(),
            "absent: not opened yet"
        );
        sink.accept(&Arc::new(Frame::solid(1, 4, 4, [0, 0, 0])), 0);
        let word = state.lock().expect("state").word();
        assert!(
            word.starts_with("absent: /nonexistent/coderos-camera/video10"),
            "{word}"
        );
        let first = sink.last_try;
        sink.accept(&Arc::new(Frame::solid(2, 4, 4, [0, 0, 0])), 0);
        assert_eq!(
            sink.last_try, first,
            "no second try inside the retry window"
        );
    }

    #[test]
    fn the_state_words_are_what_status_prints() {
        assert_eq!(LoopbackState::Up.word(), "up");
        assert_eq!(
            LoopbackState::Absent("no such file".into()).word(),
            "absent: no such file"
        );
    }

    #[test]
    fn an_open_failure_names_the_module_when_it_is_not_loaded() {
        let err = std::io::Error::from_raw_os_error(2);
        let text = describe_open("/dev/video10", &err, std::path::Path::new("/nonexistent"));
        assert!(
            text.contains("the v4l2loopback module is not loaded"),
            "{text}"
        );
        assert!(text.contains("current-system"), "{text}");
    }

    #[test]
    fn an_open_failure_names_the_permission_when_the_module_is_there() {
        let dir = std::env::temp_dir();
        let denied = std::io::Error::from_raw_os_error(13);
        let text = describe_open("/dev/video10", &denied, &dir);
        assert!(text.contains("video group"), "{text}");
        let missing = std::io::Error::from_raw_os_error(2);
        let text = describe_open("/dev/video10", &missing, &dir);
        assert_eq!(text, format!("/dev/video10: {missing}"));
    }
}
