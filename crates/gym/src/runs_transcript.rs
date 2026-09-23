//! A run's transcript: what the agent said and did, in order, as blocks a
//! person can read.
//!
//! Four kinds of record become the same blocks:
//!
//! - Coder One's episode log, `episode.atif.jsonl` (or its live copy while
//!   the trial runs): the task, the briefing's judgments, each hand-off to
//!   an executor, the checks, repairs, and persistence rounds.
//! - The executor's own stream, `artifacts/delegate-N.stream.jsonl`, from
//!   Claude Code (`stream-json`) or Codex (`exec --json`). When a session's
//!   stream is on disk its blocks replace the log's shorter executor
//!   events; while a trial runs, the log's events are all there is.
//! - Harbor's trajectory for a Claude Code or Codex trial,
//!   `agent/trajectory.json` (ATIF), and Coder One's older
//!   `trajectory.atif.json`.
//! - Harbor's native Claude Code or Codex output, `claude-code.txt` or
//!   `codex.txt`, which is what a trial in progress has.
//!
//! The model is plain data. [`Block::headline`] and [`Block::body`] say
//! what a block reads as, so the `gym runs` text and the terminal pane
//! draw the same words.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::terminal_bench::timestamp_ms;

/// How many lines of a command's output an expanded block shows before it
/// says how many more there are.
pub const OUTPUT_LINES: usize = 60;

/// A transcript: blocks in order, and where they came from.
#[derive(Clone, Debug, Default)]
pub struct Transcript {
    pub blocks: Vec<Block>,
    /// The files read, most important first.
    pub sources: Vec<PathBuf>,
    /// Per executor session: who ran it, how long, what it cost, and what
    /// it reported last.
    pub sessions: Vec<Session>,
    /// How many times Coder One's monitor looked, and how many of those
    /// looks raised a flag.
    pub monitor: (usize, usize),
}

/// One executor session, as the transcript saw it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    /// `Claude Code on Opus 5.5`, for example.
    pub who: String,
    /// Why it started, in plain words, when the record says.
    pub why: Option<String>,
    pub milliseconds: Option<u64>,
    pub cost_usd: Option<f64>,
    pub turns: Option<u64>,
    /// The session's final report.
    pub report: Option<String>,
    pub commands: usize,
    pub edits: usize,
}

/// One block of a transcript.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// When it happened, in milliseconds since the epoch, when known.
    pub at: Option<i64>,
    pub kind: Kind,
}

/// What a block is.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// The task's instruction.
    Task(String),
    /// A new chapter: an executor takes over, or Coder One starts
    /// checking.
    Section {
        title: String,
        note: Option<String>,
        milliseconds: Option<u64>,
        cost_usd: Option<f64>,
    },
    /// Something the agent said.
    Say(String),
    /// Reasoning the agent wrote down.
    Think(String),
    /// A shell command and what it printed.
    Command {
        command: String,
        output: String,
        exit: Option<i64>,
        failed: bool,
    },
    /// A file the agent created, changed, or deleted.
    Edit {
        path: String,
        action: &'static str,
        added: usize,
        removed: usize,
        body: String,
    },
    /// A read or a search that changes nothing.
    Look { what: String, output: String },
    /// Any other tool call.
    Tool {
        name: String,
        input: String,
        output: String,
    },
    /// A typed judgment, from Jev or from a rule.
    Decision {
        question: String,
        answer: String,
        detail: Vec<(String, String)>,
        /// The probability behind a single yes-or-no answer.
        probability: Option<f64>,
        milliseconds: Option<u64>,
        cost_usd: Option<f64>,
    },
    /// Coder One's checks, repair, second executor, or persistence round.
    Check {
        title: String,
        verdict: String,
        lines: Vec<String>,
        /// `Some(true)` when it found nothing wrong, `Some(false)` when it
        /// found a problem.
        good: Option<bool>,
    },
    /// The executor's final report.
    Report(String),
    /// A quiet line about the run itself.
    Note(String),
}

impl Block {
    fn new(at: Option<i64>, kind: Kind) -> Self {
        Block { at, kind }
    }

    /// Whether the block has more to show than its headline.
    #[must_use]
    pub fn expandable(&self) -> bool {
        match &self.kind {
            Kind::Task(text) => text.lines().count() > 8,
            Kind::Report(text) => text.lines().count() > 10,
            Kind::Say(text) => text.lines().count() > 12,
            Kind::Think(text) => !text.trim().is_empty(),
            Kind::Command { output, .. }
            | Kind::Look { output, .. }
            | Kind::Tool { output, .. } => !output.trim().is_empty(),
            Kind::Edit { body, .. } => !body.trim().is_empty(),
            Kind::Decision { detail, .. } => !detail.is_empty(),
            Kind::Check { lines, .. } => !lines.is_empty(),
            Kind::Section { .. } | Kind::Note(_) => false,
        }
    }

    /// The one line a collapsed block reads as.
    #[must_use]
    pub fn headline(&self) -> String {
        match &self.kind {
            Kind::Task(_) => "The task".to_owned(),
            Kind::Section { title, .. } => title.clone(),
            Kind::Say(text) => first_line(text),
            Kind::Think(text) => format!("thinking: {}", first_line(text)),
            Kind::Command {
                command,
                exit,
                failed,
                output,
            } => {
                let mut line = format!("$ {}", first_line(command));
                if command.trim().lines().count() > 1 {
                    line.push_str(" …");
                }
                let lines = output.trim_end().lines().count();
                let mut tail = Vec::new();
                match exit {
                    Some(0) | None if !failed => {}
                    Some(code) => tail.push(format!("exit {code}")),
                    None => tail.push("failed".to_owned()),
                }
                if lines > 0 {
                    tail.push(plural(lines, "line"));
                }
                if !tail.is_empty() {
                    line.push_str(&format!("   ({})", tail.join(", ")));
                }
                line
            }
            Kind::Edit {
                path,
                action,
                added,
                removed,
                ..
            } => {
                let counts = match (added, removed) {
                    (0, 0) => String::new(),
                    (a, 0) => format!("  +{a}"),
                    (0, r) => format!("  −{r}"),
                    (a, r) => format!("  +{a} −{r}"),
                };
                format!("{action} {path}{counts}")
            }
            Kind::Look { what, .. } => what.clone(),
            Kind::Tool { name, input, .. } => {
                if input.is_empty() {
                    name.clone()
                } else {
                    format!("{name}: {}", first_line(input))
                }
            }
            Kind::Decision {
                question, answer, ..
            } => format!("{question} {answer}"),
            Kind::Check { title, verdict, .. } => format!("{title}: {verdict}"),
            Kind::Report(_) => "Final report".to_owned(),
            Kind::Note(text) => text.clone(),
        }
    }

    /// The lines under the headline: all of them when `expanded`, a short
    /// preview otherwise. Long output is cut with a count of what was left
    /// out, so one command cannot flood the screen.
    #[must_use]
    pub fn body(&self, expanded: bool) -> Vec<String> {
        match &self.kind {
            Kind::Task(text) => preview(text, if expanded { 400 } else { 8 }),
            Kind::Report(text) => preview(text, if expanded { 400 } else { 10 }),
            Kind::Say(text) => preview(text, if expanded { 400 } else { 12 })
                .into_iter()
                .skip(1)
                .collect(),
            Kind::Think(text) if expanded => preview(text, 200),
            Kind::Command {
                command, output, ..
            } if expanded => {
                let mut lines: Vec<String> = Vec::new();
                if command.trim().lines().count() > 1 {
                    lines.extend(preview(command, 40).into_iter().skip(1));
                    lines.push(String::new());
                }
                lines.extend(preview(output, OUTPUT_LINES));
                lines
            }
            Kind::Look { output, .. } | Kind::Tool { output, .. } if expanded => {
                preview(output, OUTPUT_LINES)
            }
            Kind::Edit { body, .. } if expanded => preview(body, OUTPUT_LINES),
            Kind::Decision { detail, .. } if expanded => detail
                .iter()
                .map(|(name, value)| format!("{name}: {value}"))
                .collect(),
            Kind::Check { lines, .. } => {
                if expanded {
                    lines.clone()
                } else {
                    lines.iter().take(3).cloned().collect()
                }
            }
            Kind::Section { note, .. } => note.iter().cloned().collect(),
            _ => Vec::new(),
        }
    }
}

