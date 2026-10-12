//! What a recorded session did: stats and anomaly scans over the three
//! record shapes this repository writes.
//!
//! The crate's [`crate::log`] module is about fidelity — read back exactly
//! what was written, faults and all. This module is about reading *for*
//! something: how many calls went to each tool, what repeated itself, where
//! the session stalled, and which calls a refusal or a failure stopped.
//! A person auditing a trace — "what was it doing, why did it ask five
//! times, where did the cost go" — wants these answers without reading the
//! record end to end.
//!
//! # The shapes it reads
//!
//! - **Session logs** (`*.atif.jsonl`): the append-only record, read through
//!   [`crate::log::read`], so its faults surface as anomalies rather than
//!   disappearing.
//! - **Exported documents** (`*.atif.json`): the rendered document a
//!   consumer gets; steps read back through the document's own fields.
//! - **Event streams** (`*-events.jsonl`): the per-step stream a Claude
//!   Agent SDK session writes — `assistant` records carrying `tool_use`
//!   blocks, `user` records carrying `tool_result` blocks — which is what
//!   a solver hand, a `claude` delegation, or a burn-down subagent leaves
//!   behind.
//! - **A run directory** holds a `summary.json` (whose `solver.hands` gives
//!   each hand's wall time, turns, tokens and cost) plus one events file
//!   per hand; [`read_dir`] rolls them into one view.
//!
//! Everything lands in one [`Trace`]: ordered [`ObservedCall`]s and
//! [`ObservedStep`]s, which the scans then read.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::document::{Outcome, Source, digest, parse_iso};
use crate::log;

/// A gap between two consecutive records this long reads as a stall: the
/// process sat waiting on something the trace does not show — a build, a
/// permission prompt, a model turn the stream skipped.
const STALL_MS: u64 = 60_000;

/// The same call — same tool, same arguments — this many times in a row is
/// a loop, not a retry policy. The harness study's reminder fires at five;
/// a reader wants to know earlier.
const STUCK_SAME: usize = 3;

/// The same call failing identically this many times in a row means the
/// step after the first failure changed nothing that mattered.
const STUCK_FAILED: usize = 2;

/// One tool result larger than this is worth naming: it is the context
/// weight the trace carried, and often the thing a briefing should have
/// filtered down.
const LARGE_RESULT_CHARS: usize = 64 * 1024;

/// A step's prompt tokens past both this floor and four times the median
/// is an outlier worth naming.
const OUTLIER_TOKENS: u64 = 20_000;

/// One call, normalized to what the scans read.
#[derive(Clone, Debug)]
pub struct ObservedCall {
    /// The step or record it belongs to, numbered from one in the order
    /// the file holds.
    pub step: usize,
    /// When it ran, milliseconds since the epoch where the record knows.
    /// Event streams carry relative milliseconds instead; either way the
    /// deltas between neighbors are real.
    pub at_ms: Option<u64>,
    /// The tool or question's name.
    pub name: String,
    /// What identifies it as "the same call again": the name plus the
    /// canonical arguments, so two `Read`s of different files are two
    /// calls and two `Read`s of the same file are one.
    pub key: String,
    /// How it ended, when the record says.
    pub outcome: Option<Outcome>,
    /// How long it took, when the record says.
    pub milliseconds: Option<u64>,
    /// The arguments it was given, for naming it to a person.
    pub arguments: Value,
    /// The size of what came back, in characters.
    pub output_chars: usize,
    /// Whether the call was a decision-model question rather than work on
    /// the machine.
    pub decision: bool,
}

/// One step, normalized the same way.
#[derive(Clone, Debug)]
pub struct ObservedStep {
    /// Its number in the file's order, from one.
    pub ordinal: usize,
    /// When it happened; see [`ObservedCall::at_ms`].
    pub at_ms: Option<u64>,
    /// Who it came from.
    pub source: Source,
    /// Whether it carries text a person or another model would read.
    pub spoke: bool,
    /// The turn's prompt tokens, when the record counts them.
    pub prompt_tokens: Option<u64>,
    /// The turn's completion tokens, when the record counts them.
    pub completion_tokens: Option<u64>,
    /// The model that produced it, when the record says.
    pub model: Option<String>,
}

/// Which of the three shapes the file was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceKind {
    /// The append-only session log.
    Log,
    /// A rendered ATIF document.
    Document,
    /// An SDK event stream (`assistant`/`user`/`result` records).
    Events,
}

