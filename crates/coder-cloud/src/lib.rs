//! Durable remote jobs shared by Coder's terminal and CLI.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub mod boat_backend;
pub mod claude;
pub mod claude_task;
pub mod gce_backend;
pub mod operator;
mod operator_adapters;
pub mod pool;
pub mod release;
pub mod runtime;
pub mod workspace;

pub type Result<T> = std::result::Result<T, String>;
pub const MAX_RECORD_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_EVENT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    Boat,
    Gce,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Integrated,
    Coder,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spec {
    pub placement: Placement,
    pub mode: Mode,
    pub agent: String,
    pub task: String,
    pub model: Option<String>,
    pub reasoning: Option<String>,
    pub cwd: PathBuf,
    pub timeout_seconds: u64,
    pub size: String,
    pub template: Option<String>,
    /// Only explicitly selected credential names, never their values.
    pub credential_names: Vec<String>,
}
impl Spec {
    pub fn validate(&self) -> Result<()> {
        if self.task.is_empty() || self.task.len() > 1024 * 1024 || self.task.contains('\0') {
            return Err("A remote task must contain 1 byte to 1 MiB of text.".into());
        }
        validate_id(&self.agent)?;
        if self.timeout_seconds == 0 || self.timeout_seconds > 12 * 3600 {
            return Err("The remote deadline must be between 1 second and 12 hours.".into());
        }
        if self.placement == Placement::Gce && self.mode == Mode::Integrated {
            return Err("GCE runs the Coder runtime; integrated agents require Boat.".into());
        }
        if self.placement == Placement::Gce && (self.template.is_some() || self.size != "default") {
            return Err("GCE uses its granted pool shape and image. Configure the pool with cloud up; size and template options select Boat resources.".into());
        }
        for name in &self.credential_names {
            if name.is_empty()
                || !name
                    .bytes()
                    .next()
                    .is_some_and(|c| c.is_ascii_uppercase() || c == b'_')
                || name.len() > 128
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
            {
                return Err(
                    "Credential names must be uppercase environment variable names.".into(),
                );
            }
            // CLAUDE_CODE_OAUTH_TOKEN names the user's own subscription
            // token (#11204); its value is admitted in `Credentials`.
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Created,
    Provisioning,
    Resuming,
    Ready,
    Dispatching,
    Running,
    /// A Claude Code usage limit stopped the turn; the operator continues
    /// it after the reset Claude Code reported ([`claude_task`]).
    Paused,
    Completed,
    Failed,
    Cancelled,
}
impl State {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub conversation: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub schema: String,
    pub id: String,
    pub spec: Spec,
    pub state: State,
    pub created_ms: u64,
    pub updated_ms: u64,
    pub resource: Option<String>,
    pub remote_task: Option<Task>,
    pub cursor: Option<String>,
    pub events: Vec<Value>,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub usage: Option<Value>,
    pub cleanup_complete: bool,
    pub cleanup_error: Option<String>,
    #[serde(default)]
    pub cancel_requested: bool,
    /// Backend-specific references, excluding credentials.
    #[serde(default)]
    pub binding: Value,
    #[serde(default)]
    pub workspace: Option<workspace::Snapshot>,
    #[serde(default)]
    pub artifacts: Option<workspace::Artifacts>,
    #[serde(default)]
    pub artifact_error: Option<String>,
    #[serde(default)]
    pub turns: Vec<Value>,
    /// The exact saved environment version this job started with, resolved
    /// once at admission (ENV-06). Later selection changes never alter it;
    /// a startup failure on its image is a failure, never a fallback to
    /// the base template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<coder_environment::VersionPin>,
}
impl Record {
    pub fn new(id: &str, spec: Spec) -> Result<Self> {
        validate_id(id)?;
        spec.validate()?;
        Ok(Self {
            schema: "openagents.coder.remote-job.v1".into(),
            id: id.into(),
            spec,
            state: State::Created,
            created_ms: now_ms(),
            updated_ms: now_ms(),
            resource: None,
            remote_task: None,
            cursor: None,
            events: vec![],
            result: None,
            error: None,
            usage: None,
            cleanup_complete: false,
            cleanup_error: None,
            cancel_requested: false,
            binding: Value::Null,
            workspace: None,
            artifacts: None,
            artifact_error: None,
            turns: vec![],
            environment: None,
        })
    }
}
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err("Remote job and agent IDs accept letters, numbers, underscores, and hyphens, up to 128 bytes.".into());
    }
    Ok(())
}

