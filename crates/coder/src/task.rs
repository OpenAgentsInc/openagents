//! A local, durable inbox for requested work.
//!
//! Enqueueing records intent. It grants no execution authority, starts no
//! executor, and makes no claim about checks or results. This local command
//! format uses exact-byte retry identity; it is not a Nostr session protocol.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use nostr::contracts::{digest_bytes, parse_strict_bounded};
use serde::{Deserialize, Serialize};

/// The local command format this implementation accepts.
pub const COMMAND_SCHEMA: &str = "openagents.coder.task-command.v1";
/// The local receipt format this implementation returns.
pub const RECEIPT_SCHEMA: &str = "openagents.coder.task-receipt.v1";
/// The persisted inbox format: one file per task beside a marker, an
/// identity log, and the stable lock (#10231).
pub const STORE_SCHEMA: &str = "openagents.coder.task-store.v3";
/// The single-document format a v3 store migrates from.
pub const LEGACY_STORE_SCHEMA: &str = "openagents.coder.task-store.v2";
const LEGACY_V1_SCHEMA: &str = "openagents.coder.task-store.v1";
/// One task's file: its state and its own command and owner journal.
pub const TASK_FILE_SCHEMA: &str = "openagents.coder.task-file.v1";

pub mod adapter;
pub mod agent;
mod agent_asked;
pub mod agent_consolidate;
pub mod agent_crew;
pub mod agent_crew_control;
pub mod agent_engrams;
pub mod agent_git_sign;
pub mod agent_hiring;
pub mod agent_host;
pub mod agent_interview;
pub mod agent_jobs;
pub mod agent_key;
pub mod agent_lifecycle;
pub mod agent_memory;
pub mod agent_place;
pub mod agent_plan;
pub mod agent_preset;
pub mod agent_profile;
pub mod agent_queue;
pub mod agent_recall;
pub mod agent_reflect;
pub mod agent_remote;
pub mod agent_route;
pub mod agent_share;
pub mod agent_spend;
pub mod agent_steer;
pub mod agent_sync;
pub mod archive;
pub mod artifact;
pub mod autostart;
pub mod coder_v1;
/// Which login each engine is signed in as, as a salted fingerprint
/// (#10105): `microcoder_loop::account`, so readings and holds follow the
/// login they were about.
pub use microcoder_loop::account;
pub mod apply;
pub mod capacity;
pub mod chat_client;
pub mod checks;
pub mod cli;
pub mod commands;
pub mod freshen;
pub mod interaction;
pub mod issue_pick;
pub mod issue_run;
pub mod land_queue;
pub mod landing;
pub mod lifecycle;
pub mod local;
pub mod local_checks;
pub mod media;
pub mod owner;
pub mod publish;
pub mod recent;
pub mod remote;
pub mod resume;
pub mod retire;
pub mod review;
pub mod run_artifacts;
pub mod sales;
pub mod settings;
pub mod shadow;
pub mod spare;
pub mod steer;
pub mod studio;
pub mod studio_sim;
pub mod targets;
pub use targets::facts as background_facts;
pub mod usage;
pub mod view;
pub mod worktree_hooks;
pub mod worktrees;
/// Per-engine steering semantics; see [`coder_delegate::steering`].
pub use coder_delegate::steering;
/// The most turns one task can take. A follow-up past it is refused.
pub const MAX_TURNS: usize = 64;
/// The largest command, including JSON whitespace, in bytes.
pub const MAX_COMMAND_BYTES: usize = 64 * 1024;
/// The largest task file (and the largest legacy document), in bytes.
pub const MAX_STORE_BYTES: usize = 16 * 1024 * 1024;
/// The largest number of retained tasks. No task is silently pruned.
pub const MAX_TASKS: usize = 1024;
/// The largest number of accepted commands. Retry identities are not pruned.
pub const MAX_COMMANDS: usize = 2048;
/// The store's marker within its private directory. Its presence means the
/// store is initialized; it is never recreated once missing.
pub const STORE_FILE: &str = "store.json";
/// The single document a store before #10231 kept everything in. A v3
/// store migrates it once and keeps it as [`LEGACY_BACKUP_FILE`].
pub const LEGACY_STORE_FILE: &str = "tasks.json";
/// Where a migrated [`LEGACY_STORE_FILE`] is kept, untouched.
pub const LEGACY_BACKUP_FILE: &str = "tasks.v2.json";
/// The stable sibling lock. Removing it while a process runs is unsafe. It
/// is held only for moments: to initialize or migrate the store and to
/// reserve a command identity.
pub const LOCK_FILE: &str = "tasks.lock";
/// The private directory of task files (`<id>.json`) and their write locks
/// (`<id>.lock`).
pub const TASK_DIR: &str = "task";
/// The append-only log of accepted command identities, one JSON line each.
pub const IDENTITY_FILE: &str = "identities.log";
/// Held while an owner event that reserves a workspace checks every task.
const WORKSPACE_LOCK_FILE: &str = "workspaces.lock";
/// The largest identity log, in bytes.
const MAX_IDENTITY_BYTES: u64 = 4 * 1024 * 1024;
const PENDING_FILE: &str = ".tasks.pending";
const LOCK_WAIT: Duration = Duration::from_secs(5);
/// How long a task's own owner process, which no device waits on, waits out
/// another holder of the store lock before it refuses with [`Error::Busy`]:
/// a store save's disk sync on a nearly full volume can take longer than
/// [`Store::open`]'s five seconds.
pub const OWNER_LOCK_WAIT: Duration = Duration::from_secs(120);
/// How long a reader following a task (`openagents chat follow`, an issue
/// flow waiting for its turn) waits out another process holding the store
/// before it says it cannot read the task: as long as the task's own owner
/// waits, so a slow disk sync that the owner survives never ends the reader.
pub const READER_BUSY_WAIT: Duration = OWNER_LOCK_WAIT;

/// A follower's reads of one task, which wait out a busy store.
///
/// Each read opens the store as [`Store::open`] does. A store another
/// process holds past that open's wait is not a failure while it has been
/// busy for less than [`READER_BUSY_WAIT`] in a row: the read returns
/// `None`, and the follower polls again. Reading never changes the task.
#[derive(Debug)]
pub struct Reading {
    open: Duration,
    limit: Duration,
    since: Option<Instant>,
}

impl Default for Reading {
    fn default() -> Self {
        Self::within(LOCK_WAIT, READER_BUSY_WAIT)
    }
}

impl Reading {
    /// Reads that wait `open` for each open and `limit` in a row for a
    /// busy store.
    #[must_use]
    pub fn within(open: Duration, limit: Duration) -> Self {
        Self {
            open,
            limit,
            since: None,
        }
    }

    /// `task` in the store at `dir`, or `None` while another process has
    /// held the store for less than the limit.
    ///
    /// # Errors
    /// The store cannot be read, holds no such task, or stayed busy past
    /// the limit.
    pub fn show(&mut self, dir: &Path, task: &str) -> Result<Option<Task>, Error> {
        match Store::open_waiting(dir, self.open).and_then(|store| store.show(task)) {
            Ok(task) => {
                self.since = None;
                Ok(Some(task))
            }
            Err(Error::Busy) => {
                let since = *self.since.get_or_insert_with(Instant::now);
                if since.elapsed() < self.limit {
                    Ok(None)
                } else {
                    Err(Error::Busy)
                }
            }
            Err(error) => {
                self.since = None;
                Err(error)
            }
        }
    }
}

/// A requested adapter and model, not an admitted execution configuration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequestedConfiguration {
    pub adapter: String,
    pub model: Option<String>,
}

/// An unverified workspace reference. Admission must resolve it before running.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub path: String,
    pub source_revision: Option<String>,
}

/// User intent, separate from execution policy and observed results.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskIntent {
    pub title: String,
    pub prompt: String,
    pub workspace: Workspace,
    pub configuration: RequestedConfiguration,
    /// Images the person attached, kept in the task store by digest
    /// ([`media`]). The intent's digest, which the execution grant binds,
    /// covers them. Absent means none, so a task without images keeps its
    /// bytes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<coder_host::access::media::ImageRef>,
}

