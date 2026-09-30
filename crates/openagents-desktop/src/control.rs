//! The window's side of the host's local control socket.
//!
//! The host serves the control protocol on a Unix socket (`0600`, in a
//! `0700` directory) and answers only a peer whose user ID equals its own
//! ([spec](../../../docs/coder/design/2026-09-29-auto-pairing.md), "The local
//! owner surface"). The window is one such client; it asks for codes, lists
//! and removes phones, and sets the project and auto-start policy. It never
//! reads a key.
//!
//! The message types are `openagents-connect`'s (`openagents.control.v1`):
//! each message is a 4-byte big-endian length and that many bytes of JSON,
//! one [`Request`] and its [`Response`] with the same `id`. That crate's
//! client is async; the window's is this small blocking one, run on the
//! worker thread. [`HostControl`] is the seam the model and the tests use;
//! [`SocketControl`] is the real client and [`crate::fake::FakeHost`] the
//! in-process one.

pub use openagents_connect::control::{
    Autostart, Device, MAX_MESSAGE_BYTES, NearbyPrompt, Op, Project, Reply, Request, Response,
    SOCKET_NAME, Status, VERSION, socket_path, socket_path_for,
};
use serde::Serialize;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

/// How long one request may take before the window treats the host as
/// unreachable.
pub const TIMEOUT: Duration = Duration::from_secs(4);

impl std::fmt::Debug for Invite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Invite")
            .field("invitation", &self.invitation)
            .field("expires_at", &self.expires_at)
            .field("rights", &self.rights)
            .finish_non_exhaustive()
    }
}

/// Whether `device` may open a terminal.
pub fn terminal(device: &Device) -> bool {
    device.rights.iter().any(|right| right == "terminal")
}

/// A fresh code. `code` is a bearer secret until it is redeemed or
/// cancelled; `Debug` leaves it out.
#[derive(Clone, PartialEq, Eq)]
pub struct Invite {
    pub invitation: String,
    pub code: String,
    pub expires_at: u64,
    pub rights: Vec<String>,
}

/// Why a request failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlError {
    /// No host answers the socket: it is not running yet, or not this
    /// user's.
    Unreachable,
    /// The host answered with a refusal.
    Refused { code: String, message: String },
    /// The host answered with something this client can't read.
    Malformed,
}

impl std::fmt::Display for ControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable => f.write_str("the host does not answer its control socket"),
            Self::Refused { code, message } => write!(f, "the host refused ({code}): {message}"),
            Self::Malformed => f.write_str("the host's answer is malformed"),
        }
    }
}

impl std::error::Error for ControlError {}

pub type ControlResult<T> = Result<T, ControlError>;

/// The operations the window uses, over any transport.
pub trait HostControl: Send {
    fn status(&mut self) -> ControlResult<Status>;
    fn invite(&mut self) -> ControlResult<Invite>;
    fn cancel(&mut self, invitation: &str) -> ControlResult<u32>;
    fn cancel_all(&mut self) -> ControlResult<u32>;
    fn devices(&mut self) -> ControlResult<Vec<Device>>;
    fn revoke(&mut self, device: &str) -> ControlResult<()>;
    fn autostart(&mut self) -> ControlResult<Autostart>;
    fn set_autostart(&mut self, policy: Autostart) -> ControlResult<Autostart>;
    fn projects(&mut self) -> ControlResult<Vec<Project>>;
    fn add_project(&mut self, path: &str) -> ControlResult<Vec<Project>>;
    /// Takes the project `label` off the host; the host drops it from the
    /// auto-start policy too.
    fn remove_project(&mut self, label: &str) -> ControlResult<Vec<Project>>;
    /// The phone nearby waiting for a click (`DSK-04`), if any.
    fn nearby_pending(&mut self) -> ControlResult<Option<NearbyPrompt>>;
    /// **Connect** or **Don't connect** for the nearby request `id`.
    fn nearby_decide(&mut self, id: u64, connect: bool) -> ControlResult<()>;
}

