//! The local control protocol.
//!
//! The host serves it on a Unix socket (`0600`, in a `0700` directory) and
//! answers only a peer whose user ID equals its own; the desktop app and
//! `openagents connect` are its clients. A caller that passes is the local
//! operator, which NIP-HOST treats as the owner acting on that machine. No
//! device, relay message, or grant reaches this protocol.
//!
//! Each connection carries length-prefixed JSON messages
//! ([`crate::wire`]): a [`Request`] and its [`Response`] with the same `id`,
//! in order. Requests are closed enums: an unknown operation or field is
//! `malformed`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::wire::{read_message, write_message};
use crate::{Code, Error, Result, fail};

/// Message version string.
pub const VERSION: &str = "openagents.control.v1";
/// Largest message, in bytes of JSON.
pub const MAX_MESSAGE_BYTES: usize = 256 * 1024;
/// The socket's file name.
pub const SOCKET_NAME: &str = "control.sock";

/// Where the host's control socket lives: on macOS
/// `~/Library/Application Support/OpenAgents/control.sock`, on Linux
/// `$XDG_RUNTIME_DIR/openagents/control.sock`. `None` when the variable it
/// needs is unset, or on another platform.
#[must_use]
pub fn socket_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    socket_path_for(std::env::consts::OS, home.as_deref(), runtime.as_deref())
}

/// [`socket_path`] for a given platform and environment.
#[must_use]
pub fn socket_path_for(os: &str, home: Option<&Path>, runtime: Option<&Path>) -> Option<PathBuf> {
    let dir = match os {
        "macos" => home?.join("Library/Application Support/OpenAgents"),
        "linux" => runtime?.join("openagents"),
        _ => return None,
    };
    Some(dir.join(SOCKET_NAME))
}

/// One request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub v: String,
    /// Chosen by the client; echoed in the response.
    pub id: u64,
    pub op: Op,
}

impl Request {
    #[must_use]
    pub fn new(id: u64, op: Op) -> Self {
        Self {
            v: VERSION.into(),
            id,
            op,
        }
    }
}

/// The operations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    /// Create a Coder task from explicitly selected retained context and keep
    /// its conversation binding. This grants no new execution authority.
    ImportTask {
        request: String,
        chat: String,
        task: coder_access::protocol::TaskCreate,
    },
    /// Same-user broker for a typed NIP-HOST task operation. The host signs;
    /// the caller supplies a stable 64-character hexadecimal identity.
    Task {
        request: String,
        operation: coder_access::protocol::Operation,
    },
    /// Read only the resident host's configured Coder transcript source.
    TaskHistory {
        query: coder_connect::protocol::Query,
    },
    /// The same task activity a paired phone receives, over the local socket.
    TaskActivity { task: String },
    /// Hosted chat; admitted only as this machine's local operator.
    Chat {
        command: openagents_chat::service::Command,
    },
    /// Move the threads `openagents chat` kept without a host, in the chat
    /// home `home` (`<home>/threads`), into the host's own store, once
    /// (`openagents_chat::migrate`). The host reads that home's device key
    /// itself, and refuses a scratch store or a home another user owns.
    ChatMigrate { home: String },
    /// The host's identity, reachability, and counts.
    Status {},
    /// Create a host invitation and its `openagents-connect:` code. A QR
    /// pairing grants every right an owner's phone uses: `observe`,
    /// `operate`, `terminal`, `review`, `access_read`, and `access_admin`.
    InviteCreate {},
    /// Cancel one unredeemed invitation.
    InviteCancel { invitation: String },
    /// Cancel every unredeemed invitation.
    InviteCancelAll {},
    /// The enrolled devices.
    DeviceList {},
    /// Revoke a device's grant; its open channels close.
    DeviceRevoke { device: String },
    /// Read the auto-start policy.
    AutostartGet {},
    /// Replace the auto-start policy.
    AutostartSet { policy: Autostart },
    /// Coder's engine, model, sign-in, and usage on this computer.
    /// Read-only: the answer has no credential and changes nothing.
    EngineStatus {},
    /// The projects (workspaces) the host admits.
    ProjectList {},
    /// Admit a Git checkout as a project.
    ProjectAdd { path: String },
    /// Stop admitting a project.
    ProjectRemove { label: String },
    /// The nearby request waiting for a click (`DSK-04`), if any.
    NearbyPending {},
    /// Answer the nearby request `id`: **Connect** (with the same rights
    /// as a QR pairing) or **Don't connect**.
    NearbyDecide { id: u64, connect: bool },
    /// Make this computer use the owner key the person uses on their other
    /// computers (`openagents connect owner import`), so one owner
    /// directory lists them all. `secret` is the owner's Nostr secret key,
    /// 64 lowercase hex characters; the host keeps it in its key source and
    /// never echoes it. Refused while any phone holds a current grant.
    OwnerImport { secret: String },
}

