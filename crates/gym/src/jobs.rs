//! The batch-job book: a durable record of a fan-out of requests.
//!
//! A batch job is a claim a coordinator makes about work it is running:
//! which inputs were accepted, which attempts were dispatched, and what
//! each attempt came to. The claim has to outlive the process that made
//! it, because a coordinator that dies mid-fan-out leaves the next
//! process asking exactly three questions — what was accepted, what
//! settled, and what is still unknown — and the book's job is that the
//! answer invents nothing.
//!
//! # The record
//!
//! The book is an append-only record log at a path the caller supplies:
//! one JSON object per line, written and fsynced before the call that
//! produced it returns. A submission lands on disk before it is
//! acknowledged, so a job the book returned is a job the book holds.
//! Recovery replays the log, and a torn tail — a last line cut off by a
//! crash mid-write — is truncated at the last complete record, because a
//! record that never finished writing was never acknowledged either.
//!
//! # The lifecycle
//!
//! `Queued` → `Running` → `Done`, with `Cancelling` → `Cancelled` for a
//! stop that has to wind down, and `Failed` wherever the coordinator
//! gives up. A transition the machine does not allow is refused by name —
//! a queued job cannot become `Done`, because nothing about it ran.
//!
//! `Done` is not a coverage claim. The counts say what the job actually
//! recorded — `expected` inputs, `attempted` inputs, and one standing per
//! input under each outcome — so a job that finished with inputs
//! unanswered reads as completed, never as covered.
//!
//! # Attempts and settlement
//!
//! [`Book::begin`] records that an attempt was dispatched;
//! [`Book::mark`] records what it came to. The pair is the idempotency
//! the book keeps: the same `(input, attempt)` marked twice with the same
//! outcome is the same record, not a second one, and marked with a
//! different outcome is a conflict, because one attempt does not have two
//! outcomes. A mark for an input the manifest does not declare is
//! refused — the book does not invent inputs, and a mark before any
//! `begin` is refused for the same reason: it does not invent attempts.
//!
//! # Recovery
//!
//! [`Book::recover`] replays the log and closes every attempt recorded
//! started but never settled as `unknown`. It does not guess whether the
//! work ran — the response may have been lost either way — and it does
//! not silently replay: a settled attempt is never applied twice, an
//! orphaned one keeps its outcome, and reconciliation over the unknowns
//! is the caller's explicit work. The book claims at-least-once
//! bookkeeping, never exactly-once inference.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

/// The schema tag a job manifest carries.
pub const SCHEMA: &str = "openagents.gym.job_manifest.v1";

/// What an attempt came to.
///
/// The same outcome vocabulary the execution receipt and the quota
/// ledger carry, so an outcome means the same thing in every record that
/// holds one.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// The door produced an answer.
    Answered,
    /// The door declined — a refusal is a recorded outcome, not an
    /// absence.
    Refused,
    /// Transport or capacity denied the call before the door decided:
    /// timeout, overload, dead door.
    Unavailable,
    /// The attempt was admitted but never dispatched.
    Unattempted,
    /// The record cannot say what happened — a crash after dispatch, a
    /// lost response, a recovery that found the attempt unsettled.
    Unknown,
}

impl Outcome {
    /// A short name for the outcome, for an error or a report line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Answered => "answered",
            Self::Refused => "refused",
            Self::Unavailable => "unavailable",
            Self::Unattempted => "unattempted",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One input's identity inside a submission: its stable id and the
/// `(request, attempt)` pair its first dispatch runs under — the same
/// idempotency pair the quota ledger holds, so an input's retry keeps
/// the request and bumps the attempt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Input {
    /// The input's id within the job — what `begin` and `mark` name.
    pub id: String,
    /// The logical request the input's first dispatch runs under.
    pub request: String,
    /// The attempt number that first dispatch runs under.
    pub attempt: u32,
}

/// A versioned job submission: what the book accepted.
///
/// `digest` covers every field but itself — canonicalized, key-sorted
/// JSON over SHA-256, the same canonicalization the tenancy manifest
/// uses — so a resubmission under the same job id is idempotent only
/// while the content is identical. [`Manifest::seal`] fills it in.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Manifest {
    /// The schema tag — [`SCHEMA`].
    pub v: String,
    /// The stable job id, which is also the idempotency key.
    pub job: String,
    /// The tenant the job runs for.
    pub tenant: String,
    /// The inputs, in the order the job asks them.
    pub inputs: Vec<Input>,
    /// The execution policy the job binds.
    pub policy: String,
    /// The model identity the job binds.
    pub model: String,
    /// The digest over every field above.
    pub digest: String,
}

impl Manifest {
    /// Fill in `digest` over the manifest's other fields.
    pub fn seal(&mut self) {
        self.digest = self.compute_digest();
    }

    /// The digest over every field but `digest`.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a manifest serializes");
        value
            .as_object_mut()
            .expect("a manifest is an object")
            .remove("digest");
        let mut hasher = Sha256::new();
        hasher.update(canonicalize(&value).as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }

    /// The checks [`Book::submit`] applies before a manifest is
    /// persisted: the schema tag this build writes, a digest that
    /// recomputes over the contents, and a declared input set that is
    /// non-empty, uniquely named, and carrying its dispatch identities.
    /// A submission that fails here is refused rather than filed: a
    /// manifest the book cannot read back is not a job.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != SCHEMA {
            return Err(format!("schema `{}` is not `{SCHEMA}`", self.v));
        }
        if self.digest != self.compute_digest() {
            return Err("the manifest's digest does not recompute over its contents".to_string());
        }
        if self.job.is_empty() {
            return Err("the manifest carries no job id".to_string());
        }
        if self.tenant.is_empty() {
            return Err("the manifest carries no tenant".to_string());
        }
        if self.policy.is_empty() {
            return Err("the manifest binds no policy".to_string());
        }
        if self.model.is_empty() {
            return Err("the manifest binds no model".to_string());
        }
        if self.inputs.is_empty() {
            return Err("the manifest declares no inputs".to_string());
        }
        let mut ids = BTreeSet::new();
        for input in &self.inputs {
            if input.id.is_empty() {
                return Err("the manifest declares an input with no id".to_string());
            }
            if input.request.is_empty() {
                return Err(format!("input `{}` names no request", input.id));
            }
            if !ids.insert(&input.id) {
                return Err(format!(
                    "input `{}` is declared twice, and a mark has to name one input",
                    input.id
                ));
            }
        }
        Ok(())
    }
}

