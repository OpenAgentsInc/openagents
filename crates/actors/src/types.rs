//! Shared, versioned values for the actor runtime and its adapters.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type Timestamp = i64;
pub type Result<T> = std::result::Result<T, ActorError>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActorError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}
impl ActorError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
        }
    }
    pub fn retry(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: true,
        }
    }
}
impl std::fmt::Display for ActorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for ActorError {}
impl From<serde_json::Error> for ActorError {
    fn from(_: serde_json::Error) -> Self {
        Self::new("bad_args", "The data has an invalid format.")
    }
}
#[cfg(feature = "server")]
impl From<tokio_postgres::Error> for ActorError {
    fn from(error: tokio_postgres::Error) -> Self {
        let code = error.code().map(|c| c.code()).unwrap_or("");
        match code {
            "23505" => Self::new("conflict", "That record already exists."),
            "23503" => Self::new("not_found", "The related record was not found."),
            _ => Self::retry("storage", "Storage is temporarily unavailable."),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
pub struct ActorId {
    pub workspace_id: String,
    pub actor_type: String,
    pub key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Live,
    Blocked,
    Destroyed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub uid: String,
    pub id: ActorId,
    pub owner: Option<String>,
    pub status: Status,
    pub state_version: u32,
    pub version: u64,
    pub event_seq: u64,
    pub inbox_seq: u64,
    pub state: Value,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Member,
    Owner,
    Service,
    Admin,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutorGrant {
    pub id: String,
    pub queues: Vec<String>,
    pub targets: Vec<String>,
    pub generation: u64,
    pub expires_at: Timestamp,
    pub max_claims: u32,
}
/// Trusted authorization result supplied by the hosting service, never decoded from a request body.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Caller {
    pub principal: String,
    pub workspace_id: String,
    pub account_id: Option<String>,
    pub role: Role,
    pub executor: Option<ExecutorGrant>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    Member,
    Owner,
    AccountOwner,
    Service,
    Admin,
    Internal,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Action,
    Inbox,
    Internal,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub name: String,
    pub args: Value,
    pub origin: Origin,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RetryPolicy {
    Idempotent,
    Reconcile,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub name: String,
    pub payload: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventRecord {
    pub uid: String,
    pub seq: u64,
    pub version: u64,
    pub event: Event,
    pub at: Timestamp,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkSpec {
    pub item_id: String,
    pub queue: String,
    pub target: Option<String>,
    pub payload: Value,
    pub lease_ms: i64,
    pub max_attempts: u32,
    pub retry: RetryPolicy,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EffectSpec {
    pub id: String,
    pub kind: String,
    pub payload: Value,
    pub timeout_ms: i64,
    pub max_attempts: u32,
    pub retry: RetryPolicy,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlarmSpec {
    pub name: String,
    pub due_at: Timestamp,
    pub message: Envelope,
    pub interval_ms: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Command {
    Emit {
        event: Event,
    },
    Send {
        to: ActorId,
        message: Envelope,
        idempotency_key: String,
    },
    Schedule {
        alarm: AlarmSpec,
    },
    CancelAlarm {
        name: String,
    },
    Effect {
        effect: EffectSpec,
    },
    Work {
        work: WorkSpec,
    },
    CancelWork {
        item_id: String,
    },
    Destroy,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prepared {
    pub state: Value,
    pub state_version: u32,
    pub reply: Value,
    pub commands: Vec<Command>,
    pub read_only: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionRequest {
    pub id: ActorId,
    pub message: Envelope,
    pub input: Option<Value>,
    pub idempotency_key: Option<String>,
    pub expected_version: Option<u64>,
    /// A work claim the call is fenced by: the store checks it against the
    /// claim in the same transaction and hands it to the handler
    /// ([`crate::Ctx::fence`]). Only the executor holding that claim, at
    /// that epoch, before its lease ends, can make the call.
    #[serde(default)]
    pub fence: Option<WorkFence>,
}
/// The claim a fenced action names.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkFence {
    pub item_id: String,
    pub epoch: u64,
}
/// What a handler learns of a verified fence.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fenced {
    pub item_id: String,
    pub epoch: u64,
    /// The work was cancelled; the claim still stands until it is released.
    pub cancel: bool,
    /// The lease, renewed by this call unless cancelled.
    pub heartbeat_until: Timestamp,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionReply {
    pub reply: Value,
    pub version: u64,
    pub event_seq: u64,
    pub replayed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InboxReceipt {
    pub seq: u64,
    pub state: String,
    pub reply: Option<Value>,
    pub error: Option<ActorError>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClaimedWork {
    pub actor: ActorId,
    pub uid: String,
    pub item_id: String,
    pub queue: String,
    pub payload: Value,
    pub epoch: u64,
    pub heartbeat_until: Timestamp,
    pub cancel: bool,
    pub attempts: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Progress {
    pub seq: u64,
    pub value: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeartbeatReply {
    pub heartbeat_until: Timestamp,
    pub cancel: bool,
    pub progress_seq: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClaimedEffect {
    pub actor: ActorId,
    pub uid: String,
    pub id: String,
    pub kind: String,
    pub payload: Value,
    pub token: String,
    pub claimed_until: Timestamp,
    pub timeout_ms: i64,
    pub attempts: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventPage {
    pub events: Vec<EventRecord>,
    pub reset: bool,
    pub latest_seq: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViewReply {
    pub view: Value,
    pub version: u64,
    pub event_seq: u64,
}
