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
/// The persisted inbox format.
pub const STORE_SCHEMA: &str = "openagents.coder.task-store.v1";
/// The largest command, including JSON whitespace, in bytes.
pub const MAX_COMMAND_BYTES: usize = 64 * 1024;
/// The largest persisted inbox document, in bytes.
pub const MAX_STORE_BYTES: usize = 16 * 1024 * 1024;
/// The largest number of retained tasks. No task is silently pruned.
pub const MAX_TASKS: usize = 1024;
/// The largest number of accepted commands. Retry identities are not pruned.
pub const MAX_COMMANDS: usize = 2048;
/// The inbox document's filename within its private directory.
pub const STORE_FILE: &str = "tasks.json";
/// The stable sibling lock. Removing it while a process runs is unsafe.
pub const LOCK_FILE: &str = "tasks.lock";
const PENDING_FILE: &str = ".tasks.pending";
const LOCK_WAIT: Duration = Duration::from_secs(5);

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
}

/// The only two transitions this inbox can perform.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Submit { intent: TaskIntent },
    Cancel { reason: String },
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
}

/// This inbox never starts an executor.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Execution {
    NotStarted,
}

/// This inbox never runs a check.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Checks {
    NotRun,
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
}

/// A closed refusal classification. Messages do not echo command contents.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    InvalidCommand(&'static str),
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
}

impl Error {
    /// Stable refusal codes for machine-readable callers.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::InvalidCommand(_) => "invalid_command",
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
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "task store I/O failed: {error}"),
            Self::InvalidCommand(message) | Self::Corrupt(message) => formatter.write_str(message),
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

fn identifier(value: &str, slash: bool) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || b"._-".contains(&byte) || (slash && byte == b'/')
        })
}

