//! Complete command evidence (ENV-02a): every argument, stdout, and stderr
//! byte of a run's tool calls, paired with exact call identity, redacted of
//! selected credentials before anything is persisted.
//!
//! A [`Recorder`] owns one private evidence directory:
//!
//! - `events.jsonl`: append-only [`Entry`] lines, each with a stable,
//!   strictly increasing `seq`. Readers trust events, never raw file sizes;
//!   paging and replay (ENV-02b) read this log by sequence.
//! - `streams/<call>.stdout|stderr`: the retained (post-redaction) bytes of
//!   each stream. Every [`Event::Output`] names its offset, length, and
//!   SHA-256 in that file.
//! - `children/<id>/`: archived nested child runs, each a full evidence
//!   directory linked by manifest digest.
//! - `manifest.json`: the sealed [`Manifest`], written once at
//!   [`Recorder::finish`]; its file digest is the evidence digest a
//!   verification cites.
//!
//! Bytes are spooled, then the event is appended; both are synced before the
//! [`Ack`] reports the input as persisted. Selected credential values are
//! replaced in memory first ([`Redactor`]); lengths and digests describe
//! the retained bytes. A budget bounds retained bytes: exhausting it keeps
//! what fits, records exactly what was dropped, and seals the evidence
//! [`EvidenceStatus::Incomplete`], which [`crate::transition`] refuses to
//! let pass verification or save a version. Nothing here caps output for
//! display; model-visible excerpts are separate ([`Recorder::atif_call`]).

pub mod redact;

pub use redact::{REDACTION_MARKER, Redactor, Scrubbed};

