//! An executor's native stream, read while the session runs: bounded
//! records, explicit gaps, and normalized events as they arrive.
//!
//! [`crate::stream::normalize`] reads a whole stream after the session
//! ends. A host that watches a running session gets the stream in chunks
//! instead, from [`supervise::Live::take`], and can't hold an unbounded
//! line or an unbounded backlog while it waits. [`Reader`] splits chunks
//! into records, one per line, with three rules:
//!
//! - A record longer than its cap is not kept. Its bytes are skipped to
//!   the next line end and recorded as a [`Gap`] with the cause
//!   `record_too_long`.
//! - Bytes the supervisor dropped because the host fell behind are a gap
//!   with the cause `dropped`. The record they interrupted and the partial
//!   record after them can't be read, so the gap runs from the start of
//!   the interrupted record to the next line end.
//! - An unterminated last line is a record when the stream ends, unless it
//!   was being skipped, in which case it is a gap with the cause
//!   `unterminated`.
//!
//! Every record keeps its line number and byte offset, so a normalized
//! event names exactly where in the native stream it came from, and a gap
//! says exactly what the host never saw. [`Normalizer`] turns records into
//! [`Event`]s as they complete.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::stream::{self, Event, Format};

/// The longest record a reader keeps: 4 MiB.
pub const MAX_RECORD: usize = 4 * 1024 * 1024;

/// Why part of a stream was not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    /// One line was longer than the record cap.
    RecordTooLong,
    /// The supervisor dropped bytes the host hadn't taken in time.
    Dropped,
    /// The stream ended inside a skipped record.
    Unterminated,
}

/// A span of the native stream the reader did not turn into a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gap {
    /// Where the span starts, in bytes from the start of the stream.
    pub offset: u64,
    pub bytes: u64,
    pub cause: Cause,
    /// The line the span ends, from 1; a skipped line still counts.
    pub line: usize,
}

/// One complete line of the native stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// Its line number, from 1.
    pub line: usize,
    /// The byte offset of its first byte.
    pub offset: u64,
    /// The line without its line end. Invalid UTF-8 is replaced.
    pub text: String,
}

/// Splits a chunked stream into bounded records.
#[derive(Clone, Debug)]
pub struct Reader {
    max_record: usize,
    partial: Vec<u8>,
    /// The stream offset of `partial[0]`, or of the next byte when empty.
    partial_offset: u64,
    /// While a record is being skipped: where it started, and why.
    skipping: Option<(u64, Cause)>,
    /// The next stream offset the reader expects.
    next: u64,
    lines: usize,
    gaps: Vec<Gap>,
}

impl Default for Reader {
    fn default() -> Self {
        Self::new(MAX_RECORD)
    }
}

impl Reader {
    /// A reader that keeps records up to `max_record` bytes.
    #[must_use]
    pub fn new(max_record: usize) -> Self {
        Reader {
            max_record: max_record.max(1),
            partial: Vec::new(),
            partial_offset: 0,
            skipping: None,
            next: 0,
            lines: 0,
            gaps: Vec::new(),
        }
    }

    /// Every gap so far, in stream order.
    #[must_use]
    pub fn gaps(&self) -> &[Gap] {
        &self.gaps
    }

