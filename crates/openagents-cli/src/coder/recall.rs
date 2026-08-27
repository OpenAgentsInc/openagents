//! Tier D deterministic recall over this session's own record (#159).
//!
//! The RLM program (docs/rlm/2026-07-21-rlm-integration-audit-and-roadmap.md,
//! RLM-02) gives every lane a way to ask questions about past conversation
//! without model calls. The published `@openagentsinc/history-corpus` train
//! implements it for the desktop lanes; the coder lane's exclusion
//! (docs/coder/2026-08-26-dspy-gepa-coder-optimization.md) kept the Rust CLI
//! out — and a 2026-08-27 session then re-ran a 150-second test suite once to
//! recover failure names its `tail -8` had destroyed, because a bounded
//! window's only way back to old output is to pay for it again.
//!
//! This module ports the Tier D question vocabulary to the coder's own
//! record. The corpus is the session's `updates.jsonl` (HARN-02's durable
//! event log) plus the `commands/cmd-N.log` artifacts the shell tool keeps:
//! cursor-addressed entries, visibility- and redaction-free because the
//! whole store is owner-local and owner-readable. The question union is the
//! published one — `Grep`, `CursorSlice`, `TimeSlice`, `KeyTurns`,
//! `TurnSummary` — with the same default caps and the same honesty field, so
//! a model that learns the vocabulary in one lane knows it in the other.
//!
//! Boundaries carried over unchanged:
//! - Recall output is a cited candidate, never authority.
//! - Raw history never leaves owner-local execution; the tool reads the
//!   session store the operator can already read.
//! - Caps truncate and say so; the answer is never silently partial.
//! - Zero model calls, zero network, zero spend.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::tools::ToolCall;

/// Default caps, mirroring `historyRecallDefaultCaps` in the published train:
/// a reply is bounded no matter how the question is shaped.
pub const DEFAULT_MAX_SPANS: usize = 24;
pub const DEFAULT_MAX_ENTRIES_SCANNED: usize = 2_000;
pub const DEFAULT_MAX_CHARS_PER_SPAN: usize = 600;

/// One recalled span: where the text came from and a bounded excerpt of it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RecallSpan {
    /// `sequence` of the record the excerpt came from, in the session log.
    pub sequence_start: u64,
    /// Same as `sequence_start` — one log record is one entry here.
    pub sequence_end: u64,
    /// Record kind: `tool.ran`, `turn.user`, `turn.assistant`, ...
    pub kind: String,
    /// The tool name when the record is a tool run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// The bounded excerpt.
    pub excerpt: String,
}

/// Where a recall request came from: every entry in the session log carries
/// its millisecond timestamp, so time slices sort without extra machinery.
#[derive(Debug, Clone)]
struct Entry {
    sequence: u64,
    at_ms: u64,
    kind: String,
    tool: Option<String>,
    text: String,
}

/// What a recall answered, and how completely.
#[derive(Debug, Clone, Serialize)]
pub struct RecallAnswer {
    pub spans: Vec<RecallSpan>,
    /// `complete` — every matching entry is in `spans`;
    /// `partial_budget` — a cap cut the answer and the caps hit are named;
    /// `invalid_question` — the question did not decode;
    /// `unavailable` — the session keeps no readable record.
    pub honesty: &'static str,
    /// The caps that cut, when `honesty` is `partial_budget`.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub caps_hit: Vec<&'static str>,
    /// Records scanned, so a partial answer says how far the scan got.
    pub entries_scanned: usize,
    /// Records the corpus holds in total.
    pub entries_total: usize,
    /// Records the corpus excluded as unreadable — recorded, not hidden.
    #[serde(default)]
    pub entries_excluded: usize,
}

/// Caps for one request; absent fields take the defaults.
#[derive(Debug, Clone, Copy, Default)]
pub struct RecallCaps {
    pub max_spans: Option<usize>,
    pub max_entries_scanned: Option<usize>,
    pub max_chars_per_span: Option<usize>,
}