impl TraceKind {
    /// The word a report spells the kind with.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Log => "log",
            Self::Document => "document",
            Self::Events => "events",
        }
    }
}

/// A trace normalized for the scans.
#[derive(Clone, Debug)]
pub struct Trace {
    /// Where it was read from.
    pub source_path: PathBuf,
    /// Which shape the file was.
    pub kind: TraceKind,
    /// The session's identity, where the record carries one.
    pub session_id: String,
    /// The models seen on the session's steps, in first-seen order.
    pub models: Vec<String>,
    /// How the session ended: `ended`, `interrupted`, or empty when the
    /// shape does not say.
    pub state: String,
    /// Wall time the record attributes to the session, in seconds.
    pub seconds: Option<u64>,
    /// The steps in file order. Event streams treat each `tool_use` as the
    /// step it belongs to; the ordinal is the record's place in the file.
    pub steps: Vec<ObservedStep>,
    /// The calls in file order, each naming its step.
    pub calls: Vec<ObservedCall>,
    /// Lines the reader could not use, in words.
    pub faults: Vec<String>,
}

/// One thing a scan found worth naming.
#[derive(Clone, Debug)]
pub struct Anomaly {
    /// The finding's kind, a `snake_case` word: `stuck`, `stuck_failure`,
    /// `repeated`, `cancelled`, `stall`, `large_result`, `outlier`,
    /// `empty_step`, `fault`.
    pub kind: &'static str,
    /// The first step the finding covers.
    pub step: usize,
    /// What it is, in a sentence.
    pub detail: String,
}

/// Reads `path` as whichever of the three shapes it is.
///
/// The shape is chosen by looking at the records, not the extension: a
/// file whose first data line is a `record` envelope is a log; a whole
/// file that parses as a document with `steps` is a document; a lines file
/// of `type` records is an event stream. A file that is none of these is
/// an `InvalidData` error rather than a misleading empty report.
///
/// # Errors
///
/// Returns the underlying filesystem error, or `InvalidData` when the file
/// holds none of the three shapes.
pub fn read(path: &Path) -> io::Result<Trace> {
    let bytes = std::fs::read(path)?;
    sniff(path, &bytes)
}

/// The shape-dispatch half of [`read`], over bytes already in hand.
fn sniff(path: &Path, bytes: &[u8]) -> io::Result<Trace> {
    let first = bytes
        .split(|byte| *byte == b'\n')
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    if let Ok(line) = std::str::from_utf8(first)
        && let Ok(record) = serde_json::from_str::<Map<String, Value>>(line)
    {
        if record.contains_key("record") {
            return read_log(path, bytes);
        }
        if record.contains_key("type") {
            return read_events(path, bytes);
        }
    }
    if let Ok(document) = serde_json::from_slice::<Value>(bytes)
        && document.get("steps").and_then(Value::as_array).is_some()
    {
        return read_document(path, &document);
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "{} is not a session log, an ATIF document, or an event stream",
            path.display()
        ),
    ))
}

/// A session log through the recovering reader, faults included.
fn read_log(path: &Path, bytes: &[u8]) -> io::Result<Trace> {
    let recording = log::read_bytes(path, bytes)?;
    let mut models = Vec::new();
    let mut steps = Vec::new();
    let mut calls = Vec::new();
    for (index, step) in recording.steps.iter().enumerate() {
        let ordinal = index + 1;
        if let Some(model) = &step.model
            && !models.contains(model)
        {
            models.push(model.clone());
        }
        steps.push(ObservedStep {
            ordinal,
            at_ms: Some(step.at),
            source: step.source,
            spoke: !step.message.trim().is_empty() || step.reasoning.is_some(),
            prompt_tokens: step.tokens.map(|(prompt, _)| prompt),
            completion_tokens: step.tokens.map(|(_, completion)| completion),
            model: step.model.clone(),
        });
        if let Some(call) = &step.call {
            calls.push(ObservedCall {
                step: ordinal,
                at_ms: Some(step.at),
                name: call.name.clone(),
                key: key(&call.name, &call.arguments),
                outcome: Some(call.outcome),
                milliseconds: Some(call.milliseconds),
                arguments: call.arguments.clone(),
                output_chars: call.output.chars().count(),
                decision: call.is_decision(),
            });
        }
    }
    if models.is_empty() && !recording.session.model.is_empty() {
        models.push(recording.session.model.clone());
    }
    Ok(Trace {
        source_path: path.to_path_buf(),
        kind: TraceKind::Log,
        session_id: recording.session.id.clone(),
        models,
        state: recording.session.state.clone(),
        seconds: Some(recording.session.seconds),
        steps,
        calls,
        faults: recording
            .faults
            .iter()
            .map(|fault| fault.to_string())
            .collect(),
    })
}

