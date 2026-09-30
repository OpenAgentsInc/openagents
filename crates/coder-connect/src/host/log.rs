//! What reads leave behind, kept off the book so a read never rewrites it.
//!
//! Each admitted request is bound to its exact event in memory and in an
//! append-only request log, `observer.requests`, before the host reads for
//! it; a relay reply is appended before it is sent. Appends happen under
//! the store lock and are not synced to disk: the log survives a host
//! process crash, and another process sharing the store catches up on it
//! before admitting a request. Grant changes still rewrite and sync the book.
use super::*;
use crate::store;
use std::collections::HashMap;
use std::io::{Read as _, Seek, SeekFrom, Write};

pub(super) const LOG: &str = "observer.requests";
/// The log is rewritten with only its live records past this size.
const COMPACT_AT: u64 = if cfg!(test) { 16 * 1024 } else { 4 << 20 };
/// The log never holds more than the book's own ceiling.
const MAX_LOG: u64 = 64 * 1024 * 1024;

/// One line of the log: a request's binding and its grant's read window
/// after it, and for a relay reply, the exact reply.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub request: String,
    pub grant: String,
    pub request_event: String,
    pub expires_at: u64,
    pub window_start: u64,
    pub reads: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<Event>,
}

/// What the host holds for a bound request.
pub(super) enum Answer {
    /// This process is reading for it now.
    Pending,
    /// Another process, or this store before a restart, admitted it and
    /// left no reply here.
    Claimed,
    /// A signed relay reply, sent as is to an exact retry.
    Relay(Event),
    /// A direct reply and its detached body, kept only in memory.
    Direct { reply: Event, payload: String },
}
pub(super) struct Binding {
    pub grant: String,
    pub request_event: String,
    pub expires_at: u64,
    pub answer: Answer,
}

/// A store's read state within this process, behind its gate.
#[derive(Default)]
pub(super) struct Reads {
    /// The host key, read once.
    pub secret: Option<SecretKey>,
    /// The log file this process has read, and how far.
    file: Option<(u64, u64)>,
    offset: u64,
    pub bindings: HashMap<String, Binding>,
}

/// Merge a later read window into a grant's: the later window wins, and
/// within one window the higher count.
pub(super) fn merge_window(admission: &mut Admission, window_start: u64, reads: u32) {
    let reads = reads.min(MAX_READS_PER_MINUTE);
    if window_start > admission.window_start {
        admission.window_start = window_start;
        admission.reads = reads;
    } else if window_start == admission.window_start {
        admission.reads = admission.reads.max(reads);
    }
}