/// The first non-empty line, whitespace squeezed.
#[must_use]
pub fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// At most `limit` lines of `text`, with a closing line that counts the
/// rest.
#[must_use]
pub fn preview(text: &str, limit: usize) -> Vec<String> {
    let lines: Vec<&str> = text.trim_matches('\n').lines().collect();
    let mut out: Vec<String> = lines
        .iter()
        .take(limit)
        .map(|line| line.trim_end().replace('\t', "    "))
        .collect();
    if lines.len() > limit {
        let more = lines.len() - limit;
        out.push(format!(
            "… {more} more {}",
            if more == 1 { "line" } else { "lines" }
        ));
    }
    out
}

/// `3 lines`, `1 line`.
#[must_use]
pub fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// A model's name as people say it: `claude-opus-5-5` reads `Opus 5.5`,
/// `gpt-6-luna` reads `GPT-6 Luna`. An unknown name is kept.
#[must_use]
pub fn model_name(model: &str) -> String {
    let model = model.trim();
    if let Some(rest) = model.strip_prefix("claude-") {
        let mut parts = rest.split('-');
        let family = parts.next().unwrap_or_default();
        let version: Vec<&str> = parts
            .take_while(|part| part.chars().all(|c| c.is_ascii_digit()) && part.len() <= 2)
            .collect();
        let mut name = capitalize(family);
        if !version.is_empty() {
            name.push(' ');
            name.push_str(&version.join("."));
        }
        return name;
    }
    if let Some(rest) = model.strip_prefix("gpt-") {
        let mut parts = rest.splitn(2, '-');
        let version = parts.next().unwrap_or_default();
        return match parts.next() {
            Some(name) => format!("GPT-{version} {}", capitalize(name)),
            None => format!("GPT-{version}"),
        };
    }
    model.to_owned()
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// `Claude Code` or `Codex` for an executor adapter's name.
#[must_use]
pub fn executor_name(agent: &str) -> String {
    match agent {
        "claude-code" => "Claude Code".to_owned(),
        "codex" => "Codex".to_owned(),
        "coder-one" | "coder-one-tunable" => "Coder One".to_owned(),
        other => other.to_owned(),
    }
}

/// `Claude Code on Opus 5.5`.
#[must_use]
pub fn executor_with_model(agent: &str, model: Option<&str>) -> String {
    match model.filter(|model| !model.is_empty() && *model != "free") {
        Some(model) => format!("{} on {}", executor_name(agent), model_name(model)),
        None => executor_name(agent),
    }
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn read_lines(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// The text of a tool result's content: a string, or a list of text parts.
fn content_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .and_then(Value::as_str)
                    .or_else(|| part.as_str())
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Removes a shell wrapper Codex puts around every command.
fn unwrap_shell(command: &str) -> String {
    let command = command.trim();
    for prefix in ["/bin/bash -lc ", "bash -lc ", "/bin/sh -c ", "sh -c "] {
        if let Some(rest) = command.strip_prefix(prefix) {
            let rest = rest.trim();
            if rest.len() >= 2
                && ((rest.starts_with('\'') && rest.ends_with('\''))
                    || (rest.starts_with('"') && rest.ends_with('"')))
            {
                let inner = &rest[1..rest.len() - 1];
                return if rest.starts_with('"') {
                    inner.replace("\\\"", "\"").replace("\\\\", "\\")
                } else {
                    inner.replace("'\\''", "'")
                };
            }
            return rest.to_owned();
        }
    }
    command.to_owned()
}

/// The block one tool call becomes, given its input and its result.
fn tool_block(name: &str, input: &Value, output: String, failed: bool) -> Kind {
    let get = |key: &str| {
        input
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let lines = |text: &str| text.lines().count();
    match name {
        "Bash" | "bash" | "shell" | "execute_command" | "run_command" | "exec_command" => {
            let command = if input.get("command").is_some() {
                get("command")
            } else if input.get("cmd").is_some() {
                get("cmd")
            } else {
                input.to_string()
            };
            let (exit, output) = exit_prefix(&output);
            Kind::Command {
                command: unwrap_shell(&command),
                failed: failed || exit.is_some_and(|code| code != 0),
                exit,
                output,
            }
        }
        "Read" | "read_file" => Kind::Look {
            what: format!("Read {}", short_path(&get("file_path"))),
            output,
        },
        "Glob" => Kind::Look {
            what: format!("Looked for files matching {}", get("pattern")),
            output,
        },
        "Grep" => Kind::Look {
            what: format!("Searched for \"{}\"", get("pattern")),
            output,
        },
        "LS" => Kind::Look {
            what: format!("Listed {}", short_path(&get("path"))),
            output,
        },
        "Edit" => {
            let old = get("old_string");
            let new = get("new_string");
            Kind::Edit {
                path: short_path(&get("file_path")),
                action: "Edited",
                added: lines(&new),
                removed: lines(&old),
                body: diff_body(&old, &new),
            }
        }
        "MultiEdit" => {
            let edits = input
                .get("edits")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut body = String::new();
            let (mut added, mut removed) = (0, 0);
            for edit in &edits {
                let old = edit
                    .get("old_string")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let new = edit
                    .get("new_string")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                added += lines(new);
                removed += lines(old);
                body.push_str(&diff_body(old, new));
                body.push('\n');
            }
            Kind::Edit {
                path: short_path(&get("file_path")),
                action: "Edited",
                added,
                removed,
                body,
            }
        }
        "Write" | "write_file" => {
            let content = get("content");
            Kind::Edit {
                path: short_path(&get("file_path")),
                action: "Wrote",
                added: lines(&content),
                removed: 0,
                body: content,
            }
        }
        "TodoWrite" => {
            let items: Vec<String> = input
                .get("todos")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|todo| {
                    let mark = match todo.get("status").and_then(Value::as_str) {
                        Some("completed") => "[x]",
                        Some("in_progress") => "[~]",
                        _ => "[ ]",
                    };
                    format!(
                        "{mark} {}",
                        todo.get("content")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                    )
                })
                .collect();
            Kind::Tool {
                name: "Updated its to-do list".to_owned(),
                input: String::new(),
                output: items.join("\n"),
            }
        }
        "Task" | "Agent" => Kind::Tool {
            name: "Started a helper agent".to_owned(),
            input: get("description"),
            output,
        },
        "WebFetch" => Kind::Tool {
            name: "Fetched a web page".to_owned(),
            input: get("url"),
            output,
        },
        "WebSearch" => Kind::Tool {
            name: "Searched the web".to_owned(),
            input: get("query"),
            output,
        },
        other => Kind::Tool {
            name: other.to_owned(),
            input: compact_json(input),
            output,
        },
    }
}

/// A result that starts `Exit code N` carries the code; the rest is the
/// output.
fn exit_prefix(output: &str) -> (Option<i64>, String) {
    if let Some(rest) = output.strip_prefix("Exit code ") {
        let (code, rest) = rest.split_once('\n').unwrap_or((rest, ""));
        if let Ok(code) = code.trim().parse::<i64>() {
            return (Some(code), rest.to_owned());
        }
    }
    (None, output.to_owned())
}

fn diff_body(old: &str, new: &str) -> String {
    let mut body = String::new();
    for line in old.lines() {
        body.push_str("- ");
        body.push_str(line);
        body.push('\n');
    }
    for line in new.lines() {
        body.push_str("+ ");
        body.push_str(line);
        body.push('\n');
    }
    body
}

fn compact_json(value: &Value) -> String {
    let text = value.to_string();
    if text == "{}" || text == "null" {
        String::new()
    } else {
        text
    }
}

/// `/app/src/lib.rs` stays as it is; only very long paths lose their
/// middle.
fn short_path(path: &str) -> String {
    if path.chars().count() <= 70 {
        return path.to_owned();
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() > 4 {
        format!(
            "{}/…/{}",
            parts[..2].join("/"),
            parts[parts.len() - 2..].join("/")
        )
    } else {
        path.to_owned()
    }
}

/// Maps a native stream's line numbers to the times the episode log
/// recorded for them, so a stream without timestamps still has a clock.
#[derive(Clone, Debug, Default)]
pub struct Clock {
    marks: Vec<(u64, i64)>,
}

impl Clock {
    fn at(&self, line: u64) -> Option<i64> {
        match self.marks.binary_search_by_key(&line, |(mark, _)| *mark) {
            Ok(index) => Some(self.marks[index].1),
            Err(0) => self.marks.first().map(|(_, at)| *at),
            Err(index) => Some(self.marks[index - 1].1),
        }
    }
}

/// What a native stream says at its end.
#[derive(Clone, Debug, Default)]
pub struct StreamEnd {
    pub report: Option<String>,
    pub cost_usd: Option<f64>,
    pub milliseconds: Option<u64>,
    pub turns: Option<u64>,
}

/// Reads a Claude Code `stream-json` log into blocks. Tool calls pair
/// with their results by id; a call still waiting for its result shows
/// with no output.
#[must_use]
pub fn claude_stream(lines: &[Value], clock: &Clock) -> (Vec<Block>, StreamEnd) {
    let mut blocks: Vec<Block> = Vec::new();
    let mut pending: HashMap<String, (usize, String, Value)> = HashMap::new();
    let mut end = StreamEnd::default();
    let mut last_at = None;
    for (index, line) in lines.iter().enumerate() {
        let at = line
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(timestamp_ms)
            .or_else(|| clock.at(index as u64 + 1))
            .or(last_at);
        last_at = at;
        match line.get("type").and_then(Value::as_str) {
            Some("assistant") => {
                for item in line
                    .pointer("/message/content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    match item.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            let text = text(item, "text").unwrap_or_default();
                            if !text.trim().is_empty() {
                                blocks.push(Block::new(at, Kind::Say(text)));
                            }
                        }
                        Some("thinking") => {
                            let text = text(item, "thinking").unwrap_or_default();
                            if !text.trim().is_empty() {
                                blocks.push(Block::new(at, Kind::Think(text)));
                            }
                        }
                        Some("tool_use") => {
                            let id = text(item, "id").unwrap_or_default();
                            let name = text(item, "name").unwrap_or_default();
                            let input = item.get("input").cloned().unwrap_or(Value::Null);
                            blocks.push(Block::new(
                                at,
                                tool_block(&name, &input, String::new(), false),
                            ));
                            pending.insert(id, (blocks.len() - 1, name, input));
                        }
                        _ => {}
                    }
                }
            }
            Some("user") => {
                for item in line
                    .pointer("/message/content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if item.get("type").and_then(Value::as_str) != Some("tool_result") {
                        continue;
                    }
                    let id = text(item, "tool_use_id").unwrap_or_default();
                    if let Some((index, name, input)) = pending.remove(&id) {
                        let output = content_text(item.get("content").unwrap_or(&Value::Null));
                        let failed = item.get("is_error").and_then(Value::as_bool) == Some(true);
                        blocks[index].kind = tool_block(&name, &input, output, failed);
                    }
                }
            }
            Some("result") => {
                end.report = text(line, "result").filter(|text| !text.trim().is_empty());
                if let Some(report) = &end.report {
                    close_with_report(&mut blocks, report, at);
                }
                end.cost_usd = line.get("total_cost_usd").and_then(Value::as_f64);
                end.milliseconds = line.get("duration_ms").and_then(Value::as_u64);
                end.turns = line.get("num_turns").and_then(Value::as_u64);
            }
            _ => {}
        }
    }
    (blocks, end)
}

/// Reads a Codex `exec --json` log into blocks.
#[must_use]
pub fn codex_stream(lines: &[Value], clock: &Clock) -> (Vec<Block>, StreamEnd) {
    let mut blocks: Vec<Block> = Vec::new();
    let mut started: HashMap<String, usize> = HashMap::new();
    let mut end = StreamEnd::default();
    let mut last_message = None;
    let mut last_at = None;
    for (index, line) in lines.iter().enumerate() {
        let at = clock.at(index as u64 + 1).or(last_at);
        last_at = at;
        let kind = line.get("type").and_then(Value::as_str).unwrap_or_default();
        let Some(item) = line.get("item") else {
            continue;
        };
        let id = text(item, "id").unwrap_or_default();
        let block = match item.get("type").and_then(Value::as_str) {
            Some("agent_message") if kind == "item.completed" => {
                let text = text(item, "text").unwrap_or_default();
                last_message = Some(text.clone());
                Some(Kind::Say(text))
            }
            Some("reasoning") if kind == "item.completed" => {
                let text = text(item, "text").unwrap_or_default();
                (!text.trim().is_empty()).then_some(Kind::Think(text))
            }
            Some("command_execution") => {
                let exit = item.get("exit_code").and_then(Value::as_i64);
                Some(Kind::Command {
                    command: unwrap_shell(&text(item, "command").unwrap_or_default()),
                    output: text(item, "aggregated_output").unwrap_or_default(),
                    failed: exit.is_some_and(|code| code != 0)
                        || text(item, "status").as_deref() == Some("failed"),
                    exit,
                })
            }
            Some("file_change") if kind == "item.completed" => {
                for change in item
                    .get("changes")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let action = match change.get("kind").and_then(Value::as_str) {
                        Some("add") => "Created",
                        Some("delete") => "Deleted",
                        _ => "Edited",
                    };
                    blocks.push(Block::new(
                        at,
                        Kind::Edit {
                            path: short_path(&text(change, "path").unwrap_or_default()),
                            action,
                            added: 0,
                            removed: 0,
                            body: String::new(),
                        },
                    ));
                }
                None
            }
            Some("mcp_tool_call" | "web_search") if kind == "item.completed" => Some(Kind::Tool {
                name: text(item, "type").unwrap_or_default(),
                input: text(item, "query")
                    .or_else(|| text(item, "tool"))
                    .unwrap_or_default(),
                output: String::new(),
            }),
            _ => None,
        };
        if let Some(block) = block {
            match started.get(&id) {
                Some(&index) if matches!(block, Kind::Command { .. }) => {
                    blocks[index].kind = block;
                }
                _ => {
                    blocks.push(Block::new(at, block));
                    if kind == "item.started" {
                        started.insert(id, blocks.len() - 1);
                    }
                }
            }
        }
    }
    if let Some(report) = &last_message {
        close_with_report(&mut blocks, report, last_at);
    }
    end.report = last_message;
    (blocks, end)
}