/// A rendered ATIF document, read back from its own fields.
fn read_document(path: &Path, document: &Value) -> io::Result<Trace> {
    let mut models = Vec::new();
    let mut steps = Vec::new();
    let mut calls = Vec::new();
    for (index, step) in document["steps"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let ordinal = step
            .get("step_id")
            .and_then(Value::as_u64)
            .map_or(index + 1, |id| id as usize);
        let model = step.get("model_name").and_then(Value::as_str);
        if let Some(model) = model
            && !models.iter().any(|have| have == model)
        {
            models.push(model.to_string());
        }
        let source = match step.get("source").and_then(Value::as_str) {
            Some("user") => Source::User,
            Some("system") => Source::System,
            _ => Source::Agent,
        };
        let metrics = step.get("metrics");
        steps.push(ObservedStep {
            ordinal,
            at_ms: step
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(parse_iso),
            source,
            spoke: step
                .get("message")
                .and_then(Value::as_str)
                .is_some_and(|message| !message.trim().is_empty())
                || step.get("reasoning_content").is_some(),
            prompt_tokens: metrics
                .and_then(|m| m.get("prompt_tokens"))
                .and_then(Value::as_u64),
            completion_tokens: metrics
                .and_then(|m| m.get("completion_tokens"))
                .and_then(Value::as_u64),
            model: model.map(str::to_string),
        });
        for tool_call in step
            .get("tool_calls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = tool_call
                .get("function_name")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string();
            let arguments = tool_call.get("arguments").cloned().unwrap_or(Value::Null);
            let observation = step
                .get("observation")
                .and_then(|obs| obs.get("results"))
                .and_then(Value::as_array)
                .and_then(|results| {
                    results.iter().find(|result| {
                        result.get("source_call_id") == tool_call.get("tool_call_id")
                    })
                })
                .or_else(|| {
                    step.get("observation")
                        .and_then(|obs| obs.get("results"))
                        .and_then(Value::as_array)
                        .and_then(|results| results.first())
                });
            let outcome = observation
                .and_then(|result| result.get("extra"))
                .and_then(|extra| extra.get("status"))
                .and_then(Value::as_str)
                .map(outcome);
            calls.push(ObservedCall {
                step: ordinal,
                at_ms: steps.last().and_then(|s| s.at_ms),
                name: name.clone(),
                key: key(&name, &arguments),
                outcome,
                arguments,
                milliseconds: observation
                    .and_then(|result| result.get("extra"))
                    .and_then(|extra| extra.get("duration_ms"))
                    .and_then(Value::as_u64),
                output_chars: observation
                    .and_then(|result| result.get("content"))
                    .and_then(Value::as_str)
                    .map_or(0, str::len),
                decision: tool_call
                    .get("extra")
                    .and_then(|extra| extra.get("schema"))
                    .and_then(Value::as_str)
                    == Some(crate::document::DECISION_CALL_SCHEMA),
            });
        }
    }
    if models.is_empty()
        && let Some(model) = document
            .get("agent")
            .and_then(|agent| agent.get("model_name"))
            .and_then(Value::as_str)
    {
        models.push(model.to_string());
    }
    let state = document
        .get("extra")
        .and_then(|extra| extra.get("state"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Ok(Trace {
        source_path: path.to_path_buf(),
        kind: TraceKind::Document,
        session_id: document
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        models,
        state,
        seconds: document
            .get("final_metrics")
            .and_then(|m| m.get("extra"))
            .and_then(|e| e.get("wall_seconds"))
            .and_then(Value::as_f64)
            .map(|s| s as u64),
        steps,
        calls,
        faults: document
            .get("extra")
            .and_then(|extra| extra.get("faults"))
            .and_then(Value::as_array)
            .map(|faults| {
                faults
                    .iter()
                    .filter_map(|f| {
                        Some(format!(
                            "line {}: {}",
                            f.get("line")?.as_u64()?,
                            f.get("fault")?.as_str()?
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// An SDK event stream: `assistant` records carry `tool_use` blocks and
/// `user` records carry the `tool_result` blocks that answer them.
///
/// The records' `t_ms` is relative, so durations between neighbors are
/// real but absolute times are not. A `tool_use` pairs to its result by
/// `tool_use_id`; a use that never got a result reads as a call with no
/// outcome.
fn read_events(path: &Path, bytes: &[u8]) -> io::Result<Trace> {
    let mut models = Vec::new();
    let mut steps = Vec::new();
    let mut calls = Vec::new();
    // Call ids to the call's place in `calls`, so a result can fill in the
    // outcome and output size after the fact.
    let mut waiting: BTreeMap<String, usize> = BTreeMap::new();
    let mut records = 0usize;
    for raw in bytes.split(|byte| *byte == b'\n') {
        if raw.is_empty() {
            continue;
        }
        records += 1;
        let Ok(event) = serde_json::from_slice::<Value>(raw) else {
            continue;
        };
        let t_ms = event.get("t_ms").and_then(Value::as_u64);
        let kind = event.get("type").and_then(Value::as_str);
        let message = event.get("message");
        match kind {
            Some("assistant") => {
                if let Some(model) = message.and_then(|m| m.get("model")).and_then(Value::as_str)
                    && !models.iter().any(|have| have == model)
                {
                    models.push(model.to_string());
                }
                let content = message
                    .and_then(|m| m.get("content"))
                    .and_then(Value::as_array);
                let mut spoke = false;
                for block in content.into_iter().flatten() {
                    match block.get("type").and_then(Value::as_str) {
                        Some("tool_use") => {
                            let name = block
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("?")
                                .to_string();
                            let arguments = block.get("input").cloned().unwrap_or(Value::Null);
                            calls.push(ObservedCall {
                                step: records,
                                at_ms: t_ms,
                                name: name.clone(),
                                key: key(&name, &arguments),
                                outcome: None,
                                milliseconds: None,
                                arguments,
                                output_chars: 0,
                                decision: false,
                            });
                            if let Some(id) = block.get("id").and_then(Value::as_str) {
                                waiting.insert(id.to_string(), calls.len() - 1);
                            }
                        }
                        Some("text") => {
                            spoke |= block
                                .get("text")
                                .and_then(Value::as_str)
                                .is_some_and(|text| !text.trim().is_empty());
                        }
                        Some("thinking") => spoke = true,
                        _ => {}
                    }
                }
                steps.push(ObservedStep {
                    ordinal: records,
                    at_ms: t_ms,
                    source: Source::Agent,
                    spoke,
                    prompt_tokens: message
                        .and_then(|m| m.get("usage"))
                        .and_then(|u| u.get("input_tokens"))
                        .and_then(Value::as_u64),
                    completion_tokens: message
                        .and_then(|m| m.get("usage"))
                        .and_then(|u| u.get("output_tokens"))
                        .and_then(Value::as_u64),
                    model: message
                        .and_then(|m| m.get("model"))
                        .and_then(Value::as_str)
                        .map(str::to_string),
                });
            }
            Some("user") => {
                for block in message
                    .and_then(|m| m.get("content"))
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if block.get("type").and_then(Value::as_str) != Some("tool_result") {
                        continue;
                    }
                    let Some(at) = block
                        .get("tool_use_id")
                        .and_then(Value::as_str)
                        .and_then(|id| waiting.remove(id))
                    else {
                        continue;
                    };
                    let failed = block
                        .get("is_error")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    calls[at].outcome = Some(if failed {
                        Outcome::Failed
                    } else {
                        Outcome::Completed
                    });
                    calls[at].milliseconds =
                        t_ms.and_then(|t| calls[at].at_ms.map(|start| t.saturating_sub(start)));
                    calls[at].output_chars =
                        block.get("content").map(content_chars).unwrap_or_default();
                }
            }
            Some("result") => {
                // The closing record carries the run's usage summary;
                // nothing to add per step.
            }
            _ => {
                steps.push(ObservedStep {
                    ordinal: records,
                    at_ms: t_ms,
                    source: match kind {
                        Some("system") => Source::System,
                        _ => Source::User,
                    },
                    spoke: event.get("set").is_some() || kind.is_some(),
                    prompt_tokens: None,
                    completion_tokens: None,
                    model: None,
                });
            }
        }
    }
    Ok(Trace {
        source_path: path.to_path_buf(),
        kind: TraceKind::Events,
        session_id: String::new(),
        models,
        state: String::new(),
        seconds: steps
            .last()
            .and_then(|step| step.at_ms)
            .map(|t_ms| t_ms / 1_000),
        steps,
        calls,
        faults: Vec::new(),
    })
}

/// The character count of a `tool_result` block's content, which arrives
/// as a string or a list of typed parts.
fn content_chars(content: &Value) -> usize {
    match content {
        Value::String(text) => text.len(),
        Value::Array(parts) => parts
            .iter()
            .map(|part| part.get("text").and_then(Value::as_str).map_or(0, str::len))
            .sum(),
        other => serde_json::to_string(other).map_or(0, |s| s.len()),
    }
}

/// The identity two calls share when they did the same thing: the tool's
/// name and a canonical digest of its arguments, so reordering an object
/// does not split a pair.
fn key(name: &str, arguments: &Value) -> String {
    format!("{name}:{}", digest(arguments))
}

/// A call as a short string a person can read: the tool's name and its
/// most telling argument.
fn describe(call: &ObservedCall) -> String {
    let hint = Some(&call.arguments)
        .and_then(|args| {
            for field in [
                "command",
                "path",
                "file_path",
                "pattern",
                "cmd",
                "args",
                "url",
            ] {
                if let Some(value) = args.get(field).and_then(Value::as_str) {
                    return Some(value.to_string());
                }
                if let Some(value) = args.get(field) {
                    return Some(value.to_string());
                }
            }
            args.as_object()
                .and_then(|obj| obj.values().next())
                .map(|value| value.to_string())
        })
        .unwrap_or_default();
    let hint: String = hint.chars().take(80).collect();
    if hint.is_empty() {
        call.name.clone()
    } else {
        format!("{} {}", call.name, hint.trim())
    }
}

/// An outcome word to the enum, tolerating the spellings the three shapes
/// use.
fn outcome(word: &str) -> Outcome {
    match word {
        "cancelled" | "canceled" | "denied" | "refused" => Outcome::Cancelled,
        "failed" | "error" => Outcome::Failed,
        _ => Outcome::Completed,
    }
}

/// What [`stats`] and [`anomalies`] work over when the path is a run
/// directory: the summary plus each events file, as one report.
#[derive(Clone, Debug)]
pub struct RunDir {
    /// The `summary.json` value, when the directory has one that parses.
    pub summary: Option<Value>,
    /// One trace per events file in the directory, named by file.
    pub traces: Vec<(PathBuf, Trace)>,
}

/// Reads a directory: its `summary.json` plus every `*-events.jsonl` it
/// holds, each through the event-stream reader.
///
/// # Errors
///
/// Returns the underlying filesystem error, or `InvalidData` when the
/// directory has neither a summary nor an events file.
pub fn read_dir(path: &Path) -> io::Result<RunDir> {
    let summary = std::fs::read(path.join("summary.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let mut traces = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let file = entry?.path();
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if name.ends_with("-events.jsonl") || name == "events.jsonl" {
            if let Ok(bytes) = std::fs::read(&file)
                && let Ok(trace) = sniff(&file, &bytes)
            {
                traces.push((file, trace));
            }
        }
    }
    if summary.is_none() && traces.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} holds no summary.json and no readable events file",
                path.display()
            ),
        ));
    }
    traces.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(RunDir { summary, traces })
}

/// The `solver.hands` table of a run directory's summary, as rows of
/// (hand, round, wall, turns, cost, tokens, first verdict line).
fn hands(summary: &Value) -> Vec<Value> {
    summary
        .get("solver")
        .and_then(|solver| solver.get("hands"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// The stats report over one trace, as a JSON value.
#[must_use]
pub fn stats(trace: &Trace) -> Value {
    let mut by_source: Map<String, Value> = Map::new();
    for step in &trace.steps {
        let key = step.source.word();
        let count = by_source.get(key).and_then(Value::as_u64).unwrap_or(0);
        by_source.insert(key.to_string(), json!(count + 1));
    }
    let mut by_tool: BTreeMap<String, (u64, u64, u64)> = BTreeMap::new();
    let mut failed = 0u64;
    let mut cancelled = 0u64;
    let mut decisions = 0u64;
    for call in &trace.calls {
        let entry = by_tool.entry(call.name.clone()).or_default();
        entry.0 += 1;
        entry.2 += call.milliseconds.unwrap_or(0);
        match call.outcome {
            Some(Outcome::Failed) => {
                entry.1 += 1;
                failed += 1;
            }
            Some(Outcome::Cancelled) => {
                entry.1 += 1;
                cancelled += 1;
            }
            _ => {}
        }
        if call.decision {
            decisions += 1;
        }
    }
    let tools: Vec<Value> = {
        let mut rows: Vec<(&String, &(u64, u64, u64))> = by_tool.iter().collect();
        rows.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(b.1.2.cmp(&a.1.2)));
        rows.into_iter()
            .map(|(name, (count, bad, ms))| {
                json!({"name": name, "calls": count, "not_completed": bad, "ms": ms})
            })
            .collect()
    };
    let prompt: u64 = trace.steps.iter().filter_map(|s| s.prompt_tokens).sum();
    let completion: u64 = trace.steps.iter().filter_map(|s| s.completion_tokens).sum();
    let mut step_prompts: Vec<u64> = trace.steps.iter().filter_map(|s| s.prompt_tokens).collect();
    step_prompts.sort_unstable();
    let median_prompt = step_prompts
        .get(step_prompts.len() / 2)
        .copied()
        .unwrap_or(0);
    let mut largest: Vec<(&ObservedCall, usize)> = trace
        .calls
        .iter()
        .map(|call| (call, call.output_chars))
        .collect();
    largest.sort_by(|a, b| b.1.cmp(&a.1));
    let largest: Vec<Value> = largest
        .into_iter()
        .take(5)
        .filter(|(_, chars)| *chars > 0)
        .map(|(call, chars)| json!({"step": call.step, "name": call.name, "chars": chars}))
        .collect();
    json!({
        "path": trace.source_path.display().to_string(),
        "kind": trace.kind.word(),
        "session_id": trace.session_id,
        "models": trace.models,
        "state": trace.state,
        "wall_seconds": trace.seconds,
        "steps": trace.steps.len(),
        "by_source": by_source,
        "calls": trace.calls.len(),
        "calls_failed": failed,
        "calls_cancelled": cancelled,
        "decision_calls": decisions,
        "tokens": {
            "prompt": prompt,
            "completion": completion,
            "median_step_prompt": median_prompt,
        },
        "tools": tools,
        "repeated": repeated(trace),
        "largest_results": largest,
        "stalls_over_60s": stalls(trace).len(),
        "faults": trace.faults.len(),
    })
}

/// Groups of identical calls — same tool and same arguments — that ran at
/// least twice anywhere in the trace, most-repeated first.
fn repeated(trace: &Trace) -> Value {
    let mut groups: BTreeMap<&str, Vec<&ObservedCall>> = BTreeMap::new();
    for call in &trace.calls {
        groups.entry(call.key.as_str()).or_default().push(call);
    }
    let mut rows: Vec<Value> = groups
        .into_iter()
        .filter(|(_, calls)| calls.len() > 1)
        .map(|(_, calls)| {
            json!({
                "what": describe(&calls[0]),
                "executions": calls.len(),
                "steps": calls.iter().map(|call| call.step).collect::<Vec<_>>(),
                "ms": calls.iter().filter_map(|call| call.milliseconds).sum::<u64>(),
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        b["executions"]
            .as_u64()
            .cmp(&a["executions"].as_u64())
            .then(b["ms"].as_u64().cmp(&a["ms"].as_u64()))
    });
    Value::Array(rows)
}

/// The stall windows: pairs of consecutive timed records more than
/// `STALL_MS` apart, longest first.
fn stalls(trace: &Trace) -> Vec<(usize, u64)> {
    let mut timed: Vec<(usize, u64)> = trace
        .steps
        .iter()
        .filter_map(|step| step.at_ms.map(|at| (step.ordinal, at)))
        .collect();
    timed.sort_by_key(|(_, at)| *at);
    let mut gaps: Vec<(usize, u64)> = timed
        .windows(2)
        .filter_map(|pair| {
            let gap = pair[1].1.saturating_sub(pair[0].1);
            (gap > STALL_MS).then_some((pair[0].0, gap))
        })
        .collect();
    gaps.sort_by(|a, b| b.1.cmp(&a.1));
    gaps
}

/// The anomalies of one trace: everything a scan names, in step order.
#[must_use]
pub fn anomalies(trace: &Trace) -> Vec<Anomaly> {
    let mut found: Vec<Anomaly> = Vec::new();

    // The same call in a row is a loop the model did not notice it was
    // in; the same call failing in a row is the stronger version of it.
    let mut start = 0usize;
    for index in 1..=trace.calls.len() {
        let continuing =
            index < trace.calls.len() && trace.calls[index].key == trace.calls[start].key;
        if continuing {
            continue;
        }
        let calls = &trace.calls[start..index];
        if calls.len() >= STUCK_SAME {
            found.push(Anomaly {
                kind: "stuck",
                step: calls[0].step,
                detail: format!(
                    "{} ran identically {} times in a row (steps {}-{})",
                    describe(&calls[0]),
                    calls.len(),
                    calls[0].step,
                    calls[calls.len() - 1].step,
                ),
            });
        }
        if calls.len() >= STUCK_FAILED
            && calls
                .iter()
                .all(|call| call.outcome == Some(Outcome::Failed))
        {
            found.push(Anomaly {
                kind: "stuck_failure",
                step: calls[0].step,
                detail: format!(
                    "{} failed identically {} times in a row (steps {}-{})",
                    describe(&calls[0]),
                    calls.len(),
                    calls[0].step,
                    calls[calls.len() - 1].step,
                ),
            });
        }
        start = index;
    }

    for call in &trace.calls {
        if call.outcome == Some(Outcome::Cancelled) {
            found.push(Anomaly {
                kind: "cancelled",
                step: call.step,
                detail: format!("{} was refused or cancelled", describe(call)),
            });
        }
        if call.outcome == Some(Outcome::Failed) {
            found.push(Anomaly {
                kind: "failed",
                step: call.step,
                detail: format!("{} failed", describe(call)),
            });
        }
        if call.output_chars > LARGE_RESULT_CHARS {
            found.push(Anomaly {
                kind: "large_result",
                step: call.step,
                detail: format!(
                    "{} returned {} characters",
                    describe(call),
                    call.output_chars,
                ),
            });
        }
    }

    for (ordinal, gap) in stalls(trace) {
        found.push(Anomaly {
            kind: "stall",
            step: ordinal,
            detail: format!(
                "{}s with no record between steps {} and the next",
                gap / 1_000,
                ordinal
            ),
        });
    }

    let mut prompts: Vec<u64> = trace
        .steps
        .iter()
        .filter_map(|step| step.prompt_tokens)
        .collect();
    prompts.sort_unstable();
    let median = prompts.get(prompts.len() / 2).copied().unwrap_or(0);
    let outlier_at = (4 * median).max(OUTLIER_TOKENS);
    for step in &trace.steps {
        if let Some(prompt) = step.prompt_tokens
            && prompt > outlier_at
        {
            found.push(Anomaly {
                kind: "outlier",
                step: step.ordinal,
                detail: format!(
                    "step {} prompted {prompt} tokens against a {median} median",
                    step.ordinal
                ),
            });
        }
        if step.source == Source::Agent && !step.spoke {
            // An agent step with no text is fine when it made a call; an
            // empty one is a malformed turn.
            let called = trace.calls.iter().any(|call| call.step == step.ordinal);
            if !called {
                found.push(Anomaly {
                    kind: "empty_step",
                    step: step.ordinal,
                    detail: format!(
                        "agent step {} carries no message, reasoning, or call",
                        step.ordinal
                    ),
                });
            }
        }
    }

    for fault in &trace.faults {
        found.push(Anomaly {
            kind: "fault",
            step: 0,
            detail: fault.clone(),
        });
    }

    found.sort_by_key(|anomaly| anomaly.step);
    found
}

/// The stats report as text: a header line, then the tables.
#[must_use]
pub fn stats_text(trace: &Trace) -> String {
    let report = stats(trace);
    let mut out = String::new();
    out.push_str(&format!(
        "{} ({})\n",
        trace.source_path.display(),
        trace.kind.word()
    ));
    out.push_str(&format!(
        "session {}  state {}  wall {}s  steps {}  calls {} ({} failed, {} cancelled, {} decision)\n",
        trace.session_id,
        if trace.state.is_empty() {
            "unknown"
        } else {
            &trace.state
        },
        trace.seconds.unwrap_or(0),
        trace.steps.len(),
        trace.calls.len(),
        report["calls_failed"],
        report["calls_cancelled"],
        report["decision_calls"],
    ));
    if !trace.models.is_empty() {
        out.push_str(&format!("models: {}\n", trace.models.join(", ")));
    }
    let tokens = &report["tokens"];
    out.push_str(&format!(
        "tokens: prompt {}  completion {}  median step prompt {}\n",
        tokens["prompt"], tokens["completion"], tokens["median_step_prompt"]
    ));
    if let Some(tools) = report["tools"].as_array()
        && !tools.is_empty()
    {
        out.push_str("\ncalls by tool:\n");
        for tool in tools {
            out.push_str(&format!(
                "  {:<28} {:>5} calls  {:>4} not completed  {:>8.1}s\n",
                tool["name"].as_str().unwrap_or("?"),
                tool["calls"].as_u64().unwrap_or(0),
                tool["not_completed"].as_u64().unwrap_or(0),
                tool["ms"].as_u64().unwrap_or(0) as f64 / 1_000.0,
            ));
        }
    }
    if let Some(repeated) = report["repeated"].as_array()
        && !repeated.is_empty()
    {
        out.push_str("\nrepeated identical calls:\n");
        for group in repeated.iter().take(15) {
            out.push_str(&format!(
                "  {:<60} {}x  steps {}\n",
                group["what"].as_str().unwrap_or("?"),
                group["executions"].as_u64().unwrap_or(0),
                group["steps"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_u64)
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            ));
        }
    }
    if let Some(largest) = report["largest_results"].as_array()
        && !largest.is_empty()
    {
        out.push_str("\nlargest tool results:\n");
        for result in largest {
            out.push_str(&format!(
                "  step {:<5} {:<20} {} chars\n",
                result["step"], result["name"], result["chars"],
            ));
        }
    }
    out
}

/// The anomalies as text, one finding per line.
#[must_use]
pub fn anomalies_text(trace: &Trace) -> String {
    let mut out = String::new();
    let found = anomalies(trace);
    if found.is_empty() {
        out.push_str("nothing anomalous\n");
        return out;
    }
    for anomaly in found {
        out.push_str(&format!(
            "{:<14} step {:<5} {}\n",
            anomaly.kind, anomaly.step, anomaly.detail
        ));
    }
    out
}

/// A run directory's summary as text: the outcome line and the per-hand
/// table `summary.json` already carries.
#[must_use]
pub fn run_dir_text(dir: &RunDir, path: &Path) -> String {
    let mut out = String::new();
    out.push_str(&format!("{} (run directory)\n", path.display()));
    if let Some(summary) = &dir.summary {
        out.push_str(&format!(
            "issue #{}  {}  wall {:.1}s  cost ${}\n",
            summary["issue"].as_u64().unwrap_or(0),
            summary["outcome"]["status"].as_str().unwrap_or("?"),
            summary["wall_ms"].as_f64().unwrap_or(0.0) / 1_000.0,
            summary["cost"]["total_usd"].as_f64().unwrap_or(0.0),
        ));
        let rows = hands(summary);
        if !rows.is_empty() {
            out.push_str("\nhands:\n");
            for hand in rows {
                let verdict = hand["verdict"]
                    .as_str()
                    .and_then(|v| v.lines().last())
                    .unwrap_or("")
                    .chars()
                    .take(70)
                    .collect::<String>();
                out.push_str(&format!(
                    "  {:<8} round {}  {:>7.1}s  {:>3} turns  ${:<8.4}  in {:>6} out {:>6}  {}\n",
                    hand["hand"].as_str().unwrap_or("?"),
                    hand["round"].as_u64().unwrap_or(0),
                    hand["wall_ms"].as_f64().unwrap_or(0.0) / 1_000.0,
                    hand["turns"].as_u64().unwrap_or(0),
                    hand["cost_usd"].as_f64().unwrap_or(0.0),
                    hand["input_tokens"].as_u64().unwrap_or(0),
                    hand["output_tokens"].as_u64().unwrap_or(0),
                    verdict,
                ));
            }
        }
    }
    for (file, trace) in &dir.traces {
        let report = stats(trace);
        out.push_str(&format!(
            "\n{}: {} records, {} calls, {} anomalies\n",
            file.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default(),
            trace.steps.len(),
            trace.calls.len(),
            anomalies(trace)
                .len()
                .max(report["stalls_over_60s"].as_u64().unwrap_or(0) as usize),
        ));
    }
    out
}
