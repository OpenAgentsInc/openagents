//! A Claude Code session (`~/.claude/projects/<project>/<session>.jsonl`)
//! as an ATIF trace, for `coder trace upload --file` (#11154).
//!
//! Claude Code writes one JSON record per line: the person's messages,
//! each assistant message block by block (text, thinking, tool calls),
//! tool results as the next user record, and bookkeeping (queue
//! operations, titles, file snapshots, modes) that is not the
//! conversation. This keeps the conversation: one `user` step per
//! message the person wrote, one `agent` step per assistant message with
//! its reasoning and tool calls, and each tool's result as that step's
//! observation. Side chains (subagents), injected meta records, and
//! images are left out, and said so in `extra.left_out`.
//!
//! An upload is at most 8 MB, and a long session is far more, almost all
//! of it tool output. So long text is cut to its start and end, with a
//! note of how much was cut, at the largest length that fits: 16,000
//! characters per piece, then less until the trace fits. Redaction
//! happens after, on upload (`coder_sync::traces::prepare`).

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use serde_json::{Map, Value, json};

/// The size the converted trace must fit in, leaving room under the
/// 8 MB upload limit for redaction's markers.
pub const BUDGET: usize = 7 * 1024 * 1024 + 512 * 1024;

/// The largest session file read.
const MAX_FILE: u64 = 512 * 1024 * 1024;

/// The lengths tried for each long piece, largest first.
const CAPS: [usize; 8] = [16_000, 8_000, 4_000, 2_000, 1_000, 500, 250, 120];

/// Whether `path` looks like a Claude Code session rather than an ATIF
/// file: a `.jsonl` file that is not Coder's own `.atif.jsonl` log.
#[must_use]
pub fn is_session(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    name.ends_with(".jsonl") && !name.ends_with(".atif.jsonl")
}

/// Read a Claude Code session file and convert it.
///
/// # Errors
/// The file cannot be read, holds no conversation, or does not fit even
/// cut to the shortest length.
pub fn read(path: &Path) -> Result<Value, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| "Cannot read that file.")?;
    if !metadata.is_file() || metadata.len() > MAX_FILE {
        return Err("That file isn't a regular file, or it is over 512 MB.".into());
    }
    let mut text = String::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(MAX_FILE + 1).read_to_string(&mut text))
        .map_err(|_| "Cannot read that file.")?;
    convert(&text)
}

/// Convert a Claude Code session's lines, cutting long text until the
/// trace fits in [`BUDGET`].
///
/// # Errors
/// No conversation in it, or too big even cut short.
pub fn convert(text: &str) -> Result<Value, String> {
    let records: Vec<Value> = text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let mut last = 0;
    for cap in CAPS {
        let document = build(&records, cap);
        if document["steps"].as_array().is_none_or(Vec::is_empty) {
            return Err("That file has no Claude Code conversation in it.".into());
        }
        let size = serde_json::to_vec(&document).map_or(usize::MAX, |bytes| bytes.len());
        if size <= BUDGET {
            return Ok(document);
        }
        last = size;
    }
    Err(format!(
        "This session is {:.1} MB even with long output cut short; the most you can upload is 8 MB.",
        last as f64 / (1024.0 * 1024.0)
    ))
}

/// Cut `text` to its first and last parts when it is longer than `cap`
/// characters, saying how much was left out.
fn cut(text: &str, cap: usize) -> String {
    let count = text.chars().count();
    if count <= cap {
        return text.to_owned();
    }
    let head: String = text.chars().take(cap * 3 / 4).collect();
    let tail: String = text.chars().skip(count - cap / 4).collect();
    format!(
        "{head}\n… [{} characters cut to fit the upload] …\n{tail}",
        count - head.chars().count() - tail.chars().count()
    )
}