/// Where a job stands.
///
/// `Cancelling` is a real state, not a flag: a running job asked to stop
/// has attempts in flight to wind down before it may report `Cancelled`,
/// and the record keeps both halves. A queued job has nothing to wind
/// down and may go straight to `Cancelled`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Accepted, not yet started.
    Queued,
    /// Dispatching and settling attempts.
    Running,
    /// Asked to stop; winding down.
    Cancelling,
    /// The coordinator finished. A claim about the job's bookkeeping,
    /// never a claim that every input was covered — the counts carry
    /// that.
    Done,
    /// The stop completed.
    Cancelled,
    /// The coordinator gave up, with the reason it recorded.
    Failed(String),
}

impl Status {
    /// A short name for the status, for an error or a report line.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Cancelling => "cancelling",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
            Self::Failed(_) => "failed",
        }
    }

    /// Whether the status is terminal — `Done`, `Cancelled`, or `Failed`.
    /// A terminal job does not leave.
    #[must_use]
    pub const fn terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Cancelled | Self::Failed(_))
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where one attempt stands.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttemptState {
    /// Dispatched, not yet settled — the state recovery orphans.
    Started,
    /// Resolved with a recorded outcome.
    Settled,
    /// Recovery closed it `unknown`: the book cannot say whether it ran.
    Orphaned,
}

/// One attempt's record: its state, and the outcome settlement left.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Attempt {
    /// The input it belongs to.
    pub input: String,
    /// The attempt number — `(input, attempt)` is the idempotency pair.
    pub attempt: u32,
    /// Where the attempt stands.
    pub state: AttemptState,
    /// The recorded outcome, when one settled — or `unknown` for an
    /// orphaned attempt.
    pub outcome: Option<Outcome>,
    /// Unix seconds when the attempt was recorded started.
    pub began_unix: u64,
    /// Unix seconds when it resolved, when it did.
    pub resolved_unix: Option<u64>,
}

/// A job's current state, folded from the record log.
///
/// `Job` is a folded record, not a wire type: the log is the durable
/// form, and this is what replay built.
#[derive(Clone, Debug)]
pub struct Job {
    /// The submission the book accepted.
    pub manifest: Manifest,
    /// Where the job stands.
    pub status: Status,
    /// Every recorded attempt, keyed `(input, attempt)`.
    pub attempts: BTreeMap<(String, u32), Attempt>,
    /// Unix seconds when the job was accepted.
    pub submitted_unix: u64,
}

impl Job {
    /// The job id — the manifest's stable id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.manifest.job
    }

    /// The job's bookkeeping: expected inputs, inputs ever attempted,
    /// and one standing per input under each outcome.
    ///
    /// An input's standing is the outcome of its highest-numbered
    /// resolved attempt, so a retry replaces the standing it earned
    /// rather than adding a second one. Inputs with no resolved attempt
    /// hold no standing — they are the gap [`Counts::unaccounted`] names.
    #[must_use]
    pub fn counts(&self) -> Counts {
        let mut counts = Counts {
            expected: self.manifest.inputs.len(),
            ..Counts::default()
        };
        let mut attempted = BTreeSet::new();
        // `attempts` iterates in ascending `(input, attempt)` order, so
        // the last standing written per input is its highest-numbered
        // resolved attempt.
        let mut standing: BTreeMap<&str, Outcome> = BTreeMap::new();
        for ((input, _), record) in &self.attempts {
            attempted.insert(input.as_str());
            if let Some(outcome) = record.outcome {
                standing.insert(input.as_str(), outcome);
            }
        }
        counts.attempted = attempted.len();
        for outcome in standing.values() {
            match outcome {
                Outcome::Answered => counts.answered += 1,
                Outcome::Refused => counts.refused += 1,
                Outcome::Unavailable => counts.unavailable += 1,
                Outcome::Unattempted => counts.unattempted += 1,
                Outcome::Unknown => counts.unknown += 1,
            }
        }
        counts
    }

    /// Whether the job reached `Done` — a claim about the coordinator's
    /// bookkeeping, never about coverage. Compare
    /// [`Counts::complete_coverage`].
    #[must_use]
    pub fn completed(&self) -> bool {
        self.status == Status::Done
    }
}

/// A job's bookkeeping, counted rather than narrated.
///
/// The outcome fields hold one standing per input, so they partition the
/// inputs the job resolved rather than the attempts it made — a retried
/// input still stands once, under the outcome its last resolved attempt
/// earned. `expected - attempted` names the inputs no attempt ever
/// reached; `attempted - settled()` the inputs whose attempts are still
/// open.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Counts {
    /// Inputs the manifest declared.
    pub expected: usize,
    /// Inputs with at least one recorded attempt.
    pub attempted: usize,
    /// Inputs whose standing is an answer.
    pub answered: usize,
    /// Inputs whose standing is a refusal.
    pub refused: usize,
    /// Inputs whose standing is unavailable.
    pub unavailable: usize,
    /// Inputs whose standing is unattempted.
    pub unattempted: usize,
    /// Inputs whose standing is unknown — orphaned, or marked unknown.
    pub unknown: usize,
}

