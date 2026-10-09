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
    Autostart, Device, EngineAccount, EngineReport, EngineRoute, MAX_MESSAGE_BYTES, NearbyPrompt,
    Op, Project, Reply, Request, Response, RouteUsage, SOCKET_NAME, Status, UsageWindow, VERSION,
    socket_path, socket_path_for,
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
            // Shown in the window as said here: plain words (#11120).
            Self::Unreachable => f.write_str("Coder isn't answering on this computer yet."),
            Self::Refused { message, .. } => f.write_str(message),
            Self::Malformed => f.write_str("Coder sent an answer this app can't read."),
        }
    }
}

impl std::error::Error for ControlError {}

pub type ControlResult<T> = Result<T, ControlError>;

/// How to update Coder on this computer: run its install command again,
/// as https://openagents.com/download says.
pub const UPDATE_CODER: &str = if cfg!(windows) {
    "run `irm https://openagents.com/cli/install.ps1 | iex` in PowerShell."
} else {
    "run `curl -fsSL https://openagents.com/cli/install.sh | bash` in a terminal."
};

/// The operations the window uses, over any transport.
pub trait HostControl: Send {
    fn import_task(
        &mut self,
        request: String,
        chat: String,
        task: coder_access::protocol::TaskCreate,
    ) -> ControlResult<openagents_chat::service::Snapshot> {
        let _ = (request, chat, task);
        Err(ControlError::Refused {
            code: "unsupported".into(),
            message: format!(
                "Update Coder on this computer to continue saved sessions: {}",
                UPDATE_CODER
            ),
        })
    }
    fn task_chat(
        &mut self,
        request: openagents_chat_app::task_chat::Request,
    ) -> ControlResult<openagents_chat_app::task_chat::Answer> {
        let _ = request;
        Err(ControlError::Refused {
            code: "unsupported".into(),
            message: format!(
                "Update Coder on this computer to open this chat: {}",
                UPDATE_CODER
            ),
        })
    }

    /// Hosted chat data over the same local authority as pairing operations.
    fn chat(
        &mut self,
        command: openagents_chat::service::Command,
    ) -> ControlResult<openagents_chat::service::Snapshot> {
        let _ = command;
        Err(ControlError::Refused {
            code: "unsupported".into(),
            message: format!(
                "Update Coder on this computer to chat here: {}",
                UPDATE_CODER
            ),
        })
    }

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
    /// Coder's engine, model, sign-in, and usage. The default leaves the
    /// header unchanged: a stand-in host has nothing to show.
    fn engine_status(&mut self) -> ControlResult<EngineReport> {
        Err(ControlError::Unreachable)
    }
    /// [`HostControl::engine_status`] after the host reads `providers`'
    /// usage now (#10105). The default reads nothing new.
    fn engine_refresh(&mut self, providers: &[String]) -> ControlResult<EngineReport> {
        let _ = providers;
        self.engine_status()
    }
}

/// Why [`pick_project`] stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickError {
    /// The host would not take the folder.
    Folder,
    /// The folder is in, but the old project or the switch did not follow.
    Setting,
}

/// How long a call waits for the host while it starts again after a
/// settings change, and how often it asks meanwhile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Patience {
    pub wait: Duration,
    pub every: Duration,
}

/// The window's patience: a host that starts again answers within a
/// second or two, and one its service manager has to start again within
/// ten; thirty seconds covers both with room.
pub const PATIENCE: Patience = Patience {
    wait: Duration::from_secs(30),
    every: Duration::from_millis(250),
};

/// Runs `call`, trying again while the host does not answer (it is
/// starting again after a settings change) for up to `patience.wait`.
fn again<T>(
    control: &mut dyn HostControl,
    patience: Patience,
    mut call: impl FnMut(&mut dyn HostControl) -> ControlResult<T>,
) -> ControlResult<T> {
    let deadline = std::time::Instant::now() + patience.wait;
    loop {
        match call(control) {
            Err(ControlError::Unreachable) if std::time::Instant::now() < deadline => {
                std::thread::sleep(patience.every);
            }
            result => return result,
        }
    }
}