/// One private file per job, replaced atomically with its committed event cursor.
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
}
pub struct Lease {
    path: PathBuf,
    _lock: File,
}
impl Store {
    pub fn under(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    fn path(&self, id: &str) -> Result<PathBuf> {
        validate_id(id)?;
        Ok(self.root.join(format!("{id}.json")))
    }
    pub fn lease(&self, id: &str) -> Result<Lease> {
        let path = self.path(id)?;
        fs::create_dir_all(&self.root).map_err(|_| "Cannot create the remote job store.")?;
        protect_dir(&self.root)?;
        let lock_path = path.with_extension("lock");
        regular_or_missing(&lock_path)?;
        let file = private_options()
            .create(true)
            .read(true)
            .write(true)
            .open(lock_path)
            .map_err(|_| "Cannot open the remote job lock.")?;
        file.try_lock()
            .map_err(|_| "Another process is using this remote job.")?;
        Ok(Lease { path, _lock: file })
    }
    pub fn read(&self, id: &str) -> Result<Record> {
        read_record(&self.path(id)?, id)
    }
    pub fn list(&self) -> Result<Vec<Record>> {
        if !self.root.exists() {
            return Ok(vec![]);
        }
        let mut rows = vec![];
        for entry in fs::read_dir(&self.root).map_err(|_| "Cannot list remote jobs.")? {
            let p = entry
                .map_err(|_| "Cannot read the remote job directory.")?
                .path();
            if p.extension().and_then(|v| v.to_str()) != Some("json") {
                continue;
            }
            let id = p
                .file_stem()
                .and_then(|v| v.to_str())
                .ok_or("Invalid remote job file.")?;
            rows.push(read_record(&p, id)?);
        }
        rows.sort_by_key(|r| std::cmp::Reverse(r.created_ms));
        Ok(rows)
    }
    /// Cancellation is independent of the driver's writer lock.
    pub fn cancel(&self, id: &str) -> Result<()> {
        let record_path = self.path(id)?;
        let _snapshot = snapshot_lock(&record_path)?;
        let path = record_path.with_extension("cancel");
        let record = self.read(id)?;
        if record.cleanup_complete {
            return Ok(());
        }
        regular_or_missing(&path)?;
        if path.exists() {
            return Ok(());
        }
        private_options()
            .create_new(true)
            .write(true)
            .open(path)
            .and_then(|f| f.sync_all())
            .map_err(|_| "Cannot request remote cancellation.".into())
    }
    /// Request cancellation only if the canonical record still matches the
    /// reviewed snapshot. This short fence also serializes every record save.
    pub fn cancel_exact(&self, id: &str, expected_digest: &str) -> Result<()> {
        self.cancel_exact_evidence(id, expected_digest, &Value::Null)
    }
    pub fn cancel_exact_evidence(
        &self,
        id: &str,
        expected_digest: &str,
        evidence: &Value,
    ) -> Result<()> {
        let path = self.path(id)?;
        let _snapshot = snapshot_lock(&path)?;
        let bytes = workspace::read_bounded(&path, MAX_RECORD_BYTES)?;
        if workspace::digest(&bytes) != expected_digest {
            return Err("The cloud job snapshot changed before cancellation.".into());
        }
        let bytes = serde_json::to_vec(evidence)
            .map_err(|_| "Cannot encode cloud cancellation evidence.")?;
        if bytes.len() > 16 * 1024 {
            return Err("Cloud cancellation evidence exceeds its limit.".into());
        }
        let marker = path.with_extension("cancel");
        regular_or_missing(&marker)?;
        if marker.exists() {
            let original = workspace::read_bounded(&marker, 16 * 1024)?;
            if evidence.is_null() || original == bytes {
                return Ok(());
            }
            return Err("The cloud cancellation evidence already names another request.".into());
        }
        private_options()
            .create_new(true)
            .write(true)
            .open(marker)
            .and_then(|mut f| f.write_all(&bytes).and_then(|_| f.sync_all()))
            .map_err(|_| "Cannot retain cloud cancellation evidence.".into())
    }
    pub fn cancellation_requested(&self, id: &str) -> Result<bool> {
        let path = self.path(id)?.with_extension("cancel");
        regular_or_missing(&path)?;
        Ok(path.exists())
    }
    pub fn cancellation_evidence(&self, id: &str) -> Result<Option<Vec<u8>>> {
        let path = self.path(id)?.with_extension("cancel");
        regular_or_missing(&path)?;
        if path.exists() {
            workspace::read_bounded(&path, 16 * 1024).map(Some)
        } else {
            Ok(None)
        }
    }
}
fn snapshot_lock(path: &Path) -> Result<File> {
    let lock = path.with_extension("snapshot.lock");
    regular_or_missing(&lock)?;
    let file = private_options()
        .create(true)
        .read(true)
        .write(true)
        .open(lock)
        .map_err(|_| "Cannot open the cloud snapshot fence.")?;
    file.lock()
        .map_err(|_| "Cannot hold the cloud snapshot fence.")?;
    Ok(file)
}
impl Lease {
    pub fn read(&self, id: &str) -> Result<Record> {
        read_record(&self.path, id)
    }
    pub fn file(&self, name: &str) -> Result<PathBuf> {
        if !matches!(
            name,
            "input.json"
                | "changes.patch"
                | "events.ndjson"
                | "trajectory.atif.json"
                | "result.json"
                | "manifest.json"
        ) {
            return Err("Invalid remote artifact path.".into());
        }
        let root = self.path.parent().unwrap().join(format!(
            "{}.artifacts",
            self.path.file_stem().unwrap().to_string_lossy()
        ));
        if fs::symlink_metadata(&root).is_ok_and(|m| !m.is_dir() || m.file_type().is_symlink()) {
            return Err("The artifact directory must be a regular directory.".into());
        }
        fs::create_dir_all(&root).map_err(|_| "Cannot create the artifact directory.")?;
        protect_dir(&root)?;
        Ok(root.join(name))
    }
    pub fn clear_cancel(&self) -> Result<()> {
        match fs::remove_file(self.path.with_extension("cancel")) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("Cannot clear the remote cancellation marker.".into()),
        }
    }
    pub fn exists(&self) -> bool {
        self.path.exists()
    }
    pub fn cancelled(&self) -> bool {
        self.path.with_extension("cancel").exists()
    }
    pub fn save(&self, record: &Record) -> Result<()> {
        let _snapshot = snapshot_lock(&self.path)?;
        validate_id(&record.id)?;
        if self.path.file_stem().and_then(|v| v.to_str()) != Some(&record.id) {
            return Err("Remote job identity mismatch.".into());
        }
        record.spec.validate()?;
        let bytes = serde_json::to_vec(record).map_err(|_| "Cannot encode the remote job.")?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err("The remote job exceeds its retained output limit.".into());
        }
        regular_or_missing(&self.path)?;
        let temp = self.path.with_extension("writing");
        regular_or_missing(&temp)?;
        let mut file = private_options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&temp)
            .map_err(|_| "Cannot write the remote job.")?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Cannot retain the remote job.")?;
        fs::rename(temp, &self.path).map_err(|_| "Cannot replace the remote job.")?;
        File::open(self.path.parent().unwrap())
            .and_then(|f| f.sync_all())
            .map_err(|_| "Cannot sync the remote job directory.".into())
    }
}
fn private_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}
fn protect_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Cannot protect the remote job directory.")?;
    }
    Ok(())
}
fn regular_or_missing(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => Ok(()),
        Ok(_) => Err("Remote job files must be regular files.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Cannot inspect a remote job file.".into()),
    }
}
fn read_record(path: &Path, id: &str) -> Result<Record> {
    regular_or_missing(path)?;
    let mut bytes = vec![];
    File::open(path)
        .and_then(|f| f.take(MAX_RECORD_BYTES as u64 + 1).read_to_end(&mut bytes))
        .map_err(|_| "Cannot read the remote job.")?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err("The remote job exceeds its retained output limit.".into());
    }
    let record: Record =
        serde_json::from_slice(&bytes).map_err(|_| "Cannot decode the remote job.")?;
    if record.binding["turn_start"]
        .as_u64()
        .is_some_and(|n| n > record.events.len() as u64)
    {
        return Err("Invalid retained turn cursor.".into());
    }
    if let Some(s) = &record.workspace {
        let expected = path
            .parent()
            .unwrap()
            .join(format!("{id}.artifacts/input.json"));
        if s.input_path != expected
            || !s
                .working_directory
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
        {
            return Err("Invalid retained workspace path.".into());
        }
    }
    if record.schema != "openagents.coder.remote-job.v1" || record.id != id {
        return Err("Remote job schema or identity mismatch.".into());
    }
    record.spec.validate()?;
    Ok(record)
}