    /// The bytes the stream has produced so far, read or not.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.next
    }

    /// Lines completed so far, skipped ones included.
    #[must_use]
    pub fn lines(&self) -> usize {
        self.lines
    }

    /// Reads `bytes`, which start at stream offset `offset`, and returns
    /// the records they complete. Bytes before the reader's position are
    /// already read and ignored; a jump past it is a dropped span.
    pub fn feed(&mut self, offset: u64, bytes: &[u8]) -> Vec<Record> {
        if offset > self.next {
            self.dropped(self.next, offset - self.next);
        }
        let skip = usize::try_from(self.next.saturating_sub(offset)).unwrap_or(usize::MAX);
        if skip >= bytes.len() {
            return Vec::new();
        }
        let bytes = &bytes[skip..];
        let start = self.next;
        self.next += bytes.len() as u64;
        let mut records = Vec::new();
        let mut from = 0usize;
        while from < bytes.len() {
            let end = bytes[from..].iter().position(|byte| *byte == b'\n');
            let (segment, newline) = match end {
                Some(i) => (&bytes[from..from + i], true),
                None => (&bytes[from..], false),
            };
            let segment_end = start + (from + segment.len()) as u64;
            if self.skipping.is_none() {
                if self.partial.is_empty() {
                    self.partial_offset = start + from as u64;
                }
                if self.partial.len() + segment.len() > self.max_record {
                    self.skipping = Some((self.partial_offset, Cause::RecordTooLong));
                    self.partial.clear();
                } else {
                    self.partial.extend_from_slice(segment);
                }
            }
            if newline {
                self.lines += 1;
                match self.skipping.take() {
                    Some((at, cause)) => self.gaps.push(Gap {
                        offset: at,
                        bytes: segment_end + 1 - at,
                        cause,
                        line: self.lines,
                    }),
                    None => {
                        let mut text = String::from_utf8_lossy(&self.partial).into_owned();
                        if text.ends_with('\r') {
                            text.pop();
                        }
                        records.push(Record {
                            line: self.lines,
                            offset: self.partial_offset,
                            text,
                        });
                    }
                }
                self.partial.clear();
                self.partial_offset = segment_end + 1;
                from += segment.len() + 1;
            } else {
                from = bytes.len();
            }
        }
        records
    }

    /// Records that the supervisor dropped `bytes` bytes at `offset`. The
    /// record in progress and the partial record after the span are
    /// skipped with it.
    pub fn dropped(&mut self, offset: u64, bytes: u64) {
        let at = match self.skipping {
            Some((at, _)) => at,
            None if !self.partial.is_empty() => self.partial_offset,
            None => offset,
        };
        self.partial.clear();
        self.skipping = Some((at, Cause::Dropped));
        self.next = self.next.max(offset + bytes);
    }

    /// Ends the stream: an unterminated last line becomes a record, or a
    /// gap when it was being skipped.
    pub fn finish(&mut self) -> Option<Record> {
        if let Some((at, _)) = self.skipping.take() {
            self.lines += 1;
            self.gaps.push(Gap {
                offset: at,
                bytes: self.next - at,
                cause: Cause::Unterminated,
                line: self.lines,
            });
            return None;
        }
        if self.partial.is_empty() {
            return None;
        }
        self.lines += 1;
        let record = Record {
            line: self.lines,
            offset: self.partial_offset,
            text: String::from_utf8_lossy(&self.partial).into_owned(),
        };
        self.partial.clear();
        self.partial_offset = self.next;
        Some(record)
    }
}

/// Reads a chunked native stream into normalized events as they arrive.
#[derive(Clone, Debug, Default)]
pub struct Normalizer {
    pub reader: Reader,
    /// The stream's format, once known: given, or detected from the first
    /// line that says.
    pub format: Option<Format>,
    seq: u64,
}

impl Normalizer {
    /// A normalizer for a stream of a known format.
    #[must_use]
    pub fn of(format: Format) -> Self {
        Normalizer {
            format: Some(format),
            ..Normalizer::default()
        }
    }

    /// The last sequence number handed out.
    #[must_use]
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// Reads a chunk, calls `on_line` with each complete record's text,
    /// and returns the events the records hold.
    pub fn feed(&mut self, offset: u64, bytes: &[u8], on_line: &mut dyn FnMut(&str)) -> Vec<Event> {
        let records = self.reader.feed(offset, bytes);
        self.events(records, on_line)
    }

    /// Ends the stream, reading an unterminated last line.
    pub fn finish(&mut self, on_line: &mut dyn FnMut(&str)) -> Vec<Event> {
        let records: Vec<Record> = self.reader.finish().into_iter().collect();
        self.events(records, on_line)
    }

    fn events(&mut self, records: Vec<Record>, on_line: &mut dyn FnMut(&str)) -> Vec<Event> {
        let mut events = Vec::new();
        for record in records {
            on_line(&record.text);
            if self.format.is_none() {
                self.format = detect_line(&record.text);
            }
            let Some(format) = self.format else {
                continue;
            };
            events.extend(
                stream::normalize_line(format, &record.text, record.line, &mut self.seq)
                    .into_iter()
                    .map(|mut event| {
                        event.offset = Some(record.offset);
                        event
                    }),
            );
        }
        events
    }
}

