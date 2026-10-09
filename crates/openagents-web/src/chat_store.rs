//! Private public-chat records shared by HTTP commands and event readers.
//!
//! The disk adapter uses an operating-system lock and an atomic rename. The
//! Cloud Storage adapter uses generation preconditions, so another replica
//! cannot replace a record that changed after it was read. Neither adapter
//! treats an HTTP connection as the owner of a running answer.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::stream::{self, StreamExt, TryStreamExt};
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, broadcast};

const SCHEMA: &str = "openagents.web.chat.v1";
const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;
const MAX_LIST: usize = 256;
const STORAGE_API: &str = "https://storage.googleapis.com";
const METADATA_TOKEN: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Conversation {
    pub id: String,
    pub owner: String,
    pub revision: u64,
    pub title: String,
    pub messages: Vec<Message>,
    pub pending: Option<Pending>,
    pub requests: Vec<Request>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<Selection>,
    pub updated_unix: u64,
    /// When the owner pinned the chat; pinned chats list in this order at
    /// the top of the sidebar. Older records have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_unix: Option<u64>,
    /// When the owner archived the chat; archived chats leave the sidebar
    /// and list on the Archived page until restored. Older records have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_unix: Option<u64>,
    /// The project (`prj_…`, a connected GitHub repository of the signed-in
    /// account) the chat belongs to; the sidebar groups it there. The name
    /// and repository come from the account, never from this record, so a
    /// chat shows under its project only to that account. Older records
    /// have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// A chat synced from Coder in a terminal (#11046): read-only on the
    /// web. Web chats have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<Terminal>,
    /// The saved environment the chat's tasks run in (#11037), set when a
    /// task starts from the chat. Older records have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<ChatEnvironment>,
    /// The tasks started from the chat, oldest first (at most
    /// [`MAX_TASKS`]). Older records have none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<ChatTask>,
    /// When the owner last had the chat open, written only while a long
    /// task's Done waits to be seen (`pages::chat_work::unseen_done`), so
    /// opening the chat clears it. Older records have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opened_unix: Option<u64>,
}

/// Where a synced Coder chat came from, and what Coder last said about it
/// (`crate::coder_sync`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Terminal {
    /// The computer's name, as Coder reported it.
    pub computer: String,
    /// Coder's own session id.
    pub session: String,
    /// The title Coder last sent; a web rename differs from it and is kept.
    pub title: String,
    /// A digest of the last upload, so an unchanged one writes nothing.
    pub digest: String,
    /// When Coder last said it is replying; cleared when it says it is idle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_unix: Option<u64>,
    /// Deleted on the web: kept, empty, until Coder hears of it and deletes
    /// its own copy, so the next upload doesn't bring it back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_unix: Option<u64>,
    /// Replies sent on the website, waiting for Coder on the computer to
    /// take them (#11048). Coder takes them into the transcript, oldest
    /// first. At most [`MAX_WEB_REPLIES`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replies: Vec<WebReply>,
    /// The ids of the last replies sent on the website, so a resent form
    /// isn't queued twice. At most [`MAX_REPLY_IDS`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reply_ids: Vec<String>,
    /// Messages added on the website while the computer was offline: the
    /// person's words and the answers of runs on a Cloud computer
    /// (#11050). Coder's own copy doesn't have them, so each upload keeps
    /// them after Coder's transcript. At most [`MAX_CONTINUED`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub continued: Vec<Message>,
}

/// The most messages a Coder chat keeps from runs on a Cloud computer.
pub(crate) const MAX_CONTINUED: usize = 64;
/// The longest such message, in bytes (an answer is cut to fit).
pub(crate) const MAX_CONTINUED_BYTES: usize = 256 * 1024;

/// A reply typed on the website for a Coder chat (#11048).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WebReply {
    pub id: String,
    pub text: String,
    pub sent_unix: u64,
}

/// The most replies that wait for Coder in one chat.
pub(crate) const MAX_WEB_REPLIES: usize = 4;
/// How many sent reply ids a Coder chat remembers.
pub(crate) const MAX_REPLY_IDS: usize = 16;
/// The longest reply sent from the website, in bytes.
pub(crate) const MAX_WEB_REPLY_BYTES: usize = 16 * 1024;

/// Per account: the computers whose Coder checked in, each with when it
/// last did, and the Coder chats with replies from the website waiting
/// (session to computer), so Coder finds them with one read (#11048). One
/// small record beside the account's chats.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Computers {
    #[serde(default)]
    pub seen: BTreeMap<String, u64>,
    #[serde(default)]
    pub waiting: BTreeMap<String, String>,
}

/// The most computers one account's record keeps (the oldest go first).
pub(crate) const MAX_COMPUTERS: usize = 16;
/// The most Coder chats with replies waiting at once.
pub(crate) const MAX_WAITING_CHATS: usize = 64;

impl Computers {
    fn valid(&self) -> bool {
        self.seen.len() <= MAX_COMPUTERS
            && self.waiting.len() <= MAX_WAITING_CHATS
            && self.seen.keys().all(|name| bounded_text(name, 128))
            && self
                .waiting
                .iter()
                .all(|(session, computer)| coder_session(session) && bounded_text(computer, 128))
    }
}

#[derive(Serialize, Deserialize)]
struct ComputersRecord {
    schema: String,
    computers: Computers,
}

const COMPUTERS_FILE: &str = ".coder.json";

fn computers_bytes(computers: &Computers) -> Result<Vec<u8>, Error> {
    if !computers.valid() {
        return Err(Error::Invalid("The computer list is invalid."));
    }
    serde_json::to_vec(&ComputersRecord {
        schema: format!("{SCHEMA}.coder"),
        computers: computers.clone(),
    })
    .map_err(|_| Error::Invalid("The computer list is invalid."))
}

fn decode_computers(bytes: &[u8]) -> Result<Computers, Error> {
    let record: ComputersRecord = serde_json::from_slice(bytes)
        .map_err(|_| Error::Corrupt("The computer list is invalid."))?;
    if record.schema != format!("{SCHEMA}.coder") || !record.computers.valid() {
        return Err(Error::Corrupt("The computer list is invalid."));
    }
    Ok(record.computers)
}

/// Coder's session ids: letters, numbers, `_`, and `-`, up to 128 bytes.
fn coder_session(session: &str) -> bool {
    !session.is_empty()
        && session.len() <= 128
        && session
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// How long a "working" heartbeat from Coder shows the chat as Working.
pub(crate) const TERMINAL_WORKING_SECONDS: u64 = 90;

impl Conversation {
    /// Whether the chat is being answered now: a web answer is running, or
    /// Coder said it is replying within the last
    /// [`TERMINAL_WORKING_SECONDS`].
    pub(crate) fn working(&self) -> bool {
        self.pending.is_some()
            || self.terminal.as_ref().is_some_and(|terminal| {
                terminal
                    .working_unix
                    .is_some_and(|at| now_unix().saturating_sub(at) <= TERMINAL_WORKING_SECONDS)
            })
    }

    /// Whether this is a Coder chat deleted on the web (hidden everywhere).
    pub(crate) fn deleted(&self) -> bool {
        self.terminal
            .as_ref()
            .is_some_and(|terminal| terminal.deleted_unix.is_some())
    }
}

/// The most tasks a chat keeps; the oldest go first.
pub(crate) const MAX_TASKS: usize = 64;

/// A saved environment (`/environments/{id}`) a chat runs tasks in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChatEnvironment {
    pub id: String,
    /// `owner/name`: the repository the environment sets up.
    pub repository: String,
    /// The saved version the newest task ran on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
    /// The environment no longer exists; the chat still opens.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub removed: bool,
}

/// One task started from a chat: today a Claude Code run on the chat's
/// environment (`/environments/{environment}/runs/{id}`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChatTask {
    pub id: String,
    pub kind: TaskKind,
    pub environment: String,
    /// What the person asked, cut to one short line.
    pub title: String,
    pub state: TaskState,
    pub started_unix: u64,
    /// How many messages the chat had when the task started; the task row
    /// shows after them.
    pub after_message: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
    /// When the chat first saw the task finished. Older records have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_unix: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TaskKind {
    Claude,
    /// A Coder chat continued on a Cloud computer while its own computer
    /// was offline (#11050): the run's answer joins the transcript.
    Continue,
}

/// A task's last known state, as the run reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TaskState {
    Working,
    /// Stopped for a usage limit; continues by itself.
    Paused,
    Done,
    Failed,
    Stopped,
}

impl TaskState {
    pub(crate) fn finished(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Stopped)
    }
}

impl ChatTask {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if !coder_environment::valid_id(&self.id)
            || !coder_environment::valid_id(&self.environment)
            || !bounded_text(&self.title, 512)
        {
            return Err(Error::Invalid("The chat's task is invalid."));
        }
        Ok(())
    }
}

impl ChatEnvironment {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if !coder_environment::valid_id(&self.id) || !bounded_text(&self.repository, 256) {
            return Err(Error::Invalid("The chat's environment is invalid."));
        }
        Ok(())
    }
}

/// GitHub metadata identifies a selected source; it does not authorize execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepositorySource {
    pub repository: String,
    pub branch: String,
    pub revision: String,
}

/// A runtime pins one native catalog entry and its account authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeSelection {
    pub binding: String,
    pub account: String,
    pub workspace: String,
    pub members_epoch: u64,
    pub project: String,
    pub profile: String,
    pub profile_revision: String,
    pub source_revision: String,
    pub source_digest: String,
    pub placement: String,
    pub executor: String,
    pub model: Option<String>,
    pub max_timeout_seconds: u64,
}

/// The conversation can change its selection; an accepted request retains it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Selection {
    pub revision: u64,
    pub repository: Option<RepositorySource>,
    pub runtime: Option<RuntimeSelection>,
}

