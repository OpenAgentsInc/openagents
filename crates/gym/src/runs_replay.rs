//! Full retained transcripts on an elapsed-time clock. Reading a replay
//! never executes a command from a trace or reaches a provider.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::runs::{Catalog, Run};
use crate::terminal_bench::timestamp_ms;

pub const SCHEMA: &str = "openagents.gym.public-replays.v1";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PublicTrial {
    pub id: String,
    pub task: String,
    pub model: String,
    pub agent: String,
    pub agent_version: Option<String>,
    pub effort: String,
    pub source_url: String,
    pub file: String,
    pub sha256: Option<String>,
    pub available: Option<bool>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub reward: Option<f64>,
    pub cost_usd: Option<f64>,
}

#[derive(Clone, Debug)]
pub enum Source {
    Local(Box<Run>),
    Public {
        trial: Box<PublicTrial>,
        cache: PathBuf,
    },
}

impl Source {
    pub fn started_ms(&self) -> Option<i64> {
        match self {
            Self::Local(run) => run.started_ms,
            Self::Public { trial, .. } => trial.started_at.as_deref().and_then(timestamp_ms),
        }
    }

    /// Checks a public body's identity before either replay or analysis reads it.
    pub(crate) fn verify(&self) -> Result<(), String> {
        let Self::Public { trial, cache } = self else {
            return Ok(());
        };
        let path = cache.join(&trial.file);
        let bytes = std::fs::read(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!(
                    "Fable transcript is not on this computer.\n\nGit includes the attempt list, but transcript files must be downloaded on each computer.\n\nFrom the openagents repository, run:\ncd bench/terminal-bench\nuv run python -m tbench.public_replays\n\nThen press Esc and Enter to reload the pair.\n\nFile: {}",
                    path.display()
                )
            } else {
                format!("Cannot read Fable transcript: {e}\nFile: {}", path.display())
            }
        })?;
        let actual = format!("{:x}", Sha256::digest(&bytes));
        if trial.sha256.as_deref() != Some(&actual) {
            return Err(format!("Integrity check failed: {}", path.display()));
        }
        Ok(())
    }

    pub fn id(&self) -> String {
        match self {
            Self::Local(run) => run.id(),
            Self::Public { trial, .. } => trial.id.clone(),
        }
    }
    pub fn task(&self) -> &str {
        match self {
            Self::Local(run) => &run.task,
            Self::Public { trial, .. } => &trial.task,
        }
    }
    pub fn label(&self) -> String {
        match self {
            Self::Local(run) => format!(
                "{} · {} · {}",
                run.agent_label(),
                run.outcome.word(),
                run.trial
            ),
            Self::Public { trial, cache } => format!(
                "{}{} {} · {} · {}",
                if cache.join(&trial.file).is_file() {
                    ""
                } else {
                    "[not on this computer] "
                },
                trial.model,
                trial.effort,
                reward(trial.reward),
                trial.id,
            ),
        }
    }
    pub fn description(&self) -> String {
        match self {
            Self::Local(run) => format!(
                "{} · {} · {}",
                run.job,
                run.cost_usd
                    .map(crate::runs::money)
                    .unwrap_or_else(|| "cost unknown".to_owned()),
                run.files.dir.display()
            ),
            Self::Public { trial, .. } => format!(
                "{} {} · {} · {}",
                trial.agent,
                trial.agent_version.as_deref().unwrap_or("version unknown"),
                trial
                    .cost_usd
                    .map(crate::runs::money)
                    .unwrap_or_else(|| "cost unknown".to_owned()),
                trial.source_url
            ),
        }
    }
}

fn reward(value: Option<f64>) -> &'static str {
    match value {
        Some(v) if v >= 1.0 => "passed",
        Some(_) => "failed",
        None => "not graded",
    }
}

pub fn cache_dir() -> PathBuf {
    std::env::var_os("GYM_PUBLIC_REPLAYS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".openagents/terminal-bench/public-replays")
        })
}