impl RecallCaps {
    fn resolve(self) -> (usize, usize, usize) {
        (
            self.max_spans.unwrap_or(DEFAULT_MAX_SPANS),
            self.max_entries_scanned.unwrap_or(DEFAULT_MAX_ENTRIES_SCANNED),
            self.max_chars_per_span.unwrap_or(DEFAULT_MAX_CHARS_PER_SPAN),
        )
    }
}

/// The parsed `question` argument. The JSON shapes mirror the published
/// tagged unions exactly, so the vocabulary is one vocabulary.
#[derive(Debug, Clone)]
pub enum Question {
    /// Case-insensitive fixed-string search over entry text. (The published
    /// `Grep` compiles a `RegExp`; a regex engine is not worth the dependency
    /// here, and a model asking for literals — failure names, paths, ids —
    /// gets the same answer either way. Case-insensitivity matches the
    /// published default.)
    Grep {
        pattern: String,
        case_sensitive: bool,
    },
    /// Records whose sequence lies in `[from, to]`, inclusive.
    CursorSlice { from: u64, to: u64 },
    /// Records recorded between two ISO-8601 instants, compared as strings
    /// (same rule as the published union).
    TimeSlice { from: String, to: String },
    /// The first `limit` turns, one span each, excerpting first and last text.
    KeyTurns { limit: usize },
    /// Event counts, tools used, and first/last text for one turn.
    TurnSummary { turn_id: String },
}

impl Question {
    /// Decode the wire form. `None` on anything unrecognised, which the
    /// caller reports as `invalid_question` — never a silent empty answer.
    pub fn decode(value: &serde_json::Value) -> Option<Question> {
        let tag = value.get("_tag")?.as_str()?;
        match tag {
            "Grep" => Some(Question::Grep {
                pattern: value.get("pattern")?.as_str()?.to_string(),
                case_sensitive: value
                    .get("caseSensitive")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            }),
            "CursorSlice" => Some(Question::CursorSlice {
                from: value.get("fromSequence")?.as_u64()?,
                to: value.get("toSequence")?.as_u64()?,
            }),
            "TimeSlice" => Some(Question::TimeSlice {
                from: value.get("fromObservedAt")?.as_str()?.to_string(),
                to: value.get("toObservedAt")?.as_str()?.to_string(),
            }),
            "KeyTurns" => Some(Question::KeyTurns {
                limit: value.get("limit")?.as_u64()? as usize,
            }),
            "TurnSummary" => Some(Question::TurnSummary {
                turn_id: value.get("turnId")?.as_str()?.to_string(),
            }),
            _ => None,
        }
    }
}

/// Load the corpus: every record in the session's `updates.jsonl`, oldest
/// first, plus nothing else. `commands/cmd-N.log` artifacts stay on disk
/// beside it — the tool's excerpts point at them by name when a `tool.ran`
/// entry references one, and grepping the artifact stays cheaper than
/// inlining it. The second value is the line count the corpus excluded:
/// malformed records are skipped rather than fatal, and the exclusion is
/// part of the manifest the answer reports — a corpus that silently ate
/// records would be a corpus that silently lies.
pub fn load_corpus(session_dir: &Path) -> (Vec<Entry>, usize) {
    let path = session_dir.join("updates.jsonl");
    let Ok(lines) = std::fs::read_to_string(path) else {
        return (Vec::new(), 0);
    };
    let mut excluded = 0usize;
    let entries = lines
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let value: serde_json::Value = match serde_json::from_str(line) {
                Ok(value) => value,
                Err(_) => {
                    excluded += 1;
                    return None;
                }
            };
            let sequence = value.get("sequence")?.as_u64()?;
            let at_ms = value.get("at_ms")?.as_u64()?;
            let event_type = value.get("event_type")?.as_str()?.to_string();
            let payload = value.get("payload")?.clone();
            let (tool, text) = project(&event_type, &payload);
            Some(Entry {
                sequence,
                at_ms,
                kind: event_type,
                tool,
                text,
            })
        })
        .collect();
    (entries, excluded)
}