/// The native control journal owns the signed request and execution result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CloudRequest {
    pub binding: String,
    pub request: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Message {
    pub role: Role,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Role {
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Pending {
    pub request_id: String,
    pub started_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Request {
    pub id: String,
    pub digest: String,
    pub outcome: Outcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<Selection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud: Option<CloudRequest>,
    /// What the router said about the answer that the chips under it read:
    /// its prepared answer, follow-ups, offers to run Coder or open a
    /// screen, and the plugins it shows as cards
    /// (`openagents_chat::suggestions::chip_meta`). Older records have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<openagents_chat::router::Meta>,
}

/// The most follow-ups, offers, or plugin cards a retained answer keeps.
const MAX_REPLY_CHIPS: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Outcome {
    Pending,
    Answered,
    Failed,
    Unknown,
}

/// A generation identifies the bytes that a caller read, not its authority.
#[derive(Clone, Debug)]
pub(crate) struct Loaded {
    pub conversation: Conversation,
    pub generation: String,
}

#[derive(Debug)]
pub enum Error {
    Conflict,
    Invalid(&'static str),
    Corrupt(&'static str),
    Unavailable(&'static str),
    Http(u16),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => formatter.write_str("The conversation changed. Read it again."),
            Self::Invalid(message) | Self::Corrupt(message) | Self::Unavailable(message) => {
                formatter.write_str(message)
            }
            Self::Http(status) => write!(formatter, "Conversation storage returned HTTP {status}."),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone)]
pub struct Store(Arc<Adapter>, broadcast::Sender<Change>);

/// A chat this process just wrote, for readers that follow a visitor's
/// chats without polling each one (the sidebar's live stream, see
/// `docs/web/sidebar.md` "Live updates"). Only writes made by this process
/// are announced; another replica's writes reach readers through their own
/// slower checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Change {
    pub owner: Arc<str>,
    pub id: Arc<str>,
}

/// How many announcements a slow reader may fall behind before it is told
/// it missed some (and checks every chat instead).
const CHANGES: usize = 1024;

enum Adapter {
    Disk(PathBuf),
    Gcs(Gcs),
}

struct Gcs {
    bucket: String,
    prefix: String,
    client: Client,
    metadata: Client,
    token: Mutex<Option<Token>>,
}

struct Token {
    bearer: String,
    expires: Instant,
}

#[derive(Serialize, Deserialize)]
struct Record {
    schema: String,
    conversation: Conversation,
}

#[derive(Serialize, Deserialize)]
struct Active {
    schema: String,
    request_id: String,
    expires_unix: u64,
}

impl Store {
    /// Disk storage is suitable for a single machine and survives restarts.
    pub fn local(directory: PathBuf) -> Self {
        Self(
            Arc::new(Adapter::Disk(directory)),
            broadcast::channel(CHANGES).0,
        )
    }

    /// Use a private bucket with object read, create, delete, and list rights.
    pub fn gcs(bucket: String, prefix: String) -> Result<Self, Error> {
        if bucket.is_empty()
            || bucket.len() > 222
            || !bucket.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'-' | b'_')
            })
        {
            return Err(Error::Invalid("The chat bucket name is invalid."));
        }
        let prefix = prefix.trim_matches('/').to_owned();
        if prefix.is_empty()
            || prefix.len() > 128
            || prefix
                .split('/')
                .any(|part| part.is_empty() || part == "..")
            || !prefix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_'))
        {
            return Err(Error::Invalid("The chat object prefix is invalid."));
        }
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::Unavailable("Conversation storage could not initialize."))?;
        let metadata = Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::Unavailable("Cloud identity could not initialize."))?;
        Ok(Self(
            Arc::new(Adapter::Gcs(Gcs {
                bucket,
                prefix,
                client,
                metadata,
                token: Mutex::new(None),
            })),
            broadcast::channel(CHANGES).0,
        ))
    }

    /// Announcements of this process's chat writes, from now on.
    pub(crate) fn changes(&self) -> broadcast::Receiver<Change> {
        self.1.subscribe()
    }

    pub(crate) async fn load(&self, owner: &str, id: &str) -> Result<Option<Loaded>, Error> {
        validate_address(owner, id)?;
        match self.0.as_ref() {
            Adapter::Disk(root) => {
                let path = path(root, owner, id);
                let owner = owner.to_owned();
                let id = id.to_owned();
                blocking(move || read_disk(&path, &owner, &id)).await
            }
            Adapter::Gcs(gcs) => gcs.load(owner, id).await,
        }
    }

    /// Create succeeds once, including when two replicas race to create it.
    pub(crate) async fn create(&self, conversation: &Conversation) -> Result<Loaded, Error> {
        validate_address(&conversation.owner, &conversation.id)?;
        if conversation.revision != 1 {
            return Err(Error::Invalid("A new conversation must have revision 1."));
        }
        self.write(conversation, None).await
    }

    /// A mutation must preserve identity and advance the revision once.
    pub(crate) async fn compare_and_swap(
        &self,
        previous: &Loaded,
        conversation: &Conversation,
    ) -> Result<Loaded, Error> {
        validate_address(&conversation.owner, &conversation.id)?;
        if previous.conversation.owner != conversation.owner
            || previous.conversation.id != conversation.id
            || previous.conversation.revision.checked_add(1) != Some(conversation.revision)
        {
            return Err(Error::Invalid("The conversation mutation is invalid."));
        }
        validate_retained_requests(&previous.conversation, conversation)?;
        self.write(conversation, Some(previous.generation.clone()))
            .await
    }

    async fn write(
        &self,
        conversation: &Conversation,
        previous: Option<String>,
    ) -> Result<Loaded, Error> {
        let bytes = encode(conversation)?;
        let generation = match self.0.as_ref() {
            Adapter::Disk(root) => {
                let path = path(root, &conversation.owner, &conversation.id);
                let owner = conversation.owner.clone();
                let id = conversation.id.clone();
                blocking(move || write_disk(&path, &owner, &id, &bytes, previous.as_deref()))
                    .await?
            }
            Adapter::Gcs(gcs) => {
                gcs.write(conversation, bytes, previous.as_deref().unwrap_or("0"))
                    .await?
            }
        };
        // Nobody listening is not an error.
        let _ = self.1.send(Change {
            owner: conversation.owner.as_str().into(),
            id: conversation.id.as_str().into(),
        });
        Ok(Loaded {
            conversation: conversation.clone(),
            generation,
        })
    }

    /// The list is private to one visitor. Fail explicitly if it exceeds 256.
    /// Coder chats deleted on the web are left out ([`Conversation::deleted`]).
    pub(crate) async fn list(&self, owner: &str) -> Result<Vec<Conversation>, Error> {
        let mut rows = self.list_with_deleted(owner).await?;
        rows.retain(|chat| !chat.deleted());
        Ok(rows)
    }

    /// [`Self::list`], with the deleted Coder chats Coder hasn't heard of yet.
    pub(crate) async fn list_with_deleted(&self, owner: &str) -> Result<Vec<Conversation>, Error> {
        validate_owner(owner)?;
        let ids = match self.0.as_ref() {
            Adapter::Disk(root) => {
                let directory = root.join(owner_digest(owner));
                blocking(move || {
                    let entries = match fs::read_dir(directory) {
                        Ok(entries) => entries,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            return Ok(Vec::new());
                        }
                        Err(_) => {
                            return Err(Error::Unavailable("The chat list could not be read."));
                        }
                    };
                    let mut ids = Vec::new();
                    for entry in entries {
                        let entry = entry
                            .map_err(|_| Error::Unavailable("The chat list could not be read."))?;
                        let name = entry.file_name();
                        let Some(id) = name.to_str().and_then(|name| name.strip_suffix(".json"))
                        else {
                            continue;
                        };
                        if valid_id(id) {
                            ids.push(id.to_owned());
                            if ids.len() > MAX_LIST {
                                return Err(Error::Unavailable("The chat list exceeds its limit."));
                            }
                        }
                    }
                    Ok(ids)
                })
                .await?
            }
            Adapter::Gcs(gcs) => gcs.list(owner).await?,
        };
        let mut conversations: Vec<Conversation> = stream::iter(ids)
            .map(|id| async move { self.load(owner, &id).await })
            .buffer_unordered(8)
            .try_filter_map(|loaded| async { Ok(loaded.map(|loaded| loaded.conversation)) })
            .try_collect()
            .await?;
        conversations.sort_by(|left, right| {
            right
                .updated_unix
                .cmp(&left.updated_unix)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(conversations)
    }

    /// Claim one answer per visitor across processes and replicas.
    ///
    /// A duplicate claim returns false, including a matching request identity.
    /// An expired claim can be replaced, but the old answer remains unknown
    /// until its conversation records that outcome. Do not replay that answer.
    pub(crate) async fn claim(
        &self,
        owner: &str,
        request_id: &str,
        expires_unix: u64,
    ) -> Result<bool, Error> {
        validate_address(owner, request_id)?;
        let now = now_unix();
        if expires_unix <= now || expires_unix > now.saturating_add(3600) {
            return Err(Error::Invalid("The answer lease expiration is invalid."));
        }
        let active = Active {
            schema: format!("{SCHEMA}.active"),
            request_id: request_id.to_owned(),
            expires_unix,
        };
        match self.0.as_ref() {
            Adapter::Disk(root) => {
                let directory = root.join(owner_digest(owner));
                blocking(move || {
                    create_directory(&directory)?;
                    let path = directory.join(".active.json");
                    let _lock = lock(&directory.join(".active.lock"))?;
                    if read_active_disk(&path)?.is_some_and(|claim| claim.expires_unix > now) {
                        return Ok(false);
                    }
                    let bytes = active_bytes(&active)?;
                    atomic_write(&path, &bytes)?;
                    Ok(true)
                })
                .await
            }
            Adapter::Gcs(gcs) => {
                let name = format!("{}.active.json", gcs.owner_prefix(owner));
                for _ in 0..4 {
                    let loaded = gcs.read_object(&name).await?;
                    let generation = match loaded {
                        Some((bytes, generation)) => {
                            if decode_active(&bytes)?.expires_unix > now {
                                return Ok(false);
                            }
                            generation
                        }
                        None => "0".to_owned(),
                    };
                    match gcs
                        .put_object(&name, active_bytes(&active)?, &generation)
                        .await
                    {
                        Ok(_) => return Ok(true),
                        Err(Error::Conflict) => continue,
                        Err(error) => return Err(error),
                    }
                }
                Err(Error::Conflict)
            }
        }
    }

    /// A completed answer can release its own lease, never a newer answer's.
    pub(crate) async fn release(&self, owner: &str, request_id: &str) -> Result<(), Error> {
        validate_address(owner, request_id)?;
        match self.0.as_ref() {
            Adapter::Disk(root) => {
                let directory = root.join(owner_digest(owner));
                let request_id = request_id.to_owned();
                blocking(move || {
                    create_directory(&directory)?;
                    let path = directory.join(".active.json");
                    let _lock = lock(&directory.join(".active.lock"))?;
                    let Some(mut active) = read_active_disk(&path)? else {
                        return Ok(());
                    };
                    if active.request_id == request_id {
                        active.request_id.clear();
                        active.expires_unix = 0;
                        atomic_write(&path, &active_bytes(&active)?)?;
                    }
                    Ok(())
                })
                .await
            }
            Adapter::Gcs(gcs) => {
                let name = format!("{}.active.json", gcs.owner_prefix(owner));
                for _ in 0..4 {
                    let Some((bytes, generation)) = gcs.read_object(&name).await? else {
                        return Ok(());
                    };
                    let mut active = decode_active(&bytes)?;
                    if active.request_id != request_id {
                        return Ok(());
                    }
                    active.request_id.clear();
                    active.expires_unix = 0;
                    match gcs
                        .put_object(&name, active_bytes(&active)?, &generation)
                        .await
                    {
                        Ok(_) => return Ok(()),
                        Err(Error::Conflict) => continue,
                        Err(error) => return Err(error),
                    }
                }
                Err(Error::Conflict)
            }
        }
    }

    /// The account's Coder computers and waiting replies (#11048). None
    /// saved is empty.
    pub(crate) async fn computers(&self, owner: &str) -> Result<Computers, Error> {
        validate_owner(owner)?;
        match self.0.as_ref() {
            Adapter::Disk(root) => {
                let path = root.join(owner_digest(owner)).join(COMPUTERS_FILE);
                blocking(move || read_computers_disk(&path)).await
            }
            Adapter::Gcs(gcs) => {
                let name = format!("{}{COMPUTERS_FILE}", gcs.owner_prefix(owner));
                match gcs.read_object(&name).await? {
                    Some((bytes, _)) => decode_computers(&bytes),
                    None => Ok(Computers::default()),
                }
            }
        }
    }

    /// Change the account's Coder computers record with `change`, which
    /// says whether it changed anything; nothing is written when it
    /// didn't. Returns the record as it now stands.
    pub(crate) async fn update_computers<F>(
        &self,
        owner: &str,
        change: F,
    ) -> Result<Computers, Error>
    where
        F: Fn(&mut Computers) -> bool + Send + Sync + 'static,
    {
        validate_owner(owner)?;
        match self.0.as_ref() {
            Adapter::Disk(root) => {
                let directory = root.join(owner_digest(owner));
                blocking(move || {
                    create_directory(&directory)?;
                    let path = directory.join(COMPUTERS_FILE);
                    let _lock = lock(&directory.join(".coder.lock"))?;
                    let mut computers = read_computers_disk(&path)?;
                    if change(&mut computers) {
                        atomic_write(&path, &computers_bytes(&computers)?)?;
                    }
                    Ok(computers)
                })
                .await
            }
            Adapter::Gcs(gcs) => {
                let name = format!("{}{COMPUTERS_FILE}", gcs.owner_prefix(owner));
                for _ in 0..4 {
                    let (mut computers, generation) = match gcs.read_object(&name).await? {
                        Some((bytes, generation)) => (decode_computers(&bytes)?, generation),
                        None => (Computers::default(), "0".to_owned()),
                    };
                    if !change(&mut computers) {
                        return Ok(computers);
                    }
                    match gcs
                        .put_object(&name, computers_bytes(&computers)?, &generation)
                        .await
                    {
                        Ok(_) => return Ok(computers),
                        Err(Error::Conflict) => continue,
                        Err(error) => return Err(error),
                    }
                }
                Err(Error::Conflict)
            }
        }
    }

    /// Remove one chat for good. `expected` is the generation the caller
    /// read: a chat that changed since then is not removed (`Conflict`), so
    /// a delete never drops a message the person did not see. Returns false
    /// when the chat was already gone. Nothing recreates a deleted chat: a
    /// later compare-and-swap against it fails.
    pub(crate) async fn delete(
        &self,
        owner: &str,
        id: &str,
        expected: &str,
    ) -> Result<bool, Error> {
        validate_address(owner, id)?;
        match self.0.as_ref() {
            Adapter::Disk(root) => {
                let path = path(root, owner, id);
                let owner = owner.to_owned();
                let id = id.to_owned();
                let expected = expected.to_owned();
                blocking(move || delete_disk(&path, &owner, &id, &expected)).await
            }
            Adapter::Gcs(gcs) => gcs.delete_chat(&gcs.object(owner, id), expected).await,
        }
    }

    /// Delete a chat the person asked to delete, read at `loaded`. A Coder
    /// chat is emptied and marked deleted instead, until Coder deletes its
    /// own copy (`crate::coder_sync`); other chats go for good. Returns
    /// false when it was already gone.
    pub(crate) async fn remove(&self, loaded: &Loaded) -> Result<bool, Error> {
        let chat = &loaded.conversation;
        if chat.terminal.is_none() {
            return self.delete(&chat.owner, &chat.id, &loaded.generation).await;
        }
        if chat.deleted() {
            return Ok(false);
        }
        let mut next = chat.clone();
        next.revision += 1;
        next.updated_unix = now_unix();
        next.title = String::new();
        next.messages.clear();
        next.pinned_unix = None;
        next.archived_unix = None;
        if let Some(terminal) = &mut next.terminal {
            terminal.title.clear();
            terminal.digest.clear();
            terminal.working_unix = None;
            terminal.replies.clear();
            terminal.reply_ids.clear();
            terminal.continued.clear();
            terminal.deleted_unix = Some(now_unix());
        }
        self.compare_and_swap(loaded, &next).await.map(|_| true)
    }

    /// Move one chat to `to` (a signed-in account's owner, see
    /// [`account_owner`]) when its browser signs in. The copy is written
    /// only where nothing exists yet, then the original is removed fenced by
    /// the generation `from` was read at. If the original changed or went
    /// away meanwhile, the copy is removed again and the chat stays where
    /// it was. Returns whether the chat moved.
    pub(crate) async fn adopt(&self, from: &Loaded, to: &str) -> Result<bool, Error> {
        let source = &from.conversation;
        validate_address(&source.owner, &source.id)?;
        validate_address(to, &source.id)?;
        if source.owner == to {
            return Err(Error::Invalid("The chat already belongs here."));
        }
        let mut moved = source.clone();
        moved.owner = to.to_owned();
        validate_conversation(&moved)?;
        let copy = match self.write(&moved, None).await {
            Ok(copy) => copy,
            Err(Error::Conflict) => return Ok(false),
            Err(error) => return Err(error),
        };
        match self
            .delete(&source.owner, &source.id, &from.generation)
            .await
        {
            Ok(true) => Ok(true),
            Ok(false) | Err(Error::Conflict) => {
                self.delete(to, &source.id, &copy.generation).await?;
                Ok(false)
            }
            Err(error) => {
                let _ = self.delete(to, &source.id, &copy.generation).await;
                Err(error)
            }
        }
    }

    /// Remove every chat, for every visitor, untouched since `cutoff_unix`
    /// (the server's `--chat-retention-days`; see
    /// `docs/deployment/openagents-web.md`). On disk a chat's last activity
    /// is its `updated_unix`; in the bucket it is the time its current
    /// object was written, which every change rewrites. Each removal is
    /// fenced by the generation it was judged on, so a chat that gets a new
    /// message during the sweep stays. Answer leases are left alone. Returns
    /// how many chats were removed.
    pub async fn expire_untouched(&self, cutoff_unix: u64) -> Result<usize, Error> {
        match self.0.as_ref() {
            Adapter::Disk(root) => {
                let root = root.clone();
                blocking(move || expire_disk(&root, cutoff_unix)).await
            }
            Adapter::Gcs(gcs) => gcs.expire(cutoff_unix).await,
        }
    }
}