/// User requests accepted by the durable inbox.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Submit {
        intent: TaskIntent,
    },
    Cancel {
        reason: String,
    },
    Correct {
        prompt: String,
        reason: String,
    },
    /// A follow-up: start the next turn of an ended task with a new user
    /// message. The earlier turns stay retained; the task is queued again
    /// and needs a fresh execution grant, exactly like a new task.
    Continue {
        prompt: String,
    },
}

/// A command identity is global to this store, not scoped to a task or action.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub schema: String,
    pub command_id: String,
    pub task_id: String,
    pub expected_revision: Option<u64>,
    pub action: Action,
}

/// Queue state is independent of an executor's progress or outcome.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued,
    Cancelled,
    Running,
    CancelRequested,
    Finished,
    Unknown,
}

/// Observed execution state, independent of verification.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Execution {
    NotStarted,
    Running,
    Finished,
    Failed,
    Stopped,
    Unknown,
}

/// Independent check state, separate from execution and integration.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Checks {
    NotRun,
    Running,
    Passed,
    Failed,
    Unavailable,
    Disputed,
}

/// The current materialized task. Its intent is immutable after submission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub task_id: String,
    pub revision: u64,
    pub intent: TaskIntent,
    pub intent_digest: String,
    pub status: Status,
    pub execution: Execution,
    pub checks: Checks,
    pub cancellation_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corrections: Vec<Correction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<owner::Run>,
    /// Follow-up messages, one per turn after the first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub follow_ups: Vec<FollowUp>,
    /// The runs of earlier turns, oldest first. The current turn's run is
    /// `run`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub earlier: Vec<owner::Run>,
}

/// A user's message that started a later turn of a task.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FollowUp {
    /// The task revision the follow-up created.
    pub revision: u64,
    pub prompt: String,
}

/// A retained replacement instruction. Earlier instructions and effects remain visible.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Correction {
    pub revision: u64,
    pub prompt: String,
    pub reason: String,
}

impl Task {
    /// Whether the task is over for good: finished or cancelled, its checks
    /// not running, and its delegation group clear. Its build directory and
    /// worktree are then disposable (`targets::cleanup`, the background
    /// disk monitor).
    #[must_use]
    pub fn ended(&self) -> bool {
        matches!(self.status, Status::Finished | Status::Cancelled)
            && self.checks != Checks::Running
            && self
                .run
                .as_ref()
                .is_none_or(|run| run.result.as_ref().is_some_and(|result| result.group_clear))
    }

    /// Current user instructions; this does not change a running grant.
    ///
    /// The newest of the last correction and the last follow-up applies:
    /// a follow-up starts a turn with its message, and a correction after
    /// it replaces that message before the turn runs.
    pub fn effective_prompt(&self) -> &str {
        match (self.corrections.last(), self.follow_ups.last()) {
            (Some(correction), Some(follow_up)) if follow_up.revision > correction.revision => {
                &follow_up.prompt
            }
            (Some(correction), _) => &correction.prompt,
            (None, Some(follow_up)) => &follow_up.prompt,
            (None, None) => &self.intent.prompt,
        }
    }

    /// The current turn, from one. Each follow-up starts the next.
    pub fn turn(&self) -> usize {
        self.earlier.len() + 1
    }

    /// The task revision the current turn started at: one, or the
    /// revision its follow-up created.
    pub fn turn_started(&self) -> u64 {
        self.follow_ups
            .last()
            .map_or(1, |follow_up| follow_up.revision)
    }

    /// The trace file of `turn`: `<task>.<turn>.atif.jsonl`.
    pub fn trace_file(&self, turn: usize) -> String {
        format!("{}.{turn}.atif.jsonl", self.task_id)
    }

    /// Corrections routed to this task that no admitted run has read: the
    /// ledger of steers not yet consumed. A run consumes every correction
    /// up to the task revision its admission records.
    pub fn unconsumed_steers(&self) -> Vec<&Correction> {
        let read = self
            .run
            .as_ref()
            .or(self.earlier.last())
            .map_or(0, |run| run.admission.context.task_revision);
        self.corrections
            .iter()
            .filter(|item| item.revision > read)
            .collect()
    }

    fn context_superseded(&self) -> bool {
        self.run.as_ref().is_some_and(|run| {
            self.corrections
                .last()
                .is_some_and(|item| item.revision > run.admission.context.task_revision)
        })
    }
}

/// The trace steps that record each correction a run consumes as it
/// starts, one step per correction, before the adapter's own admission
/// step. `task` is the task as it was before admission; `steering` is the
/// adapter's statement, whose acknowledgment the step names.
pub(crate) fn consumed_steers(
    task: &Task,
    steering: &coder_delegate::steering::Steering,
) -> Vec<atif::Step> {
    task.unconsumed_steers()
        .into_iter()
        .map(|correction| {
            atif::Step::said(
                atif::Source::System,
                "Steer consumed: this turn starts with the corrected instructions.",
            )
            .noting(
                "steer_consumed",
                serde_json::json!({
                    "revision": correction.revision,
                    "reason": correction.reason,
                    "adapter": steering.adapter,
                    "acknowledgment": steering.acknowledgment,
                }),
            )
        })
        .collect()
}

/// The original result of an accepted command, returned again on exact retry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema: String,
    pub command_id: String,
    pub task_id: String,
    pub request_digest: String,
    pub sequence: u64,
    pub revision: u64,
    pub status: Status,
    pub execution: Execution,
    pub checks: Checks,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Accepted {
    request: String,
    receipt: Receipt,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema: String,
    sequence: u64,
    tasks: BTreeMap<String, Task>,
    commands: Vec<Accepted>,
    #[serde(default)]
    host_events: Vec<owner::Record>,
}

/// A closed refusal classification. Messages do not echo command contents.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    InvalidCommand(&'static str),
    /// The workspace's source snapshot cannot admit a run: the observation
    /// was incomplete (which limit, or which file could not be read), or
    /// it differs from the one the grant pinned. The message names a path
    /// inside the workspace, never command contents.
    SourceSnapshot(String),
    UnsupportedSchema,
    Conflict,
    RevisionMismatch,
    NotFound,
    InvalidTransition,
    LimitExceeded,
    Corrupt(&'static str),
    UnsafePath,
    Busy,
    UnsupportedPlatform,
    ReopenRequired,
    /// Another task's run still holds the workspace's tree (#10124): its
    /// owner or a process it recorded is alive, or its checks run.
    WorkspaceBusy,
    /// Builds wait instead of starting with insufficient disk space.
    BuildDiskLow {
        free: u64,
        floor: u64,
    },
}

impl Error {
    /// Stable refusal codes for machine-readable callers.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::InvalidCommand(_) | Self::SourceSnapshot(_) => "invalid_command",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::Conflict => "command_conflict",
            Self::RevisionMismatch => "revision_mismatch",
            Self::NotFound => "not_found",
            Self::InvalidTransition => "invalid_transition",
            Self::LimitExceeded => "limit_exceeded",
            Self::Corrupt(_) => "corrupt_store",
            Self::UnsafePath => "unsafe_path",
            Self::Busy => "store_busy",
            Self::UnsupportedPlatform => "unsupported_platform",
            Self::ReopenRequired => "reopen_required",
            Self::WorkspaceBusy => "workspace_busy",
            Self::BuildDiskLow { .. } => "build_disk_low",
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "task store I/O failed: {error}"),
            Self::InvalidCommand(message) | Self::Corrupt(message) => formatter.write_str(message),
            Self::SourceSnapshot(message) => formatter.write_str(message),
            Self::UnsupportedSchema => formatter.write_str("the task schema is not supported"),
            Self::Conflict => {
                formatter.write_str("the command identity already names different bytes")
            }
            Self::RevisionMismatch => {
                formatter.write_str("the expected task revision does not match")
            }
            Self::NotFound => formatter.write_str("the task does not exist"),
            Self::InvalidTransition => formatter.write_str("the task cannot make this transition"),
            Self::LimitExceeded => formatter.write_str("the task inbox capacity is exhausted"),
            Self::UnsafePath => formatter
                .write_str("the task store requires private regular files and a real directory"),
            Self::Busy => formatter
                .write_str("another process holds the task store lock; retry after it exits"),
            Self::UnsupportedPlatform => {
                formatter.write_str("the task store requires Unix filesystem protections")
            }
            Self::ReopenRequired => formatter
                .write_str("a write failed; reopen the store before retrying the exact command"),
            Self::BuildDiskLow { free, floor } => write!(
                formatter,
                "builds are waiting for disk space: {free} bytes free, {floor} required"
            ),
            Self::WorkspaceBusy => {
                formatter.write_str("another Coder task is still running in this project")
            }
        }
    }
}

impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Validate the complete, closed command before a caller opens a store.
pub fn parse_command(bytes: &[u8]) -> Result<Command, Error> {
    if bytes.len() > MAX_COMMAND_BYTES {
        return Err(Error::LimitExceeded);
    }
    let value = parse_strict_bounded(bytes, MAX_COMMAND_BYTES).map_err(|_| {
        Error::InvalidCommand("the command must be strict JSON without duplicate keys")
    })?;
    let command: Command = serde_json::from_value(value)
        .map_err(|_| Error::InvalidCommand("the command does not match the closed task schema"))?;
    if command.schema != COMMAND_SCHEMA {
        return Err(Error::UnsupportedSchema);
    }
    if !identifier(&command.command_id, false)
        || !identifier(&command.task_id, false)
        || command.command_id == command.task_id
    {
        return Err(Error::InvalidCommand(
            "command and task identities must be distinct bounded identifiers",
        ));
    }
    match &command.action {
        Action::Submit { intent } => {
            if command.expected_revision.is_some() {
                return Err(Error::InvalidCommand(
                    "submission requires a null expected revision",
                ));
            }
            validate_intent(intent)?;
        }
        Action::Correct { prompt, reason } => {
            if command.expected_revision.is_none()
                || !text(prompt, 32 * 1024, true)
                || !text(reason, 2048, true)
            {
                return Err(Error::InvalidCommand(
                    "correction requires a revision, prompt, and reason",
                ));
            }
        }
        Action::Continue { prompt } => {
            if command.expected_revision.is_none() || !text(prompt, 32 * 1024, true) {
                return Err(Error::InvalidCommand(
                    "a follow-up requires a revision and a bounded prompt",
                ));
            }
        }
        Action::Cancel { reason } => {
            if command.expected_revision.is_none() || !text(reason, 2048, true) {
                return Err(Error::InvalidCommand(
                    "cancellation requires a revision and a nonempty bounded reason",
                ));
            }
        }
    }
    Ok(command)
}

pub(crate) fn identifier(value: &str, slash: bool) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || b"._-".contains(&byte) || (slash && byte == b'/')
        })
}

pub(crate) fn text(value: &str, max: usize, multiline: bool) -> bool {
    !value.trim().is_empty()
        && value.len() <= max
        && value
            .chars()
            .all(|ch| !ch.is_control() || (multiline && matches!(ch, '\n' | '\r' | '\t')))
}

fn validate_intent(intent: &TaskIntent) -> Result<(), Error> {
    let path = Path::new(&intent.workspace.path);
    let revision_valid = intent
        .workspace
        .source_revision
        .as_ref()
        .is_none_or(|revision| {
            matches!(revision.len(), 40 | 64)
                && revision
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        });
    if !text(&intent.title, 256, false)
        || !text(&intent.prompt, 32 * 1024, true)
        || !text(&intent.workspace.path, 4096, false)
        || !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        || !revision_valid
        || !identifier(&intent.configuration.adapter, false)
        || intent
            .configuration
            .model
            .as_ref()
            .is_some_and(|model| !identifier(model, true))
    {
        return Err(Error::InvalidCommand(
            "task intent has an invalid title, prompt, workspace, revision, or requested configuration",
        ));
    }
    Ok(())
}

/// One task's file: its materialized state and the commands and owner
/// events that produced it, in order. Reading it replays that journal.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TaskFile {
    schema: String,
    task: Task,
    commands: Vec<Accepted>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    host_events: Vec<owner::Record>,
}

impl TaskFile {
    /// The sequence the next command or event of this task takes.
    fn next_sequence(&self) -> u64 {
        let commands = self.commands.last().map_or(0, |item| item.receipt.sequence);
        let events = self.host_events.last().map_or(0, |item| item.sequence);
        commands.max(events) + 1
    }
}

/// The store's marker.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    schema: String,
    /// The legacy document this store was migrated from, when it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    migrated_from: Option<String>,
}

/// One accepted command identity: which task it belongs to.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    command_id: String,
    task_id: String,
    submit: bool,
}

/// A handle on the inbox directory. It holds no lock between calls.
///
/// Each task lives in its own file, replaced atomically; a write takes only
/// that task's lock, plus the stable lock for the moment it reserves a
/// command identity. Reads take no lock and see the latest committed state.
/// Commands commit before returning a receipt. No host, model, relay, or
/// executor is called.
pub struct Store {
    dir: PathBuf,
    /// The stable lock's handle, which proves the directory is the one opened.
    lock: File,
    /// How long a write waits for another writer of the same task.
    wait: Duration,
    healthy: bool,
    #[cfg(test)]
    fault: std::cell::Cell<Option<Fault>>,
}

/// A private fence for one task's journal and native transitions.
struct TaskWriteGuard {
    dir: PathBuf,
    task: String,
    file: File,
}

/// Read a retained task without initializing, migrating, settling, or cleaning its store.
/// This replays the owning journal and requires an existing private v3 store.
/// Missing or legacy stores remain unchanged; no execution or write lock is acquired.
pub fn retained_task(dir: &Path, id: &str) -> Result<Task, Error> {
    retained_snapshot(dir, id).map(|(task, _)| task)
}

/// Capture the validated journal and its exact bytes without creating or settling a store.
pub(super) fn retained_snapshot(dir: &Path, id: &str) -> Result<(Task, Vec<u8>), Error> {
    if !identifier(id, false) {
        return Err(Error::NotFound);
    }
    verify_directory(dir)?;
    let _lock = private_open(&dir.join(LOCK_FILE), false, false)?;
    validate_initialized(dir)?;
    let bytes =
        task_file_bytes(&dir.join(TASK_DIR).join(format!("{id}.json")))?.ok_or(Error::NotFound)?;
    let file = parse_task_file(&bytes, id)?;
    Ok((file.task, bytes))
}

/// Existing task identities in stable order, without initializing or repairing their store.
pub(super) fn retained_ids(dir: &Path) -> Result<Vec<String>, Error> {
    verify_directory(dir)?;
    let _lock = private_open(&dir.join(LOCK_FILE), false, false)?;
    validate_initialized(dir)?;
    let mut ids = Vec::new();
    for entry in std::fs::read_dir(dir.join(TASK_DIR))? {
        let name = entry?.file_name();
        if let Some(id) = name.to_str().and_then(|name| name.strip_suffix(".json"))
            && identifier(id, false)
        {
            ids.push(id.to_owned());
            if ids.len() > MAX_TASKS {
                return Err(Error::LimitExceeded);
            }
        }
    }
    ids.sort();
    Ok(ids)
}

impl Store {
    /// Open or initialize a dedicated private directory outside any checkout.
    /// A write waits up to five seconds for another writer of its task.
    pub fn open(dir: &Path) -> Result<Self, Error> {
        Self::open_waiting(dir, LOCK_WAIT)
    }

    /// [`Store::open`] for a task's own owner process (its launcher, its
    /// admission, and every record it makes while it runs), waiting up to
    /// [`OWNER_LOCK_WAIT`] for a busy task instead of failing it.
    pub fn open_for_owner(dir: &Path) -> Result<Self, Error> {
        Self::open_waiting(dir, OWNER_LOCK_WAIT)
    }