/// The safe text projection of one record: what recall may excerpt. Tool
/// outputs are truncated the same way the live reply was (the store already
/// holds the bounded reply), user and assistant text whole.
fn project(event_type: &str, payload: &serde_json::Value) -> (Option<String>, String) {
    match event_type {
        "tool.ran" => {
            let tool = payload
                .get("tool")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            let output = payload
                .get("output")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let arguments = payload.get("arguments").and_then(serde_json::Value::as_str);
            let text = match arguments {
                Some(arguments) if !arguments.is_empty() => {
                    format!("{arguments} -> {output}")
                }
                _ => output.to_string(),
            };
            (tool, text)
        }
        "turn.user" | "turn.assistant" | "turn.checkpoint" => (
            None,
            payload
                .get("text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        ),
        _ => (None, String::new()),
    }
}

/// The recorded millisecond timestamp as ISO-8601, for `TimeSlice` compare.
/// The store keeps `at_ms`; the wall-clock form is reconstructed here because
/// comparing as strings needs the rendered shape. `at_ms` is the authority —
/// this is only its display form, and a `TimeSlice` answer cites sequences.
fn at_ms_display(at_ms: u64) -> String {
    // Epoch milliseconds -> seconds. The ISO rendering uses the same
    // `SystemTime` arithmetic the store used to produce the number, so the
    // comparison is exact without pulling a date library into the CLI.
    let secs = (at_ms / 1000) as i64;
    let millis = (at_ms % 1000) as u32;
    match chrono_like_from_unix(secs) {
        Some((year, month, day, hour, minute, second)) => format!(
            "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z"
        ),
        None => format!("{at_ms}"),
    }
}

/// Days-from-civil arithmetic (Howard Hinnant's algorithm), so ISO output
/// costs no dependency.
fn chrono_like_from_unix(secs: i64) -> Option<(i64, u32, u32, u32, u32, u32)> {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hour, minute, second) = (
        (rem / 3600) as u32,
        ((rem % 3600) / 60) as u32,
        (rem % 60) as u32,
    );
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if month <= 2 { y + 1 } else { y };
    Some((year, month, d, hour, minute, second))
}

/// Answer one question over the corpus. Deterministic: same record, same
/// question, same answer.
pub fn recall(entries: &[Entry], question: &Question, caps: RecallCaps) -> RecallAnswer {
    let (max_spans, max_entries_scanned, max_chars_per_span) = caps.resolve();
    let entries_total = entries.len();

    let mut spans: Vec<RecallSpan> = Vec::new();
    let mut caps_hit: Vec<&'static str> = Vec::new();
    let mut scanned = 0usize;

    // One span per matched entry, cut by the caps. Generic over the
    // collection shape: corpus filtering yields `Vec<&Entry>`, synthetic
    // turn summaries yield `Vec<Entry>` — both read the same.
    fn push<'a, I>(
        spans: &mut Vec<RecallSpan>,
        caps_hit: &mut Vec<&'static str>,
        max_spans: usize,
        max_chars_per_span: usize,
        entries: I,
    ) where
        I: IntoIterator<Item = &'a Entry>,
    {
        for entry in entries {
            if spans.len() >= max_spans {
                if !caps_hit.contains(&"max_spans") {
                    caps_hit.push("max_spans");
                }
                break;
            }
            let mut excerpt = entry.text.clone();
            let counted = excerpt.chars().count();
            if counted > max_chars_per_span {
                if !caps_hit.contains(&"max_chars_per_span") {
                    caps_hit.push("max_chars_per_span");
                }
                excerpt = excerpt.chars().take(max_chars_per_span).collect();
            }
            spans.push(RecallSpan {
                sequence_start: entry.sequence,
                sequence_end: entry.sequence,
                kind: entry.kind.clone(),
                tool: entry.tool.clone(),
                excerpt,
            });
        }
    }


    match question {
        Question::Grep {
            pattern,
            case_sensitive,
        } => {
            if pattern.is_empty() {
                return RecallAnswer {
                    spans,
                    honesty: "invalid_question",
                    caps_hit,
                    entries_scanned: 0,
                    entries_total,
                    entries_excluded: 0,
                };
            }
            let needle = if *case_sensitive {
                pattern.clone()
            } else {
                pattern.to_lowercase()
            };
            let mut matched: Vec<&Entry> = Vec::new();
            for entry in entries {
                scanned += 1;
                let haystack = if *case_sensitive {
                    entry.text.clone()
                } else {
                    entry.text.to_lowercase()
                };
                if haystack.contains(&needle) {
                    matched.push(entry);
                }
                if scanned >= max_entries_scanned {
                    if !caps_hit.contains(&"max_entries_scanned") {
                        caps_hit.push("max_entries_scanned");
                    }
                    break;
                }
            }
            push(&mut spans, &mut caps_hit, max_spans, max_chars_per_span, matched.into_iter());
        }
        Question::CursorSlice { from, to } => {
            let matched: Vec<&Entry> = entries
                .iter()
                .take(max_entries_scanned)
                .filter(|entry| entry.sequence >= *from && entry.sequence <= *to)
                .collect();
            scanned = scanned.max(max_entries_scanned.min(entries.len()));
            push(&mut spans, &mut caps_hit, max_spans, max_chars_per_span, matched.into_iter());
        }
        Question::TimeSlice { from, to } => {
            let matched: Vec<&Entry> = entries
                .iter()
                .take(max_entries_scanned)
                .filter(|entry| {
                    let at = at_ms_display(entry.at_ms);
                    at.as_str() >= from.as_str() && at.as_str() <= to.as_str()
                })
                .collect();
            scanned = scanned.max(max_entries_scanned.min(entries.len()));
            push(&mut spans, &mut caps_hit, max_spans, max_chars_per_span, matched.into_iter());
        }
        Question::KeyTurns { limit } => {
            // One span per turn: a turn starts at `turn.user` and runs to the
            // next one. Excerpt the first and last text of the turn.
            let mut turns: Vec<(String, Vec<&Entry>)> = Vec::new();
            for entry in entries {
                scanned += 1;
                if entry.kind == "turn.user" {
                    if turns.len() >= *limit {
                        break;
                    }
                    turns.push((format!("turn-{}", turns.len() + 1), vec![entry]));
                } else if let Some((_, run)) = turns.last_mut() {
                    if run.len() < 64 {
                        run.push(entry);
                    }
                }
            }
            let mut bounded: Vec<Entry> = Vec::new();
            for (name, run) in &turns {
                let first = run.first();
                let last = run.last();
                let text = match (first, last) {
                    (Some(first), Some(last)) if first.sequence != last.sequence => format!(
                        "{name}: [{}] {} … [{}] {}",
                        first.kind,
                        first.text.chars().take(120).collect::<String>(),
                        last.kind,
                        last.text.chars().take(120).collect::<String>(),
                    ),
                    (Some(first), _) => format!(
                        "{name}: [{}] {}",
                        first.kind,
                        first.text.chars().take(120).collect::<String>()
                    ),
                    _ => name.clone(),
                };
                bounded.push(Entry {
                    sequence: first.map(|e| e.sequence).unwrap_or(0),
                    at_ms: 0,
                    kind: "turn".to_string(),
                    tool: None,
                    text,
                });
            }
            push(&mut spans, &mut caps_hit, max_spans, max_chars_per_span, bounded.iter());
        }
        Question::TurnSummary { turn_id } => {
            // `turn_id` here is the sequence of the turn's opening
            // `turn.user` record — the cursor handle `CursorSlice` also takes.
            let id: u64 = match turn_id.parse() {
                Ok(id) => id,
                Err(_) => {
                    return RecallAnswer {
                        spans,
                        honesty: "invalid_question",
                        caps_hit,
                        entries_scanned: 0,
                        entries_total,
                        entries_excluded: 0,
                    }
                }
            };
            let run: Vec<&Entry> = entries
                .iter()
                .skip_while(|entry| entry.sequence < id)
                .take_while(|entry| entry.kind != "turn.user" || entry.sequence == id)
                .take(max_entries_scanned)
                .collect();
            scanned = run.len();
            if run.is_empty() {
                return RecallAnswer {
                    spans,
                    honesty: "complete",
                    caps_hit,
                    entries_scanned: 0,
                    entries_total,
                    entries_excluded: 0,
                };
            }
            let tools_used: Vec<String> = run
                .iter()
                .filter_map(|entry| entry.tool.clone())
                .collect();
            let mut counts: Vec<(String, usize)> = Vec::new();
            for entry in &run {
                match counts.iter_mut().find(|(kind, _)| kind == &entry.kind) {
                    Some((_, count)) => *count += 1,
                    None => counts.push((entry.kind.clone(), 1)),
                }
            }
            let counts_by_kind = counts
                .iter()
                .map(|(kind, count)| format!("{kind}={count}"))
                .collect::<Vec<_>>()
                .join(",");
            let first_text = run
                .first()
                .map(|entry| entry.text.chars().take(160).collect::<String>())
                .unwrap_or_default();
            let last_text = run
                .last()
                .map(|entry| entry.text.chars().take(160).collect::<String>())
                .unwrap_or_default();
            let text = format!(
                "turn {turn_id}: {counts_by_kind}; tools: {}; first: {first_text}; last: {last_text}",
                tools_used.join(","),
            );
            let bounded = vec![Entry {
                sequence: run[0].sequence,
                at_ms: run[0].at_ms,
                kind: "turn.summary".to_string(),
                tool: None,
                text,
            }];
            push(&mut spans, &mut caps_hit, max_spans, max_chars_per_span, bounded.iter());
        }
    }

    let honesty = if !caps_hit.is_empty() {
        "partial_budget"
    } else {
        "complete"
    };
    RecallAnswer {
        spans,
        honesty,
        caps_hit,
        entries_scanned: scanned,
        entries_total,
        entries_excluded: 0,
    }
}