impl Gcs {
    async fn bearer(&self) -> Result<String, Error> {
        // Only credential refresh holds this lock. Record reads and CAS do not.
        let mut cached = self.token.lock().await;
        if let Some(token) = cached.as_ref()
            && token.expires > Instant::now()
        {
            return Ok(token.bearer.clone());
        }
        let response = self
            .metadata
            .get(METADATA_TOKEN)
            .header("Metadata-Flavor", "Google")
            .send()
            .await
            .map_err(|_| Error::Unavailable("The cloud identity is unavailable."))?;
        check_status(&response)?;
        #[derive(Deserialize)]
        struct Credentials {
            access_token: String,
            expires_in: u64,
        }
        let bytes = limited_body(response, 16 * 1024).await?;
        let credentials: Credentials = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Unavailable("The cloud identity response is invalid."))?;
        if credentials.access_token.is_empty() || credentials.expires_in <= 60 {
            return Err(Error::Unavailable("The cloud identity has expired."));
        }
        let bearer = credentials.access_token;
        *cached = Some(Token {
            bearer: bearer.clone(),
            expires: Instant::now()
                + Duration::from_secs(credentials.expires_in.saturating_sub(60).min(3600)),
        });
        Ok(bearer)
    }

    fn object(&self, owner: &str, id: &str) -> String {
        format!("{}{id}.json", self.owner_prefix(owner))
    }

    fn owner_prefix(&self, owner: &str) -> String {
        format!("{}/{}/", self.prefix, owner_digest(owner))
    }

    fn object_url(&self, object: &str) -> Result<url::Url, Error> {
        let mut url = url::Url::parse(STORAGE_API)
            .map_err(|_| Error::Unavailable("The storage endpoint is invalid."))?;
        url.path_segments_mut()
            .map_err(|_| Error::Unavailable("The storage endpoint is invalid."))?
            .extend(["storage", "v1", "b", &self.bucket, "o", object]);
        Ok(url)
    }

    async fn load(&self, owner: &str, id: &str) -> Result<Option<Loaded>, Error> {
        let Some((bytes, generation)) = self.read_object(&self.object(owner, id)).await? else {
            return Ok(None);
        };
        Ok(Some(Loaded {
            conversation: decode(&bytes, owner, id)?,
            generation,
        }))
    }

    async fn read_object(&self, name: &str) -> Result<Option<(Vec<u8>, String)>, Error> {
        for attempt in 0..3 {
            match self.read_object_once(name).await {
                Err(Error::Conflict) if attempt < 2 => {
                    // A bucket without versioning can replace the generation
                    // between metadata and media reads. Read the new pair;
                    // never relabel newer bytes with the older generation.
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                result => return result,
            }
        }
        Err(Error::Conflict)
    }

    async fn read_object_once(&self, name: &str) -> Result<Option<(Vec<u8>, String)>, Error> {
        let bearer = self.bearer().await?;
        let url = self.object_url(name)?;
        let response = self
            .client
            .get(url.clone())
            .query(&[("fields", "generation")])
            .bearer_auth(&bearer)
            .send()
            .await
            .map_err(|_| Error::Unavailable("The conversation could not be read."))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        check_status(&response)?;
        #[derive(Deserialize)]
        struct Object {
            generation: String,
        }
        let object: Object = serde_json::from_slice(&limited_body(response, 16 * 1024).await?)
            .map_err(|_| Error::Corrupt("The object generation is invalid."))?;
        if object.generation.is_empty()
            || !object.generation.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(Error::Corrupt("The object generation is invalid."));
        }
        let response = self
            .client
            .get(url)
            .query(&[("alt", "media"), ("generation", object.generation.as_str())])
            .bearer_auth(&bearer)
            .send()
            .await
            .map_err(|_| Error::Unavailable("The conversation could not be read."))?;
        // A generation replaced between metadata and media reads is a conflict,
        // not permission to return a newer record under the older generation.
        if response.status() == StatusCode::NOT_FOUND {
            return Err(Error::Conflict);
        }
        check_status(&response)?;
        let bytes = limited_body(response, MAX_RECORD_BYTES).await?;
        Ok(Some((bytes, object.generation)))
    }

    async fn write(
        &self,
        conversation: &Conversation,
        bytes: Vec<u8>,
        generation: &str,
    ) -> Result<String, Error> {
        self.put_object(
            &self.object(&conversation.owner, &conversation.id),
            bytes,
            generation,
        )
        .await
    }

    async fn put_object(
        &self,
        name: &str,
        bytes: Vec<u8>,
        generation: &str,
    ) -> Result<String, Error> {
        if generation.is_empty() || !generation.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(Error::Invalid("The object generation is invalid."));
        }
        let response = self
            .client
            .post(format!(
                "{STORAGE_API}/upload/storage/v1/b/{}/o",
                self.bucket
            ))
            .query(&[
                ("uploadType", "media"),
                ("name", name),
                ("ifGenerationMatch", generation),
                ("fields", "generation"),
            ])
            .bearer_auth(self.bearer().await?)
            .header("Content-Type", "application/json")
            .body(bytes)
            .send()
            .await
            .map_err(|_| Error::Unavailable("The conversation write has an unknown outcome."))?;
        if response.status() == StatusCode::PRECONDITION_FAILED
            || response.status() == StatusCode::CONFLICT
        {
            return Err(Error::Conflict);
        }
        check_status(&response)?;
        #[derive(Deserialize)]
        struct Object {
            generation: String,
        }
        let object: Object = serde_json::from_slice(&limited_body(response, 16 * 1024).await?)
            .map_err(|_| Error::Corrupt("The object generation is invalid."))?;
        if object.generation.is_empty()
            || !object.generation.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(Error::Corrupt("The object generation is invalid."));
        }
        Ok(object.generation)
    }

    /// Delete a chat whose live object is at `generation`, then every older
    /// version of it a versioned bucket kept, so nothing of it stays
    /// readable (the bucket's own soft-delete window aside; see
    /// `docs/deployment/web-chat-retention.md`). A missing live object is
    /// `false`; its older versions are still removed.
    async fn delete_chat(&self, name: &str, generation: &str) -> Result<bool, Error> {
        let removed = self.delete_object(name, generation).await?;
        self.purge_versions(name).await?;
        Ok(removed)
    }

    /// Remove every stored version of `name`. Without versioning there are
    /// none left after the live object is deleted.
    async fn purge_versions(&self, name: &str) -> Result<(), Error> {
        let mut page = String::new();
        loop {
            let response = self
                .client
                .get(format!("{STORAGE_API}/storage/v1/b/{}/o", self.bucket))
                .query(&[
                    ("prefix", name),
                    ("versions", "true"),
                    ("maxResults", "1000"),
                    ("fields", "items(name,generation),nextPageToken"),
                    ("pageToken", page.as_str()),
                ])
                .bearer_auth(self.bearer().await?)
                .send()
                .await
                .map_err(|_| Error::Unavailable("The chat delete has an unknown outcome."))?;
            check_status(&response)?;
            #[derive(Deserialize)]
            struct Object {
                name: String,
                generation: String,
            }
            #[derive(Deserialize)]
            struct Page {
                #[serde(default)]
                items: Vec<Object>,
                #[serde(default, rename = "nextPageToken")]
                next: String,
            }
            let listed: Page = serde_json::from_slice(&limited_body(response, 1024 * 1024).await?)
                .map_err(|_| Error::Corrupt("The chat versions are invalid."))?;
            for object in listed.items.iter().filter(|object| object.name == name) {
                if object.generation.is_empty()
                    || !object.generation.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(Error::Corrupt("The object generation is invalid."));
                }
                let response = self
                    .client
                    .delete(self.object_url(name)?)
                    .query(&[("generation", object.generation.as_str())])
                    .bearer_auth(self.bearer().await?)
                    .send()
                    .await
                    .map_err(|_| Error::Unavailable("The chat delete has an unknown outcome."))?;
                if response.status() != StatusCode::NOT_FOUND {
                    check_status(&response)?;
                }
            }
            if listed.next.is_empty() {
                return Ok(());
            }
            if listed.next == page {
                return Err(Error::Corrupt("The chat versions cursor did not advance."));
            }
            page = listed.next;
        }
    }

    /// Delete the live object at `generation`; a missing object is `false`.
    async fn delete_object(&self, name: &str, generation: &str) -> Result<bool, Error> {
        if generation.is_empty() || !generation.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(Error::Invalid("The object generation is invalid."));
        }
        let response = self
            .client
            .delete(self.object_url(name)?)
            .query(&[("ifGenerationMatch", generation)])
            .bearer_auth(self.bearer().await?)
            .send()
            .await
            .map_err(|_| Error::Unavailable("The chat delete has an unknown outcome."))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(false);
        }
        if response.status() == StatusCode::PRECONDITION_FAILED
            || response.status() == StatusCode::CONFLICT
        {
            return Err(Error::Conflict);
        }
        check_status(&response)?;
        Ok(true)
    }

    /// One pass over every chat object; those last written before
    /// `cutoff_unix` are removed at the generation that was listed.
    async fn expire(&self, cutoff_unix: u64) -> Result<usize, Error> {
        let prefix = format!("{}/", self.prefix);
        let mut page = String::new();
        let mut removed = 0;
        loop {
            let response = self
                .client
                .get(format!("{STORAGE_API}/storage/v1/b/{}/o", self.bucket))
                .query(&[
                    ("prefix", prefix.as_str()),
                    ("maxResults", "1000"),
                    ("fields", "items(name,generation,updated),nextPageToken"),
                    ("pageToken", page.as_str()),
                ])
                .bearer_auth(self.bearer().await?)
                .send()
                .await
                .map_err(|_| Error::Unavailable("The chat list could not be read."))?;
            check_status(&response)?;
            #[derive(Deserialize)]
            struct Object {
                name: String,
                generation: String,
                updated: String,
            }
            #[derive(Deserialize)]
            struct Page {
                #[serde(default)]
                items: Vec<Object>,
                #[serde(default, rename = "nextPageToken")]
                next: String,
            }
            let listed: Page = serde_json::from_slice(&limited_body(response, 1024 * 1024).await?)
                .map_err(|_| Error::Corrupt("The chat list is invalid."))?;
            for object in listed.items {
                if !expirable_object(&prefix, &object.name) {
                    continue;
                }
                let Some(updated) = rfc3339_unix(&object.updated) else {
                    continue;
                };
                if updated >= cutoff_unix {
                    continue;
                }
                match self.delete_chat(&object.name, &object.generation).await {
                    Ok(true) => removed += 1,
                    // Gone already, or written again since the listing.
                    Ok(false) | Err(Error::Conflict) => {}
                    Err(error) => return Err(error),
                }
            }
            if listed.next.is_empty() {
                return Ok(removed);
            }
            if listed.next == page {
                return Err(Error::Corrupt("The chat list cursor did not advance."));
            }
            page = listed.next;
        }
    }

    async fn list(&self, owner: &str) -> Result<Vec<String>, Error> {
        let prefix = self.owner_prefix(owner);
        let bearer = self.bearer().await?;
        let mut page = String::new();
        let mut ids = Vec::new();
        loop {
            let response = self
                .client
                .get(format!("{STORAGE_API}/storage/v1/b/{}/o", self.bucket))
                .query(&[
                    ("prefix", prefix.as_str()),
                    ("maxResults", "257"),
                    ("fields", "items(name),nextPageToken"),
                    ("pageToken", page.as_str()),
                ])
                .bearer_auth(&bearer)
                .send()
                .await
                .map_err(|_| Error::Unavailable("The chat list could not be read."))?;
            check_status(&response)?;
            #[derive(Deserialize)]
            struct Object {
                name: String,
            }
            #[derive(Deserialize)]
            struct Page {
                #[serde(default)]
                items: Vec<Object>,
                #[serde(default, rename = "nextPageToken")]
                next: String,
            }
            let listed: Page = serde_json::from_slice(&limited_body(response, 128 * 1024).await?)
                .map_err(|_| Error::Corrupt("The chat list is invalid."))?;
            for object in listed.items {
                if object.name == format!("{prefix}.active.json")
                    || object.name == format!("{prefix}{COMPUTERS_FILE}")
                {
                    continue;
                }
                let Some(id) = object
                    .name
                    .strip_prefix(&prefix)
                    .and_then(|name| name.strip_suffix(".json"))
                else {
                    return Err(Error::Corrupt("The chat list contains an invalid object."));
                };
                if !valid_id(id) {
                    return Err(Error::Corrupt("The chat list contains an invalid object."));
                }
                ids.push(id.to_owned());
                if ids.len() > MAX_LIST {
                    return Err(Error::Unavailable("The chat list exceeds its limit."));
                }
            }
            if listed.next.is_empty() {
                return Ok(ids);
            }
            if listed.next == page {
                return Err(Error::Corrupt("The chat list cursor did not advance."));
            }
            page = listed.next;
        }
    }
}

