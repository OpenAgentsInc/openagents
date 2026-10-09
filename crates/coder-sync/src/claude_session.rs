//! A Claude Code session as a trace tree (#11178): the main conversation
//! and every agent it started, each one an ATIF trajectory, linked parent
//! to child, ready for [`crate::traces::upload_tree`].
//!
//! Claude Code keeps a session as JSON lines at
//! `~/.claude/projects/<project>/<session>.jsonl`, and the agents it started
//! (and the agents those started) beside it, flat, in
//! `<session>/subagents/agent-<id>.jsonl`, each with an `agent-<id>.meta.json`
//! naming its parent agent (`parentAgentId`, none for the main thread's own
//! agents) and the tool call that started it (`toolUseId`). This reads those
//! files and nothing else, read-only.
//!
//! Each file becomes one trajectory: the person's messages (typed, or queued
//! while the agent was busy), the agent's replies, thinking, tool calls and
//! their results, and the agent hand-back notices, with their times; tokens
//! and an estimated cost (at Anthropic's list prices) per reply. Every
//! string is redacted with the given [`Screen`] before anything is cut, so a
//! cut never splits a credential past the screen. Long tool output is
//! shortened (head and tail) so each trajectory fits [`NODE_BYTES`];
//! `extra.shortened_to` says how far. Images are left out.

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use secret_screen::{Counts, Screen};
use serde_json::{Map, Value, json};

/// The largest one trajectory may be once converted, leaving room under
/// the website's 8 MB for redaction and encoding.
pub const NODE_BYTES: usize = 7 * 1024 * 1024;

/// What a Claude Code agent is called in a trace.
pub const AGENT_NAME: &str = "claude-code";

/// One trajectory in the tree.
#[derive(Clone, Debug)]
pub struct Node {
    /// `main`, or the Claude Code agent id.
    pub key: String,
    /// The index of the parent node; `None` for the main conversation.
    pub parent: Option<usize>,
    pub document: Value,
}

/// Whether `path` looks like a Claude Code session rather than an ATIF
/// file: a `.jsonl` file that isn't Coder's own `.atif.jsonl` log.
#[must_use]
pub fn is_session_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    name.ends_with(".jsonl") && !name.ends_with(".atif.jsonl")
}

/// The session's main file from what the person typed: a `.jsonl` file, a
/// session folder (the one holding `subagents/`), or a session id looked up
/// under `~/.claude/projects`.
///
/// # Errors
/// When nothing matches, in words to show.
pub fn find(arg: &str, home: &Path, cwd: &Path) -> Result<PathBuf, String> {
    let given = cwd.join(arg);
    if given.is_file() {
        return Ok(given);
    }
    if given.is_dir() {
        let beside = given.with_extension("jsonl");
        if beside.is_file() {
            return Ok(beside);
        }
        return Err(format!(
            "{arg} is a folder, but there's no {} beside it.",
            beside
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
        ));
    }
    let id = arg.trim_end_matches(".jsonl");
    if !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        let projects = home.join(".claude").join("projects");
        let mut found: Vec<PathBuf> = std::fs::read_dir(&projects)
            .into_iter()
            .flatten()
            .flatten()
            .map(|project| project.path().join(format!("{id}.jsonl")))
            .filter(|path| path.is_file())
            .collect();
        found.sort();
        if let Some(path) = found.into_iter().next() {
            return Ok(path);
        }
    }
    Err(format!(
        "No Claude Code session {arg}. Give its id, its .jsonl file, or its folder."
    ))
}