/// Why [`pick_project`] stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickError {
    /// The host would not take the folder.
    Folder,
    /// The folder is in, but the old project or the switch did not follow.
    Setting,
}

/// How many times a call after a project change is tried while the host
/// starts again, and how long apart.
const AGAIN: (u32, Duration) = (20, Duration::from_millis(250));

/// Runs `call`, trying again while the host is starting again after a
/// project change (it is unreachable for a moment).
fn again<T>(
    control: &mut dyn HostControl,
    mut call: impl FnMut(&mut dyn HostControl) -> ControlResult<T>,
) -> ControlResult<T> {
    let mut tries = 1;
    loop {
        match call(control) {
            Err(ControlError::Unreachable) if tries < AGAIN.0 => {
                tries += 1;
                std::thread::sleep(AGAIN.1);
            }
            result => return result,
        }
    }
}

/// The label the host gave the folder at `path`: the one project that was
/// not there before, else (a folder it already had) the one whose picked
/// folder or path is `path`.
fn label_of(before: &[Project], after: &[Project], path: &str) -> Option<String> {
    let new: Vec<&Project> = after
        .iter()
        .filter(|project| before.iter().all(|old| old.label != project.label))
        .collect();
    if let [only] = new.as_slice() {
        return Some(only.label.clone());
    }
    let canonical = std::fs::canonicalize(path)
        .ok()
        .map(|path| path.display().to_string());
    after
        .iter()
        .find(|project| {
            [project.folder.as_deref(), Some(project.path.as_str())]
                .into_iter()
                .flatten()
                .any(|held| held == path || Some(held) == canonical.as_deref())
        })
        .map(|project| project.label.clone())
}

/// Makes the folder at `path` the project the window shows: admits it,
/// takes off `replace` (the project it shows now, when it is another one),
/// and points the auto-start policy at the new project's label, the one
/// the host gave it, with the switch `autostart`. The policy keeps any
/// other project the host still admits and names no project the host no
/// longer has, so a phone's task in the shown project starts when the
/// switch is on.
pub fn pick_project(
    control: &mut dyn HostControl,
    path: &str,
    replace: Option<&str>,
    autostart: bool,
) -> Result<(), PickError> {
    let before = again(control, |c| c.projects()).unwrap_or_default();
    let mut after = control.add_project(path).map_err(|_| PickError::Folder)?;
    let label = label_of(&before, &after, path).ok_or(PickError::Setting)?;
    if let Some(old) = replace.filter(|old| *old != label) {
        after = again(control, |c| c.remove_project(old)).map_err(|_| PickError::Setting)?;
    }
    let policy = again(control, |c| c.autostart()).map_err(|_| PickError::Setting)?;
    let mut projects: Vec<String> = policy
        .projects
        .into_iter()
        .filter(|held| *held != label && after.iter().any(|project| project.label == *held))
        .collect();
    projects.push(label);
    again(control, |c| {
        c.set_autostart(Autostart {
            enabled: autostart,
            projects: projects.clone(),
            max_running: policy.max_running.max(1),
        })
    })
    .map(|_| ())
    .map_err(|_| PickError::Setting)
}

/// The blocking client for the host's socket. One connection a request.
#[derive(Debug)]
pub struct SocketControl {
    path: PathBuf,
    next: u64,
}

impl SocketControl {
    pub fn new(path: PathBuf) -> SocketControl {
        SocketControl { path, next: 1 }
    }

    /// Sends `op` and returns the host's reply; a `refused` reply is an
    /// error.
    pub fn call(&mut self, op: Op) -> ControlResult<Reply> {
        let id = self.next;
        self.next += 1;
        let request = Request::new(id, op);
        let mut stream = self.connect()?;
        write_message(&mut stream, &request)?;
        let response: Response = read_message(&mut stream)?;
        if response.v != VERSION || response.id != id {
            return Err(ControlError::Malformed);
        }
        match response.result {
            Reply::Refused { code, message } => Err(ControlError::Refused { code, message }),
            reply => Ok(reply),
        }
    }

