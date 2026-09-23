//! An episode as a timeline of component invocations.
//!
//! Coder One writes a start and an end event for every component
//! invocation (`openagents.coder-one.invocation.v1`) into its durable
//! episode log, `episode.atif.jsonl`, and the trajectory derived from it
//! carries the same events. This module reads either one into a timeline:
//! every invocation in start order, with its parent, component, duration,
//! cost, and the spend accumulated so far.
//!
//! An attempt recorded before invocations existed has only trajectory
//! steps. Its timeline is derived from them, one entry per Jev request,
//! briefing, delegation, and closing check, and it says so: derived timing
//! is when each answer arrived, not when each component started.
//!
//! A log without an end record, or an invocation without an end event, is
//! incomplete, and the timeline says which invocations never ended.
//!
//! An executor session's normalized events (`executor_event` steps:
//! command started and completed, artifact changed, assistant claim, usage
//! update, session started and ended) appear inline among the
//! invocations, each with its session, process generation, sequence
//! number, workspace revision, and native stream line.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::terminal_bench::{Attempt, timestamp_ms};

/// The schema every invocation event carries.
pub const INVOCATION_SCHEMA: &str = "openagents.coder-one.invocation.v1";

/// The schema of the timeline's JSON.
pub const TIMELINE_SCHEMA: &str = "openagents.gym.coder-timeline.v1";

/// Jev's published rate, dollars per million input tokens, for a derived
/// timeline's Jev entries. The rate Coder One's usage record applies.
const JEV_USD_PER_MILLION_INPUT: f64 = 0.042;

/// Where a timeline came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The durable episode log.
    Log,
    /// Invocation events inside a trajectory document.
    Trajectory,
    /// Derived from trajectory steps; no invocation events were recorded.
    Derived,
}

impl Source {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Source::Log => "invocation log",
            Source::Trajectory => "trajectory invocation events",
            Source::Derived => "derived from trajectory steps",
        }
    }
}

/// One invocation.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub parent: Option<String>,
    /// How many ancestors it has.
    pub depth: usize,
    pub component: String,
    pub name: Option<String>,
    pub implementation: Option<String>,
    /// Milliseconds since the timeline's first event.
    pub offset_ms: u64,
    pub duration_ms: Option<u64>,
    pub cost_usd: Option<f64>,
    pub cost_provenance: Option<String>,
    /// `completed`, `failed`, `skipped`, or `unknown` when no end was
    /// recorded.
    pub outcome: String,
    pub effects: bool,
    /// A typed host operation's declared effect class: `observe`, `write`,
    /// or `install`.
    pub effect_class: Option<String>,
    /// Spend accumulated over every entry up to and including this one;
    /// `None` once an unknown cost has been passed.
    pub accumulated_usd: Option<f64>,
    /// The end event's output summary, when one was recorded.
    pub output: Value,
    /// The digest of the output, when the end event recorded one.
    pub output_digest: Option<String>,
    /// When it started, in milliseconds since the epoch.
    pub started_at: Option<u64>,
}

/// One normalized executor event.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutorEvent {
    /// Milliseconds since the timeline's first event.
    pub offset_ms: u64,
    /// When it was recorded, in milliseconds since the epoch.
    pub at: u64,
    pub adapter: Option<String>,
    pub session_id: Option<String>,
    pub generation: Option<u64>,
    /// The controller's sequence number.
    pub seq: Option<u64>,
    /// The workspace revision the event was observed at.
    pub revision: Option<u64>,
    /// `command_started`, `assistant_claim`, and so on.
    pub kind: String,
    /// A short description, such as the command or the claim.
    pub summary: String,
    /// The native stream line it came from.
    pub line: Option<u64>,
    /// The event as recorded.
    pub record: Value,
}

