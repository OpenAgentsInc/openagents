//! One attempt's retained episode as a scrubbed, bounded trace bundle.
//!
//! The bundle reads only the retained files a viewer needs: Coder One's
//! trajectory (`trajectory.atif.json`, whose executor events are the
//! delegate's timed timeline), Jev's record, the briefing, and the
//! verifier's output. It never reads a credential file, never runs a
//! command, and never reads the raw delegate stream, whose thinking
//! signatures and rate-limit windows a viewer doesn't need. Every file it
//! reads must match the digest `retention.json` recorded when the trace
//! was retained.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::contract::{
    Attempt, Bar, EvidenceFile, JevCandidate, JevDecision, JevRequirement, ScrubReport, StepKind,
    TRACE_BUNDLE_SCHEMA, TestResult, Text, TraceBundle, TraceOutcome, TraceStep, VerifierDetail,
};
use crate::evidence::{Reader, Result, array, at, fail, opt_number, opt_string, opt_u64, string};
use crate::scrub::Scrubber;

/// A bundle's serialized size bound. A phone fetches one on demand.
pub const MAX_BUNDLE_BYTES: usize = 256 * 1024;

/// Per-field bounds tried in order until the bundle fits.
const FIELD_BOUNDS: [usize; 5] = [4096, 2048, 1024, 512, 256];

/// The briefing's bound: the briefing is the point of these attempts.
const BRIEFING_BOUND: usize = 24 * 1024;

/// The task instruction's bound.
pub const INSTRUCTION_BOUND: usize = 8 * 1024;

/// The verifier output tail's bound.
pub const VERIFIER_TAIL: usize = 2048;

/// What a bundle is built from.
pub struct Input<'a> {
    pub board: &'a str,
    pub attempt: &'a Attempt,
    pub bar: &'a Bar,
    /// The retained episode directory, repository-relative.
    pub episode: &'a str,
    /// Knowledge entry IDs written from this attempt's task.
    pub own_entries: &'a BTreeSet<String>,
}

/// Builds the bundle, shrinking field bounds until it fits.
pub fn build(reader: &Reader, input: &Input<'_>) -> Result<TraceBundle> {
    let files = Files::read(reader, input.episode)?;
    fit(&input.attempt.id, |bound, keep| {
        assemble(&files, input, bound, keep)
    })
}

/// Calls `assemble` with shrinking per-field bounds until the bundle fits
/// [`MAX_BUNDLE_BYTES`]; then, at the smallest bound, with fewer steps
/// kept (the start and the end), until it does.
pub fn fit(
    id: &str,
    assemble: impl Fn(usize, Option<usize>) -> Result<TraceBundle>,
) -> Result<TraceBundle> {
    for bound in FIELD_BOUNDS {
        let bundle = assemble(bound, None)?;
        if size(&bundle)? <= MAX_BUNDLE_BYTES {
            return Ok(bundle);
        }
    }
    // Still too large at the smallest bound: keep the start and the end.
    let smallest = FIELD_BOUNDS[FIELD_BOUNDS.len() - 1];
    let mut keep = 400;
    loop {
        let bundle = assemble(smallest, Some(keep))?;
        if size(&bundle)? <= MAX_BUNDLE_BYTES {
            return Ok(bundle);
        }
        if keep <= 20 {
            return Err(fail!("{id}: can't fit the bundle bound"));
        }
        keep /= 2;
    }
}

/// Keeps the first and last `keep / 2` steps and a marker between them;
/// returns how many were left out.
pub fn keep_ends(steps: &mut Vec<TraceStep>, keep: Option<usize>) -> usize {
    let Some(keep) = keep else { return 0 };
    if steps.len() <= keep {
        return 0;
    }
    let dropped = steps.len() - keep;
    let tail = steps.split_off(steps.len() - keep / 2);
    steps.truncate(keep - keep / 2);
    steps.push(TraceStep {
        at_ms: None,
        kind: StepKind::Host {
            text: Text {
                text: format!("[{dropped} steps left out to fit the bundle bound]"),
                original_bytes: None,
            },
        },
    });
    steps.extend(tail);
    dropped
}