use crate::{RunLink, digest, valid_digest, valid_id};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const EVIDENCE_SCHEMA: &str = "openagents.environment.evidence.v1";
pub const MAX_CALLS: usize = 4096;
pub const MAX_NOTE_BYTES: usize = 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum EvidenceError {
    Invalid(&'static str),
    Credential(&'static str),
    UnknownCall(String),
    DuplicateCall(String),
    /// Output arrived for a stream that was already closed.
    Closed(String),
    Finalized,
    Corrupt(&'static str),
    Io(&'static str),
}
impl std::fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(m) | Self::Credential(m) | Self::Corrupt(m) | Self::Io(m) => {
                f.write_str(m)
            }
            Self::UnknownCall(id) => write!(f, "No call {id} was started."),
            Self::DuplicateCall(id) => write!(f, "Call {id} was already started."),
            Self::Closed(id) => write!(f, "Call {id} received output after its stream closed."),
            Self::Finalized => f.write_str("The evidence is already finalized."),
        }
    }
}
impl std::error::Error for EvidenceError {}
pub type Result<T> = std::result::Result<T, EvidenceError>;

/// Evidence lifecycle. Execution completion never implies complete evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Recording,
    /// Finalized: every byte of every call is retained unchanged.
    Complete,
    /// Finalized: every byte is retained after declared redaction.
    CompleteWithRedactions,
    /// Finalized with a budget truncation, gap, unresolved call, capture
    /// error, or incomplete child.
    Incomplete,
    Unavailable,
}
impl EvidenceStatus {
    pub fn complete(self) -> bool {
        matches!(self, Self::Complete | Self::CompleteWithRedactions)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamName {
    Stdout,
    Stderr,
}
impl StreamName {
    fn index(self) -> usize {
        self as usize
    }
    fn word(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

/// Exact identity of one tool call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallIdentity {
    /// Stable within this evidence record.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub run: RunLink,
    pub tool: String,
    /// The source request that caused the call (a turn or tool-call ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    /// The backend operation that ran it (a Boat process ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
}

/// One direct result observation. A call can see several (an engine error,
/// then an exit); [`CallOutcome`] folds them without letting a later
/// success hide an earlier failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CallResult {
    Exited {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<i64>,
        success: bool,
    },
    TimedOut {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<i64>,
    },
    Signalled {
        signal: String,
    },
    /// The engine or transport reported a failure.
    EngineError {
        message: String,
    },
    /// The recorder could not learn how the call ended.
    Unresolved {
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallOutcome {
    Running,
    Succeeded,
    Failed,
    Unresolved,
}
impl CallOutcome {
    /// Any failure is final; otherwise a definite success settles an
    /// earlier unknown.
    fn fold(results: &[CallResult]) -> Self {
        let (mut failed, mut succeeded, mut unresolved) = (false, false, false);
        for r in results {
            match r {
                CallResult::Exited { success: true, .. } => succeeded = true,
                CallResult::Unresolved { .. } => unresolved = true,
                _ => failed = true,
            }
        }
        if failed {
            Self::Failed
        } else if succeeded {
            Self::Succeeded
        } else if unresolved {
            Self::Unresolved
        } else {
            Self::Running
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamState {
    Open,
    /// The source declared the stream ended and every byte is retained.
    Complete,
    /// The budget ran out; `dropped_bytes` were not retained.
    Truncated,
    /// The stream ended without a definite end (lost engine, unfinished
    /// call), so retained bytes are not proven to be all of it.
    Gap,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamSummary {
    pub state: StreamState,
    /// Retained bytes and their SHA-256.
    pub length: u64,
    pub digest: String,
    pub redactions: u64,
    /// Bytes the source delivered, before redaction.
    pub input_bytes: u64,
    /// Scrubbed bytes the budget could not retain.
    pub dropped_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seq: Option<u64>,
}

/// A nested child run archived under this evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildLink {
    pub evidence_id: String,
    pub call: String,
    pub run: RunLink,
    pub manifest_digest: String,
    pub status: EvidenceStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallSummary {
    pub identity: CallIdentity,
    /// Retained (post-redaction) arguments; `None` when the budget could
    /// not hold them.
    #[serde(default)]
    pub arguments: Option<Value>,
    pub arguments_digest: String,
    pub arguments_bytes: u64,
    pub argument_redactions: u64,
    pub stdout: StreamSummary,
    pub stderr: StreamSummary,
    pub results: Vec<CallResult>,
    pub outcome: CallOutcome,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capture_errors: Vec<String>,
    pub started_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_ms: Option<u64>,
}
impl CallSummary {
    pub fn stream(&self, name: StreamName) -> &StreamSummary {
        match name {
            StreamName::Stdout => &self.stdout,
            StreamName::Stderr => &self.stderr,
        }
    }
    fn stream_mut(&mut self, name: StreamName) -> &mut StreamSummary {
        match name {
            StreamName::Stdout => &mut self.stdout,
            StreamName::Stderr => &mut self.stderr,
        }
    }
    fn redacted(&self) -> bool {
        self.argument_redactions + self.stdout.redactions + self.stderr.redactions > 0
    }
    fn complete(&self) -> bool {
        self.arguments.is_some()
            && self.outcome != CallOutcome::Running
            && self.outcome != CallOutcome::Unresolved
            && self.capture_errors.is_empty()
            && self.stdout.state == StreamState::Complete
            && self.stderr.state == StreamState::Complete
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetUse {
    pub limit: u64,
    /// Retained bytes, including bytes reserved for child runs.
    pub used: u64,
    pub exhausted: bool,
}

/// The sealed evidence record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunLink>,
    pub status: EvidenceStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
    pub budget: BudgetUse,
    pub calls: Vec<CallSummary>,
    pub children: Vec<ChildLink>,
    /// Sequence of the last event before the manifest was sealed.
    pub last_seq: u64,
    pub finalized_ms: u64,
}

/// A finalized evidence record's citation: what a verification reports.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sealed {
    pub id: String,
    pub digest: String,
    pub status: EvidenceStatus,
}
impl Sealed {
    /// The verdict for checks that passed: `Passed` with this evidence,
    /// which the transition turns into `Incomplete` unless it is complete.
    pub fn passed(&self) -> crate::transition::VerificationObservation {
        crate::transition::VerificationObservation::Passed {
            evidence_digest: self.digest.clone(),
            evidence: self.status,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    CallStarted {
        call: CallIdentity,
        #[serde(default)]
        arguments: Option<Value>,
        arguments_digest: String,
        arguments_bytes: u64,
        argument_redactions: u64,
        at_ms: u64,
    },
    /// Retained bytes `[offset, offset + length)` of a stream file.
    Output {
        call: String,
        stream: StreamName,
        offset: u64,
        length: u64,
        digest: String,
        redactions: u64,
        /// Source bytes accounted for through this event.
        input_end: u64,
    },
    BudgetExhausted {
        limit: u64,
        used: u64,
    },
    Dropped {
        call: String,
        stream: StreamName,
        bytes: u64,
    },
    StreamClosed {
        call: String,
        stream: StreamName,
        state: StreamState,
        length: u64,
        digest: String,
        redactions: u64,
    },
    Result {
        call: String,
        result: CallResult,
        at_ms: u64,
    },
    CaptureError {
        call: String,
        note: String,
    },
    ChildReserved {
        call: String,
        evidence_id: String,
        budget: u64,
    },
    ChildArchived {
        child: ChildLink,
    },
    Finalized {
        status: EvidenceStatus,
        manifest_digest: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub seq: u64,
    pub event: Event,
}

/// Source bytes of one stream durably accounted for (retained, redacted,
/// or recorded as dropped). Bytes still held back for split credentials
/// are not acknowledged until a later chunk or the close decides them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ack {
    pub input_end: u64,
}

struct Live {
    summary: CallSummary,
    held: [Vec<u8>; 2],
    hashers: [Sha256; 2],
    spools: [Option<File>; 2],
    closed: [bool; 2],
}

struct Reserved {
    call: String,
    run: RunLink,
    budget: u64,
}

pub struct Recorder {
    dir: PathBuf,
    id: String,
    run: Option<RunLink>,
    redactor: Redactor,
    limit: u64,
    used: u64,
    exhausted: bool,
    seq: u64,
    log: File,
    calls: BTreeMap<String, Live>,
    order: Vec<String>,
    children: Vec<ChildLink>,
    reserved: BTreeMap<String, Reserved>,
    sealed: Option<Sealed>,
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recorder")
            .field("id", &self.id)
            .field("seq", &self.seq)
            .finish_non_exhaustive()
    }
}

fn io(m: &'static str) -> impl Fn(std::io::Error) -> EvidenceError {
    move |_| EvidenceError::Io(m)
}

fn private_dir(path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            EvidenceError::Invalid("The evidence directory already exists.")
        } else {
            EvidenceError::Io("The evidence directory could not be created.")
        }
    })
}

fn private_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.create_new(true).append(true).read(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options
        .open(path)
        .map_err(io("An evidence file could not be created."))
}

fn note(text: &str) -> String {
    let mut end = text.len().min(MAX_NOTE_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

fn empty_stream() -> StreamSummary {
    StreamSummary {
        state: StreamState::Open,
        length: 0,
        digest: digest(b""),
        redactions: 0,
        input_bytes: 0,
        dropped_bytes: 0,
        first_seq: None,
        last_seq: None,
    }
}

impl Recorder {
    /// Start a new evidence record in `dir`, which must not exist yet.
    /// `budget` bounds every retained byte (arguments, output, children).
    pub fn create(
        dir: impl Into<PathBuf>,
        id: &str,
        run: Option<RunLink>,
        redactor: Redactor,
        budget: u64,
    ) -> Result<Self> {
        if !valid_id(id) {
            return Err(EvidenceError::Invalid("The evidence ID is invalid."));
        }
        let dir = dir.into();
        if let Some(parent) = dir.parent() {
            fs::create_dir_all(parent).map_err(io("The evidence root could not be created."))?;
        }
        private_dir(&dir)?;
        private_dir(&dir.join("streams"))?;
        let log = private_file(&dir.join("events.jsonl"))?;
        Ok(Self {
            dir,
            id: id.into(),
            run,
            redactor,
            limit: budget,
            used: 0,
            exhausted: false,
            seq: 0,
            log,
            calls: BTreeMap::new(),
            order: Vec::new(),
            children: Vec::new(),
            reserved: BTreeMap::new(),
            sealed: None,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn last_seq(&self) -> u64 {
        self.seq
    }
    pub fn call(&self, id: &str) -> Option<&CallSummary> {
        self.calls.get(id).map(|l| &l.summary)
    }

    fn live(&mut self) -> Result<()> {
        if self.sealed.is_some() {
            return Err(EvidenceError::Finalized);
        }
        Ok(())
    }

    fn append(&mut self, event: Event) -> Result<u64> {
        let seq = self.seq + 1;
        let mut line = serde_json::to_vec(&Entry { seq, event }).expect("event encodes");
        line.push(b'\n');
        self.log
            .write_all(&line)
            .and_then(|()| self.log.sync_data())
            .map_err(io("The evidence log could not be written."))?;
        self.seq = seq;
        Ok(seq)
    }

    fn mark_exhausted(&mut self) -> Result<()> {
        if !self.exhausted {
            self.exhausted = true;
            self.append(Event::BudgetExhausted {
                limit: self.limit,
                used: self.used,
            })?;
        }
        Ok(())
    }

    /// Record a call's identity and its arguments (scrubbed first).
    pub fn start_call(&mut self, call: CallIdentity, arguments: &Value, now_ms: u64) -> Result<()> {
        self.live()?;
        if !valid_id(&call.id)
            || !valid_id(&call.run.cloud_job)
            || call.tool.is_empty()
            || call.tool.len() > 128
            || call.parent.as_deref().is_some_and(|p| !valid_id(p))
        {
            return Err(EvidenceError::Invalid("The call identity is invalid."));
        }
        if self.calls.contains_key(&call.id) {
            return Err(EvidenceError::DuplicateCall(call.id));
        }
        if self.calls.len() >= MAX_CALLS {
            return Err(EvidenceError::Invalid("The evidence holds too many calls."));
        }
        let mut retained = arguments.clone();
        let argument_redactions = self.redactor.redact_json(&mut retained);
        let bytes = serde_json::to_vec(&retained).expect("arguments encode");
        let arguments_digest = digest(&bytes);
        let arguments_bytes = bytes.len() as u64;
        let fits = !self.exhausted && arguments_bytes <= self.limit - self.used;
        let retained = if fits {
            self.used += arguments_bytes;
            Some(retained)
        } else {
            self.mark_exhausted()?;
            None
        };
        let mut spools = [None, None];
        for name in [StreamName::Stdout, StreamName::Stderr] {
            spools[name.index()] = Some(private_file(&self.dir.join("streams").join(format!(
                "{}.{}",
                call.id,
                name.word()
            )))?);
        }
        self.append(Event::CallStarted {
            call: call.clone(),
            arguments: retained.clone(),
            arguments_digest: arguments_digest.clone(),
            arguments_bytes,
            argument_redactions,
            at_ms: now_ms,
        })?;
        let id = call.id.clone();
        self.order.push(id.clone());
        self.calls.insert(
            id,
            Live {
                summary: CallSummary {
                    identity: call,
                    arguments: retained,
                    arguments_digest,
                    arguments_bytes,
                    argument_redactions,
                    stdout: empty_stream(),
                    stderr: empty_stream(),
                    results: Vec::new(),
                    outcome: CallOutcome::Running,
                    capture_errors: Vec::new(),
                    started_ms: now_ms,
                    ended_ms: None,
                },
                held: [Vec::new(), Vec::new()],
                hashers: [Sha256::new(), Sha256::new()],
                spools,
                closed: [false, false],
            },
        );
        Ok(())
    }

    /// Spool the next source bytes of one stream.
    pub fn output(&mut self, call: &str, stream: StreamName, input: &[u8]) -> Result<Ack> {
        self.write(call, stream, input, false)
    }

    /// Declare that a stream ended at its source; flushes held bytes.
    pub fn close_stream(&mut self, call: &str, stream: StreamName) -> Result<Ack> {
        self.close(call, stream, StreamState::Complete)
    }

    fn close(&mut self, call: &str, stream: StreamName, state: StreamState) -> Result<Ack> {
        self.live()?;
        let live = self
            .calls
            .get(call)
            .ok_or_else(|| EvidenceError::UnknownCall(call.into()))?;
        if live.closed[stream.index()] {
            return Ok(Ack {
                input_end: live.summary.stream(stream).input_bytes,
            });
        }
        let ack = self.write(call, stream, &[], true)?;
        let live = self.calls.get_mut(call).expect("call exists");
        let hasher = std::mem::take(&mut live.hashers[stream.index()]);
        live.spools[stream.index()] = None;
        live.closed[stream.index()] = true;
        let s = live.summary.stream_mut(stream);
        s.digest = format!("{:x}", hasher.finalize());
        if s.state == StreamState::Open {
            s.state = state;
        }
        let event = Event::StreamClosed {
            call: call.into(),
            stream,
            state: s.state,
            length: s.length,
            digest: s.digest.clone(),
            redactions: s.redactions,
        };
        let seq = self.append(event)?;
        let s = self
            .calls
            .get_mut(call)
            .expect("call exists")
            .summary
            .stream_mut(stream);
        s.first_seq.get_or_insert(seq);
        s.last_seq = Some(seq);
        Ok(ack)
    }

    fn write(&mut self, call: &str, stream: StreamName, input: &[u8], last: bool) -> Result<Ack> {
        self.live()?;
        let live = self
            .calls
            .get_mut(call)
            .ok_or_else(|| EvidenceError::UnknownCall(call.into()))?;
        let i = stream.index();
        if live.closed[i] {
            let note = format!("{} output arrived after the stream closed", stream.word());
            live.summary.capture_errors.push(note.clone());
            self.append(Event::CaptureError {
                call: call.into(),
                note,
            })?;
            return Err(EvidenceError::Closed(call.into()));
        }
        live.summary.stream_mut(stream).input_bytes += input.len() as u64;
        let held_before = live.held[i].len() as u64;
        let scrubbed = self.redactor.scrub(&mut live.held[i], input, last);
        let held_after = live.held[i].len() as u64;
        let consumed = held_before + input.len() as u64 - held_after;
        if consumed == 0 {
            return Ok(Ack {
                input_end: live.summary.stream(stream).input_bytes - held_after,
            });
        }
        let room = if self.exhausted {
            0
        } else {
            self.limit - self.used
        };
        let keep = (scrubbed.bytes.len() as u64).min(room) as usize;
        let mut events = Vec::new();
        let s = live.summary.stream_mut(stream);
        let input_end = s.input_bytes - held_after;
        if keep > 0 {
            let bytes = &scrubbed.bytes[..keep];
            let spool = live.spools[i].as_mut().expect("open stream has a spool");
            spool
                .write_all(bytes)
                .and_then(|()| spool.sync_data())
                .map_err(io("An evidence stream could not be written."))?;
            live.hashers[i].update(bytes);
            events.push(Event::Output {
                call: call.into(),
                stream,
                offset: s.length,
                length: keep as u64,
                digest: digest(bytes),
                redactions: scrubbed.redactions,
                input_end,
            });
            s.length += keep as u64;
            s.redactions += scrubbed.redactions;
            self.used += keep as u64;
        }
        let truncated = keep < scrubbed.bytes.len();
        if truncated {
            let dropped = (scrubbed.bytes.len() - keep) as u64;
            s.state = StreamState::Truncated;
            s.dropped_bytes += dropped;
            events.push(Event::Dropped {
                call: call.into(),
                stream,
                bytes: dropped,
            });
        }
        if truncated {
            self.mark_exhausted()?;
        }
        for event in events {
            let seq = self.append(event)?;
            let s = self
                .calls
                .get_mut(call)
                .expect("call exists")
                .summary
                .stream_mut(stream);
            s.first_seq.get_or_insert(seq);
            s.last_seq = Some(seq);
        }
        Ok(Ack { input_end })
    }

    /// Record one direct result observation. Earlier failures stay failures.
    pub fn result(&mut self, call: &str, result: CallResult, now_ms: u64) -> Result<CallOutcome> {
        self.live()?;
        let result = match result {
            CallResult::EngineError { message } => CallResult::EngineError {
                message: note(&self.redactor.redact_text(&message).0),
            },
            CallResult::Unresolved { reason } => CallResult::Unresolved {
                reason: note(&reason),
            },
            CallResult::Signalled { signal } => CallResult::Signalled {
                signal: note(&signal),
            },
            other => other,
        };
        let live = self
            .calls
            .get_mut(call)
            .ok_or_else(|| EvidenceError::UnknownCall(call.into()))?;
        live.summary.results.push(result.clone());
        live.summary.outcome = CallOutcome::fold(&live.summary.results);
        live.summary.ended_ms = Some(now_ms);
        let outcome = live.summary.outcome;
        self.append(Event::Result {
            call: call.into(),
            result,
            at_ms: now_ms,
        })?;
        Ok(outcome)
    }

    /// Reserve part of the budget for a nested child run and open its
    /// evidence under `children/<evidence_id>`. Archive it with
    /// [`Recorder::archive_child`]; an unarchived child leaves this record
    /// incomplete.
    pub fn child(
        &mut self,
        call: &str,
        evidence_id: &str,
        run: RunLink,
        budget: u64,
    ) -> Result<Recorder> {
        self.live()?;
        if !self.calls.contains_key(call) {
            return Err(EvidenceError::UnknownCall(call.into()));
        }
        if self.reserved.contains_key(evidence_id)
            || self.children.iter().any(|c| c.evidence_id == evidence_id)
        {
            return Err(EvidenceError::Invalid(
                "That child evidence already exists.",
            ));
        }
        let room = if self.exhausted {
            0
        } else {
            self.limit - self.used
        };
        let granted = budget.min(room);
        let children = self.dir.join("children");
        if !children.exists() {
            private_dir(&children)?;
        }
        let recorder = Recorder::create(
            children.join(evidence_id),
            evidence_id,
            Some(run.clone()),
            self.redactor.clone(),
            granted,
        )?;
        self.used += granted;
        self.reserved.insert(
            evidence_id.into(),
            Reserved {
                call: call.into(),
                run,
                budget: granted,
            },
        );
        self.append(Event::ChildReserved {
            call: call.into(),
            evidence_id: evidence_id.into(),
            budget: granted,
        })?;
        Ok(recorder)
    }

    /// Finalize a child run and link it, by manifest digest, to its call.
    pub fn archive_child(&mut self, mut child: Recorder, now_ms: u64) -> Result<ChildLink> {
        self.live()?;
        let reserved = self
            .reserved
            .remove(&child.id)
            .ok_or(EvidenceError::Invalid("That child was not reserved here."))?;
        let sealed = child.finish(now_ms)?;
        self.used = self.used - reserved.budget + child.used.min(reserved.budget);
        let link = ChildLink {
            evidence_id: sealed.id,
            call: reserved.call,
            run: reserved.run,
            manifest_digest: sealed.digest,
            status: sealed.status,
        };
        self.append(Event::ChildArchived {
            child: link.clone(),
        })?;
        self.children.push(link.clone());
        Ok(link)
    }

    /// Seal the record. Open streams close as gaps, calls without a result
    /// get an explicit unresolved result, and the manifest is written once.
    pub fn finish(&mut self, now_ms: u64) -> Result<Sealed> {
        if let Some(sealed) = &self.sealed {
            return Ok(sealed.clone());
        }
        for id in self.order.clone() {
            for stream in [StreamName::Stdout, StreamName::Stderr] {
                self.close(&id, stream, StreamState::Gap)?;
            }
            if self.calls[&id].summary.results.is_empty() {
                self.result(
                    &id,
                    CallResult::Unresolved {
                        reason: "The call had no result when the evidence was sealed.".into(),
                    },
                    now_ms,
                )?;
            }
        }
        let calls: Vec<CallSummary> = self
            .order
            .iter()
            .map(|id| self.calls[id].summary.clone())
            .collect();
        let mut reasons = Vec::new();
        if self.exhausted {
            reasons.push("The evidence budget was exhausted.".to_owned());
        }
        for c in calls.iter().filter(|c| !c.complete()) {
            reasons.push(format!(
                "Call {} is not completely retained.",
                c.identity.id
            ));
        }
        for id in self.reserved.keys() {
            reasons.push(format!("Child run {id} was not archived."));
        }
        for c in self.children.iter().filter(|c| !c.status.complete()) {
            reasons.push(format!("Child run {} is incomplete.", c.evidence_id));
        }
        let redacted = calls.iter().any(CallSummary::redacted)
            || self
                .children
                .iter()
                .any(|c| c.status == EvidenceStatus::CompleteWithRedactions);
        let status = if !reasons.is_empty() {
            EvidenceStatus::Incomplete
        } else if redacted {
            EvidenceStatus::CompleteWithRedactions
        } else {
            EvidenceStatus::Complete
        };
        let manifest = Manifest {
            schema: EVIDENCE_SCHEMA.into(),
            id: self.id.clone(),
            run: self.run.clone(),
            status,
            reasons,
            budget: BudgetUse {
                limit: self.limit,
                used: self.used,
                exhausted: self.exhausted,
            },
            calls,
            children: self.children.clone(),
            last_seq: self.seq,
            finalized_ms: now_ms,
        };
        let bytes = serde_json::to_vec_pretty(&manifest).expect("manifest encodes");
        let manifest_digest = digest(&bytes);
        let tmp = self.dir.join("manifest.json.tmp");
        let mut file = private_file(&tmp)?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(io("The evidence manifest could not be written."))?;
        fs::rename(&tmp, self.dir.join("manifest.json"))
            .map_err(io("The evidence manifest could not be written."))?;
        self.append(Event::Finalized {
            status,
            manifest_digest: manifest_digest.clone(),
        })?;
        let sealed = Sealed {
            id: self.id.clone(),
            digest: manifest_digest,
            status,
        };
        self.sealed = Some(sealed.clone());
        Ok(sealed)
    }

    /// Feed one Boat stream or follower frame for `call`. `Exit` closes
    /// both streams and records the exit; `Error` closes them as gaps and
    /// records an engine failure that a later exit cannot turn into success.
    pub fn observe_boat(
        &mut self,
        call: &str,
        frame: &boat::CommandFrame,
        now_ms: u64,
    ) -> Result<()> {
        use boat::CommandFrame as F;
        match frame {
            F::Started => Ok(()),
            F::Stdout(text) => self
                .output(call, StreamName::Stdout, text.as_bytes())
                .map(drop),
            F::Stderr(text) => self
                .output(call, StreamName::Stderr, text.as_bytes())
                .map(drop),
            F::Exit {
                exit_code,
                success,
                timed_out,
            } => {
                for stream in [StreamName::Stdout, StreamName::Stderr] {
                    self.close_stream(call, stream)?;
                }
                let result = if *timed_out {
                    CallResult::TimedOut { code: *exit_code }
                } else {
                    CallResult::Exited {
                        code: *exit_code,
                        success: *success,
                    }
                };
                self.result(call, result, now_ms).map(drop)
            }
            F::Error { error, message, .. } => {
                for stream in [StreamName::Stdout, StreamName::Stderr] {
                    self.close(call, stream, StreamState::Gap)?;
                }
                let message = [error.as_deref(), message.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(": ");
                let message = if message.is_empty() {
                    "The command stream failed.".to_owned()
                } else {
                    message
                };
                self.result(call, CallResult::EngineError { message }, now_ms)
                    .map(drop)
            }
            F::Unknown(_) => {
                let note = "An unknown command frame could not be captured.".to_owned();
                self.calls
                    .get_mut(call)
                    .ok_or_else(|| EvidenceError::UnknownCall(call.into()))?
                    .summary
                    .capture_errors
                    .push(note.clone());
                self.append(Event::CaptureError {
                    call: call.into(),
                    note,
                })
                .map(drop)
            }
        }
    }

    /// An ATIF call for `call` whose output is a model-visible excerpt of
    /// the retained (already redacted) stdout and stderr, at most
    /// `excerpt_bytes` of each; `extra.evidence` links the full archive.
    pub fn atif_call(&self, call: &str, excerpt_bytes: u64) -> Result<atif::Call> {
        let summary = self
            .call(call)
            .ok_or_else(|| EvidenceError::UnknownCall(call.into()))?;
        let mut output = String::new();
        for stream in [StreamName::Stdout, StreamName::Stderr] {
            let s = summary.stream(stream);
            let mut bytes = Vec::new();
            File::open(
                self.dir
                    .join("streams")
                    .join(format!("{call}.{}", stream.word())),
            )
            .and_then(|f| f.take(s.length.min(excerpt_bytes)).read_to_end(&mut bytes))
            .map_err(io("An evidence stream could not be read."))?;
            output.push_str(&String::from_utf8_lossy(&bytes));
        }
        let outcome = match summary.outcome {
            CallOutcome::Succeeded => atif::Outcome::Completed,
            _ => atif::Outcome::Failed,
        };
        let mut extra = Map::new();
        extra.insert(
            "evidence".into(),
            serde_json::json!({
                "schema": EVIDENCE_SCHEMA,
                "evidence_id": self.id,
                "call": summary.identity.id,
                "outcome": summary.outcome,
                "stdout": {"length": summary.stdout.length, "digest": summary.stdout.digest, "state": summary.stdout.state},
                "stderr": {"length": summary.stderr.length, "digest": summary.stderr.digest, "state": summary.stderr.state},
                "excerpt_bytes": excerpt_bytes,
            }),
        );
        Ok(atif::Call {
            id: summary.identity.id.clone(),
            name: summary.identity.tool.clone(),
            arguments: summary.arguments.clone().unwrap_or(Value::Null),
            output,
            outcome,
            milliseconds: summary
                .ended_ms
                .map_or(0, |e| e.saturating_sub(summary.started_ms)),
            purpose: None,
            extra,
        })
    }
}

/// Read a sealed manifest and check it against the digest a verification
/// cites.
pub fn load_manifest(dir: &Path, expected_digest: &str) -> Result<Manifest> {
    if !valid_digest(expected_digest) {
        return Err(EvidenceError::Invalid("The evidence digest is invalid."));
    }
    let bytes = fs::read(dir.join("manifest.json")).map_err(io("No sealed evidence manifest."))?;
    if digest(&bytes) != expected_digest {
        return Err(EvidenceError::Corrupt(
            "The evidence manifest does not match its digest.",
        ));
    }
    serde_json::from_slice(&bytes).map_err(|_| EvidenceError::Corrupt("The manifest is invalid."))
}

/// Read every event of an evidence log in sequence order, checking that
/// sequence IDs are contiguous from 1.
pub fn read_events(dir: &Path) -> Result<Vec<Entry>> {
    let text = fs::read_to_string(dir.join("events.jsonl"))
        .map_err(io("The evidence log could not be read."))?;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let entry: Entry = serde_json::from_str(line)
            .map_err(|_| EvidenceError::Corrupt("An evidence event is invalid."))?;
        if entry.seq != i as u64 + 1 {
            return Err(EvidenceError::Corrupt("The evidence sequence has a gap."));
        }
        out.push(entry);
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