/// Ends a session's blocks with its final report: the last thing the
/// agent said becomes the report when it is the same text, so it is not
/// shown twice.
fn close_with_report(blocks: &mut Vec<Block>, report: &str, at: Option<i64>) {
    let same = |text: &str| text.trim() == report.trim();
    if let Some(index) = blocks
        .iter()
        .rposition(|block| matches!(&block.kind, Kind::Say(text) if same(text)))
    {
        blocks[index].kind = Kind::Report(report.to_owned());
    } else {
        blocks.push(Block::new(at, Kind::Report(report.to_owned())));
    }
}

/// Reads a native stream, Claude Code or Codex, whichever it is.
#[must_use]
pub fn native_stream(path: &Path, clock: &Clock) -> (Vec<Block>, StreamEnd) {
    let lines = read_lines(path);
    let codex = lines.iter().take(5).any(|line| {
        line.get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind == "thread.started" || kind.starts_with("item."))
    });
    if codex {
        codex_stream(&lines, clock)
    } else {
        claude_stream(&lines, clock)
    }
}

/// The session a native stream belongs to.
fn stream_session(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines().take(20) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(id) = text_of(&value, "session_id").or_else(|| text_of(&value, "thread_id")) {
            return Some(id);
        }
    }
    None
}

fn text_of(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// The executor streams in a Coder One episode's artifacts, in dispatch
/// order.
fn delegate_streams(episode: &Path) -> Vec<PathBuf> {
    let mut streams: Vec<(u64, PathBuf)> = std::fs::read_dir(episode.join("artifacts"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.to_owned();
            let stem = name.strip_suffix(".stream.jsonl")?;
            let order = stem
                .strip_prefix("delegate-")
                .and_then(|n| n.parse().ok())
                .unwrap_or(if stem == "delegate" { 0 } else { 1000 });
            Some((order, path))
        })
        .collect();
    streams.sort();
    streams.into_iter().map(|(_, path)| path).collect()
}

/// What a Jev question is asking, in plain words.
fn question_of(component: &str, name: &str) -> &'static str {
    match (component, name) {
        (_, "jev_requirements") | ("task.requirements", _) => "Which sentences are requirements?",
        (_, "jev_probe") | ("evidence.probes.selector", _) => "Which workspace probes are useful?",
        (_, "jev_survey") | ("evidence.select", _) => "Which files matter?",
        (_, "jev_profile") | ("task.profile", _) => "How hard is the task?",
        (_, "jev_coverage") | ("evidence.pack", _) => "What should the briefing cover?",
        (_, "jev_close") | ("verify.close", _) => "Is the task done?",
        (_, "jev_support") | ("verify.support", _) => "Does the evidence support each requirement?",
        (_, "jev_monitor") | ("control.monitor", _) => "Is the executor making progress?",
        (_, "jev_step") => "What should the next step be?",
        ("control.route", _) => "Which executor starts?",
        _ => "Jev judged",
    }
}