    /// [`Store::open`], waiting up to `wait` for another holder of a lock
    /// this handle needs before it refuses with [`Error::Busy`]. Opening
    /// takes the stable lock only to initialize or migrate the store.
    pub fn open_waiting(dir: &Path, wait: Duration) -> Result<Self, Error> {
        if !cfg!(any(unix, windows)) {
            return Err(Error::UnsupportedPlatform);
        }
        prepare_directory(dir)?;
        let dir = dir.canonicalize()?;
        let marker = dir.join(STORE_FILE);
        let legacy = dir.join(LEGACY_STORE_FILE);
        let lock_path = dir.join(LOCK_FILE);
        let started_any = |dir: &Path| -> Result<bool, Error> {
            Ok(regular_or_absent(&marker)?
                || regular_or_absent(&legacy)?
                || regular_or_absent(&dir.join(PENDING_FILE))?
                || std::fs::symlink_metadata(dir.join(TASK_DIR)).is_ok())
        };
        if !regular_or_absent(&lock_path)? && started_any(&dir)? && !regular_or_absent(&lock_path)?
        {
            return Err(Error::Corrupt(
                "an existing or incomplete task store has no stable lock file",
            ));
        }
        let lock_created = match private_open(&lock_path, true, true) {
            Ok(file) => Some(file),
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
            Err(error) => return Err(error),
        };
        let fresh = lock_created.is_some();
        let lock = match lock_created {
            Some(file) => file,
            None => private_open(&lock_path, false, true)?,
        };
        let started = Instant::now();
        let mut created = false;
        loop {
            take_lock(&lock, wait)?;
            verify_same_file(&lock_path, &lock)?;
            let outcome = if regular_or_absent(&marker)? {
                if fresh {
                    Err(Error::Corrupt(
                        "an existing task store has no stable lock file",
                    ))
                } else {
                    open_initialized(&dir)
                }
            } else if regular_or_absent(&legacy)? {
                if fresh {
                    Err(Error::Corrupt(
                        "an existing task document has no stable lock file",
                    ))
                } else {
                    created = true;
                    migrate(&dir)
                }
            } else if !fresh && started.elapsed() < LOCK_WAIT {
                // The lock creator may not have acquired its own lock yet.
                // Give it a chance to initialize, but never make absence
                // permission to initialize another process's store.
                lock.unlock()?;
                std::thread::sleep(Duration::from_millis(10));
                continue;
            } else if !fresh
                || dir.join(PENDING_FILE).try_exists()?
                || std::fs::symlink_metadata(dir.join(TASK_DIR)).is_ok()
            {
                Err(Error::Corrupt(
                    "the task store marker is missing from an initialized or incomplete store",
                ))
            } else {
                created = true;
                initialize(&dir)
            };
            lock.unlock()?;
            outcome?;
            break;
        }
        if created {
            // A new store's files and every directory entry above it are
            // durable before the first command trusts them. An existing
            // store's writes made their own files durable.
            lock.sync_all()?;
            sync_directory_ancestry(&dir)?;
        }
        Ok(Self {
            dir,
            lock,
            wait,
            healthy: true,
            #[cfg(test)]
            fault: std::cell::Cell::new(None),
        })
    }

    /// Apply a command, or return its original receipt for an exact-byte retry.
    pub fn apply(&mut self, bytes: &[u8]) -> Result<Receipt, Error> {
        self.check_healthy()?;
        let command = parse_command(bytes)?;
        // A reserved identity of another task refuses before any lock.
        if self
            .identities()?
            .iter()
            .any(|item| item.command_id == command.command_id && item.task_id != command.task_id)
        {
            return Err(Error::Conflict);
        }
        let _task = self.lock_task(&command.task_id)?;
        self.apply_under_task_lock(bytes, &command)
    }

    /// The caller holds this store's validated write lock for the command's task.
    fn apply_locked(&mut self, bytes: &[u8], guard: &TaskWriteGuard) -> Result<Receipt, Error> {
        self.check_healthy()?;
        let command = parse_command(bytes)?;
        if guard.dir != self.dir || guard.task != command.task_id {
            return Err(Error::Conflict);
        }
        verify_same_file(
            &self.dir.join(TASK_DIR).join(format!("{}.lock", guard.task)),
            &guard.file,
        )?;
        if self
            .identities()?
            .iter()
            .any(|item| item.command_id == command.command_id && item.task_id != command.task_id)
        {
            return Err(Error::Conflict);
        }
        self.apply_under_task_lock(bytes, &command)
    }

    fn apply_under_task_lock(&mut self, bytes: &[u8], command: &Command) -> Result<Receipt, Error> {
        let current = self.read_task(&command.task_id)?;
        if let Some(accepted) = current.as_ref().and_then(|file| {
            file.commands
                .iter()
                .find(|accepted| accepted.receipt.command_id == command.command_id)
        }) {
            if accepted.request.as_bytes() != bytes {
                return Err(Error::Conflict);
            }
            // A previous writer may have renamed and failed its durability
            // barrier. Visibility alone cannot authorize a retry receipt.
            self.barrier(&command.task_id)?;
            return Ok(accepted.receipt.clone());
        }
        let sequence = current.as_ref().map_or(1, TaskFile::next_sequence);
        let mut tasks = BTreeMap::new();
        if let Some(file) = &current {
            tasks.insert(command.task_id.clone(), file.task.clone());
        }
        let receipt = transition(&command, &digest_bytes(bytes), sequence, &mut tasks)?;
        let request = std::str::from_utf8(bytes)
            .map_err(|_| Error::InvalidCommand("the command is not UTF-8"))?
            .to_owned();
        self.reserve(&command)?;
        let task = tasks
            .remove(&command.task_id)
            .ok_or(Error::Corrupt("a transition lost its task"))?;
        let mut file = current.unwrap_or_else(|| TaskFile {
            schema: TASK_FILE_SCHEMA.into(),
            task: task.clone(),
            commands: Vec::new(),
            host_events: Vec::new(),
        });
        file.task = task;
        file.commands.push(Accepted {
            request,
            receipt: receipt.clone(),
        });
        if let Err(error) = self.write_task(&file) {
            self.healthy = false;
            return Err(error);
        }
        Ok(receipt)
    }

