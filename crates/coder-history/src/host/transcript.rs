use super::{History, catalog, confined, digest, encoded_len};
use crate::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs::{File, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

struct Prefix {
    hash: Sha256,
    record_offset: u64,
    record_index: u64,
    tail: Vec<u8>,
}

/// The hash state of a file's bytes up to a record boundary, so a later
/// read hashes onward from there rather than from byte 0.
#[derive(Clone)]
struct Mark {
    hash: Sha256,
    record_index: u64,
}

/// Bytes between marks: hashing onward from the nearest one costs at most
/// this much before the requested offset.
const MARK_INTERVAL: u64 = 1024 * 1024;
/// The most leading bytes kept to recognize a marked file.
const HEAD_BYTES: u64 = 4096;
/// Files whose marks are kept, the least recently used leaving first.
const MARKED_FILES: usize = 64;

/// A file's marks, and the length and last write they were taken under.
struct Marked {
    size: u64,
    modified: Option<SystemTime>,
    /// The file's first bytes when marked, which a reused inode or a
    /// rewritten file almost always changes.
    head: Vec<u8>,
    marks: BTreeMap<u64, Mark>,
    used: u64,
}

#[derive(Default)]
struct Marks {
    files: HashMap<String, Marked>,
    clock: u64,
}

/// Prefix-hash marks for the life of the process, keyed by file identity
/// (the incarnation: device, inode, and creation time).
///
/// A mark stands for the hash of `0..offset` without reading those bytes
/// again, which holds while the file only grows by appending, as the
/// harnesses write their session files. A file that is no longer
/// append-only is caught as far as its length and last write show it:
/// shorter than when marked, or the same length with a different last
/// write, or with different first bytes (a reused inode with no creation
/// time, or a rewrite), and every mark for it is dropped, so the read hashes from byte 0
/// as it always did and a changed prefix refuses. A file rewritten in place
/// and made longer between two reads, without a new inode, is the one case
/// a mark can hide; the bytes from the nearest mark onward are still hashed
/// and compared on every read.
static MARKS: LazyLock<Mutex<Marks>> = LazyLock::new(Mutex::default);

fn marks() -> std::sync::MutexGuard<'static, Marks> {
    MARKS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The nearest usable mark at or before `through`, or `None` when the file
/// has none or has changed in a way appending cannot explain.
fn nearest_mark(
    incarnation: &str,
    meta: &Metadata,
    head: &[u8],
    through: u64,
) -> Option<(u64, Mark)> {
    let mut marks = marks();
    marks.clock += 1;
    let clock = marks.clock;
    let marked = marks.files.get_mut(incarnation)?;
    let modified = meta.modified().ok();
    if meta.len() < marked.size
        || (meta.len() == marked.size && modified != marked.modified)
        || !head.starts_with(&marked.head)
    {
        marks.files.remove(incarnation);
        return None;
    }
    marked.used = clock;
    marked
        .marks
        .range(..=through)
        .next_back()
        .map(|(offset, mark)| (*offset, mark.clone()))
}

/// Keep `found` for this file, taken while it was as `meta` describes. The
/// length and last write are those seen before hashing, so a rewrite during
/// the read shows as a changed last write at the next.
fn remember(incarnation: &str, meta: &Metadata, head: Vec<u8>, found: Vec<(u64, Mark)>) {
    let mut marks = marks();
    marks.clock += 1;
    let clock = marks.clock;
    let modified = meta.modified().ok();
    if !marks.files.contains_key(incarnation) {
        if found.is_empty() {
            return;
        }
        if marks.files.len() >= MARKED_FILES {
            let oldest = marks
                .files
                .iter()
                .min_by_key(|(_, marked)| marked.used)
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                marks.files.remove(&oldest);
            }
        }
    }
    let marked = marks
        .files
        .entry(incarnation.to_owned())
        .or_insert_with(|| Marked {
            size: meta.len(),
            modified,
            head: Vec::new(),
            marks: BTreeMap::new(),
            used: clock,
        });
    marked.size = meta.len();
    marked.modified = modified;
    marked.head = head;
    marked.used = clock;
    marked.marks.extend(found);
}