/// Condenses Jev's answers into one phrase and a detail list.
fn answers_phrase(answers: &Value) -> (String, Vec<(String, String)>) {
    let Some(map) = answers.as_object() else {
        return ("answered".to_owned(), Vec::new());
    };
    let mut detail = Vec::new();
    let (mut yes, mut total) = (0, 0);
    let mut choices: BTreeMap<String, usize> = BTreeMap::new();
    for (key, answer) in map {
        let value = match answer.get("type").and_then(Value::as_str) {
            Some("noul") => {
                let p = answer.get("noul").and_then(Value::as_f64).unwrap_or(0.0);
                total += 1;
                if p >= 0.5 {
                    yes += 1;
                }
                format!("{p:.2}")
            }
            Some("choice") => {
                let choice = text(answer, "choice").unwrap_or_default();
                *choices.entry(choice.clone()).or_default() += 1;
                match answer.get("confidence").and_then(Value::as_f64) {
                    Some(c) => format!("{choice} ({c:.2})"),
                    None => choice,
                }
            }
            Some("score") => {
                let score = answer.get("score").and_then(Value::as_f64).unwrap_or(0.0);
                format!("{score:.2}")
            }
            _ => compact_json(answer),
        };
        detail.push((key.replace('_', " "), value));
    }
    let phrase = if !choices.is_empty() {
        choices
            .iter()
            .map(|(choice, n)| format!("{n} {choice}"))
            .collect::<Vec<_>>()
            .join(", ")
    } else if total == 1 {
        let p = map
            .values()
            .find_map(|answer| answer.get("noul").and_then(Value::as_f64))
            .unwrap_or(0.0);
        format!("{} ({p:.2})", likelihood(p))
    } else if total > 0 {
        format!("yes to {yes} of {total}")
    } else {
        "answered".to_owned()
    };
    (phrase, detail)
}

/// `likely`, `unlikely`, and the words between.
#[must_use]
pub fn likelihood(p: f64) -> &'static str {
    match p {
        p if p >= 0.85 => "very likely",
        p if p >= 0.6 => "likely",
        p if p >= 0.4 => "unsure",
        p if p >= 0.15 => "unlikely",
        _ => "very unlikely",
    }
}

/// `the delegate mode is always` and its kin, in plain words.
fn why_phrase(why: &str) -> String {
    let why = why.trim().trim_end_matches('.');
    if why.contains("control.persist round") {
        let round = why.rsplit(' ').next().unwrap_or("");
        return format!("persistence round {round}: a fresh session to test and polish the work");
    }
    if why.contains("delegate mode is always") {
        return "Coder One always hands the work to an executor".to_owned();
    }
    if let Some(rest) = why.strip_prefix("control.handoff moved the work: ") {
        return format!("the work was handed over ({rest})");
    }
    why.replace("control.", "").replace("verify.", "")
}

/// Parses `Delegating to claude-code (claude-opus-5-5) because X. Briefing:
/// N characters, sha256 …`.
fn delegation(message: &str) -> Option<(String, Option<String>, Option<String>)> {
    let rest = message.strip_prefix("Delegating to ")?;
    let (who, rest) = rest.split_once(" because ").unwrap_or((rest, ""));
    let (agent, model) = match who.split_once(" (") {
        Some((agent, model)) => (agent, Some(model.trim_end_matches(')'))),
        None => (who, None),
    };
    let why = rest.split(". Briefing:").next().map(why_phrase);
    let chars = rest
        .split("Briefing: ")
        .nth(1)
        .and_then(|b| b.split(' ').next())
        .map(|n| format!("{n}-character briefing"));
    Some((
        executor_with_model(agent, model),
        why.filter(|why| !why.is_empty()),
        chars,
    ))
}

/// A check scenario's name, without the catalog's prefix.
fn scenario_name(name: &str) -> String {
    let name = name.strip_prefix("generic.").unwrap_or(name);
    match name.split_once(':') {
        Some((kind, subject)) => format!("{} {subject}", kind.replace(['-', '_'], " ")),
        None => name.replace(['-', '_'], " "),
    }
}

fn verdict_counts(verdicts: &Value) -> (String, Option<bool>) {
    let Some(map) = verdicts.as_object() else {
        return ("no verdicts".to_owned(), None);
    };
    let count = |key: &str| map.get(key).and_then(Value::as_u64).unwrap_or(0);
    let failed = count("failed") + count("contradicted");
    let parts: Vec<String> = map
        .iter()
        .filter_map(|(key, n)| {
            let n = n.as_u64()?;
            (n > 0).then(|| format!("{n} {}", key.replace('_', " ")))
        })
        .collect();
    let good = if failed > 0 {
        Some(false)
    } else if count("passed") > 0 {
        Some(true)
    } else {
        None
    };
    (parts.join(", "), good)
}