    /// List tasks in task-identity order, including cancelled tasks.
    pub fn list(&self) -> Result<Vec<Task>, Error> {
        self.check_healthy()?;
        let directory = self.dir.join(TASK_DIR);
        let mut ids = Vec::new();
        for entry in std::fs::read_dir(&directory)? {
            let name = entry?.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            // Temporary files start with a dot and are never read.
            if let Some(id) = name.strip_suffix(".json")
                && identifier(id, false)
            {
                ids.push(id.to_owned());
            }
        }
        ids.sort();
        let mut tasks = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(file) = self.read_task(&id)? {
                tasks.push(file.task);
            }
        }
        Ok(tasks)
    }

    /// Read one task's latest state.
    pub fn show(&self, id: &str) -> Result<Task, Error> {
        self.check_healthy()?;
        if !identifier(id, false) {
            return Err(Error::NotFound);
        }
        self.read_task(id)?
            .map(|file| file.task)
            .ok_or(Error::NotFound)
    }

    fn check_healthy(&self) -> Result<(), Error> {
        if !self.healthy {
            return Err(Error::ReopenRequired);
        }
        verify_directory(&self.dir)?;
        verify_directory(&self.dir.join(TASK_DIR))?;
        verify_same_file(&self.dir.join(LOCK_FILE), &self.lock)
    }

    fn task_path(&self, id: &str) -> PathBuf {
        self.dir.join(TASK_DIR).join(format!("{id}.json"))
    }

    /// `id`'s validated file, or `None` when the task does not exist.
    fn read_task(&self, id: &str) -> Result<Option<TaskFile>, Error> {
        read_task_file(&self.task_path(id), id)
    }

    fn task_guard(&self, id: &str) -> Result<TaskWriteGuard, Error> {
        if !identifier(id, false) {
            return Err(Error::NotFound);
        }
        Ok(TaskWriteGuard {
            dir: self.dir.clone(),
            task: id.to_owned(),
            file: self.lock_task(id)?,
        })
    }

    /// Take `id`'s write lock, waiting up to this handle's wait.
    fn lock_task(&self, id: &str) -> Result<File, Error> {
        let path = self.dir.join(TASK_DIR).join(format!("{id}.lock"));
        let lock = open_lock(&path)?;
        #[cfg(test)]
        match lock.try_lock() {
            Ok(()) => lock.unlock()?,
            Err(std::fs::TryLockError::WouldBlock) => {
                TASK_LOCK_WAITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            Err(std::fs::TryLockError::Error(_)) => {}
        }
        take_lock(&lock, self.wait)?;
        verify_same_file(&path, &lock)?;
        Ok(lock)
    }

    /// Take the lock an owner event reserving a workspace holds while it
    /// checks every other task.
    fn lock_workspaces(&self) -> Result<File, Error> {
        let path = self.dir.join(WORKSPACE_LOCK_FILE);
        let lock = open_lock(&path)?;
        take_lock(&lock, self.wait)?;
        verify_same_file(&path, &lock)?;
        Ok(lock)
    }

    /// Every reserved command identity, read without a lock. A final line
    /// without its newline is an append a crash interrupted and is ignored.
    fn identities(&self) -> Result<Vec<Identity>, Error> {
        Ok(read_identities(&self.dir)?.0)
    }

    /// Reserve `command`'s identity for its task under the stable lock,
    /// refusing an identity another task holds and new work past the
    /// retained capacity.
    fn reserve(&self, command: &Command) -> Result<(), Error> {
        take_lock(&self.lock, self.wait)?;
        let result = (|| {
            verify_same_file(&self.dir.join(LOCK_FILE), &self.lock)?;
            let (identities, length) = read_identities(&self.dir)?;
            if let Some(item) = identities
                .iter()
                .find(|item| item.command_id == command.command_id)
            {
                return if item.task_id == command.task_id {
                    // An earlier attempt reserved it and never committed.
                    Ok(())
                } else {
                    Err(Error::Conflict)
                };
            }
            if identities.len() >= MAX_COMMANDS {
                return Err(Error::LimitExceeded);
            }
            let submit = matches!(command.action, Action::Submit { .. });
            if submit {
                let tasks: BTreeSet<&str> = identities
                    .iter()
                    .filter(|item| item.submit)
                    .map(|item| item.task_id.as_str())
                    .collect();
                if !tasks.contains(command.task_id.as_str()) && tasks.len() >= MAX_TASKS {
                    return Err(Error::LimitExceeded);
                }
            }
            let mut line = serde_json::to_vec(&Identity {
                command_id: command.command_id.clone(),
                task_id: command.task_id.clone(),
                submit,
            })
            .map_err(|_| Error::Corrupt("a command identity could not be encoded"))?;
            line.push(b'\n');
            let mut file = private_open(&self.dir.join(IDENTITY_FILE), false, true)?;
            // Drop an append a crash interrupted before adding this one.
            file.set_len(length)?;
            std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(length))?;
            file.write_all(&line)?;
            file.sync_all()?;
            Ok(())
        })();
        self.lock.unlock()?;
        result
    }

    /// Make `id`'s file and its directory entry durable.
    fn barrier(&self, id: &str) -> Result<(), Error> {
        #[cfg(test)]
        if BARRIER_SYNC_FAIL.with(|fault| fault.replace(false)) {
            return Err(Error::Io(std::io::Error::other(
                "injected retry barrier failure",
            )));
        }
        private_open(&self.task_path(id), false, cfg!(windows))?.sync_all()?;
        sync_directory(&self.dir.join(TASK_DIR))
    }

    fn write_task(&self, file: &TaskFile) -> Result<(), Error> {
        self.check_healthy()?;
        let bytes = serde_json::to_vec(file)
            .map_err(|_| Error::Corrupt("the task file could not be encoded"))?;
        if bytes.len() > MAX_STORE_BYTES {
            return Err(Error::LimitExceeded);
        }
        let directory = self.dir.join(TASK_DIR);
        let name = format!("{}.json", file.task.task_id);
        let temporary = directory.join(format!(".{name}.tmp"));
        let result = (|| {
            write_new(&temporary, &bytes)?;
            #[cfg(test)]
            self.inject_fault(Fault::BeforeRename)?;
            regular_or_absent(&directory.join(&name))?;
            std::fs::rename(&temporary, directory.join(&name))?;
            #[cfg(test)]
            self.inject_fault(Fault::AfterRename)?;
            sync_directory(&directory)
        })();
        // A failure after rename is ambiguous to the caller. The next
        // retry revalidates disk and returns the original receipt only
        // after a durability barrier.
        if result.is_err() && regular_or_absent(&temporary).unwrap_or(false) {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }

    #[cfg(test)]
    fn inject_fault(&self, point: Fault) -> Result<(), Error> {
        if self.fault.get() == Some(point) {
            self.fault.set(None);
            return Err(Error::Io(std::io::Error::other(
                "injected task store failure",
            )));
        }
        Ok(())
    }
}

/// Whether `dir` holds a task store, current or not yet migrated.
#[must_use]
pub fn present(dir: &Path) -> bool {
    dir.join(STORE_FILE).is_file() || dir.join(LEGACY_STORE_FILE).is_file()
}

/// How many times this process found a task's write lock held by another
/// writer: what writers of different tasks must never do.
#[cfg(test)]
pub(crate) static TASK_LOCK_WAITS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Wait up to `wait` for `lock`'s exclusive OS lock.
pub(crate) fn take_lock(lock: &File, wait: Duration) -> Result<(), Error> {
    let started = Instant::now();
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(()),
            Err(std::fs::TryLockError::WouldBlock) if started.elapsed() < wait => {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(std::fs::TryLockError::WouldBlock) => return Err(Error::Busy),
            Err(std::fs::TryLockError::Error(error)) => return Err(Error::Io(error)),
        }
    }
}

/// Open a stable private lock file, creating it when absent.
pub(crate) fn open_lock(path: &Path) -> Result<File, Error> {
    match private_open(path, true, true) {
        Ok(file) => Ok(file),
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            private_open(path, false, true)
        }
        Err(error) => Err(error),
    }
}

/// Write `bytes` to a new private file at `path` and make them durable,
/// replacing a leftover temporary file first.
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    if regular_or_absent(path)? {
        std::fs::remove_file(path)?;
    }
    let mut file = private_open(path, true, true)?;
    file.write_all(bytes)?;
    // Tests cannot observe a power loss, and a flush per save of a large
    // store (the sales qualification test saves over a thousand times)
    // holds the crate's tests past their time limit.
    if !cfg!(test) {
        file.sync_all()?;
    }
    Ok(())
}

/// Replace `dir/name` with `bytes` atomically: a durable temporary file,
/// a rename, and a directory sync.
pub(crate) fn replace_file(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), Error> {
    let temporary = dir.join(format!(".{name}.tmp"));
    write_new(&temporary, bytes)?;
    regular_or_absent(&dir.join(name))?;
    std::fs::rename(&temporary, dir.join(name))?;
    sync_directory(dir)
}

/// Create the private task directory, or check the one there.
fn task_directory(dir: &Path) -> Result<PathBuf, Error> {
    let path = dir.join(TASK_DIR);
    match std::fs::symlink_metadata(&path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            make_private_directory(&path)?;
            sync_directory(dir)?;
        }
        Err(error) => return Err(Error::Io(error)),
    }
    verify_directory(&path)?;
    Ok(path)
}

/// Start an empty store: its task directory, an empty identity log, and
/// last its marker. Called under the stable lock.
fn initialize(dir: &Path) -> Result<(), Error> {
    task_directory(dir)?;
    replace_file(dir, IDENTITY_FILE, b"")?;
    write_marker(dir, None)
}

