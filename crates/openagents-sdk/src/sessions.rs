//! Computer sessions in the account's chat list (#11897).
//!
//! A desktop or phone host publishes one summary row per session it runs:
//! title, computer, project, status, last activity, and a short recap. The
//! web and every app list them with the account's chats, marked
//! `source: "session"` with their computer home. A row holds no transcript.
//! The transcript is opt-in per chat or per project, off by default; while
//! it is on, the host uploads a read-only copy.
use crate::{CancellationToken, Client, Error, Session};
pub use chat_home::{Home, MoveRequest, PartRef};
use reqwest::Method;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};
use std::future::Future;

/// The most sessions one publish may carry.
pub const MAX_PER_COMPUTER: usize = 200;
/// Bound the combined authorized shared-project session list.
pub const MAX_LISTED: usize = 4096;

/// What a session is doing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionStatus {
    Idle,
    Working,
    AwaitingInput,
    Errored,
    Other(String),
}
impl SessionStatus {
    /// The status as the service writes it.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Idle => "idle",
            Self::Working => "working",
            Self::AwaitingInput => "awaiting_input",
            Self::Errored => "errored",
            Self::Other(s) => s,
        }
    }
}
impl Serialize for SessionStatus {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}
impl<'de> Deserialize<'de> for SessionStatus {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(d)?.as_str() {
            "idle" => Self::Idle,
            "working" => Self::Working,
            "awaiting_input" => Self::AwaitingInput,
            "errored" => Self::Errored,
            s => Self::Other(s.to_owned()),
        })
    }
}

/// One session row as the account lists it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct SessionSummary {
    /// Opaque authorized owner/device/session identity, including shared members.
    #[serde(default)]
    pub record_key: Option<String>,
    /// Exact published-row version for viewer-scoped read acknowledgment.
    #[serde(default)]
    pub activity_revision: Option<String>,
    /// Unknown on older services; never inferred from a generic opened time.
    #[serde(default)]
    pub unseen: Option<bool>,
    pub id: String,
    pub home: Home,
    pub title: String,
    /// The computer's name.
    pub computer: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub folder: Option<String>,
    pub status: SessionStatus,
    pub last_activity_unix: u64,
    #[serde(default)]
    pub recap: String,
    /// Whether the computer published lately.
    #[serde(default)]
    pub online: bool,
    /// Whether the transcript is on, so a copy may be on the web.
    #[serde(default)]
    pub transcript: bool,
    /// The `openagents://` link that opens it in the app.
    #[serde(default)]
    pub app_link: Option<String>,
    /// The earlier part, whether it ran in the cloud or on a computer.
    #[serde(default)]
    pub continues: Option<PartRef>,
    /// The web chat that continues it.
    #[serde(default)]
    pub continued_by: Option<PartRef>,
    #[serde(default)]
    pub home_transfer: bool,
    /// The saved agent the chat runs (#11966).
    #[serde(default)]
    pub agent: Option<String>,
}
impl SessionSummary {
    /// Stable list identity. New services include the authorized record owner; old ones stay home-qualified.
    pub fn navigation_key(&self) -> String {
        self.record_key.clone().unwrap_or_else(|| match &self.home {
            Home::Computer(device) => format!("legacy:{device}:{}", self.id),
            Home::Cloud => format!("invalid:{}", self.id),
        })
    }
    pub(crate) fn valid(&self) -> bool {
        self.record_key
            .as_ref()
            .is_none_or(|r| r.len() == 64 && r.bytes().all(|b| b.is_ascii_hexdigit()))
            && self
                .activity_revision
                .as_ref()
                .is_none_or(|r| r.len() == 64 && r.bytes().all(|b| b.is_ascii_hexdigit()))
            && chat_home::valid_id(&self.id)
            && matches!(&self.home, Home::Computer(_))
            && self
                .continues
                .as_ref()
                .is_none_or(|p| p.valid() && p.id != self.id)
            && self.continued_by.as_ref().is_none_or(PartRef::valid)
    }
}

