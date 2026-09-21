//! The durable ledger: one document, one writer, checked transitions.
//!
//! The plan is a pure function; the ledger is the durable state it
//! reads. One `scheduler-ledger.json` document lives in a directory the
//! caller places *outside any checkout* — beside the checkout and a
//! stray `git clean` takes the run's memory with it. A sibling
//! `scheduler-ledger.lock` file serializes writers with the OS's
//! cross-process file lock, so two processes cannot interleave a read
//! and a write.
//!
//! # Writes
//!
//! Every mutation rewrites the document atomically: the new document is
//! written to a sibling temporary file, `fsync`ed, renamed over the old
//! one, and the directory is `fsync`ed so the rename itself is durable.
//! A reader sees the old document or the new one, never a torn write.
//! The document carries a schema tag and a monotonically increasing
//! `sequence` — version validation refuses a document this build cannot
//! read.
//!
//! # Transitions
//!
//! The record's lifecycle is checked — every mutation names the state it
//! expects and refuses anything else:
//!
//! - `claim` — `queued` → `dispatched`, minting a unique attempt id and
//!   binding the task's content digest and the claiming owner.
//! - `settle` — `dispatched` → `review`, recording the result's digest.
//!   The caller must present the attempt id and the owner that claimed
//!   it: ownership is compare-and-set, so a second owner cannot settle
//!   an attempt it did not dispatch.
//! - `accept` — `review` → `completed`, the only path that releases a
//!   task's dependents. Acceptance re-checks the task's content digest:
//!   a task whose base, input, footprint, or resources changed since
//!   dispatch cannot accept the old attempt's result.
//! - `reject` — `review` → `rejected`, with a cause.
//! - `requeue` — `rejected` or `unknown` → `queued`. Explicit, always:
//!   nothing requeues itself.
//!
//! # Crash recovery
//!
//! On open, every record still `dispatched` moves to `unknown`. An
//! in-flight attempt may have run, may have written, or may never have
//! started — the ledger says `unknown` rather than guessing, the task
//! stays blocked, and nothing is replayed automatically. Reconciling an
//! `unknown` task is the operator's explicit `requeue` after looking at
//! what actually landed.
//!
//! There is no exactly-once dispatch here and none is claimed: the
//! ledger serializes *transitions*, so a caller that crashes between
//! dispatching work and reading its own record reopens to find the
//! attempt either `dispatched`, recovered to `unknown`, or settled —
//! each distinguishable by the unique attempt id.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::catalog::Catalog;
use crate::plan::Status;

/// The document the ledger keeps, and its lock.
const LEDGER: &str = "scheduler-ledger.json";
const LOCK: &str = "scheduler-ledger.lock";

/// The schema tag the ledger document carries.
pub const SCHEMA: &str = "openagents.scheduler.ledger.v1";

/// How long a writer waits for the ledger's lock.
const LOCK_WAIT: Duration = Duration::from_secs(10);

/// One task's durable record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TaskRecord {
    /// Where the task stands.
    pub status: Status,
    /// The task's content digest as bound at the last dispatch. A task
    /// that changed since cannot accept the old attempt's result.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub task_digest: String,
    /// The latest attempt's id, unique under this ledger.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub attempt: String,
    /// The owner that claimed the latest attempt — settlement and
    /// acceptance must present it back.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub owner: String,
    /// The digest of the result the latest attempt returned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_digest: Option<String>,
    /// The reject cause or the recovery note, when one applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
    /// How many times the task has been claimed.
    #[serde(default)]
    pub attempts: u32,
    /// Unix seconds of the last transition.
    #[serde(default)]
    pub updated_unix: u64,
}

/// The ledger document on disk.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Document {
    /// The schema tag.
    v: String,
    /// The ledger instance's id, minted once at creation — attempt ids
    /// carry it, so attempts from two ledgers never share a name.
    run: String,
    /// The revision: bumped on every committed write. Two writers never
    /// see the same sequence, because the lock serializes them.
    sequence: u64,
    /// The next attempt counter.
    next_attempt: u64,
    /// A record per registered task id.
    tasks: BTreeMap<String, TaskRecord>,
}