/// Sets the auto-start policy, waiting for a host that is starting again.
/// The host changes only whether the policy is on, its projects, and how
/// many run (`coder host autostart on --keep-engine`, or `off`); the engine
/// the owner set up stays.
pub fn set_autostart(
    control: &mut dyn HostControl,
    policy: Autostart,
    patience: Patience,
) -> ControlResult<Autostart> {
    again(control, patience, |c| c.set_autostart(policy.clone()))
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
///
/// Each step changes the host's settings and may start it again, so each
/// waits for it ([`PATIENCE`]). Every step is idempotent: run again after a
/// failure partway, the same call finishes the swap (the folder is already
/// in, the old project already gone) instead of leaving two projects or a
/// policy naming a label the host no longer has.
pub fn pick_project(
    control: &mut dyn HostControl,
    path: &str,
    replace: Option<&str>,
    autostart: bool,
) -> Result<(), PickError> {
    pick_project_with(control, path, replace, autostart, PATIENCE)
}

/// [`pick_project`] with `patience` for the host.
pub fn pick_project_with(
    control: &mut dyn HostControl,
    path: &str,
    replace: Option<&str>,
    autostart: bool,
    patience: Patience,
) -> Result<(), PickError> {
    let setting = |_| PickError::Setting;
    let before = again(control, patience, |c| c.projects()).map_err(setting)?;
    let mut after = match again(control, patience, |c| c.add_project(path)) {
        Ok(after) => after,
        Err(ControlError::Refused { .. }) => return Err(PickError::Folder),
        Err(_) => return Err(PickError::Setting),
    };
    let label = label_of(&before, &after, path).ok_or(PickError::Setting)?;
    if let Some(old) = replace.filter(|old| *old != label)
        && after.iter().any(|project| project.label == old)
    {
        after = match again(control, patience, |c| c.remove_project(old)) {
            Ok(after) => after,
            // A host that took it off and started again before it answered
            // says it has no such project now: it is gone either way.
            Err(ControlError::Refused { .. }) => {
                let now = again(control, patience, |c| c.projects()).map_err(setting)?;
                if now.iter().any(|project| project.label == old) {
                    return Err(PickError::Setting);
                }
                now
            }
            Err(_) => return Err(PickError::Setting),
        };
    }
    let policy = again(control, patience, |c| c.autostart()).map_err(setting)?;
    let mut projects: Vec<String> = policy
        .projects
        .iter()
        .filter(|held| **held != label && after.iter().any(|project| project.label == **held))
        .cloned()
        .collect();
    projects.push(label);
    let wanted = Autostart {
        enabled: autostart,
        projects,
        max_running: policy.max_running.max(1),
    };
    if policy == wanted {
        return Ok(());
    }
    set_autostart(control, wanted, patience)
        .map(|_| ())
        .map_err(setting)
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

    /// Run one task operation through the same portable client as a phone.
    /// The caller keeps `request` stable when retrying an uncertain response.
    pub fn task_operation(
        &mut self,
        request: &str,
        operation: coder_access::protocol::Operation,
    ) -> coder_access::Result<coder_access::protocol::Outcome> {
        use coder_access::{Code, Error};
        operation.validate()?;
        match self.call(Op::Task {
            request: request.into(),
            operation: operation.clone(),
        }) {
            Ok(Reply::Task { outcome }) if outcome.answers(&operation) => {
                outcome.validate()?;
                Ok(outcome)
            }
            Ok(_) => Err(Error::new(
                Code::Malformed,
                "Coder sent an unexpected answer. Try again.",
            )),
            Err(ControlError::Refused { code, message }) => Err(Error::new(
                serde_json::from_value(serde_json::Value::String(code))
                    .unwrap_or(Code::Unavailable),
                message,
            )),
            Err(_) => Err(Error::new(Code::Unavailable, "Coder could not be reached")),
        }
    }

    pub fn create_task(
        &mut self,
        request: &str,
        task: coder_access::protocol::TaskCreate,
    ) -> coder_access::Result<String> {
        coder_access::client::tasks::Tasks::new(|operation| self.task_operation(request, operation))
            .create(task)
    }

    pub fn cancel_task(
        &mut self,
        request: &str,
        task: &str,
        revision: u64,
        reason: &str,
    ) -> coder_access::Result<()> {
        coder_access::client::tasks::Tasks::new(|operation| self.task_operation(request, operation))
            .cancel(task, revision, reason)
    }

    /// Read a bounded source-bound catalog or transcript page. The broker
    /// uses the same observer client as the phones and exposes no key.
    pub fn task_history(
        &mut self,
        query: coder_connect::protocol::Query,
    ) -> ControlResult<coder_connect::protocol::Observation> {
        match self.call(Op::TaskHistory { query })? {
            Reply::TaskHistory { observation } => Ok(observation),
            _ => Err(ControlError::Malformed),
        }
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
    fn import_task(
        &mut self,
        request: String,
        chat: String,
        task: coder_access::protocol::TaskCreate,
    ) -> ControlResult<openagents_chat::service::Snapshot> {
        match self.call(Op::ImportTask {
            request,
            chat,
            task,
        })? {
            Reply::Chat { snapshot } => Ok(snapshot),
            _ => Err(ControlError::Malformed),
        }
    }

    fn task_chat(
        &mut self,
        request: openagents_chat_app::task_chat::Request,
    ) -> ControlResult<openagents_chat_app::task_chat::Answer> {
        use openagents_chat_app::task_chat::{Answer, Request};
        match request {
            Request::Activity { task } => match self.call(Op::TaskActivity { task })? {
                Reply::TaskActivity { summary } => Ok(Answer::Activity(summary)),
                _ => unexpected(),
            },
            Request::History { query } => self.task_history(query).map(Answer::History),
            Request::Operation { request, operation } => {
                // An older host does not know `task.review`, or reviews no
                // change for this task: the card then shows the
                // transcript's diff, with no revisions.
                let review = matches!(
                    operation,
                    coder_access::protocol::Operation::ReviewTask { .. }
                );
                match self.task_operation(&request, operation) {
                    Ok(outcome) => Ok(Answer::Operation(outcome)),
                    Err(error)
                        if review
                            && matches!(
                                error.code,
                                coder_access::Code::Unsupported | coder_access::Code::Malformed
                            ) =>
                    {
                        Ok(Answer::Unsupported)
                    }
                    Err(error) => Err(ControlError::Refused {
                        code: format!("{:?}", error.code),
                        message: error.message,
                    }),
                }
            }
        }
    }

    fn chat(
        &mut self,
        command: openagents_chat::service::Command,
    ) -> ControlResult<openagents_chat::service::Snapshot> {
        match self.call(Op::Chat {
            command,
            caller: None,
        })? {
            Reply::Chat { snapshot } => Ok(snapshot),
            _ => unexpected(),
        }
    }

    fn status(&mut self) -> ControlResult<Status> {
        match self.call(Op::Status {})? {
            Reply::Status(status) => Ok(status),
            _ => unexpected(),
        }
    }

    fn engine_status(&mut self) -> ControlResult<EngineReport> {
        match self.call(Op::EngineStatus {})? {
            Reply::EngineStatus { report } => Ok(report),
            _ => unexpected(),
        }
    }

    fn engine_refresh(&mut self, providers: &[String]) -> ControlResult<EngineReport> {
        let op = Op::EngineRefresh {
            providers: providers.to_vec(),
        };
        match self.call(op)? {
            Reply::EngineStatus { report } => Ok(report),
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

    /// A host that applies a removal and starts again before its answer
    /// arrives: the window hears nothing, asks again, and is told there is
    /// no such project. The swap still finishes, and running it again
    /// changes nothing.
    #[test]
    fn a_swap_whose_answer_was_lost_finishes_and_runs_again_harmlessly() {
        use crate::fake::FakeHost;
        struct LostReply {
            host: FakeHost,
            lose: bool,
        }
        impl HostControl for LostReply {
            fn status(&mut self) -> ControlResult<Status> {
                self.host.status()
            }
            fn invite(&mut self) -> ControlResult<Invite> {
                self.host.invite()
            }
            fn cancel(&mut self, invitation: &str) -> ControlResult<u32> {
                self.host.cancel(invitation)
            }
            fn cancel_all(&mut self) -> ControlResult<u32> {
                self.host.cancel_all()
            }
            fn devices(&mut self) -> ControlResult<Vec<Device>> {
                self.host.devices()
            }
            fn revoke(&mut self, device: &str) -> ControlResult<()> {
                self.host.revoke(device)
            }
            fn autostart(&mut self) -> ControlResult<Autostart> {
                self.host.autostart()
            }
            fn set_autostart(&mut self, policy: Autostart) -> ControlResult<Autostart> {
                self.host.set_autostart(policy)
            }
            fn projects(&mut self) -> ControlResult<Vec<Project>> {
                self.host.projects()
            }
            fn add_project(&mut self, path: &str) -> ControlResult<Vec<Project>> {
                self.host.add_project(path)
            }
            fn remove_project(&mut self, label: &str) -> ControlResult<Vec<Project>> {
                let result = self.host.remove_project(label);
                if std::mem::take(&mut self.lose) {
                    return Err(ControlError::Unreachable);
                }
                result
            }
            fn nearby_pending(&mut self) -> ControlResult<Option<NearbyPrompt>> {
                self.host.nearby_pending()
            }
            fn nearby_decide(&mut self, id: u64, connect: bool) -> ControlResult<()> {
                self.host.nearby_decide(id, connect)
            }
        }
        let patience = Patience {
            wait: Duration::from_millis(200),
            every: Duration::from_millis(1),
        };
        let host = FakeHost::default();
        let mut control = LostReply {
            host: host.clone(),
            lose: false,
        };
        pick_project_with(&mut control, "/code/openagents", None, true, patience).unwrap();
        control.lose = true;
        pick_project_with(
            &mut control,
            "/code/omarchy",
            Some("openagents"),
            true,
            patience,
        )
        .unwrap();
        let check = |control: &mut LostReply| {
            let projects = control.host.projects().unwrap();
            assert_eq!(projects.len(), 1, "{projects:?}");
            assert_eq!(projects[0].label, "omarchy");
            let policy = control.host.autostart().unwrap();
            assert!(policy.enabled);
            assert_eq!(policy.projects, ["omarchy"]);
        };
        check(&mut control);
        // Run again, as the window does after a failure: nothing changes.
        pick_project_with(
            &mut control,
            "/code/omarchy",
            Some("openagents"),
            true,
            patience,
        )
        .unwrap();
        check(&mut control);
        // A host that never answers is a setting that did not change, not
        // a folder it refused.
        host.set_down(true);
        assert_eq!(
            pick_project_with(&mut control, "/code/site", None, true, patience),
            Err(PickError::Setting)
        );
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