/// One response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub v: String,
    pub id: u64,
    pub result: Reply,
}

impl Response {
    #[must_use]
    pub fn new(id: u64, result: Reply) -> Self {
        Self {
            v: VERSION.into(),
            id,
            result,
        }
    }
}

/// What the host answers, by operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    TaskActivity {
        #[serde(with = "activity_json")]
        summary: nostr::activity_summary::ActivitySummary,
    },
    Task {
        outcome: coder_access::protocol::Outcome,
    },
    TaskHistory {
        observation: coder_connect::protocol::Observation,
    },
    /// A bounded hosted chat page and current streaming state.
    Chat {
        snapshot: openagents_chat::service::Snapshot,
    },
    /// What `chat_migrate` did: threads written into the host's store
    /// now, and threads it already held.
    ChatMigrated {
        moved: u32,
        present: u32,
    },
    Status(Status),
    /// `code` is the `openagents-connect:` text. It carries a bearer
    /// capability: show it only in the code window.
    Invite {
        invitation: String,
        code: String,
        expires_at: u64,
        rights: Vec<String>,
    },
    /// How many invitations were cancelled.
    Cancelled {
        count: u32,
    },
    Devices {
        devices: Vec<Device>,
    },
    /// The device's grant epoch after revocation.
    Revoked {
        device: String,
        epoch: u64,
    },
    Autostart {
        policy: Autostart,
    },
    /// The engine report from `engine_status`.
    EngineStatus {
        report: EngineReport,
    },
    Projects {
        projects: Vec<Project>,
    },
    /// The nearby request waiting for a click, after `nearby_pending` or
    /// `nearby_decide`.
    Nearby {
        pending: Option<NearbyPrompt>,
    },
    /// The host's owner public key, after `owner_import`.
    Owner {
        owner: String,
    },
    /// The operation was refused.
    Refused {
        code: String,
        message: String,
    },
}

/// The host at a glance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    /// Host Nostr key, lowercase hex.
    pub host: String,
    /// Host iroh endpoint ID, lowercase hex.
    pub endpoint: String,
    /// The computer's display name.
    pub label: String,
    /// Whether the endpoint reaches its relay or has a direct address.
    pub online: bool,
    pub relay: Option<String>,
    pub devices: u32,
    pub outstanding_invitations: u32,
    /// The host build, for display.
    pub version: String,
}

/// An enrolled device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    /// Device Nostr key, lowercase hex.
    pub device: String,
    pub label: String,
    pub rights: Vec<String>,
    pub grant: String,
    pub epoch: u64,
    pub enrolled_at: u64,
    pub last_seen: Option<u64>,
    pub revoked: bool,
}

/// The auto-start policy as the desktop app edits it. The host maps it onto
/// its full policy and refuses what it cannot admit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Autostart {
    pub enabled: bool,
    /// Project labels whose tasks may start on their own.
    pub projects: Vec<String>,
    /// Most auto-started tasks at once, 1 to 8.
    pub max_running: u8,
}

/// Coder's engine and usage on one computer. Percents and reset times only:
/// no credential, account identifier, or controller path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineReport {
    pub enabled: bool,
    /// The engine adapter name, such as `microcoder-repository`. The window
    /// does not show it.
    pub adapter: String,
    /// The policy's model, truncated.
    pub model: String,
    pub routes: Vec<EngineRoute>,
    /// Codex, then Claude Code, whether or not a route names them.
    pub accounts: Vec<EngineAccount>,
    /// The usage-probe threshold, in percent, when the owner turned probes on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_probe: Option<u8>,
    /// A probed provider is due for a refresh. The window ignores this; the
    /// host uses it to start a background read.
    pub refresh_due: bool,
}

