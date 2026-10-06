//! A client's record streams on one attachment: snapshot streams, which
//! restore a terminal, and history streams, which add older rows to it.
//!
//! A host sends a snapshot stream when an attachment joins by snapshot and
//! again whenever the attachment falls behind its replay buffer, and a
//! history stream for each history read. Parts of different streams can
//! interleave and, over a relay, arrive out of order. [`Streams`] holds
//! each stream's parts until its first records say what it is, checks it
//! with `coder_pty::ext::Assembler`, and reports what to apply.

use std::collections::{BTreeMap, VecDeque};

use coder_pty::ext::{
    Assembler, HistoryRecord, PARTS_AHEAD, RECORD_HEADER, Record, RecordsFrame, StreamKind, Tag,
};
use coder_pty::wire::{Exit, Reason, Refusal, TerminalRef};

use crate::Terminal;
use crate::snapshot::Restore;

/// The most streams a client holds at once.
const STREAMS_MAX: usize = 4;
/// How many ended streams a client remembers, to ignore a relay's replay
/// of their parts.
const DONE_MAX: usize = 64;

/// What a part of a record stream completed.
#[derive(Debug)]
pub enum StreamEvent {
    /// A snapshot reached `READY`: draw `terminal` and apply sequenced
    /// frames after `through`.
    Ready {
        terminal: Box<Terminal>,
        through: u64,
        /// The process's exit, when it ended at or before `through`.
        exit: Option<Exit>,
    },
    /// A page of older rows, for `Terminal::attach_history` in `epoch`.
    History { epoch: u64, page: HistoryRecord },
    /// A stream ended with `FINISH`.
    Finished { stream: String },
}

/// A history read this client sent and expects a stream for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Read {
    epoch: u64,
    before: u64,
    rows: u64,
}

enum Stream {
    /// The kind is not known yet: parts by index.
    Unknown(BTreeMap<u32, RecordsFrame>),
    Snapshot {
        assembler: Assembler,
        restore: Restore,
    },
    History {
        assembler: Assembler,
        epoch: u64,
    },
}

/// The record streams of one attachment.
pub struct Streams {
    terminal: TerminalRef,
    scrollback: usize,
    read: Option<Read>,
    streams: BTreeMap<String, Stream>,
    /// Streams that ended or failed, newest last.
    done: VecDeque<String>,
}

impl std::fmt::Debug for Streams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Streams")
            .field("streams", &self.streams.len())
            .finish_non_exhaustive()
    }
}

impl Streams {
    /// Streams for `terminal`, restoring terminals that keep at most
    /// `scrollback` history lines.
    #[must_use]
    pub fn new(terminal: TerminalRef, scrollback: usize) -> Self {
        Streams {
            terminal,
            scrollback,
            read: None,
            streams: BTreeMap::new(),
            done: VecDeque::new(),
        }
    }

    /// Expects the stream of a history read just sent.
    pub fn expect_history(&mut self, epoch: u64, before: u64, rows: u64) {
        self.read = Some(Read {
            epoch,
            before,
            rows,
        });
    }

    /// Whether a history read is outstanding.
    #[must_use]
    pub fn reading(&self) -> bool {
        self.read.is_some()
    }

    /// Takes one part. A malformed stream refuses, and only that stream is
    /// dropped: what an earlier stream restored stays. A part of a stream
    /// that already ended, as a relay replays after a reconnect, changes
    /// nothing.
    pub fn push(&mut self, part: &RecordsFrame) -> Result<Vec<StreamEvent>, Refusal> {
        part.check()?;
        let id = part.stream.clone();
        if self.done.contains(&id) {
            return Ok(Vec::new());
        }
        let result = self.push_inner(&id, part);
        if result.is_err() {
            self.end(&id);
        }
        result
    }

    fn end(&mut self, id: &str) {
        self.streams.remove(id);
        self.done.push_back(id.to_owned());
        if self.done.len() > DONE_MAX {
            self.done.pop_front();
        }
    }