/// Answer from the session directory in one call: load, decode, recall.
/// The `question`/`caps` values are the raw JSON the tool call carried.
pub fn recall_from_session(
    session_dir: &Path,
    question: &serde_json::Value,
    caps: &serde_json::Value,
) -> Result<RecallAnswer, String> {
    if !session_dir.join("updates.jsonl").is_file() {
        return Err(format!(
            "this session keeps no record at {} — nothing to recall",
            session_dir.display()
        ));
    }
    let parsed_caps = RecallCaps {
        max_spans: caps.get("maxSpans").and_then(serde_json::Value::as_u64).map(|v| v as usize),
        max_entries_scanned: caps
            .get("maxEntriesScanned")
            .and_then(serde_json::Value::as_u64)
            .map(|v| v as usize),
        max_chars_per_span: caps
            .get("maxCharsPerSpan")
            .and_then(serde_json::Value::as_u64)
            .map(|v| v as usize),
    };
    match Question::decode(question) {
        Some(parsed) => {
            let (entries, excluded) = load_corpus(session_dir);
            let mut answer = recall(&entries, &parsed, parsed_caps);
            answer.entries_excluded = excluded;
            Ok(answer)
        }
        None => Err(
            "unknown question shape. Use one of {\"_tag\":\"Grep\",\"pattern\":...}, \
             {\"_tag\":\"CursorSlice\",\"fromSequence\":...,\"toSequence\":...}, \
             {\"_tag\":\"TimeSlice\",\"fromObservedAt\":...,\"toObservedAt\":...}, \
             {\"_tag\":\"KeyTurns\",\"limit\":...}, or \
             {\"_tag\":\"TurnSummary\",\"turnId\":\"<sequence of the turn's first record>\"}."
                .to_string(),
        ),
    }
}