/// The hash, record count, and unfinished record of `0..through`, hashing
/// only onward from the file's nearest mark, and keeping the marks passed.
fn prefix(
    file: &mut File,
    incarnation: &str,
    meta: &Metadata,
    through: u64,
) -> Result<Prefix, Error> {
    let head = read_at(file, 0, meta.len().min(HEAD_BYTES))?;
    let (start, mark) = nearest_mark(incarnation, meta, &head, through).unwrap_or((
        0,
        Mark {
            hash: Sha256::new(),
            record_index: 0,
        },
    ));
    file.seek(SeekFrom::Start(start))
        .map_err(|_| Error::SourceUnreadable)?;
    let mut out = Prefix {
        hash: mark.hash,
        record_offset: start,
        record_index: mark.record_index,
        tail: Vec::new(),
    };
    let mut found = Vec::new();
    let mut last_mark = start;
    let mut position = start;
    let mut buffer = vec![0u8; 256 * 1024];
    while position < through {
        let count = usize::try_from((through - position).min(buffer.len() as u64))
            .map_err(|_| Error::ResourceLimit)?;
        file.read_exact(&mut buffer[..count])
            .map_err(|_| Error::SourceChanged)?;
        let mut hashed = 0;
        let mut chunk_end = position;
        for chunk in buffer[..count].split_inclusive(|b| *b == b'\n') {
            chunk_end += chunk.len() as u64;
            if chunk_end - out.record_offset <= confined::MAX_PARSE_BYTES as u64 {
                out.tail.extend_from_slice(chunk);
            } else {
                out.tail.clear();
            }
            if chunk.ends_with(b"\n") {
                out.record_offset = chunk_end;
                out.record_index += 1;
                out.tail.clear();
                if chunk_end - last_mark >= MARK_INTERVAL {
                    let upto = (chunk_end - position) as usize;
                    out.hash.update(&buffer[hashed..upto]);
                    hashed = upto;
                    found.push((
                        chunk_end,
                        Mark {
                            hash: out.hash.clone(),
                            record_index: out.record_index,
                        },
                    ));
                    last_mark = chunk_end;
                }
            }
        }
        out.hash.update(&buffer[hashed..count]);
        position += count as u64;
    }
    remember(incarnation, meta, head, found);
    Ok(out)
}

fn hash_string(hash: &Sha256) -> String {
    hash.clone()
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn page(
    history: &History,
    request: TranscriptRequest,
    limits: Limits,
) -> Result<TranscriptPage, Error> {
    if request.max_bytes == 0
        || request.max_bytes > limits.page_bytes
        || request.source_id.len() != 64
        || !request.source_id.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(Error::InvalidRequest);
    }
    let mut source = catalog::find(history, &request.source_id)?;
    let mut file = match history.roots[source.root].open_file(&source.relative) {
        Ok(file) => file,
        // A remembered place can be gone: list again before refusing.
        Err(_) if source.stat.is_none() => {
            let (sources, _) = catalog::scan(history)?;
            source = sources
                .into_iter()
                .find(|s| s.id == request.source_id)
                .ok_or(Error::SourceMissing)?;
            history.roots[source.root].open_file(&source.relative)?
        }
        Err(error) => return Err(error),
    };
    let root = &history.roots[source.root];
    let meta = file.metadata().map_err(|_| Error::SourceUnreadable)?;
    if meta.len() > confined::MAX_SOURCE_BYTES {
        return Err(Error::ResourceLimit);
    }
    let incarnation = confined::incarnation(&meta);
    if let Some(end) = request.end {
        if request.cursor.is_some() {
            return Err(Error::InvalidRequest);
        }
        let (mut start, end) =
            backward_window(&mut file, meta.len(), end, request.max_bytes, limits.chunks)?;
        loop {
            let progress = prefix(&mut file, &incarnation, &meta, start)?;
            match read_range(
                root,
                &source,
                &mut file,
                &incarnation,
                meta.len(),
                (progress, start),
                end,
                true,
                limits,
            ) {
                // Records that do not fit the encoded bound together: keep
                // the newer half of them, down to the newest record alone.
                Err(Error::ResourceLimit) => match newer_half(&mut file, start, end)? {
                    Some(newer) => start = newer,
                    None => return Err(Error::ResourceLimit),
                },
                other => return other,
            }
        }
    }
    let cursor = request.cursor.unwrap_or_else(|| TranscriptCursor {
        source_id: source.id.clone(),
        incarnation: incarnation.clone(),
        offset: 0,
        record_offset: 0,
        record_index: 0,
        prefix_sha256: digest(b""),
    });
    if cursor.source_id != source.id
        || cursor.incarnation != incarnation
        || cursor.offset > meta.len()
    {
        return Err(Error::SourceChanged);
    }
    let progress = prefix(&mut file, &incarnation, &meta, cursor.offset)?;
    if cursor.record_offset != progress.record_offset
        || cursor.record_index != progress.record_index
        || cursor.prefix_sha256 != hash_string(&progress.hash)
    {
        return Err(Error::SourceChanged);
    }
    let end = cursor.offset + u64::from(request.max_bytes).min(meta.len() - cursor.offset);
    read_range(
        root,
        &source,
        &mut file,
        &incarnation,
        meta.len(),
        (progress, cursor.offset),
        end,
        false,
        limits,
    )
}