#[derive(Default)]
pub struct Observation {
    pub events: Vec<Value>,
    pub cursor: Option<String>,
    pub end: Option<std::result::Result<Value, String>>,
}
/// Backend methods retain their own bounded network and process deadlines.
#[allow(async_fn_in_trait)]
pub trait Backend {
    /// Provisioning must use the retained job ID as its idempotency identity.
    async fn provision(&self, record: &mut Record) -> Result<String>;
    /// Restore files and prepare the workspace before committing a dispatch intent.
    /// Resolve image defaults before persisting the provisioning intent.
    async fn resolve(&self, _record: &mut Record) -> Result<()> {
        Ok(())
    }
    async fn prepare(&self, _record: &Record) -> Result<()> {
        Ok(())
    }
    async fn dispatch(&self, record: &Record) -> Result<Task>;
    /// Read-only reconciliation after an interrupted dispatch; never submit again.
    async fn recover(&self, record: &Record) -> Result<Option<Task>>;
    async fn poll(&self, record: &Record) -> Result<Observation>;
    async fn cancel(&self, record: &Record) -> Result<()>;
    async fn collect(&self, _record: &Record) -> Result<Option<Value>> {
        Ok(None)
    }
    async fn restart(&self, _record: &Record) -> Result<()> {
        Err("This backend cannot continue a completed session.".into())
    }
    async fn cleanup(&self, record: &Record) -> Result<Option<Value>>;
}