impl ExecutorEvent {
    /// Reads one `executor_event` extension recorded at `at`.
    #[must_use]
    pub fn read(record: &Value, at: u64) -> Option<Self> {
        let event = record.get("event")?;
        let kind = event.get("kind").and_then(Value::as_str)?.to_owned();
        let text = |key: &str| event.get(key).and_then(Value::as_str).unwrap_or_default();
        let summary = match kind.as_str() {
            "session_started" => format!("session {}", text("session_id")),
            "command_started" => text("command").to_owned(),
            "command_completed" => format!(
                "exit {} · {}",
                event
                    .get("exit_code")
                    .and_then(Value::as_i64)
                    .map_or("?".to_owned(), |code| code.to_string()),
                text("command")
            ),
            "artifact_changed" => format!("{} {}", text("change"), text("path")),
            "assistant_claim" => text("text").to_owned(),
            "usage_update" => {
                let usage = &event["usage"];
                let get = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
                format!(
                    "in {} · cached {} · out {}",
                    get("input_tokens"),
                    get("cache_read_input_tokens") + get("cached_input_tokens"),
                    get("output_tokens")
                )
            }
            "session_ended" => format!(
                "{}{}",
                if event.get("error").and_then(Value::as_bool) == Some(true) {
                    "error · "
                } else {
                    ""
                },
                text("result")
            ),
            _ => String::new(),
        };
        let summary: String = summary
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(120)
            .collect();
        let number = |key: &str| record.get(key).and_then(Value::as_u64);
        Some(ExecutorEvent {
            offset_ms: 0,
            at,
            adapter: record
                .get("adapter")
                .and_then(Value::as_str)
                .map(str::to_owned),
            session_id: record
                .get("session_id")
                .and_then(Value::as_str)
                .map(str::to_owned),
            generation: number("generation"),
            seq: number("seq").or_else(|| event.get("seq").and_then(Value::as_u64)),
            revision: number("revision"),
            kind,
            summary,
            line: event.get("line").and_then(Value::as_u64),
            record: record.clone(),
        })
    }

    /// The event as one text row.
    #[must_use]
    pub fn row(&self) -> String {
        format!(
            "  {:>7}  {:>7}  ▸ {:<18} {}{}",
            seconds(Some(self.offset_ms)),
            self.generation
                .map_or(String::new(), |generation| format!("g{generation}")),
            self.kind,
            self.summary,
            match (self.seq, self.revision, self.line) {
                (Some(seq), Some(revision), Some(line)) => {
                    format!("  [#{seq} rev {revision} line {line}]")
                }
                (Some(seq), _, Some(line)) => format!("  [#{seq} line {line}]"),
                _ => String::new(),
            }
        )
    }

    /// The event as versioned JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "offset_ms": self.offset_ms,
            "at": self.at,
            "adapter": self.adapter,
            "session_id": self.session_id,
            "generation": self.generation,
            "seq": self.seq,
            "revision": self.revision,
            "kind": self.kind,
            "summary": self.summary,
            "line": self.line,
        })
    }
}

/// An episode's timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    pub source: Source,
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    /// The executor's normalized events, in the order they arrived.
    pub events: Vec<ExecutorEvent>,
    /// Whether the log ended and every invocation ended.
    pub complete: bool,
    /// Readable-prefix faults in the log, such as a torn last line.
    pub faults: usize,
    pub notes: Vec<String>,
}

impl Timeline {
    /// Leaf spend over the whole timeline, or `None` when a cost is
    /// unknown.
    #[must_use]
    pub fn total_usd(&self) -> Option<f64> {
        self.entries.last().and_then(|entry| entry.accumulated_usd)
    }

