//! Issue work and a computer's granted cloud pool over a caller-supplied
//! connection. The host owns execution and budgets; this client never reads
//! local credentials or substitutes the website's separate work-host queue.

use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::CancellationToken;

/// Start an issue run with a caller-chosen request ID.
pub const START: &str = "work.start";
/// List issue runs on the connected host.
pub const LIST: &str = "work.list";
/// Read a bounded page of run progress.
pub const EVENTS: &str = "work.events";
/// Request that the host stop a run.
pub const CANCEL: &str = "work.cancel";
/// Read the granted pool and its compute budget.
pub const POOL_STATUS: &str = "pool.status";
/// Grow an existing pool within the saved agent budget.
pub const POOL_UP: &str = "pool.up";
/// Stop the connected host pool.
pub const POOL_DOWN: &str = "pool.down";

/// Where a verified change goes. `Direct` (the default, #12165) rebases
/// onto main, replays the touched checks, and pushes to main; a conflict, a
/// failing check, or rewritten files send it to the landing queue instead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LandMode {
    /// Rebase onto main, replay the touched checks, and push to main.
    #[default]
    Direct,
    Queue,
    Pr,
    None,
}

/// The explicitly requested computer. A host must refuse an unavailable
/// placement rather than silently sending work somewhere else.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    Gce,
    Boat,
    Computer(String),
}

/// One issue to run on an explicitly selected host under a saved agent budget.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkRequest {
    pub repo: String,
    pub issue: u64,
    #[serde(default)]
    pub land: LandMode,
    #[serde(default)]
    pub engine: Option<String>,
    #[serde(default)]
    pub on: Option<Placement>,
    /// The saved agent definition ID, never its display name.
    pub agent: String,
}

impl WorkRequest {
    /// Check identifiers and options before contacting the host.
    pub fn validate(&self) -> Result<(), Error> {
        let parts: Vec<_> = self.repo.split('/').collect();
        if parts.len() != 2
            || parts.iter().any(|part| !identifier(part))
            || self.issue == 0
            || !identifier(&self.agent)
            || self.engine.as_deref().is_some_and(|engine| {
                engine.is_empty()
                    || engine.len() > 128
                    || !engine
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"_-:.".contains(&byte))
            })
            || matches!(&self.on, Some(Placement::Computer(id)) if !identifier(id))
        {
            return Err(Error::invalid(
                "Use a repository owner/name, issue number, agent ID, and supported engine.",
            ));
        }
        Ok(())
    }
}

/// Whether an identifier is a bounded path-free name accepted by this client.
pub fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-.".contains(&byte))
}

/// The host's saved run identity, state, and latest outcome.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkRun {
    pub id: String,
    pub request: WorkRequest,
    pub state: String,
    pub created_unix: u64,
    pub updated_unix: u64,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub usage: Option<Value>,
}

/// A bounded page of progress. Save `next` and pass it as `after` on the next
/// call. Canceling a read never cancels execution on the host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkEvents {
    pub run: WorkRun,
    pub events: Vec<Value>,
    pub next: u64,
    pub complete: bool,
}

/// The connected pool's hosts, slot capacity, and current compute spend.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PoolStatus {
    pub pool: Option<String>,
    pub granted: bool,
    pub hosts: Vec<Value>,
    pub activity: Vec<Value>,
    pub slots: u64,
    pub spent_usd: f64,
    pub daily_usd: f64,
    pub agent_spent_usd: Option<f64>,
    pub agent_daily_usd: Option<f64>,
}