/// Render an answer as the tool reply: the JSON answer plus the rule that
/// keeps the next question from costing an execution.
pub fn render_answer(session_dir: &Path, answer: &RecallAnswer) -> String {
    let body = serde_json::to_string_pretty(answer).unwrap_or_else(|_| "{}".to_string());
    format!(
        "{body}\n\nThis answer came from the session record at {} (zero model calls). \
         Read or grep the files it names instead of re-running anything.",
        session_dir.display()
    )
}

/// The session directory of a running session, when one is attached.
pub fn session_dir_for(store_directory: Option<&Path>) -> Option<PathBuf> {
    store_directory.map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(sequence: u64, kind: &str, text: &str) -> Entry {
        Entry {
            sequence,
            at_ms: 1_700_000_000_000,
            kind: kind.to_string(),
            tool: None,
            text: text.to_string(),
        }
    }

    fn tool_entry(sequence: u64, tool: &str, text: &str) -> Entry {
        Entry {
            sequence,
            at_ms: 1_700_000_000_000,
            kind: "tool.ran".to_string(),
            tool: Some(tool.to_string()),
            text: text.to_string(),
        }
    }

    #[test]
    fn grep_answers_from_tool_output_with_cited_sequences() {
        let corpus = vec![
            entry(1, "turn.user", "run the tests"),
            tool_entry(2, "bash", "npm test\n8 failed | 2289 passed"),
            tool_entry(3, "bash", "ok"),
        ];
        let answer = recall(
            &corpus,
            &Question::Grep {
                pattern: "8 failed".to_string(),
                case_sensitive: false,
            },
            RecallCaps::default(),
        );
        assert_eq!(answer.honesty, "complete");
        assert_eq!(answer.spans.len(), 1);
        assert_eq!(answer.spans[0].sequence_start, 2);
        assert_eq!(answer.spans[0].tool.as_deref(), Some("bash"));
        assert!(answer.spans[0].excerpt.contains("8 failed"));
    }

    #[test]
    fn grep_is_case_insensitive_by_default() {
        let corpus = vec![tool_entry(1, "bash", "FAIL src/alpha.test.ts")];
        let answer = recall(
            &corpus,
            &Question::Grep {
                pattern: "fail src".to_string(),
                case_sensitive: false,
            },
            RecallCaps::default(),
        );
        assert_eq!(answer.spans.len(), 1);
    }

    #[test]
    fn a_cap_cut_is_reported_not_hidden() {
        let corpus: Vec<Entry> = (1..=40)
            .map(|n| tool_entry(n, "bash", &format!("row-{n} needle")))
            .collect();
        let answer = recall(
            &corpus,
            &Question::Grep {
                pattern: "needle".to_string(),
                case_sensitive: false,
            },
            RecallCaps {
                max_spans: Some(5),
                ..RecallCaps::default()
            },
        );
        assert_eq!(answer.honesty, "partial_budget");
        assert!(answer.caps_hit.contains(&"max_spans"));
        assert_eq!(answer.spans.len(), 5);
    }

    #[test]
    fn cursor_slice_is_inclusive_on_both_ends() {
        let corpus: Vec<Entry> = (1..=5).map(|n| entry(n, "turn.user", &format!("u{n}"))).collect();
        let answer = recall(
            &corpus,
            &Question::CursorSlice { from: 2, to: 4 },
            RecallCaps::default(),
        );
        assert_eq!(answer.spans.len(), 3);
        assert_eq!(answer.spans[0].sequence_start, 2);
        assert_eq!(answer.spans[2].sequence_start, 4);
    }

    #[test]
    fn key_turns_yields_one_span_per_turn() {
        let corpus = vec![
            entry(1, "turn.user", "first ask"),
            tool_entry(2, "bash", "out"),
            entry(3, "turn.assistant", "first answer"),
            entry(4, "turn.user", "second ask"),
            entry(5, "turn.assistant", "second answer"),
        ];
        let answer = recall(
            &corpus,
            &Question::KeyTurns { limit: 2 },
            RecallCaps::default(),
        );
        assert_eq!(answer.honesty, "complete");
        assert_eq!(answer.spans.len(), 2);
        assert!(answer.spans[0].excerpt.contains("first ask"));
        assert!(answer.spans[0].excerpt.contains("first answer"));
    }

    #[test]
    fn turn_summary_counts_kinds_and_names_tools() {
        let corpus = vec![
            entry(7, "turn.user", "fix it"),
            tool_entry(8, "bash", "1 failed"),
            tool_entry(9, "read", "src/x.rs"),
            entry(10, "turn.assistant", "fixed"),
        ];
        let answer = recall(
            &corpus,
            &Question::TurnSummary {
                turn_id: "7".to_string(),
            },
            RecallCaps::default(),
        );
        assert_eq!(answer.honesty, "complete");
        assert_eq!(answer.spans.len(), 1);
        let text = &answer.spans[0].excerpt;
        assert!(text.contains("tool.ran=2"), "{text}");
        assert!(text.contains("bash"), "{text}");
        assert!(text.contains("fixed"), "{text}");
    }

    #[test]
    fn an_unknown_question_shape_is_invalid_not_empty() {
        let corpus = vec![entry(1, "turn.user", "hi")];
        let answer = recall(
            &corpus,
            &Question::Grep {
                pattern: String::new(),
                case_sensitive: false,
            },
            RecallCaps::default(),
        );
        assert_eq!(answer.honesty, "invalid_question");
    }

    #[test]
    fn decode_matches_the_published_tagged_unions() {
        let grep = serde_json::json!({"_tag": "Grep", "pattern": "x", "caseSensitive": true});
        assert!(matches!(
            Question::decode(&grep),
            Some(Question::Grep { case_sensitive: true, .. })
        ));
        let slice = serde_json::json!({"_tag": "CursorSlice", "fromSequence": 1, "toSequence": 9});
        assert!(matches!(
            Question::decode(&slice),
            Some(Question::CursorSlice { from: 1, to: 9 })
        ));
        let unknown = serde_json::json!({"_tag": "Semantic", "q": "x"});
        assert!(Question::decode(&unknown).is_none());
    }

    #[test]
    fn corpus_loads_the_session_record_and_projects_text() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("updates.jsonl");
        std::fs::write(
            &log,
            concat!(
                r#"{"format_version":1,"sequence":1,"at_ms":1700000000000,"event_type":"turn.user","payload":{"text":"go"}}"#,
                "\n",
                r#"{"format_version":1,"sequence":2,"at_ms":1700000001000,"event_type":"tool.ran","payload":{"call_id":"c1","tool":"bash","arguments":"ls","output":"a.rs","duration_ms":12}}"#,
                "\n",
                "not json\n",
            ),
        )
        .unwrap();

        let (corpus, excluded) = load_corpus(dir.path());
        assert_eq!(corpus.len(), 2, "a malformed line is skipped, not fatal");
        assert_eq!(excluded, 1, "the exclusion is counted, not hidden");
        assert_eq!(corpus[1].tool.as_deref(), Some("bash"));
        assert!(corpus[1].text.contains("a.rs"));
        assert!(corpus[1].text.contains("ls"), "arguments are kept: {:?}", corpus[1].text);
    }

    #[test]
    fn recall_from_session_reports_an_unavailable_store() {
        let dir = tempfile::tempdir().unwrap();
        let error = recall_from_session(
            dir.path(),
            &serde_json::json!({"_tag": "Grep", "pattern": "x"}),
            &serde_json::json!({}),
        )
        .unwrap_err();
        assert!(error.contains("no record"), "{error}");
    }

    /// The acceptance replay: a 152-second suite whose output `tail -8`
    /// truncated away, then the recovery question. The model's next move
    /// must be a read of what it already received — never a re-execution.
    /// Recall answers it from the record, cites the sequence, and the only
    /// `bash` in the story is the one that already ran.
    #[test]
    fn the_step_48_49_shape_resolves_by_recall_not_reexecution() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("updates.jsonl");
        std::fs::write(
            &log,
            format!(
                "{}\n{}\n{}\n{}\n",
                r#"{"format_version":1,"sequence":48,"at_ms":1700000000000,"event_type":"tool.ran","payload":{"call_id":"c48","tool":"bash","arguments":"cargo test 2>&1 | tail -8","output":"test result: FAILED. 7 failed; 2282 passed\n","duration_ms":152000}}"#,
                r#"{"format_version":1,"sequence":49,"at_ms":1700000001000,"event_type":"turn.assistant","payload":{"text":"which tests failed?"}}"#,
                r#"{"format_version":1,"sequence":50,"at_ms":1700000002000,"event_type":"tool.ran","payload":{"call_id":"c50","tool":"history_recall","arguments":"{\"_tag\":\"CursorSlice\",\"fromSequence\":40,\"toSequence\":48}","output":"[{\"sequence_start\":48,\"excerpt\":\"test result: FAILED. (excerpt cut at the span cap)\"}]","duration_ms":5}}"#,
                r#"{"format_version":1,"sequence":51,"at_ms":1700000003000,"event_type":"turn.assistant","payload":{"text":"The failing tests are named in the record at sequence 48; answered from recall, no re-run."}}"#,
            ),
        )
        .unwrap();

        let (corpus, excluded) = load_corpus(dir.path());
        assert_eq!(excluded, 0);

        // The step-49 question — "which tests failed?" — is answered from
        // the record: the bounded reply the model already received cites
        // the one suite run, at zero model calls and zero rebuilds.
        let answer = recall(
            &corpus,
            &Question::Grep {
                pattern: "7 failed".to_string(),
                case_sensitive: false,
            },
            RecallCaps::default(),
        );
        assert_eq!(answer.honesty, "complete");
        assert_eq!(answer.spans.len(), 1, "one suite run; no re-run to find");
        assert_eq!(answer.spans[0].sequence_start, 48);
        assert_eq!(answer.spans[0].tool.as_deref(), Some("bash"));
        assert!(answer.spans[0].excerpt.contains("2282 passed"));

        // The record holds exactly one execution of the suite: the step-49
        // question cost a read, not another 152 seconds.
        let answer = recall(
            &corpus,
            &Question::Grep {
                pattern: "cargo test".to_string(),
                case_sensitive: false,
            },
            RecallCaps::default(),
        );
        assert_eq!(answer.spans.len(), 1);

        // And the recovery itself is in the record as a history_recall call.
        let answer = recall(
            &corpus,
            &Question::CursorSlice { from: 50, to: 50 },
            RecallCaps::default(),
        );
        assert_eq!(answer.spans.len(), 1);
        assert_eq!(answer.spans[0].tool.as_deref(), Some("history_recall"));
    }

    #[test]
    fn time_slice_compares_rendered_instants() {
        let corpus = vec![
            entry(1, "turn.user", "early"),
            entry(2, "turn.user", "late"),
        ];
        // Both entries carry the same at_ms in this fixture; the slice picks
        // the window by the rendered instant.
        let answer = recall(
            &corpus,
            &Question::TimeSlice {
                from: "2023-11-14T22:13:00.000Z".to_string(),
                to: "2023-11-14T22:13:59.999Z".to_string(),
            },
            RecallCaps::default(),
        );
        assert_eq!(answer.spans.len(), 2);
    }
}