fn check_status(response: &Response) -> Result<(), Error> {
    if response.status().is_success() {
        Ok(())
    } else {
        // Provider response bodies can contain record names. Do not report them.
        Err(Error::Http(response.status().as_u16()))
    }
}

async fn limited_body(mut response: Response, limit: usize) -> Result<Vec<u8>, Error> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(Error::Corrupt(
            "The storage response exceeds its size limit.",
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| Error::Unavailable("The storage response was interrupted."))?
    {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(Error::Corrupt(
                "The storage response exceeds its size limit.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn encode(conversation: &Conversation) -> Result<Vec<u8>, Error> {
    validate_conversation(conversation)?;
    let bytes = serde_json::to_vec(&Record {
        schema: SCHEMA.to_owned(),
        conversation: conversation.clone(),
    })
    .map_err(|_| Error::Invalid("The conversation could not be encoded."))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(Error::Invalid(
            "The conversation exceeds its retained size limit.",
        ));
    }
    Ok(bytes)
}

fn decode(bytes: &[u8], owner: &str, id: &str) -> Result<Conversation, Error> {
    let record: Record = serde_json::from_slice(bytes)
        .map_err(|_| Error::Corrupt("The retained conversation is invalid."))?;
    if record.schema != SCHEMA
        || record.conversation.owner != owner
        || record.conversation.id != id
        || record.conversation.revision == 0
    {
        return Err(Error::Corrupt(
            "The retained conversation identity is invalid.",
        ));
    }
    validate_conversation(&record.conversation)
        .map_err(|_| Error::Corrupt("The retained conversation selection is invalid."))?;
    Ok(record.conversation)
}

impl RepositorySource {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        coder_access::cloud::repository(&self.repository)
            .map_err(|_| Error::Invalid("The selected repository is invalid."))?;
        coder_access::cloud::branch(&self.branch)
            .map_err(|_| Error::Invalid("The selected branch is invalid."))?;
        if !commit(&self.revision) {
            return Err(Error::Invalid(
                "The selected repository revision is invalid.",
            ));
        }
        Ok(())
    }
}

impl RuntimeSelection {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if !binding_id(&self.binding)
            || !bounded_text(&self.account, 128)
            || !self.account.bytes().all(|byte| byte.is_ascii_graphic())
            || self.members_epoch > 9_007_199_254_740_991
            || !commit(&self.source_revision)
            || !matches!(self.placement.as_str(), "boat" | "gce")
            || !bounded_text(&self.executor, 128)
            || self
                .model
                .as_ref()
                .is_some_and(|model| !bounded_text(model, 128))
            || self.max_timeout_seconds == 0
            || self.max_timeout_seconds > 43_200
        {
            return Err(Error::Invalid("The selected runtime is invalid."));
        }
        for alias in [&self.workspace, &self.project, &self.profile] {
            coder_access::cloud::alias(alias)
                .map_err(|_| Error::Invalid("The selected runtime scope is invalid."))?;
        }
        for pin in [&self.profile_revision, &self.source_digest] {
            coder_access::cloud::digest(pin)
                .map_err(|_| Error::Invalid("The selected runtime pin is invalid."))?;
        }
        Ok(())
    }
}

