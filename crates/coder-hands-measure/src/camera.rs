//! Where a run's landmarks come from, chosen when the recording starts.
//!
//! On CoderOS the camera daemon owns the camera and publishes one landmark
//! line a frame on its hands socket, and this module subscribes to it.
//!
//! In the private Coder repository a Mac recorded from its built-in camera
//! through a macOS tracker crate that did not move to this repository, so
//! here the daemon is the only camera a run records from. A run recorded
//! on a Mac still scores: its header names [`Source::Vision`], and the
//! scorer reads the file, not the camera.
//!
//! The source goes into the run's header. A run recorded from one camera
//! is not a run recorded from another, because cameras run at different
//! rates and frame the person differently, so the scorer names the source
//! in its first lines rather than letting two runs be compared as if they
//! were the same measurement.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use coder_hands::socket::{CAPTURE_VAR, Sockets, TRACK_ON, ask};
use coder_hands::wire::Line;

use crate::run::Source;

/// How long the recorder waits for the daemon's socket before it gives
/// up. The first `hands on` fetches the landmark model.
const CONNECT_WAIT: Duration = Duration::from_secs(30);

/// The camera a run is recorded from: where its frames arrive, what to
/// write in the header, and the capture size the rules measure a palm
/// against.
pub struct Camera {
    /// Every landmark line, in the order the camera published it.
    pub frames: Receiver<Line>,
    /// What the run's header says it was recorded from.
    pub source: Source,
    /// The capture size, such as `1280x720`, when the camera names one.
    pub capture: Option<String>,
}

/// Opens the camera this machine has and answers where its frames
/// arrive.
///
/// # Errors
///
/// Returns the sentence that says no camera daemon answered on this
/// machine.
pub fn open(say: impl Fn(&str)) -> Result<Camera, String> {
    let sockets = Sockets::from_process();
    if !answering(&sockets) {
        say("No camera daemon has a socket here yet; waiting for one.");
    }
    daemon(&sockets, say)
}

/// Whether the camera daemon is there to be read: either socket on disk
/// is the daemon's, and a host that starts the compositor before the
/// daemon has neither yet.
fn answering(sockets: &Sockets) -> bool {
    sockets.hands.exists() || sockets.control.exists()
}

/// The CoderOS camera daemon: ask it to track, then read its socket.
fn daemon(sockets: &Sockets, say: impl Fn(&str)) -> Result<Camera, String> {
    if let Err(error) = ask(&sockets.control, TRACK_ON) {
        say(&format!("The camera daemon was not told to track: {error}"));
    }
    let frames = subscribe(&sockets.hands, &say)?;
    Ok(Camera {
        frames,
        source: Source::Daemon,
        capture: std::env::var(CAPTURE_VAR)
            .ok()
            .filter(|size| !size.is_empty()),
    })
}

/// Connects to the hands socket, waiting while the daemon loads its
/// model, and reads it on a thread of its own.
fn subscribe(socket: &Path, say: &impl Fn(&str)) -> Result<Receiver<Line>, String> {
    let until = Instant::now() + CONNECT_WAIT;
    let mut said = false;
    let stream = loop {
        match UnixStream::connect(socket) {
            Ok(stream) => break stream,
            Err(err) => {
                if Instant::now() >= until {
                    return Err(format!("no camera daemon at {}: {err}", socket.display()));
                }
                if !said {
                    said = true;
                    say(&format!(
                        "Waiting for the camera daemon at {}.",
                        socket.display()
                    ));
                }
                thread::sleep(Duration::from_millis(250));
            }
        }
    };
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("hands-record".into())
        .spawn(move || read(stream, &tx))
        .map_err(|err| format!("the reader did not start: {err}"))?;
    Ok(rx)
}

/// Every line off the socket, to the loop that writes them. It ends when
/// the daemon closes the socket or the loop stops reading.
fn read(stream: UnixStream, tx: &Sender<Line>) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let mut lines = BufReader::new(stream);
    let mut text = String::new();
    loop {
        text.clear();
        match lines.read_line(&mut text) {
            Ok(0) => return,
            Ok(_) => {
                if let Ok(line) = Line::parse(&text)
                    && tx.send(line).is_err()
                {
                    return;
                }
            }
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_with_no_socket_on_disk_has_no_daemon_to_read() {
        let dir = std::env::temp_dir().join(format!("coder-hands-camera-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the scratch directory");
        let sockets = Sockets {
            hands: dir.join("hands.sock"),
            control: dir.join("control.sock"),
        };
        assert!(!answering(&sockets));
        std::fs::write(&sockets.control, b"").expect("a file where the socket goes");
        assert!(answering(&sockets));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