/// The pinned manifest is small; trajectory bodies load only when selected.
pub fn public_sources(cache: &Path, manifest: &Path) -> Result<Vec<Source>, String> {
    let bytes = std::fs::read(manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if value["schema"] != SCHEMA {
        return Err("Unsupported public replay manifest".to_owned());
    }
    let trials: Vec<PublicTrial> =
        serde_json::from_value(value["trials"].clone()).map_err(|e| e.to_string())?;
    for trial in &trials {
        if trial.file != format!("{}.json", trial.id)
            || trial.id.is_empty()
            || !trial.id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
        {
            return Err("Invalid public replay file identity".to_owned());
        }
    }
    let mut sources: Vec<_> = trials
        .into_iter()
        .map(|trial| Source::Public {
            trial: Box::new(trial),
            cache: cache.to_path_buf(),
        })
        .collect();
    sources.sort_by_key(|source| match source {
        Source::Public { trial, .. } => (
            trial.task.clone(),
            match trial.effort.as_str() {
                "max" => 0,
                "xhigh" => 1,
                "high" => 2,
                "medium" => 3,
                _ => 4,
            },
            trial.id.clone(),
        ),
        _ => unreachable!(),
    });
    Ok(sources)
}

pub fn sources(catalog: &Catalog) -> (Vec<Source>, Vec<Source>, Vec<String>) {
    let mut runs: std::collections::BTreeMap<String, Run> = catalog
        .runs
        .iter()
        .cloned()
        .map(|run| (run.id(), run))
        .collect();
    let mirrored = PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join(".openagents/terminal-bench/replay-jobs");
    if mirrored.is_dir() {
        let mirror = Catalog::load(crate::runs::Sources {
            jobs: Some(mirrored),
            traces: None,
            tasks: catalog.sources.tasks.clone(),
            index: catalog.sources.index.clone(),
        });
        for run in mirror.runs {
            if runs.get(&run.id()).is_none_or(|existing| existing.retained) {
                runs.insert(run.id(), run);
            }
        }
    }
    let mut runs: Vec<_> = runs.into_values().collect();
    runs.sort_by(|a, b| b.started_ms.cmp(&a.started_ms).then(a.id().cmp(&b.id())));
    let local = runs
        .into_iter()
        .map(|run| Source::Local(Box::new(run)))
        .collect();
    let cache = cache_dir();
    let manifest = cache.join("manifest.json");
    let fallback = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/terminal-bench/reference/fable-5.1-replays.json");
    match public_sources(
        &cache,
        if manifest.is_file() {
            &manifest
        } else {
            &fallback
        },
    ) {
        Ok(public) => (local, public, Vec::new()),
        Err(error) => (
            local,
            Vec::new(),
            vec![
                error,
                "Acquire: cd bench/terminal-bench && uv run python -m tbench.public_replays"
                    .to_owned(),
            ],
        ),
    }
}

#[derive(Clone, Debug)]
pub struct Event {
    pub elapsed_ms: u64,
    pub timing: &'static str,
    pub title: String,
    pub text: String,
    /// Prose and literal evidence remain distinct even within one timed step.
    pub parts: Vec<Part>,
    /// Complete readable record, including metadata, available with `d`.
    pub record: String,
}

#[derive(Clone, Debug)]
pub struct Part {
    pub text: String,
    pub markdown: bool,
}

impl Part {
    fn new(text: String, markdown: bool) -> Self {
        Self {
            text: safe_text(&text),
            markdown,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Replay {
    pub events: Vec<Event>,
    pub duration_ms: u64,
    pub warnings: Vec<String>,
    pub recorded: usize,
    pub estimated: usize,
    pub origin: String,
    /// Each event's tool calls, parsed from its record, in event order.
    pub extracted: Vec<crate::runs_phases::Extracted>,
}

struct Pending {
    at: Option<i64>,
    timing: &'static str,
    title: String,
    text: String,
    parts: Vec<Part>,
    record: String,
    extracted: crate::runs_phases::Extracted,
}

fn time(value: &Value) -> Option<i64> {
    value.get("at").and_then(Value::as_i64).or_else(|| {
        value
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(timestamp_ms)
    })
}

/// All string contents, with JSON structure retained for non-text fields.
/// Control bytes are escaped before they reach the terminal buffer.
pub fn display_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => {
            if text.trim_start().starts_with(['{', '['])
                && text.contains("\"base64\"")
                && let Ok(parsed) = serde_json::from_str::<Value>(text)
            {
                return display_value(&parsed);
            }
            text.clone()
        }
        Value::Array(items) => items
            .iter()
            .map(display_value)
            .collect::<Vec<_>>()
            .join("\n\n"),
        Value::Object(fields) if fields.get("type").and_then(Value::as_str) == Some("base64") => {
            let data = fields
                .get("data")
                .and_then(Value::as_str)
                .unwrap_or_default();
            format!(
                "[Image attachment: {} encoded bytes, SHA-256 {:x}; full image retained in source file]",
                data.len(),
                Sha256::digest(data.as_bytes())
            )
        }
        Value::Object(fields) => fields
            .iter()
            .map(|(key, value)| format!("{key}:\n{}", display_value(value)))
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => value.to_string(),
    }
}

fn safe_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\n' => out.push('\n'),
            '\t' => out.push_str("    "),
            c if c.is_control() => out.extend(c.escape_default()),
            c => out.push(c),
        }
    }
    out
}