impl Selection {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.revision == 0 && (self.repository.is_some() || self.runtime.is_some()) {
            return Err(Error::Invalid("The selection revision is invalid."));
        }
        if let Some(repository) = &self.repository {
            repository.validate()?;
        }
        if let Some(runtime) = &self.runtime {
            runtime.validate()?;
        }
        if let (Some(repository), Some(runtime)) = (&self.repository, &self.runtime)
            && repository.revision != runtime.source_revision
        {
            return Err(Error::Invalid(
                "The selected source differs from the runtime source.",
            ));
        }
        Ok(())
    }
}

impl CloudRequest {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if !binding_id(&self.binding) || coder_access::protocol::identity(&self.request).is_err() {
            return Err(Error::Invalid(
                "The native Cloud request reference is invalid.",
            ));
        }
        Ok(())
    }
}

fn validate_conversation(conversation: &Conversation) -> Result<(), Error> {
    validate_address(&conversation.owner, &conversation.id)?;
    if conversation.revision == 0 {
        return Err(Error::Invalid("The conversation revision is invalid."));
    }
    if let Some(selection) = &conversation.selection {
        selection.validate()?;
    }
    if conversation
        .project
        .as_deref()
        .is_some_and(|project| !oa_auth::repos::project_id(project))
    {
        return Err(Error::Invalid("The chat's project is invalid."));
    }
    if let Some(terminal) = &conversation.terminal
        && (!bounded_text(&terminal.computer, 128)
            || terminal.title.len() > 512
            || terminal.title.chars().any(char::is_control)
            || terminal.session.is_empty()
            || terminal.session.len() > 128
            || !terminal
                .session
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            || !(terminal.digest.is_empty() || terminal.digest.len() == 64)
            || terminal.replies.len() > MAX_WEB_REPLIES
            || terminal.replies.iter().any(|reply| {
                !valid_id(&reply.id)
                    || reply.text.trim().is_empty()
                    || reply.text.len() > MAX_WEB_REPLY_BYTES
            })
            || terminal.reply_ids.len() > MAX_REPLY_IDS
            || !terminal.reply_ids.iter().all(|id| valid_id(id))
            || terminal.continued.len() > MAX_CONTINUED
            || terminal
                .continued
                .iter()
                .any(|message| message.text.len() > MAX_CONTINUED_BYTES)
            || !conversation.requests.is_empty()
            || conversation.pending.is_some())
    {
        return Err(Error::Invalid("This chat could not be opened or saved."));
    }
    if let Some(environment) = &conversation.environment {
        environment.validate()?;
    }
    if conversation.tasks.len() > MAX_TASKS {
        return Err(Error::Invalid("The chat has too many tasks."));
    }
    for task in &conversation.tasks {
        task.validate()?;
    }
    let mut identities = HashSet::new();
    for request in &conversation.requests {
        if !valid_id(&request.id)
            || !identities.insert(&request.id)
            || request.digest.len() != 64
            || !request
                .digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Error::Invalid("The retained message identity is invalid."));
        }
        if let Some(selection) = &request.selection {
            selection.validate()?;
        }
        if let Some(reply) = &request.reply
            && (reply.followups.len() > MAX_REPLY_CHIPS
                || reply.offers.len() > MAX_REPLY_CHIPS
                || reply.plugins.len() > MAX_REPLY_CHIPS
                || reply.plugins.iter().any(|slug| !bounded_text(slug, 64))
                || reply
                    .answer
                    .as_ref()
                    .is_some_and(|answer| !bounded_text(answer, 96))
                || reply
                    .followups
                    .iter()
                    .any(|followup| !bounded_text(&followup.label, 512)))
        {
            return Err(Error::Invalid("This chat could not be opened or saved."));
        }
        if let Some(cloud) = &request.cloud {
            cloud.validate()?;
            if request
                .selection
                .as_ref()
                .and_then(|selection| selection.runtime.as_ref())
                .is_none_or(|runtime| runtime.binding != cloud.binding)
            {
                return Err(Error::Invalid(
                    "The native Cloud request changed its selected runtime.",
                ));
            }
        }
    }
    Ok(())
}

fn validate_retained_requests(previous: &Conversation, next: &Conversation) -> Result<(), Error> {
    for request in &previous.requests {
        let retained = next
            .requests
            .iter()
            .find(|retained| retained.id == request.id);
        if retained.is_none_or(|retained| {
            retained.digest != request.digest
                || retained.selection != request.selection
                || request
                    .cloud
                    .as_ref()
                    .is_some_and(|cloud| retained.cloud.as_ref() != Some(cloud))
        }) {
            return Err(Error::Invalid(
                "An accepted message cannot change its source or native request.",
            ));
        }
    }
    Ok(())
}