impl Reads {
    /// Apply every record another writer appended since this process last
    /// looked. A torn last line (a crash mid-append) is skipped.
    pub fn catch_up(&mut self, directory: &Path, book: &mut Book) -> Result<()> {
        let Some(mut file) = store::private_append(&directory.join(LOG), false)? else {
            self.file = None;
            self.offset = 0;
            return Ok(());
        };
        let m = file.metadata().map_err(unavailable)?;
        let id = store::file_id(&file, &m)?;
        if self.file != Some(id) || m.len() < self.offset {
            self.file = Some(id);
            self.offset = 0;
        }
        if m.len() == self.offset {
            return Ok(());
        }
        if m.len() > MAX_LOG {
            return fail(ErrorCode::Bounds, "observer request log exceeds its bound");
        }
        let mut bytes = Vec::with_capacity((m.len() - self.offset) as usize);
        file.seek(SeekFrom::Start(self.offset))
            .and_then(|_| file.take(m.len() - self.offset).read_to_end(&mut bytes))
            .map_err(unavailable)?;
        let whole = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        for line in bytes[..whole].split(|b| *b == b'\n') {
            if let Ok(record) = serde_json::from_slice::<Record>(line) {
                self.apply(record, book);
            }
        }
        self.offset += whole as u64;
        Ok(())
    }
    fn apply(&mut self, record: Record, book: &mut Book) {
        let Some(admission) = book.admissions.get_mut(&record.grant) else {
            return;
        };
        merge_window(admission, record.window_start, record.reads);
        // Only a reply this host signed for exactly this request is kept.
        let reply = record.reply.filter(|reply| {
            reply.pubkey == book.host
                && reply.tag_values("h").collect::<Vec<_>>() == [record.request.as_str()]
        });
        match self.bindings.entry(record.request) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(Binding {
                    grant: record.grant,
                    request_event: record.request_event,
                    expires_at: record.expires_at,
                    answer: reply.map_or(Answer::Claimed, Answer::Relay),
                });
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let binding = entry.get_mut();
                if binding.request_event == record.request_event
                    && matches!(binding.answer, Answer::Claimed)
                    && let Some(reply) = reply
                {
                    binding.answer = Answer::Relay(reply);
                }
            }
        }
    }
    /// Append one record. Call only under the store lock, after
    /// [`Reads::catch_up`], so the log's end is this process's offset.
    pub fn append(&mut self, directory: &Path, record: &Record) -> Result<()> {
        let mut line = serde_json::to_vec(record)
            .map_err(|_| Error::new(ErrorCode::Malformed, "observer record serialization"))?;
        line.push(b'\n');
        if self.offset + line.len() as u64 > MAX_LOG {
            return fail(ErrorCode::Bounds, "observer store retention limit exceeded");
        }
        let mut file =
            store::private_append(&directory.join(LOG), true)?.ok_or_else(|| unavailable(()))?;
        let m = file.metadata().map_err(unavailable)?;
        let id = store::file_id(&file, &m)?;
        if self.file.is_none() && m.len() == 0 {
            self.file = Some(id);
        }
        if self.file != Some(id) || m.len() < self.offset {
            // Another writer's records are unread: refuse rather than skip them.
            return fail(ErrorCode::Conflict, "observer request log changed");
        }
        // Past the offset is only a torn line; end it so it stays one line.
        if m.len() > self.offset {
            line.insert(0, b'\n');
        }
        file.write_all(&line).map_err(unavailable)?;
        self.offset = m.len() + line.len() as u64;
        Ok(())
    }
    /// Past its size bound, rewrite the log with only the live records of
    /// live grants. Call only under the store lock, after catching up.
    pub fn compact(&mut self, directory: &Path, book: &Book, now: u64) -> Result<()> {
        if self.offset < COMPACT_AT {
            return Ok(());
        }
        self.bindings.retain(|_, b| b.expires_at > now);
        let mut bytes = Vec::new();
        for (request, binding) in &self.bindings {
            let Some(admission) = book
                .admissions
                .get(&binding.grant)
                .filter(|a| a.revoked_at.is_none())
            else {
                continue;
            };
            let record = Record {
                request: request.clone(),
                grant: binding.grant.clone(),
                request_event: binding.request_event.clone(),
                expires_at: binding.expires_at,
                window_start: admission.window_start,
                reads: admission.reads,
                reply: match &binding.answer {
                    Answer::Relay(reply) => Some(reply.clone()),
                    _ => None,
                },
            };
            bytes.extend(
                serde_json::to_vec(&record).map_err(|_| {
                    Error::new(ErrorCode::Malformed, "observer record serialization")
                })?,
            );
            bytes.push(b'\n');
        }
        let pending = directory.join(format!(".{LOG}.pending"));
        if pending.exists() {
            let _checked = store::private(&pending, false)?;
            std::fs::remove_file(&pending).map_err(unavailable)?;
        }
        let mut file = store::private(&pending, true)?;
        file.write_all(&bytes).map_err(unavailable)?;
        let m = file.metadata().map_err(unavailable)?;
        let id = store::file_id(&file, &m)?;
        std::fs::rename(&pending, directory.join(LOG)).map_err(unavailable)?;
        self.file = Some(id);
        self.offset = bytes.len() as u64;
        Ok(())
    }
}

fn unavailable<E>(_: E) -> Error {
    Error::new(
        ErrorCode::Unavailable,
        "private observer store is unavailable",
    )
}