fn title(value: &Value) -> String {
    let source = value
        .get("source")
        .or_else(|| value.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("event");
    let kind = value
        .pointer("/extensions/executor_event/event/kind")
        .or_else(|| value.get("subtype"))
        .and_then(Value::as_str)
        .unwrap_or("");
    format!("{source} {kind}").trim().to_owned()
}

/// Conversation contents first; the complete record remains one key away.
fn transcript_parts(value: &Value) -> Vec<Part> {
    fn content(value: &Value) -> String {
        match value {
            Value::Array(items) => items.iter().map(content).collect::<Vec<_>>().join("\n\n"),
            Value::Object(_) => match value["type"].as_str() {
                Some("text") => display_value(&value["text"]),
                Some("thinking") => format!("Thinking\n{}", display_value(&value["thinking"])),
                Some("tool_use") => format!(
                    "Tool: {} ({})\n{}",
                    value["name"].as_str().unwrap_or("tool"),
                    value["id"].as_str().unwrap_or(""),
                    display_value(&value["input"])
                ),
                Some("tool_result") => format!(
                    "Result for {}\n{}",
                    value["tool_use_id"].as_str().unwrap_or("tool"),
                    content(&value["content"])
                ),
                _ => display_value(value),
            },
            _ => display_value(value),
        }
    }
    fn prose(value: &Value, markdown: bool, parts: &mut Vec<Part>) {
        match value {
            Value::Array(items) => {
                for item in items {
                    prose(item, markdown, parts);
                }
            }
            Value::Object(_) => match value["type"].as_str() {
                Some("text" | "input_text" | "output_text") => {
                    parts.push(Part::new(display_value(&value["text"]), markdown));
                }
                Some("thinking") => {
                    parts.push(Part::new("Thinking".to_owned(), false));
                    parts.push(Part::new(display_value(&value["thinking"]), markdown));
                }
                // Tool payloads can contain Markdown-looking shell syntax,
                // source code, diffs, or logs. Keep those bytes as evidence.
                _ => parts.push(Part::new(content(value), false)),
            },
            Value::String(_) => parts.push(Part::new(display_value(value), markdown)),
            _ => parts.push(Part::new(display_value(value), false)),
        }
    }
    let mut parts = Vec::new();
    if let Some(reasoning) = value
        .get("reasoning_content")
        .or_else(|| value.get("reasoning"))
    {
        parts.push(Part::new("Thinking".to_owned(), false));
        prose(reasoning, true, &mut parts);
    }
    if let Some(message) = value.get("message") {
        let role = message
            .get("role")
            .or_else(|| value.get("source"))
            .or_else(|| value.get("type"))
            .and_then(Value::as_str);
        prose(
            message.get("content").unwrap_or(message),
            !role.is_some_and(|role| {
                ["tool", "function", "tool_result"]
                    .iter()
                    .any(|literal| role.eq_ignore_ascii_case(literal))
            }),
            &mut parts,
        );
    }
    if let Some(item) = value.get("item")
        && matches!(item["type"].as_str(), Some("agent_message" | "reasoning"))
        && let Some(text) = item.get("text")
    {
        if item["type"] == "reasoning" {
            parts.push(Part::new("Thinking".to_owned(), false));
        }
        prose(text, true, &mut parts);
    }
    for key in ["tool_calls", "calls"] {
        for call in value[key].as_array().into_iter().flatten() {
            let name = call
                .get("function_name")
                .or_else(|| call.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("tool");
            parts.push(Part::new(
                format!("Tool: {name}\n{}", display_value(&call["arguments"])),
                false,
            ));
            if let Some(output) = call.get("output") {
                parts.push(Part::new(format!("Result\n{}", content(output)), false));
            }
        }
    }
    for result in value
        .pointer("/observation/results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        parts.push(Part::new(
            format!(
                "Result for {}\n{}",
                result["source_call_id"].as_str().unwrap_or("tool"),
                content(&result["content"])
            ),
            false,
        ));
    }
    if let Some(result) = value.get("result") {
        parts.push(Part::new("Result".to_owned(), false));
        prose(result, value["type"] == "result", &mut parts);
    }
    parts.retain(|part| !part.text.is_empty());
    if parts.is_empty() {
        parts.push(Part::new(display_value(value), false));
    }
    parts
}

