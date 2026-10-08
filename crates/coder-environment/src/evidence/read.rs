//! Reading retained evidence (ENV-02b): original-byte paging, resumable
//! replay, gap disclosure, and a portable export.
//!
//! An [`EvidenceReader`] reads one evidence directory written by a
//! [`super::Recorder`] (recording or sealed) and never changes it: it opens
//! files read-only and creates no directory, lock, or cache.
//!
//! - Event pages ([`EvidenceReader::events`]) are ordered by the log's
//!   contiguous `seq`, never by timestamp, so records sharing a timestamp are
//!   never skipped. An [`EventCursor`] is a boundary after one sequence ID,
//!   bound to the evidence ID and the digest of the exact record at that
//!   position; a cursor from another source or a rewritten log is refused.
//! - Both latest and history windows are bounded: [`EventWindow::Latest`]
//!   always shows the newest records, [`EventWindow::Before`] walks older
//!   history, and [`EventWindow::After`] resumes after a dropped connection
//!   without duplicates. The same cursor always yields the same records.
//! - Stream pages ([`EvidenceReader::stream`]) serve the retained bytes the
//!   log's `Output` events account for, by byte offset. Every chunk is
//!   checked against its recorded length and SHA-256 before any byte of it
//!   is served; a mismatch is disclosed as a [`Gap::Corrupt`] range and its
//!   bytes are withheld.
//! - Every page and the [`EvidenceSummary`] carry the [`Gap`]s that apply:
//!   still recording, budget drops, gapped streams, dropped arguments,
//!   unresolved calls, capture errors, missing or incomplete children,
//!   damaged log or manifest, and corrupt bytes. `complete` is true only for
//!   a sealed complete record with no gap; a `Finalized` event alone never
//!   implies complete capture.
//! - [`EvidenceReader::export`] writes a directory bundle with every
//!   retained file, its length and digest, and the gaps, in `export.json`.

use super::{
    CallOutcome, CallResult, ChildLink, Entry, Event, EvidenceError, EvidenceStatus, Result,
    Sealed, StreamName, StreamState, private_dir,
};
use crate::{digest, valid_id};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub const EXPORT_SCHEMA: &str = "openagents.environment.evidence_export.v1";
/// Page bounds. A page bound never caps what is retained or exported.
pub const MAX_PAGE_EVENTS: usize = 1000;
pub const MAX_PAGE_BYTES: u64 = 4 << 20;

/// A position after event `seq` (0 is before the first event), bound to the
/// evidence ID and the digest of the record at `seq`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventCursor {
    pub evidence_id: String,
    pub seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
}