fn write_marker(dir: &Path, migrated_from: Option<&str>) -> Result<(), Error> {
    let bytes = serde_json::to_vec(&Marker {
        schema: STORE_SCHEMA.into(),
        migrated_from: migrated_from.map(Into::into),
    })
    .map_err(|_| Error::Corrupt("the task store marker could not be encoded"))?;
    replace_file(dir, STORE_FILE, &bytes)
}

/// Check an initialized store under the stable lock: its marker, task
/// directory, and identity log; a legacy document a crash left behind
/// after its migration becomes the backup; a stale legacy pending file
/// is discarded.
fn open_initialized(dir: &Path) -> Result<(), Error> {
    validate_initialized(dir)?;
    if regular_or_absent(&dir.join(LEGACY_STORE_FILE))? {
        keep_legacy(dir)?;
    }
    discard_pending(dir)
}

/// Read only the current store's marker and required directory/log identities.
fn validate_initialized(dir: &Path) -> Result<(), Error> {
    let file = private_open(&dir.join(STORE_FILE), false, false)?;
    if file.metadata()?.len() > 64 * 1024 {
        return Err(Error::LimitExceeded);
    }
    let mut bytes = Vec::new();
    file.take(64 * 1024).read_to_end(&mut bytes)?;
    let value = parse_strict_bounded(&bytes, 64 * 1024)
        .map_err(|_| Error::Corrupt("the task store marker is not strict JSON"))?;
    let marker: Marker = serde_json::from_value(value)
        .map_err(|_| Error::Corrupt("the task store marker does not match its schema"))?;
    if marker.schema != STORE_SCHEMA {
        return Err(Error::UnsupportedSchema);
    }
    if std::fs::symlink_metadata(dir.join(TASK_DIR)).is_err() {
        return Err(Error::Corrupt(
            "the task directory is missing from an initialized store",
        ));
    }
    verify_directory(&dir.join(TASK_DIR))?;
    if !regular_or_absent(&dir.join(IDENTITY_FILE))? {
        return Err(Error::Corrupt(
            "the command identity log is missing from an initialized store",
        ));
    }
    Ok(())
}

/// Move the migrated legacy document aside, never over an earlier backup.
fn keep_legacy(dir: &Path) -> Result<(), Error> {
    let mut backup = dir.join(LEGACY_BACKUP_FILE);
    let mut index = 1;
    while std::fs::symlink_metadata(&backup).is_ok() {
        backup = dir.join(format!("tasks.v2.{index}.json"));
        index += 1;
    }
    std::fs::rename(dir.join(LEGACY_STORE_FILE), backup)?;
    sync_directory(dir)
}

fn discard_pending(dir: &Path) -> Result<(), Error> {
    let path = dir.join(PENDING_FILE);
    if regular_or_absent(&path)? {
        std::fs::remove_file(path)?;
        sync_directory(dir)?;
    }
    Ok(())
}

/// Migrate the legacy single document under the stable lock (#10231).
///
/// The document is validated exactly as before. Each task's file is
/// written from its own commands and events, every legacy command identity
/// goes into the identity log, and only then is the marker written and the
/// document moved to [`LEGACY_BACKUP_FILE`]. A crash before the marker
/// leaves the document untouched, and the next open migrates again.
fn migrate(dir: &Path) -> Result<(), Error> {
    discard_pending(dir)?;
    let document = read_document(&dir.join(LEGACY_STORE_FILE))?;
    let directory = task_directory(dir)?;
    let mut files: BTreeMap<String, TaskFile> = document
        .tasks
        .iter()
        .map(|(id, task)| {
            (
                id.clone(),
                TaskFile {
                    schema: TASK_FILE_SCHEMA.into(),
                    task: task.clone(),
                    commands: Vec::new(),
                    host_events: Vec::new(),
                },
            )
        })
        .collect();
    let mut identities = Vec::with_capacity(document.commands.len() * 96);
    for accepted in &document.commands {
        let command = parse_command(accepted.request.as_bytes())
            .map_err(|_| Error::Corrupt("the retained task command is invalid"))?;
        files
            .get_mut(&command.task_id)
            .ok_or(Error::Corrupt("a retained command names no task"))?
            .commands
            .push(accepted.clone());
        let mut line = serde_json::to_vec(&Identity {
            submit: matches!(command.action, Action::Submit { .. }),
            command_id: command.command_id,
            task_id: command.task_id,
        })
        .map_err(|_| Error::Corrupt("a command identity could not be encoded"))?;
        line.push(b'\n');
        identities.extend_from_slice(&line);
    }
    for record in &document.host_events {
        files
            .get_mut(record.task_id.as_str())
            .ok_or(Error::Corrupt("a retained owner event names no task"))?
            .host_events
            .push(record.clone());
    }
    // Write every task's temporary file and make it durable, then rename
    // them all and sync the directory once.
    let mut renames = Vec::with_capacity(files.len());
    for (id, file) in &files {
        let bytes = serde_json::to_vec(file)
            .map_err(|_| Error::Corrupt("the task file could not be encoded"))?;
        if bytes.len() > MAX_STORE_BYTES {
            return Err(Error::LimitExceeded);
        }
        let name = format!("{id}.json");
        let temporary = directory.join(format!(".{name}.tmp"));
        write_new(&temporary, &bytes)?;
        renames.push((temporary, directory.join(name)));
    }
    for (temporary, path) in renames {
        regular_or_absent(&path)?;
        std::fs::rename(temporary, path)?;
    }
    sync_directory(&directory)?;
    // Every written file must read back as the task the document held.
    for (id, file) in &files {
        let read = read_task_file(&directory.join(format!("{id}.json")), id)?
            .ok_or(Error::Corrupt("a migrated task file is missing"))?;
        if read.task != file.task {
            return Err(Error::Corrupt(
                "a migrated task file differs from its document",
            ));
        }
    }
    replace_file(dir, IDENTITY_FILE, &identities)?;
    write_marker(dir, Some(LEGACY_BACKUP_FILE))?;
    keep_legacy(dir)
}

/// The identity log's complete lines and their length in bytes.
fn read_identities(dir: &Path) -> Result<(Vec<Identity>, u64), Error> {
    let file = private_open(&dir.join(IDENTITY_FILE), false, false)?;
    if file.metadata()?.len() > MAX_IDENTITY_BYTES {
        return Err(Error::LimitExceeded);
    }
    let mut bytes = Vec::new();
    file.take(MAX_IDENTITY_BYTES + 1).read_to_end(&mut bytes)?;
    let complete = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |at| at + 1);
    let mut identities = Vec::new();
    for line in bytes[..complete].split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        let identity: Identity = serde_json::from_slice(line)
            .map_err(|_| Error::Corrupt("the command identity log has an invalid line"))?;
        if !identifier(&identity.command_id, false) || !identifier(&identity.task_id, false) {
            return Err(Error::Corrupt(
                "the command identity log has an invalid line",
            ));
        }
        identities.push(identity);
    }
    Ok((identities, complete as u64))
}

/// How many times a lock-free read reopens a file a writer replaced
/// between the read's open and its identity check.
const REPLACED_RETRIES: u32 = 200;

/// [`private_open`] for a file writers replace by rename while readers hold
/// no lock (a task file). A writer that renames a new file over `path`
/// between this open and its identity check makes the path name a
/// different file than the one opened: that is a replacement, not an
/// unsafe path, while the path is still a regular file, so the read opens
/// it again (#10301, #10355). A path that stays unsafe is still refused.
fn open_replaced_file(path: &Path) -> Result<File, Error> {
    let mut attempt = 0;
    loop {
        match private_open(path, false, false) {
            Err(Error::UnsafePath)
                if attempt < REPLACED_RETRIES
                    && std::fs::symlink_metadata(path)
                        .is_ok_and(|meta| meta.is_file() && !meta.file_type().is_symlink()) =>
            {
                attempt += 1;
                std::thread::sleep(Duration::from_millis(1));
            }
            other => return other,
        }
    }
}