/// Cut every long string inside a tool call's arguments.
fn cut_value(value: &Value, cap: usize) -> Value {
    match value {
        Value::String(text) => Value::String(cut(text, cap)),
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| cut_value(item, cap)).collect())
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, item)| (key.clone(), cut_value(item, cap)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The text of a tool result's content: a string, or text blocks.
fn result_text(content: &Value, images: &mut u64) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| match block.get("type").and_then(Value::as_str) {
                Some("text") => block.get("text").and_then(Value::as_str).map(str::to_owned),
                Some("image") => {
                    *images += 1;
                    Some("[image left out]".to_owned())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Text Claude Code put in a user record that the person did not write:
/// command echoes, hook output, reminders, interruption notes.
fn injected(text: &str) -> bool {
    let text = text.trim_start();
    text.starts_with("<command-")
        || text.starts_with("<local-command-")
        || text.starts_with("<system-reminder>")
        || text.starts_with("<bash-")
        || text.starts_with("<task-notification>")
        || text.starts_with("Caveat: The messages below")
        || text.starts_with("[Request interrupted")
}

fn build(records: &[Value], cap: usize) -> Value {
    let mut steps: Vec<Value> = Vec::new();
    let mut calls: HashMap<String, usize> = HashMap::new();
    let mut session = String::new();
    let mut title: Option<String> = None;
    let mut version: Option<String> = None;
    let mut model: Option<String> = None;
    let (mut images, mut sidechain, mut meta) = (0_u64, 0_u64, 0_u64);
    let mut last_message_id: Option<String> = None;
    for record in records {
        if session.is_empty()
            && let Some(id) = record.get("sessionId").and_then(Value::as_str)
        {
            session = id.to_owned();
        }
        if let Some(found) = record.get("aiTitle").and_then(Value::as_str) {
            title = Some(found.to_owned());
        }
        if version.is_none() {
            version = record
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
        let kind = record
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if kind != "user" && kind != "assistant" {
            continue;
        }
        if record.get("isSidechain").and_then(Value::as_bool) == Some(true) {
            sidechain += 1;
            continue;
        }
        if record.get("isMeta").and_then(Value::as_bool) == Some(true) {
            meta += 1;
            continue;
        }
        let timestamp = record.get("timestamp").cloned().unwrap_or(Value::Null);
        let Some(message) = record.get("message") else {
            continue;
        };
        let content = message.get("content").cloned().unwrap_or(Value::Null);
        if kind == "user" {
            last_message_id = None;
            let mut said = Vec::new();
            match &content {
                Value::String(text) if !injected(text) => said.push(text.clone()),
                Value::Array(blocks) => {
                    for block in blocks {
                        match block.get("type").and_then(Value::as_str) {
                            Some("text") => {
                                if let Some(text) = block.get("text").and_then(Value::as_str)
                                    && !injected(text)
                                {
                                    said.push(text.to_owned());
                                }
                            }
                            Some("image") => images += 1,
                            Some("tool_result") => {
                                let id = block
                                    .get("tool_use_id")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default();
                                let text = result_text(
                                    block.get("content").unwrap_or(&Value::Null),
                                    &mut images,
                                );
                                if let Some(&at) = calls.get(id) {
                                    let step = &mut steps[at];
                                    let results = step
                                        .as_object_mut()
                                        .expect("a step is an object")
                                        .entry("observation")
                                        .or_insert_with(|| json!({"results": []}))
                                        .get_mut("results")
                                        .and_then(Value::as_array_mut)
                                        .expect("results is an array");
                                    results.push(json!({
                                        "source_call_id": id,
                                        "content": cut(&text, cap),
                                    }));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => meta += 1,
            }
            let text = said.join("\n\n");
            if !text.trim().is_empty() {
                steps.push(json!({
                    "step_id": steps.len() + 1,
                    "timestamp": timestamp,
                    "source": "user",
                    "message": cut(&text, cap * 4),
                }));
            }
            continue;
        }
        // An assistant message arrives block by block, one record each.
        let message_id = message.get("id").and_then(Value::as_str).map(str::to_owned);
        if let Some(name) = message.get("model").and_then(Value::as_str)
            && name != "<synthetic>"
        {
            model = Some(name.to_owned());
        }
        let continuing = message_id.is_some() && message_id == last_message_id;
        if !continuing {
            let mut step = Map::new();
            step.insert("step_id".into(), json!(steps.len() + 1));
            step.insert("timestamp".into(), timestamp);
            step.insert("source".into(), json!("agent"));
            step.insert("message".into(), json!(""));
            if let Some(name) = message.get("model").and_then(Value::as_str) {
                step.insert("model_name".into(), json!(name));
            }
            steps.push(Value::Object(step));
        }
        last_message_id = message_id;
        let at = steps.len() - 1;
        for block in content.as_array().into_iter().flatten() {
            let step = steps[at].as_object_mut().expect("a step is an object");
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    let text = block
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let before = step["message"].as_str().unwrap_or_default();
                    let joined = if before.is_empty() {
                        text.to_owned()
                    } else {
                        format!("{before}\n\n{text}")
                    };
                    step.insert("message".into(), json!(cut(&joined, cap * 4)));
                }
                Some("thinking") => {
                    let text = block
                        .get("thinking")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if !text.is_empty() {
                        let before = step
                            .get("reasoning_content")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let joined = if before.is_empty() {
                            text.to_owned()
                        } else {
                            format!("{before}\n\n{text}")
                        };
                        step.insert("reasoning_content".into(), json!(cut(&joined, cap)));
                    }
                }
                Some("tool_use") => {
                    let id = block.get("id").and_then(Value::as_str).unwrap_or_default();
                    let call = json!({
                        "tool_call_id": id,
                        "function_name": block.get("name").cloned().unwrap_or(json!("tool")),
                        "arguments": cut_value(block.get("input").unwrap_or(&json!({})), cap),
                    });
                    step.entry("tool_calls")
                        .or_insert_with(|| json!([]))
                        .as_array_mut()
                        .expect("tool_calls is an array")
                        .push(call);
                    calls.insert(id.to_owned(), at);
                }
                _ => {}
            }
        }
    }
    let mut agent = json!({"name": "claude-code"});
    if let Some(version) = version {
        agent["version"] = json!(version);
    }
    if let Some(model) = model {
        agent["model_name"] = json!(model);
    }
    let mut extra = json!({
        "converted_from": "claude-code-jsonl",
        "cut_to_characters": cap,
        "left_out": {"images": images, "subagent_records": sidechain, "meta_records": meta},
    });
    if let Some(title) = title {
        extra["title"] = json!(title);
    }
    json!({
        "schema_version": atif::SCHEMA_VERSION,
        "session_id": session,
        "agent": agent,
        "steps": steps,
        "extra": extra,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(value: Value) -> String {
        format!("{value}\n")
    }

    fn session(output: &str) -> String {
        [
            line(json!({"type": "ai-title", "aiTitle": "Fix the build", "sessionId": "s1"})),
            line(json!({"type": "user", "sessionId": "s1", "version": "2.1.0",
                "timestamp": "2026-10-09T00:00:00Z",
                "message": {"role": "user", "content": "Fix the build please"}})),
            line(json!({"type": "user", "isMeta": true,
                "message": {"role": "user", "content": "<local-command-stdout>x</local-command-stdout>"}})),
            line(json!({"type": "assistant", "timestamp": "2026-10-09T00:00:01Z",
                "message": {"id": "m1", "model": "claude-opus-5-5",
                    "content": [{"type": "thinking", "thinking": "Look first."}]}})),
            line(json!({"type": "assistant",
                "message": {"id": "m1", "model": "claude-opus-5-5",
                    "content": [{"type": "tool_use", "id": "t1", "name": "Bash",
                        "input": {"command": "cargo build"}}]}})),
            line(json!({"type": "user", "message": {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "t1", "content": output}]}})),
            line(json!({"type": "assistant", "isSidechain": true,
                "message": {"id": "x", "content": [{"type": "text", "text": "side"}]}})),
            line(json!({"type": "assistant",
                "message": {"id": "m2", "model": "claude-opus-5-5",
                    "content": [{"type": "text", "text": "Fixed."}]}})),
            line(json!({"type": "queue-operation", "operation": "enqueue"})),
            "not json\n".to_owned(),
        ]
        .concat()
    }

    #[test]
    fn a_session_becomes_steps_with_calls_and_their_output() {
        let document = convert(&session("error: none")).unwrap();
        assert!(atif::validate(&document).is_empty());
        crate::trajectory::from_document(&document).unwrap();
        assert_eq!(document["session_id"], "s1");
        assert_eq!(document["extra"]["title"], "Fix the build");
        assert_eq!(document["agent"]["model_name"], "claude-opus-5-5");
        let steps = document["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 3, "{steps:#?}");
        assert_eq!(steps[0]["source"], "user");
        assert_eq!(steps[0]["message"], "Fix the build please");
        assert_eq!(steps[1]["reasoning_content"], "Look first.");
        assert_eq!(steps[1]["tool_calls"][0]["function_name"], "Bash");
        assert_eq!(
            steps[1]["observation"]["results"][0]["content"],
            "error: none"
        );
        assert_eq!(steps[2]["message"], "Fixed.");
        assert_eq!(document["extra"]["left_out"]["subagent_records"], 1);
        assert_eq!(document["extra"]["left_out"]["meta_records"], 1);
    }

    #[test]
    fn long_output_is_cut_until_the_trace_fits() {
        let huge = "x".repeat(20_000);
        let text = (0..600).map(|_| session(&huge)).collect::<String>();
        assert!(text.len() > 10 * 1024 * 1024);
        let document = convert(&text).unwrap();
        assert!(serde_json::to_vec(&document).unwrap().len() <= BUDGET);
        let cap = document["extra"]["cut_to_characters"].as_u64().unwrap();
        assert!(cap < 16_000);
        let output = document["steps"][1]["observation"]["results"][0]["content"]
            .as_str()
            .unwrap();
        assert!(output.contains("characters cut to fit the upload"));
    }

    #[test]
    fn only_plain_jsonl_is_a_session() {
        assert!(is_session(Path::new("/a/3ba3.jsonl")));
        assert!(!is_session(Path::new("/a/run.atif.jsonl")));
        assert!(!is_session(Path::new("/a/trace.json")));
        assert!(convert("{\"type\": \"queue-operation\"}\n").is_err());
    }
}