/// What went wrong with the ledger itself or a transition it checked.
#[derive(Debug)]
pub enum LedgerError {
    /// The filesystem refused.
    Io(std::io::Error),
    /// The document did not parse — the ledger is corrupt.
    Corrupt(String),
    /// The document's schema is not this build's.
    UnknownSchema(String),
    /// Another ledger writer holds the lock.
    Locked(String),
    /// The transition names a state the record is not in.
    Transition {
        /// The task.
        task: String,
        /// Where it stands.
        from: Status,
        /// Where the caller tried to move it.
        to: &'static str,
    },
    /// No record exists under this task id — register it first.
    Unregistered(String),
    /// The presented attempt id or owner is not the record's — a
    /// compare-and-set miss.
    Ownership {
        /// The task.
        task: String,
        /// Which check failed.
        check: &'static str,
    },
    /// The presented task digest is not the digest bound at dispatch —
    /// a changed task cannot accept the old attempt's result.
    TaskChanged {
        /// The task.
        task: String,
        /// The digest the record holds.
        bound: String,
        /// The digest the caller presented.
        presented: String,
    },
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Corrupt(message) => write!(f, "{message}"),
            Self::UnknownSchema(schema) => {
                write!(f, "ledger schema `{schema}` is not `{SCHEMA}`")
            }
            Self::Locked(path) => write!(
                f,
                "another writer holds {path}. The ledger takes one writer at a \
                 time: wait for it to finish, or remove the lock file if no \
                 writer is running"
            ),
            Self::Transition { task, from, to } => {
                write!(f, "task `{task}` is {from:?} — it cannot move to {to}")
            }
            Self::Unregistered(task) => {
                write!(f, "task `{task}` has no record — register it first")
            }
            Self::Ownership { task, check } => write!(
                f,
                "task `{task}`: the presented {check} is not the record's"
            ),
            Self::TaskChanged {
                task,
                bound,
                presented,
            } => write!(
                f,
                "task `{task}` changed since dispatch ({bound} vs {presented}) — \
                 the old attempt's result does not apply"
            ),
        }
    }
}

impl std::error::Error for LedgerError {}

impl From<std::io::Error> for LedgerError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// A record whose bound task identity no longer matches the catalog's —
/// a changed task holding an old result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Drift {
    /// The task.
    pub task: String,
    /// Where its record stands.
    pub status: Status,
    /// The digest bound at dispatch.
    pub bound: String,
    /// The digest the catalog now computes.
    pub current: String,
}

/// The open ledger. Hold it for the run's mutations; the OS lock is held
/// for its lifetime, so a second `Ledger::open` on the same directory
/// refuses rather than interleaves.
pub struct Ledger {
    dir: PathBuf,
    path: PathBuf,
    document: Document,
    _lock: std::fs::File,
}

impl Ledger {
    /// Open the ledger under `dir`, creating it on first use.
    ///
    /// `dir` must live outside any checkout — the ledger is the run's
    /// memory, and a checkout is a place work gets cleaned out of. See
    /// [`Ledger::default_dir`] for the conventional location.
    pub fn open(dir: &Path) -> Result<Self, LedgerError> {
        Self::open_at(dir, unix_now())
    }