/// One session a host publishes. Text is bounded and screened by the
/// service; never put transcript content in `recap`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PublishedSession {
    pub id: String,
    #[serde(skip_serializing_if = "is_false")]
    pub home_transfer: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continues: Option<PartRef>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    pub status: SessionStatus,
    pub last_activity_unix: u64,
    pub recap: String,
    /// The app's 16-character workspace locator, so the web can link back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// The saved agent the chat runs (#11966); lists show it by name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Requests from the web wait for this computer's normal handoff path.
pub async fn pending_moves(
    client: &Client,
    session: &Session,
    device: &str,
    cancel: &CancellationToken,
) -> Result<Vec<MoveRequest>, Error> {
    client.check_session(session)?;
    if !device_ok(device) {
        return Err(Error::InvalidReference);
    }
    let value: Value = cancellable(
        cancel,
        client.request(
            Method::GET,
            &["history", "sessions", "computers", device, "moves"],
            &[],
            None,
            Some(session.token()),
        ),
    )
    .await?;
    let moves: Vec<MoveRequest> =
        serde_json::from_value(value["moves"].clone()).map_err(|_| Error::InvalidResponse)?;
    if moves.len() > MAX_PER_COMPUTER {
        return Err(Error::InvalidResponse);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::InvalidResponse)?
        .as_secs();
    if moves.iter().any(|r| r.expires_unix > now && !r.valid(now)) {
        return Err(Error::InvalidResponse);
    }
    Ok(moves.into_iter().filter(|r| r.valid(now)).collect())
}

/// Who wrote a message in a transcript copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopiedRole {
    User,
    Assistant,
    Tool,
}

/// One message of a session's read-only transcript copy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopiedMessage {
    pub role: CopiedRole,
    pub text: String,
}

/// One session, and its transcript copy when it is shared with the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionDetail {
    pub session: SessionSummary,
    pub messages: Option<Vec<CopiedMessage>>,
}

async fn cancellable<T>(
    cancel: &CancellationToken,
    f: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    tokio::select! {biased; _=cancel.cancelled()=>Err(Error::Cancelled), result=f=>result.map_err(|e| match e {
        Error::Refused { status: 404, .. } => Error::NotFound,
        e => e,
    })}
}

fn device_ok(device: &str) -> bool {
    Home::computer(device).is_some()
}