    /// One connection to the host's Unix socket.
    #[cfg(unix)]
    fn connect(&self) -> ControlResult<std::os::unix::net::UnixStream> {
        let stream = std::os::unix::net::UnixStream::connect(&self.path)
            .map_err(|_| ControlError::Unreachable)?;
        stream
            .set_read_timeout(Some(TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(TIMEOUT)))
            .map_err(|_| ControlError::Unreachable)?;
        Ok(stream)
    }

    /// One connection to the host's named pipe (`\\.\pipe\openagents-control-<SID>`),
    /// opened as a file. The pipe's DACL lets only this user open it.
    #[cfg(windows)]
    fn connect(&self) -> ControlResult<std::fs::File> {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)
            .map_err(|_| ControlError::Unreachable)
    }
}

/// Writes one length-prefixed message.
pub fn write_message<W: Write, T: Serialize>(writer: &mut W, message: &T) -> ControlResult<()> {
    let body = serde_json::to_vec(message).map_err(|_| ControlError::Malformed)?;
    if body.len() > MAX_MESSAGE_BYTES {
        return Err(ControlError::Malformed);
    }
    let mut frame = Vec::with_capacity(4 + body.len());
    frame.extend((body.len() as u32).to_be_bytes());
    frame.extend(body);
    writer
        .write_all(&frame)
        .and_then(|()| writer.flush())
        .map_err(|_| ControlError::Unreachable)
}

/// Reads one length-prefixed message.
pub fn read_message<R: Read, T: serde::de::DeserializeOwned>(reader: &mut R) -> ControlResult<T> {
    let mut length = [0u8; 4];
    reader
        .read_exact(&mut length)
        .map_err(|_| ControlError::Unreachable)?;
    let length = u32::from_be_bytes(length) as usize;
    if length > MAX_MESSAGE_BYTES {
        return Err(ControlError::Malformed);
    }
    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .map_err(|_| ControlError::Unreachable)?;
    serde_json::from_slice(&body).map_err(|_| ControlError::Malformed)
}

fn unexpected<T>() -> ControlResult<T> {
    Err(ControlError::Malformed)
}

impl HostControl for SocketControl {
    fn status(&mut self) -> ControlResult<Status> {
        match self.call(Op::Status {})? {
            Reply::Status(status) => Ok(status),
            _ => unexpected(),
        }
    }

    fn invite(&mut self) -> ControlResult<Invite> {
        match self.call(Op::InviteCreate {})? {
            Reply::Invite {
                invitation,
                code,
                expires_at,
                rights,
            } => Ok(Invite {
                invitation,
                code,
                expires_at,
                rights,
            }),
            _ => unexpected(),
        }
    }

    fn cancel(&mut self, invitation: &str) -> ControlResult<u32> {
        match self.call(Op::InviteCancel {
            invitation: invitation.into(),
        })? {
            Reply::Cancelled { count } => Ok(count),
            _ => unexpected(),
        }
    }

    fn cancel_all(&mut self) -> ControlResult<u32> {
        match self.call(Op::InviteCancelAll {})? {
            Reply::Cancelled { count } => Ok(count),
            _ => unexpected(),
        }
    }

    fn devices(&mut self) -> ControlResult<Vec<Device>> {
        match self.call(Op::DeviceList {})? {
            Reply::Devices { devices } => Ok(devices),
            _ => unexpected(),
        }
    }

    fn revoke(&mut self, device: &str) -> ControlResult<()> {
        match self.call(Op::DeviceRevoke {
            device: device.into(),
        })? {
            Reply::Revoked { .. } => Ok(()),
            _ => unexpected(),
        }
    }

    fn autostart(&mut self) -> ControlResult<Autostart> {
        match self.call(Op::AutostartGet {})? {
            Reply::Autostart { policy } => Ok(policy),
            _ => unexpected(),
        }
    }

    fn set_autostart(&mut self, policy: Autostart) -> ControlResult<Autostart> {
        match self.call(Op::AutostartSet { policy })? {
            Reply::Autostart { policy } => Ok(policy),
            _ => unexpected(),
        }
    }