    /// The timeline as versioned JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema": TIMELINE_SCHEMA,
            "source": self.source.label(),
            "path": self.path.display().to_string(),
            "complete": self.complete,
            "faults": self.faults,
            "total_usd": self.total_usd(),
            "notes": self.notes,
            "executor_events": self.events.iter().map(ExecutorEvent::to_json).collect::<Vec<_>>(),
            "invocations": self.entries.iter().map(|entry| json!({
                "id": entry.id,
                "parent": entry.parent,
                "depth": entry.depth,
                "component": entry.component,
                "name": entry.name,
                "implementation": entry.implementation,
                "offset_ms": entry.offset_ms,
                "duration_ms": entry.duration_ms,
                "cost_usd": entry.cost_usd,
                "cost_provenance": entry.cost_provenance,
                "outcome": entry.outcome,
                "effects": entry.effects,
                "effect_class": entry.effect_class,
                "accumulated_usd": entry.accumulated_usd,
            })).collect::<Vec<_>>(),
        })
    }

    /// The timeline as text rows, for the CLI and the terminal.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "Episode timeline · {} · {} · {} invocations · spend {}",
            self.source.label(),
            if self.complete {
                "complete"
            } else {
                "INCOMPLETE"
            },
            self.entries.len(),
            money(self.total_usd()),
        )];
        lines.extend(self.notes.iter().map(|note| format!("  {note}")));
        if !self.events.is_empty() {
            lines.push(format!(
                "  {} executor events inline, marked ▸: generation, kind, what, and [#sequence, workspace revision, native line]",
                self.events.len()
            ));
        }
        lines.push(
            "  start     took     component / name                                   outcome     cost        spend      parent"
                .to_owned(),
        );
        let mut events = self.events.iter().peekable();
        for entry in &self.entries {
            while let Some(event) = events.next_if(|event| event.offset_ms < entry.offset_ms) {
                lines.push(event.row());
            }
            let label = format!(
                "{}{}{}{}",
                "  ".repeat(entry.depth),
                entry
                    .effect_class
                    .as_deref()
                    .map_or(String::new(), |class| format!("[{class}] ")),
                entry.component,
                entry
                    .name
                    .as_deref()
                    .map_or(String::new(), |name| format!(" · {name}"))
            );
            lines.push(format!(
                "  {:>7}  {:>7}  {:<50} {:<10}  {:<10}  {:<9}  {}",
                seconds(Some(entry.offset_ms)),
                seconds(entry.duration_ms),
                clip(&label, 50),
                if entry.effects && entry.outcome == "unknown" {
                    "unknown!".to_owned()
                } else {
                    entry.outcome.clone()
                },
                money(entry.cost_usd),
                money(entry.accumulated_usd),
                entry.parent.as_deref().unwrap_or("—"),
            ));
        }
        lines.extend(events.map(ExecutorEvent::row));
        lines
    }
}

fn clip(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

fn seconds(value: Option<u64>) -> String {
    value.map_or("—".to_owned(), |ms| format!("{:.1}s", ms as f64 / 1000.0))
}

fn money(value: Option<f64>) -> String {
    value.map_or("—".to_owned(), |usd| format!("${usd:.4}"))
}

/// One invocation event and when it happened.
struct Event {
    at: u64,
    record: Value,
}

/// Reads an episode log (`*.atif.jsonl`).
///
/// # Errors
///
/// Returns a message when the file doesn't read as a session log.
pub fn read_log(path: &Path) -> Result<Timeline, String> {
    let recording =
        atif::log::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let events: Vec<Event> = recording
        .steps
        .iter()
        .filter_map(|step| {
            let record = step.extensions.get("invocation")?;
            (record.get("schema").and_then(Value::as_str) == Some(INVOCATION_SCHEMA)).then(|| {
                Event {
                    at: step.at,
                    record: record.clone(),
                }
            })
        })
        .collect();
    let mut timeline = build(Source::Log, path, &events);
    let executor: Vec<ExecutorEvent> = recording
        .steps
        .iter()
        .filter_map(|step| ExecutorEvent::read(step.extensions.get(EXECUTOR_EVENT_KEY)?, step.at))
        .collect();
    attach(&mut timeline, executor);
    timeline.faults = recording.faults.len();
    if !recording.ended() {
        timeline.complete = false;
        timeline.notes.insert(
            0,
            "The log has no end record: the episode was interrupted or is still running."
                .to_owned(),
        );
    }
    if timeline.faults > 0 {
        timeline.complete = false;
        timeline.notes.push(format!(
            "{} unreadable lines; the timeline is the readable prefix.",
            timeline.faults
        ));
    }
    Ok(timeline)
}

/// Reads an ATIF trajectory document: its invocation events when it has
/// them, and a timeline derived from its steps when it doesn't.
///
/// # Errors
///
/// Returns a message when the file isn't an ATIF document.
pub fn read_trajectory(path: &Path) -> Result<Timeline, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let document: Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    let steps = document
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{}: no steps", path.display()))?;
    let events: Vec<Event> = steps
        .iter()
        .filter_map(|step| {
            let record = step.pointer("/extra/invocation")?;
            (record.get("schema").and_then(Value::as_str) == Some(INVOCATION_SCHEMA)).then(|| {
                Event {
                    at: step_ms(step).unwrap_or_default(),
                    record: record.clone(),
                }
            })
        })
        .collect();
    let executor: Vec<ExecutorEvent> = steps
        .iter()
        .filter_map(|step| {
            ExecutorEvent::read(
                step.get("extra")?.get(EXECUTOR_EVENT_KEY)?,
                step_ms(step).unwrap_or_default(),
            )
        })
        .collect();
    let mut timeline = if events.is_empty() {
        derive(path, steps)
    } else {
        build(Source::Trajectory, path, &events)
    };
    attach(&mut timeline, executor);
    Ok(timeline)
}