fn pending(value: &Value, at: Option<i64>, timing: &'static str) -> Pending {
    let parts = transcript_parts(value);
    Pending {
        at,
        timing,
        title: title(value),
        text: parts
            .iter()
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n"),
        parts,
        record: safe_text(&display_value(value)),
        extracted: crate::runs_phases::extract(value),
    }
}

fn json(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?)
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn lines(path: &Path, warnings: &mut Vec<String>) -> Vec<(u64, Value)> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            warnings.push(format!("{}: {e}", path.display()));
            return Vec::new();
        }
    };
    text.lines()
        .enumerate()
        .filter_map(|(i, line)| {
            if line.trim().is_empty() {
                return None;
            }
            match serde_json::from_str(line) {
                Ok(value) => Some((i as u64 + 1, value)),
                Err(_) => {
                    warnings.push(format!(
                        "{} line {} is not JSON; retained as text",
                        path.display(),
                        i + 1
                    ));
                    Some((i as u64 + 1, Value::String(line.to_owned())))
                }
            }
        })
        .collect()
}

fn trajectory(path: &Path) -> Result<Vec<Pending>, String> {
    let value = json(path)?;
    let steps = value["steps"]
        .as_array()
        .ok_or_else(|| "Trajectory has no steps".to_owned())?;
    Ok(steps
        .iter()
        .map(|step| pending(step, time(step), "step timestamp"))
        .collect())
}

fn native(path: &Path, marks: &[(u64, i64)], warnings: &mut Vec<String>) -> Vec<Pending> {
    lines(path, warnings)
        .into_iter()
        .map(|(line, value)| {
            let exact = marks.iter().find(|(n, _)| *n == line).map(|(_, at)| *at);
            let next = marks
                .iter()
                .find(|(n, _)| *n >= line)
                .or_else(|| marks.last())
                .map(|(_, at)| *at);
            let at = time(&value);
            pending(
                &value,
                at.or(exact).or(next),
                if at.is_some() {
                    "message timestamp"
                } else if exact.is_some() {
                    "host receipt"
                } else {
                    "estimated, no timestamp"
                },
            )
        })
        .collect()
}