    fn projects(&mut self) -> ControlResult<Vec<Project>> {
        match self.call(Op::ProjectList {})? {
            Reply::Projects { projects } => Ok(projects),
            _ => unexpected(),
        }
    }

    fn add_project(&mut self, path: &str) -> ControlResult<Vec<Project>> {
        match self.call(Op::ProjectAdd { path: path.into() })? {
            Reply::Projects { projects } => Ok(projects),
            _ => unexpected(),
        }
    }

    fn remove_project(&mut self, label: &str) -> ControlResult<Vec<Project>> {
        match self.call(Op::ProjectRemove {
            label: label.into(),
        })? {
            Reply::Projects { projects } => Ok(projects),
            _ => unexpected(),
        }
    }

    fn nearby_pending(&mut self) -> ControlResult<Option<NearbyPrompt>> {
        match self.call(Op::NearbyPending {})? {
            Reply::Nearby { pending } => Ok(pending),
            _ => unexpected(),
        }
    }

    fn nearby_decide(&mut self, id: u64, connect: bool) -> ControlResult<()> {
        match self.call(Op::NearbyDecide { id, connect })? {
            Reply::Nearby { .. } => Ok(()),
            _ => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn the_socket_lives_in_application_support_on_a_mac() {
        assert_eq!(
            socket_path_for("macos", Some(Path::new("/Users/kai")), None),
            Some(PathBuf::from(
                "/Users/kai/Library/Application Support/OpenAgents/control.sock"
            ))
        );
        assert_eq!(
            socket_path_for("linux", None, Some(Path::new("/run/user/501"))),
            Some(PathBuf::from("/run/user/501/openagents/control.sock"))
        );
        assert_eq!(socket_path_for("windows", None, None), None);
    }

    /// The wire shape matches `openagents-connect::control`: tagged by
    /// `kind` in snake case, framed by a big-endian length.
    #[test]
    fn requests_encode_as_the_control_protocol() {
        let request = Request::new(7, Op::InviteCreate {});
        let mut bytes = Vec::new();
        write_message(&mut bytes, &request).expect("encodes");
        let body = &bytes[4..];
        assert_eq!(
            u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize,
            body.len()
        );
        assert_eq!(
            std::str::from_utf8(body).unwrap(),
            r#"{"v":"openagents.control.v1","id":7,"op":{"kind":"invite_create"}}"#
        );
        let back: Request = read_message(&mut &bytes[..]).expect("decodes");
        assert_eq!(back, request);
    }

    #[test]
    fn an_absent_socket_is_unreachable() {
        let dir = tempfile::tempdir().expect("a directory");
        let mut control = SocketControl::new(dir.path().join(SOCKET_NAME));
        assert_eq!(control.status(), Err(ControlError::Unreachable));
    }

    /// A socket that answers with a refusal surfaces it as an error, and a
    /// reply for another request is malformed.
    #[cfg(unix)]
    #[test]
    fn a_refusal_and_a_wrong_id_come_back_as_errors() {
        use std::os::unix::net::UnixListener;
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join(SOCKET_NAME);
        let listener = UnixListener::bind(&path).expect("binds");
        let server = std::thread::spawn(move || {
            for (index, stream) in listener.incoming().take(2).enumerate() {
                let mut stream = stream.expect("a connection");
                let request: Request = read_message(&mut stream).expect("a request");
                let response = Response::new(
                    if index == 0 {
                        request.id
                    } else {
                        request.id + 1
                    },
                    Reply::Refused {
                        code: "forbidden".into(),
                        message: "no".into(),
                    },
                );
                write_message(&mut stream, &response).expect("answers");
            }
        });
        let mut control = SocketControl::new(path);
        assert!(matches!(
            control.invite(),
            Err(ControlError::Refused { code, .. }) if code == "forbidden"
        ));
        assert_eq!(control.devices(), Err(ControlError::Malformed));
        server.join().expect("the server");
    }
}