impl Counts {
    /// Inputs holding a settled standing.
    #[must_use]
    pub fn settled(&self) -> usize {
        self.answered + self.refused + self.unavailable + self.unattempted + self.unknown
    }

    /// Inputs with no settled standing — never dispatched, or still open.
    /// These are what a reconciler has left to ask about.
    #[must_use]
    pub fn unaccounted(&self) -> usize {
        self.expected.saturating_sub(self.settled())
    }

    /// Whether every expected input holds a settled standing. This is
    /// the coverage claim [`Status::Done`] does not make: a job may be
    /// `Done` with inputs unanswered, and it reads as completed, never
    /// as covered.
    #[must_use]
    pub fn complete_coverage(&self) -> bool {
        self.attempted == self.expected && self.settled() == self.expected
    }
}

/// Everything the book refuses to do, and why.
#[derive(Debug, thiserror::Error)]
pub enum BookError {
    #[error("could not read {path}: {detail}")]
    Read { path: String, detail: String },

    #[error("could not write {path}: {detail}")]
    Write { path: String, detail: String },

    #[error("{path} line {line}: {detail}")]
    Corrupt {
        path: String,
        line: usize,
        detail: String,
    },

    #[error(
        "another writer holds {lock}{holder}. The book takes one writer at a \
         time: wait for that writer to finish, or remove the lock file if no \
         writer is running"
    )]
    Locked { lock: String, holder: String },

    #[error("the job manifest is invalid: {detail}")]
    Invalid { detail: String },

    #[error(
        "job `{key}` was already submitted for different content — an \
         idempotency key does not rename a new job"
    )]
    Conflict { key: String },

    #[error(
        "job `{job}` input `{input}` attempt {attempt} settled as {held}, so \
         marking it {requested} gives one attempt two outcomes"
    )]
    MarkConflict {
        job: String,
        input: String,
        attempt: u32,
        held: Outcome,
        requested: Outcome,
    },

    #[error("job `{job}` is {from}, so it cannot become {to}")]
    IllegalTransition {
        job: String,
        from: Status,
        to: Status,
    },

    #[error("the job log has no job `{job}`")]
    UnknownJob { job: String },

    #[error(
        "job `{job}` declared no input `{input}`, and the job log accepts only declared inputs"
    )]
    UnknownInput { job: String, input: String },

    #[error("job `{job}` is {status}; attempts begin only while a job is running or cancelling")]
    NotRunning { job: String, status: Status },

    #[error(
        "job `{job}` input `{input}` has no record that attempt {attempt} \
         started, and the job log accepts only attempts that started"
    )]
    Unstarted {
        job: String,
        input: String,
        attempt: u32,
    },

    #[error("job `{job}` input `{input}` attempt {attempt} is already resolved")]
    Resolved {
        job: String,
        input: String,
        attempt: u32,
    },
}

/// One line in the record log.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "record", rename_all = "kebab-case")]
enum Record {
    /// A submission the book accepted.
    Submitted { at: u64, manifest: Manifest },
    /// A job changed state.
    Transition { at: u64, job: String, to: Status },
    /// An attempt was recorded dispatched.
    Started {
        at: u64,
        job: String,
        input: String,
        attempt: u32,
    },
    /// An attempt resolved with its outcome.
    Settled {
        at: u64,
        job: String,
        input: String,
        attempt: u32,
        outcome: Outcome,
    },
    /// Recovery closed an unsettled attempt `unknown`.
    Orphaned {
        at: u64,
        job: String,
        input: String,
        attempt: u32,
    },
}

/// The exclusive lock one writer holds for the book's open lifetime.
///
/// Same shape as the store's and the quota ledger's: `create_new` makes
/// the lock atomic, the file's absence is the release, and a dropped
/// guard removes it. A writer killed outright leaves the file, and the
/// pid inside says who it was.
struct Lock {
    path: PathBuf,
}

impl Lock {
    fn acquire(book: &Path) -> Result<Self, BookError> {
        let path = lock_path(book);
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|e| BookError::Write {
                path: parent.display().to_string(),
                detail: e.to_string(),
            })?;
        }
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                let _ = writeln!(file, "pid {}", std::process::id());
                Ok(Self { path })
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Err(BookError::Locked {
                lock: path.display().to_string(),
                holder: match fs::read_to_string(&path) {
                    Ok(pid) if !pid.trim().is_empty() => format!(", held by {}", pid.trim()),
                    _ => String::new(),
                },
            }),
            Err(e) => Err(BookError::Write {
                path: path.display().to_string(),
                detail: e.to_string(),
            }),
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn lock_path(book: &Path) -> PathBuf {
    let mut name = book.as_os_str().to_os_string();
    name.push(".lock");
    PathBuf::from(name)
}

/// The open book: the record log folded into current job state.
///
/// Open once per coordinating process — the lock is held for the
/// lifetime, so a second [`Book::recover`] on the same path is refused
/// rather than interleaved.
pub struct Book {
    path: PathBuf,
    jobs: BTreeMap<String, Job>,
    _lock: Lock,
}

impl Book {
    /// Open the book at `path`: replay the log, truncate a torn tail,
    /// and close every attempt recorded started but never settled as
    /// `unknown`.
    ///
    /// A book that does not exist yet recovers as empty and is created by
    /// the first record. Recovery is the only way in — every open is a
    /// recovery, because the questions the book answers are the ones a
    /// restart asks.
    pub fn recover(path: impl Into<PathBuf>) -> Result<Self, BookError> {
        let path = path.into();
        let lock = Lock::acquire(&path)?;
        let mut book = Self {
            path,
            jobs: BTreeMap::new(),
            _lock: lock,
        };
        book.replay()?;
        book.sweep()?;
        Ok(book)
    }

    /// Where the record log lives.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// One job's folded record, when the book holds it.
    #[must_use]
    pub fn job(&self, job: &str) -> Option<&Job> {
        self.jobs.get(job)
    }