fn project_ok(id: &str) -> bool {
    id.starts_with("prj_")
        && id.len() > 4
        && id.len() <= 128
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

pub(crate) fn rows(value: &Value) -> Result<Vec<SessionSummary>, Error> {
    let rows: Vec<SessionSummary> =
        serde_json::from_value(value.clone()).map_err(|_| Error::InvalidResponse)?;
    if rows.len() > MAX_LISTED || rows.iter().any(|r| !r.valid()) {
        return Err(Error::InvalidResponse);
    }
    Ok(rows)
}

/// The account's session rows, newest first, or a shared project's rows.
pub async fn list(
    client: &Client,
    session: &Session,
    project: Option<&str>,
    cancel: &CancellationToken,
) -> Result<Vec<SessionSummary>, Error> {
    client.check_session(session)?;
    let mut query = Vec::new();
    if let Some(project) = project {
        if !project_ok(project) {
            return Err(Error::InvalidReference);
        }
        query.push(("project", project.to_owned()));
    }
    let value: Value = cancellable(
        cancel,
        client.request(
            Method::GET,
            &["history", "sessions"],
            &query,
            None,
            Some(session.token()),
        ),
    )
    .await?;
    rows(&value["data"])
}

/// One session, with its transcript copy when it is shared with the caller.
pub async fn get(
    client: &Client,
    session: &Session,
    id: &str,
    cancel: &CancellationToken,
) -> Result<SessionDetail, Error> {
    get_in_project(client, session, id, None, cancel).await
}

/// Read a session in the same authorized shared-project scope as its list.
pub async fn get_in_project(
    client: &Client,
    session: &Session,
    id: &str,
    project: Option<&str>,
    cancel: &CancellationToken,
) -> Result<SessionDetail, Error> {
    get_scoped(client, session, id, project, None, cancel).await
}
/// Read exactly the authorized indexed identity from the list, including shared-owner collisions.
pub async fn get_row(
    client: &Client,
    session: &Session,
    row: &SessionSummary,
    project: Option<&str>,
    cancel: &CancellationToken,
) -> Result<SessionDetail, Error> {
    if !row.valid() {
        return Err(Error::InvalidReference);
    }
    let detail = get_scoped(
        client,
        session,
        &row.id,
        project,
        row.record_key.as_deref(),
        cancel,
    )
    .await?;
    if detail.session.home != row.home || detail.session.record_key != row.record_key {
        return Err(Error::InvalidResponse);
    }
    Ok(detail)
}
async fn get_scoped(
    client: &Client,
    session: &Session,
    id: &str,
    project: Option<&str>,
    record: Option<&str>,
    cancel: &CancellationToken,
) -> Result<SessionDetail, Error> {
    let mut query = Vec::new();
    if let Some(project) = project {
        if !project_ok(project) {
            return Err(Error::InvalidReference);
        }
        query.push(("project", project.to_owned()));
    }
    if let Some(record) = record {
        query.push(("record", record.to_owned()));
    }
    client.check_session(session)?;
    if !chat_home::valid_id(id) {
        return Err(Error::InvalidReference);
    }
    let value: Value = cancellable(
        cancel,
        client.request(
            Method::GET,
            &["history", "sessions", id],
            &query,
            None,
            Some(session.token()),
        ),
    )
    .await?;
    let found: SessionSummary =
        serde_json::from_value(value["session"].clone()).map_err(|_| Error::InvalidResponse)?;
    if !found.valid()
        || found.id != id
        || project.is_some_and(|p| found.project.as_deref() != Some(p))
    {
        return Err(Error::InvalidResponse);
    }
    let messages: Option<Vec<CopiedMessage>> =
        serde_json::from_value(value["messages"].clone()).map_err(|_| Error::InvalidResponse)?;
    Ok(SessionDetail {
        session: found,
        messages,
    })
}

/// Publish this computer's current sessions, replacing what it published
/// before. Returns the ids whose transcript is on, which the host may
/// upload with [`upload_transcript`]. Publishing also says the computer is
/// online, so a host republishes about every 30 seconds.
pub async fn publish(
    client: &Client,
    session: &Session,
    device: &str,
    name: &str,
    sessions: &[PublishedSession],
    cancel: &CancellationToken,
) -> Result<Vec<String>, Error> {
    client.check_session(session)?;
    if !device_ok(device)
        || sessions.len() > MAX_PER_COMPUTER
        || sessions.iter().any(|s| {
            !chat_home::valid_id(&s.id)
                || s.continues
                    .as_ref()
                    .is_some_and(|p| !p.valid() || p.id == s.id)
        })
    {
        return Err(Error::InvalidReference);
    }
    let value: Value = cancellable(
        cancel,
        client.request(
            Method::PUT,
            &["history", "sessions", "computers", device],
            &[],
            Some(json!({"name": name, "sessions": sessions})),
            Some(session.token()),
        ),
    )
    .await?;
    let ids: Vec<String> =
        serde_json::from_value(value["transcripts"].clone()).map_err(|_| Error::InvalidResponse)?;
    if ids.iter().any(|id| !sessions.iter().any(|s| &s.id == id)) {
        return Err(Error::InvalidResponse);
    }
    Ok(ids)
}

/// Remove this computer's rows and transcript copies, as when it signs out.
pub async fn forget_computer(
    client: &Client,
    session: &Session,
    device: &str,
    cancel: &CancellationToken,
) -> Result<bool, Error> {
    client.check_session(session)?;
    if !device_ok(device) {
        return Err(Error::InvalidReference);
    }
    let value: Value = cancellable(
        cancel,
        client.request(
            Method::DELETE,
            &["history", "sessions", "computers", device],
            &[],
            None,
            Some(session.token()),
        ),
    )
    .await?;
    value["removed"].as_bool().ok_or(Error::InvalidResponse)
}

/// Turn one chat's transcript on or off on the web. Returns whether it is
/// now on, counting its project's choice. Off deletes the copy.
pub async fn set_transcript(
    client: &Client,
    session: &Session,
    id: &str,
    on: bool,
    cancel: &CancellationToken,
) -> Result<bool, Error> {
    client.check_session(session)?;
    if !chat_home::valid_id(id) {
        return Err(Error::InvalidReference);
    }
    let value: Value = cancellable(
        cancel,
        client.request(
            Method::PUT,
            &["history", "sessions", id, "transcript"],
            &[],
            Some(json!({"on": on})),
            Some(session.token()),
        ),
    )
    .await?;
    value["transcript"].as_bool().ok_or(Error::InvalidResponse)
}

/// Turn the transcripts of every session in a project on or off.
pub async fn set_project_transcripts(
    client: &Client,
    session: &Session,
    project: &str,
    on: bool,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    client.check_session(session)?;
    if !project_ok(project) {
        return Err(Error::InvalidReference);
    }
    let value: Value = cancellable(
        cancel,
        client.request(
            Method::PUT,
            &["history", "sessions", "projects", project, "transcript"],
            &[],
            Some(json!({"on": on})),
            Some(session.token()),
        ),
    )
    .await?;
    if value["on"].as_bool() != Some(on) {
        return Err(Error::InvalidResponse);
    }
    Ok(())
}

/// Upload a session's read-only transcript copy, replacing the last one.
/// Refused with `transcript_off` while its transcript is off.
pub async fn upload_transcript(
    client: &Client,
    session: &Session,
    id: &str,
    messages: &[CopiedMessage],
    cancel: &CancellationToken,
) -> Result<usize, Error> {
    client.check_session(session)?;
    if !chat_home::valid_id(id) {
        return Err(Error::InvalidReference);
    }
    let value: Value = cancellable(
        cancel,
        client.request(
            Method::PUT,
            &["history", "sessions", id, "messages"],
            &[],
            Some(json!({"messages": messages})),
            Some(session.token()),
        ),
    )
    .await?;
    value["saved"]
        .as_u64()
        .map(|n| n as usize)
        .ok_or(Error::InvalidResponse)
}

/// Acknowledge the authorized indexed row actually displayed, not a newer activity.
pub async fn mark_seen(
    client: &Client,
    session: &Session,
    row: &SessionSummary,
    project: Option<&str>,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    client.check_session(session)?;
    if !row.valid() {
        return Err(Error::InvalidReference);
    }
    let record = row.record_key.as_deref().ok_or(Error::InvalidReference)?;
    let revision = row
        .activity_revision
        .as_deref()
        .ok_or(Error::InvalidReference)?;
    let mut query = Vec::new();
    if let Some(project) = project {
        if !project_ok(project) {
            return Err(Error::InvalidReference);
        }
        query.push(("project", project.to_owned()));
    }
    query.push(("record", record.to_owned()));
    let value: Value = cancellable(
        cancel,
        client.request(
            Method::PUT,
            &["history", "sessions", &row.id, "seen"],
            &query,
            Some(json!({"activity_revision":revision})),
            Some(session.token()),
        ),
    )
    .await?;
    if row
        .record_key
        .as_deref()
        .is_some_and(|record| value["record_key"].as_str() != Some(record))
        || value["id"].as_str() != Some(row.id.as_str())
        || value["activity_revision"].as_str() != Some(revision)
        || value["unseen"].as_bool() != Some(false)
    {
        return Err(Error::InvalidResponse);
    }
    Ok(())
}