/// One provider login, without any secret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineAccount {
    /// `codex` or `claude`.
    pub provider: String,
    /// The name on screen, such as `Codex` or `Claude Code`.
    pub name: String,
    pub signed_in: bool,
}

/// One admitted route, in the policy's preference order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineRoute {
    pub provider: String,
    pub name: String,
    /// The route's model, truncated.
    pub model: String,
    pub signed_in: bool,
    pub usage: RouteUsage,
}

/// How much of a route's usage limit is used.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RouteUsage {
    /// The owner has not turned usage probes on.
    Off,
    /// This provider has no usage endpoint.
    Unsupported,
    /// No fresh reading. `reason` is a closed code, never provider text.
    Unknown { reason: String },
    /// Fresh windows. `used_percent` is the fullest window.
    Windows {
        windows: Vec<UsageWindow>,
        limit_reached: bool,
        used_percent: u8,
    },
}

/// One usage window, as percents and a reset time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageWindow {
    /// `five_hour`, `seven_day`, `primary`, or `secondary`.
    pub name: String,
    /// The words on screen, such as `5 hours` or `Primary`.
    pub label: String,
    pub used_percent: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<u64>,
    /// `YYYY-MM-DD HH:MM UTC`, when the provider said when the window resets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets: Option<String>,
}

/// A phone nearby that wants to connect (`DSK-04`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NearbyPrompt {
    /// Answers this request in `nearby_decide`.
    pub id: u64,
    /// The phone's label; it chose it, so it is a name, never an identity.
    pub label: String,
    /// The six-digit confirmation code, digits only.
    pub code: String,
}

/// A project the host admits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub label: String,
    /// Where Coder works: the host's worktree of the picked folder, or the
    /// folder itself when it was already such a worktree.
    pub path: String,
    /// The folder the person picked, when `path` is the host's worktree of
    /// it. Screens show this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

impl Project {
    /// The folder to show the person: the one they picked, else `path`.
    #[must_use]
    pub fn shown(&self) -> &str {
        self.folder.as_deref().unwrap_or(&self.path)
    }
}

/// Send a request and read its response, on one connection.
///
/// # Errors
/// `unavailable` for a closed socket; `malformed` for a response with
/// another ID; `unsupported_version` for another version.
pub async fn call<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    request: &Request,
) -> Result<Reply> {
    write_message(stream, request, MAX_MESSAGE_BYTES).await?;
    let response: Response = read_message(stream, MAX_MESSAGE_BYTES)
        .await?
        .ok_or_else(|| Error::new(Code::Unavailable, "host closed the control socket"))?;
    if response.v != VERSION {
        return fail(Code::UnsupportedVersion, "control response version");
    }
    if response.id != request.id {
        return fail(Code::Malformed, "control response for another request");
    }
    Ok(response.result)
}

/// Read the next request on a host's side. `Ok(None)` when the client
/// closed.
///
/// # Errors
/// `malformed` or `bounds` for a bad message; `unsupported_version` for
/// another version.
pub async fn next_request<S: AsyncRead + Unpin>(stream: &mut S) -> Result<Option<Request>> {
    let Some(request) = read_message::<_, Request>(stream, MAX_MESSAGE_BYTES).await? else {
        return Ok(None);
    };
    if request.v != VERSION {
        return fail(Code::UnsupportedVersion, "control request version");
    }
    Ok(Some(request))
}

/// Write a response on a host's side.
///
/// # Errors
/// `unavailable` when the write fails.
pub async fn respond<S: AsyncWrite + Unpin>(stream: &mut S, response: &Response) -> Result<()> {
    write_message(stream, response, MAX_MESSAGE_BYTES).await
}

mod activity_json {
    use nostr::activity_summary::{self, ActivitySummary};
    use serde::{Deserialize, Serialize};
    pub fn serialize<S: serde::Serializer>(
        summary: &ActivitySummary,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        activity_summary::to_value(summary).serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ActivitySummary, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        activity_summary::verify_value(&value).map_err(serde::de::Error::custom)
    }
}