fn size(bundle: &TraceBundle) -> Result<usize> {
    serde_json::to_vec(bundle)
        .map(|v| v.len())
        .map_err(|e| fail!("serialize: {e}"))
}

/// The retained files, read and checked against `retention.json`.
struct Files {
    sources: Vec<EvidenceFile>,
    trajectory: Value,
    jev: Option<Value>,
    briefing: Option<String>,
    ctrf: Option<Value>,
    reward: Option<f64>,
    stdout: Option<String>,
}

impl Files {
    fn read(reader: &Reader, episode: &str) -> Result<Self> {
        let (retention, retention_file) = reader.json(&format!("{episode}/retention.json"))?;
        let recorded: BTreeMap<String, String> = array(&retention, "files")?
            .iter()
            .filter_map(|f| Some((opt_string(f, "path")?, opt_string(f, "sha256")?)))
            .collect();
        let mut sources = vec![retention_file];
        let mut checked = |entry: EvidenceFile, name: &str| -> Result<EvidenceFile> {
            match recorded.get(name) {
                Some(sha) if *sha != entry.sha256 => Err(fail!(
                    "{}: its digest differs from retention.json",
                    entry.path
                )),
                _ => {
                    sources.push(entry.clone());
                    Ok(entry)
                }
            }
        };
        let optional = |name: &str| {
            let rel = format!("{episode}/{name}");
            reader.exists(&rel).then_some(rel)
        };

        let rel = format!("{episode}/trajectory.atif.json");
        let (trajectory, entry) = reader.json(&rel)?;
        checked(entry, "trajectory.atif.json")?;
        let jev = match optional("artifacts/briefing-jev.json") {
            Some(rel) => {
                let (v, e) = reader.json(&rel)?;
                checked(e, "artifacts/briefing-jev.json")?;
                Some(v)
            }
            None => None,
        };
        let briefing = match optional("artifacts/delegate-1.briefing.md") {
            Some(rel) => {
                let (t, e) = reader.text(&rel)?;
                checked(e, "artifacts/delegate-1.briefing.md")?;
                Some(t)
            }
            None => None,
        };
        let ctrf = match optional("verifier/ctrf.json") {
            Some(rel) => {
                let (v, e) = reader.json(&rel)?;
                checked(e, "verifier/ctrf.json")?;
                Some(v)
            }
            None => None,
        };
        let reward = match optional("verifier/reward.txt") {
            Some(rel) => {
                let (t, e) = reader.text(&rel)?;
                checked(e, "verifier/reward.txt")?;
                t.trim().parse().ok()
            }
            None => None,
        };
        let stdout = match optional("verifier/test-stdout.txt") {
            Some(rel) => {
                let (t, e) = reader.text(&rel)?;
                checked(e, "verifier/test-stdout.txt")?;
                Some(t)
            }
            None => None,
        };
        Ok(Self {
            sources,
            trajectory,
            jev,
            briefing,
            ctrf,
            reward,
            stdout,
        })
    }
}

