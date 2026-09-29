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
    Autostart, Device, MAX_MESSAGE_BYTES, Op, Project, Reply, Request, Response, SOCKET_NAME,
    Status, VERSION, socket_path, socket_path_for,
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
    fn invite(&mut self, terminal: bool) -> ControlResult<Invite>;
    fn cancel(&mut self, invitation: &str) -> ControlResult<u32>;
    fn cancel_all(&mut self) -> ControlResult<u32>;
    fn devices(&mut self) -> ControlResult<Vec<Device>>;
    fn revoke(&mut self, device: &str) -> ControlResult<()>;
    fn autostart(&mut self) -> ControlResult<Autostart>;
    fn set_autostart(&mut self, policy: Autostart) -> ControlResult<Autostart>;
    fn projects(&mut self) -> ControlResult<Vec<Project>>;
    fn add_project(&mut self, path: &str) -> ControlResult<Vec<Project>>;
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
        #[cfg(unix)]
        {
            let id = self.next;
            self.next += 1;
            let request = Request::new(id, op);
            let mut stream = std::os::unix::net::UnixStream::connect(&self.path)
                .map_err(|_| ControlError::Unreachable)?;
            stream
                .set_read_timeout(Some(TIMEOUT))
                .and_then(|()| stream.set_write_timeout(Some(TIMEOUT)))
                .map_err(|_| ControlError::Unreachable)?;
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
        #[cfg(not(unix))]
        {
            let _ = op;
            Err(ControlError::Unreachable)
        }
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

    fn invite(&mut self, terminal: bool) -> ControlResult<Invite> {
        match self.call(Op::InviteCreate { terminal })? {
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
        let request = Request::new(7, Op::InviteCreate { terminal: true });
        let mut bytes = Vec::new();
        write_message(&mut bytes, &request).expect("encodes");
        let body = &bytes[4..];
        assert_eq!(
            u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize,
            body.len()
        );
        assert_eq!(
            std::str::from_utf8(body).unwrap(),
            r#"{"v":"openagents.control.v1","id":7,"op":{"kind":"invite_create","terminal":true}}"#
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
            control.invite(false),
            Err(ControlError::Refused { code, .. }) if code == "forbidden"
        ));
        assert_eq!(control.devices(), Err(ControlError::Malformed));
        server.join().expect("the server");
    }
}
