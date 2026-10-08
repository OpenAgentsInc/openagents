//! Protected browser admission and exact native request packets, without domain state.

use super::session::{SessionError, now};
use coder_access::client::Pending;
use coder_access::protocol::{Operation, Outcome, ReplyResult};
use coder_access::{Code, Right};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::future::Future;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, SessionError>;
const MAX_RECORD: usize = 256 * 1024;
const MAX_SCOPE: usize = 16 * 1024;
const MAX_ENTRIES: usize = 4096;
const SCHEMA: &str = "openagents.cloud.effect.v1";

/// The current actor and binding scope is supplied by the authenticated adapter.
/// No key, session bearer, private task store, or ambient credential is admitted.
pub(crate) struct Effects {
    directory: Directory,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum State {
    Prepared,
    Unknown,
    Answered,
    Refused,
}

impl State {
    fn terminal(self) -> bool {
        matches!(self, Self::Answered | Self::Refused)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Refusal {
    pub code: Code,
    pub missing: Option<Right>,
}

impl Refusal {
    fn valid(&self) -> bool {
        (self.code == Code::MissingRight) == self.missing.is_some()
    }
    fn definitive(&self) -> bool {
        !matches!(self.code, Code::Unavailable | Code::Transport)
    }
}

/// A scoped projection. The signed packet remains in the protected journal.
#[derive(Clone)]
pub(crate) struct Snapshot {
    pub id: String,
    pub packet_digest: String,
    pub action: Operation,
    pub expires_at: u64,
    pub state: State,
    pub outcome: Option<Outcome>,
    pub refusal: Option<Refusal>,
    pub failure: Option<SessionError>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    scope: Value,
    id: String,
    action: Operation,
    action_digest: String,
    pending: Pending,
    state: State,
    outcome: Option<Outcome>,
    refusal: Option<Refusal>,
    failure: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Enrollment {
    schema: String,
    scope: Value,
    descriptor_digest: String,
}

impl Record {
    fn validate(&self, scope: &Value, id: &str) -> Result<()> {
        if self.scope != *scope || self.id != id {
            return Err(SessionError::Forbidden);
        }
        if self.schema != SCHEMA
            || self.pending.request.request != id
            || self.pending.request.op != self.action
            || self.action_digest != action_digest(&self.action)?
            || !allowed(&self.action)
            || self.action.validate().is_err()
            || self.pending.request.expires_at <= self.pending.request.issued_at
            || self
                .failure
                .as_deref()
                .is_some_and(|code| failure(code).is_none())
            || (self.state == State::Answered) != self.outcome.is_some()
            || self.state == State::Refused
                && !self.refusal.as_ref().is_some_and(Refusal::definitive)
            || self.refusal.as_ref().is_some_and(|refusal| {
                !refusal.valid()
                    || !matches!(self.state, State::Unknown | State::Refused)
                    || (self.state == State::Refused) != refusal.definitive()
                    || self.failure.is_some()
            })
            || self.state == State::Prepared && self.failure.is_some()
            || self.state.terminal() && self.failure.is_some()
        {
            return Err(SessionError::Unavailable);
        }
        if let Some(outcome) = &self.outcome {
            if outcome.validate().is_err()
                || !outcome.answers(&self.action)
                || coder_access::task_read::bounded(outcome, 64 * 1024).is_err()
            {
                return Err(SessionError::Unavailable);
            }
        }
        Ok(())
    }
    fn snapshot(&self) -> Snapshot {
        let expired = !self.state.terminal() && self.pending.request.expires_at <= now();
        Snapshot {
            id: self.id.clone(),
            packet_digest: self.pending.event.id.clone(),
            action: self.action.clone(),
            expires_at: self.pending.request.expires_at,
            state: if expired { State::Unknown } else { self.state },
            outcome: self.outcome.clone(),
            refusal: self.refusal.clone(),
            failure: if expired && self.refusal.is_none() {
                Some(SessionError::Unavailable)
            } else {
                self.failure.as_deref().and_then(failure)
            },
        }
    }
}

fn failure(code: &str) -> Option<SessionError> {
    [
        SessionError::Unauthenticated,
        SessionError::Forbidden,
        SessionError::Unavailable,
        SessionError::InvalidRequest,
        SessionError::Csrf,
        SessionError::Conflict,
    ]
    .into_iter()
    .find(|error| error.code() == code)
}

fn allowed(operation: &Operation) -> bool {
    matches!(
        operation.name(),
        "task.create"
            | "task.steer"
            | "task.cancel"
            | "task.command.at_revision"
            | "task.queue.at_revision"
            | "task.publish"
    )
}

fn identity(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn descriptor(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(identity)
}
fn digest(value: &impl Serialize) -> Result<String> {
    let bytes = serde_json::to_vec(value).map_err(|_| SessionError::InvalidRequest)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn action_digest(action: &Operation) -> Result<String> {
    digest(action)
}
fn checked_scope(scope: &Value) -> Result<()> {
    if !scope.is_object() || coder_access::task_read::bounded(scope, MAX_SCOPE).is_err() {
        return Err(SessionError::InvalidRequest);
    }
    fn metadata(value: &Value, depth: usize) -> bool {
        if depth > 16 {
            return false;
        }
        match value {
            Value::Object(fields) => fields.iter().all(|(key, value)| {
                key.len() <= 128
                    && !matches!(
                        key.to_ascii_lowercase().as_str(),
                        "key"
                            | "secret"
                            | "token"
                            | "credential"
                            | "cookie"
                            | "authorization"
                            | "api_key"
                            | "password"
                            | "session_bearer"
                            | "device_secret"
                    )
                    && metadata(value, depth + 1)
            }),
            Value::Array(values) => {
                values.len() <= 128 && values.iter().all(|v| metadata(v, depth + 1))
            }
            Value::String(value) => {
                value.len() <= 2048
                    && !value.chars().any(char::is_control)
                    && !value.starts_with("sess_")
                    && !value.starts_with("oak_")
            }
            _ => true,
        }
    }
    if !metadata(scope, 0) {
        return Err(SessionError::InvalidRequest);
    }
    Ok(())
}

impl Effects {
    /// Open an existing explicit private directory and pin its identity.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            directory: Directory::open(path)?,
        })
    }

    fn enrollment_name(scope: &Value, descriptor_digest: &str) -> Result<String> {
        checked_scope(scope)?;
        if !descriptor(descriptor_digest) {
            return Err(SessionError::InvalidRequest);
        }
        Ok(format!(
            "enrollment-{}",
            digest(&(scope, descriptor_digest))?
        ))
    }

    /// Record only the exact metadata the browser reviewed for this actor.
    pub(crate) fn accept_enrollment(&self, scope: &Value, descriptor_digest: &str) -> Result<()> {
        let name = Self::enrollment_name(scope, descriptor_digest)?;
        let _lock = self.directory.lock(&name)?;
        let record = Enrollment {
            schema: "openagents.cloud.enrollment.v1".into(),
            scope: scope.clone(),
            descriptor_digest: descriptor_digest.into(),
        };
        if let Some(bytes) = self.directory.read(&name)? {
            let original: Enrollment = parse(&bytes)?;
            if original.schema != record.schema
                || original.scope != *scope
                || original.descriptor_digest != descriptor_digest
            {
                return Err(SessionError::Conflict);
            }
            return Ok(());
        }
        self.directory.write(&name, &record)
    }

    pub(crate) fn enrolled(&self, scope: &Value, descriptor_digest: &str) -> Result<bool> {
        let name = Self::enrollment_name(scope, descriptor_digest)?;
        let Some(bytes) = self.directory.read(&name)? else {
            return Ok(false);
        };
        let record: Enrollment = parse(&bytes)?;
        if record.schema != "openagents.cloud.enrollment.v1"
            || record.scope != *scope
            || record.descriptor_digest != descriptor_digest
        {
            return Err(SessionError::Unavailable);
        }
        Ok(true)
    }

    /// Persist and sync once before dispatch. A repeated form reuses the packet.
    pub(crate) fn stage(
        &self,
        scope: &Value,
        id: &str,
        action: &Operation,
        prepare: impl FnOnce() -> Result<Pending>,
    ) -> Result<Snapshot> {
        checked_scope(scope)?;
        if !identity(id) || !allowed(action) || action.validate().is_err() {
            return Err(SessionError::InvalidRequest);
        }
        let _lock = self.directory.lock(id)?;
        if let Some(record) = self.record(scope, id)? {
            if record.action != *action {
                return Err(SessionError::Conflict);
            }
            return Ok(record.snapshot());
        }
        let pending = prepare()?;
        let record = Record {
            schema: SCHEMA.into(),
            scope: scope.clone(),
            id: id.into(),
            action: action.clone(),
            action_digest: action_digest(action)?,
            pending,
            state: State::Prepared,
            outcome: None,
            refusal: None,
            failure: None,
        };
        record.validate(scope, id)?;
        self.directory.write(id, &record)?;
        Ok(record.snapshot())
    }

    /// Read-only recovery never changes a packet, scope, or dispatch decision.
    pub(crate) fn lookup(&self, scope: &Value, id: &str) -> Result<Snapshot> {
        checked_scope(scope)?;
        if !identity(id) {
            return Err(SessionError::InvalidRequest);
        }
        let mut record = self.record(scope, id)?.ok_or(SessionError::Forbidden)?;
        if record.state == State::Prepared && record.pending.request.expires_at <= now() {
            record.state = State::Unknown;
            record.failure = Some(SessionError::Unavailable.code().into());
        }
        Ok(record.snapshot())
    }

    fn record(&self, scope: &Value, id: &str) -> Result<Option<Record>> {
        let Some(bytes) = self.directory.read(id)? else {
            return Ok(None);
        };
        let record: Record = parse(&bytes)?;
        record.validate(scope, id)?;
        Ok(Some(record))
    }

    /// Keep an original outcome verified by a current native recovery read.
    /// This resolves uncertainty without preparing or dispatching an effect.
    pub(crate) fn reconcile(&self, scope: &Value, id: &str, outcome: Outcome) -> Result<Snapshot> {
        self.reconcile_reply(scope, id, ReplyResult::Ok { outcome })
    }

    /// Persist an exact result verified by the native request recovery read.
    /// Definitive refusals are terminal. An unavailable native result can
    /// follow an effect, so its signed proof preserves uncertainty.
    pub(crate) fn reconcile_reply(
        &self,
        scope: &Value,
        id: &str,
        reply: ReplyResult,
    ) -> Result<Snapshot> {
        checked_scope(scope)?;
        if !identity(id) {
            return Err(SessionError::InvalidRequest);
        }
        let _lock = self.directory.lock(id)?;
        let mut record = self.record(scope, id)?.ok_or(SessionError::Forbidden)?;
        if coder_access::task_read::bounded(&reply, 64 * 1024).is_err() {
            return Err(SessionError::Conflict);
        }
        let (state, outcome, refusal) = match reply {
            ReplyResult::Ok { outcome }
                if outcome.validate().is_ok() && outcome.answers(&record.action) =>
            {
                (State::Answered, Some(outcome), None)
            }
            ReplyResult::Refused { code, missing } => {
                let refusal = Refusal { code, missing };
                if !refusal.valid() {
                    return Err(SessionError::Conflict);
                }
                let state = if refusal.definitive() {
                    State::Refused
                } else {
                    State::Unknown
                };
                (state, None, Some(refusal))
            }
            _ => return Err(SessionError::Conflict),
        };
        if record.state.terminal() {
            return if record.state == state
                && record.outcome == outcome
                && record.refusal == refusal
            {
                Ok(record.snapshot())
            } else {
                Err(SessionError::Conflict)
            };
        }
        record.state = state;
        record.outcome = outcome;
        record.refusal = refusal;
        record.failure = None;
        self.directory.write(id, &record)?;
        Ok(record.snapshot())
    }

    /// An explicit retry sends only the original still-fresh signed packet.
    /// Native admission and the operation's durable owner remain authoritative.
    pub(crate) async fn dispatch<F, Fut>(
        &self,
        scope: &Value,
        id: &str,
        send: F,
    ) -> Result<Snapshot>
    where
        F: FnOnce(Pending) -> Fut,
        Fut: Future<Output = Result<Outcome>>,
    {
        checked_scope(scope)?;
        if !identity(id) {
            return Err(SessionError::InvalidRequest);
        }
        let _lock = self.directory.lock(id)?;
        let mut record = self.record(scope, id)?.ok_or(SessionError::Forbidden)?;
        if record.state.terminal() || record.refusal.is_some() {
            return Ok(record.snapshot());
        }
        // A restart or a cancelled HTTP request finds uncertainty, never a
        // fresh packet. Sync this marker before handing bytes to a transport.
        record.state = State::Unknown;
        record.failure = Some(SessionError::Unavailable.code().into());
        self.directory.write(id, &record)?;
        if record.pending.request.expires_at <= now() {
            return Ok(record.snapshot());
        }
        match send(record.pending.clone()).await {
            Ok(outcome)
                if outcome.validate().is_ok()
                    && outcome.answers(&record.action)
                    && coder_access::task_read::bounded(&outcome, 64 * 1024).is_ok() =>
            {
                record.state = State::Answered;
                record.outcome = Some(outcome);
                record.failure = None;
            }
            Ok(_) => record.failure = Some(SessionError::Unavailable.code().into()),
            Err(error) => record.failure = Some(error.code().into()),
        }
        self.directory.write(id, &record)?;
        Ok(record.snapshot())
    }
}

fn parse<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|_| SessionError::Unavailable)
}

struct Directory {
    path: PathBuf,
    file: File,
}

#[cfg(unix)]
impl Directory {
    fn open(path: &Path) -> Result<Self> {
        use std::ffi::CString;
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::MetadataExt;
        use std::path::Component;
        if !path.is_absolute() {
            return Err(SessionError::Unavailable);
        }
        let parts = path.components().collect::<Vec<_>>();
        if parts.len() < 2 || parts[0] != Component::RootDir {
            return Err(SessionError::Unavailable);
        }
        let user = unsafe { libc::geteuid() };
        let mut directory = File::open("/").map_err(|_| SessionError::Unavailable)?;
        for (index, component) in parts[1..].iter().enumerate() {
            let Component::Normal(name) = component else {
                return Err(SessionError::Unavailable);
            };
            let parent = directory
                .metadata()
                .map_err(|_| SessionError::Unavailable)?;
            let sticky = parent.uid() == 0 && parent.mode() & 0o1000 != 0;
            if !parent.is_dir()
                || parent.uid() != 0 && parent.uid() != user
                || parent.mode() & 0o022 != 0 && !sticky
            {
                return Err(SessionError::Unavailable);
            }
            let name = CString::new(name.as_bytes()).map_err(|_| SessionError::Unavailable)?;
            let descriptor = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if descriptor < 0 {
                return Err(SessionError::Unavailable);
            }
            directory = unsafe { File::from_raw_fd(descriptor) };
            if index + 2 == parts.len() {
                let metadata = directory
                    .metadata()
                    .map_err(|_| SessionError::Unavailable)?;
                if metadata.uid() != user || metadata.mode() & 0o077 != 0 {
                    return Err(SessionError::Unavailable);
                }
            }
        }
        Ok(Self {
            path: path.into(),
            file: directory,
        })
    }
    fn check(&self) -> Result<()> {
        use std::os::unix::fs::MetadataExt;
        let current = Self::open(&self.path)?;
        let original = self
            .file
            .metadata()
            .map_err(|_| SessionError::Unavailable)?;
        let observed = current
            .file
            .metadata()
            .map_err(|_| SessionError::Unavailable)?;
        if original.dev() != observed.dev() || original.ino() != observed.ino() {
            return Err(SessionError::Unavailable);
        }
        Ok(())
    }
    fn file(&self, name: &str, flags: libc::c_int) -> Result<Option<File>> {
        use std::ffi::CString;
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::fs::MetadataExt;
        self.check()?;
        let name = CString::new(name).map_err(|_| SessionError::Unavailable)?;
        let descriptor = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                0o600,
            )
        };
        if descriptor < 0 {
            return if std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(SessionError::Unavailable)
            };
        }
        let file = unsafe { File::from_raw_fd(descriptor) };
        let metadata = file.metadata().map_err(|_| SessionError::Unavailable)?;
        if !metadata.is_file()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
            || metadata.nlink() != 1
            || metadata.len() > MAX_RECORD as u64
        {
            return Err(SessionError::Unavailable);
        }
        Ok(Some(file))
    }
    fn lock(&self, id: &str) -> Result<Lock> {
        use std::os::fd::AsRawFd;
        let name = format!("{id}.lock");
        if self.file(&name, libc::O_RDWR)?.is_none()
            && std::fs::read_dir(&self.path)
                .map_err(|_| SessionError::Unavailable)?
                .take(MAX_ENTRIES)
                .count()
                >= MAX_ENTRIES
        {
            return Err(SessionError::Unavailable);
        }
        let file = self
            .file(&name, libc::O_RDWR | libc::O_CREAT)?
            .ok_or(SessionError::Unavailable)?;
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::WouldBlock {
                    SessionError::Conflict
                } else {
                    SessionError::Unavailable
                },
            );
        }
        Ok(Lock(file))
    }
    fn read(&self, id: &str) -> Result<Option<Vec<u8>>> {
        use std::io::Read;
        let Some(file) = self.file(&format!("{id}.json"), libc::O_RDONLY)? else {
            return Ok(None);
        };
        let mut bytes = Vec::new();
        file.take(MAX_RECORD as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| SessionError::Unavailable)?;
        if bytes.len() > MAX_RECORD {
            return Err(SessionError::Unavailable);
        }
        Ok(Some(bytes))
    }
    fn write(&self, id: &str, value: &impl Serialize) -> Result<()> {
        use std::ffi::CString;
        use std::io::Write;
        use std::os::fd::AsRawFd;
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        self.check()?;
        if std::fs::read_dir(&self.path)
            .map_err(|_| SessionError::Unavailable)?
            .take(MAX_ENTRIES + 1)
            .count()
            > MAX_ENTRIES
        {
            return Err(SessionError::Unavailable);
        }
        coder_access::task_read::bounded(value, MAX_RECORD)
            .map_err(|_| SessionError::Unavailable)?;
        let bytes = serde_json::to_vec(value).map_err(|_| SessionError::Unavailable)?;
        let temporary = format!(
            ".{id}.{}-{}.pending",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let mut file = self
            .file(&temporary, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL)?
            .ok_or(SessionError::Unavailable)?;
        let source = CString::new(temporary).map_err(|_| SessionError::Unavailable)?;
        let target = CString::new(format!("{id}.json")).map_err(|_| SessionError::Unavailable)?;
        let saved = file
            .write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| SessionError::Unavailable)
            .and_then(|_| {
                self.check()?;
                if unsafe {
                    libc::renameat(
                        self.file.as_raw_fd(),
                        source.as_ptr(),
                        self.file.as_raw_fd(),
                        target.as_ptr(),
                    )
                } != 0
                {
                    return Err(SessionError::Unavailable);
                }
                self.file.sync_all().map_err(|_| SessionError::Unavailable)
            });
        if saved.is_err() {
            unsafe {
                libc::unlinkat(self.file.as_raw_fd(), source.as_ptr(), 0);
            }
        }
        saved
    }
}