    /// The conventional state directory: `XDG_DATA_HOME/openagents/
    /// scheduler` when `XDG_DATA_HOME` is set and absolute, `HOME/
    /// .local/share/openagents/scheduler` otherwise. `None` when neither
    /// names an absolute path — a state directory nobody can name is not
    /// one to write.
    #[must_use]
    pub fn default_dir() -> Option<PathBuf> {
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|path| path.is_absolute())
                    .map(|home| home.join(".local/share"))
            })?;
        Some(data_home.join("openagents/scheduler"))
    }

    /// Open at an explicit time — the seam tests and recovery share.
    fn open_at(dir: &Path, now: u64) -> Result<Self, LedgerError> {
        std::fs::create_dir_all(dir)?;
        let lock = lock_file(&dir.join(LOCK))?;
        let path = dir.join(LEDGER);
        let mut ledger = Self {
            dir: dir.to_path_buf(),
            path,
            document: Document {
                v: SCHEMA.to_string(),
                run: String::new(),
                sequence: 0,
                next_attempt: 1,
                tasks: BTreeMap::new(),
            },
            _lock: lock,
        };
        ledger.load()?;
        ledger.recover(now)?;
        Ok(ledger)
    }

    /// Read the document, or mint a fresh one — and commit it, so the
    /// run id exists on disk before any attempt does.
    fn load(&mut self) -> Result<(), LedgerError> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => {
                let document: Document = serde_json::from_str(&text).map_err(|error| {
                    LedgerError::Corrupt(format!("{}: {error}", self.path.display()))
                })?;
                if document.v != SCHEMA {
                    return Err(LedgerError::UnknownSchema(document.v));
                }
                self.document = document;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.document.run = fresh_run()?;
                self.commit()?;
            }
            Err(error) => return Err(LedgerError::Io(error)),
        }
        Ok(())
    }

    /// Crash recovery on open: every record still `dispatched` becomes
    /// `unknown`. It stays blocked; nothing replays it.
    fn recover(&mut self, now: u64) -> Result<(), LedgerError> {
        let crashed: Vec<String> = self
            .document
            .tasks
            .iter()
            .filter(|(_, record)| record.status == Status::Active)
            .map(|(id, _)| id.clone())
            .collect();
        if crashed.is_empty() {
            return Ok(());
        }
        for id in crashed {
            let record = self.document.tasks.get_mut(&id).expect("the id exists");
            record.status = Status::Unknown;
            record.cause =
                Some("the writer closed with this attempt in flight — outcome unknown".to_string());
            record.updated_unix = now;
        }
        self.commit()
    }

    /// Write the document: bump the revision, write a sibling temporary
    /// file, `fsync` it, rename it over the old document, and `fsync`
    /// the directory so the rename is durable too.
    fn commit(&mut self) -> Result<(), LedgerError> {
        self.document.sequence += 1;
        let text = serde_json::to_string_pretty(&self.document)
            .map_err(|error| LedgerError::Corrupt(error.to_string()))?;
        let temporary = self.path.with_extension("tmp");
        {
            let mut file = std::fs::File::create(&temporary)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
        }
        std::fs::rename(&temporary, &self.path)?;
        std::fs::File::open(&self.dir)?.sync_all()?;
        Ok(())
    }

    /// One record, by task id.
    #[must_use]
    pub fn record(&self, task: &str) -> Option<&TaskRecord> {
        self.document.tasks.get(task)
    }

    /// Every record, for the planner's view.
    #[must_use]
    pub fn records(&self) -> &BTreeMap<String, TaskRecord> {
        &self.document.tasks
    }

    /// The planner's status view over the records.
    #[must_use]
    pub fn statuses(&self) -> BTreeMap<String, Status> {
        self.document
            .tasks
            .iter()
            .map(|(id, record)| (id.clone(), record.status))
            .collect()
    }

    /// The document's revision — bumped on every committed write.
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.document.sequence
    }

    /// Register a catalog's tasks: insert a `queued` record for each id
    /// the ledger has never seen, and rebind the content digest of
    /// records still `queued` — a task may be re-pinned before it ever
    /// runs. A record past `queued` keeps the digest it was dispatched
    /// under; [`Ledger::drift`] reports the mismatch instead of
    /// silently rewriting a bound attempt.
    pub fn register(&mut self, catalog: &Catalog) -> Result<(), LedgerError> {
        let now = unix_now();
        let mut changed = false;
        for task in &catalog.tasks {
            match self.document.tasks.get_mut(&task.id) {
                None => {
                    self.document.tasks.insert(
                        task.id.clone(),
                        TaskRecord {
                            status: Status::Queued,
                            task_digest: task.digest(),
                            attempt: String::new(),
                            owner: String::new(),
                            result_digest: None,
                            cause: None,
                            attempts: 0,
                            updated_unix: now,
                        },
                    );
                    changed = true;
                }
                Some(record)
                    if record.status == Status::Queued && record.task_digest != task.digest() =>
                {
                    record.task_digest = task.digest();
                    record.updated_unix = now;
                    changed = true;
                }
                _ => {}
            }
        }
        if changed {
            self.commit()?;
        }
        Ok(())
    }

    /// Records whose bound digest no longer matches the catalog's — a
    /// changed task holding an old result or claim.
    #[must_use]
    pub fn drift(&self, catalog: &Catalog) -> Vec<Drift> {
        let mut drift = Vec::new();
        for task in &catalog.tasks {
            if let Some(record) = self.document.tasks.get(&task.id)
                && !record.task_digest.is_empty()
                && record.task_digest != task.digest()
            {
                drift.push(Drift {
                    task: task.id.clone(),
                    status: record.status,
                    bound: record.task_digest.clone(),
                    current: task.digest(),
                });
            }
        }
        drift
    }

    /// Claim a queued task for dispatch under `owner`, binding the
    /// presented task digest. Returns the minted attempt id.
    ///
    /// The caller presents the digest rather than trusting the record's,
    /// so a dispatch against a re-pinned task refuses instead of binding
    /// the stale identity the caller still holds.
    pub fn claim(
        &mut self,
        task: &str,
        owner: &str,
        task_digest: &str,
    ) -> Result<String, LedgerError> {
        let record = self
            .document
            .tasks
            .get_mut(task)
            .ok_or_else(|| LedgerError::Unregistered(task.to_string()))?;
        if record.status != Status::Queued {
            return Err(LedgerError::Transition {
                task: task.to_string(),
                from: record.status,
                to: "dispatched",
            });
        }
        if record.task_digest != task_digest {
            return Err(LedgerError::TaskChanged {
                task: task.to_string(),
                bound: record.task_digest.clone(),
                presented: task_digest.to_string(),
            });
        }
        let attempt = format!(
            "att-{}-{:06}",
            self.document.run, self.document.next_attempt
        );
        self.document.next_attempt += 1;
        record.status = Status::Active;
        record.attempt = attempt.clone();
        record.owner = owner.to_string();
        record.attempts += 1;
        record.result_digest = None;
        record.cause = None;
        record.updated_unix = unix_now();
        self.commit()?;
        Ok(attempt)
    }

    /// The one record under `task`, checked to be `status` and to carry
    /// this attempt and owner — the compare-and-set every settlement
    /// shares.
    fn expect(
        &mut self,
        task: &str,
        attempt: &str,
        owner: &str,
        status: Status,
        to: &'static str,
    ) -> Result<&mut TaskRecord, LedgerError> {
        let record = self
            .document
            .tasks
            .get_mut(task)
            .ok_or_else(|| LedgerError::Unregistered(task.to_string()))?;
        if record.status != status {
            return Err(LedgerError::Transition {
                task: task.to_string(),
                from: record.status,
                to,
            });
        }
        if record.attempt != attempt {
            return Err(LedgerError::Ownership {
                task: task.to_string(),
                check: "attempt id",
            });
        }
        if record.owner != owner {
            return Err(LedgerError::Ownership {
                task: task.to_string(),
                check: "owner",
            });
        }
        Ok(record)
    }

    /// Settle an in-flight attempt: `dispatched` → `review` with the
    /// result's digest. The attempt id and owner must match the claim —
    /// ownership is compare-and-set.
    ///
    /// The result is a claim awaiting independent review; settling it
    /// completes nothing.
    pub fn settle(
        &mut self,
        task: &str,
        attempt: &str,
        owner: &str,
        result_digest: &str,
    ) -> Result<(), LedgerError> {
        let record = self.expect(task, attempt, owner, Status::Active, "review")?;
        record.status = Status::Review;
        record.result_digest = Some(result_digest.to_string());
        record.updated_unix = unix_now();
        self.commit()
    }

    /// Accept a reviewed result: `review` → `completed`, the only path
    /// that releases dependents.
    ///
    /// The presented task digest must equal the digest bound at
    /// dispatch — a task changed since its attempt ran cannot accept the
    /// old result, because the result answers different work.
    pub fn accept(
        &mut self,
        task: &str,
        attempt: &str,
        owner: &str,
        task_digest: &str,
    ) -> Result<(), LedgerError> {
        let record = self.expect(task, attempt, owner, Status::Review, "completed")?;
        if record.task_digest != task_digest {
            return Err(LedgerError::TaskChanged {
                task: task.to_string(),
                bound: record.task_digest.clone(),
                presented: task_digest.to_string(),
            });
        }
        record.status = Status::Completed;
        record.updated_unix = unix_now();
        self.commit()
    }

    /// Refuse a reviewed result: `review` → `rejected`, with the cause
    /// kept for the record.
    pub fn reject(
        &mut self,
        task: &str,
        attempt: &str,
        owner: &str,
        cause: &str,
    ) -> Result<(), LedgerError> {
        let record = self.expect(task, attempt, owner, Status::Review, "rejected")?;
        record.status = Status::Rejected;
        record.cause = Some(cause.to_string());
        record.updated_unix = unix_now();
        self.commit()
    }

    /// Return a `rejected` or `unknown` task to `queued`. Explicit —
    /// reconciliation after a crash is the operator's stated act, never
    /// an automatic replay.
    pub fn requeue(&mut self, task: &str) -> Result<(), LedgerError> {
        let record = self
            .document
            .tasks
            .get_mut(task)
            .ok_or_else(|| LedgerError::Unregistered(task.to_string()))?;
        match record.status {
            Status::Rejected | Status::Unknown => {}
            from => {
                return Err(LedgerError::Transition {
                    task: task.to_string(),
                    from,
                    to: "queued",
                });
            }
        }
        record.status = Status::Queued;
        record.owner = String::new();
        record.updated_unix = unix_now();
        self.commit()
    }
}

