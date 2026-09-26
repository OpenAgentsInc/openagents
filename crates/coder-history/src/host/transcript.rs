use super::{History, catalog, confined, digest, encoded_len};
use crate::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

const MAX_CHUNKS: usize = 128;

struct Prefix {
    hash: Sha256,
    record_offset: u64,
    record_index: u64,
    tail: Vec<u8>,
}

fn prefix(file: &mut File, through: u64) -> Result<Prefix, Error> {
    file.seek(SeekFrom::Start(0))
        .map_err(|_| Error::SourceUnreadable)?;
    let mut out = Prefix {
        hash: Sha256::new(),
        record_offset: 0,
        record_index: 0,
        tail: Vec::new(),
    };
    let mut position = 0;
    let mut buffer = [0u8; 64 * 1024];
    while position < through {
        let count = usize::try_from((through - position).min(buffer.len() as u64))
            .map_err(|_| Error::ResourceLimit)?;
        file.read_exact(&mut buffer[..count])
            .map_err(|_| Error::SourceChanged)?;
        out.hash.update(&buffer[..count]);
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
            }
        }
        position += count as u64;
    }
    Ok(out)
}

fn hash_string(hash: &Sha256) -> String {
    hash.clone()
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn page(history: &History, request: TranscriptRequest) -> Result<TranscriptPage, Error> {
    if request.max_bytes == 0
        || request.max_bytes > MAX_PAGE_BYTES
        || request.source_id.len() != 64
        || !request.source_id.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(Error::InvalidRequest);
    }
    let (sources, _) = catalog::scan(history)?;
    let source = sources
        .into_iter()
        .find(|s| s.id == request.source_id)
        .ok_or(Error::SourceMissing)?;
    let root = &history.roots[source.root];
    let mut file = root.open_file(&source.relative)?;
    let meta = file.metadata().map_err(|_| Error::SourceUnreadable)?;
    if meta.len() > confined::MAX_SOURCE_BYTES {
        return Err(Error::ResourceLimit);
    }
    let incarnation = confined::incarnation(&meta);
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
    let mut progress = prefix(&mut file, cursor.offset)?;
    if cursor.record_offset != progress.record_offset
        || cursor.record_index != progress.record_index
        || cursor.prefix_sha256 != hash_string(&progress.hash)
    {
        return Err(Error::SourceChanged);
    }
    let to_read = u64::from(request.max_bytes).min(meta.len() - cursor.offset) as usize;
    let mut bytes = vec![0; to_read];
    file.read_exact(&mut bytes)
        .map_err(|_| Error::SourceChanged)?;
    let mut page = TranscriptPage {
        source_id: source.id.clone(),
        incarnation: incarnation.clone(),
        snapshot_bytes: meta.len(),
        chunks: Vec::new(),
        next: cursor.clone(),
        has_more: false,
        pending_line: false,
        notices: Vec::new(),
    };
    let mut position = cursor.offset;
    let mut consumed = 0;
    let mut readable_budget = 1024usize;
    for chunk in bytes
        .split_inclusive(|b| *b == b'\n')
        .flat_map(|line| line.chunks(MAX_CHUNK_BYTES))
        .take(MAX_CHUNKS)
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
    let verified = prefix(&mut current, position)?;
    if hash_string(&verified.hash) != hash_string(&progress.hash) {
        return Err(Error::SourceChanged);
    }
    page.next = TranscriptCursor {
        source_id: source.id,
        incarnation,
        offset: position,
        record_offset: progress.record_offset,
        record_index: progress.record_index,
        prefix_sha256: hash_string(&progress.hash),
    };
    page.has_more = consumed < bytes.len() || position < meta.len();
    page.pending_line = progress.record_offset < position;
    if page.pending_line && !page.has_more {
        page.notices.push(Notice {
            code: "pending_record_tail".into(),
            source_id: Some(page.source_id.clone()),
        });
    }
    if encoded_len(&page)? > MAX_RESPONSE_BYTES {
        for chunk in &mut page.chunks {
            chunk.readable = None;
        }
        page.notices.push(Notice {
            code: "readable_projection_deferred".into(),
            source_id: Some(page.source_id.clone()),
        });
    }
    if encoded_len(&page)? > MAX_RESPONSE_BYTES {
        return Err(Error::ResourceLimit);
    }
    Ok(page)
}