/// The format one line says it is, when it says.
fn detect_line(line: &str) -> Option<Format> {
    serde_json::from_str::<Value>(line).ok()?;
    Format::detect(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn retained(path: &str) -> Option<String> {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../bench/terminal-bench/traces")
                .join(path),
        )
        .ok()
    }

    /// A Claude Code stream and a Codex stream from retained trials.
    fn fixtures() -> Vec<(Format, String)> {
        [
            (
                Format::Claude,
                "panel--coder-one-jevprobe3-opus-lean-low--headless-terminal/headless-terminal__iwCZshP.episode/artifacts/delegate-1.stream.jsonl",
            ),
            (
                Format::Codex,
                "panel--coder-one-jevprobe3-luna--fix-git/fix-git__c2MR9Lx.episode/artifacts/delegate-1.stream.jsonl",
            ),
        ]
        .into_iter()
        .filter_map(|(format, path)| retained(path).map(|text| (format, text)))
        .collect()
    }

    #[test]
    fn chunks_of_any_size_read_the_same_events_as_the_whole_stream() {
        let fixtures = fixtures();
        assert!(!fixtures.is_empty(), "the retained streams are checked in");
        for (format, text) in fixtures {
            let whole = stream::normalize(format, &text);
            assert!(whole.len() > 5, "{} has events", format.word());
            for size in [1usize, 7, 64, 4096, text.len()] {
                let mut normalizer = Normalizer::default();
                let mut events = Vec::new();
                let mut lines = 0;
                for (i, chunk) in text.as_bytes().chunks(size).enumerate() {
                    events.extend(normalizer.feed((i * size) as u64, chunk, &mut |_| lines += 1));
                }
                events.extend(normalizer.finish(&mut |_| lines += 1));
                assert_eq!(normalizer.format, Some(format));
                assert_eq!(events.len(), whole.len(), "chunk size {size}");
                for (got, want) in events.iter().zip(&whole) {
                    assert_eq!(got.kind, want.kind);
                    assert_eq!(got.seq, want.seq);
                    assert_eq!(got.line, want.line);
                    let offset = usize::try_from(got.offset.unwrap()).unwrap();
                    let line = text[offset..].lines().next().unwrap();
                    assert_eq!(Some(line), text.lines().nth(got.line - 1));
                }
                assert_eq!(lines, text.lines().count());
                assert!(normalizer.reader.gaps().is_empty());
            }
        }
    }

    #[test]
    fn a_record_past_the_cap_is_a_gap_and_reading_resumes_after_it() {
        let text = "{\"type\":\"turn.started\"}\n{\"long\":\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"}\n{\"type\":\"turn.completed\",\"usage\":{}}\n";
        let mut reader = Reader::new(40);
        let mut records = Vec::new();
        for (i, chunk) in text.as_bytes().chunks(5).enumerate() {
            records.extend(reader.feed((i * 5) as u64, chunk));
        }
        assert_eq!(records.len(), 2);
        assert_eq!(records[1].line, 3);
        assert_eq!(
            records[1].text,
            "{\"type\":\"turn.completed\",\"usage\":{}}"
        );
        let gap = reader.gaps()[0];
        assert_eq!(gap.cause, Cause::RecordTooLong);
        assert_eq!(gap.line, 2);
        assert_eq!(gap.offset, 24);
        let long = text.lines().nth(1).unwrap().len() as u64 + 1;
        assert_eq!(gap.bytes, long);
        assert_eq!(records[1].offset, 24 + long);
    }

    #[test]
    fn dropped_bytes_are_a_gap_from_the_interrupted_record_to_the_next_line_end() {
        let mut reader = Reader::new(1024);
        let first = reader.feed(0, b"one\ntw");
        assert_eq!(first.len(), 1);
        // Bytes 6 to 15 never arrived; the reader resumes at 16.
        let rest = reader.feed(16, b"o-tail\nfour\n");
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].text, "four");
        let gap = reader.gaps()[0];
        assert_eq!(gap.cause, Cause::Dropped);
        assert_eq!(gap.offset, 4);
        assert_eq!(gap.bytes, 23 - 4);
        assert_eq!(rest[0].offset, 23);
        assert_eq!(reader.bytes(), 28);
    }

    #[test]
    fn an_unterminated_last_line_is_a_record_at_the_end() {
        let mut reader = Reader::new(1024);
        assert!(reader.feed(0, b"a\nb").len() == 1);
        let last = reader.finish().unwrap();
        assert_eq!(last.text, "b");
        assert_eq!(last.line, 2);
        assert_eq!(last.offset, 2);
        let mut skipped = Reader::new(2);
        let _ = skipped.feed(0, b"abcdef");
        assert!(skipped.finish().is_none());
        assert_eq!(skipped.gaps()[0].cause, Cause::Unterminated);
        assert_eq!(skipped.gaps()[0].bytes, 6);
    }

    #[test]
    fn a_repeated_chunk_is_read_once() {
        let mut reader = Reader::new(1024);
        assert_eq!(reader.feed(0, b"a\nb\n").len(), 2);
        assert_eq!(reader.feed(0, b"a\nb\nc\n").len(), 1);
        assert_eq!(reader.lines(), 3);
    }
}