/// The step extension that holds one normalized executor event.
pub const EXECUTOR_EVENT_KEY: &str = "executor_event";

/// Places executor events on the timeline's clock, in arrival order.
fn attach(timeline: &mut Timeline, mut executor: Vec<ExecutorEvent>) {
    // The timeline's origin is where its entries' offsets count from.
    let first = timeline
        .entries
        .iter()
        .find_map(|entry| Some(entry.started_at?.saturating_sub(entry.offset_ms)))
        .or_else(|| executor.iter().map(|event| event.at).min())
        .unwrap_or(0);
    for event in &mut executor {
        event.offset_ms = event.at.saturating_sub(first);
    }
    executor.sort_by_key(|event| (event.at, event.seq));
    timeline.events = executor;
}

fn step_ms(step: &Value) -> Option<u64> {
    step.get("timestamp")
        .and_then(Value::as_str)
        .and_then(timestamp_ms)
        .and_then(|ms| u64::try_from(ms).ok())
}

fn build(source: Source, path: &Path, events: &[Event]) -> Timeline {
    let mut entries: Vec<Entry> = Vec::new();
    let mut ends: BTreeMap<String, &Value> = BTreeMap::new();
    let first = events.iter().map(|event| event.at).min().unwrap_or(0);
    for event in events {
        let record = &event.record;
        let text = |key: &str| record.get(key).and_then(Value::as_str).map(str::to_owned);
        match record.get("event").and_then(Value::as_str) {
            Some("start") => {
                let at = record.get("at").and_then(Value::as_u64).unwrap_or(event.at);
                entries.push(Entry {
                    id: text("id").unwrap_or_default(),
                    parent: text("parent"),
                    depth: 0,
                    component: text("component").unwrap_or_default(),
                    name: text("name"),
                    implementation: record
                        .pointer("/implementation/digest")
                        .and_then(Value::as_str)
                        .map(|digest| digest.chars().take(12).collect()),
                    offset_ms: at.saturating_sub(first),
                    duration_ms: None,
                    cost_usd: None,
                    cost_provenance: None,
                    outcome: "unknown".to_owned(),
                    effects: record.get("effects").and_then(Value::as_bool) == Some(true),
                    effect_class: text("effect_class"),
                    accumulated_usd: None,
                    output: Value::Null,
                    output_digest: None,
                    started_at: Some(at),
                });
            }
            Some("end") => {
                if let Some(id) = record.get("id").and_then(Value::as_str) {
                    ends.insert(id.to_owned(), record);
                }
            }
            _ => {}
        }
    }
    for entry in &mut entries {
        if let Some(end) = ends.get(&entry.id) {
            entry.outcome = end
                .get("outcome")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned();
            entry.duration_ms = end.get("milliseconds").and_then(Value::as_u64);
            entry.cost_usd = end.pointer("/cost/usd").and_then(Value::as_f64);
            entry.cost_provenance = end
                .pointer("/cost/provenance")
                .and_then(Value::as_str)
                .map(str::to_owned);
            entry.output = end
                .pointer("/output/summary")
                .cloned()
                .unwrap_or(Value::Null);
            entry.output_digest = end
                .pointer("/output/digest")
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
    }
    finish(source, path, entries)
}

/// Depths, accumulated spend, completeness, and notes.
fn finish(source: Source, path: &Path, mut entries: Vec<Entry>) -> Timeline {
    let parents: BTreeMap<String, Option<String>> = entries
        .iter()
        .map(|entry| (entry.id.clone(), entry.parent.clone()))
        .collect();
    for entry in &mut entries {
        let mut depth = 0;
        let mut cursor = entry.parent.clone();
        while let Some(parent) = cursor {
            depth += 1;
            cursor = parents.get(&parent).cloned().flatten();
            if depth > 32 {
                break;
            }
        }
        entry.depth = depth;
    }
    // Leaves carry costs; a parent records none, so a sum of recorded
    // costs counts each dollar once. A leaf whose cost is unknown makes
    // the running total unknown from there on.
    let has_children: Vec<bool> = entries
        .iter()
        .map(|entry| {
            entries
                .iter()
                .any(|other| other.parent.as_deref() == Some(&entry.id))
        })
        .collect();
    let mut total = Some(0.0);
    for (entry, parent) in entries.iter_mut().zip(has_children) {
        if !parent || entry.cost_usd.is_some() {
            total = match (total, entry.cost_usd, entry.cost_provenance.as_deref()) {
                (Some(sum), Some(cost), _) => Some(sum + cost),
                // A leaf that recorded no cost at all, such as an
                // operation or an unfinished invocation, adds nothing it
                // claims; an explicit unknown cost makes the total unknown.
                (Some(sum), None, None) if entry.outcome != "unknown" => Some(sum),
                _ => None,
            };
        }
        entry.accumulated_usd = total;
    }
    let unfinished: Vec<&Entry> = entries
        .iter()
        .filter(|entry| entry.outcome == "unknown")
        .collect();
    let mut notes = Vec::new();
    if !unfinished.is_empty() {
        notes.push(format!(
            "{} invocations never ended ({} with effects, whose result is unknown): {}",
            unfinished.len(),
            unfinished.iter().filter(|entry| entry.effects).count(),
            unfinished
                .iter()
                .map(|entry| format!("{} {}", entry.id, entry.component))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Timeline {
        source,
        path: path.to_path_buf(),
        complete: unfinished.is_empty(),
        entries,
        events: Vec::new(),
        faults: 0,
        notes,
    }
}

/// The component a retained call belongs to.
fn component_of(name: &str) -> &'static str {
    match name {
        "jev_setup" => "evidence.setup",
        "jev_probe" => "evidence.probes",
        "jev_survey" => "evidence.select",
        "jev_close" => "verify.close",
        "delegate" => "exec.session",
        _ => "exec.explore",
    }
}

/// A timeline derived from trajectory steps that carry no invocation
/// events: an `episode` root, and one entry per call, briefing, and
/// generation.
fn derive(path: &Path, steps: &[Value]) -> Timeline {
    let times: Vec<u64> = steps.iter().filter_map(step_ms).collect();
    let first = times.iter().copied().min().unwrap_or(0);
    let last = times.iter().copied().max().unwrap_or(first);
    let mut entries = vec![Entry {
        id: "derived-0".to_owned(),
        parent: None,
        depth: 0,
        component: "episode".to_owned(),
        name: Some("derived".to_owned()),
        implementation: None,
        offset_ms: 0,
        duration_ms: Some(last - first),
        cost_usd: None,
        cost_provenance: None,
        outcome: "completed".to_owned(),
        effects: true,
        effect_class: None,
        accumulated_usd: None,
        output: Value::Null,
        output_digest: None,
        started_at: Some(first),
    }];
    let mut add = |component: &str,
                   name: String,
                   end: u64,
                   duration: Option<u64>,
                   cost: Option<(Option<f64>, String)>,
                   outcome: &str| {
        let number = entries.len();
        entries.push(Entry {
            id: format!("derived-{number}"),
            parent: Some("derived-0".to_owned()),
            depth: 1,
            component: component.to_owned(),
            name: Some(name),
            implementation: None,
            offset_ms: end
                .saturating_sub(duration.unwrap_or(0))
                .saturating_sub(first),
            duration_ms: duration,
            cost_usd: cost.as_ref().and_then(|(usd, _)| *usd),
            cost_provenance: cost.map(|(_, provenance)| provenance),
            outcome: outcome.to_owned(),
            effects: component == "exec.session",
            effect_class: None,
            accumulated_usd: None,
            output: Value::Null,
            output_digest: None,
            started_at: Some(end.saturating_sub(duration.unwrap_or(0))),
        });
    };
    for step in steps {
        let at = step_ms(step).unwrap_or(first);
        let calls = step.get("tool_calls").and_then(Value::as_array);
        if let Some(message) = step
            .get("message")
            .and_then(Value::as_str)
            .filter(|message| message.starts_with("Delegating to "))
        {
            let chars = message
                .split("Briefing: ")
                .nth(1)
                .and_then(|rest| rest.split(' ').next())
                .unwrap_or("?");
            add(
                "evidence.pack",
                format!("briefing, {chars} characters"),
                at,
                Some(0),
                Some((Some(0.0), "none".to_owned())),
                "completed",
            );
            continue;
        }
        for call in calls.into_iter().flatten() {
            let name = call
                .get("function_name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let id = call
                .get("tool_call_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let result = step
                .pointer("/observation/results")
                .and_then(Value::as_array)
                .and_then(|results| {
                    results.iter().find(|result| {
                        result.get("source_call_id").and_then(Value::as_str) == Some(id)
                    })
                });
            let duration = result
                .and_then(|result| result.pointer("/extra/duration_ms"))
                .or_else(|| step.pointer("/extra/duration_ms"))
                .and_then(Value::as_u64);
            let status = result
                .and_then(|result| result.pointer("/extra/status"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let outcome = if status == "completed" {
                "completed"
            } else {
                "failed"
            };
            let cost = if name == "delegate" {
                let usd = call
                    .pointer("/extra/total_cost_usd")
                    .and_then(Value::as_f64);
                Some((
                    usd,
                    call.pointer("/extra/cost_provenance")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_owned(),
                ))
            } else if name.starts_with("jev_") {
                let tokens = step
                    .pointer("/extra/jev_usage/input_tokens")
                    .and_then(Value::as_u64);
                Some(match tokens {
                    Some(tokens) => (
                        Some(tokens as f64 * JEV_USD_PER_MILLION_INPUT / 1_000_000.0),
                        "price_estimate".to_owned(),
                    ),
                    None => (None, "unknown".to_owned()),
                })
            } else {
                None
            };
            add(
                component_of(name),
                format!("{name} {id}"),
                at,
                duration,
                cost,
                outcome,
            );
        }
        if calls.is_none_or(Vec::is_empty)
            && step.get("source").and_then(Value::as_str) == Some("agent")
            && step.pointer("/metrics/prompt_tokens").is_some()
        {
            let cost = step
                .pointer("/extra/cost_microusd")
                .and_then(Value::as_u64)
                .map(|micro| {
                    (
                        Some(micro as f64 / 1_000_000.0),
                        "provider_reported".to_owned(),
                    )
                })
                .or(Some((None, "unknown".to_owned())));
            add(
                "exec.explore",
                "generation".to_owned(),
                at,
                step.pointer("/extra/duration_ms").and_then(Value::as_u64),
                cost,
                "completed",
            );
        }
    }
    let mut timeline = finish(Source::Derived, path, entries);
    timeline.notes.insert(
        0,
        "Derived from trajectory steps: this attempt recorded no invocation events. Each entry ends when its answer arrived; setup, probe, and survey commands are not separately timed.".to_owned(),
    );
    timeline
}

/// The timeline for an attempt: its invocation log when one was retained,
/// else its trajectory's events, else one derived from its trajectory.
#[must_use]
pub fn for_attempt(attempt: &Attempt) -> Option<Result<Timeline, String>> {
    let path_of = |kind: &str| {
        attempt
            .evidence
            .iter()
            .find(|evidence| evidence.kind == kind)
            .and_then(|evidence| evidence.path.clone())
            .filter(|path| path.is_file())
    };
    if let Some(log) = path_of("invocation_log") {
        return Some(read_log(&log));
    }
    path_of("trajectory").map(|path| read_trajectory(&path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retained() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../bench/terminal-bench/traces/panel--coder-one-jevprobe3-luna--build-cython-ext/build-cython-ext__jFQbtoW.json",
        )
    }

    #[test]
    fn executor_events_appear_inline_between_invocations() {
        let dir = std::env::temp_dir().join(format!("gym-timeline-events-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("episode.atif.jsonl");
        let session = atif::Session::opening("e", "none", "mini-task", "/w", "v");
        let mut log = atif::Log::create_at(&path, &session).unwrap();
        let step = |at: u64, extensions: Value| -> atif::document::Step {
            serde_json::from_value(json!({
                "at": at, "source": "System", "message": "", "extensions": extensions
            }))
            .unwrap()
        };
        let invocation = |event: &str, id: &str, component: &str, at: u64| {
            json!({ "invocation": {
                "schema": INVOCATION_SCHEMA, "event": event, "id": id, "component": component,
                "at": at, "outcome": "completed", "milliseconds": 100,
            }})
        };
        let executor = |seq: u64, revision: u64, event: Value| {
            json!({ "executor_event": {
                "schema": "openagents.coder-one.executor-event.v1", "adapter": "codex",
                "session_id": "s-1", "generation": 1, "seq": seq, "revision": revision,
                "at_ms": seq * 10, "event": event,
            }})
        };
        log.append(&step(1_000, invocation("start", "inv-1", "episode", 1_000)))
            .unwrap();
        log.append(&step(
            1_010,
            invocation("start", "inv-2", "exec.session", 1_010),
        ))
        .unwrap();
        log.append(&step(
            1_020,
            executor(
                1,
                0,
                json!({"seq": 1, "line": 3, "kind": "command_started", "command": "pytest -q"}),
            ),
        ))
        .unwrap();
        log.append(&step(
            1_030,
            executor(2, 1, json!({"seq": 2, "line": 5, "kind": "artifact_changed", "path": "run.py", "change": "write"})),
        ))
        .unwrap();
        log.append(&step(
            1_050,
            invocation("start", "inv-3", "verify.close", 1_050),
        ))
        .unwrap();
        log.append(&step(
            1_060,
            invocation("end", "inv-3", "verify.close", 1_060),
        ))
        .unwrap();
        log.append(&step(
            1_070,
            invocation("end", "inv-2", "exec.session", 1_070),
        ))
        .unwrap();
        log.append(&step(1_080, invocation("end", "inv-1", "episode", 1_080)))
            .unwrap();
        log.finish(atif::log::ENDED).unwrap();
        let timeline = read_log(&path).unwrap();
        assert_eq!(timeline.events.len(), 2);
        assert_eq!(timeline.events[0].offset_ms, 20);
        assert_eq!(timeline.events[1].revision, Some(1));
        let lines = timeline.lines();
        let at = |needle: &str| lines.iter().position(|line| line.contains(needle)).unwrap();
        assert!(at("exec.session") < at("▸ command_started"));
        assert!(at("▸ artifact_changed") < at("verify.close"));
        assert!(lines[at("▸ command_started")].contains("pytest -q"));
        assert!(lines[at("▸ artifact_changed")].contains("[#2 rev 1 line 5]"));
        let value = timeline.to_json();
        assert_eq!(value["executor_events"][0]["kind"], "command_started");
        assert_eq!(value["executor_events"][1]["session_id"], "s-1");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_retained_v3_trial_derives_the_same_timeline_twice() {
        let first = read_trajectory(&retained()).unwrap();
        let second = read_trajectory(&retained()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.source, Source::Derived);
        let components: Vec<&str> = first.entries.iter().map(|e| e.component.as_str()).collect();
        assert_eq!(
            components,
            [
                "episode",
                "evidence.setup",
                "evidence.probes",
                "evidence.select",
                "evidence.select",
                "evidence.pack",
                "exec.session",
                "verify.close"
            ]
        );
        // The delegate's list-price cost and the Jev estimates add up.
        let total = first.total_usd().unwrap();
        assert!(total > 0.0);
        assert!(
            first
                .lines()
                .iter()
                .any(|line| line.contains("exec.session"))
        );
    }

    fn event(
        kind: &str,
        id: &str,
        parent: Option<&str>,
        component: &str,
        at: u64,
        extra: Value,
    ) -> String {
        let mut record = json!({
            "schema": INVOCATION_SCHEMA, "event": kind, "id": id, "parent": parent,
            "component": component, "at": at, "effects": component == "exec.session",
        });
        if let (Some(record), Some(extra)) = (record.as_object_mut(), extra.as_object()) {
            record.extend(extra.clone());
        }
        json!({ "record": "step", "step": { "at": at, "source": "System", "message": "", "extensions": { "invocation": record } } }).to_string()
    }

    #[test]
    fn a_killed_episode_reads_as_an_incomplete_prefix() {
        let dir = std::env::temp_dir().join(format!("gym-timeline-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("episode.atif.jsonl");
        let session = atif::Session::opening("killed", "free", "door", "/r", "v");
        drop(atif::Log::create_at(&path, &session).unwrap());
        let lines = [
            event("start", "inv-1", None, "episode", 1000, json!({})),
            event(
                "start",
                "inv-2",
                Some("inv-1"),
                "evidence.select",
                1100,
                json!({}),
            ),
            event(
                "start",
                "inv-3",
                Some("inv-1"),
                "evidence.select",
                1101,
                json!({}),
            ),
            event(
                "end",
                "inv-3",
                None,
                "evidence.select",
                1300,
                json!({ "outcome": "completed", "milliseconds": 199, "cost": { "usd": 0.0001, "provenance": "price_estimate" } }),
            ),
            event(
                "end",
                "inv-2",
                None,
                "evidence.select",
                1400,
                json!({ "outcome": "completed", "milliseconds": 300, "cost": { "usd": 0.0002, "provenance": "price_estimate" } }),
            ),
            event(
                "start",
                "inv-4",
                Some("inv-1"),
                "exec.session",
                1500,
                json!({}),
            ),
            event(
                "start",
                "inv-5",
                Some("inv-1"),
                "host.operation",
                1501,
                json!({ "name": "git status", "effect_class": "observe" }),
            ),
        ];
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str(&lines.join("\n"));
        text.push_str("\n{\"record\":\"step\",\"st");
        std::fs::write(&path, text).unwrap();
        let timeline = read_log(&path).unwrap();
        assert!(!timeline.complete);
        assert_eq!(timeline.faults, 1);
        assert_eq!(timeline.entries.len(), 5);
        assert_eq!(timeline.entries[4].effect_class.as_deref(), Some("observe"));
        assert!(
            timeline
                .lines()
                .iter()
                .any(|line| line.contains("[observe] host.operation · git status"))
        );
        // Two concurrent requests of one component differ by ID.
        assert_ne!(timeline.entries[1].id, timeline.entries[2].id);
        assert_eq!(timeline.entries[3].outcome, "unknown");
        assert!(timeline.entries[3].effects);
        assert!(
            timeline
                .notes
                .iter()
                .any(|note| note.contains("never ended"))
        );
        assert!(timeline.lines()[0].contains("INCOMPLETE"));
        let spend = timeline.entries[2].accumulated_usd.unwrap();
        assert!((spend - 0.0003).abs() < 1e-12);
        let _ = std::fs::remove_dir_all(dir);
    }
}
