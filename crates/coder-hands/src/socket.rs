//! Where the camera daemon keeps its sockets, and the one verb a reader
//! asks it.
//!
//! `crates/coderos-camera` publishes landmarks on a socket under
//! `$XDG_RUNTIME_DIR/coderos-camera/` and answers control requests on
//! another beside it. Two programs read them: `crates/coder-compositor`,
//! which turns landmarks into desk input, and `crates/coder-hands-measure`,
//! which records a labelled run at the camera. They resolve the paths
//! through this module, so a test that points either one at a stand-in
//! daemon points both the same way.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The variable that names the hands socket, for a run beside a daemon
/// that keeps its sockets elsewhere.
pub const SOCKET_VAR: &str = "CODEROS_HANDS_SOCKET";

/// The variable the daemon itself reads for the directory its sockets
/// live in, which a reader honors the same way.
pub const DIR_VAR: &str = "CODEROS_CAMERA_DIR";

/// The variable the session sets to the camera's capture size, such as
/// `1280x720`, which gives the frame its aspect ratio.
pub const CAPTURE_VAR: &str = "CODEROS_CAMERA_CAPTURE";

/// How long a control request may take. The first `hands on` fetches the
/// landmark model, which is why a caller runs it off its own loop.
pub const CONTROL_TIMEOUT: Duration = Duration::from_secs(120);

/// The verb that starts the daemon's tracker, which is
/// `crates/coderos-camera`'s `Verb::HandsOn` as its tag serializes.
pub const TRACK_ON: &str = "hands_on";

/// The verb that stops it, `Verb::HandsOff`.
pub const TRACK_OFF: &str = "hands_off";

/// Where the daemon keeps its sockets: the hands socket, and the control
/// socket beside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sockets {
    pub hands: PathBuf,
    pub control: PathBuf,
}

impl Sockets {
    /// The sockets the environment names: [`SOCKET_VAR`] for the hands
    /// socket, [`DIR_VAR`] for the directory, and otherwise
    /// `coderos-camera` under the runtime directory, the way the daemon
    /// picks them.
    pub fn from_environment(value: impl Fn(&str) -> Option<String>) -> Sockets {
        let dir = value(DIR_VAR)
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                value("XDG_RUNTIME_DIR")
                    .filter(|dir| !dir.is_empty())
                    .map(|dir| PathBuf::from(dir).join("coderos-camera"))
            })
            .unwrap_or_else(|| {
                let home = value("HOME").unwrap_or_else(|| "/tmp".to_string());
                PathBuf::from(home).join(".openagents/camera")
            });
        let hands = value(SOCKET_VAR)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| dir.join("hands.sock"));
        Sockets {
            hands,
            control: dir.join("control.sock"),
        }
    }

    /// The sockets this process's own environment names.
    pub fn from_process() -> Sockets {
        Sockets::from_environment(|name| std::env::var(name).ok())
    }
}

/// Asks the daemon one verb over its control socket, in the line shape
/// `crates/coderos-camera/src/protocol.rs` reads, and answers whether it
/// said `done`.
pub fn ask(control: &Path, verb: &str) -> Result<(), String> {
    let mut stream = UnixStream::connect(control)
        .map_err(|err| format!("no camera daemon at {}: {err}", control.display()))?;
    let _ = stream.set_read_timeout(Some(CONTROL_TIMEOUT));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let request = format!("{{\"generation\":1,\"type\":\"{verb}\"}}\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|err| format!("{}: {err}", control.display()))?;
    let mut answer = String::new();
    BufReader::new(stream)
        .read_line(&mut answer)
        .map_err(|err| format!("{}: {err}", control.display()))?;
    let value: serde_json::Value = serde_json::from_str(answer.trim())
        .map_err(|err| format!("the daemon's answer did not parse: {err}"))?;
    match value.get("type").and_then(|kind| kind.as_str()) {
        Some("done") => Ok(()),
        Some("refused") => Err(format!(
            "the daemon refused {verb}: {}",
            value
                .get("message")
                .and_then(|message| message.as_str())
                .unwrap_or("no reason")
        )),
        other => Err(format!(
            "the daemon answered {verb} with {}",
            other.unwrap_or("nothing")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.to_string())
        }
    }

    #[test]
    fn the_sockets_sit_under_the_runtime_directory_unless_named() {
        let sockets = Sockets::from_environment(env(&[("XDG_RUNTIME_DIR", "/run/user/1000")]));
        assert_eq!(
            sockets.hands,
            PathBuf::from("/run/user/1000/coderos-camera/hands.sock")
        );
        assert_eq!(
            sockets.control,
            PathBuf::from("/run/user/1000/coderos-camera/control.sock")
        );
        let named = Sockets::from_environment(env(&[
            ("XDG_RUNTIME_DIR", "/run/user/1000"),
            (DIR_VAR, "/tmp/cam"),
            (SOCKET_VAR, "/tmp/fake/hands.sock"),
        ]));
        assert_eq!(named.hands, PathBuf::from("/tmp/fake/hands.sock"));
        assert_eq!(named.control, PathBuf::from("/tmp/cam/control.sock"));
        let home = Sockets::from_environment(env(&[("HOME", "/home/me")]));
        assert_eq!(
            home.hands,
            PathBuf::from("/home/me/.openagents/camera/hands.sock")
        );
    }

    #[test]
    fn the_verbs_are_the_daemons_own() {
        assert_eq!(TRACK_ON, "hands_on");
        assert_eq!(TRACK_OFF, "hands_off");
    }

    #[test]
    fn a_missing_daemon_is_named_and_a_done_answer_is_taken() {
        let dir = std::env::temp_dir().join(format!("coder-hands-ask-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the scratch directory");
        let err = ask(&dir.join("none.sock"), "hands_on").unwrap_err();
        assert!(err.starts_with("no camera daemon at "), "{err}");
        let socket = dir.join("control.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind");
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("a connection");
            let mut request = String::new();
            BufReader::new(&stream)
                .read_line(&mut request)
                .expect("the request");
            let mut stream = &stream;
            stream
                .write_all(b"{\"generation\":1,\"type\":\"done\"}\n")
                .expect("the answer");
            request
        });
        assert_eq!(ask(&socket, "hands_on"), Ok(()));
        let request = server.join().expect("the server");
        assert_eq!(request.trim(), "{\"generation\":1,\"type\":\"hands_on\"}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