/// The ledger's lock: an OS file lock on a sibling file, taken for the
/// open lifetime. `try_lock` is retried for `LOCK_WAIT` so a brief
/// second writer waits rather than collides; a writer that outlives the
/// wait is named in the refusal.
fn lock_file(path: &Path) -> Result<std::fs::File, LedgerError> {
    let file = std::fs::File::create(path)?;
    let started = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) if started.elapsed() < LOCK_WAIT => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(LedgerError::Locked(path.display().to_string()));
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(LedgerError::Io(error)),
        }
    }
}

/// The ledger instance's id — 16 bytes of randomness, hex, minted once.
fn fresh_run() -> Result<String, LedgerError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|error| LedgerError::Corrupt(format!("no randomness available: {error}")))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Unix seconds now.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Catalog, Footprint, Task};
    use crate::resources::Resources;

    fn task(id: &str) -> Task {
        Task {
            id: id.to_string(),
            issue: 1,
            base: "sha256:base".to_string(),
            input: format!("sha256:input-{id}"),
            depends_on: vec![],
            footprint: Footprint::Declared {
                reads: vec![],
                writes: vec![format!("crates/{id}/src/lib.rs")],
            },
            priority: 0,
            resources: Resources::default(),
            estimate_ticks: 1,
        }
    }

    fn catalog(ids: &[&str]) -> Catalog {
        let mut catalog = Catalog::new("test", ids.iter().map(|id| task(id)).collect());
        catalog.seal().unwrap();
        catalog
    }

    #[test]
    fn a_record_walks_the_checked_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = catalog(&["a"]);
        let mut ledger = Ledger::open(dir.path()).unwrap();
        ledger.register(&catalog).unwrap();

        let digest = catalog.task("a").unwrap().digest();
        let attempt = ledger.claim("a", "host:1", &digest).unwrap();
        assert!(attempt.starts_with("att-"));
        ledger
            .settle("a", &attempt, "host:1", "sha256:result")
            .unwrap();
        ledger.accept("a", &attempt, "host:1", &digest).unwrap();
        assert_eq!(ledger.record("a").unwrap().status, Status::Completed);
    }

    #[test]
    fn transitions_refuse_states_they_do_not_name() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = catalog(&["a"]);
        let mut ledger = Ledger::open(dir.path()).unwrap();
        ledger.register(&catalog).unwrap();
        let digest = catalog.task("a").unwrap().digest();

        // Settle before dispatch, accept before review, requeue queued.
        assert!(matches!(
            ledger.settle("a", "att-x", "host:1", "sha256:r"),
            Err(LedgerError::Transition { .. })
        ));
        assert!(matches!(
            ledger.accept("a", "att-x", "host:1", &digest),
            Err(LedgerError::Transition { .. })
        ));
        assert!(matches!(
            ledger.requeue("a"),
            Err(LedgerError::Transition { .. })
        ));
        // A second claim on a live attempt refuses.
        let attempt = ledger.claim("a", "host:1", &digest).unwrap();
        assert!(matches!(
            ledger.claim("a", "host:2", &digest),
            Err(LedgerError::Transition { .. })
        ));
        ledger.settle("a", &attempt, "host:1", "sha256:r").unwrap();
    }

    #[test]
    fn settlement_is_compare_and_set_on_owner_and_attempt() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = catalog(&["a"]);
        let mut ledger = Ledger::open(dir.path()).unwrap();
        ledger.register(&catalog).unwrap();
        let digest = catalog.task("a").unwrap().digest();
        let attempt = ledger.claim("a", "host:1", &digest).unwrap();

        // A wrong owner cannot settle.
        assert!(matches!(
            ledger.settle("a", &attempt, "host:2", "sha256:r"),
            Err(LedgerError::Ownership { check, .. }) if check == "owner"
        ));
        // A wrong attempt id cannot settle.
        assert!(matches!(
            ledger.settle("a", "att-forged", "host:1", "sha256:r"),
            Err(LedgerError::Ownership { check, .. }) if check == "attempt id"
        ));
        ledger.settle("a", &attempt, "host:1", "sha256:r").unwrap();
        assert_eq!(ledger.record("a").unwrap().status, Status::Review);
    }

    #[test]
    fn attempt_ids_are_unique_across_requeues() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = catalog(&["a"]);
        let mut ledger = Ledger::open(dir.path()).unwrap();
        ledger.register(&catalog).unwrap();
        let digest = catalog.task("a").unwrap().digest();

        let first = ledger.claim("a", "host:1", &digest).unwrap();
        ledger.settle("a", &first, "host:1", "sha256:r1").unwrap();
        ledger
            .reject("a", &first, "host:1", "wrong answer")
            .unwrap();
        ledger.requeue("a").unwrap();
        let second = ledger.claim("a", "host:1", &digest).unwrap();
        assert_ne!(first, second);
        assert_eq!(ledger.record("a").unwrap().attempts, 2);
    }

    #[test]
    fn a_changed_task_cannot_accept_the_old_result() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = catalog(&["a"]);
        let mut ledger = Ledger::open(dir.path()).unwrap();
        ledger.register(&catalog).unwrap();
        let digest = catalog.task("a").unwrap().digest();
        let attempt = ledger.claim("a", "host:1", &digest).unwrap();
        ledger.settle("a", &attempt, "host:1", "sha256:r").unwrap();

        // The task is re-pinned — different input, different digest.
        let mut changed = task("a");
        changed.input = "sha256:new-input".to_string();
        let mut catalog2 = Catalog::new("test", vec![changed]);
        catalog2.seal().unwrap();
        let new_digest = catalog2.task("a").unwrap().digest();

        assert!(matches!(
            ledger.accept("a", &attempt, "host:1", &new_digest),
            Err(LedgerError::TaskChanged { .. })
        ));
        // Registering the changed catalog reports the drift rather than
        // rewriting the bound record.
        ledger.register(&catalog2).unwrap();
        let drift = ledger.drift(&catalog2);
        assert_eq!(drift.len(), 1);
        assert_eq!(drift[0].task, "a");
        assert_eq!(drift[0].status, Status::Review);
    }

    #[test]
    fn a_crash_recovers_in_flight_work_as_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = catalog(&["a", "b"]);
        let digest_a = catalog.task("a").unwrap().digest();
        {
            let mut ledger = Ledger::open(dir.path()).unwrap();
            ledger.register(&catalog).unwrap();
            ledger.claim("a", "host:1", &digest_a).unwrap();
            // The writer goes away with `a` dispatched and `b` queued.
        }
        let mut ledger = Ledger::open(dir.path()).unwrap();
        assert_eq!(ledger.record("a").unwrap().status, Status::Unknown);
        assert_eq!(ledger.record("b").unwrap().status, Status::Queued);
        // Unknown stays blocked — settle refuses, and only an explicit
        // requeue returns it to the queue.
        assert!(matches!(
            ledger.settle(
                "a",
                &ledger.record("a").unwrap().attempt.clone(),
                "host:1",
                "sha256:r"
            ),
            Err(LedgerError::Transition { .. })
        ));
        ledger.requeue("a").unwrap();
        assert_eq!(ledger.record("a").unwrap().status, Status::Queued);
    }

    #[test]
    fn a_second_writer_waits_on_the_os_lock() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = Ledger::open(dir.path()).unwrap();
        let path = dir.path().to_path_buf();
        let contender = std::thread::spawn(move || {
            let started = Instant::now();
            let second = Ledger::open(&path);
            (started.elapsed(), second.is_ok())
        });
        std::thread::sleep(Duration::from_millis(200));
        drop(ledger);
        let (waited, opened) = contender.join().unwrap();
        assert!(opened, "the lock frees with the first writer");
        assert!(
            waited >= Duration::from_millis(150),
            "the second writer waited for the lock, not past it: {waited:?}"
        );
    }

    #[test]
    fn the_document_survives_a_reopen_and_validates_its_version() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = catalog(&["a"]);
        {
            let mut ledger = Ledger::open(dir.path()).unwrap();
            ledger.register(&catalog).unwrap();
        }
        let ledger = Ledger::open(dir.path()).unwrap();
        assert_eq!(ledger.record("a").unwrap().status, Status::Queued);
        drop(ledger);

        // A foreign schema refuses.
        let text = std::fs::read_to_string(dir.path().join(LEDGER)).unwrap();
        let foreign = text.replacen(SCHEMA, "openagents.scheduler.ledger.v0", 1);
        std::fs::write(dir.path().join(LEDGER), foreign).unwrap();
        assert!(matches!(
            Ledger::open(dir.path()),
            Err(LedgerError::UnknownSchema(_))
        ));
    }

    #[test]
    fn a_torn_write_is_corrupt_not_a_state() {
        let dir = tempfile::tempdir().unwrap();
        let _ledger = Ledger::open(dir.path()).unwrap();
        std::fs::write(dir.path().join(LEDGER), "{\"v\":").unwrap();
        drop(_ledger);
        assert!(matches!(
            Ledger::open(dir.path()),
            Err(LedgerError::Corrupt(_))
        ));
        // The temporary file is never left behind by a clean write.
        assert!(!dir.path().join("scheduler-ledger.tmp").exists());
    }
}