/// The start of the newer half of the whole records in `start..end`: just
/// after the first newline at or past their middle, else the newest record's
/// start; `None` when they are one record.
fn newer_half(file: &mut File, start: u64, end: u64) -> Result<Option<u64>, Error> {
    let middle = start + (end - start) / 2;
    // The last byte is the final record's newline; a newline before it
    // starts a newer record.
    let bytes = read_at(file, middle, end.saturating_sub(1).saturating_sub(middle))?;
    let newer = match bytes.iter().position(|b| *b == b'\n') {
        Some(index) => middle + index as u64 + 1,
        // The newest record started before the middle: keep it alone.
        None => last_newline_end(file, end.saturating_sub(1))?,
    };
    Ok(Some(newer).filter(|newer| *newer > start && *newer < end))
}

/// The most records larger than a page that one backward read passes over.
const MAX_SKIPPED: usize = 32;

/// The byte range of a backward read: whole records that end at or before
/// `end` (the newest complete record for `u64::MAX`), within `max_bytes`.
/// A record larger than the page never fits one, so the read passes over it
/// and any such records before it, up to [`MAX_SKIPPED`], and pages the
/// records that end where they start; when only such records remain, the
/// range is empty at the start of the earliest one passed over.
fn backward_window(
    file: &mut File,
    len: u64,
    end: u64,
    max_bytes: u32,
    max_chunks: usize,
) -> Result<(u64, u64), Error> {
    let limit = end.min(len);
    // End at a record boundary: after the last newline at or before limit.
    let mut end = last_newline_end(file, limit)?;
    for _ in 0..=MAX_SKIPPED {
        let window = end.saturating_sub(u64::from(max_bytes));
        if window == 0 {
            return Ok((0, end));
        }
        let bytes = read_at(file, window - 1, end - (window - 1))?;
        // Start just after a newline at or after window - 1, so the window
        // begins at a record boundary.
        match bytes.iter().position(|b| *b == b'\n') {
            Some(index) if window + (index as u64) < end => {
                let start = window + index as u64;
                return Ok((within_chunk_limit(file, start, end, max_chunks)?, end));
            }
            // One record fills the page: pass over it.
            _ => end = last_newline_end(file, window)?,
        }
    }
    Ok((end, end))
}

/// Move `start` past whole records until `start..end` fits in one page's
/// chunk limit, keeping the newest records.
fn within_chunk_limit(
    file: &mut File,
    start: u64,
    end: u64,
    max_chunks: usize,
) -> Result<u64, Error> {
    let bytes = read_at(file, start, end - start)?;
    let mut chunks = 0;
    let mut kept = end;
    for line in bytes.split_inclusive(|b| *b == b'\n').rev() {
        chunks += line.len().div_ceil(MAX_CHUNK_BYTES).max(1);
        if chunks > max_chunks {
            break;
        }
        kept -= line.len() as u64;
    }
    Ok(kept)
}

/// The offset just past the last newline at or before `limit`, or 0.
fn last_newline_end(file: &mut File, limit: u64) -> Result<u64, Error> {
    let mut position = limit;
    while position > 0 {
        let from = position.saturating_sub(64 * 1024);
        let bytes = read_at(file, from, position - from)?;
        if let Some(index) = bytes.iter().rposition(|b| *b == b'\n') {
            return Ok(from + index as u64 + 1);
        }
        position = from;
    }
    Ok(0)
}

fn read_at(file: &mut File, from: u64, count: u64) -> Result<Vec<u8>, Error> {
    let count = usize::try_from(count).map_err(|_| Error::ResourceLimit)?;
    let mut bytes = vec![0; count];
    file.seek(SeekFrom::Start(from))
        .map_err(|_| Error::SourceUnreadable)?;
    file.read_exact(&mut bytes)
        .map_err(|_| Error::SourceChanged)?;
    Ok(bytes)
}