    fn push_inner(&mut self, id: &str, part: &RecordsFrame) -> Result<Vec<StreamEvent>, Refusal> {
        if !self.streams.contains_key(id) {
            if self.streams.len() >= STREAMS_MAX {
                return Err(Refusal::new(
                    Reason::LimitExceeded,
                    "too many record streams at once",
                ));
            }
            self.streams
                .insert(id.to_owned(), Stream::Unknown(BTreeMap::new()));
        }
        let stream = self.streams.get_mut(id).expect("inserted");
        if let Stream::Unknown(parts) = stream {
            if parts.len() >= PARTS_AHEAD as usize {
                return Err(Refusal::new(
                    Reason::LimitExceeded,
                    "too many parts before a stream's kind is known",
                ));
            }
            parts.insert(part.part, part.clone());
            let Some(kind) = classify(parts) else {
                return Ok(Vec::new());
            };
            let held = std::mem::take(parts);
            *stream = match kind {
                Tag::State => Stream::Snapshot {
                    assembler: Assembler::new(StreamKind::Snapshot, self.terminal.clone()),
                    restore: Restore::new(self.scrollback),
                },
                _ => {
                    let Some(read) = self.read.take() else {
                        return Err(Refusal::new(
                            Reason::Malformed,
                            "a history stream nobody asked for",
                        ));
                    };
                    Stream::History {
                        assembler: Assembler::new(
                            StreamKind::History {
                                epoch: read.epoch,
                                before: read.before,
                                rows: read.rows,
                            },
                            self.terminal.clone(),
                        ),
                        epoch: read.epoch,
                    }
                }
            };
            let mut events = Vec::new();
            for part in held.values() {
                events.extend(self.apply(id, part)?);
            }
            return Ok(events);
        }
        self.apply(id, part)
    }

    fn apply(&mut self, id: &str, part: &RecordsFrame) -> Result<Vec<StreamEvent>, Refusal> {
        let stream = self.streams.get_mut(id).expect("known");
        let mut events = Vec::new();
        match stream {
            Stream::Unknown(_) => unreachable!("classified"),
            Stream::Snapshot { assembler, restore } => {
                for record in assembler.push(part)? {
                    match record {
                        Record::History(page) => {
                            let epoch = restore.binding().map_or(0, |binding| binding.epoch);
                            events.push(StreamEvent::History { epoch, page });
                        }
                        Record::Finish(_) => {}
                        record => {
                            if let Some(terminal) = restore.push(&record)? {
                                let binding = restore.binding().expect("bound at READY");
                                events.push(StreamEvent::Ready {
                                    terminal: Box::new(terminal),
                                    through: binding.through,
                                    exit: binding.exit,
                                });
                            }
                        }
                    }
                }
                if assembler.finished() {
                    self.end(id);
                    events.push(StreamEvent::Finished { stream: id.into() });
                }
            }
            Stream::History { assembler, epoch } => {
                let epoch = *epoch;
                for record in assembler.push(part)? {
                    if let Record::History(page) = record {
                        events.push(StreamEvent::History { epoch, page });
                    }
                }
                if assembler.finished() {
                    self.end(id);
                    events.push(StreamEvent::Finished { stream: id.into() });
                }
            }
        }
        Ok(events)
    }
}

/// A stream's kind from its first two records, once the parts from the
/// first hold both headers: `STATE` second for a snapshot, anything else
/// for history.
fn classify(parts: &BTreeMap<u32, RecordsFrame>) -> Option<Tag> {
    let mut prefix = Vec::new();
    for (expected, (index, part)) in parts.iter().enumerate() {
        if *index != expected as u32 {
            break;
        }
        prefix.extend_from_slice(&part.data);
        if prefix.len() >= RECORD_HEADER {
            let length = u32::from_le_bytes([prefix[2], prefix[3], prefix[4], prefix[5]]) as usize;
            let second = RECORD_HEADER.checked_add(length)?;
            if prefix.len() >= second + 2 {
                let tag = u16::from_le_bytes([prefix[second], prefix[second + 1]]);
                return Some(if tag == Tag::State as u16 {
                    Tag::State
                } else {
                    Tag::History
                });
            }
        }
        if part.last {
            // A stream too short to hold two records is a history stream
            // or malformed; its assembler decides.
            return Some(Tag::History);
        }
    }
    None
}
