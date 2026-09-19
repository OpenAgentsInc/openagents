use std::{fmt, io::BufRead};

use serde::{Deserialize, Serialize};

use crate::{
    domain::{Event, Tag},
    store::{AdmissionOutcome, AdmissionRejection, Store, StoreError},
};

pub const MAX_JSONL_EVENT_BYTES: usize = 131_072;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct BulkImportReport {
    pub input_lines: usize,
    pub stored: usize,
    pub duplicate: usize,
    pub already_removed: usize,
    pub ephemeral: usize,
    pub expired: usize,
}

#[derive(Debug)]
pub enum BulkImportError {
    Io(std::io::Error),
    InvalidLine { line: usize, reason: String },
    Rejected { line: usize, code: &'static str },
    Store { line: usize, source: StoreError },
}

impl fmt::Display for BulkImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "JSONL input failed: {error}"),
            Self::InvalidLine { line, reason } => {
                write!(formatter, "JSONL line {line} is invalid: {reason}")
            }
            Self::Rejected { line, code } => {
                write!(
                    formatter,
                    "JSONL line {line} was rejected by policy: {code}"
                )
            }
            Self::Store { line, source } => {
                write!(formatter, "JSONL line {line} failed admission: {source}")
            }
        }
    }
}

impl std::error::Error for BulkImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Store { source, .. } => Some(source),
            Self::InvalidLine { .. } | Self::Rejected { .. } => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictEvent {
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u16,
    tags: Vec<Tag>,
    content: String,
    sig: String,
}

impl From<StrictEvent> for Event {
    fn from(event: StrictEvent) -> Self {
        Self {
            id: event.id,
            pubkey: event.pubkey,
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags,
            content: event.content,
            sig: event.sig,
        }
    }
}

pub async fn import_jsonl(
    mut reader: impl BufRead,
    store: &mut Store,
    now: u64,
) -> Result<BulkImportReport, BulkImportError> {
    let mut report = BulkImportReport::default();
    loop {
        let Some(bytes) = read_line(&mut reader, report.input_lines.saturating_add(1))? else {
            break;
        };
        report.input_lines = report.input_lines.saturating_add(1);
        let strict = serde_json::from_slice::<StrictEvent>(&bytes).map_err(|error| {
            BulkImportError::InvalidLine {
                line: report.input_lines,
                reason: error.to_string(),
            }
        })?;
        let event = Event::from(strict);
        event
            .validate_nip01_structure()
            .and_then(|()| event.validate_crypto())
            .map_err(|error| BulkImportError::InvalidLine {
                line: report.input_lines,
                reason: error.to_string(),
            })?;
        if event.is_expired(now) {
            report.expired = report.expired.saturating_add(1);
            continue;
        }
        let outcome = store
            .admit_historical(&event, now)
            .await
            .map_err(|source| BulkImportError::Store {
                line: report.input_lines,
                source,
            })?;
        match outcome {
            AdmissionOutcome::Stored { .. } => report.stored = report.stored.saturating_add(1),
            AdmissionOutcome::Duplicate => report.duplicate = report.duplicate.saturating_add(1),
            AdmissionOutcome::Ephemeral => report.ephemeral = report.ephemeral.saturating_add(1),
            AdmissionOutcome::Rejected(
                AdmissionRejection::Deleted | AdmissionRejection::Superseded,
            ) => report.already_removed = report.already_removed.saturating_add(1),
            AdmissionOutcome::Rejected(rejection) => {
                return Err(BulkImportError::Rejected {
                    line: report.input_lines,
                    code: rejection.code(),
                });
            }
        }
    }
    Ok(report)
}

fn read_line(reader: &mut impl BufRead, line: usize) -> Result<Option<Vec<u8>>, BulkImportError> {
    let mut bytes = Vec::new();
    let mut read_any = false;
    loop {
        let available = reader.fill_buf().map_err(BulkImportError::Io)?;
        if available.is_empty() {
            break;
        }
        read_any = true;
        let newline = available.iter().position(|byte| *byte == b'\n');
        let wanted = newline.map_or(available.len(), |position| position.saturating_add(1));
        let remaining = MAX_JSONL_EVENT_BYTES
            .saturating_add(1)
            .saturating_sub(bytes.len());
        if remaining == 0 {
            return Err(oversized_line(line, bytes.len().saturating_add(1)));
        }
        let consumed = wanted.min(remaining);
        bytes.extend_from_slice(&available[..consumed]);
        reader.consume(consumed);
        if newline.is_some_and(|position| consumed == position.saturating_add(1)) {
            break;
        }
        if bytes.len() > MAX_JSONL_EVENT_BYTES {
            return Err(oversized_line(line, bytes.len()));
        }
    }
    if !read_any {
        return Ok(None);
    }
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    if bytes.is_empty() {
        return Err(BulkImportError::InvalidLine {
            line,
            reason: "blank lines are not allowed".to_owned(),
        });
    }
    if bytes.len() > MAX_JSONL_EVENT_BYTES {
        return Err(oversized_line(line, bytes.len()));
    }
    Ok(Some(bytes))
}

fn oversized_line(line: usize, bytes: usize) -> BulkImportError {
    BulkImportError::InvalidLine {
        line,
        reason: format!("event is {bytes} bytes; maximum is {MAX_JSONL_EVENT_BYTES}"),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Fixture {
        schema: String,
        maximum_line_bytes: usize,
        known_jsonl_events: Vec<serde_json::Value>,
        invalid_lines: Vec<InvalidLine>,
    }

    #[derive(Deserialize)]
    struct InvalidLine {
        name: String,
        raw: String,
    }

    #[test]
    fn fixture_pins_strict_jsonl_records_and_refusals() {
        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../tests/fixtures/migration/signed-event-import-v1.json"
        ))
        .expect("bulk import fixture parses");
        assert_eq!(
            fixture.schema,
            "openagents.nostr-relay.signed-event-import.v1"
        );
        assert_eq!(fixture.maximum_line_bytes, MAX_JSONL_EVENT_BYTES);
        assert_eq!(fixture.known_jsonl_events.len(), 2);
        for value in fixture.known_jsonl_events {
            let bytes = serde_json::to_vec(&value).expect("fixture event serializes");
            let strict = serde_json::from_slice::<StrictEvent>(&bytes)
                .expect("known fixture event is strict");
            let event = Event::from(strict);
            event
                .validate_nip01_structure()
                .expect("known fixture event has NIP-01 structure");
            event
                .validate_crypto()
                .expect("known fixture event is signed");
        }
        for invalid in fixture.invalid_lines {
            let error = serde_json::from_slice::<StrictEvent>(invalid.raw.as_bytes())
                .expect_err("invalid JSONL fixture must be refused");
            assert!(
                !error.to_string().is_empty(),
                "{} must retain a parse reason",
                invalid.name
            );
        }
    }

    #[test]
    fn line_reader_is_bounded_and_rejects_blanks() {
        let blank = read_line(&mut Cursor::new(b"\n"), 1).expect_err("blank line must fail");
        assert!(blank.to_string().contains("blank"));
        let oversized = vec![b'x'; MAX_JSONL_EVENT_BYTES + 1];
        let error = read_line(&mut Cursor::new(oversized), 1).expect_err("oversize must fail");
        assert!(error.to_string().contains("maximum"));
    }
}