/// A byte offset in one retained stream, bound to the evidence ID and the
/// `Output` event (sequence and chunk digest) whose bytes reach `offset`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamCursor {
    pub evidence_id: String,
    pub call: String,
    pub stream: StreamName,
    pub offset: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_digest: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventWindow {
    Oldest,
    /// The newest `limit` events.
    Latest,
    /// Events after the cursor: forward paging and replay after a drop.
    After(EventCursor),
    /// Events at or before the cursor's position: older history.
    Before(EventCursor),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamWindow {
    Oldest,
    /// The newest `max_bytes` retained bytes.
    Latest,
    After(StreamCursor),
    Before(StreamCursor),
}

/// One disclosed missing or unproven range of evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Gap {
    /// The record is not sealed; more evidence may still arrive.
    Recording,
    /// The log could not be read past `after_seq`.
    Log {
        after_seq: u64,
        reason: String,
    },
    /// The record was finalized but its manifest is missing or damaged.
    Manifest {
        reason: String,
    },
    BudgetExhausted {
        seq: u64,
        limit: u64,
        used: u64,
    },
    ArgumentsDropped {
        seq: u64,
        call: String,
    },
    /// Scrubbed bytes the budget could not retain.
    Dropped {
        seq: u64,
        call: String,
        stream: StreamName,
        bytes: u64,
    },
    /// The stream ended without a definite end.
    StreamGap {
        seq: u64,
        call: String,
        stream: StreamName,
    },
    Unresolved {
        seq: u64,
        call: String,
    },
    CaptureError {
        seq: u64,
        call: String,
        note: String,
    },
    ChildUnarchived {
        seq: u64,
        call: String,
        evidence_id: String,
    },
    ChildMissing {
        seq: u64,
        evidence_id: String,
        reason: String,
    },
    ChildIncomplete {
        seq: u64,
        evidence_id: String,
        status: EvidenceStatus,
    },
    /// Retained bytes `[start, end)` failed their recorded length or digest
    /// and are withheld.
    Corrupt {
        seq: u64,
        call: String,
        stream: StreamName,
        start: u64,
        end: u64,
    },
}
impl Gap {
    /// The event a gap is anchored at; `None` for record-wide gaps.
    pub fn seq(&self) -> Option<u64> {
        match self {
            Self::Recording | Self::Log { .. } | Self::Manifest { .. } => None,
            Self::BudgetExhausted { seq, .. }
            | Self::ArgumentsDropped { seq, .. }
            | Self::Dropped { seq, .. }
            | Self::StreamGap { seq, .. }
            | Self::Unresolved { seq, .. }
            | Self::CaptureError { seq, .. }
            | Self::ChildUnarchived { seq, .. }
            | Self::ChildMissing { seq, .. }
            | Self::ChildIncomplete { seq, .. }
            | Self::Corrupt { seq, .. } => Some(*seq),
        }
    }
    /// Whether the gap concerns this stream (or the whole record or call).
    fn touches(&self, call: &str, stream: StreamName) -> bool {
        match self {
            Self::Recording | Self::Log { .. } | Self::Manifest { .. } => true,
            Self::BudgetExhausted { .. } => false,
            Self::ArgumentsDropped { call: c, .. }
            | Self::Unresolved { call: c, .. }
            | Self::CaptureError { call: c, .. } => c == call,
            Self::Dropped {
                call: c, stream: s, ..
            }
            | Self::StreamGap {
                call: c, stream: s, ..
            }
            | Self::Corrupt {
                call: c, stream: s, ..
            } => c == call && *s == stream,
            Self::ChildUnarchived { call: c, .. } => c == call,
            Self::ChildMissing { .. } | Self::ChildIncomplete { .. } => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventPage {
    pub evidence_id: String,
    pub entries: Vec<Entry>,
    /// Continue into older history; `None` when the first event is shown.
    pub older: Option<EventCursor>,
    /// Resume here after a drop: the boundary after the last event shown.
    pub newer: EventCursor,
    /// The newest sequence ID currently readable.
    pub head: u64,
    pub more_newer: bool,
    pub status: EvidenceStatus,
    pub complete: bool,
    /// Record-wide gaps and gaps anchored at events in this page.
    pub gaps: Vec<Gap>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamPage {
    pub evidence_id: String,
    pub call: String,
    pub stream: StreamName,
    /// Offset of `bytes[0]` in the retained stream.
    pub start: u64,
    /// Verified original retained bytes.
    pub bytes: Vec<u8>,
    pub digest: String,
    /// Retained bytes currently accounted for by the log.
    pub length: u64,
    /// `None` while the stream is open.
    pub state: Option<StreamState>,
    pub older: Option<StreamCursor>,
    pub newer: StreamCursor,
    pub more_newer: bool,
    pub status: EvidenceStatus,
    pub complete: bool,
    /// Record-wide gaps, this call's and stream's gaps, and corrupt ranges
    /// found while serving this page.
    pub gaps: Vec<Gap>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamCoverage {
    pub length: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<StreamState>,
    pub dropped_bytes: u64,
    pub chunks: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallCoverage {
    pub call: String,
    pub tool: String,
    pub outcome: CallOutcome,
    pub arguments_retained: bool,
    pub stdout: StreamCoverage,
    pub stderr: StreamCoverage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSummary {
    pub evidence_id: String,
    pub head: EventCursor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed: Option<Sealed>,
    pub status: EvidenceStatus,
    pub complete: bool,
    pub calls: Vec<CallCoverage>,
    pub children: Vec<ChildLink>,
    pub gaps: Vec<Gap>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportFile {
    pub path: String,
    pub length: u64,
    pub digest: String,
}

/// `export.json` of a portable bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportManifest {
    pub schema: String,
    pub evidence_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed: Option<Sealed>,
    pub head_seq: u64,
    pub status: EvidenceStatus,
    /// True only for a sealed complete record whose every byte verified and
    /// whose every child exported complete.
    pub complete: bool,
    /// Every gap; corrupt ranges are zero-filled in the bundle's stream
    /// files and listed here, never presented as retained bytes.
    pub gaps: Vec<Gap>,
    pub files: Vec<ExportFile>,
}

#[derive(Clone, Debug)]
struct Chunk {
    seq: u64,
    offset: u64,
    length: u64,
    digest: String,
}
impl Chunk {
    fn end(&self) -> u64 {
        self.offset + self.length
    }
}

#[derive(Clone, Debug, Default)]
struct StreamModel {
    chunks: Vec<Chunk>,
    length: u64,
    state: Option<StreamState>,
    dropped: u64,
}
impl StreamModel {
    /// The chunk whose bytes reach `offset` (`offset` in `(start, end]`).
    fn reaching(&self, offset: u64) -> Option<&Chunk> {
        if offset == 0 {
            return None;
        }
        let i = self.chunks.partition_point(|c| c.end() < offset);
        self.chunks.get(i)
    }
}

#[derive(Clone, Debug)]
struct CallModel {
    tool: String,
    started_seq: u64,
    arguments_retained: bool,
    results: Vec<CallResult>,
    last_result_seq: Option<u64>,
    streams: [StreamModel; 2],
}

pub struct EvidenceReader {
    dir: PathBuf,
    id: String,
    /// The verified log prefix, byte for byte.
    log: Vec<u8>,
    entries: Vec<Entry>,
    records: Vec<String>,
    calls: BTreeMap<String, CallModel>,
    order: Vec<String>,
    reserved: BTreeMap<String, (u64, String)>,
    children: Vec<(u64, ChildLink)>,
    finalized: Option<(u64, String)>,
    sealed: Option<Sealed>,
    manifest: Option<Vec<u8>>,
    gaps: Vec<Gap>,
}

impl std::fmt::Debug for EvidenceReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvidenceReader")
            .field("id", &self.id)
            .field("head", &self.head())
            .finish_non_exhaustive()
    }
}

fn cursor_err(m: &'static str) -> EvidenceError {
    EvidenceError::Cursor(m)
}

impl EvidenceReader {
    /// Read the evidence `id` in `dir` as it is now. A damaged log is read
    /// up to the damage and disclosed as [`Gap::Log`].
    pub fn open(dir: impl Into<PathBuf>, id: &str) -> Result<Self> {
        if !valid_id(id) {
            return Err(EvidenceError::Invalid("The evidence ID is invalid."));
        }
        let dir = dir.into();
        let bytes = fs::read(dir.join("events.jsonl"))
            .map_err(|_| EvidenceError::Io("The evidence log could not be read."))?;
        let mut r = Self {
            dir,
            id: id.into(),
            log: Vec::new(),
            entries: Vec::new(),
            records: Vec::new(),
            calls: BTreeMap::new(),
            order: Vec::new(),
            reserved: BTreeMap::new(),
            children: Vec::new(),
            finalized: None,
            sealed: None,
            manifest: None,
            gaps: Vec::new(),
        };
        let mut pos = 0;
        let mut damage = None;
        while pos < bytes.len() {
            let Some(nl) = bytes[pos..].iter().position(|&b| b == b'\n') else {
                damage = Some("The last evidence event was not completely written.");
                break;
            };
            let line = &bytes[pos..pos + nl];
            let expected = r.entries.len() as u64 + 1;
            let entry = match serde_json::from_slice::<Entry>(line) {
                Ok(entry) if entry.seq == expected => entry,
                Ok(_) => {
                    damage = Some("The evidence sequence is not contiguous.");
                    break;
                }
                Err(_) => {
                    damage = Some("An evidence event is invalid.");
                    break;
                }
            };
            if let Err(reason) = r.apply(&entry) {
                damage = Some(reason);
                break;
            }
            r.records.push(digest(line));
            r.entries.push(entry);
            pos += nl + 1;
        }
        r.log = bytes[..pos].to_vec();
        if let Some(reason) = damage {
            r.gaps.push(Gap::Log {
                after_seq: r.head(),
                reason: reason.into(),
            });
        }
        r.settle();
        Ok(r)
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn head(&self) -> u64 {
        self.entries.len() as u64
    }

    fn apply(&mut self, entry: &Entry) -> std::result::Result<(), &'static str> {
        let seq = entry.seq;
        if self.finalized.is_some() {
            return Err("Events follow the sealed manifest.");
        }
        fn call_mut<'a>(
            calls: &'a mut BTreeMap<String, CallModel>,
            id: &str,
        ) -> std::result::Result<&'a mut CallModel, &'static str> {
            calls
                .get_mut(id)
                .ok_or("An event names a call that never started.")
        }
        match &entry.event {
            Event::CallStarted {
                call, arguments, ..
            } => {
                if self.calls.contains_key(&call.id) {
                    return Err("A call started twice.");
                }
                if arguments.is_none() {
                    self.gaps.push(Gap::ArgumentsDropped {
                        seq,
                        call: call.id.clone(),
                    });
                }
                self.order.push(call.id.clone());
                self.calls.insert(
                    call.id.clone(),
                    CallModel {
                        tool: call.tool.clone(),
                        started_seq: seq,
                        arguments_retained: arguments.is_some(),
                        results: Vec::new(),
                        last_result_seq: None,
                        streams: Default::default(),
                    },
                );
            }
            Event::Output {
                call,
                stream,
                offset,
                length,
                digest,
                ..
            } => {
                let s = &mut call_mut(&mut self.calls, call)?.streams[stream.index()];
                if s.state.is_some() {
                    return Err("Output follows a closed stream.");
                }
                if *offset != s.length || *length == 0 {
                    return Err("Stream offsets are not contiguous.");
                }
                s.chunks.push(Chunk {
                    seq,
                    offset: *offset,
                    length: *length,
                    digest: digest.clone(),
                });
                s.length += length;
            }
            Event::BudgetExhausted { limit, used } => self.gaps.push(Gap::BudgetExhausted {
                seq,
                limit: *limit,
                used: *used,
            }),
            Event::Dropped {
                call,
                stream,
                bytes,
            } => {
                call_mut(&mut self.calls, call)?.streams[stream.index()].dropped += bytes;
                self.gaps.push(Gap::Dropped {
                    seq,
                    call: call.clone(),
                    stream: *stream,
                    bytes: *bytes,
                });
            }
            Event::StreamClosed {
                call,
                stream,
                state,
                length,
                ..
            } => {
                let s = &mut call_mut(&mut self.calls, call)?.streams[stream.index()];
                if s.state.is_some() {
                    return Err("A stream closed twice.");
                }
                if *length != s.length {
                    return Err("A stream closed at a length its output does not account for.");
                }
                s.state = Some(*state);
                if *state == StreamState::Gap {
                    self.gaps.push(Gap::StreamGap {
                        seq,
                        call: call.clone(),
                        stream: *stream,
                    });
                }
            }
            Event::Result { call, result, .. } => {
                let c = call_mut(&mut self.calls, call)?;
                c.results.push(result.clone());
                c.last_result_seq = Some(seq);
            }
            Event::CaptureError { call, note } => {
                call_mut(&mut self.calls, call)?;
                self.gaps.push(Gap::CaptureError {
                    seq,
                    call: call.clone(),
                    note: note.clone(),
                });
            }
            Event::ChildReserved {
                call, evidence_id, ..
            } => {
                call_mut(&mut self.calls, call)?;
                self.reserved
                    .insert(evidence_id.clone(), (seq, call.clone()));
            }
            Event::ChildArchived { child } => {
                if self.reserved.remove(&child.evidence_id).is_none() {
                    return Err("A child was archived without a reservation.");
                }
                self.children.push((seq, child.clone()));
            }
            Event::Finalized {
                manifest_digest, ..
            } => self.finalized = Some((seq, manifest_digest.clone())),
        }
        Ok(())
    }

    /// Derive gaps that need the whole log: unresolved calls, children, and
    /// the sealed manifest.
    fn settle(&mut self) {
        for id in &self.order {
            let c = &self.calls[id];
            if CallOutcome::fold(&c.results) == CallOutcome::Unresolved {
                self.gaps.push(Gap::Unresolved {
                    seq: c.last_result_seq.unwrap_or(c.started_seq),
                    call: id.clone(),
                });
            }
        }
        for (evidence_id, (seq, call)) in &self.reserved {
            self.gaps.push(Gap::ChildUnarchived {
                seq: *seq,
                call: call.clone(),
                evidence_id: evidence_id.clone(),
            });
        }
        for (seq, link) in &self.children {
            let path = self
                .dir
                .join("children")
                .join(&link.evidence_id)
                .join("manifest.json");
            let reason = match fs::read(&path) {
                Err(_) => Some("The child evidence is missing."),
                Ok(bytes) if digest(&bytes) != link.manifest_digest => {
                    Some("The child manifest does not match its link.")
                }
                Ok(_) => None,
            };
            if let Some(reason) = reason {
                self.gaps.push(Gap::ChildMissing {
                    seq: *seq,
                    evidence_id: link.evidence_id.clone(),
                    reason: reason.into(),
                });
            } else if !link.status.complete() {
                self.gaps.push(Gap::ChildIncomplete {
                    seq: *seq,
                    evidence_id: link.evidence_id.clone(),
                    status: link.status,
                });
            }
        }
        let Some((fseq, expected)) = self.finalized.clone() else {
            self.gaps.insert(0, Gap::Recording);
            return;
        };
        let manifest = fs::read(self.dir.join("manifest.json"))
            .map_err(|_| "The sealed manifest is missing.")
            .and_then(|bytes| {
                if digest(&bytes) != expected {
                    return Err("The sealed manifest does not match its digest.");
                }
                let m: super::Manifest = serde_json::from_slice(&bytes)
                    .map_err(|_| "The sealed manifest is invalid.")?;
                if m.id != self.id || m.last_seq + 1 != fseq {
                    return Err("The sealed manifest belongs to another record.");
                }
                Ok((bytes, m.status))
            });
        match manifest {
            Ok((bytes, status)) => {
                self.sealed = Some(Sealed {
                    id: self.id.clone(),
                    digest: expected,
                    status,
                });
                self.manifest = Some(bytes);
            }
            Err(reason) => self.gaps.insert(
                0,
                Gap::Manifest {
                    reason: reason.into(),
                },
            ),
        }
    }

    fn verdict(&self, gaps: &[Gap]) -> (EvidenceStatus, bool) {
        let complete = gaps.is_empty() && self.sealed.as_ref().is_some_and(|s| s.status.complete());
        let status = match (&self.sealed, self.finalized.is_some()) {
            (Some(s), _) if complete => s.status,
            (_, true) => EvidenceStatus::Incomplete,
            (_, false) => EvidenceStatus::Recording,
        };
        (status, complete)
    }

    fn event_cursor(&self, seq: u64) -> EventCursor {
        EventCursor {
            evidence_id: self.id.clone(),
            seq,
            record: seq.checked_sub(1).map(|i| self.records[i as usize].clone()),
        }
    }

    fn check_event_cursor(&self, c: &EventCursor) -> Result<u64> {
        if c.evidence_id != self.id {
            return Err(cursor_err("The cursor belongs to another evidence record."));
        }
        if c.seq > self.head() {
            return Err(cursor_err("The cursor is past the readable evidence."));
        }
        if self.event_cursor(c.seq).record != c.record {
            return Err(cursor_err("The cursor does not match the retained record."));
        }
        Ok(c.seq)
    }

    /// A bounded page of events, ordered by sequence ID.
    pub fn events(&self, window: EventWindow, limit: usize) -> Result<EventPage> {
        let limit = limit.clamp(1, MAX_PAGE_EVENTS) as u64;
        let head = self.head();
        // Inclusive sequence range; empty when lo > hi.
        let (lo, hi) = match &window {
            EventWindow::Oldest => (1, limit.min(head)),
            EventWindow::Latest => (head.saturating_sub(limit) + 1, head),
            EventWindow::After(c) => {
                let s = self.check_event_cursor(c)?;
                (s + 1, (s + limit).min(head))
            }
            EventWindow::Before(c) => {
                let s = self.check_event_cursor(c)?;
                (s.saturating_sub(limit) + 1, s)
            }
        };
        let entries = if lo <= hi {
            self.entries[(lo - 1) as usize..hi as usize].to_vec()
        } else {
            Vec::new()
        };
        let gaps: Vec<Gap> = self
            .gaps
            .iter()
            .filter(|g| g.seq().is_none_or(|s| (lo..=hi).contains(&s)))
            .cloned()
            .collect();
        let (status, complete) = self.verdict(&self.gaps);
        Ok(EventPage {
            evidence_id: self.id.clone(),
            entries,
            older: (lo > 1).then(|| self.event_cursor(lo - 1)),
            newer: self.event_cursor(hi.max(lo - 1)),
            head,
            more_newer: hi < head,
            status,
            complete,
            gaps,
        })
    }

    fn stream_model(&self, call: &str, stream: StreamName) -> Result<&StreamModel> {
        Ok(&self
            .calls
            .get(call)
            .ok_or_else(|| EvidenceError::UnknownCall(call.into()))?
            .streams[stream.index()])
    }

    fn stream_cursor(
        &self,
        call: &str,
        stream: StreamName,
        model: &StreamModel,
        offset: u64,
    ) -> StreamCursor {
        let chunk = model.reaching(offset);
        StreamCursor {
            evidence_id: self.id.clone(),
            call: call.into(),
            stream,
            offset,
            chunk_seq: chunk.map(|c| c.seq),
            chunk_digest: chunk.map(|c| c.digest.clone()),
        }
    }

    fn check_stream_cursor(
        &self,
        call: &str,
        stream: StreamName,
        model: &StreamModel,
        c: &StreamCursor,
    ) -> Result<u64> {
        if c.evidence_id != self.id || c.call != call || c.stream != stream {
            return Err(cursor_err("The cursor belongs to another stream."));
        }
        if c.offset > model.length {
            return Err(cursor_err("The cursor is past the retained stream."));
        }
        if self.stream_cursor(call, stream, model, c.offset) != *c {
            return Err(cursor_err("The cursor does not match the retained stream."));
        }
        Ok(c.offset)
    }

    fn spool(&self, call: &str, stream: StreamName) -> Option<File> {
        File::open(
            self.dir
                .join("streams")
                .join(format!("{call}.{}", stream.word())),
        )
        .ok()
    }

    /// The chunk's bytes if they match its recorded length and digest.
    fn read_chunk(file: &mut Option<File>, chunk: &Chunk) -> Option<Vec<u8>> {
        let file = file.as_mut()?;
        file.seek(SeekFrom::Start(chunk.offset)).ok()?;
        let mut bytes = Vec::with_capacity(chunk.length as usize);
        file.take(chunk.length).read_to_end(&mut bytes).ok()?;
        (bytes.len() as u64 == chunk.length && digest(&bytes) == chunk.digest).then_some(bytes)
    }

    fn corrupt(call: &str, stream: StreamName, c: &Chunk) -> Gap {
        Gap::Corrupt {
            seq: c.seq,
            call: call.into(),
            stream,
            start: c.offset,
            end: c.end(),
        }
    }

    /// One contiguous verified run within `[a, b)`. Corrupt chunks at the
    /// near end are skipped (so paging always advances); one after served
    /// bytes ends the run. `backward` prefers the bytes nearest `b`.
    fn serve(
        &self,
        call: &str,
        stream: StreamName,
        model: &StreamModel,
        a: u64,
        b: u64,
        backward: bool,
    ) -> (u64, Vec<u8>, Vec<Gap>) {
        let first = model.chunks.partition_point(|c| c.end() <= a);
        let last = model.chunks.partition_point(|c| c.offset < b);
        let span = &model.chunks[first..last.max(first)];
        let mut file = self.spool(call, stream);
        let mut parts: Vec<(u64, Vec<u8>)> = Vec::new();
        let mut gaps = Vec::new();
        let mut empty_at = if backward { b } else { a };
        let order: Box<dyn Iterator<Item = &Chunk>> = if backward {
            Box::new(span.iter().rev())
        } else {
            Box::new(span.iter())
        };
        for c in order {
            match Self::read_chunk(&mut file, c) {
                Some(bytes) => {
                    let lo = a.max(c.offset);
                    let hi = b.min(c.end());
                    parts.push((
                        lo,
                        bytes[(lo - c.offset) as usize..(hi - c.offset) as usize].to_vec(),
                    ));
                }
                None => {
                    gaps.push(Self::corrupt(call, stream, c));
                    if !parts.is_empty() {
                        break;
                    }
                    empty_at = if backward { c.offset } else { c.end() };
                }
            }
        }
        if backward {
            parts.reverse();
        }
        let start = parts.first().map_or(empty_at, |p| p.0);
        (start, parts.into_iter().flat_map(|p| p.1).collect(), gaps)
    }

    /// A bounded page of one stream's verified original retained bytes.
    pub fn stream(
        &self,
        call: &str,
        stream: StreamName,
        window: StreamWindow,
        max_bytes: u64,
    ) -> Result<StreamPage> {
        let model = self.stream_model(call, stream)?;
        let max = max_bytes.clamp(1, MAX_PAGE_BYTES);
        let len = model.length;
        let (a, b, backward) = match &window {
            StreamWindow::Oldest => (0, max.min(len), false),
            StreamWindow::Latest => (len.saturating_sub(max), len, true),
            StreamWindow::After(c) => {
                let o = self.check_stream_cursor(call, stream, model, c)?;
                (o, (o + max).min(len), false)
            }
            StreamWindow::Before(c) => {
                let o = self.check_stream_cursor(call, stream, model, c)?;
                (o.saturating_sub(max), o, true)
            }
        };
        let (start, bytes, corrupt) = self.serve(call, stream, model, a, b, backward);
        let end = start + bytes.len() as u64;
        let mut gaps: Vec<Gap> = self
            .gaps
            .iter()
            .filter(|g| g.touches(call, stream))
            .cloned()
            .collect();
        let found = !corrupt.is_empty();
        gaps.extend(corrupt);
        let (status, mut complete) = self.verdict(&self.gaps);
        if found {
            complete = false;
        }
        let status = if found && status.complete() {
            EvidenceStatus::Incomplete
        } else {
            status
        };
        Ok(StreamPage {
            evidence_id: self.id.clone(),
            call: call.into(),
            stream,
            start,
            digest: digest(&bytes),
            bytes,
            length: len,
            state: model.state,
            older: (start > 0).then(|| self.stream_cursor(call, stream, model, start)),
            newer: self.stream_cursor(call, stream, model, end),
            more_newer: end < len || model.state.is_none(),
            status,
            complete,
            gaps,
        })
    }

    fn summarize(&self, gaps: Vec<Gap>) -> EvidenceSummary {
        let coverage = |s: &StreamModel| StreamCoverage {
            length: s.length,
            state: s.state,
            dropped_bytes: s.dropped,
            chunks: s.chunks.len() as u64,
        };
        let (status, complete) = self.verdict(&gaps);
        EvidenceSummary {
            evidence_id: self.id.clone(),
            head: self.event_cursor(self.head()),
            sealed: self.sealed.clone(),
            status,
            complete,
            calls: self
                .order
                .iter()
                .map(|id| {
                    let c = &self.calls[id];
                    CallCoverage {
                        call: id.clone(),
                        tool: c.tool.clone(),
                        outcome: CallOutcome::fold(&c.results),
                        arguments_retained: c.arguments_retained,
                        stdout: coverage(&c.streams[0]),
                        stderr: coverage(&c.streams[1]),
                    }
                })
                .collect(),
            children: self.children.iter().map(|(_, l)| l.clone()).collect(),
            gaps,
        }
    }

    /// Coverage and gaps from the log and manifests, without reading
    /// stream bytes.
    pub fn summary(&self) -> EvidenceSummary {
        self.summarize(self.gaps.clone())
    }

    /// [`Self::summary`] after checking every retained byte and every
    /// archived child.
    pub fn verify(&self) -> EvidenceSummary {
        let mut gaps = self.gaps.clone();
        for id in &self.order {
            for stream in [StreamName::Stdout, StreamName::Stderr] {
                let model = &self.calls[id].streams[stream.index()];
                let mut file = self.spool(id, stream);
                for c in &model.chunks {
                    if Self::read_chunk(&mut file, c).is_none() {
                        gaps.push(Self::corrupt(id, stream, c));
                    }
                }
            }
        }
        for (seq, link) in &self.children {
            if self.flagged(&gaps, &link.evidence_id) {
                continue;
            }
            match self.child(&link.evidence_id).map(|c| c.verify()) {
                Ok(s) if s.complete => {}
                Ok(s) => gaps.push(Gap::ChildIncomplete {
                    seq: *seq,
                    evidence_id: link.evidence_id.clone(),
                    status: s.status,
                }),
                Err(_) => gaps.push(Gap::ChildMissing {
                    seq: *seq,
                    evidence_id: link.evidence_id.clone(),
                    reason: "The child evidence could not be read.".into(),
                }),
            }
        }
        self.summarize(gaps)
    }

    fn flagged(&self, gaps: &[Gap], evidence_id: &str) -> bool {
        gaps.iter().any(|g| {
            matches!(g, Gap::ChildMissing { evidence_id: e, .. }
                | Gap::ChildIncomplete { evidence_id: e, .. } if e == evidence_id)
        })
    }

    /// A reader for an archived child, checked against its link digest.
    pub fn child(&self, evidence_id: &str) -> Result<EvidenceReader> {
        let (_, link) = self
            .children
            .iter()
            .find(|(_, l)| l.evidence_id == evidence_id)
            .ok_or(EvidenceError::Invalid("No such archived child."))?;
        let child = EvidenceReader::open(self.dir.join("children").join(evidence_id), evidence_id)?;
        if child.sealed.as_ref().map(|s| s.digest.as_str()) != Some(link.manifest_digest.as_str()) {
            return Err(EvidenceError::Corrupt(
                "The child evidence does not match its link.",
            ));
        }
        Ok(child)
    }

    /// Write a portable directory bundle at `dest` (which must not exist):
    /// the verified log, the sealed manifest, every stream with verified
    /// bytes at their offsets, each child's bundle, and `export.json`,
    /// written last, listing every file's length and digest and every gap.
    pub fn export(&self, dest: &Path) -> Result<ExportManifest> {
        if dest.exists() {
            return Err(EvidenceError::Invalid(
                "The export directory already exists.",
            ));
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| EvidenceError::Io("The export root could not be created."))?;
        }
        private_dir(dest)?;
        let mut files = Vec::new();
        let mut gaps = self.gaps.clone();
        write_file(dest, "events.jsonl", &self.log, &mut files)?;
        if let Some(manifest) = &self.manifest {
            write_file(dest, "manifest.json", manifest, &mut files)?;
        }
        if !self.order.is_empty() {
            private_dir(&dest.join("streams"))?;
        }
        for id in &self.order {
            for stream in [StreamName::Stdout, StreamName::Stderr] {
                let model = &self.calls[id].streams[stream.index()];
                let rel = format!("streams/{id}.{}", stream.word());
                let mut out = new_file(&dest.join(&rel))?;
                let mut hasher = Sha256::new();
                let mut spool = self.spool(id, stream);
                for c in &model.chunks {
                    let bytes = Self::read_chunk(&mut spool, c).unwrap_or_else(|| {
                        gaps.push(Self::corrupt(id, stream, c));
                        vec![0; c.length as usize]
                    });
                    hasher.update(&bytes);
                    out.write_all(&bytes)
                        .map_err(|_| EvidenceError::Io("An export file could not be written."))?;
                }
                out.sync_all()
                    .map_err(|_| EvidenceError::Io("An export file could not be written."))?;
                files.push(ExportFile {
                    path: rel,
                    length: model.length,
                    digest: format!("{:x}", hasher.finalize()),
                });
            }
        }
        for (seq, link) in &self.children {
            if self.flagged(&gaps, &link.evidence_id) {
                continue;
            }
            if !dest.join("children").exists() {
                private_dir(&dest.join("children"))?;
            }
            let rel = format!("children/{}", link.evidence_id);
            match self.child(&link.evidence_id) {
                Ok(child) => {
                    let exported = child.export(&dest.join(&rel))?;
                    let bytes = fs::read(dest.join(&rel).join("export.json"))
                        .map_err(|_| EvidenceError::Io("A child export could not be read."))?;
                    files.push(ExportFile {
                        path: format!("{rel}/export.json"),
                        length: bytes.len() as u64,
                        digest: digest(&bytes),
                    });
                    if !exported.complete {
                        gaps.push(Gap::ChildIncomplete {
                            seq: *seq,
                            evidence_id: link.evidence_id.clone(),
                            status: exported.status,
                        });
                    }
                }
                Err(_) => gaps.push(Gap::ChildMissing {
                    seq: *seq,
                    evidence_id: link.evidence_id.clone(),
                    reason: "The child evidence could not be read.".into(),
                }),
            }
        }
        let (status, complete) = self.verdict(&gaps);
        let manifest = ExportManifest {
            schema: EXPORT_SCHEMA.into(),
            evidence_id: self.id.clone(),
            sealed: self.sealed.clone(),
            head_seq: self.head(),
            status,
            complete,
            gaps,
            files,
        };
        let bytes = serde_json::to_vec_pretty(&manifest).expect("export manifest encodes");
        write_file(dest, "export.json", &bytes, &mut Vec::new())?;
        Ok(manifest)
    }
}

fn new_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options
        .open(path)
        .map_err(|_| EvidenceError::Io("An export file could not be created."))
}

fn write_file(dest: &Path, rel: &str, bytes: &[u8], files: &mut Vec<ExportFile>) -> Result<()> {
    let mut file = new_file(&dest.join(rel))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| EvidenceError::Io("An export file could not be written."))?;
    files.push(ExportFile {
        path: rel.into(),
        length: bytes.len() as u64,
        digest: digest(bytes),
    });
    Ok(())
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