fn text(value: &str, max: usize, multiline: bool) -> bool {
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

/// An exclusively locked inbox. Dropping it releases the OS lock.
///
/// The lock file stays in place. Commands commit an atomic document replacement
/// before returning a receipt. No host, model, relay, or executor is called.
pub struct Store {
    dir: PathBuf,
    document: Document,
    lock: File,
    healthy: bool,
    #[cfg(test)]
    fault: std::cell::Cell<Option<Fault>>,
}

impl Store {
    /// Open or initialize a dedicated private directory outside any checkout.
    pub fn open(dir: &Path) -> Result<Self, Error> {
        if !cfg!(unix) {
            return Err(Error::UnsupportedPlatform);
        }
        prepare_directory(dir)?;
        let dir = dir.canonicalize()?;
        let path = dir.join(STORE_FILE);
        let lock_path = dir.join(LOCK_FILE);
        if !regular_or_absent(&lock_path)?
            && (regular_or_absent(&path)? || regular_or_absent(&dir.join(PENDING_FILE))?)
            && !regular_or_absent(&lock_path)?
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
        let (document, initialize) = loop {
            loop {
                match lock.try_lock() {
                    Ok(()) => break,
                    Err(std::fs::TryLockError::WouldBlock) if started.elapsed() < LOCK_WAIT => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(std::fs::TryLockError::WouldBlock) => return Err(Error::Busy),
                    Err(std::fs::TryLockError::Error(error)) => return Err(Error::Io(error)),
                }
            }
            verify_same_file(&lock_path, &lock)?;
            match read_document(&path) {
                Ok(document) => {
                    if fresh {
                        return Err(Error::Corrupt(
                            "an existing task document has no stable lock file",
                        ));
                    }
                    break (document, false);
                }
                Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                    if !fresh && started.elapsed() < LOCK_WAIT {
                        // The lock creator may not have acquired its own lock
                        // yet. Give it a chance to initialize, but never make
                        // absence permission to initialize another process's store.
                        lock.unlock()?;
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    if !fresh || dir.join(PENDING_FILE).try_exists()? {
                        return Err(Error::Corrupt(
                            "the task document is missing from an initialized or incomplete store",
                        ));
                    }
                    break (
                        Document {
                            schema: STORE_SCHEMA.into(),
                            sequence: 0,
                            tasks: BTreeMap::new(),
                            commands: Vec::new(),
                        },
                        true,
                    );
                }
                Err(error) => return Err(error),
            }
        };
        let store = Self {
            dir,
            document,
            lock,
            healthy: true,
            #[cfg(test)]
            fault: std::cell::Cell::new(None),
        };
        if initialize {
            store.commit(&store.document)?;
        } else {
            // A pending file is an uncommitted candidate, never an instruction
            // to replay. A complete validated document remains authoritative.
            store.discard_pending()?;
        }
        // A previous writer may have renamed successfully and failed its
        // durability barrier. Visibility alone cannot authorize a retry receipt.
        #[cfg(test)]
        if OPEN_SYNC_FAIL.with(|fault| fault.replace(false)) {
            return Err(Error::Io(std::io::Error::other(
                "injected reopen sync failure",
            )));
        }
        private_open(&store.dir.join(STORE_FILE), false, false)?.sync_all()?;
        store.lock.sync_all()?;
        sync_directory_ancestry(&store.dir)?;
        Ok(store)
    }

    /// Apply a command, or return its original receipt for an exact-byte retry.
    pub fn apply(&mut self, bytes: &[u8]) -> Result<Receipt, Error> {
        self.check_healthy()?;
        let command = parse_command(bytes)?;
        if let Some(accepted) = self
            .document
            .commands
            .iter()
            .find(|accepted| accepted.receipt.command_id == command.command_id)
        {
            return if accepted.request.as_bytes() == bytes {
                Ok(accepted.receipt.clone())
            } else {
                Err(Error::Conflict)
            };
        }
        if self.document.commands.len() >= MAX_COMMANDS {
            return Err(Error::LimitExceeded);
        }
        let mut next = self.document.clone();
        next.sequence += 1;
        let receipt = transition(
            &command,
            &digest_bytes(bytes),
            next.sequence,
            &mut next.tasks,
        )?;
        let request = std::str::from_utf8(bytes)
            .map_err(|_| Error::InvalidCommand("the command is not UTF-8"))?
            .to_owned();
        next.commands.push(Accepted {
            request,
            receipt: receipt.clone(),
        });
        if let Err(error) = self.commit(&next) {
            self.healthy = false;
            return Err(error);
        }
        self.document = next;
        Ok(receipt)
    }

    /// List tasks in task-identity order, including cancelled tasks.
    pub fn list(&self) -> Result<Vec<Task>, Error> {
        self.check_healthy()?;
        Ok(self.document.tasks.values().cloned().collect())
    }

    /// Read one task's latest state.
    pub fn show(&self, id: &str) -> Result<Task, Error> {
        self.check_healthy()?;
        self.document.tasks.get(id).cloned().ok_or(Error::NotFound)
    }

    fn check_healthy(&self) -> Result<(), Error> {
        if !self.healthy {
            return Err(Error::ReopenRequired);
        }
        verify_directory(&self.dir)?;
        verify_same_file(&self.dir.join(LOCK_FILE), &self.lock)
    }

    fn commit(&self, document: &Document) -> Result<(), Error> {
        self.check_healthy()?;
        let bytes = serde_json::to_vec(document)
            .map_err(|_| Error::Corrupt("the task document could not be encoded"))?;
        if bytes.len() > MAX_STORE_BYTES {
            return Err(Error::LimitExceeded);
        }
        let pending = self.dir.join(PENDING_FILE);
        let result = (|| {
            let mut file = private_open(&pending, true, true)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            #[cfg(test)]
            self.inject_fault(Fault::BeforeRename)?;
            regular_or_absent(&self.dir.join(STORE_FILE))?;
            std::fs::rename(&pending, self.dir.join(STORE_FILE))?;
            #[cfg(test)]
            self.inject_fault(Fault::AfterRename)?;
            File::open(&self.dir)?.sync_all()?;
            Ok(())
        })();
        // A failure after rename is ambiguous to the caller. The next opener
        // revalidates disk and can return the original receipt on exact retry.
        if result.is_err() {
            let _ = self.discard_pending();
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

    fn discard_pending(&self) -> Result<(), Error> {
        let path = self.dir.join(PENDING_FILE);
        if regular_or_absent(&path)? {
            std::fs::remove_file(path)?;
            File::open(&self.dir)?.sync_all()?;
        }
        Ok(())
    }
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
                },
            );
        }
        Action::Cancel { reason } => {
            let task = tasks.get_mut(&command.task_id).ok_or(Error::NotFound)?;
            if command.expected_revision != Some(task.revision) {
                return Err(Error::RevisionMismatch);
            }
            if task.status != Status::Queued {
                return Err(Error::InvalidTransition);
            }
            task.status = Status::Cancelled;
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
        execution: Execution::NotStarted,
        checks: Checks::NotRun,
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
    if document.schema != STORE_SCHEMA {
        return Err(Error::UnsupportedSchema);
    }
    if document.tasks.len() > MAX_TASKS || document.commands.len() > MAX_COMMANDS {
        return Err(Error::LimitExceeded);
    }
    if document.sequence != document.commands.len() as u64 {
        return Err(Error::Corrupt(
            "the task document sequence does not match its command history",
        ));
    }
    let mut tasks = BTreeMap::new();
    let mut identities = BTreeSet::new();
    for (index, accepted) in document.commands.iter().enumerate() {
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
            index as u64 + 1,
            &mut tasks,
        )
        .map_err(|_| Error::Corrupt("the task command history contains an invalid transition"))?;
        if expected != accepted.receipt {
            return Err(Error::Corrupt(
                "the task receipt does not match its command and transition",
            ));
        }
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
        File::open(ancestor)?.sync_all()?;
    }
    Ok(())
}

#[cfg(unix)]
fn prepare_directory(path: &Path) -> Result<(), Error> {
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
        File::open(directory.parent().ok_or(Error::UnsafePath)?)?.sync_all()?;
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

fn regular_or_absent(path: &Path) -> Result<bool, Error> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(true),
        Ok(_) => Err(Error::UnsafePath),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(Error::Io(error)),
    }
}

#[cfg(unix)]
fn verify_same_file(path: &Path, file: &File) -> Result<(), Error> {
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
fn private_open(path: &Path, create: bool, write: bool) -> Result<File, Error> {
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

#[cfg(not(unix))]
fn prepare_directory(_: &Path) -> Result<(), Error> {
    Err(Error::UnsupportedPlatform)
}
#[cfg(not(unix))]
fn verify_directory(_: &Path) -> Result<(), Error> {
    Err(Error::UnsupportedPlatform)
}
#[cfg(not(unix))]
fn verify_same_file(_: &Path, _: &File) -> Result<(), Error> {
    Err(Error::UnsupportedPlatform)
}
#[cfg(not(unix))]
fn private_open(_: &Path, _: bool, _: bool) -> Result<File, Error> {
    Err(Error::UnsupportedPlatform)
}

#[cfg(test)]
std::thread_local! {
    static OPEN_SYNC_FAIL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    BeforeRename,
    AfterRename,
}

#[cfg(test)]
mod tests;