/// A refusal preserves the owner's exact command across the connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Error {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_command: Option<String>,
}
impl Error {
    /// Build a refusal for invalid request parameters.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "invalid".into(),
            message: message.into(),
            owner_command: None,
        }
    }
    /// Build a host or connection refusal.
    pub fn refused(message: impl Into<String>) -> Self {
        Self {
            code: "refused".into(),
            message: message.into(),
            owner_command: None,
        }
    }
    /// Build the typed result for a canceled client call.
    pub fn cancelled() -> Self {
        Self {
            code: "cancelled".into(),
            message: "Canceled.".into(),
            owner_command: None,
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Error {}

/// One asynchronous response from a caller-supplied host connection.
pub type Reply<'a> = Pin<Box<dyn Future<Output = Result<Value, Error>> + Send + 'a>>;

/// The public Work/Pool connection boundary. A remote connection supplies
/// authentication and timeouts; the receiver checks grants on every mutation.
pub trait Transport: Send + Sync {
    /// Send one named operation, preserving typed refusals and cancellation.
    fn request<'a>(
        &'a self,
        method: &'static str,
        params: Value,
        cancel: &'a CancellationToken,
    ) -> Reply<'a>;
}

async fn call<T: DeserializeOwned>(
    transport: &dyn Transport,
    method: &'static str,
    params: Value,
    cancel: &CancellationToken,
) -> Result<T, Error> {
    let value = tokio::select! { biased;
        _ = cancel.cancelled() => return Err(Error::cancelled()),
        result = transport.request(method, params, cancel) => result?,
    };
    serde_json::from_value(value).map_err(|_| Error {
        code: "invalid_response".into(),
        message: "The work response could not be read.".into(),
        owner_command: None,
    })
}

/// A client for issue work on one connected host, usable by any harness.
pub struct Work<'a> {
    transport: &'a dyn Transport,
}
impl<'a> Work<'a> {
    /// Borrow an authenticated host connection without adopting local credentials.
    pub fn new(transport: &'a dyn Transport) -> Self {
        Self { transport }
    }
    /// Reuse the same request ID and body after response loss. The host must
    /// return the original run, or refuse a changed body under the same ID.
    pub async fn start(
        &self,
        request_id: &str,
        request: WorkRequest,
        cancel: &CancellationToken,
    ) -> Result<WorkRun, Error> {
        if !identifier(request_id) {
            return Err(Error::invalid(
                "Use a request ID of up to 128 letters, numbers, dots, underscores, or hyphens.",
            ));
        }
        request.validate()?;
        call(
            self.transport,
            START,
            json!({"request_id": request_id, "request": request}),
            cancel,
        )
        .await
    }
    /// List runs saved by the connected host.
    pub async fn list(&self, cancel: &CancellationToken) -> Result<Vec<WorkRun>, Error> {
        call(self.transport, LIST, json!({}), cancel).await
    }
    /// Read progress after a saved cursor without changing execution.
    pub async fn events(
        &self,
        id: &str,
        after: u64,
        cancel: &CancellationToken,
    ) -> Result<WorkEvents, Error> {
        if !identifier(id) {
            return Err(Error::invalid("Choose a work run."));
        }
        call(
            self.transport,
            EVENTS,
            json!({"id": id, "after": after}),
            cancel,
        )
        .await
    }
    /// Ask the host to stop the run; subsequent reads report its outcome.
    pub async fn cancel(&self, id: &str, cancel: &CancellationToken) -> Result<(), Error> {
        if !identifier(id) {
            return Err(Error::invalid("Choose a work run."));
        }
        call(self.transport, CANCEL, json!({"id": id}), cancel).await
    }
}

/// The connected host's granted pool, with spend attributed to one saved agent.
pub struct Pool<'a> {
    transport: &'a dyn Transport,
    agent: &'a str,
}
impl<'a> Pool<'a> {
    /// Borrow the connection with the saved agent ID used for budget checks.
    pub fn new(transport: &'a dyn Transport, agent: &'a str) -> Result<Self, Error> {
        if !identifier(agent) {
            return Err(Error::invalid("Choose a saved agent ID."));
        }
        Ok(Self { transport, agent })
    }
    /// Read hosts, activity, and spend for this saved agent.
    pub async fn status(&self, cancel: &CancellationToken) -> Result<PoolStatus, Error> {
        call(
            self.transport,
            POOL_STATUS,
            json!({"agent": self.agent}),
            cancel,
        )
        .await
    }
    /// Reach the requested host count within an existing owner grant.
    pub async fn up(&self, hosts: u32, cancel: &CancellationToken) -> Result<PoolStatus, Error> {
        if !(1..=32).contains(&hosts) {
            return Err(Error::invalid("Choose 1 to 32 pool hosts."));
        }
        call(
            self.transport,
            POOL_UP,
            json!({"agent": self.agent,"hosts": hosts}),
            cancel,
        )
        .await
    }
    /// Stop the connected pool without deleting its saved run records.
    pub async fn down(&self, cancel: &CancellationToken) -> Result<(), Error> {
        call(
            self.transport,
            POOL_DOWN,
            json!({"agent": self.agent}),
            cancel,
        )
        .await
    }
}