/// Read and validate the task file at `path` for task `id`: replay its
/// commands and owner events in sequence order and compare the task and
/// every receipt. `None` when the file does not exist.
fn read_task_file(path: &Path, id: &str) -> Result<Option<TaskFile>, Error> {
    task_file_bytes(path)?
        .map(|bytes| parse_task_file(&bytes, id))
        .transpose()
}

fn task_file_bytes(path: &Path) -> Result<Option<Vec<u8>>, Error> {
    let file = match open_replaced_file(path) {
        Ok(file) => file,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if file.metadata()?.len() > MAX_STORE_BYTES as u64 {
        return Err(Error::LimitExceeded);
    }
    let mut bytes = Vec::new();
    file.take(MAX_STORE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_STORE_BYTES {
        return Err(Error::LimitExceeded);
    }
    Ok(Some(bytes))
}

fn parse_task_file(bytes: &[u8], id: &str) -> Result<TaskFile, Error> {
    let value = parse_strict_bounded(bytes, MAX_STORE_BYTES)
        .map_err(|_| Error::Corrupt("the task file is not strict JSON"))?;
    let file: TaskFile = serde_json::from_value(value)
        .map_err(|_| Error::Corrupt("the task file does not match the closed store schema"))?;
    if file.schema != TASK_FILE_SCHEMA {
        return Err(Error::UnsupportedSchema);
    }
    if file.task.task_id != id {
        return Err(Error::Corrupt("the task file names another task"));
    }
    if file.commands.len() > MAX_COMMANDS || file.host_events.len() > owner::MAX_HOST_EVENTS {
        return Err(Error::LimitExceeded);
    }
    let mut tasks = BTreeMap::new();
    let mut identities = BTreeSet::new();
    let mut commands = file.commands.iter().peekable();
    let mut events = file.host_events.iter().peekable();
    let mut last = 0;
    loop {
        let command_next = commands.peek().map(|item| item.receipt.sequence);
        let event_next = events.peek().map(|item| item.sequence);
        let sequence = match (command_next, event_next) {
            (None, None) => break,
            (Some(command), Some(event)) => command.min(event),
            (Some(sequence), None) | (None, Some(sequence)) => sequence,
        };
        if sequence <= last || command_next == event_next {
            return Err(Error::Corrupt("the task journal is out of order"));
        }
        last = sequence;
        if event_next == Some(sequence) {
            let record = events.next().expect("peeked record");
            if record.task_id != id {
                return Err(Error::Corrupt("the task journal names another task"));
            }
            owner::transition(record, &mut tasks).map_err(|_| {
                Error::Corrupt("the task event history contains an invalid transition")
            })?;
            continue;
        }
        let accepted = commands.next().expect("peeked command");
        let command = parse_command(accepted.request.as_bytes())
            .map_err(|_| Error::Corrupt("the retained task command is invalid"))?;
        if command.task_id != id {
            return Err(Error::Corrupt("the task journal names another task"));
        }
        if !identities.insert(command.command_id.clone()) {
            return Err(Error::Corrupt("the task file repeats a command identity"));
        }
        let expected = transition(
            &command,
            &digest_bytes(accepted.request.as_bytes()),
            sequence,
            &mut tasks,
        )
        .map_err(|_| Error::Corrupt("the task command history contains an invalid transition"))?;
        if expected != accepted.receipt {
            return Err(Error::Corrupt(
                "the task receipt does not match its command and transition",
            ));
        }
    }
    if tasks.len() != 1 || tasks.get(id) != Some(&file.task) {
        return Err(Error::Corrupt(
            "the task state does not match its command history",
        ));
    }
    Ok(file)
}

fn transition(
    command: &Command,
    digest: &str,
    sequence: u64,
    tasks: &mut BTreeMap<String, Task>,
) -> Result<Receipt, Error> {
    match &command.action {
        Action::Submit { intent } => {
            if tasks.contains_key(&command.task_id) {
                return Err(Error::InvalidTransition);
            }
            if tasks.len() >= MAX_TASKS {
                return Err(Error::LimitExceeded);
            }
            let intent_bytes = serde_json::to_vec(intent)
                .map_err(|_| Error::InvalidCommand("the task intent could not be encoded"))?;
            tasks.insert(
                command.task_id.clone(),
                Task {
                    task_id: command.task_id.clone(),
                    revision: 1,
                    intent: intent.clone(),
                    intent_digest: digest_bytes(&intent_bytes),
                    status: Status::Queued,
                    execution: Execution::NotStarted,
                    checks: Checks::NotRun,
                    cancellation_reason: None,
                    corrections: Vec::new(),
                    run: None,
                    follow_ups: Vec::new(),
                    earlier: Vec::new(),
                },
            );
        }
        Action::Correct { prompt, reason } => {
            let task = tasks.get_mut(&command.task_id).ok_or(Error::NotFound)?;
            if command.expected_revision != Some(task.revision) {
                return Err(Error::RevisionMismatch);
            }
            task.revision += 1;
            task.corrections.push(Correction {
                revision: task.revision,
                prompt: prompt.clone(),
                reason: reason.clone(),
            });
            if task.run.is_some() {
                // A correction never rewrites completed evidence or authorizes a replacement effect.
                if task.checks != Checks::Running {
                    task.checks = Checks::Disputed;
                }
                if task.status == Status::Running {
                    task.status = Status::CancelRequested;
                    task.cancellation_reason =
                        Some("instructions corrected; current context superseded".into());
                }
            }
        }
        Action::Continue { prompt } => {
            let task = tasks.get_mut(&command.task_id).ok_or(Error::NotFound)?;
            if command.expected_revision != Some(task.revision) {
                return Err(Error::RevisionMismatch);
            }
            // Only an ended turn continues: never a running, queued, or
            // unknown one, and never while its checks run.
            if !matches!(task.status, Status::Finished | Status::Cancelled)
                || task.checks == Checks::Running
            {
                return Err(Error::InvalidTransition);
            }
            if task.turn() >= MAX_TURNS {
                return Err(Error::LimitExceeded);
            }
            if let Some(run) = task.run.take() {
                task.earlier.push(run);
            }
            task.revision += 1;
            task.status = Status::Queued;
            task.execution = Execution::NotStarted;
            task.checks = Checks::NotRun;
            task.cancellation_reason = None;
            task.follow_ups.push(FollowUp {
                revision: task.revision,
                prompt: prompt.clone(),
            });
        }
        Action::Cancel { reason } => {
            let task = tasks.get_mut(&command.task_id).ok_or(Error::NotFound)?;
            if command.expected_revision != Some(task.revision) {
                return Err(Error::RevisionMismatch);
            }
            task.status = match task.status {
                Status::Queued => Status::Cancelled,
                Status::Running => Status::CancelRequested,
                _ => return Err(Error::InvalidTransition),
            };
            task.revision += 1;
            task.cancellation_reason = Some(reason.clone());
        }
    }
    let task = &tasks[&command.task_id];
    Ok(Receipt {
        schema: RECEIPT_SCHEMA.into(),
        command_id: command.command_id.clone(),
        task_id: command.task_id.clone(),
        request_digest: digest.into(),
        sequence,
        revision: task.revision,
        status: task.status,
        execution: task.execution,
        checks: task.checks,
    })
}

fn read_document(path: &Path) -> Result<Document, Error> {
    let file = private_open(path, false, false)?;
    if file.metadata()?.len() > MAX_STORE_BYTES as u64 {
        return Err(Error::LimitExceeded);
    }
    let mut bytes = Vec::new();
    file.take(MAX_STORE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_STORE_BYTES {
        return Err(Error::LimitExceeded);
    }
    let value = parse_strict_bounded(&bytes, MAX_STORE_BYTES)
        .map_err(|_| Error::Corrupt("the task document is not strict JSON"))?;
    let document: Document = serde_json::from_value(value)
        .map_err(|_| Error::Corrupt("the task document does not match the closed store schema"))?;
    if document.schema != LEGACY_STORE_SCHEMA && document.schema != LEGACY_V1_SCHEMA {
        return Err(Error::UnsupportedSchema);
    }
    if document.tasks.len() > MAX_TASKS
        || document.commands.len() > MAX_COMMANDS
        || document.host_events.len() > owner::MAX_HOST_EVENTS
    {
        return Err(Error::LimitExceeded);
    }
    if document.schema == LEGACY_V1_SCHEMA && !document.host_events.is_empty() {
        return Err(Error::Corrupt(
            "a legacy inbox cannot contain execution events",
        ));
    }
    if document.sequence != (document.commands.len() + document.host_events.len()) as u64 {
        return Err(Error::Corrupt(
            "the task document sequence does not match its command history",
        ));
    }
    let mut tasks = BTreeMap::new();
    let mut identities = BTreeSet::new();
    let mut host_events = document.host_events.iter().peekable();
    let mut commands = document.commands.iter().peekable();
    for sequence in 1..=document.sequence {
        if host_events
            .peek()
            .is_some_and(|record| record.sequence == sequence)
        {
            let record = host_events.next().expect("peeked record");
            owner::transition(record, &mut tasks)?;
            continue;
        }
        let accepted = commands
            .next()
            .ok_or(Error::Corrupt("the task journal has a sequence gap"))?;
        if accepted.receipt.sequence != sequence {
            return Err(Error::Corrupt("the task journal is out of order"));
        }
        let command = parse_command(accepted.request.as_bytes())
            .map_err(|_| Error::Corrupt("the retained task command is invalid"))?;
        if !identities.insert(command.command_id.clone()) {
            return Err(Error::Corrupt(
                "the task document repeats a command identity",
            ));
        }
        let expected = transition(
            &command,
            &digest_bytes(accepted.request.as_bytes()),
            sequence,
            &mut tasks,
        )
        .map_err(|_| Error::Corrupt("the task command history contains an invalid transition"))?;
        if expected != accepted.receipt {
            return Err(Error::Corrupt(
                "the task receipt does not match its command and transition",
            ));
        }
    }
    if commands.next().is_some() || host_events.next().is_some() {
        return Err(Error::Corrupt("the task journal has extra records"));
    }
    if tasks != document.tasks {
        return Err(Error::Corrupt(
            "the task states do not match their command history",
        ));
    }
    Ok(document)
}

fn sync_directory_ancestry(path: &Path) -> Result<(), Error> {
    for ancestor in path.ancestors() {
        sync_directory(ancestor)?;
    }
    Ok(())
}

/// Flushes a directory's entries.
#[cfg(unix)]
pub(crate) fn sync_directory(path: &Path) -> Result<(), Error> {
    let directory = File::open(path)?;
    // As in `write_new`: tests check that the directory opens, not the flush.
    if !cfg!(test) {
        directory.sync_all()?;
    }
    Ok(())
}

/// Windows cannot flush a directory, and NTFS journals its entries, so
/// this only checks that it is there.
#[cfg(windows)]
pub(crate) fn sync_directory(path: &Path) -> Result<(), Error> {
    private_fs::open_dir(path)?;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn sync_directory(_: &Path) -> Result<(), Error> {
    Err(Error::UnsupportedPlatform)
}

/// The Windows form of the store directory's rule: every missing directory
/// is created for this user alone (an owner-only DACL), and the store
/// directory must be one this user owns that admits no one else.
#[cfg(windows)]
pub(crate) fn prepare_directory(path: &Path) -> Result<(), Error> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => return verify_directory(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::Io(error)),
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    private_fs::create_dir_all(&absolute)?;
    verify_directory(path)
}

#[cfg(windows)]
fn verify_directory(path: &Path) -> Result<(), Error> {
    let directory = private_fs::open_dir(path)?;
    let metadata = directory.metadata()?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || !private_fs::is_private(&directory)?
    {
        return Err(Error::UnsafePath);
    }
    Ok(())
}

#[cfg(windows)]
pub(crate) fn verify_same_file(path: &Path, file: &File) -> Result<(), Error> {
    let (at_path, path_metadata) = private_fs::identity_of(path)?;
    let opened = private_fs::identity(file)?;
    if !path_metadata.is_file()
        || path_metadata.file_type().is_symlink()
        || !file.metadata()?.is_file()
        || opened.links != 1
        || (at_path.volume, at_path.index) != (opened.volume, opened.index)
        || !private_fs::is_private(file)?
    {
        return Err(Error::UnsafePath);
    }
    Ok(())
}

#[cfg(windows)]
pub(crate) fn private_open(path: &Path, create: bool, write: bool) -> Result<File, Error> {
    if !create {
        regular_or_absent(path)?;
    }
    let file = private_fs::nofollow(
        OpenOptions::new()
            .read(true)
            .write(write)
            .create_new(create),
    )
    .open(path)?;
    verify_same_file(path, &file)?;
    Ok(file)
}

#[cfg(unix)]
fn make_private_directory(path: &Path) -> Result<(), Error> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(path)?;
    Ok(())
}

#[cfg(windows)]
fn make_private_directory(path: &Path) -> Result<(), Error> {
    private_fs::create_dir_all(path)?;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn make_private_directory(_: &Path) -> Result<(), Error> {
    Err(Error::UnsupportedPlatform)
}

#[cfg(unix)]
pub(crate) fn prepare_directory(path: &Path) -> Result<(), Error> {
    use std::os::unix::fs::DirBuilderExt;
    match std::fs::symlink_metadata(path) {
        Ok(_) => return verify_directory(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::Io(error)),
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut missing = Vec::new();
    let mut current = absolute.as_path();
    loop {
        match std::fs::symlink_metadata(current) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(current.to_path_buf());
                current = current.parent().ok_or(Error::UnsafePath)?;
            }
            Err(error) => return Err(Error::Io(error)),
        }
    }
    for directory in missing.into_iter().rev() {
        match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(Error::Io(error)),
        }
        verify_directory(&directory)?;
        // Persist each new directory entry as well as the document inside it.
        sync_directory(directory.parent().ok_or(Error::UnsafePath)?)?;
    }
    verify_directory(path)
}

#[cfg(unix)]
fn verify_directory(path: &Path) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(Error::UnsafePath);
    }
    Ok(())
}