    /// Every job the book holds — what a reconciler iterates to ask what
    /// was accepted.
    pub fn jobs(&self) -> impl Iterator<Item = &Job> {
        self.jobs.values()
    }

    /// Accept a submission.
    ///
    /// The manifest lands on disk before the job is acknowledged, so a
    /// job this call returns is a job the book holds. Resubmitting the
    /// same job id with identical content returns the original record —
    /// a retry is not a second job. The same id with changed content is
    /// a [`BookError::Conflict`] naming the key: an idempotency key does
    /// not rename a new job.
    pub fn submit(&mut self, manifest: &Manifest) -> Result<Job, BookError> {
        manifest
            .validate()
            .map_err(|detail| BookError::Invalid { detail })?;
        if let Some(held) = self.jobs.get(&manifest.job) {
            if held.manifest.digest == manifest.digest {
                return Ok(held.clone());
            }
            return Err(BookError::Conflict {
                key: manifest.job.clone(),
            });
        }
        let at = unix_now();
        self.append(&Record::Submitted {
            at,
            manifest: manifest.clone(),
        })?;
        let job = Job {
            manifest: manifest.clone(),
            status: Status::Queued,
            attempts: BTreeMap::new(),
            submitted_unix: at,
        };
        self.jobs.insert(manifest.job.clone(), job.clone());
        Ok(job)
    }

    /// Move a job to `to`, recording the transition.
    ///
    /// A transition the machine does not allow is refused by name —
    /// [`BookError::IllegalTransition`] carries the states it names. A
    /// terminal job accepts no transition, including back into itself.
    pub fn transition(&mut self, job: &str, to: Status) -> Result<Job, BookError> {
        let held = self
            .jobs
            .get(job)
            .ok_or_else(|| BookError::UnknownJob { job: job.into() })?;
        if !legal_transition(&held.status, &to) {
            return Err(BookError::IllegalTransition {
                job: job.into(),
                from: held.status.clone(),
                to,
            });
        }
        let at = unix_now();
        self.append(&Record::Transition {
            at,
            job: job.into(),
            to: to.clone(),
        })?;
        let record = self.jobs.get_mut(job).expect("the job was checked above");
        record.status = to;
        Ok(record.clone())
    }

    /// Record that an input's attempt was dispatched.
    ///
    /// Attempts begin only while a job is `Running` or `Cancelling` — a
    /// queued job has dispatched nothing, and a terminal job dispatches
    /// nothing more. Beginning the same `(input, attempt)` twice returns
    /// the record that exists; beginning one already resolved is refused.
    pub fn begin(&mut self, job: &str, input: &str, attempt: u32) -> Result<Attempt, BookError> {
        let held = self
            .jobs
            .get(job)
            .ok_or_else(|| BookError::UnknownJob { job: job.into() })?;
        if !held
            .manifest
            .inputs
            .iter()
            .any(|declared| declared.id == input)
        {
            return Err(BookError::UnknownInput {
                job: job.into(),
                input: input.into(),
            });
        }
        match held.status {
            Status::Running | Status::Cancelling => {}
            ref status => {
                return Err(BookError::NotRunning {
                    job: job.into(),
                    status: status.clone(),
                });
            }
        }
        let key = (input.to_string(), attempt);
        if let Some(existing) = held.attempts.get(&key) {
            return match existing.state {
                AttemptState::Started => Ok(existing.clone()),
                _ => Err(BookError::Resolved {
                    job: job.into(),
                    input: input.into(),
                    attempt,
                }),
            };
        }
        let at = unix_now();
        self.append(&Record::Started {
            at,
            job: job.into(),
            input: input.into(),
            attempt,
        })?;
        let record = self
            .jobs
            .get_mut(job)
            .expect("the job was checked above")
            .attempts
            .entry(key)
            .insert_entry(Attempt {
                input: input.into(),
                attempt,
                state: AttemptState::Started,
                outcome: None,
                began_unix: at,
                resolved_unix: None,
            })
            .into_mut();
        Ok(record.clone())
    }

    /// Record an attempt's outcome.
    ///
    /// The same `(input, attempt)` marked twice with the same outcome is
    /// the same record — a settlement that arrived twice is not the
    /// settlement twice — and marked with a different outcome is a
    /// [`BookError::MarkConflict`], because one attempt does not have two
    /// outcomes. An input the manifest does not declare is refused, and
    /// an attempt that was never begun is refused: the book does not
    /// invent either. An orphaned attempt is closed — a late answer
    /// conflicts with the `unknown` recovery recorded.
    pub fn mark(
        &mut self,
        job: &str,
        input: &str,
        attempt: u32,
        outcome: Outcome,
    ) -> Result<Attempt, BookError> {
        let held = self
            .jobs
            .get(job)
            .ok_or_else(|| BookError::UnknownJob { job: job.into() })?;
        if !held
            .manifest
            .inputs
            .iter()
            .any(|declared| declared.id == input)
        {
            return Err(BookError::UnknownInput {
                job: job.into(),
                input: input.into(),
            });
        }
        let key = (input.to_string(), attempt);
        match held.attempts.get(&key) {
            Some(existing) => match existing.outcome {
                Some(held_outcome) if held_outcome == outcome => return Ok(existing.clone()),
                Some(held_outcome) => {
                    return Err(BookError::MarkConflict {
                        job: job.into(),
                        input: input.into(),
                        attempt,
                        held: held_outcome,
                        requested: outcome,
                    });
                }
                None => {}
            },
            None => {
                return Err(BookError::Unstarted {
                    job: job.into(),
                    input: input.into(),
                    attempt,
                });
            }
        }
        let at = unix_now();
        self.append(&Record::Settled {
            at,
            job: job.into(),
            input: input.into(),
            attempt,
            outcome,
        })?;
        let record = self
            .jobs
            .get_mut(job)
            .expect("the job was checked above")
            .attempts
            .get_mut(&key)
            .expect("the attempt was checked above");
        record.state = AttemptState::Settled;
        record.outcome = Some(outcome);
        record.resolved_unix = Some(at);
        Ok(record.clone())
    }