/// The session at `main` and every agent in its `subagents/` folder, as a
/// tree: the main conversation first, every parent before its children.
///
/// # Errors
/// When the main file can't be read or holds no conversation.
pub fn convert(main: &Path, screen: &Screen) -> Result<(Vec<Node>, Counts), String> {
    let mut counts = Counts::new();
    let lines = read_lines(main)?;
    let session = main
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
    let title = lines
        .iter()
        .rev()
        .find(|line| line["type"] == "ai-title")
        .and_then(|line| line["aiTitle"].as_str())
        .map(str::to_owned);
    // Older Claude Code versions wrote their agents' lines into the main
    // file as side chains; the main conversation is the rest.
    let main_lines: Vec<Value> = lines
        .into_iter()
        .filter(|line| line["isSidechain"].as_bool() != Some(true))
        .collect();
    let mut root = trajectory(&main_lines, &session, screen, &mut counts);
    if root["steps"].as_array().is_none_or(Vec::is_empty) {
        return Err("That session has no conversation in it.".into());
    }
    if let Some(title) = title {
        root["extra"]["title"] = json!(screen_text(&title, screen, &mut counts));
    }
    root["extra"]["claude_session_id"] = json!(session);
    let mut nodes = vec![Node {
        key: "main".into(),
        parent: None,
        document: root,
    }];

    // The agents, flat in subagents/, each naming its parent.
    let folder = main.with_extension("").join("subagents");
    let mut agents: Vec<(String, Value, Value)> = Vec::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    files.sort();
    for path in files {
        let Some(id) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.strip_prefix("agent-"))
            .map(str::to_owned)
        else {
            continue;
        };
        let meta: Value = std::fs::read(path.with_extension("meta.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(Value::Null);
        let Ok(lines) = read_lines(&path) else {
            continue;
        };
        let mut document = trajectory(&lines, &id, screen, &mut counts);
        if document["steps"].as_array().is_none_or(Vec::is_empty) {
            continue;
        }
        let extra = &mut document["extra"];
        extra["claude_agent_id"] = json!(id);
        if let Some(description) = meta["description"].as_str() {
            extra["title"] = json!(screen_text(description, screen, &mut counts));
        }
        for (from, to) in [
            ("agentType", "agent_type"),
            ("toolUseId", "parent_tool_call_id"),
        ] {
            if let Some(value) = meta[from].as_str() {
                extra[to] = json!(value);
            }
        }
        agents.push((id, meta, document));
    }
    // Parents before children: by depth, then by start time.
    let parent_of: HashMap<String, Option<String>> = agents
        .iter()
        .map(|(id, meta, _)| {
            (
                id.clone(),
                meta["parentAgentId"].as_str().map(str::to_owned),
            )
        })
        .collect();
    let depth = |id: &str| {
        let mut depth = 0;
        let mut at = id.to_owned();
        while let Some(Some(parent)) = parent_of.get(&at) {
            depth += 1;
            if depth > 64 {
                break;
            }
            at.clone_from(parent);
        }
        depth
    };
    agents.sort_by_cached_key(|(id, _, document)| {
        (
            depth(id),
            document["steps"][0]["timestamp"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            id.clone(),
        )
    });
    let mut index: HashMap<String, usize> = HashMap::new();
    for (id, meta, document) in agents {
        let parent = meta["parentAgentId"]
            .as_str()
            .and_then(|parent| index.get(parent).copied())
            .unwrap_or(0);
        index.insert(id.clone(), nodes.len());
        nodes.push(Node {
            key: id,
            parent: Some(parent),
            document,
        });
    }
    for node in &mut nodes {
        fit(&mut node.document, NODE_BYTES);
    }
    Ok((nodes, counts))
}

fn read_lines(path: &Path) -> Result<Vec<Value>, String> {
    let file = File::open(path).map_err(|_| format!("Couldn't open {}.", path.display()))?;
    Ok(BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
        .collect())
}

fn screen_text(text: &str, screen: &Screen, counts: &mut Counts) -> String {
    let mut value = Value::String(text.to_owned());
    for (rule, count) in screen.redact_document(&mut value) {
        *counts.entry(rule).or_default() += count;
    }
    value.as_str().unwrap_or_default().to_owned()
}

/// The longest text one conversion keeps before [`fit`] cuts further.
const FIRST_CUT: usize = 64 * 1024;

/// Price per million tokens: input, output, cache read. Cache writes cost
/// 1.25 times input (five minutes) or twice input (an hour).
fn price(model: &str) -> Option<(f64, f64, f64)> {
    let model = model.trim_start_matches("anthropic.");
    let table: [(&str, (f64, f64, f64)); 15] = [
        ("claude-fable-5-1", (10.0, 50.0, 0.25)),
        ("claude-mythos", (10.0, 50.0, 1.0)),
        ("claude-fable", (10.0, 50.0, 1.0)),
        ("claude-opus-5-5", (4.0, 20.0, 0.2)),
        ("claude-opus-5", (5.0, 25.0, 0.5)),
        ("claude-opus-4-5", (5.0, 25.0, 0.5)),
        ("claude-opus-4-6", (5.0, 25.0, 0.5)),
        ("claude-opus-4-7", (5.0, 25.0, 0.5)),
        ("claude-opus-4-8", (5.0, 25.0, 0.5)),
        ("claude-opus-4", (15.0, 75.0, 1.5)),
        ("claude-sonnet-5", (2.0, 10.0, 0.2)),
        ("claude-sonnet-4", (3.0, 15.0, 0.3)),
        ("claude-haiku-5", (0.1, 0.5, 0.01)),
        ("claude-haiku-4", (1.0, 5.0, 0.1)),
        ("claude-3-5-haiku", (0.8, 4.0, 0.08)),
    ];
    table
        .iter()
        .find(|(prefix, _)| model.starts_with(prefix))
        .map(|(_, price)| *price)
}

/// A reply's tokens: prompt (all input), completion, cached, and its
/// estimated dollars when the model's price is known.
fn usage_metrics(model: &str, usage: &Value) -> Value {
    let n = |key: &str| usage[key].as_u64().unwrap_or(0);
    let input = n("input_tokens");
    let written = n("cache_creation_input_tokens");
    let read = n("cache_read_input_tokens");
    let output = n("output_tokens");
    let mut metrics = json!({
        "prompt_tokens": input + written + read,
        "completion_tokens": output,
        "cached_tokens": read,
    });
    if let Some((input_price, output_price, read_price)) = price(model) {
        let hour = usage
            .pointer("/cache_creation/ephemeral_1h_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(written);
        let minutes = written - hour;
        #[allow(clippy::cast_precision_loss)]
        let dollars = (input as f64 * input_price
            + minutes as f64 * input_price * 1.25
            + hour as f64 * input_price * 2.0
            + read as f64 * read_price
            + output as f64 * output_price)
            / 1_000_000.0;
        metrics["cost_usd"] = json!((dollars * 1_000_000.0).round() / 1_000_000.0);
    }
    metrics
}

/// The text of a content value: a string, or its text parts; images and
/// other parts become a short note.
fn content_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| match part["type"].as_str() {
                Some("text") => part["text"].as_str().map(str::to_owned),
                Some("image") => Some("[image]".into()),
                Some("document") => Some("[document]".into()),
                Some("tool_reference") => part["tool_name"]
                    .as_str()
                    .map(|name| format!("[tool {name}]")),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

struct Builder<'a> {
    screen: &'a Screen,
    counts: &'a mut Counts,
    steps: Vec<Value>,
    /// The reply being built, by API message id, and its step.
    reply: Option<(String, usize)>,
    /// Tool call id to the step that made it.
    calls: HashMap<String, usize>,
    models: BTreeMap<String, usize>,
    version: Option<String>,
}

impl Builder<'_> {
    fn text(&mut self, text: &str) -> String {
        let screened = screen_text(text, self.screen, self.counts);
        secret_screen::head_and_tail(&screened, FIRST_CUT)
    }

    fn push(&mut self, mut step: Value) -> usize {
        step["step_id"] = json!(self.steps.len() + 1);
        self.steps.push(step);
        self.reply = None;
        self.steps.len() - 1
    }

    fn said(&mut self, source: &str, text: &str, at: &Value) {
        let text = self.text(text);
        if text.trim().is_empty() {
            return;
        }
        let mut step = json!({"source": source, "message": text});
        if let Some(at) = at.as_str() {
            step["timestamp"] = json!(at);
        }
        self.push(step);
    }

    fn assistant(&mut self, line: &Value) {
        let message = &line["message"];
        let id = message["id"].as_str().unwrap_or_default().to_owned();
        let model = message["model"].as_str().unwrap_or_default().to_owned();
        let index = match &self.reply {
            Some((current, index)) if *current == id && !id.is_empty() => *index,
            _ => {
                let mut step = json!({"source": "agent", "message": ""});
                if let Some(at) = line["timestamp"].as_str() {
                    step["timestamp"] = json!(at);
                }
                if !model.is_empty() && model != "<synthetic>" {
                    step["model_name"] = json!(model);
                    *self.models.entry(model.clone()).or_default() += 1;
                }
                let index = self.push(step);
                self.reply = Some((id, index));
                index
            }
        };
        if message["usage"].is_object() && model != "<synthetic>" {
            self.steps[index]["metrics"] = usage_metrics(&model, &message["usage"]);
        }
        for part in message["content"].as_array().into_iter().flatten() {
            match part["type"].as_str() {
                Some("text") => {
                    let text = self.text(part["text"].as_str().unwrap_or_default());
                    append(&mut self.steps[index], "message", &text);
                }
                Some("thinking") => {
                    let text = self.text(part["thinking"].as_str().unwrap_or_default());
                    append(&mut self.steps[index], "reasoning_content", &text);
                }
                Some("tool_use" | "server_tool_use") => {
                    let call_id = part["id"].as_str().unwrap_or_default().to_owned();
                    let mut arguments = part["input"].clone();
                    for (rule, count) in self.screen.redact_document(&mut arguments) {
                        *self.counts.entry(rule).or_default() += count;
                    }
                    cut_strings(&mut arguments, FIRST_CUT);
                    let step = &mut self.steps[index];
                    if !step["tool_calls"].is_array() {
                        step["tool_calls"] = json!([]);
                    }
                    if let Some(calls) = step["tool_calls"].as_array_mut() {
                        calls.push(json!({
                            "tool_call_id": call_id,
                            "function_name": part["name"].as_str().unwrap_or("tool"),
                            "arguments": arguments,
                        }));
                    }
                    self.calls.insert(call_id, index);
                }
                _ => {}
            }
        }
    }

    fn result(&mut self, part: &Value) {
        let call_id = part["tool_use_id"].as_str().unwrap_or_default().to_owned();
        let Some(&index) = self.calls.get(&call_id) else {
            return;
        };
        let text = self.text(&content_text(&part["content"]));
        let step = &mut self.steps[index];
        if !step["observation"]["results"].is_array() {
            step["observation"] = json!({"results": []});
        }
        let mut result = json!({"source_call_id": call_id, "content": text});
        if part["is_error"].as_bool() == Some(true) {
            result["extra"] = json!({"is_error": true});
        }
        if let Some(results) = step["observation"]["results"].as_array_mut() {
            results.push(result);
        }
    }
}

fn append(step: &mut Value, field: &str, text: &str) {
    if text.is_empty() {
        return;
    }
    let joined = match step[field].as_str() {
        Some(before) if !before.is_empty() => format!("{before}\n\n{text}"),
        _ => text.to_owned(),
    };
    step[field] = json!(joined);
}

/// One file's lines as a trajectory.
fn trajectory(lines: &[Value], id: &str, screen: &Screen, counts: &mut Counts) -> Value {
    let mut builder = Builder {
        screen,
        counts,
        steps: Vec::new(),
        reply: None,
        calls: HashMap::new(),
        models: BTreeMap::new(),
        version: None,
    };
    for line in lines {
        if builder.version.is_none() {
            builder.version = line["version"].as_str().map(str::to_owned);
        }
        let at = &line["timestamp"];
        match line["type"].as_str() {
            Some("assistant") => builder.assistant(line),
            Some("user") if line["isMeta"].as_bool() != Some(true) => {
                let content = &line["message"]["content"];
                if line["isCompactSummary"].as_bool() == Some(true) {
                    let text = format!(
                        "The conversation so far was summarized:\n\n{}",
                        content_text(content)
                    );
                    builder.said("system", &text, at);
                    continue;
                }
                match content {
                    Value::String(text) => {
                        let source = if text.trim_start().starts_with("<task-notification>") {
                            "system"
                        } else {
                            "user"
                        };
                        builder.said(source, text, at);
                    }
                    Value::Array(parts) => {
                        let mut said = Vec::new();
                        for part in parts {
                            if part["type"] == "tool_result" {
                                builder.result(part);
                            } else {
                                said.push(part.clone());
                            }
                        }
                        let text = content_text(&Value::Array(said));
                        builder.said("user", &text, at);
                    }
                    _ => {}
                }
            }
            Some("attachment") => {
                let attachment = &line["attachment"];
                if attachment["type"] == "queued_command" {
                    let source = match attachment.pointer("/origin/kind").and_then(Value::as_str) {
                        Some("human") | None => "user",
                        Some(_) => "system",
                    };
                    let text = content_text(&attachment["prompt"]);
                    builder.said(source, &text, at);
                }
            }
            Some("system") if line["subtype"] == "compact_boundary" => {
                builder.said("system", "The conversation was compacted here.", at);
            }
            _ => {}
        }
    }
    finish(builder, id)
}

fn finish(builder: Builder<'_>, id: &str) -> Value {
    let steps = builder.steps;
    let model = builder
        .models
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(model, _)| model.clone())
        .unwrap_or_default();
    let sum = |key: &str| -> u64 {
        steps
            .iter()
            .filter_map(|step| step["metrics"][key].as_u64())
            .sum()
    };
    let cost: f64 = steps
        .iter()
        .filter_map(|step| step["metrics"]["cost_usd"].as_f64())
        .sum();
    let calls: usize = steps
        .iter()
        .filter_map(|step| step["tool_calls"].as_array().map(Vec::len))
        .sum();
    let times: Vec<&str> = steps
        .iter()
        .filter_map(|step| step["timestamp"].as_str())
        .collect();
    let started = times.iter().min().copied().unwrap_or_default();
    let ended = times.iter().max().copied().unwrap_or_default();
    let wall = match (atif::parse_iso(started), atif::parse_iso(ended)) {
        (Some(start), Some(end)) if end >= start => (end - start) / 1000,
        _ => 0,
    };
    let total_steps = steps.len();
    json!({
        "schema_version": atif::SCHEMA_VERSION,
        "session_id": id,
        "trajectory_id": id,
        "agent": {
            "name": AGENT_NAME,
            "version": builder.version.unwrap_or_default(),
            "model_name": model,
        },
        "steps": steps,
        "final_metrics": {
            "total_prompt_tokens": sum("prompt_tokens"),
            "total_completion_tokens": sum("completion_tokens"),
            "total_cached_tokens": sum("cached_tokens"),
            "total_cost_usd": (cost * 1_000_000.0).round() / 1_000_000.0,
            "total_steps": total_steps,
            "extra": {
                "wall_seconds": wall,
                "started_at": started,
                "ended_at": ended,
                "tool_calls_total": calls,
                "cost_estimated": true,
            },
        },
        "extra": {"source": "claude-code"},
    })
}

fn cut_strings(value: &mut Value, limit: usize) {
    match value {
        Value::String(text) if text.len() > limit => {
            *text = secret_screen::head_and_tail(text, limit);
        }
        Value::Array(values) => values
            .iter_mut()
            .for_each(|value| cut_strings(value, limit)),
        Value::Object(fields) => fields
            .values_mut()
            .for_each(|value| cut_strings(value, limit)),
        _ => {}
    }
}

fn size(document: &Value) -> usize {
    serde_json::to_vec(document).map_or(usize::MAX, |bytes| bytes.len())
}

/// Shorten `document` until it is at most `limit` bytes: tool output and
/// arguments first, then messages and thinking, then, as a last resort,
/// the newest steps. Says how far in `extra.shortened_to`.
pub fn fit(document: &mut Value, limit: usize) {
    if size(document) <= limit {
        return;
    }
    let original = document.clone();
    for (tools, text) in [
        (16 * 1024, FIRST_CUT),
        (4 * 1024, 32 * 1024),
        (1024, 8 * 1024),
        (256, 2 * 1024),
        (64, 512),
    ] {
        let mut trial = original.clone();
        for step in trial["steps"].as_array_mut().into_iter().flatten() {
            for result in step["observation"]["results"]
                .as_array_mut()
                .into_iter()
                .flatten()
            {
                cut_strings(&mut result["content"], tools);
            }
            for call in step["tool_calls"].as_array_mut().into_iter().flatten() {
                cut_strings(&mut call["arguments"], tools);
            }
            cut_strings(&mut step["message"], text);
            cut_strings(&mut step["reasoning_content"], text);
        }
        trial["extra"]["shortened_to"] = json!({"tool_output_chars": tools, "message_chars": text});
        if size(&trial) <= limit {
            *document = trial;
            return;
        }
        *document = trial;
    }
    // Still too large: keep the earliest steps that fit.
    let steps = document["steps"].as_array().cloned().unwrap_or_default();
    let mut kept = steps.len();
    while kept > 1 {
        kept = kept * 9 / 10;
        document["steps"] = json!(steps[..kept]);
        if size(document) + 256 <= limit {
            break;
        }
    }
    if let Some(list) = document["steps"].as_array_mut() {
        list.push(json!({
            "step_id": kept + 1,
            "source": "system",
            "message": format!("{} more steps were too long to upload.", steps.len() - kept),
        }));
    }
}

/// Split a document that carries its agents inline
/// (`subagent_trajectories`, as Coder saves them) into a tree: the
/// document without them first, then each agent, recursively.
#[must_use]
pub fn split_inline(document: Value) -> Vec<Node> {
    let mut nodes = Vec::new();
    split_into(document, None, &mut nodes);
    nodes
}

fn split_into(mut document: Value, parent: Option<usize>, nodes: &mut Vec<Node>) {
    let children = document
        .as_object_mut()
        .and_then(|fields: &mut Map<String, Value>| fields.remove("subagent_trajectories"))
        .and_then(|children| match children {
            Value::Array(children) => Some(children),
            _ => None,
        })
        .unwrap_or_default();
    let key = document["session_id"]
        .as_str()
        .map_or_else(|| format!("agent-{}", nodes.len()), str::to_owned);
    let at = nodes.len();
    nodes.push(Node {
        key,
        parent,
        document,
    });
    for child in children {
        split_into(child, Some(at), nodes);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn line(value: &Value) -> String {
        format!("{value}\n")
    }

    /// A session folder like Claude Code's: a main thread that starts
    /// three agents, one of which starts another.
    pub(crate) fn fixture(dir: &Path) -> PathBuf {
        let project = dir.join(".claude/projects/-tmp-demo");
        let session = "11111111-2222-4333-8444-555555555555";
        let main = project.join(format!("{session}.jsonl"));
        let agents = project.join(session).join("subagents");
        std::fs::create_dir_all(&agents).unwrap();
        let github = format!("ghp_{}", "Z9".repeat(18));
        let usage = json!({"input_tokens": 10, "cache_creation_input_tokens": 1000,
            "cache_read_input_tokens": 5000, "output_tokens": 200});
        let mut text = String::new();
        text += &line(
            &json!({"type": "user", "timestamp": "2026-10-09T10:00:00.000Z", "version": "2.1.293",
            "message": {"role": "user", "content": format!("Fan out three agents. My token is {github}")}}),
        );
        text += &line(
            &json!({"type": "user", "isMeta": true, "timestamp": "2026-10-09T10:00:00.100Z",
            "message": {"role": "user", "content": "<local-command-caveat>"}}),
        );
        text += &line(
            &json!({"type": "user", "isSidechain": true, "timestamp": "2026-10-09T10:00:00.200Z",
            "message": {"role": "user", "content": "an old-style agent's own line"}}),
        );
        for (n, call) in ["toolu_a", "toolu_b", "toolu_c"].iter().enumerate() {
            text += &line(
                &json!({"type": "assistant", "timestamp": format!("2026-10-09T10:00:0{}.000Z", n + 1),
                "message": {"id": format!("msg_{n}"), "model": "claude-opus-5-5", "usage": usage,
                "content": [{"type": "tool_use", "id": call, "name": "Agent",
                    "input": {"description": format!("Agent {n}"), "prompt": "Do a thing"}}]}}),
            );
        }
        text += &line(
            &json!({"type": "user", "timestamp": "2026-10-09T10:05:00.000Z",
            "message": {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "toolu_a", "content": "x".repeat(200_000)},
                {"type": "tool_result", "tool_use_id": "toolu_b", "content": [{"type": "text", "text": "done b"}]},
                {"type": "tool_result", "tool_use_id": "toolu_c", "content": "done c", "is_error": true}]}}),
        );
        text += &line(
            &json!({"type": "assistant", "timestamp": "2026-10-09T10:06:00.000Z",
            "message": {"id": "msg_9", "model": "claude-opus-5-5", "usage": usage,
            "content": [{"type": "text", "text": "All three finished."}]}}),
        );
        text += &line(
            &json!({"type": "assistant", "timestamp": "2026-10-09T10:06:00.500Z",
            "message": {"id": "msg_9", "model": "claude-opus-5-5", "usage": usage,
            "content": [{"type": "text", "text": "Second block of the same reply."}]}}),
        );
        text += &line(
            &json!({"type": "attachment", "timestamp": "2026-10-09T10:07:00.000Z",
            "attachment": {"type": "queued_command", "prompt": "and one more thing", "origin": {"kind": "human"}}}),
        );
        text += &line(&json!({"type": "ai-title", "aiTitle": "Fan out demo"}));
        std::fs::write(&main, text).unwrap();
        for (id, parent, call, start) in [
            ("a1", None, "toolu_a", "2026-10-09T10:00:01.500Z"),
            ("b2", None, "toolu_b", "2026-10-09T10:00:02.500Z"),
            ("c3", None, "toolu_c", "2026-10-09T10:00:03.500Z"),
            ("d4", Some("a1"), "toolu_x", "2026-10-09T10:01:00.000Z"),
        ] {
            let mut meta = json!({"agentType": "general-purpose", "description": format!("Agent {id}"),
                "toolUseId": call});
            if let Some(parent) = parent {
                meta["parentAgentId"] = json!(parent);
            }
            std::fs::write(
                agents.join(format!("agent-{id}.meta.json")),
                meta.to_string(),
            )
            .unwrap();
            let mut text = line(
                &json!({"type": "user", "timestamp": start, "isSidechain": true,
                "message": {"role": "user", "content": "Do a thing in /Users/octo/work"}}),
            );
            text += &line(
                &json!({"type": "assistant", "timestamp": "2026-10-09T10:04:00.000Z",
                "message": {"id": format!("m_{id}"), "model": "claude-sonnet-5-5", "usage": usage,
                "content": [{"type": "thinking", "thinking": "hmm"}, {"type": "text", "text": "Done."}]}}),
            );
            std::fs::write(agents.join(format!("agent-{id}.jsonl")), text).unwrap();
        }
        main
    }

    #[test]
    fn a_session_becomes_a_tree_of_redacted_trajectories() {
        let temp = tempfile::tempdir().unwrap();
        let main = fixture(temp.path());
        let (nodes, counts) = convert(&main, &Screen::shapes()).unwrap();
        assert_eq!(nodes.len(), 5);
        assert_eq!(nodes[0].key, "main");
        assert_eq!(
            nodes.iter().map(|n| n.parent).collect::<Vec<_>>(),
            [None, Some(0), Some(0), Some(0), Some(1)]
        );
        assert_eq!(nodes[4].key, "d4");
        let all =
            serde_json::to_string(&nodes.iter().map(|n| &n.document).collect::<Vec<_>>()).unwrap();
        assert!(!all.contains("ghp_") && !all.contains("octo"), "{all}");
        assert!(counts.values().sum::<u32>() >= 5, "{counts:?}");
        for node in &nodes {
            assert!(
                atif::validate(&node.document).is_empty(),
                "{:?}",
                atif::validate(&node.document)
            );
        }
        let root = &nodes[0].document;
        assert_eq!(root["extra"]["title"], "Fan out demo");
        let steps = root["steps"].as_array().unwrap();
        // user, 3 calls, reply (two blocks, one step), queued message.
        assert_eq!(steps.len(), 6, "{steps:#?}");
        assert_eq!(steps[5]["message"], "and one more thing");
        assert_eq!(
            steps[4]["message"],
            "All three finished.\n\nSecond block of the same reply."
        );
        let long = steps[1]["observation"]["results"][0]["content"]
            .as_str()
            .unwrap();
        assert!(long.len() <= FIRST_CUT && long.contains("cut"));
        assert_eq!(
            steps[3]["observation"]["results"][0]["extra"]["is_error"],
            true
        );
        // Four replies at 6010 prompt tokens each; usage counted once per reply.
        assert_eq!(root["final_metrics"]["total_prompt_tokens"], 4 * 6010);
        assert_eq!(root["final_metrics"]["total_completion_tokens"], 800);
        let cost = root["final_metrics"]["total_cost_usd"].as_f64().unwrap();
        // (10*4 + 1000*5 + 5000*0.2 + 200*20) / 1e6 = 0.01004 per reply.
        assert!((cost - 4.0 * 0.01004).abs() < 1e-6, "{cost}");
        assert_eq!(root["final_metrics"]["extra"]["wall_seconds"], 420);
        let child = &nodes[1].document;
        assert_eq!(child["extra"]["title"], "Agent a1");
        assert_eq!(child["extra"]["parent_tool_call_id"], "toolu_a");
        assert_eq!(child["agent"]["model_name"], "claude-sonnet-5-5");
        assert_eq!(child["steps"][1]["reasoning_content"], "hmm");
    }

    #[test]
    fn sessions_are_found_by_id_file_or_folder() {
        let temp = tempfile::tempdir().unwrap();
        let main = fixture(temp.path());
        assert!(is_session_file(&main));
        assert!(!is_session_file(Path::new("chat.atif.jsonl")));
        assert!(!is_session_file(Path::new("trace.json")));
        let id = "11111111-2222-4333-8444-555555555555";
        assert_eq!(find(id, temp.path(), temp.path()).unwrap(), main);
        assert_eq!(
            find(main.to_str().unwrap(), temp.path(), temp.path()).unwrap(),
            main
        );
        let folder = main.with_extension("");
        assert_eq!(
            find(folder.to_str().unwrap(), temp.path(), temp.path()).unwrap(),
            main
        );
        assert!(
            find("nope", temp.path(), temp.path())
                .unwrap_err()
                .contains("No Claude Code session")
        );
        assert!(find("../../etc", temp.path(), temp.path()).is_err());
    }

    #[test]
    fn a_large_trajectory_is_shortened_to_fit() {
        let big = "y".repeat(50_000);
        let steps: Vec<Value> = (0..200)
            .map(|n| {
                json!({"step_id": n + 1, "source": "agent", "message": big,
                "tool_calls": [{"tool_call_id": format!("c{n}"), "function_name": "Bash", "arguments": {"command": big}}],
                "observation": {"results": [{"source_call_id": format!("c{n}"), "content": big}]}})
            })
            .collect();
        let mut document =
            json!({"schema_version": atif::SCHEMA_VERSION, "steps": steps, "extra": {}});
        fit(&mut document, 1024 * 1024);
        assert!(size(&document) <= 1024 * 1024);
        assert_eq!(document["steps"].as_array().unwrap().len(), 200);
        assert!(document["extra"]["shortened_to"].is_object());
        let mut huge = json!({"schema_version": atif::SCHEMA_VERSION, "steps": (0..5000).map(|n| json!({"step_id": n, "source": "user", "message": "z".repeat(600)})).collect::<Vec<_>>(), "extra": {}});
        fit(&mut huge, 200 * 1024);
        assert!(size(&huge) <= 200 * 1024);
        assert!(
            huge["steps"].as_array().unwrap().last().unwrap()["message"]
                .as_str()
                .unwrap()
                .contains("too long")
        );
    }

    #[test]
    fn inline_agents_split_into_a_tree_and_times_parse() {
        let document = json!({"session_id": "main", "steps": [],
            "subagent_trajectories": [
                {"session_id": "one", "steps": [], "subagent_trajectories": [{"session_id": "deep", "steps": []}]},
                {"session_id": "two", "steps": []}]});
        let nodes = split_inline(document);
        assert_eq!(
            nodes
                .iter()
                .map(|n| (n.key.as_str(), n.parent))
                .collect::<Vec<_>>(),
            [
                ("main", None),
                ("one", Some(0)),
                ("deep", Some(1)),
                ("two", Some(0))
            ]
        );
        assert!(nodes[0].document.get("subagent_trajectories").is_none());
    }
}