/// Read `start..end` as one page. A backward page reports `previous` and
/// never includes a partial record.
#[allow(clippy::too_many_arguments)]
fn read_range(
    root: &confined::Root,
    source: &catalog::Source,
    file: &mut File,
    incarnation: &str,
    len: u64,
    (mut progress, start): (Prefix, u64),
    end: u64,
    backward: bool,
    limits: Limits,
) -> Result<TranscriptPage, Error> {
    let incarnation = incarnation.to_owned();
    let cursor = TranscriptCursor {
        source_id: source.id.clone(),
        incarnation: incarnation.clone(),
        offset: start,
        record_offset: progress.record_offset,
        record_index: progress.record_index,
        prefix_sha256: hash_string(&progress.hash),
    };
    let to_read = (end - start) as usize;
    let mut bytes = vec![0; to_read];
    file.read_exact(&mut bytes)
        .map_err(|_| Error::SourceChanged)?;
    let mut page = TranscriptPage {
        source_id: source.id.clone(),
        incarnation: incarnation.clone(),
        snapshot_bytes: len,
        chunks: Vec::new(),
        next: cursor.clone(),
        has_more: false,
        pending_line: false,
        notices: Vec::new(),
        previous: (backward && start > 0).then_some(start),
    };
    let mut position = cursor.offset;
    let mut consumed = 0;
    let mut readable_budget = 1024usize;
    for chunk in bytes
        .split_inclusive(|b| *b == b'\n')
        .flat_map(|line| line.chunks(MAX_CHUNK_BYTES))
        .take(limits.chunks)
    {
        let end = position + chunk.len() as u64;
        let complete = chunk.ends_with(b"\n");
        let record_offset = progress.record_offset;
        let oversized = end - record_offset > confined::MAX_PARSE_BYTES as u64;
        let mut readable = None;
        if !oversized {
            progress.tail.extend_from_slice(chunk);
        } else {
            progress.tail.clear();
        }
        if complete && !oversized {
            readable = readable_record(&progress.tail);
            if let Some(view) = &mut readable {
                let (text, trimmed) = super::bounded(&view.text, readable_budget);
                readable_budget = readable_budget.saturating_sub(text.len());
                view.text = text;
                view.text_truncated |= trimmed;
            }
        }
        let id = record_id(&source.id, &incarnation, record_offset);
        page.chunks.push(RecordChunk {
            id,
            index: progress.record_index,
            record_offset,
            offset: position,
            end_offset: end,
            raw_base64: STANDARD.encode(chunk),
            complete,
            oversized,
            readable,
        });
        progress.hash.update(chunk);
        if complete {
            progress.record_offset = end;
            progress.record_index += 1;
            progress.tail.clear();
        }
        position = end;
        consumed += chunk.len();
    }
    // Detect a rewrite or truncation during the read, including in a prior
    // chunk of an unfinished record. Reopening catches path replacement.
    let mut current = root
        .open_file(&source.relative)
        .map_err(|_| Error::SourceChanged)?;
    let current_meta = current.metadata().map_err(|_| Error::SourceChanged)?;
    if confined::incarnation(&current_meta) != incarnation || current_meta.len() < position {
        return Err(Error::SourceChanged);
    }
    let verified = prefix(&mut current, &incarnation, &current_meta, position)?;
    if hash_string(&verified.hash) != hash_string(&progress.hash) {
        return Err(Error::SourceChanged);
    }
    page.next = TranscriptCursor {
        source_id: source.id.clone(),
        incarnation,
        offset: position,
        record_offset: progress.record_offset,
        record_index: progress.record_index,
        prefix_sha256: hash_string(&progress.hash),
    };
    page.has_more = consumed < bytes.len() || position < len;
    page.pending_line = progress.record_offset < position;
    if page.pending_line && !page.has_more {
        page.notices.push(Notice {
            code: "pending_record_tail".into(),
            source_id: Some(page.source_id.clone()),
        });
    }
    if encoded_len(&page)? > limits.response_bytes {
        for chunk in &mut page.chunks {
            chunk.readable = None;
        }
        page.notices.push(Notice {
            code: "readable_projection_deferred".into(),
            source_id: Some(page.source_id.clone()),
        });
    }
    if encoded_len(&page)? > limits.response_bytes {
        return Err(Error::ResourceLimit);
    }
    Ok(page)
}