fn assemble(
    files: &Files,
    input: &Input<'_>,
    bound: usize,
    keep: Option<usize>,
) -> Result<TraceBundle> {
    let mut scrub = Scrubber::new(bound);
    let steps_json = array(&files.trajectory, "steps")?;
    let origin = steps_json
        .first()
        .and_then(|s| opt_string(s, "timestamp"))
        .and_then(|t| iso_ms(&t));
    let delegate_model = steps_json.iter().find_map(|s| {
        let calls = at(s, "tool_calls").as_array()?;
        calls
            .iter()
            .any(|c| at(c, "function_name").as_str() == Some("delegate"))
            .then(|| opt_string(s, "model_name"))
            .flatten()
    });

    let mut instruction = None;
    let mut steps = Vec::new();
    let mut usage = [0_u64; 4];
    for step in steps_json {
        let at_ms = opt_string(step, "timestamp")
            .and_then(|t| iso_ms(&t))
            .zip(origin)
            .map(|(t, o)| t.saturating_sub(o));
        let source = opt_string(step, "source").unwrap_or_default();
        let message = opt_string(step, "message").unwrap_or_default();
        if let Some(event) = at(step, "extra/executor_event/event").as_object() {
            let event = Value::Object(event.clone());
            let kind = match at(&event, "kind").as_str().unwrap_or("") {
                "session_started" => Some(StepKind::DelegateStarted {
                    agent: opt_string(step, "extra/executor_event/adapter")
                        .unwrap_or_else(|| "delegate".into()),
                    model: delegate_model.clone(),
                }),
                "assistant_claim" => Some(StepKind::Say {
                    text: scrub.text(&opt_string(&event, "text").unwrap_or_default()),
                }),
                "command_started" => Some(StepKind::Command {
                    command: scrub.text(&opt_string(&event, "command").unwrap_or_default()),
                }),
                "command_completed" => Some(StepKind::CommandResult {
                    exit_code: at(&event, "exit_code").as_i64(),
                    output: scrub.text(&opt_string(&event, "output").unwrap_or_default()),
                }),
                "usage_update" => {
                    let u = at(&event, "usage");
                    for (slot, key) in [
                        "input_tokens",
                        "cache_creation_input_tokens",
                        "cache_read_input_tokens",
                        "output_tokens",
                    ]
                    .iter()
                    .enumerate()
                    {
                        usage[slot] += opt_u64(u, key).unwrap_or(0);
                    }
                    Some(StepKind::Usage {
                        input_tokens: usage[0],
                        cache_write_tokens: usage[1],
                        cache_read_tokens: usage[2],
                        output_tokens: usage[3],
                    })
                }
                "session_ended" => Some(StepKind::DelegateEnded {
                    error: at(&event, "error").as_bool().unwrap_or(false),
                    result: scrub.text(&opt_string(&event, "result").unwrap_or_default()),
                }),
                _ => None,
            };
            if let Some(kind) = kind {
                steps.push(TraceStep { at_ms, kind });
            }
            continue;
        }
        if source == "user" && instruction.is_none() {
            instruction = Some(scrub.text_within(&message, INSTRUCTION_BOUND));
            continue;
        }
        let calls = at(step, "tool_calls").as_array();
        if let Some(call) = calls.and_then(|c| c.first()) {
            let name = string(call, "function_name")?;
            if name == "delegate" {
                // Its prompt is the briefing, carried once below.
                continue;
            }
            steps.push(TraceStep {
                at_ms,
                kind: StepKind::Decision {
                    duration_ms: opt_u64(step, "extra/duration_ms"),
                    name,
                    question: None,
                    answers: Vec::new(),
                    detail: Vec::new(),
                    cost_usd: None,
                },
            });
            continue;
        }
        if !message.is_empty() {
            steps.push(TraceStep {
                at_ms,
                kind: StepKind::Host {
                    text: scrub.text(&message),
                },
            });
        }
    }
    let dropped = keep_ends(&mut steps, keep);

    let jev = files
        .jev
        .as_ref()
        .map(|j| jev_decision(j, input.own_entries, &mut scrub))
        .transpose()?;
    let briefing = files
        .briefing
        .as_deref()
        .map(|b| scrub.text_within(b, BRIEFING_BOUND));
    let verifier = files.ctrf.as_ref().map(|ctrf| {
        let tests: Vec<TestResult> = at(ctrf, "results/tests")
            .as_array()
            .map(|tests| {
                tests
                    .iter()
                    .map(|t| TestResult {
                        name: opt_string(t, "name").unwrap_or_default(),
                        status: opt_string(t, "status").unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let tail = last_part(files.stdout.as_deref().unwrap_or(""), VERIFIER_TAIL * 4);
        VerifierDetail {
            reward: files.reward,
            passed: opt_u64(ctrf, "results/summary/passed").unwrap_or(0) as u32,
            failed: opt_u64(ctrf, "results/summary/failed").unwrap_or(0) as u32,
            tests,
            output_tail: scrub.text_within(tail, VERIFIER_TAIL),
        }
    });

    let attempt = input.attempt;
    Ok(TraceBundle {
        schema: TRACE_BUNDLE_SCHEMA.into(),
        board: input.board.into(),
        attempt: attempt.id.clone(),
        task: attempt.task.clone(),
        sources: files.sources.clone(),
        instruction: instruction.unwrap_or_default(),
        jev,
        briefing,
        steps,
        verifier,
        outcome: TraceOutcome {
            passed: attempt.passed,
            beat: attempt.beat,
            seconds: attempt.seconds,
            cost: attempt.cost,
            bar: input.bar.clone(),
            how_it_ended: attempt.how_it_ended.clone(),
        },
        scrub: ScrubReport {
            redactions: scrub.redactions,
            truncated_fields: scrub.truncated,
            dropped_steps: u32::try_from(dropped).unwrap_or(u32::MAX),
            field_bound: u32::try_from(bound).unwrap_or(u32::MAX),
        },
    })
}

fn jev_decision(
    record: &Value,
    own: &BTreeSet<String>,
    scrub: &mut Scrubber,
) -> Result<JevDecision> {
    let questions = at(record, "questions")
        .as_object()
        .map(|q| {
            q.iter()
                .map(|(k, v)| format!("{k}: {}", v.as_str().unwrap_or("")))
                .collect()
        })
        .unwrap_or_default();
    let candidates = array(record, "candidates")?
        .iter()
        .map(|c| {
            let id = string(c, "id")?;
            Ok(JevCandidate {
                rank: opt_u64(c, "rank").unwrap_or(0) as u32,
                version: opt_u64(c, "version").unwrap_or(0) as u32,
                title: opt_string(c, "title").map(|t| scrub.redact(&t)),
                sha256: opt_string(c, "sha256").unwrap_or_default(),
                score: opt_number(c, "score").unwrap_or(f64::NAN),
                p: opt_number(c, "p").ok_or_else(|| fail!("candidate {id}: no p"))?,
                kept: at(c, "kept").as_bool().unwrap_or(false),
                fate: opt_string(c, "fate").unwrap_or_default(),
                written_from_this_task: Some(own.contains(&id)),
                id,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let requirements = array(record, "requirements")?
        .iter()
        .map(|r| {
            Ok(JevRequirement {
                text: scrub.redact(&string(r, "text")?),
                p: opt_number(r, "p").ok_or_else(|| fail!("requirement without p"))?,
                flagged: at(r, "flagged").as_bool().unwrap_or(false),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(JevDecision {
        question_set: opt_string(record, "question_set").unwrap_or_default(),
        questions,
        keep_threshold: opt_number(record, "thresholds/keep").unwrap_or(f64::NAN),
        flag_threshold: opt_number(record, "thresholds/flag").unwrap_or(f64::NAN),
        budget_chars: opt_u64(record, "thresholds/budget_chars"),
        milliseconds: opt_u64(record, "milliseconds"),
        input_tokens: opt_u64(record, "input_tokens"),
        candidates,
        requirements,
    })
}

/// The last `bytes` bytes of `text` (or a little less, on a character
/// boundary): the part of a verifier's output that holds its verdict.
#[must_use]
pub fn last_part(text: &str, bytes: usize) -> &str {
    let start = text.len().saturating_sub(bytes);
    let start = (start..=text.len())
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(text.len());
    &text[start..]
}

/// Milliseconds since the epoch of `YYYY-MM-DDTHH:MM:SS[.fff]Z`.
#[must_use]
pub fn iso_ms(text: &str) -> Option<u64> {
    let text = text.strip_suffix('Z')?;
    let (date, time) = text.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let (hms, frac) = time.split_once('.').unwrap_or((time, "0"));
    let mut t = hms.split(':').map(|p| p.parse::<i64>().ok());
    let (h, min, s) = (t.next()??, t.next()??, t.next()??);
    let ms: i64 = format!("{frac:0<3}")[..3].parse().ok()?;
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(((days * 24 + h) * 60 + min) * 60_000 + s * 1000 + ms).ok()
}

#[cfg(test)]
mod tests {
    use super::iso_ms;

    #[test]
    fn iso_timestamps_parse_to_epoch_milliseconds() {
        assert_eq!(iso_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso_ms("2026-09-27T23:20:14.056Z"), Some(1_790_551_214_056));
        assert_eq!(
            iso_ms("2026-09-27T23:20:15.072Z")
                .zip(iso_ms("2026-09-27T23:20:14.056Z"))
                .map(|(a, b)| a - b),
            Some(1016)
        );
        assert_eq!(iso_ms("not a time"), None);
    }
}