fn binding_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
}

fn bounded_text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}

fn commit(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

async fn blocking<T, F>(operation: F) -> Result<T, Error>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, Error> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|_| Error::Unavailable("Conversation storage stopped unexpectedly."))?
}

fn path(root: &Path, owner: &str, id: &str) -> PathBuf {
    root.join(owner_digest(owner)).join(format!("{id}.json"))
}

fn read_disk(path: &Path, owner: &str, id: &str) -> Result<Option<Loaded>, Error> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Unavailable("The conversation could not be read.")),
    };
    let mut bytes = Vec::new();
    file.take((MAX_RECORD_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Unavailable("The conversation could not be read."))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(Error::Corrupt(
            "The retained conversation exceeds its size limit.",
        ));
    }
    Ok(Some(Loaded {
        conversation: decode(&bytes, owner, id)?,
        generation: digest(&bytes),
    }))
}

fn write_disk(
    path: &Path,
    owner: &str,
    id: &str,
    bytes: &[u8],
    previous: Option<&str>,
) -> Result<String, Error> {
    let directory = path
        .parent()
        .ok_or(Error::Invalid("The chat directory is invalid."))?;
    create_directory(directory)?;
    let _lock = lock(&path.with_extension("lock"))?;
    let current = read_disk(path, owner, id)?;
    match (previous, current.as_ref()) {
        (None, None) => {}
        (Some(expected), Some(loaded)) if expected == loaded.generation => {}
        _ => return Err(Error::Conflict),
    }
    atomic_write(path, bytes)
}

fn delete_disk(path: &Path, owner: &str, id: &str, expected: &str) -> Result<bool, Error> {
    let directory = path
        .parent()
        .ok_or(Error::Invalid("The chat directory is invalid."))?;
    if !directory.exists() {
        return Ok(false);
    }
    let lock_path = path.with_extension("lock");
    let _lock = lock(&lock_path)?;
    let Some(current) = read_disk(path, owner, id)? else {
        let _ = fs::remove_file(&lock_path);
        return Ok(false);
    };
    if current.generation != expected {
        return Err(Error::Conflict);
    }
    fs::remove_file(path)
        .map_err(|_| Error::Unavailable("The chat delete has an unknown outcome."))?;
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|_| Error::Unavailable("The chat delete has an unknown outcome."))?;
    // Only a create with this same random ID could wait on the old lock.
    let _ = fs::remove_file(&lock_path);
    Ok(true)
}

/// One pass over the disk store; see [`Store::expire_untouched`].
fn expire_disk(root: &Path, cutoff_unix: u64) -> Result<usize, Error> {
    let owners = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(_) => return Err(Error::Unavailable("The chat store could not be read.")),
    };
    let mut removed = 0;
    for directory in owners {
        let directory =
            directory.map_err(|_| Error::Unavailable("The chat store could not be read."))?;
        let folder = directory.file_name();
        let Some(folder) = folder.to_str() else {
            continue;
        };
        if folder.len() != 64 || !folder.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let Ok(entries) = fs::read_dir(directory.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(id) = name.to_str().and_then(|name| name.strip_suffix(".json")) else {
                continue;
            };
            if !valid_id(id) {
                continue;
            }
            let path = entry.path();
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            if bytes.len() > MAX_RECORD_BYTES {
                continue;
            }
            let Ok(record) = serde_json::from_slice::<Record>(&bytes) else {
                continue;
            };
            let owner = &record.conversation.owner;
            if record.conversation.id != id
                || validate_owner(owner).is_err()
                || owner_digest(owner) != folder
                || record.conversation.updated_unix >= cutoff_unix
            {
                continue;
            }
            match delete_disk(&path, owner, id, &digest(&bytes)) {
                Ok(true) => removed += 1,
                // Gone already, or written again since it was read.
                Ok(false) | Err(Error::Conflict) => {}
                Err(error) => return Err(error),
            }
        }
    }
    Ok(removed)
}

/// A chat record under `prefix` (`{prefix}{owner digest}/{id}.json`), not
/// an answer lease or anything else that shares the bucket.
fn expirable_object(prefix: &str, name: &str) -> bool {
    let Some((digest, file)) = name
        .strip_prefix(prefix)
        .and_then(|rest| rest.split_once('/'))
    else {
        return false;
    };
    digest.len() == 64
        && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        && file.strip_suffix(".json").is_some_and(valid_id)
}

/// Seconds since 1970 for a Cloud Storage `updated` time
/// (`2026-10-08T12:34:56.789Z`). Anything else is `None`.
fn rfc3339_unix(value: &str) -> Option<u64> {
    let bytes = value.as_bytes();
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || !value.ends_with('Z')
    {
        return None;
    }
    let number = |range: std::ops::Range<usize>| -> Option<u64> {
        let part = value.get(range)?;
        part.bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    let rest = &value[19..value.len() - 1];
    if !(rest.is_empty()
        || (rest.starts_with('.')
            && rest.len() > 1
            && rest[1..].bytes().all(|b| b.is_ascii_digit())))
    {
        return None;
    }
    if year < 1970
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm), March-based.
    let (y, m) = if month <= 2 {
        (year - 1, month + 9)
    } else {
        (year, month - 3)
    };
    let era = y / 400;
    let year_of_era = y - era * 400;
    let day_of_year = (153 * m + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second)
}

fn create_directory(directory: &Path) -> Result<(), Error> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(directory)
        .map_err(|_| Error::Unavailable("The chat directory could not be created."))
}

fn read_active_disk(path: &Path) -> Result<Option<Active>, Error> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Unavailable("The answer lease could not be read.")),
    };
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Unavailable("The answer lease could not be read."))?;
    if bytes.len() > 4096 {
        return Err(Error::Corrupt(
            "The retained answer lease exceeds its size limit.",
        ));
    }
    Ok(Some(decode_active(&bytes)?))
}

fn read_computers_disk(path: &Path) -> Result<Computers, Error> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Computers::default());
        }
        Err(_) => return Err(Error::Unavailable("The computer list could not be read.")),
    };
    let mut bytes = Vec::new();
    file.take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Unavailable("The computer list could not be read."))?;
    if bytes.len() > 64 * 1024 {
        return Err(Error::Corrupt("The computer list exceeds its size limit."));
    }
    decode_computers(&bytes)
}

fn active_bytes(active: &Active) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(active).map_err(|_| Error::Invalid("The answer lease is invalid."))
}

fn decode_active(bytes: &[u8]) -> Result<Active, Error> {
    let active: Active = serde_json::from_slice(bytes)
        .map_err(|_| Error::Corrupt("The retained answer lease is invalid."))?;
    if active.schema != format!("{SCHEMA}.active")
        || (!active.request_id.is_empty() && !valid_id(&active.request_id))
        || (active.request_id.is_empty() && active.expires_unix != 0)
    {
        return Err(Error::Corrupt(
            "The retained answer lease identity is invalid.",
        ));
    }
    Ok(active)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<String, Error> {
    let directory = path
        .parent()
        .ok_or(Error::Invalid("The chat directory is invalid."))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(Error::Invalid("The chat file name is invalid."))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Unavailable("The system clock is unavailable."))?
        .as_nanos();
    let temporary = directory.join(format!(".{name}.{}.{stamp}.tmp", std::process::id()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|_| Error::Unavailable("The conversation could not be staged."))?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| Error::Unavailable("The conversation could not be retained."))?;
        fs::rename(&temporary, path)
            .map_err(|_| Error::Unavailable("The conversation write has an unknown outcome."))?;
        File::open(directory)
            .and_then(|file| file.sync_all())
            .map_err(|_| Error::Unavailable("The conversation write has an unknown outcome."))?;
        Ok(digest(bytes))
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

/// The kernel releases this lock if a process exits before writing a record.
#[cfg(unix)]
fn lock(path: &Path) -> Result<File, Error> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
        .map_err(|_| Error::Unavailable("The conversation lock could not be opened."))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // SAFETY: The descriptor belongs to the retained File and remains open.
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result == 0 {
            return Ok(file);
        }
        let error = std::io::Error::last_os_error();
        if !matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
        ) || Instant::now() >= deadline
        {
            return Err(Error::Unavailable(
                "The conversation lock could not be acquired.",
            ));
        }
        // This wait runs only in spawn_blocking, never on an async executor.
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(not(unix))]
fn lock(_path: &Path) -> Result<File, Error> {
    Err(Error::Unavailable(
        "Local conversation storage requires operating-system file locks.",
    ))
}

fn validate_address(owner: &str, id: &str) -> Result<(), Error> {
    validate_owner(owner)?;
    if !valid_id(id) {
        return Err(Error::Invalid("The conversation ID is invalid."));
    }
    Ok(())
}

/// An owner is a browser's visitor cookie (32 hex characters) or a
/// signed-in account ([`account_owner`]). The two shapes never overlap, so a
/// cookie can never name an account's chats.
fn validate_owner(owner: &str) -> Result<(), Error> {
    let visitor = owner.len() == 32 && owner.bytes().all(|byte| byte.is_ascii_hexdigit());
    let account = owner.strip_prefix(ACCOUNT_OWNER).is_some_and(|rest| {
        rest.len() == 64
            && rest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    });
    if !visitor && !account {
        return Err(Error::Invalid("The visitor identity is invalid."));
    }
    Ok(())
}

/// The prefix of an account's owner value.
const ACCOUNT_OWNER: &str = "account:";

/// The owner value for a signed-in account's chats: a digest of the account
/// id, so storage never holds the id itself, in a shape no visitor cookie
/// can take.
pub(crate) fn account_owner(account_id: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"openagents.web.chat.account.v1\0");
    hash.update(account_id.as_bytes());
    format!("{ACCOUNT_OWNER}{:x}", hash.finalize())
}