/// Builds a Coder One transcript from its episode log, splicing each
/// session's native stream in where the stream is on disk.
#[must_use]
pub fn coder_one(episode: Option<&Path>, log: &Path) -> Transcript {
    let steps: Vec<Value> = read_lines(log)
        .into_iter()
        .filter_map(|record| record.get("step").cloned())
        .collect();
    let mut transcript = Transcript {
        sources: vec![log.to_path_buf()],
        ..Transcript::default()
    };
    // Streams by the session they record, and each session's clock.
    let mut streams: HashMap<String, PathBuf> = HashMap::new();
    if let Some(episode) = episode {
        for path in delegate_streams(episode) {
            if let Some(id) = stream_session(&path) {
                streams.insert(id, path);
            }
        }
    }
    let mut clocks: HashMap<String, Clock> = HashMap::new();
    for step in &steps {
        if let Some(event) = step.pointer("/extensions/executor_event") {
            let (Some(id), Some(line), Some(at)) = (
                text(event, "session_id"),
                event.pointer("/event/line").and_then(Value::as_u64),
                step.get("at").and_then(Value::as_i64),
            ) else {
                continue;
            };
            clocks.entry(id).or_default().marks.push((line, at));
        }
    }
    for clock in clocks.values_mut() {
        clock.marks.sort_unstable();
        clock.marks.dedup_by_key(|(line, _)| *line);
    }

    let mut section: Option<usize> = None;
    let mut spliced: Vec<String> = Vec::new();
    let mut commands: HashMap<String, usize> = HashMap::new();
    let mut seen_task = false;
    let mut names: HashMap<String, String> = HashMap::new();
    for step in &steps {
        let at = step.get("at").and_then(Value::as_i64);
        let source = step.get("source").and_then(Value::as_str).unwrap_or("");
        let message = step.get("message").and_then(Value::as_str).unwrap_or("");
        if source == "User" && !seen_task {
            seen_task = true;
            transcript
                .blocks
                .push(Block::new(at, Kind::Task(message.to_owned())));
            continue;
        }
        if let Some((who, why, briefing)) = delegation(message) {
            let note = match (why, briefing) {
                (Some(why), Some(briefing)) => Some(format!("Why: {why}. It read a {briefing}.")),
                (Some(why), None) => Some(format!("Why: {why}.")),
                (None, Some(briefing)) => Some(format!("It read a {briefing}.")),
                (None, None) => None,
            };
            transcript.sessions.push(Session {
                who: who.clone(),
                why: note.clone(),
                ..Session::default()
            });
            transcript.blocks.push(Block::new(
                at,
                Kind::Section {
                    title: format!("{who} takes over"),
                    note,
                    milliseconds: None,
                    cost_usd: None,
                },
            ));
            section = Some(transcript.blocks.len() - 1);
            continue;
        }
        if let Some(event) = step.pointer("/extensions/executor_event") {
            let session = text(event, "session_id").unwrap_or_default();
            if let Some(path) = streams.get(&session) {
                if !spliced.contains(&session) {
                    spliced.push(session.clone());
                    let clock = clocks.get(&session).cloned().unwrap_or_default();
                    let (blocks, end) = native_stream(path, &clock);
                    count_into(transcript.sessions.last_mut(), &blocks);
                    transcript.blocks.extend(blocks);
                    if let Some(current) = transcript.sessions.last_mut() {
                        current.report = end.report;
                        current.turns = current.turns.or(end.turns);
                    }
                    transcript.sources.push(path.clone());
                }
                continue;
            }
            let event = event.get("event").unwrap_or(&Value::Null);
            match event.get("kind").and_then(Value::as_str) {
                Some("command_started") => {
                    let command = text(event, "command").unwrap_or_default();
                    transcript.blocks.push(Block::new(
                        at,
                        Kind::Command {
                            command: unwrap_shell(&command),
                            output: String::new(),
                            exit: None,
                            failed: false,
                        },
                    ));
                    commands.insert(session.clone(), transcript.blocks.len() - 1);
                    if let Some(current) = transcript.sessions.last_mut() {
                        current.commands += 1;
                    }
                }
                Some("command_completed") => {
                    let exit = event.get("exit_code").and_then(Value::as_i64);
                    let output = text(event, "output").unwrap_or_default();
                    if let Some(index) = commands.remove(&session)
                        && let Kind::Command {
                            output: held,
                            exit: held_exit,
                            failed,
                            ..
                        } = &mut transcript.blocks[index].kind
                    {
                        *held = output;
                        *held_exit = exit;
                        *failed = exit.is_some_and(|code| code != 0);
                    }
                }
                Some("assistant_claim") => {
                    let claim = text(event, "text").unwrap_or_default();
                    if !claim.trim().is_empty() {
                        transcript.blocks.push(Block::new(at, Kind::Say(claim)));
                    }
                }
                Some("artifact_changed") => {
                    let change = text(event, "change").unwrap_or_default();
                    transcript.blocks.push(Block::new(
                        at,
                        Kind::Edit {
                            path: short_path(&text(event, "path").unwrap_or_default()),
                            action: match change.as_str() {
                                "created" | "added" => "Created",
                                "deleted" | "removed" => "Deleted",
                                _ => "Edited",
                            },
                            added: 0,
                            removed: 0,
                            body: String::new(),
                        },
                    ));
                    if let Some(current) = transcript.sessions.last_mut() {
                        current.edits += 1;
                    }
                }
                Some("session_ended") => {
                    if let Some(result) = text(event, "result").filter(|r| !r.trim().is_empty()) {
                        if let Some(current) = transcript.sessions.last_mut() {
                            current.report = Some(result.clone());
                        }
                        transcript.blocks.push(Block::new(at, Kind::Report(result)));
                    }
                }
                _ => {}
            }
            continue;
        }
        if let Some(judgment) = step.pointer("/extensions/monitor_judgment") {
            transcript.monitor.0 += 1;
            let flagged = judgment
                .get("jev_flags")
                .or_else(|| judgment.get("rules"))
                .and_then(Value::as_object)
                .is_some_and(|flags| flags.values().any(|flag| flag.as_bool() == Some(true)));
            if flagged {
                transcript.monitor.1 += 1;
            }
            continue;
        }
        if let Some(handoff) = step.pointer("/extensions/handoff") {
            let from = text(handoff, "from").unwrap_or_default();
            let to = text(handoff, "to").unwrap_or_default();
            let trigger = text(handoff, "trigger").unwrap_or_default();
            transcript.blocks.push(Block::new(
                at,
                Kind::Check {
                    title: "Hand-off".to_owned(),
                    verdict: format!("moved the work from {from} to {to}"),
                    lines: vec![format!("Because {trigger}.")],
                    good: None,
                },
            ));
            continue;
        }
        let Some(invocation) = step.pointer("/extensions/invocation") else {
            if source == "Agent" && message.starts_with("finished by the delegate") {
                continue;
            }
            continue;
        };
        let id = text(invocation, "id").unwrap_or_default();
        if invocation.get("event").and_then(Value::as_str) != Some("end") {
            if let Some(name) = text(invocation, "name") {
                names.insert(id, name);
            }
            continue;
        }
        let name = names.get(&id).cloned().unwrap_or_default();
        let component = text(invocation, "component").unwrap_or_default();
        let summary = invocation
            .pointer("/output/summary")
            .cloned()
            .unwrap_or(Value::Null);
        let milliseconds = invocation.get("milliseconds").and_then(Value::as_u64);
        let cost = invocation.pointer("/cost/usd").and_then(Value::as_f64);
        let outcome = text(invocation, "outcome").unwrap_or_default();
        if let Some(block) =
            invocation_block(&component, &name, &summary, &outcome, milliseconds, cost)
        {
            transcript.blocks.push(Block::new(at, block));
        }
        if component == "exec.session"
            && let Some(index) = section
        {
            if let Kind::Section {
                milliseconds: held_ms,
                cost_usd,
                ..
            } = &mut transcript.blocks[index].kind
            {
                *held_ms = milliseconds;
                *cost_usd = cost;
            }
            if let Some(current) = transcript.sessions.last_mut() {
                current.milliseconds = milliseconds;
                current.cost_usd = cost;
                current.turns = current
                    .turns
                    .or_else(|| summary.get("turns").and_then(Value::as_u64));
            }
            section = None;
        }
    }
    transcript.blocks = coalesce(std::mem::take(&mut transcript.blocks));
    transcript
}

/// Folds runs of like blocks into one: Coder One's looks around the
/// workspace, and the same Jev question asked over and over, each read
/// better as one line with the detail behind it.
fn coalesce(blocks: Vec<Block>) -> Vec<Block> {
    let key = |block: &Block| match &block.kind {
        Kind::Look { what, .. } if what.starts_with("Coder One looked: ") => {
            Some("look".to_owned())
        }
        Kind::Decision { question, .. } => Some(format!("ask {question}")),
        _ => None,
    };
    let mut out: Vec<Block> = Vec::with_capacity(blocks.len());
    let mut run: Vec<Block> = Vec::new();
    for block in blocks {
        let this = key(&block);
        if this.is_some() && run.first().and_then(key) == this {
            run.push(block);
            continue;
        }
        flush(&mut run, &mut out);
        if this.is_some() {
            run.push(block);
        } else {
            out.push(block);
        }
    }
    flush(&mut run, &mut out);
    out
}