#[cfg(unix)]
struct Lock(File);
#[cfg(unix)]
impl Drop for Lock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

#[cfg(not(unix))]
struct Lock;
#[cfg(not(unix))]
impl Directory {
    fn open(_: &Path) -> Result<Self> {
        Err(SessionError::Unavailable)
    }
    fn lock(&self, _: &str) -> Result<Lock> {
        Err(SessionError::Unavailable)
    }
    fn read(&self, _: &str) -> Result<Option<Vec<u8>>> {
        Err(SessionError::Unavailable)
    }
    fn write(&self, _: &str, _: &impl Serialize) -> Result<()> {
        Err(SessionError::Unavailable)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use coder_access::protocol::{Receipt, TaskCreate};
    use coder_access::{Client, RelayPolicy};
    use secp256k1::SecretKey;
    use serde_json::json;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture {
        _root: tempfile::TempDir,
        path: PathBuf,
        effects: Effects,
        client: Client,
        scope: Value,
        id: String,
        action: Operation,
    }
    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().canonicalize().unwrap().join("effects");
            std::fs::create_dir(&path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            let host = SecretKey::new(&mut secp256k1::rand::rng());
            let client = Client::owner(
                &secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &host)
                    .x_only_public_key()
                    .0
                    .to_string(),
                "wss://relay.example.com/",
                SecretKey::new(&mut secp256k1::rand::rng()),
                RelayPolicy::Production,
            )
            .unwrap();
            let scope = json!({"account":"synthetic-a","workspace":"synthetic-workspace",
                "session":"synthetic-session","members_epoch":4,"generation":3,
                "binding_identity":format!("sha256:{}","a".repeat(64))});
            let action = Operation::CreateTask {
                task: TaskCreate {
                    title: "Synthetic exact action".into(),
                    prompt: "Original action bytes.".into(),
                    workspace: "fixture".into(),
                    images: vec![],
                    engine: None,
                },
            };
            Self {
                effects: Effects::open(&path).unwrap(),
                _root: root,
                path,
                client,
                scope,
                id: "b".repeat(64),
                action,
            }
        }
        fn stage(&self) -> Snapshot {
            self.effects
                .stage(&self.scope, &self.id, &self.action, || {
                    self.client
                        .prepare_with_id(self.action.clone(), now(), self.id.clone())
                        .map_err(|_| SessionError::Unavailable)
                })
                .unwrap()
        }
        fn answer(&self) -> Outcome {
            Outcome::Dispatched {
                receipt: Receipt {
                    operation: self.action.name().into(),
                    reference: self.id.clone(),
                },
            }
        }
    }

    #[test]
    fn enrollment_is_exact_private_metadata_and_never_ambient_credentials() {
        let fixture = Fixture::new();
        let descriptor = format!("sha256:{}", "c".repeat(64));
        assert!(
            !fixture
                .effects
                .enrolled(&fixture.scope, &descriptor)
                .unwrap()
        );
        fixture
            .effects
            .accept_enrollment(&fixture.scope, &descriptor)
            .unwrap();
        assert!(
            Effects::open(&fixture.path)
                .unwrap()
                .enrolled(&fixture.scope, &descriptor)
                .unwrap()
        );
        let mut changed = fixture.scope.clone();
        changed["session"] = json!("new-session");
        assert!(!fixture.effects.enrolled(&changed, &descriptor).unwrap());
        changed = fixture.scope.clone();
        changed["credential"] = json!("credential-canary");
        assert!(matches!(
            fixture.effects.accept_enrollment(&changed, &descriptor),
            Err(SessionError::InvalidRequest)
        ));
    }

    #[test]
    fn identical_action_reuses_the_original_packet_and_changed_bytes_conflict() {
        let fixture = Fixture::new();
        let first = fixture.stage();
        let reopened = Effects::open(&fixture.path).unwrap();
        let second = reopened
            .stage(&fixture.scope, &fixture.id, &fixture.action, || {
                panic!("never prepare again")
            })
            .unwrap();
        assert_eq!(first.packet_digest, second.packet_digest);
        let mut changed = fixture.action.clone();
        let Operation::CreateTask { task } = &mut changed else {
            panic!("create")
        };
        task.prompt = "Replacement bytes".into();
        assert!(matches!(
            reopened.stage(&fixture.scope, &fixture.id, &changed, || panic!(
                "no replacement"
            )),
            Err(SessionError::Conflict)
        ));
        let mut other = fixture.scope.clone();
        other["account"] = json!("synthetic-b");
        assert!(matches!(
            reopened.lookup(&other, &fixture.id),
            Err(SessionError::Forbidden)
        ));
        assert!(matches!(
            reopened.lookup(&fixture.scope, "../../file"),
            Err(SessionError::InvalidRequest)
        ));
    }

    #[tokio::test]
    async fn lost_reply_and_restart_retry_only_saved_bytes_and_answer_once() {
        let fixture = Fixture::new();
        let first = fixture.stage();
        let packet = first.packet_digest.clone();
        let failed = fixture
            .effects
            .dispatch(&fixture.scope, &fixture.id, |pending| async move {
                assert_eq!(pending.event.id, packet);
                Err(SessionError::Unavailable)
            })
            .await
            .unwrap();
        assert!(failed.state == State::Unknown);
        let reopened = Effects::open(&fixture.path).unwrap();
        let answer = fixture.answer();
        let packet = first.packet_digest;
        let recovered = reopened
            .dispatch(&fixture.scope, &fixture.id, |pending| async move {
                assert_eq!(pending.event.id, packet);
                Ok(answer)
            })
            .await
            .unwrap();
        assert!(recovered.state == State::Answered && recovered.outcome.is_some());
        let same = reopened
            .dispatch(&fixture.scope, &fixture.id, |_| async {
                panic!("answered requests never dispatch")
            })
            .await
            .unwrap();
        assert_eq!(same.packet_digest, recovered.packet_digest);
    }

    #[tokio::test]
    async fn cancelled_dispatch_stays_unknown_and_an_expired_packet_never_sends() {
        let fixture = Fixture::new();
        fixture.stage();
        let cancelled = tokio::time::timeout(
            std::time::Duration::from_millis(10),
            fixture
                .effects
                .dispatch(&fixture.scope, &fixture.id, |_| std::future::pending()),
        )
        .await;
        assert!(cancelled.is_err());
        assert!(
            Effects::open(&fixture.path)
                .unwrap()
                .lookup(&fixture.scope, &fixture.id)
                .unwrap()
                .state
                == State::Unknown
        );
        let mut record = fixture
            .effects
            .record(&fixture.scope, &fixture.id)
            .unwrap()
            .unwrap();
        record.pending.request.issued_at = now() - 120;
        record.pending.request.expires_at = now() - 60;
        fixture
            .effects
            .directory
            .write(&fixture.id, &record)
            .unwrap();
        let result = fixture
            .effects
            .dispatch(&fixture.scope, &fixture.id, |_| async {
                panic!("expired request sent")
            })
            .await
            .unwrap();
        assert!(result.state == State::Unknown);
        let recovered = fixture
            .effects
            .reconcile(&fixture.scope, &fixture.id, fixture.answer())
            .unwrap();
        assert!(recovered.state == State::Answered);
    }

    #[tokio::test]
    async fn separate_replicas_cannot_dispatch_the_same_request_concurrently() {
        let fixture = Fixture::new();
        fixture.stage();
        let replica = Effects::open(&fixture.path).unwrap();
        let started = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(tokio::sync::Notify::new());
        let send = fixture.effects.dispatch(&fixture.scope, &fixture.id, |_| {
            let started = started.clone();
            let release = release.clone();
            let answer = fixture.answer();
            async move {
                started.fetch_add(1, Ordering::SeqCst);
                release.notified().await;
                Ok(answer)
            }
        });
        let check = async {
            while started.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
            assert!(matches!(
                replica
                    .dispatch(&fixture.scope, &fixture.id, |_| async {
                        panic!("duplicate dispatch")
                    })
                    .await,
                Err(SessionError::Conflict)
            ));
            release.notify_one();
        };
        let (answer, _) = tokio::join!(send, check);
        assert!(answer.unwrap().state == State::Answered);
    }

    #[tokio::test]
    async fn verified_refusal_is_terminal_and_cannot_be_replaced_or_replayed() {
        let fixture = Fixture::new();
        fixture.stage();
        for (code, missing) in [
            (Code::MissingRight, None),
            (Code::Forbidden, Some(Right::Operate)),
        ] {
            assert!(matches!(
                fixture.effects.reconcile_reply(
                    &fixture.scope,
                    &fixture.id,
                    ReplyResult::Refused { code, missing }
                ),
                Err(SessionError::Conflict)
            ));
        }
        let refusal = ReplyResult::Refused {
            code: Code::MissingRight,
            missing: Some(Right::Operate),
        };
        let result = fixture
            .effects
            .reconcile_reply(&fixture.scope, &fixture.id, refusal.clone())
            .unwrap();
        assert!(result.state == State::Refused);
        assert!(result.outcome.is_none() && result.failure.is_none());
        assert!(result.refusal.as_ref().unwrap().code == Code::MissingRight);
        assert!(
            fixture
                .effects
                .reconcile_reply(&fixture.scope, &fixture.id, refusal)
                .unwrap()
                .state
                == State::Refused
        );
        assert!(matches!(
            fixture
                .effects
                .reconcile(&fixture.scope, &fixture.id, fixture.answer()),
            Err(SessionError::Conflict)
        ));
        let reopened = Effects::open(&fixture.path).unwrap();
        let result = reopened
            .dispatch(&fixture.scope, &fixture.id, |_| async {
                panic!("refused requests never dispatch")
            })
            .await
            .unwrap();
        assert!(result.state == State::Refused && result.refusal.is_some());
        let mut record = reopened
            .record(&fixture.scope, &fixture.id)
            .unwrap()
            .unwrap();
        record.pending.request.issued_at = now() - 120;
        record.pending.request.expires_at = now() - 60;
        reopened.directory.write(&fixture.id, &record).unwrap();
        assert!(reopened.lookup(&fixture.scope, &fixture.id).unwrap().state == State::Refused);
    }

    #[tokio::test]
    async fn signed_native_unavailability_keeps_effect_uncertainty_without_redispatch() {
        for code in [Code::Unavailable, Code::Transport] {
            let fixture = Fixture::new();
            fixture.stage();
            let result = fixture
                .effects
                .reconcile_reply(
                    &fixture.scope,
                    &fixture.id,
                    ReplyResult::Refused {
                        code,
                        missing: None,
                    },
                )
                .unwrap();
            assert!(result.state == State::Unknown && result.outcome.is_none());
            assert!(result.refusal.as_ref().unwrap().code == code);
            let reopened = Effects::open(&fixture.path).unwrap();
            let result = reopened
                .dispatch(&fixture.scope, &fixture.id, |_| async {
                    panic!("native uncertainty never permits redispatch")
                })
                .await
                .unwrap();
            assert!(result.state == State::Unknown && result.refusal.is_some());
        }
    }

    #[test]
    fn protected_directory_refuses_links_widening_and_replacement() {
        let fixture = Fixture::new();
        fixture.stage();
        let linked = fixture.path.with_file_name("linked-effects");
        symlink(&fixture.path, &linked).unwrap();
        assert!(matches!(
            Effects::open(&linked),
            Err(SessionError::Unavailable)
        ));
        let record = fixture.path.join(format!("{}.json", fixture.id));
        std::fs::set_permissions(&record, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            fixture.effects.lookup(&fixture.scope, &fixture.id),
            Err(SessionError::Unavailable)
        ));
        std::fs::set_permissions(&record, std::fs::Permissions::from_mode(0o600)).unwrap();
        let original = fixture.path.with_file_name("original-effects");
        std::fs::rename(&fixture.path, &original).unwrap();
        std::fs::create_dir(&fixture.path).unwrap();
        std::fs::set_permissions(&fixture.path, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(matches!(
            fixture.effects.lookup(&fixture.scope, &fixture.id),
            Err(SessionError::Unavailable)
        ));
    }
}