/// Whether `owner` is a signed-in account's, not a browser's.
pub(crate) fn is_account_owner(owner: &str) -> bool {
    owner.starts_with(ACCOUNT_OWNER) && validate_owner(owner).is_ok()
}

fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn owner_digest(owner: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"openagents.web.chat.owner.v1\0");
    hash.update(owner.as_bytes());
    format!("{:x}", hash.finalize())
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// The longest chat retention a server accepts, in days (ten years).
pub const MAX_RETENTION_DAYS: u64 = 3650;

/// How often a server with a retention removes untouched chats.
const EXPIRY_INTERVAL: Duration = Duration::from_secs(6 * 3600);

/// Parse `--chat-retention-days` (1 to [`MAX_RETENTION_DAYS`]).
pub fn retention_days(value: &str) -> Result<u64, Error> {
    value
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|days| (1..=MAX_RETENTION_DAYS).contains(days))
        .ok_or(Error::Invalid(
            "The chat retention must be a whole number of days from 1 to 3650.",
        ))
}

/// The time before which a chat counts as untouched for `days`.
pub fn retention_cutoff(now_unix: u64, days: u64) -> u64 {
    now_unix.saturating_sub(days.saturating_mul(86_400))
}

/// Remove chats untouched for `days` now and every six hours after, for
/// the life of the server. A failed pass is logged and tried again later.
pub fn spawn_expiry(store: Arc<Store>, days: u64) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(EXPIRY_INTERVAL);
        loop {
            ticks.tick().await;
            match store
                .expire_untouched(retention_cutoff(now_unix(), days))
                .await
            {
                Ok(0) => {}
                Ok(removed) => {
                    println!("openagents-web: removed {removed} chats untouched for {days} days");
                }
                Err(error) => eprintln!("openagents-web: chat expiry: {error}"),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const OWNER: &str = "11111111111111111111111111111111";
    const ID: &str = "83e18906-00e2-436c-978b-13a4932f58b0";

    fn selection() -> Selection {
        Selection {
            revision: 1,
            repository: Some(RepositorySource {
                repository: "OpenAgentsInc/openagents".into(),
                branch: "main".into(),
                revision: "a".repeat(40),
            }),
            runtime: Some(RuntimeSelection {
                binding: "operator-fixture".into(),
                account: "account-fixture".into(),
                workspace: "workspace-fixture".into(),
                members_epoch: 7,
                project: "openagents".into(),
                profile: "boat-codex".into(),
                profile_revision: format!("sha256:{}", "b".repeat(64)),
                source_revision: "a".repeat(40),
                source_digest: format!("sha256:{}", "c".repeat(64)),
                placement: "boat".into(),
                executor: "codex".into(),
                model: Some("gpt-6.1-sol".into()),
                max_timeout_seconds: 300,
            }),
        }
    }

    fn conversation() -> Conversation {
        Conversation {
            id: ID.into(),
            owner: OWNER.into(),
            revision: 1,
            title: "Prepare the repository".into(),
            messages: vec![Message {
                role: Role::User,
                text: "Prepare the repository".into(),
                request_id: Some(ID.into()),
            }],
            pending: None,
            requests: vec![Request {
                id: ID.into(),
                digest: digest(b"Prepare the repository"),
                outcome: Outcome::Answered,
                selection: None,
                cloud: None,
                reply: None,
            }],
            selection: None,
            updated_unix: 1,
            pinned_unix: None,
            archived_unix: None,
            project: None,
            terminal: None,
            environment: None,
            tasks: Vec::new(),
            opened_unix: None,
        }
    }

    #[test]
    fn records_without_selection_fields_remain_readable() {
        let bytes = serde_json::to_vec(&json!({
            "schema": SCHEMA,
            "conversation": {
                "id": ID,
                "owner": OWNER,
                "revision": 1,
                "title": "Existing chat",
                "messages": [],
                "pending": null,
                "requests": [{
                    "id": ID,
                    "digest": "a".repeat(64),
                    "outcome": "answered"
                }],
                "updated_unix": 1
            }
        }))
        .unwrap();
        let retained = decode(&bytes, OWNER, ID).unwrap();
        assert!(retained.selection.is_none());
        assert!(retained.requests[0].selection.is_none());
        assert!(retained.requests[0].cloud.is_none());
        let encoded: serde_json::Value =
            serde_json::from_slice(&encode(&retained).unwrap()).unwrap();
        let record = encoded["conversation"].as_object().unwrap();
        assert!(!record.contains_key("selection"));
        assert!(retained.pinned_unix.is_none() && retained.archived_unix.is_none());
        assert!(!record.contains_key("pinned_unix") && !record.contains_key("archived_unix"));
        let request = encoded["conversation"]["requests"][0].as_object().unwrap();
        assert!(!request.contains_key("selection"));
        assert!(!request.contains_key("cloud"));
    }

    #[test]
    fn pins_and_archives_round_trip() {
        let mut record = conversation();
        record.pinned_unix = Some(7);
        record.archived_unix = Some(9);
        let retained = decode(&encode(&record).unwrap(), OWNER, ID).unwrap();
        assert_eq!(retained.pinned_unix, Some(7));
        assert_eq!(retained.archived_unix, Some(9));
    }

    #[test]
    fn environments_and_tasks_round_trip_and_validate() {
        let mut record = conversation();
        let plain = String::from_utf8(encode(&record).unwrap()).unwrap();
        assert!(!plain.contains("\"environment\"") && !plain.contains("\"tasks\""));
        record.environment = Some(ChatEnvironment {
            id: "env-1".into(),
            repository: "acme/app".into(),
            version: Some(3),
            removed: false,
        });
        record.tasks = vec![ChatTask {
            id: "claude-env-1-1".into(),
            kind: TaskKind::Claude,
            environment: "env-1".into(),
            title: "Fix the login".into(),
            state: TaskState::Working,
            started_unix: 5,
            after_message: 2,
            version: Some(3),
            finished_unix: None,
        }];
        let bytes = encode(&record).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.contains(r#""state":"working""#) && !text.contains("removed"));
        let retained = decode(&bytes, OWNER, ID).unwrap();
        assert_eq!(retained.environment, record.environment);
        assert_eq!(retained.tasks, record.tasks);
        assert!(TaskState::Stopped.finished() && !TaskState::Paused.finished());
        let mut bad = record.clone();
        bad.tasks[0].environment = "../x".into();
        assert!(validate_conversation(&bad).is_err());
        let mut bad = record.clone();
        bad.environment.as_mut().unwrap().repository = String::new();
        assert!(validate_conversation(&bad).is_err());
        let mut many = record;
        many.tasks = vec![many.tasks[0].clone(); MAX_TASKS + 1];
        assert!(validate_conversation(&many).is_err());
    }

    #[test]
    fn source_and_native_request_round_trip_without_credentials() {
        let mut record = conversation();
        record.selection = Some(selection());
        record.requests[0].selection = record.selection.clone();
        record.requests[0].cloud = Some(CloudRequest {
            binding: "operator-fixture".into(),
            request: "d".repeat(64),
        });
        let bytes = encode(&record).unwrap();
        let retained = decode(&bytes, OWNER, ID).unwrap();
        assert_eq!(retained.selection, record.selection);
        assert_eq!(retained.requests[0].selection, record.requests[0].selection);
        assert_eq!(retained.requests[0].cloud, record.requests[0].cloud);
        let mut value = serde_json::to_value(selection()).unwrap();
        value["runtime"]["credentials"] = json!({"OPENAI_API_KEY": "not-a-key"});
        assert!(serde_json::from_value::<Selection>(value).is_err());
    }

    #[test]
    fn selections_validate_addresses_pins_and_bounds() {
        assert!(Selection::default().validate().is_ok());
        assert!(selection().validate().is_ok());
        let mut invalid = selection();
        invalid.revision = 0;
        assert!(invalid.validate().is_err());
        let mut invalid = selection();
        invalid.repository.as_mut().unwrap().repository = "../openagents".into();
        assert!(invalid.validate().is_err());
        let mut invalid = selection();
        invalid.repository.as_mut().unwrap().branch = "main..other".into();
        assert!(invalid.validate().is_err());
        let mut invalid = selection();
        invalid.repository.as_mut().unwrap().revision = "b".repeat(40);
        assert!(invalid.validate().is_err());
        let mut invalid = selection();
        invalid.runtime.as_mut().unwrap().binding = "../host".into();
        assert!(invalid.validate().is_err());
        let mut invalid = selection();
        invalid.runtime.as_mut().unwrap().account = "account\nother".into();
        assert!(invalid.validate().is_err());
        let mut invalid = selection();
        invalid.runtime.as_mut().unwrap().profile_revision = "b".repeat(64);
        assert!(invalid.validate().is_err());
        let mut invalid = selection();
        invalid.runtime.as_mut().unwrap().members_epoch = 9_007_199_254_740_992;
        assert!(invalid.validate().is_err());
        let mut invalid = selection();
        invalid.runtime.as_mut().unwrap().max_timeout_seconds = 43_201;
        assert!(invalid.validate().is_err());
        let mut invalid = selection();
        invalid.runtime.as_mut().unwrap().model = Some("m".repeat(129));
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn native_request_requires_the_frozen_runtime_binding() {
        let mut record = conversation();
        record.requests[0].cloud = Some(CloudRequest {
            binding: "operator-fixture".into(),
            request: "d".repeat(64),
        });
        assert!(encode(&record).is_err());
        record.requests[0].selection = Some(selection());
        assert!(encode(&record).is_ok());
        record.requests[0].cloud.as_mut().unwrap().binding = "another-host".into();
        assert!(encode(&record).is_err());
        record.requests[0].cloud.as_mut().unwrap().binding = "operator-fixture".into();
        record.requests[0].cloud.as_mut().unwrap().request = ID.into();
        assert!(encode(&record).is_err());
    }

    #[tokio::test]
    async fn writes_announce_their_owner_and_chat() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::local(directory.path().join("chats"));
        let mut changes = store.changes();
        let record = conversation();
        let created = store.create(&record).await.unwrap();
        let change = changes.try_recv().unwrap();
        assert_eq!(&*change.owner, record.owner.as_str());
        assert_eq!(&*change.id, record.id.as_str());
        let mut next = record.clone();
        next.revision += 1;
        store.compare_and_swap(&created, &next).await.unwrap();
        assert_eq!(changes.try_recv().unwrap(), change);
        // A refused write announces nothing.
        assert!(store.compare_and_swap(&created, &next).await.is_err());
        assert!(changes.try_recv().is_err());
    }

    #[tokio::test]
    async fn local_cas_freezes_accepted_source_and_native_request() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::local(directory.path().join("chats"));
        let mut record = conversation();
        record.selection = Some(selection());
        record.requests[0].selection = record.selection.clone();
        let original = store.create(&record).await.unwrap();

        let mut next = record.clone();
        next.revision += 1;
        next.selection.as_mut().unwrap().revision += 1;
        next.selection
            .as_mut()
            .unwrap()
            .repository
            .as_mut()
            .unwrap()
            .branch = "next".into();
        let changed = store.compare_and_swap(&original, &next).await.unwrap();
        assert_eq!(changed.conversation.requests[0].selection, record.selection);
        assert_eq!(
            store
                .load(OWNER, ID)
                .await
                .unwrap()
                .unwrap()
                .conversation
                .selection,
            next.selection
        );
        assert!(matches!(
            store.compare_and_swap(&original, &next).await,
            Err(Error::Conflict)
        ));

        let mut rebound = next.clone();
        rebound.revision += 1;
        rebound.requests[0].selection = rebound.selection.clone();
        assert!(matches!(
            store.compare_and_swap(&changed, &rebound).await,
            Err(Error::Invalid(_))
        ));

        let mut staged = next.clone();
        staged.revision += 1;
        staged.requests[0].cloud = Some(CloudRequest {
            binding: "operator-fixture".into(),
            request: "d".repeat(64),
        });
        let accepted = store.compare_and_swap(&changed, &staged).await.unwrap();
        let mut cleared = staged.clone();
        cleared.revision += 1;
        cleared.requests[0].cloud = None;
        assert!(matches!(
            store.compare_and_swap(&accepted, &cleared).await,
            Err(Error::Invalid(_))
        ));
        let mut redirected = staged;
        redirected.revision += 1;
        redirected.requests[0].cloud.as_mut().unwrap().request = "e".repeat(64);
        assert!(matches!(
            store.compare_and_swap(&accepted, &redirected).await,
            Err(Error::Invalid(_))
        ));
    }

    #[tokio::test]
    async fn local_delete_is_fenced_final_and_private() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::local(directory.path().join("chats"));
        let original = store.create(&conversation()).await.unwrap();
        let mut next = conversation();
        next.revision = 2;
        let changed = store.compare_and_swap(&original, &next).await.unwrap();

        // A stale read cannot delete a chat that changed since.
        assert!(matches!(
            store.delete(OWNER, ID, &original.generation).await,
            Err(Error::Conflict)
        ));
        // Another visitor's address finds nothing to delete.
        let other = "22222222222222222222222222222222";
        assert!(!store.delete(other, ID, &changed.generation).await.unwrap());
        assert!(store.load(OWNER, ID).await.unwrap().is_some());

        assert!(store.delete(OWNER, ID, &changed.generation).await.unwrap());
        assert!(store.load(OWNER, ID).await.unwrap().is_none());
        assert!(store.list(OWNER).await.unwrap().is_empty());
        assert!(!store.delete(OWNER, ID, &changed.generation).await.unwrap());
        // Nothing of the chat stays on disk.
        let files: Vec<_> = fs::read_dir(directory.path().join("chats").join(owner_digest(OWNER)))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert!(files.is_empty(), "{files:?}");
        // A late answer cannot bring it back.
        let mut late = next.clone();
        late.revision = 3;
        assert!(matches!(
            store.compare_and_swap(&changed, &late).await,
            Err(Error::Conflict)
        ));
        assert!(store.load(OWNER, ID).await.unwrap().is_none());
    }

    #[test]
    fn an_account_owner_is_a_shape_no_cookie_can_take() {
        let owner = account_owner("acct_123");
        assert_eq!(owner, account_owner("acct_123"));
        assert_ne!(owner, account_owner("acct_124"));
        assert!(validate_owner(&owner).is_ok());
        assert!(is_account_owner(&owner));
        assert!(!is_account_owner(OWNER));
        assert!(validate_owner(OWNER).is_ok());
        for bad in [
            "account:",
            "account:acct_123",
            "account:ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
            "1111111111111111111111111111111",
            "../../11111111111111111111111111",
        ] {
            assert!(validate_owner(bad).is_err(), "{bad}");
        }
        // The visitor cookie reader only takes 32 hex characters.
        assert_ne!(owner.len(), 32);
    }

    #[tokio::test]
    async fn local_adopt_moves_a_chat_once_and_never_over_another() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::local(directory.path().join("chats"));
        let account = account_owner("acct_1");
        let original = store.create(&conversation()).await.unwrap();
        let mut next = conversation();
        next.revision = 2;
        next.title = "Renamed".into();
        let changed = store.compare_and_swap(&original, &next).await.unwrap();

        // A stale read moves nothing, and leaves no copy behind.
        assert!(!store.adopt(&original, &account).await.unwrap());
        assert!(store.load(&account, ID).await.unwrap().is_none());
        assert!(store.load(OWNER, ID).await.unwrap().is_some());

        assert!(store.adopt(&changed, &account).await.unwrap());
        assert!(store.load(OWNER, ID).await.unwrap().is_none());
        let moved = store.load(&account, ID).await.unwrap().unwrap();
        assert_eq!(moved.conversation.owner, account);
        assert_eq!(moved.conversation.title, "Renamed");
        assert_eq!(moved.conversation.revision, 2);
        assert_eq!(store.list(&account).await.unwrap().len(), 1);
        assert!(store.list(OWNER).await.unwrap().is_empty());
        // The moved chat keeps working under its new owner.
        let mut later = moved.conversation.clone();
        later.revision = 3;
        store.compare_and_swap(&moved, &later).await.unwrap();

        // A chat with the same id already on the account is never replaced.
        let again = store.create(&conversation()).await.unwrap();
        assert!(!store.adopt(&again, &account).await.unwrap());
        assert_eq!(
            store
                .load(&account, ID)
                .await
                .unwrap()
                .unwrap()
                .conversation
                .revision,
            3
        );
        assert!(store.load(OWNER, ID).await.unwrap().is_some());
        assert!(matches!(
            store.adopt(&again, OWNER).await,
            Err(Error::Invalid(_))
        ));
    }

    #[tokio::test]
    async fn local_expiry_removes_only_untouched_chats() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::local(directory.path().join("chats"));
        assert_eq!(store.expire_untouched(100).await.unwrap(), 0);
        let mut old = conversation();
        old.updated_unix = 50;
        store.create(&old).await.unwrap();
        let fresh_id = "93e18906-00e2-436c-978b-13a4932f58b0";
        let mut fresh = conversation();
        fresh.id = fresh_id.into();
        fresh.requests[0].id = fresh_id.into();
        fresh.messages[0].request_id = Some(fresh_id.into());
        fresh.updated_unix = 150;
        store.create(&fresh).await.unwrap();
        let other = "33333333333333333333333333333333";
        let mut theirs = conversation();
        theirs.owner = other.into();
        theirs.updated_unix = 10;
        store.create(&theirs).await.unwrap();

        assert_eq!(store.expire_untouched(100).await.unwrap(), 2);
        assert!(store.load(OWNER, ID).await.unwrap().is_none());
        assert!(store.load(other, ID).await.unwrap().is_none());
        assert!(store.load(OWNER, fresh_id).await.unwrap().is_some());
        assert_eq!(store.expire_untouched(100).await.unwrap(), 0);
    }

    #[test]
    fn retention_settings_and_bucket_names_are_bounded() {
        assert_eq!(retention_days("30").unwrap(), 30);
        assert_eq!(retention_days(" 3650 ").unwrap(), 3650);
        for bad in ["0", "3651", "-1", "30d", ""] {
            assert!(retention_days(bad).is_err(), "{bad}");
        }
        assert_eq!(retention_cutoff(10 * 86_400, 3), 7 * 86_400);
        assert_eq!(retention_cutoff(5, 3), 0);

        let digest = owner_digest(OWNER);
        assert!(expirable_object(
            "conversations/",
            &format!("conversations/{digest}/{ID}.json")
        ));
        for name in [
            format!("conversations/{digest}/.active.json"),
            format!("conversations/{digest}/{ID}.json.tmp"),
            format!("other/{digest}/{ID}.json"),
            format!("conversations/{ID}.json"),
            format!("conversations/{digest}/nested/{ID}.json"),
        ] {
            assert!(!expirable_object("conversations/", &name), "{name}");
        }
    }

    #[test]
    fn storage_times_parse_to_unix_seconds() {
        assert_eq!(rfc3339_unix("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(rfc3339_unix("1970-01-01T00:00:00.000Z"), Some(0));
        assert_eq!(rfc3339_unix("2000-03-01T00:00:00Z"), Some(951_868_800));
        assert_eq!(
            rfc3339_unix("2026-10-08T12:34:56.789Z"),
            Some(1_791_462_896)
        );
        for bad in [
            "2026-10-08",
            "2026-10-08T12:34:56+00:00",
            "2026-13-08T12:34:56Z",
            "2026-10-08T12:34:56.Z",
            "abcd-10-08T12:34:56Z",
        ] {
            assert_eq!(rfc3339_unix(bad), None, "{bad}");
        }
    }
}