/// Ends a run of like blocks: fewer than three stay as they are.
fn flush(run: &mut Vec<Block>, out: &mut Vec<Block>) {
    if run.len() < 3 {
        out.append(run);
        return;
    }
    let at = run[0].at;
    let count = run.len();
    let merged = match &run[0].kind {
        Kind::Decision { question, .. } => {
            let mut detail = Vec::new();
            let (mut ms, mut usd) = (None::<u64>, None::<f64>);
            for (n, block) in run.iter().enumerate() {
                if let Kind::Decision {
                    answer,
                    milliseconds,
                    cost_usd,
                    ..
                } = &block.kind
                {
                    detail.push((format!("{}", n + 1), answer.clone()));
                    if let Some(add) = milliseconds {
                        ms = Some(ms.unwrap_or(0) + add);
                    }
                    if let Some(add) = cost_usd {
                        usd = Some(usd.unwrap_or(0.0) + add);
                    }
                }
            }
            Kind::Decision {
                question: question.clone(),
                answer: format!("asked {count} times"),
                detail,
                probability: None,
                milliseconds: ms,
                cost_usd: usd,
            }
        }
        _ => Kind::Look {
            what: format!("Coder One looked around the workspace ({count} looks)"),
            output: run
                .iter()
                .filter_map(|block| match &block.kind {
                    Kind::Look { what, .. } => {
                        Some(what.trim_start_matches("Coder One looked: ").to_owned())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        },
    };
    run.clear();
    out.push(Block::new(at, merged));
}

fn count_into(session: Option<&mut Session>, blocks: &[Block]) {
    if let Some(session) = session {
        for block in blocks {
            match block.kind {
                Kind::Command { .. } => session.commands += 1,
                Kind::Edit { .. } => session.edits += 1,
                _ => {}
            }
        }
    }
}

/// The block one ended invocation reads as, if people need to see it.
fn invocation_block(
    component: &str,
    name: &str,
    summary: &Value,
    outcome: &str,
    milliseconds: Option<u64>,
    cost_usd: Option<f64>,
) -> Option<Kind> {
    let decision =
        |answer: String, detail: Vec<(String, String)>, probability: Option<f64>| Kind::Decision {
            question: question_of(component, "").to_owned(),
            answer,
            detail,
            probability,
            milliseconds,
            cost_usd,
        };
    match component {
        "task.requirements"
        | "evidence.probes.selector"
        | "evidence.select"
        | "evidence.pack"
        | "verify.close" => {
            let answers = summary.get("answers")?;
            let single = answers
                .as_object()
                .filter(|map| map.len() == 1)
                .and_then(|map| {
                    map.values()
                        .next()
                        .and_then(|answer| answer.get("noul"))
                        .and_then(Value::as_f64)
                });
            let (phrase, detail) = if component == "task.requirements" {
                let (_, detail) = answers_phrase(answers);
                let sentences = answers.as_object().map_or(0, serde_json::Map::len);
                let (kinds, _) = answers_phrase(answers);
                (
                    format!("sorted {}: {kinds}", plural(sentences, "sentence")),
                    detail,
                )
            } else if component == "verify.close" {
                let p = answers
                    .pointer("/done/noul")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                (
                    format!("{} ({p:.2})", likelihood(p)),
                    answers_phrase(answers).1,
                )
            } else {
                answers_phrase(answers)
            };
            let probability = if component == "verify.close" {
                answers.pointer("/done/noul").and_then(Value::as_f64)
            } else {
                single
            };
            Some(decision(phrase, detail, probability))
        }
        "task.profile" => {
            let answers = summary.get("answers")?;
            let (_, detail) = answers_phrase(answers);
            let score = answers.pointer("/difficulty/score").and_then(Value::as_f64);
            let answer = score.map_or("answered".to_owned(), |score| {
                format!("difficulty {score:.1} on Jev's scale")
            });
            Some(decision(answer, detail, None))
        }
        "control.route" => {
            let agent = summary
                .pointer("/tier/agent")
                .and_then(Value::as_str)
                .unwrap_or("executor");
            let model = summary.pointer("/tier/model").and_then(Value::as_str);
            let reason = text(summary, "reason").unwrap_or_default();
            let mut detail = vec![("reason".to_owned(), reason.clone())];
            if let Some(difficulty) = summary
                .pointer("/profile/difficulty")
                .and_then(Value::as_f64)
            {
                detail.push(("difficulty".to_owned(), format!("{difficulty:.2}")));
            }
            Some(decision(
                format!("{} — {reason}", executor_with_model(agent, model)),
                detail,
                None,
            ))
        }
        "host.operation" => {
            let label = text(summary, "label")?;
            let exit = summary.get("exit").and_then(Value::as_i64);
            Some(Kind::Look {
                what: format!(
                    "Coder One looked: {label}{}",
                    match exit {
                        Some(0) | None => String::new(),
                        Some(code) => format!(" (exit {code})"),
                    }
                ),
                output: String::new(),
            })
        }
        "verify.checks" => {
            let (verdict, good) = verdict_counts(summary.get("verdicts").unwrap_or(&Value::Null));
            let mut lines = Vec::new();
            if let Some(requirements) = summary.get("requirements").and_then(Value::as_object) {
                let parts: Vec<String> = requirements
                    .iter()
                    .filter_map(|(state, n)| Some(format!("{} {state}", n.as_u64()?)))
                    .collect();
                lines.push(format!("Requirements: {}.", parts.join(", ")));
            }
            Some(Kind::Check {
                title: "Coder One's checks".to_owned(),
                verdict: if verdict.is_empty() {
                    "no scenario ran".to_owned()
                } else {
                    verdict
                },
                lines,
                good,
            })
        }
        "verify.checks.run" => {
            let verdict = text(summary, "verdict")?;
            let coverage: Vec<String> = summary
                .get("coverage")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            Some(Kind::Note(format!(
                "check {}: {verdict}{}",
                if name.is_empty() {
                    "scenario".to_owned()
                } else {
                    scenario_name(name)
                },
                coverage
                    .first()
                    .map(|why| format!(" — {why}"))
                    .unwrap_or_default()
            )))
        }
        "verify.support" if summary.get("judged").is_some() => {
            let n = |key: &str| summary.get(key).and_then(Value::as_u64).unwrap_or(0);
            Some(Kind::Check {
                title: "Evidence review".to_owned(),
                verdict: format!(
                    "{} supported, {} contradicted, {} unresolved of {} judged",
                    n("supported"),
                    n("contradicted"),
                    n("unresolved"),
                    n("judged")
                ),
                lines: Vec::new(),
                good: if n("contradicted") > 0 {
                    Some(false)
                } else if n("supported") > 0 {
                    Some(true)
                } else {
                    None
                },
            })
        }
        "verify.repair" => {
            if let Some(skipped) = text(summary, "skipped") {
                return Some(Kind::Check {
                    title: "Repair".to_owned(),
                    verdict: format!("not needed — {skipped}"),
                    lines: Vec::new(),
                    good: None,
                });
            }
            let changed = summary.get("changed").and_then(Value::as_bool);
            Some(Kind::Check {
                title: "Repair".to_owned(),
                verdict: match changed {
                    Some(true) => "a repair session changed the work".to_owned(),
                    Some(false) => "a repair session ran and changed nothing".to_owned(),
                    None => outcome.to_owned(),
                },
                lines: Vec::new(),
                good: None,
            })
        }
        "verify.second" => {
            let verdict = match text(summary, "skipped") {
                Some(skipped) => format!("not needed — {skipped}"),
                None => match text(summary, "kept") {
                    Some(kept) => format!(
                        "a second executor tried; Coder One kept the {kept} result{}",
                        text(summary, "why")
                            .map(|why| format!(" ({why})"))
                            .unwrap_or_default()
                    ),
                    None => outcome.to_owned(),
                },
            };
            Some(Kind::Check {
                title: "Second opinion".to_owned(),
                verdict,
                lines: Vec::new(),
                good: None,
            })
        }
        "control.persist" => {
            let round = summary.get("round").and_then(Value::as_u64)?;
            let kept = summary.get("kept").and_then(Value::as_bool);
            let files: Vec<String> = summary
                .get("files_changed")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|file| text(file, "path"))
                .collect();
            let mut lines = Vec::new();
            if !files.is_empty() {
                lines.push(format!("Changed: {}.", files.join(", ")));
            }
            if let (Some(before), Some(after)) = (summary.get("before"), summary.get("after")) {
                lines.push(if before == after {
                    "The checks came out the same as before.".to_owned()
                } else {
                    "The checks came out differently from before.".to_owned()
                });
            }
            Some(Kind::Check {
                title: format!("Persistence round {round}"),
                verdict: match kept {
                    Some(true) => "kept the round's work".to_owned(),
                    Some(false) => "put the workspace back".to_owned(),
                    None => outcome.to_owned(),
                },
                lines,
                good: None,
            })
        }
        _ => None,
    }
}

/// Builds a transcript from an ATIF trajectory document: Harbor's for a
/// Claude Code or Codex trial, or Coder One's older export. `splice`
/// supplies the executor streams a Coder One `delegate` call handed off
/// to, in order.
#[must_use]
pub fn trajectory(path: &Path, mut splice: Vec<PathBuf>) -> Transcript {
    let mut transcript = Transcript {
        sources: vec![path.to_path_buf()],
        ..Transcript::default()
    };
    let Ok(value) = std::fs::read(path)
        .map_err(|_| ())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).map_err(|_| ()))
    else {
        return transcript;
    };
    let agent = value
        .pointer("/agent/name")
        .and_then(Value::as_str)
        .unwrap_or("agent")
        .to_owned();
    let model = value
        .pointer("/agent/model_name")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let native = agent != "coder-one";
    if native {
        transcript.sessions.push(Session {
            who: executor_with_model(&agent, model.as_deref()),
            cost_usd: value
                .pointer("/final_metrics/total_cost_usd")
                .and_then(Value::as_f64),
            ..Session::default()
        });
    }
    splice.reverse();
    let mut seen_task = false;
    let mut first_at = None;
    let mut last_at = None;
    for step in value
        .get("steps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let at = step
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(timestamp_ms);
        if first_at.is_none() {
            first_at = at;
        }
        last_at = at.or(last_at);
        let source = step.get("source").and_then(Value::as_str).unwrap_or("");
        let message = content_text(step.get("message").unwrap_or(&Value::Null));
        match source {
            "user" => {
                if message.trim_start().starts_with('<') {
                    continue;
                }
                if seen_task {
                    transcript.blocks.push(Block::new(at, Kind::Say(message)));
                } else {
                    seen_task = true;
                    transcript.blocks.push(Block::new(at, Kind::Task(message)));
                }
                continue;
            }
            "system" => continue,
            _ => {}
        }
        if let Some(reasoning) = step
            .get("reasoning_content")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
        {
            transcript
                .blocks
                .push(Block::new(at, Kind::Think(reasoning.to_owned())));
        }
        if !message.trim().is_empty() {
            // Coder One's own loop writes its choice as JSON; its reason is
            // the part a person reads.
            let said = serde_json::from_str::<Value>(&message)
                .ok()
                .and_then(|choice| text(&choice, "reason"))
                .unwrap_or(message.clone());
            if !said.starts_with("finished by the delegate") && !said.starts_with("Delegated to ") {
                transcript.blocks.push(Block::new(at, Kind::Say(said)));
            }
        }
        let results: HashMap<String, Value> = step
            .pointer("/observation/results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|result| Some((text(result, "source_call_id")?, result.clone())))
            .collect();
        for call in step
            .get("tool_calls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = text(call, "function_name").unwrap_or_default();
            let id = text(call, "tool_call_id").unwrap_or_default();
            let arguments = call.get("arguments").cloned().unwrap_or(Value::Null);
            let result = results.get(&id);
            if name.starts_with("jev_") {
                transcript.blocks.push(Block::new(
                    at,
                    Kind::Decision {
                        question: question_of("", &name).to_owned(),
                        answer: "answered".to_owned(),
                        detail: Vec::new(),
                        probability: None,
                        milliseconds: None,
                        cost_usd: None,
                    },
                ));
                continue;
            }
            if name == "delegate" {
                let agent = text(&arguments, "agent").unwrap_or_else(|| "executor".to_owned());
                let model = step.get("model_name").and_then(Value::as_str);
                let who = executor_with_model(&agent, model);
                transcript.sessions.push(Session {
                    who: who.clone(),
                    ..Session::default()
                });
                transcript.blocks.push(Block::new(
                    at,
                    Kind::Section {
                        title: format!("{who} takes over"),
                        note: None,
                        milliseconds: None,
                        cost_usd: None,
                    },
                ));
                if let Some(stream) = splice.pop() {
                    let (blocks, end) = native_stream(&stream, &Clock::default());
                    count_into(transcript.sessions.last_mut(), &blocks);
                    // A stream with no clock keeps no times rather than
                    // borrowing the hand-off's.
                    transcript.blocks.extend(blocks);
                    if let Some(current) = transcript.sessions.last_mut() {
                        current.report = end.report;
                        current.cost_usd = end.cost_usd;
                        current.milliseconds = end.milliseconds;
                        current.turns = end.turns;
                    }
                    transcript.sources.push(stream);
                }
                continue;
            }
            if name == "finished" {
                continue;
            }
            let output = result.map_or_else(String::new, harbor_output);
            let failed = result.is_some_and(|result| {
                result.pointer("/extra/status").and_then(Value::as_str) == Some("failed")
                    || result
                        .pointer("/extra/tool_result_is_error")
                        .and_then(Value::as_bool)
                        == Some(true)
            }) || call
                .pointer("/extra/exit")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                != 0;
            let arguments = if name == "exec" {
                codex_exec_input(&arguments)
            } else {
                arguments
            };
            let tool = if name == "exec" {
                "exec_command"
            } else {
                &name
            };
            let mut kind = tool_block(tool, &arguments, output, failed);
            if let Kind::Command { exit, .. } = &mut kind
                && exit.is_none()
            {
                *exit = call.pointer("/extra/exit").and_then(Value::as_i64);
            }
            transcript.blocks.push(Block::new(at, kind));
        }
    }
    if native {
        let final_text = transcript
            .blocks
            .iter()
            .rev()
            .find_map(|block| match &block.kind {
                Kind::Say(text) => Some(text.clone()),
                _ => None,
            });
        if let Some(report) = &final_text {
            close_with_report(&mut transcript.blocks, report, last_at);
        }
        let commands = transcript
            .blocks
            .iter()
            .filter(|block| matches!(block.kind, Kind::Command { .. }))
            .count();
        let edits = transcript
            .blocks
            .iter()
            .filter(|block| matches!(block.kind, Kind::Edit { .. }))
            .count();
        if let Some(session) = transcript.sessions.first_mut() {
            session.report = final_text;
            session.commands = commands;
            session.edits = edits;
            session.milliseconds = first_at
                .zip(last_at)
                .and_then(|(first, last)| u64::try_from(last - first).ok());
            session.turns = value
                .pointer("/final_metrics/total_steps")
                .and_then(Value::as_u64);
        }
    }
    transcript.blocks = coalesce(std::mem::take(&mut transcript.blocks));
    transcript
}

/// Harbor keeps Claude Code's clean output in the result's metadata; the
/// plain content repeats it with markers.
fn harbor_output(result: &Value) -> String {
    let meta = result.pointer("/extra/tool_result_metadata/tool_use_result");
    if let Some(meta) = meta
        && (meta.get("stdout").is_some() || meta.get("stderr").is_some())
    {
        let stdout = meta.get("stdout").and_then(Value::as_str).unwrap_or("");
        let stderr = meta.get("stderr").and_then(Value::as_str).unwrap_or("");
        return match (stdout.is_empty(), stderr.is_empty()) {
            (_, true) => stdout.to_owned(),
            (true, false) => stderr.to_owned(),
            (false, false) => format!("{stdout}\n{stderr}"),
        };
    }
    content_text(result.get("content").unwrap_or(&Value::Null))
}

/// Codex's `exec` tool takes a script; the command is its `cmd`.
fn codex_exec_input(arguments: &Value) -> Value {
    let script = arguments
        .get("input")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let command = script
        .split_once("cmd:\"")
        .and_then(|(_, rest)| {
            let mut out = String::new();
            let mut chars = rest.chars();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => match chars.next() {
                        Some('n') => out.push('\n'),
                        Some('t') => out.push('\t'),
                        Some(other) => out.push(other),
                        None => {}
                    },
                    '"' => return Some(out),
                    other => out.push(other),
                }
            }
            None
        })
        .unwrap_or_else(|| script.to_owned());
    serde_json::json!({ "command": command })
}