pub(crate) fn regular_or_absent(path: &Path) -> Result<bool, Error> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(true),
        Ok(_) => Err(Error::UnsafePath),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(Error::Io(error)),
    }
}

#[cfg(unix)]
pub(crate) fn verify_same_file(path: &Path, file: &File) -> Result<(), Error> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let path_metadata = std::fs::symlink_metadata(path)?;
    let file_metadata = file.metadata()?;
    if !path_metadata.is_file()
        || path_metadata.file_type().is_symlink()
        || !file_metadata.is_file()
        || file_metadata.nlink() != 1
        || path_metadata.dev() != file_metadata.dev()
        || path_metadata.ino() != file_metadata.ino()
        || file_metadata.permissions().mode() & 0o077 != 0
    {
        return Err(Error::UnsafePath);
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn private_open(path: &Path, create: bool, write: bool) -> Result<File, Error> {
    use std::os::unix::fs::OpenOptionsExt;
    if !create {
        regular_or_absent(path)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(write)
        .create_new(create)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    verify_same_file(path, &file)?;
    Ok(file)
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn prepare_directory(_: &Path) -> Result<(), Error> {
    Err(Error::UnsupportedPlatform)
}
#[cfg(not(any(unix, windows)))]
fn verify_directory(_: &Path) -> Result<(), Error> {
    Err(Error::UnsupportedPlatform)
}
#[cfg(not(any(unix, windows)))]
pub(crate) fn verify_same_file(_: &Path, _: &File) -> Result<(), Error> {
    Err(Error::UnsupportedPlatform)
}
#[cfg(not(any(unix, windows)))]
pub(crate) fn private_open(_: &Path, _: bool, _: bool) -> Result<File, Error> {
    Err(Error::UnsupportedPlatform)
}

#[cfg(test)]
std::thread_local! {
    static BARRIER_SYNC_FAIL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    BeforeRename,
    AfterRename,
}

#[cfg(test)]
mod tests;