/// The `history_recall` host tool, bound to one session directory.
///
/// Declared through [`crate::tools::HarnessToolRegistry::add_host_tool`] like
/// `goal`: the model sees it in the tool list, calls it like any tool, and
/// its result re-enters the neutral stream as an ordinary tool row — the
/// RLM-03 re-entry contract, no special surface. Zero model calls; the
/// answer is a bounded cited candidate, never authority.
pub fn host_tool(session_dir: PathBuf) -> crate::tools::HostTool {
    crate::tools::HostTool {
        definition: crate::tools::ToolDefinition {
            name: "history_recall".to_string(),
            description: crate::surfaces::tool_descriptions::HISTORY_RECALL.to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "object",
                        "description": "One of: {\"_tag\":\"Grep\",\"pattern\":str,\"caseSensitive\":bool?} | {\"_tag\":\"CursorSlice\",\"fromSequence\":n,\"toSequence\":n} | {\"_tag\":\"TimeSlice\",\"fromObservedAt\":iso,\"toObservedAt\":iso} | {\"_tag\":\"KeyTurns\",\"limit\":n} | {\"_tag\":\"TurnSummary\",\"turnId\":\"<sequence>\"}",
                    },
                    "caps": {
                        "type": "object",
                        "description": "Optional {maxSpans, maxEntriesScanned, maxCharsPerSpan}. Defaults: 24 / 2000 / 600. A cap hit is reported in the answer's honesty field."
                    }
                },
                "required": ["question"]
            }),
        },
        run: {
            let session_dir = session_dir.clone();
            Arc::new(move |call: &ToolCall, _cancel| {
                let session_dir = session_dir.clone();
                let question = call.arguments.get("question").cloned().unwrap_or(serde_json::Value::Null);
                let caps = call.arguments.get("caps").cloned().unwrap_or(serde_json::Value::Null);
                Box::pin(async move {
                    match recall_from_session(&session_dir, &question, &caps) {
                        Ok(answer) => (render_answer(&session_dir, &answer), false),
                        Err(message) => (format!("Nothing was recalled: {message}"), true),
                    }
                })
            })
        },
    }
}