/// Builds a transcript from a Harbor trial's native output alone — what a
/// Claude Code or Codex trial in progress has.
#[must_use]
pub fn native(path: &Path, who: String) -> Transcript {
    let (blocks, end) = native_stream(path, &Clock::default());
    let mut session = Session {
        who,
        report: end.report.clone(),
        cost_usd: end.cost_usd,
        milliseconds: end.milliseconds,
        turns: end.turns,
        ..Session::default()
    };
    count_into(Some(&mut session), &blocks);
    Transcript {
        blocks,
        sources: vec![path.to_path_buf()],
        sessions: vec![session],
        monitor: (0, 0),
    }
}

/// All of a Coder One episode's streams, for the older layout that has a
/// trajectory and no episode log.
#[must_use]
pub fn episode_streams(episode: &Path) -> Vec<PathBuf> {
    delegate_streams(episode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_coder_one_transcript_splices_each_session_in_its_place() {
        let (_dir, sources) = crate::runs::fixture_sources();
        let episode = sources.jobs.unwrap().join(
            "tb4--coder-one-tunable-v6--coq-block-bound/coq-block-bound__Mu8ygpJ/agent/episode",
        );
        let transcript = coder_one(Some(&episode), &episode.join("episode.atif.jsonl"));
        let heads: Vec<String> = transcript.blocks.iter().map(Block::headline).collect();
        assert_eq!(heads[0], "The task");
        let sections: Vec<&String> = heads.iter().filter(|h| h.ends_with("takes over")).collect();
        assert_eq!(sections.len(), 3, "{heads:#?}");
        let reports = heads.iter().filter(|h| *h == "Final report").count();
        assert_eq!(
            reports, 3,
            "each session ends in its report, once: {heads:#?}"
        );
        let position = |needle: &str| {
            heads
                .iter()
                .position(|h| h.contains(needle))
                .unwrap_or_else(|| panic!("{needle}: {heads:#?}"))
        };
        assert!(position("Which executor starts?") < position("takes over"));
        assert!(position("takes over") < position("$ coqc --version"));
        assert!(position("$ coqc --version") < position("Coder One's checks"));
        assert!(position("Coder One's checks") < position("Persistence round 1"));
        assert!(
            heads
                .iter()
                .any(|h| h.starts_with("Coder One looked around the workspace (6 looks)"))
        );
        assert!(
            heads
                .iter()
                .any(|h| h == "What should the briefing cover? asked 4 times")
        );
        // The sessions stream in with the log's clock.
        let command = transcript
            .blocks
            .iter()
            .find(|block| matches!(block.kind, Kind::Command { .. }))
            .unwrap();
        assert!(command.at.is_some());
        assert_eq!(transcript.monitor.0, 61);
    }

    #[test]
    fn a_harbor_trajectory_reads_as_one_session() {
        let (_dir, sources) = crate::runs::fixture_sources();
        let path = sources.jobs.unwrap().join(
            "tb4--claude-code-opus--wal-recovery-ordering/wal-recovery-ordering__9xaN7wM/agent/trajectory.json",
        );
        let transcript = trajectory(&path, Vec::new());
        assert_eq!(transcript.sessions.len(), 1);
        let session = &transcript.sessions[0];
        assert_eq!(session.who, "Claude Code on Opus 5.5");
        assert_eq!(session.commands, 6);
        assert!(
            session
                .report
                .as_deref()
                .is_some_and(|r| r.starts_with("I've fixed"))
        );
        assert!(matches!(transcript.blocks[0].kind, Kind::Task(_)));
        assert!(matches!(
            transcript.blocks.last().map(|b| &b.kind),
            Some(Kind::Report(_))
        ));
    }

    #[test]
    fn model_names_read_the_way_people_say_them() {
        assert_eq!(model_name("claude-opus-5-5"), "Opus 5.5");
        assert_eq!(model_name("claude-sonnet-4-6-20260101"), "Sonnet 4.6");
        assert_eq!(model_name("gpt-6-luna"), "GPT-6 Luna");
        assert_eq!(model_name("mystery"), "mystery");
        assert_eq!(
            executor_with_model("claude-code", Some("claude-opus-5-5")),
            "Claude Code on Opus 5.5"
        );
        assert_eq!(executor_with_model("codex", Some("free")), "Codex");
    }

    #[test]
    fn a_claude_stream_pairs_each_call_with_its_result() {
        let lines = vec![
            json!({"type": "system", "subtype": "init", "session_id": "s"}),
            json!({"type": "assistant", "message": {"content": [
                {"type": "text", "text": "I'll look first."},
                {"type": "tool_use", "id": "a", "name": "Bash", "input": {"command": "ls /app"}},
                {"type": "tool_use", "id": "b", "name": "Edit", "input": {"file_path": "/app/x.py", "old_string": "a", "new_string": "b\nc"}}
            ]}}),
            json!({"type": "user", "message": {"content": [
                {"type": "tool_result", "tool_use_id": "a", "content": "Exit code 2\nno such file", "is_error": true},
                {"type": "tool_result", "tool_use_id": "b", "content": "ok"}
            ]}}),
            json!({"type": "result", "result": "Done.", "total_cost_usd": 0.5, "duration_ms": 1000, "num_turns": 2}),
        ];
        let (blocks, end) = claude_stream(&lines, &Clock::default());
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[3].kind, Kind::Report("Done.".to_owned()));
        assert_eq!(blocks[0].kind, Kind::Say("I'll look first.".to_owned()));
        match &blocks[1].kind {
            Kind::Command {
                command,
                exit,
                failed,
                output,
            } => {
                assert_eq!(command, "ls /app");
                assert_eq!(*exit, Some(2));
                assert!(*failed);
                assert_eq!(output, "no such file");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(blocks[2].headline(), "Edited /app/x.py  +2 −1");
        assert_eq!(end.report.as_deref(), Some("Done."));
        assert_eq!(end.cost_usd, Some(0.5));
    }

    #[test]
    fn a_codex_stream_unwraps_its_shell_and_keeps_file_changes() {
        let lines = vec![
            json!({"type": "thread.started", "thread_id": "t"}),
            json!({"type": "item.started", "item": {"id": "1", "type": "command_execution", "command": "/bin/bash -lc 'git status'", "aggregated_output": "", "exit_code": null, "status": "in_progress"}}),
            json!({"type": "item.completed", "item": {"id": "1", "type": "command_execution", "command": "/bin/bash -lc 'git status'", "aggregated_output": "clean\n", "exit_code": 0, "status": "completed"}}),
            json!({"type": "item.completed", "item": {"id": "2", "type": "file_change", "changes": [{"path": "/app/a.md", "kind": "update"}]}}),
            json!({"type": "item.completed", "item": {"id": "3", "type": "agent_message", "text": "Merged."}}),
        ];
        let (blocks, end) = codex_stream(&lines, &Clock::default());
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].headline(), "$ git status   (1 line)");
        assert_eq!(blocks[1].headline(), "Edited /app/a.md");
        assert_eq!(end.report.as_deref(), Some("Merged."));
    }

    #[test]
    fn long_output_is_cut_with_a_count() {
        let output = (0..200)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let block = Block::new(
            None,
            Kind::Command {
                command: "seq 200".to_owned(),
                output,
                exit: Some(0),
                failed: false,
            },
        );
        assert!(block.body(false).is_empty());
        let body = block.body(true);
        assert_eq!(body.len(), OUTPUT_LINES + 1);
        assert_eq!(body.last().unwrap(), "… 140 more lines");
    }

    #[test]
    fn a_delegation_message_reads_as_who_and_why() {
        let (who, why, briefing) = delegation(
            "Delegating to claude-code (claude-opus-5-5) because the delegate mode is always. Briefing: 5670 characters, sha256 abc.",
        )
        .unwrap();
        assert_eq!(who, "Claude Code on Opus 5.5");
        assert_eq!(
            why.as_deref(),
            Some("Coder One always hands the work to an executor")
        );
        assert_eq!(briefing.as_deref(), Some("5670-character briefing"));
    }
}