/// A Coder One episode's Microluna session logs, the acceptance-suite
/// writer's included, in session order.
pub(crate) fn microluna_logs(episode: &Path) -> Vec<PathBuf> {
    let mut logs: Vec<PathBuf> = std::fs::read_dir(episode.join("artifacts"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    (name.starts_with("microluna-") || name.starts_with("accept-"))
                        && name.ends_with(".atif.jsonl")
                })
        })
        .collect();
    // `microluna-1-10` sorts after `microluna-1-9`.
    logs.sort_by_key(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| {
                name.trim_end_matches(".atif.jsonl")
                    .split('-')
                    .filter_map(|part| part.parse::<u64>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    logs
}

fn episode(dir: &Path, log: &Path, warnings: &mut Vec<String>) -> Vec<Pending> {
    let records = lines(log, warnings);
    let mut clocks: HashMap<String, Vec<(u64, i64)>> = HashMap::new();
    for (_, record) in &records {
        let step = &record["step"];
        let event = &step["extensions"]["executor_event"];
        if let (Some(id), Some(line), Some(at)) = (
            event["session_id"].as_str(),
            event["event"]["line"].as_u64(),
            time(step),
        ) {
            clocks.entry(id.to_owned()).or_default().push((line, at));
        }
    }
    let mut streams = Vec::new();
    let mut found = Vec::new();
    let mut paths = crate::runs_transcript::episode_streams(dir);
    paths.sort();
    for path in paths {
        let records = lines(&path, warnings);
        let id = records.iter().find_map(|(_, v)| {
            v.get("session_id")
                .or_else(|| v.get("thread_id"))
                .and_then(Value::as_str)
        });
        let mut marks = id
            .and_then(|id| clocks.get(id))
            .cloned()
            .unwrap_or_default();
        marks.sort_unstable();
        if let Some(id) = id {
            found.push(id.to_owned());
        }
        streams.extend(native(&path, &marks, warnings));
    }
    // Microluna sessions keep their own ATIF logs, with a time on every
    // step; they replace the host's shorter executor events.
    for path in microluna_logs(dir) {
        let records = lines(&path, warnings);
        if let Some(id) = records
            .iter()
            .find_map(|(_, v)| v.pointer("/session/id").and_then(Value::as_str))
        {
            found.push(id.to_owned());
        }
        for (_, record) in &records {
            if let Some(step) = record.get("step") {
                streams.push(pending(step, time(step), "message timestamp"));
            }
        }
    }
    let missing = clocks.keys().filter(|id| !found.contains(id)).count();
    if missing > 0 {
        warnings.push(format!("{missing} executor session(s) have no native stream; only retained host events can be shown"));
    }
    let mut result: Vec<_> = records
        .iter()
        .filter_map(|(_, record)| {
            let step = record.get("step").unwrap_or(record);
            // The full native record replaces the host's shortened duplicate.
            let id = step
                .pointer("/extensions/executor_event/session_id")
                .and_then(Value::as_str);
            if id.is_some_and(|id| found.iter().any(|found| found == id)) {
                return None;
            }
            Some(pending(step, time(step), "host timestamp"))
        })
        .collect();
    result.extend(streams);
    result
}

impl Replay {
    pub fn load(source: &Source) -> Result<Self, String> {
        let mut warnings = Vec::new();
        let (events, start, end, origin) = match source {
            Source::Public { trial, cache } => {
                let path = cache.join(&trial.file);
                source.verify()?;
                (
                    trajectory(&path)?,
                    trial.started_at.as_deref().and_then(timestamp_ms),
                    trial.finished_at.as_deref().and_then(timestamp_ms),
                    "Harbor step timestamps; tool results share their step's timestamp".to_owned(),
                )
            }
            Source::Local(run) => {
                let files = &run.files;
                let log = files
                    .episode
                    .as_ref()
                    .map(|p| p.join("episode.atif.jsonl"))
                    .filter(|p| p.is_file())
                    .or_else(|| files.live.clone());
                let events = if let Some(log) = log {
                    episode(
                        files.episode.as_deref().unwrap_or(&files.dir),
                        &log,
                        &mut warnings,
                    )
                } else if let Some(path) = files
                    .episode
                    .as_ref()
                    .map(|p| p.join("trajectory.atif.json"))
                    .filter(|p| p.is_file())
                    .or_else(|| files.trajectory.clone())
                {
                    let mut events = trajectory(&path)?;
                    if let Some(dir) = &files.episode {
                        for stream in crate::runs_transcript::episode_streams(dir) {
                            events.extend(native(&stream, &[], &mut warnings));
                        }
                    }
                    events
                } else if let Some(path) = &files.native {
                    native(path, &[], &mut warnings)
                } else {
                    return Err("No retained transcript for this attempt".to_owned());
                };
                let start = files
                    .result
                    .as_deref()
                    .and_then(|p| json(p).ok())
                    .and_then(|v| {
                        v.pointer("/agent_execution/started_at")
                            .and_then(Value::as_str)
                            .and_then(timestamp_ms)
                    })
                    .or(run.started_ms);
                let end = start
                    .zip(run.agent_ms)
                    .map(|(s, duration)| s.saturating_add(duration as i64))
                    .or(run.ended_ms);
                (
                    events,
                    start,
                    end,
                    "Times come from recorded steps, messages, and host receipts; estimates are labeled".to_owned(),
                )
            }
        };
        if events.is_empty() {
            return Err("Transcript contains no events".to_owned());
        }
        Ok(Self::build(events, start, end, warnings, origin))
    }

    fn build(
        events: Vec<Pending>,
        start: Option<i64>,
        end: Option<i64>,
        warnings: Vec<String>,
        origin: String,
    ) -> Self {
        let first = events.iter().filter_map(|e| e.at).min();
        // Include setup/task records that precede the supplied agent start.
        let start = start.into_iter().chain(first).min().unwrap_or(0);
        let mut last = start;
        let mut replay = Replay {
            warnings,
            origin,
            ..Self::default()
        };
        let mut paired = Vec::with_capacity(events.len());
        for event in events {
            let timing = if event.at.is_none() {
                "estimated, no timestamp"
            } else {
                event.timing
            };
            let at = event.at.unwrap_or(last);
            last = at;
            if timing == "estimated, no timestamp" {
                replay.estimated += 1;
            } else {
                replay.recorded += 1;
            }
            paired.push((
                Event {
                    elapsed_ms: at.saturating_sub(start).max(0) as u64,
                    timing,
                    title: event.title,
                    text: event.text,
                    parts: event.parts,
                    record: event.record,
                },
                event.extracted,
            ));
        }
        paired.sort_by_key(|(e, _)| e.elapsed_ms);
        (replay.events, replay.extracted) = paired.into_iter().unzip();
        replay.duration_ms = replay.events.last().map(|e| e.elapsed_ms).unwrap_or(0).max(
            end.map(|e| e.saturating_sub(start).max(0) as u64)
                .unwrap_or(0),
        );
        replay
    }
}

/// A monotonic shared playback clock, independent of rendering frequency.
#[derive(Clone, Debug)]
pub struct Playback {
    pub elapsed_ms: f64,
    pub duration_ms: u64,
    pub playing: bool,
    speed_index: usize,
}
impl Playback {
    pub const SPEEDS: [u32; 4] = [1, 2, 5, 10];
    pub fn new(duration_ms: u64) -> Self {
        Self {
            elapsed_ms: 0.0,
            duration_ms,
            playing: false,
            speed_index: 0,
        }
    }
    pub fn speed(&self) -> u32 {
        Self::SPEEDS[self.speed_index]
    }
    pub fn faster(&mut self) {
        self.speed_index = (self.speed_index + 1).min(Self::SPEEDS.len() - 1);
    }
    pub fn slower(&mut self) {
        self.speed_index = self.speed_index.saturating_sub(1);
    }
    pub fn advance(&mut self, elapsed: std::time::Duration) {
        if self.playing {
            self.elapsed_ms = (self.elapsed_ms
                + elapsed.as_secs_f64() * 1000.0 * f64::from(self.speed()))
            .min(self.duration_ms as f64);
        }
        if self.elapsed_ms >= self.duration_ms as f64 {
            self.playing = false;
        }
    }
    pub fn seek(&mut self, millis: i64) {
        self.elapsed_ms = (self.elapsed_ms + millis as f64).clamp(0.0, self.duration_ms as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn microluna_session_logs_replace_the_host_s_executor_events() {
        let dir = tempfile::tempdir().unwrap();
        let artifacts = dir.path().join("artifacts");
        std::fs::create_dir_all(&artifacts).unwrap();
        let host = [
            json!({"record": "step", "step": {"at": 1000, "source": "User", "message": "Fix it."}}),
            json!({"record": "step", "step": {"at": 1100, "source": "System", "message": "executor event 1",
                "extensions": {"executor_event": {"session_id": "microluna-1-1",
                    "event": {"line": 1, "kind": "command_started", "command": "ls"}}}}}),
        ];
        let log = dir.path().join("episode.atif.jsonl");
        std::fs::write(
            &log,
            host.iter().map(|v| format!("{v}\n")).collect::<String>(),
        )
        .unwrap();
        let reasoning = format!(
            "# Review\n\n{}\nLast summary paragraph.",
            "Check the boundary. ".repeat(2000)
        );
        let session = [
            json!({"record": "session", "session": {"id": "microluna-1-1"}}),
            json!({"record": "step", "step": {"at": 1150, "source": "Agent", "message": "I will check the cache.",
                "reasoning": reasoning}}),
            json!({"record": "step", "step": {"at": 1200, "source": "Agent", "message": "",
                "call": {"id": "c1", "name": "run_command", "arguments": {"command": "ls /app"},
                    "output": "[exit 0]\nsrc", "outcome": "completed", "milliseconds": 5}}}),
        ];
        std::fs::write(
            artifacts.join("microluna-1-1.atif.jsonl"),
            session.iter().map(|v| format!("{v}\n")).collect::<String>(),
        )
        .unwrap();
        let mut warnings = Vec::new();
        let events = episode(dir.path(), &log, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        // The task, full reasoning summary, and call replace the host event.
        assert_eq!(events.len(), 3);
        assert!(events.iter().any(|event| event.parts.iter().any(|part| {
            part.markdown
                && part.text.starts_with("# Review")
                && part.text.ends_with("Last summary paragraph.")
        })));
        let calls: Vec<&str> = events
            .iter()
            .flat_map(|e| e.extracted.actions.iter().map(|a| a.input.as_str()))
            .collect();
        assert_eq!(calls, vec!["ls /app"]);
    }

    #[test]
    fn microluna_logs_sort_by_session_number() {
        let dir = tempfile::tempdir().unwrap();
        let artifacts = dir.path().join("artifacts");
        std::fs::create_dir_all(&artifacts).unwrap();
        for name in [
            "microluna-1-10.atif.jsonl",
            "microluna-1-9.atif.jsonl",
            "other.jsonl",
        ] {
            std::fs::write(artifacts.join(name), "").unwrap();
        }
        let names: Vec<String> = microluna_logs(dir.path())
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            ["microluna-1-9.atif.jsonl", "microluna-1-10.atif.jsonl"]
        );
    }

    #[test]
    fn the_clock_preserves_pause_speed_seek_and_end() {
        let mut p = Playback::new(30_000);
        p.advance(Duration::from_secs(1));
        assert_eq!(p.elapsed_ms, 0.0);
        p.playing = true;
        p.advance(Duration::from_millis(1500));
        assert_eq!(p.elapsed_ms, 1500.0);
        for _ in 0..8 {
            p.faster();
        }
        assert_eq!(p.speed(), 10);
        p.advance(Duration::from_millis(200));
        assert_eq!(p.elapsed_ms, 3500.0);
        p.playing = false;
        p.advance(Duration::from_secs(10));
        assert_eq!(p.elapsed_ms, 3500.0);
        p.seek(-99_000);
        assert_eq!(p.elapsed_ms, 0.0);
        p.playing = true;
        p.advance(Duration::from_secs(4));
        assert_eq!(p.elapsed_ms, 30_000.0);
        assert!(!p.playing);
        for _ in 0..8 {
            p.slower();
        }
        assert_eq!(p.speed(), 1);
    }

    #[test]
    fn two_calendar_dates_align_to_the_same_elapsed_clock() {
        let make = |start| {
            Replay::build(
                vec![pending(
                    &json!({"message":"hello"}),
                    Some(start + 500),
                    "step timestamp",
                )],
                Some(start),
                Some(start + 5000),
                vec![],
                String::new(),
            )
        };
        assert_eq!(
            make(1_000).events[0].elapsed_ms,
            make(9_000_000).events[0].elapsed_ms
        );
        assert_eq!(make(1_000).duration_ms, 5000);
    }

    #[test]
    fn text_is_complete_and_unknown_times_are_explicit() {
        let text = format!("{}\nLAST LINE", "long output\n".repeat(2000));
        let r = Replay::build(
            vec![pending(&json!({"message":text}), None, "step timestamp")],
            None,
            None,
            vec![],
            String::new(),
        );
        assert!(r.events[0].text.ends_with("LAST LINE"));
        assert_eq!(r.recorded, 0);
        assert_eq!(r.estimated, 1);
        assert_eq!(r.events[0].timing, "estimated, no timestamp");
        assert_eq!(safe_text("\x1b[2J\ttext"), "\\u{1b}[2J    text");
    }

    #[test]
    fn markdown_prose_and_literal_tools_share_a_timestamp_without_sharing_a_parser() {
        let event = pending(
            &json!({
                "source":"agent", "message":"# Plan\n\nUse **care** and `cargo test`.",
                "reasoning_content":"A **reason** to check.",
                "tool_calls":[{"function_name":"bash", "arguments":{"command":"printf '**literal**\\n# heading'"}}],
                "observation":{"results":[{"source_call_id":"one","content":"# not a heading\n**not emphasis**\n- [ ] not a task"}]}
            }),
            Some(1000),
            "step timestamp",
        );
        assert!(
            event
                .parts
                .iter()
                .any(|part| part.markdown && part.text.starts_with("# Plan"))
        );
        assert!(
            event
                .parts
                .iter()
                .any(|part| part.markdown && part.text.contains("**reason**"))
        );
        assert!(
            event
                .parts
                .iter()
                .any(|part| !part.markdown && part.text.contains("**literal**"))
        );
        assert!(
            event
                .parts
                .iter()
                .any(|part| !part.markdown
                    && part.text.contains("# not a heading\n**not emphasis**"))
        );
        let replay = Replay::build(vec![event], Some(500), None, vec![], String::new());
        assert_eq!(replay.events[0].elapsed_ms, 500);
        assert!(replay.events[0].record.contains("# Plan"));
    }

    #[test]
    fn claude_and_codex_messages_render_markdown_but_results_remain_literal() {
        let claude = transcript_parts(&json!({"type":"assistant","message":{"content":[
            {"type":"text","text":"# Report"},
            {"type":"thinking","thinking":"A **thought**"},
            {"type":"tool_use","name":"Read","input":{"file_path":"**literal**"}},
            {"type":"tool_result","tool_use_id":"one","content":[{"type":"text","text":"# output\n**literal**"}]}
        ]}}));
        assert_eq!(claude.iter().filter(|part| part.markdown).count(), 2);
        assert!(
            claude
                .iter()
                .any(|part| !part.markdown && part.text.contains("# output\n**literal**"))
        );
        for kind in ["agent_message", "reasoning"] {
            let codex = transcript_parts(
                &json!({"type":"item.completed","item":{"type":kind,"text":"## Answer\n\n**ready**"}}),
            );
            assert!(
                codex
                    .iter()
                    .any(|part| part.markdown && part.text.starts_with("## Answer"))
            );
        }
        let command = transcript_parts(
            &json!({"type":"item.completed","item":{"type":"command_execution","command":"echo '**literal**'","aggregated_output":"# output"}}),
        );
        assert!(command.iter().all(|part| !part.markdown));
        let tool = transcript_parts(&json!({"source":"tool","message":"**literal**"}));
        assert!(tool.iter().all(|part| !part.markdown));
        let result = transcript_parts(&json!({"type":"result","result":"## Final report"}));
        assert!(
            result
                .iter()
                .any(|part| part.markdown && part.text.starts_with("## Final"))
        );
    }

    #[test]
    fn image_bytes_remain_in_the_source_without_flooding_the_screen() {
        let value = json!({"type":"image", "source":{"type":"base64","data":"abcdef"}});
        let rendered = display_value(&Value::String(value.to_string()));
        assert!(rendered.contains("6 encoded bytes"));
        assert!(rendered.contains("SHA-256"));
        assert!(!rendered.contains("abcdef"));
    }

    #[test]
    fn native_tool_result_appears_at_its_own_receipt_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stream.jsonl");
        std::fs::write(&path, "{\"type\":\"assistant\",\"message\":\"call\"}\n{\"type\":\"user\",\"message\":\"result\"}\n").unwrap();
        let events = native(&path, &[(1, 1000), (2, 4000)], &mut Vec::new());
        let replay = Replay::build(events, Some(1000), None, vec![], String::new());
        assert_eq!(replay.events[0].elapsed_ms, 0);
        assert_eq!(replay.events[1].elapsed_ms, 3000);
        assert!(!replay.events[0].text.contains("result"));
        assert!(replay.events[1].text.contains("result"));
    }

    #[test]
    fn missing_native_timestamps_use_later_receipts_and_are_labeled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stream.jsonl");
        std::fs::write(&path, "{}\n{}\n{}\n").unwrap();
        let events = native(&path, &[(2, 2000)], &mut Vec::new());
        assert_eq!(events[0].at, Some(2000));
        assert_eq!(events[0].timing, "estimated, no timestamp");
        assert_eq!(events[1].timing, "host receipt");
        assert_eq!(events[2].timing, "estimated, no timestamp");
    }

    #[test]
    fn public_bytes_are_verified_and_system_and_tool_text_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = serde_json::to_vec(&json!({"steps":[
            {"source":"system","timestamp":"2026-09-23T00:00:00Z","message":"SYSTEM INSTRUCTION"},
            {"source":"agent","timestamp":"2026-09-23T00:00:01Z","tool_calls":[{"arguments":{"command":"echo hi"}}],"observation":{"results":[{"content":"TOOL RESULT"}]}}
        ]})).unwrap();
        std::fs::write(dir.path().join("aa.json"), &bytes).unwrap();
        let manifest = json!({"schema":SCHEMA,"trials":[{"id":"aa","file":"aa.json","task":"task","model":"Fable 5.1","agent":"Claude Code","effort":"max","source_url":"https://example.com","sha256":format!("{:x}",Sha256::digest(&bytes))}]});
        let path = dir.path().join("manifest.json");
        std::fs::write(&path, manifest.to_string()).unwrap();
        let sources = public_sources(dir.path(), &path).unwrap();
        let r = Replay::load(&sources[0]).unwrap();
        assert_eq!(r.events.len(), 2);
        assert!(r.events[0].text.contains("SYSTEM INSTRUCTION"));
        assert!(r.events[1].text.contains("echo hi"));
        assert!(r.events[1].text.contains("TOOL RESULT"));
        std::fs::write(dir.path().join("aa.json"), b"{}").unwrap();
        assert!(Replay::load(&sources[0]).unwrap_err().contains("Integrity"));
        let mut bad = manifest;
        bad["trials"][0]["file"] = json!("../aa.json");
        std::fs::write(&path, bad.to_string()).unwrap();
        assert!(public_sources(dir.path(), &path).is_err());
    }

    #[test]
    fn retained_coder_and_native_fixtures_load() {
        let (_dir, sources) = crate::runs::fixture_sources();
        let catalog = Catalog::load(sources);
        for run in catalog.runs {
            if run.files.trajectory.is_some() || run.files.episode.is_some() {
                let replay = Replay::load(&Source::Local(Box::new(run.clone()))).unwrap();
                assert!(!replay.events.is_empty(), "{}", run.id());
                assert!(
                    replay
                        .events
                        .windows(2)
                        .all(|pair| pair[0].elapsed_ms <= pair[1].elapsed_ms)
                );
            }
        }
    }
}