/// Resume a retained job without repeating dispatch after an ambiguous response.
pub async fn drive<B: Backend>(
    backend: &B,
    lease: &Lease,
    record: &mut Record,
    cancel: &AtomicBool,
    interval: Duration,
    emit: &mut dyn FnMut(Value),
) -> Result<()> {
    if interval.is_zero() {
        return Err("The remote polling interval must be positive.".into());
    }
    if record.state == State::Created && (cancel.load(Ordering::Relaxed) || lease.cancelled()) {
        record.cancel_requested = true;
        record.state = State::Cancelled;
        record.cleanup_complete = true;
        lease.save(record)?;
        return Ok(());
    }
    if record.state == State::Resuming {
        if record.cancel_requested || cancel.load(Ordering::Relaxed) || lease.cancelled() {
            record.state = State::Cancelled;
        } else {
            backend.restart(record).await?;
            record.state = State::Ready;
        }
        lease.save(record)?;
    }
    if record.state == State::Created {
        if let Err(error) = backend.resolve(record).await {
            if let Some(pin) = &record.environment {
                // The pinned image cannot start: a definite failure that
                // never falls back to the base template. Nothing was
                // provisioned, so there is nothing to clean up.
                record.state = State::Failed;
                record.error = Some(format!(
                    "Environment {} version {} cannot start: {error}",
                    pin.environment, pin.version_id
                ));
                record.cleanup_complete = true;
                lease.save(record)?;
            }
            return Err(error);
        }
        record.spec.validate()?;
    }
    if record.state == State::Created || record.state == State::Provisioning {
        record.state = State::Provisioning;
        lease.save(record)?;
        let resource = backend.provision(record).await?;
        record.resource = Some(resource);
        record.state = State::Ready;
        lease.save(record)?;
    }
    if record.state == State::Ready {
        if record.cancel_requested || cancel.load(Ordering::Relaxed) || lease.cancelled() {
            record.cancel_requested = true;
            record.state = State::Cancelled;
            lease.save(record)?;
        } else {
            if let Err(error) = backend.prepare(record).await {
                record.state = State::Failed;
                record.error = Some(error.clone());
                lease.save(record)?;
                match backend.cleanup(record).await {
                    Ok(usage) => {
                        record.usage = usage;
                        record.cleanup_complete = true;
                    }
                    Err(cleanup) => record.cleanup_error = Some(cleanup),
                }
                lease.save(record)?;
                return Err(error);
            }
            if cancel.load(Ordering::Relaxed) || lease.cancelled() {
                record.cancel_requested = true;
                record.state = State::Cancelled;
                lease.save(record)?;
                record.usage = backend.cleanup(record).await?;
                record.cleanup_complete = true;
                lease.save(record)?;
                return Ok(());
            }
            record.state = State::Dispatching;
            lease.save(record)?;
            match backend.dispatch(record).await {
                Ok(task) => {
                    record.remote_task = Some(task);
                    record.state = State::Running;
                    lease.save(record)?;
                }
                Err(error) => {
                    record.error = Some(error.clone());
                    lease.save(record)?;
                    return Err(error);
                }
            }
        }
    } else if record.state == State::Dispatching
        && (record.cancel_requested || cancel.load(Ordering::Relaxed) || lease.cancelled())
    {
        record.cancel_requested = true;
        lease.save(record)?;
        backend.cancel(record).await?;
        record.state = State::Cancelled;
        lease.save(record)?;
    } else if record.state == State::Dispatching {
        let task=backend.recover(record).await?.ok_or("Remote dispatch is unresolved. Follow this job again; another task will not be submitted.")?;
        record.remote_task = Some(task);
        record.state = State::Running;
        record.error = None;
        lease.save(record)?;
    }
    while record.state == State::Running {
        let expired =
            now_ms().saturating_sub(record.created_ms) > record.spec.timeout_seconds * 1000;
        if record.cancel_requested || cancel.load(Ordering::Relaxed) || lease.cancelled() || expired
        {
            record.cancel_requested = true;
            lease.save(record)?;
            backend.cancel(record).await?;
            record.state = if expired {
                State::Failed
            } else {
                State::Cancelled
            };
            if expired {
                record.error = Some("The remote job reached its deadline.".into());
            }
            lease.save(record)?;
            break;
        }
        let observation = backend.poll(record).await?;
        let incoming =
            serde_json::to_vec(&observation.events).map_err(|_| "Cannot encode remote events.")?;
        let oversized = observation
            .events
            .iter()
            .any(|e| serde_json::to_vec(e).is_ok_and(|v| v.len() > MAX_EVENT_BYTES))
            || incoming.len()
                + serde_json::to_vec(record)
                    .map_err(|_| "Cannot encode the remote job.")?
                    .len()
                > MAX_RECORD_BYTES - 65536;
        if oversized {
            record.cancel_requested = true;
            record.error = Some(
                "Remote output reached its retained size limit; execution was stopped.".into(),
            );
            lease.save(record)?;
            backend.cancel(record).await?;
            record.state = State::Failed;
            break;
        }
        let previous = record.events.len();
        record.events.extend(observation.events);
        record.cursor = observation.cursor.or(record.cursor.take());
        record.updated_ms = now_ms();
        if let Some(end) = observation.end {
            match end {
                Ok(result) => {
                    record.state = State::Completed;
                    record.result = Some(result);
                }
                Err(error) => {
                    record.state = State::Failed;
                    record.error = Some(error);
                }
            }
        }
        lease.save(record)?;
        for event in &record.events[previous..] {
            emit(event.clone());
        }
        if record.state == State::Running {
            tokio::time::sleep(interval).await;
        }
    }
    if record.state.terminal() && record.artifacts.is_none() {
        match backend
            .collect(record)
            .await
            .and_then(|payload| workspace::retain(lease, record, payload))
        {
            Ok(artifacts) => {
                record.artifacts = Some(artifacts);
                record.artifact_error = None;
            }
            Err(error) => {
                record.artifact_error = Some(error);
            }
        }
        lease.save(record)?;
    }
    if record.state.terminal() && !record.cleanup_complete {
        match backend.cleanup(record).await {
            Ok(usage) => {
                record.usage = usage;
                record.cleanup_complete = true;
                record.cleanup_error = None;
            }
            Err(error) => {
                record.cleanup_error = Some(error.clone());
                lease.save(record)?;
                return Err(error);
            }
        }
        record.updated_ms = now_ms();
        workspace::seal_usage(lease, record)?;
        lease.save(record)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    fn spec() -> Spec {
        Spec {
            placement: Placement::Boat,
            mode: Mode::Integrated,
            agent: "codex".into(),
            task: "Read the fixture".into(),
            model: None,
            reasoning: None,
            cwd: PathBuf::from("fixture"),
            timeout_seconds: 600,
            size: "small".into(),
            template: None,
            credential_names: vec![],
        }
    }
    struct Fake {
        restarts: Cell<u32>,
        oversized: bool,
        dispatches: Cell<u32>,
        fail_dispatch: bool,
        cancels: Cell<u32>,
    }
    impl Backend for Fake {
        async fn provision(&self, _: &mut Record) -> Result<String> {
            Ok("sandbox".into())
        }
        async fn dispatch(&self, _: &Record) -> Result<Task> {
            self.dispatches.set(self.dispatches.get() + 1);
            if self.fail_dispatch {
                Err("lost response".into())
            } else {
                Ok(Task {
                    id: "task".into(),
                    conversation: None,
                })
            }
        }
        async fn recover(&self, _: &Record) -> Result<Option<Task>> {
            Ok(Some(Task {
                id: "task".into(),
                conversation: None,
            }))
        }
        async fn poll(&self, _: &Record) -> Result<Observation> {
            Ok(Observation {
                events: vec![
                    serde_json::json!({"event":"delta","text":if self.oversized {"x".repeat(MAX_EVENT_BYTES)}else{"done".into()}}),
                ],
                cursor: Some("1".into()),
                end: Some(Ok(serde_json::json!({"reply":"done"}))),
            })
        }
        async fn restart(&self, _: &Record) -> Result<()> {
            self.restarts.set(self.restarts.get() + 1);
            Ok(())
        }
        async fn cancel(&self, _: &Record) -> Result<()> {
            self.cancels.set(self.cancels.get() + 1);
            Ok(())
        }
        async fn cleanup(&self, _: &Record) -> Result<Option<Value>> {
            Ok(Some(serde_json::json!({"cost_usd":0.001})))
        }
    }
    #[tokio::test]
    async fn oversized_output_stops_execution_and_retains_confirmed_cleanup() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::under(root.path());
        let lease = store.lease("j1").unwrap();
        let mut r = Record::new("j1", spec()).unwrap();
        let b = Fake {
            restarts: Cell::new(0),
            oversized: true,
            dispatches: Cell::new(0),
            fail_dispatch: false,
            cancels: Cell::new(0),
        };
        drive(
            &b,
            &lease,
            &mut r,
            &AtomicBool::new(false),
            Duration::from_millis(1),
            &mut |_| panic!("Oversized event escaped"),
        )
        .await
        .unwrap();
        assert_eq!(r.state, State::Failed);
        assert!(r.cleanup_complete);
        assert_eq!(b.cancels.get(), 1);
        assert!(r.events.is_empty());
        assert!(r.artifacts.is_some());
    }
    #[tokio::test]
    async fn retained_continuation_restarts_before_one_dispatch_and_follow_is_inert() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::under(root.path());
        let lease = store.lease("j1").unwrap();
        let mut r = Record::new("j1", spec()).unwrap();
        r.state = State::Resuming;
        r.resource = Some("existing".into());
        lease.save(&r).unwrap();
        let b = Fake {
            restarts: Cell::new(0),
            oversized: false,
            dispatches: Cell::new(0),
            fail_dispatch: false,
            cancels: Cell::new(0),
        };
        drive(
            &b,
            &lease,
            &mut r,
            &AtomicBool::new(false),
            Duration::from_millis(1),
            &mut |_| {},
        )
        .await
        .unwrap();
        drive(
            &b,
            &lease,
            &mut r,
            &AtomicBool::new(false),
            Duration::from_millis(1),
            &mut |_| panic!("Replayed event"),
        )
        .await
        .unwrap();
        assert_eq!(b.restarts.get(), 1);
        assert_eq!(b.dispatches.get(), 1);
        assert_eq!(r.resource.as_deref(), Some("existing"));
    }
    #[tokio::test]
    async fn lost_dispatch_recovers_without_submitting_twice_and_terminal_follow_is_inert() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::under(root.path());
        let lease = store.lease("j1").unwrap();
        let mut record = Record::new("j1", spec()).unwrap();
        let backend = Fake {
            dispatches: Cell::new(0),
            fail_dispatch: true,
            restarts: Cell::new(0),
            oversized: false,
            cancels: Cell::new(0),
        };
        let cancel = AtomicBool::new(false);
        assert!(
            drive(
                &backend,
                &lease,
                &mut record,
                &cancel,
                Duration::from_millis(1),
                &mut |_| {}
            )
            .await
            .is_err()
        );
        let mut restored = store.read("j1").unwrap();
        assert_eq!(restored.state, State::Dispatching);
        drive(
            &backend,
            &lease,
            &mut restored,
            &cancel,
            Duration::from_millis(1),
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(backend.dispatches.get(), 1);
        assert!(restored.cleanup_complete);
        assert_eq!(restored.cursor.as_deref(), Some("1"));
        drive(
            &backend,
            &lease,
            &mut restored,
            &cancel,
            Duration::from_millis(1),
            &mut |_| panic!("replayed event"),
        )
        .await
        .unwrap();
    }
    #[tokio::test]
    async fn separate_cancel_request_stops_the_remote_job_and_retains_cleanup() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::under(root.path());
        let lease = store.lease("j1").unwrap();
        let mut record = Record::new("j1", spec()).unwrap();
        record.state = State::Running;
        record.resource = Some("sandbox".into());
        record.remote_task = Some(Task {
            id: "t".into(),
            conversation: None,
        });
        lease.save(&record).unwrap();
        store.cancel("j1").unwrap();
        let backend = Fake {
            dispatches: Cell::new(0),
            fail_dispatch: false,
            restarts: Cell::new(0),
            oversized: false,
            cancels: Cell::new(0),
        };
        drive(
            &backend,
            &lease,
            &mut record,
            &AtomicBool::new(false),
            Duration::from_millis(1),
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(backend.cancels.get(), 1);
        assert_eq!(store.read("j1").unwrap().state, State::Cancelled);
        assert!(record.cleanup_complete);
    }
    #[test]
    fn store_refuses_concurrent_writers_and_unsafe_ids() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::under(root.path());
        let lock = store.lease("j1").unwrap();
        assert!(store.lease("j1").is_err());
        assert!(store.lease("../other").is_err());
        drop(lock);
        // A child another test forks shares the lock until it execs.
        let started = std::time::Instant::now();
        while store.lease("j1").is_err() && started.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(store.lease("j1").is_ok());
    }
}