    /// Append one record, fsynced before the call returns — the same
    /// write path the store's append takes.
    fn append(&mut self, record: &Record) -> Result<(), BookError> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|e| BookError::Write {
                path: parent.display().to_string(),
                detail: e.to_string(),
            })?;
        }
        let mut line = serde_json::to_string(record).map_err(|e| BookError::Write {
            path: self.path.display().to_string(),
            detail: e.to_string(),
        })?;
        line.push('\n');
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| BookError::Write {
                path: self.path.display().to_string(),
                detail: e.to_string(),
            })?;
        file.write_all(line.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|e| BookError::Write {
                path: self.path.display().to_string(),
                detail: e.to_string(),
            })
    }

    /// Fold the record log into current job state, truncating a torn
    /// tail first.
    ///
    /// A last line that never finished writing is a torn tail: the record
    /// was never acknowledged, so the book drops it honestly — the file
    /// is cut back to the last complete line and fsynced, and replay runs
    /// on what survived. A record that fails anywhere but the tail is
    /// corruption, not a tear, and it is named rather than skipped.
    fn replay(&mut self) -> Result<(), BookError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(BookError::Read {
                    path: self.path.display().to_string(),
                    detail: e.to_string(),
                });
            }
        };
        let mut good = bytes.len();
        if good > 0 && !bytes.ends_with(b"\n") {
            good = bytes
                .iter()
                .rposition(|byte| *byte == b'\n')
                .map_or(0, |index| index + 1);
            truncate(&self.path, good as u64)?;
        }
        let text = std::str::from_utf8(&bytes[..good]).map_err(|e| BookError::Corrupt {
            path: self.path.display().to_string(),
            line: 0,
            detail: format!("the log is not UTF-8: {e}"),
        })?;
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            if raw.trim().is_empty() {
                continue;
            }
            let record: Record = serde_json::from_str(raw).map_err(|e| BookError::Corrupt {
                path: self.path.display().to_string(),
                line,
                detail: format!("not a record: {e}"),
            })?;
            self.fold(record, line)?;
        }
        Ok(())
    }

    /// Apply one replayed record to the folded state. Anything the log
    /// should not contain — a transition the machine refuses, a
    /// settlement for an attempt never begun, a job submitted twice under
    /// different content — is corruption, named by line.
    fn fold(&mut self, record: Record, line: usize) -> Result<(), BookError> {
        let corrupt = |detail: String| BookError::Corrupt {
            path: self.path.display().to_string(),
            line,
            detail,
        };
        match record {
            Record::Submitted { at, manifest } => {
                manifest.validate().map_err(|detail| {
                    corrupt(format!("the submitted manifest is invalid: {detail}"))
                })?;
                match self.jobs.get(&manifest.job) {
                    None => {
                        self.jobs.insert(
                            manifest.job.clone(),
                            Job {
                                manifest,
                                status: Status::Queued,
                                attempts: BTreeMap::new(),
                                submitted_unix: at,
                            },
                        );
                    }
                    Some(held) if held.manifest.digest == manifest.digest => {}
                    Some(_) => {
                        return Err(corrupt(format!(
                            "job `{}` is submitted twice under different content",
                            manifest.job
                        )));
                    }
                }
            }
            Record::Transition { job, to, .. } => {
                let held = self.jobs.get_mut(&job).ok_or_else(|| {
                    corrupt(format!(
                        "a transition names job `{job}`, which the log never submitted"
                    ))
                })?;
                if !legal_transition(&held.status, &to) {
                    return Err(corrupt(format!(
                        "job `{job}` transitions {} to {}, which the state machine refuses",
                        held.status, to
                    )));
                }
                held.status = to;
            }
            Record::Started {
                at,
                job,
                input,
                attempt,
            } => {
                let held = self.jobs.get_mut(&job).ok_or_else(|| {
                    corrupt(format!(
                        "a start names job `{job}`, which the log never submitted"
                    ))
                })?;
                if !held
                    .manifest
                    .inputs
                    .iter()
                    .any(|declared| declared.id == input)
                {
                    return Err(corrupt(format!(
                        "job `{job}` begins an attempt under input `{input}`, which its \
                         manifest does not declare"
                    )));
                }
                match held.attempts.get(&(input.clone(), attempt)) {
                    None => {
                        held.attempts.insert(
                            (input.clone(), attempt),
                            Attempt {
                                input,
                                attempt,
                                state: AttemptState::Started,
                                outcome: None,
                                began_unix: at,
                                resolved_unix: None,
                            },
                        );
                    }
                    Some(existing)
                        if existing.state == AttemptState::Started
                            && existing.outcome.is_none() => {}
                    Some(_) => {
                        return Err(corrupt(format!(
                            "job `{job}` input `{input}` attempt {attempt} is recorded \
                             started after it resolved"
                        )));
                    }
                }
            }
            Record::Settled {
                at,
                job,
                input,
                attempt,
                outcome,
            } => {
                let held = self.jobs.get_mut(&job).ok_or_else(|| {
                    corrupt(format!(
                        "a settlement names job `{job}`, which the log never submitted"
                    ))
                })?;
                match held.attempts.get_mut(&(input.clone(), attempt)) {
                    Some(existing) => match existing.outcome {
                        None => {
                            existing.state = AttemptState::Settled;
                            existing.outcome = Some(outcome);
                            existing.resolved_unix = Some(at);
                        }
                        Some(held_outcome) if held_outcome == outcome => {}
                        Some(held_outcome) => {
                            return Err(corrupt(format!(
                                "job `{job}` input `{input}` attempt {attempt} settled \
                                 {held_outcome} and then settled {outcome}"
                            )));
                        }
                    },
                    None => {
                        return Err(corrupt(format!(
                            "job `{job}` input `{input}` attempt {attempt} settles, but the \
                             log never recorded it started"
                        )));
                    }
                }
            }
            Record::Orphaned {
                at,
                job,
                input,
                attempt,
            } => {
                let held = self.jobs.get_mut(&job).ok_or_else(|| {
                    corrupt(format!(
                        "an orphan names job `{job}`, which the log never submitted"
                    ))
                })?;
                match held.attempts.get_mut(&(input.clone(), attempt)) {
                    Some(existing) => match existing.state {
                        AttemptState::Started if existing.outcome.is_none() => {
                            existing.state = AttemptState::Orphaned;
                            existing.outcome = Some(Outcome::Unknown);
                            existing.resolved_unix = Some(at);
                        }
                        AttemptState::Orphaned => {}
                        _ => {
                            return Err(corrupt(format!(
                                "job `{job}` input `{input}` attempt {attempt} is orphaned \
                                 after it resolved"
                            )));
                        }
                    },
                    None => {
                        return Err(corrupt(format!(
                            "job `{job}` input `{input}` attempt {attempt} is orphaned, but \
                             the log never recorded it started"
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Close every attempt still recorded started. The response may have
    /// been lost either way, so the honest close is `unknown`, written
    /// durably rather than guessed at — and reconciliation over it is the
    /// caller's explicit work, never a silent replay.
    fn sweep(&mut self) -> Result<(), BookError> {
        let now = unix_now();
        let unsettled: Vec<(String, String, u32)> = self
            .jobs
            .iter()
            .flat_map(|(job, record)| {
                record
                    .attempts
                    .iter()
                    .filter(|(_, existing)| {
                        existing.state == AttemptState::Started && existing.outcome.is_none()
                    })
                    .map(move |((input, attempt), _)| (job.clone(), input.clone(), *attempt))
            })
            .collect();
        for (job, input, attempt) in unsettled {
            self.append(&Record::Orphaned {
                at: now,
                job: job.clone(),
                input: input.clone(),
                attempt,
            })?;
            let record = self
                .jobs
                .get_mut(&job)
                .expect("the job was just iterated")
                .attempts
                .get_mut(&(input, attempt))
                .expect("the attempt was just iterated");
            record.state = AttemptState::Orphaned;
            record.outcome = Some(Outcome::Unknown);
            record.resolved_unix = Some(now);
        }
        Ok(())
    }
}

/// The transitions the state machine allows.
///
/// `Running` to `Cancelled` is not among them on purpose: a running job
/// has attempts in flight, and it winds down through `Cancelling` so the
/// record shows the wind-down happened. A queued job has nothing to wind
/// down, so it may cancel outright.
fn legal_transition(from: &Status, to: &Status) -> bool {
    matches!(
        (from, to),
        (
            Status::Queued,
            Status::Running | Status::Cancelling | Status::Cancelled | Status::Failed(_)
        ) | (
            Status::Running,
            Status::Done | Status::Cancelling | Status::Failed(_)
        ) | (Status::Cancelling, Status::Cancelled | Status::Failed(_))
    )
}

/// Cut the log to `len` bytes — the torn tail a crash left behind — and
/// fsync the truncation, so the honest prefix is what survives.
fn truncate(path: &Path, len: u64) -> Result<(), BookError> {
    let file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| BookError::Write {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
    file.set_len(len)
        .and_then(|()| file.sync_all())
        .map_err(|e| BookError::Write {
            path: path.display().to_string(),
            detail: e.to_string(),
        })
}

/// Unix seconds now.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default()
}

/// Canonical JSON: keys sorted, whitespace gone — the same
/// canonicalization the tenancy manifest's digest uses, so two writers
/// digest the same submission to the same bytes.
fn canonicalize(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = String::from("{");
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).expect("a key serializes"));
                out.push(':');
                out.push_str(&canonicalize(&map[*key]));
            }
            out.push('}');
            out
        }
        Value::Array(items) => {
            let mut out = String::from("[");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&canonicalize(item));
            }
            out.push(']');
            out
        }
        other => serde_json::to_string(other).expect("a scalar serializes"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest(job: &str, ids: &[&str]) -> Manifest {
        let mut manifest = Manifest {
            v: SCHEMA.to_string(),
            job: job.to_string(),
            tenant: "acme".to_string(),
            inputs: ids
                .iter()
                .map(|id| Input {
                    id: id.to_string(),
                    request: format!("req-{id}"),
                    attempt: 1,
                })
                .collect(),
            policy: "batch-v1".to_string(),
            model: "kev-1".to_string(),
            digest: String::new(),
        };
        manifest.seal();
        manifest
    }

    fn lines(path: &Path) -> Vec<String> {
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn records_named(path: &Path, name: &str) -> usize {
        lines(path)
            .iter()
            .filter(|line| {
                serde_json::from_str::<Value>(line)
                    .is_ok_and(|value| value["record"] == json!(name))
            })
            .count()
    }

    #[test]
    fn submit_persists_the_manifest_before_it_acknowledges() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.jsonl");
        let mut book = Book::recover(&path).unwrap();
        let manifest = manifest("job-1", &["a", "b"]);
        let job = book.submit(&manifest).unwrap();
        assert_eq!(job.status, Status::Queued);

        // The acknowledgment came back, and the manifest was already on
        // disk: a job the book returned is a job the book holds.
        let on_disk = lines(&path);
        assert_eq!(on_disk.len(), 1);
        let record: Value = serde_json::from_str(&on_disk[0]).unwrap();
        assert_eq!(record["record"], json!("submitted"));
        assert_eq!(record["manifest"]["job"], json!("job-1"));
        assert_eq!(record["manifest"]["digest"], json!(manifest.digest));

        // And a fresh open sees the accepted job without a resubmission.
        drop(book);
        let reopened = Book::recover(&path).unwrap();
        assert_eq!(reopened.job("job-1").unwrap().status, Status::Queued);
    }

    #[test]
    fn the_same_key_with_changed_content_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.jsonl");
        let mut book = Book::recover(&path).unwrap();
        book.submit(&manifest("job-1", &["a", "b"])).unwrap();

        let changed = manifest("job-1", &["a", "b", "c"]);
        let error = book.submit(&changed).unwrap_err();
        assert!(matches!(error, BookError::Conflict { .. }), "{error}");
        assert!(error.to_string().contains("job-1"), "{error}");
        assert_eq!(lines(&path).len(), 1, "a refused submission writes nothing");
    }

    #[test]
    fn the_same_key_with_the_same_content_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.jsonl");
        let mut book = Book::recover(&path).unwrap();
        let first = book.submit(&manifest("job-1", &["a"])).unwrap();
        let again = book.submit(&manifest("job-1", &["a"])).unwrap();
        assert_eq!(again.manifest.digest, first.manifest.digest);
        assert_eq!(again.submitted_unix, first.submitted_unix);
        assert_eq!(lines(&path).len(), 1, "an idempotent retry writes nothing");
        assert_eq!(book.jobs().count(), 1);
    }

    #[test]
    fn illegal_transitions_refuse_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.jsonl");
        let mut book = Book::recover(&path).unwrap();
        book.submit(&manifest("job-1", &["a"])).unwrap();

        // A queued job cannot be done — nothing about it ran.
        let error = book.transition("job-1", Status::Done).unwrap_err();
        assert!(
            matches!(error, BookError::IllegalTransition { .. }),
            "{error}"
        );
        assert!(error.to_string().contains("queued"), "{error}");
        assert!(error.to_string().contains("done"), "{error}");

        // A running job must wind down through cancelling.
        book.transition("job-1", Status::Running).unwrap();
        let error = book.transition("job-1", Status::Cancelled).unwrap_err();
        assert!(error.to_string().contains("running"), "{error}");
        assert!(error.to_string().contains("cancelled"), "{error}");
        book.transition("job-1", Status::Cancelling).unwrap();
        book.transition("job-1", Status::Cancelled).unwrap();

        // A terminal job does not leave.
        let error = book.transition("job-1", Status::Running).unwrap_err();
        assert!(error.to_string().contains("cancelled"), "{error}");

        // A queued job has nothing to wind down: it may cancel outright,
        // and a failure carries the reason it recorded.
        book.submit(&manifest("job-2", &["a"])).unwrap();
        book.transition("job-2", Status::Cancelled).unwrap();
        book.submit(&manifest("job-3", &["a"])).unwrap();
        book.transition("job-3", Status::Running).unwrap();
        book.transition("job-3", Status::Failed("backend gone".into()))
            .unwrap();
        assert_eq!(
            book.job("job-3").unwrap().status,
            Status::Failed("backend gone".into())
        );
    }

    #[test]
    fn every_outcome_is_counted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.jsonl");
        let mut book = Book::recover(&path).unwrap();
        book.submit(&manifest("job-1", &["a", "b", "c", "d", "e", "f"]))
            .unwrap();
        book.transition("job-1", Status::Running).unwrap();
        for (input, outcome) in [
            ("a", Outcome::Answered),
            ("b", Outcome::Refused),
            ("c", Outcome::Unavailable),
            ("d", Outcome::Unattempted),
            ("e", Outcome::Unknown),
        ] {
            book.begin("job-1", input, 1).unwrap();
            book.mark("job-1", input, 1, outcome).unwrap();
        }
        // f was dispatched and never settled: attempted, not accounted.
        book.begin("job-1", "f", 1).unwrap();

        let counts = book.job("job-1").unwrap().counts();
        assert_eq!(counts.expected, 6);
        assert_eq!(counts.attempted, 6);
        assert_eq!(
            (
                counts.answered,
                counts.refused,
                counts.unavailable,
                counts.unattempted,
                counts.unknown
            ),
            (1, 1, 1, 1, 1)
        );
        assert_eq!(counts.settled(), 5);
        assert_eq!(counts.unaccounted(), 1);
    }

    #[test]
    fn done_is_completed_not_complete_coverage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.jsonl");
        let mut book = Book::recover(&path).unwrap();
        book.submit(&manifest("job-1", &["a", "b", "c"])).unwrap();
        book.transition("job-1", Status::Running).unwrap();
        book.begin("job-1", "a", 1).unwrap();
        book.mark("job-1", "a", 1, Outcome::Answered).unwrap();
        book.transition("job-1", Status::Done).unwrap();

        let job = book.job("job-1").unwrap();
        assert!(job.completed(), "the job reached done");
        let counts = job.counts();
        assert!(
            !counts.complete_coverage(),
            "two inputs were never answered"
        );
        assert_eq!(
            (counts.expected, counts.attempted, counts.answered),
            (3, 1, 1)
        );
        assert_eq!(counts.unaccounted(), 2);

        // A job that settled every input carries the distinction the
        // other way.
        book.submit(&manifest("job-2", &["a", "b"])).unwrap();
        book.transition("job-2", Status::Running).unwrap();
        for input in ["a", "b"] {
            book.begin("job-2", input, 1).unwrap();
            book.mark("job-2", input, 1, Outcome::Answered).unwrap();
        }
        book.transition("job-2", Status::Done).unwrap();
        let counts = book.job("job-2").unwrap().counts();
        assert!(counts.complete_coverage());
    }

    #[test]
    fn a_duplicate_mark_noops_and_a_divergent_mark_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.jsonl");
        let mut book = Book::recover(&path).unwrap();
        book.submit(&manifest("job-1", &["a"])).unwrap();
        book.transition("job-1", Status::Running).unwrap();
        book.begin("job-1", "a", 1).unwrap();
        book.mark("job-1", "a", 1, Outcome::Answered).unwrap();
        assert_eq!(records_named(&path, "settled"), 1);

        // The settlement arrived twice; the second mark is the same
        // record, not a second settlement.
        let again = book.mark("job-1", "a", 1, Outcome::Answered).unwrap();
        assert_eq!(again.outcome, Some(Outcome::Answered));
        assert_eq!(records_named(&path, "settled"), 1);

        // One attempt does not have two outcomes.
        let error = book.mark("job-1", "a", 1, Outcome::Refused).unwrap_err();
        assert!(
            matches!(
                error,
                BookError::MarkConflict {
                    held: Outcome::Answered,
                    requested: Outcome::Refused,
                    ..
                }
            ),
            "{error}"
        );

        // A second attempt of the same input settles on its own.
        book.begin("job-1", "a", 2).unwrap();
        book.mark("job-1", "a", 2, Outcome::Answered).unwrap();
        assert_eq!(records_named(&path, "settled"), 2);
    }

    #[test]
    fn marks_outside_the_manifest_refuse() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.jsonl");
        let mut book = Book::recover(&path).unwrap();
        book.submit(&manifest("job-1", &["a"])).unwrap();
        book.transition("job-1", Status::Running).unwrap();

        let error = book
            .mark("job-1", "ghost", 1, Outcome::Answered)
            .unwrap_err();
        assert!(matches!(error, BookError::UnknownInput { .. }), "{error}");
        assert!(error.to_string().contains("ghost"), "{error}");

        // A declared input still needs the attempt begun first.
        let error = book.mark("job-1", "a", 1, Outcome::Answered).unwrap_err();
        assert!(matches!(error, BookError::Unstarted { .. }), "{error}");

        let error = book.mark("job-2", "a", 1, Outcome::Answered).unwrap_err();
        assert!(matches!(error, BookError::UnknownJob { .. }), "{error}");
    }

    #[test]
    fn recover_preserves_work_orphans_unsettled_attempts_and_never_resettles() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.jsonl");
        {
            let mut book = Book::recover(&path).unwrap();
            book.submit(&manifest("job-1", &["a", "b", "c"])).unwrap();
            book.transition("job-1", Status::Running).unwrap();
            book.begin("job-1", "a", 1).unwrap();
            book.mark("job-1", "a", 1, Outcome::Answered).unwrap();
            book.begin("job-1", "b", 1).unwrap();
            // c was never attempted, and the book drops mid-flight.
        }

        let mut book = Book::recover(&path).unwrap();
        let job = book.job("job-1").unwrap();
        // Accepted work is never lost.
        assert_eq!(job.manifest.inputs.len(), 3);
        assert_eq!(
            job.attempts.get(&("a".to_string(), 1)).unwrap().outcome,
            Some(Outcome::Answered)
        );
        // Started but never settled is unknown, explicitly.
        let orphaned = job.attempts.get(&("b".to_string(), 1)).unwrap();
        assert_eq!(orphaned.state, AttemptState::Orphaned);
        assert_eq!(orphaned.outcome, Some(Outcome::Unknown));
        let counts = job.counts();
        assert_eq!(
            (counts.attempted, counts.answered, counts.unknown),
            (2, 1, 1)
        );
        // The orphan record is durable: one line, written once.
        assert_eq!(records_named(&path, "orphaned"), 1);

        // An orphaned attempt is closed: a late answer conflicts with the
        // unknown recovery recorded, and the same unknown is a no-op.
        assert!(matches!(
            book.mark("job-1", "b", 1, Outcome::Answered),
            Err(BookError::MarkConflict {
                held: Outcome::Unknown,
                ..
            })
        ));
        book.mark("job-1", "b", 1, Outcome::Unknown).unwrap();

        // A second recovery replays neither the orphan nor the
        // settlement: nothing is applied twice.
        drop(book);
        let book = Book::recover(&path).unwrap();
        assert_eq!(records_named(&path, "orphaned"), 1);
        assert_eq!(records_named(&path, "settled"), 1);
        let job = book.job("job-1").unwrap();
        assert_eq!(
            job.attempts.get(&("a".to_string(), 1)).unwrap().outcome,
            Some(Outcome::Answered)
        );
        assert_eq!(
            job.attempts.get(&("b".to_string(), 1)).unwrap().state,
            AttemptState::Orphaned
        );
    }

    #[test]
    fn a_torn_tail_truncates_honestly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.jsonl");
        {
            let mut book = Book::recover(&path).unwrap();
            book.submit(&manifest("job-1", &["a"])).unwrap();
            book.transition("job-1", Status::Running).unwrap();
            book.begin("job-1", "a", 1).unwrap();
            book.mark("job-1", "a", 1, Outcome::Answered).unwrap();
        }
        let good_len = fs::metadata(&path).unwrap().len();

        // A crash mid-write leaves a partial record on the tail.
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"{\"record\":\"settled\",\"job\":\"job-1\",\"inpu")
            .unwrap();
        file.sync_all().unwrap();
        drop(file);

        let book = Book::recover(&path).unwrap();
        // The torn record is gone — it was never acknowledged — and
        // every complete record survived.
        assert_eq!(fs::metadata(&path).unwrap().len(), good_len);
        let job = book.job("job-1").unwrap();
        assert_eq!(job.status, Status::Running);
        assert_eq!(
            job.attempts.get(&("a".to_string(), 1)).unwrap().outcome,
            Some(Outcome::Answered)
        );
    }

    #[test]
    fn a_second_writer_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.jsonl");
        let _book = Book::recover(&path).unwrap();
        assert!(matches!(
            Book::recover(&path),
            Err(BookError::Locked { .. })
        ));
    }
}
