//! Foreign-session converters: Claude Code and Codex rollouts to ATIF-v1.7.
//!
//! The corpus importer inventories three stores but until now converted only
//! native ATIF, so the two largest stores on a real machine — `~/.claude/projects`
//! and `~/.codex/sessions` — were excluded wholesale as `not_redactable`. These
//! converters produce the same top-level document shape the native exporter in
//! `coder/export.rs` writes (`schema_version` ATIF-v1.7, `session_id`, `agent`,
//! `steps[]`, `final_metrics`), so everything downstream — summarize, redact,
//! tripwire, digest, upload — runs one path regardless of where a session came
//! from.
//!
//! Bounds are explicit because the real stores are hostile to slurping: Codex
//! rollouts reach multiple gigabytes. Sources are streamed line by line, a file
//! above [`MAX_SOURCE_BYTES`] is refused before any read, individual message and
//! observation bodies are capped, and a converted document that would exceed the
//! server's 10 MiB ingest cap is truncated from the middle — head and tail kept,
//! the elision recorded in `extra.truncation` — rather than refused or shipped
//! to earn a 413.

use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::trace::TraceSourceKind;

/// The converters' output schema. Matches the native exporter and the server's
/// accepted prefixes (`ATIF/1.` or `ATIF-v1.`).
pub const CONVERTER_SCHEMA_VERSION: &str = "ATIF-v1.7";

/// Sources above this are skipped with reason `oversized_source` before any
/// read. 104 Codex rollouts on the surveyed machine exceed 50 MB; the largest
/// are multi-GB, and converting one of those buys nothing a 200 MB session
/// does not already carry.
pub const MAX_SOURCE_BYTES: u64 = 200 * 1024 * 1024;

/// The server's ingest cap for one trace body.
pub const SERVER_TRACE_CAP_BYTES: usize = 10 * 1024 * 1024;

/// Truncation targets this rather than the cap itself, leaving headroom for
/// redaction markers, canonicalization, and transport framing.
const FIT_TARGET_BYTES: usize = SERVER_TRACE_CAP_BYTES - 256 * 1024;

/// A single message or observation body is capped here (in chars) so one
/// enormous tool result cannot force the middle-truncation loop to throw away
/// every other step to fit the document under the server cap.
const MAX_CONTENT_CHARS: usize = 100_000;

/// Convert a discovered candidate by its store kind. `None` for kinds that
/// have no converter (native ATIF is read directly, not converted).
pub fn convert_candidate(kind: TraceSourceKind, path: &Path) -> Option<Result<Value, String>> {
    match kind {
        TraceSourceKind::ClaudeSession => Some(convert_claude_session(path)),
        TraceSourceKind::CodexSession => Some(convert_codex_rollout(path)),
        _ => None,
    }
}

/// Cap a body at [`MAX_CONTENT_CHARS`], marking what was dropped. The marker
/// names the count so a reader knows the elision happened here, not upstream.
fn cap_content(text: &str) -> String {
    let count = text.chars().count();
    if count <= MAX_CONTENT_CHARS {
        return text.to_string();
    }
    let kept: String = text.chars().take(MAX_CONTENT_CHARS).collect();
    format!(
        "{kept}\n[converter: truncated {} chars]",
        count - MAX_CONTENT_CHARS
    )
}

/// Pull the plain text out of a content field that may be a string or a list
/// of typed blocks (`text`, `input_text`, `output_text`, `tool_result` inner
/// content, and so on).
fn content_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => {
            let mut out = String::new();
            for block in blocks {
                let text = block
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| {
                        // A nested content field (tool_result blocks carry one).
                        block.get("content").map(content_text)
                    })
                    .unwrap_or_default();
                if text.is_empty() {
                    continue;
                }
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&text);
            }
            out
        }
        _ => String::new(),
    }
}

fn str_of(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Shared assembly state: steps, the call-id index for attaching results, and
/// the skip counters that make the conversion auditable.
struct Assembly {
    steps: Vec<Value>,
    /// tool_call_id -> index into `steps`, for attaching the output when it
    /// arrives in a later record. Removed once the result lands, so a second
    /// output for the same call reads as the orphan it is.
    open_calls: BTreeMap<String, usize>,
    /// The same mapping, never removed: events that arrive after the output
    /// (Codex `patch_apply_end`) still need to find their step.
    call_index: BTreeMap<String, usize>,
    skipped: BTreeMap<String, usize>,
}

impl Assembly {
    fn new() -> Self {
        Self {
            steps: Vec::new(),
            open_calls: BTreeMap::new(),
            call_index: BTreeMap::new(),
            skipped: BTreeMap::new(),
        }
    }

    fn register_call(&mut self, call_id: String, step_index: usize) {
        self.open_calls.insert(call_id.clone(), step_index);
        self.call_index.insert(call_id, step_index);
    }

    fn skip(&mut self, kind: &str) {
        *self.skipped.entry(kind.to_string()).or_insert(0) += 1;
    }

    fn push_step(&mut self, mut step: Value) -> usize {
        if let Some(map) = step.as_object_mut() {
            map.insert("step_id".to_string(), json!(self.steps.len() + 1));
        }
        self.steps.push(step);
        self.steps.len() - 1
    }

    /// Attach a tool result to the step that carries the matching call.
    fn attach_result(&mut self, call_id: &str, content: String, status: &str) -> bool {
        let Some(&index) = self.open_calls.get(call_id) else {
            return false;
        };
        let Some(step) = self.steps.get_mut(index).and_then(Value::as_object_mut) else {
            return false;
        };
        let observation = step
            .entry("observation")
            .or_insert_with(|| json!({"results": []}));
        if let Some(results) = observation.get_mut("results").and_then(Value::as_array_mut) {
            results.push(json!({
                "source_call_id": call_id,
                "content": cap_content(&content),
                "status": status,
            }));
        }
        self.open_calls.remove(call_id);
        true
    }
}

// ---------------------------------------------------------------------------
// Claude Code sessions: ~/.claude/projects/<project>/<uuid>.jsonl
// ---------------------------------------------------------------------------

/// Convert a Claude Code session log to ATIF.
///
/// Mapping decisions, stated once:
///
/// - `user` records become user steps; a `user` record whose content carries
///   `tool_result` blocks is not a turn at all — it is the transport for the
///   previous assistant record's tool outputs, and each block is attached as an
///   observation on the agent step that made the call.
/// - `assistant` records become agent steps. A streamed assistant message is
///   written as several records sharing one `message.id` (one per content
///   block), so records are merged into a single step while the id repeats:
///   `text` blocks join the message, `thinking` blocks join
///   `reasoning_content`, `tool_use` blocks join `tool_calls`. Usage is
///   counted once per message id, not once per record.
/// - Sidechain records (`isSidechain: true`) are subagent traffic and are
///   skipped, counted under `skipped_records.sidechain`.
/// - `mode`, `permission-mode`, `last-prompt`, `ai-title`, `attachment`,
///   `queue-operation`, `system`, and `file-history-*` records are session
///   bookkeeping, not trajectory, and are skipped with counts.
/// - `cwd` and `gitBranch` from the first record carrying them land in
///   `extra`, as does the Claude Code `version`.
/// - Token totals: prompt tokens are everything presented to the model —
///   `input_tokens + cache_creation_input_tokens + cache_read_input_tokens` —
///   completion tokens are `output_tokens`.
pub fn convert_claude_session(path: &Path) -> Result<Value, String> {
    let file =
        File::open(path).map_err(|error| format!("could not open {}: {error}", path.display()))?;
    let reader = BufReader::new(file);

    let mut assembly = Assembly::new();
    let mut session_id: Option<String> = None;
    let mut cwd: Option<String> = None;
    let mut git_branch: Option<String> = None;
    let mut cli_version: Option<String> = None;
    let mut model_name: Option<String> = None;
    let mut prompt_tokens = 0u64;
    let mut completion_tokens = 0u64;
    let mut saw_tokens = false;
    let mut counted_message_ids: std::collections::BTreeSet<String> = Default::default();
    let mut last_assistant: Option<(String, usize)> = None;
    let mut parsed_lines = 0usize;

    for line in reader.lines() {
        let line = line.map_err(|error| format!("read failed in {}: {error}", path.display()))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Value>(trimmed) else {
            assembly.skip("unparseable_line");
            continue;
        };
        parsed_lines += 1;

        let kind = str_of(&record, "type").unwrap_or_else(|| "untyped".to_string());
        session_id = session_id.or_else(|| str_of(&record, "sessionId"));
        cwd = cwd.or_else(|| str_of(&record, "cwd"));
        git_branch = git_branch.or_else(|| str_of(&record, "gitBranch"));
        cli_version = cli_version.or_else(|| str_of(&record, "version"));

        if record
            .get("isSidechain")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            assembly.skip("sidechain");
            continue;
        }

        let timestamp = str_of(&record, "timestamp").unwrap_or_default();
        match kind.as_str() {
            "user" => {
                let Some(message) = record.get("message") else {
                    assembly.skip("user_without_message");
                    continue;
                };
                let content = message.get("content").cloned().unwrap_or(Value::Null);
                let mut attached_result = false;
                if let Some(blocks) = content.as_array() {
                    for block in blocks {
                        if block.get("type").and_then(Value::as_str) != Some("tool_result") {
                            continue;
                        }
                        let call_id = str_of(block, "tool_use_id").unwrap_or_default();
                        let body = block.get("content").map(content_text).unwrap_or_default();
                        let status = if block
                            .get("is_error")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                        {
                            "failed"
                        } else {
                            "completed"
                        };
                        if assembly.attach_result(&call_id, body, status) {
                            attached_result = true;
                        } else {
                            assembly.skip("orphan_tool_result");
                        }
                    }
                }
                let text = content_text(&content);
                if !attached_result && !text.is_empty() {
                    assembly.push_step(json!({
                        "step_id": 0,
                        "timestamp": timestamp,
                        "source": "user",
                        "message": cap_content(&text),
                    }));
                }
                last_assistant = None;
            }
            "assistant" => {
                let Some(message) = record.get("message") else {
                    assembly.skip("assistant_without_message");
                    continue;
                };
                let message_id = str_of(message, "id").unwrap_or_default();
                model_name = model_name.or_else(|| str_of(message, "model"));

                if let Some(usage) = message.get("usage")
                    && (message_id.is_empty() || counted_message_ids.insert(message_id.clone()))
                {
                    let read = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
                    prompt_tokens += read("input_tokens")
                        + read("cache_creation_input_tokens")
                        + read("cache_read_input_tokens");
                    completion_tokens += read("output_tokens");
                    saw_tokens = true;
                }

                // A streamed message arrives as several records sharing one
                // message id; merge them into the step the first one opened.
                let step_index = match &last_assistant {
                    Some((last_id, index)) if !message_id.is_empty() && *last_id == message_id => {
                        *index
                    }
                    _ => {
                        let index = assembly.push_step(json!({
                            "step_id": 0,
                            "timestamp": timestamp,
                            "source": "agent",
                            "message": "",
                            "model_name": str_of(message, "model")
                                .or_else(|| model_name.clone())
                                .unwrap_or_default(),
                        }));
                        last_assistant = Some((message_id.clone(), index));
                        index
                    }
                };

                let mut new_calls: Vec<(String, Value)> = Vec::new();
                if let Some(step) = assembly
                    .steps
                    .get_mut(step_index)
                    .and_then(Value::as_object_mut)
                    && let Some(blocks) = message.get("content").and_then(Value::as_array)
                {
                    for block in blocks {
                        match block.get("type").and_then(Value::as_str) {
                            Some("text") => {
                                let text = str_of(block, "text").unwrap_or_default();
                                let joined = match step.get("message").and_then(Value::as_str) {
                                    Some(existing) if !existing.is_empty() => {
                                        format!("{existing}\n{text}")
                                    }
                                    _ => text,
                                };
                                step.insert("message".to_string(), json!(cap_content(&joined)));
                            }
                            Some("thinking") => {
                                let text = str_of(block, "thinking").unwrap_or_default();
                                let joined =
                                    match step.get("reasoning_content").and_then(Value::as_str) {
                                        Some(existing) if !existing.is_empty() => {
                                            format!("{existing}\n{text}")
                                        }
                                        _ => text,
                                    };
                                step.insert(
                                    "reasoning_content".to_string(),
                                    json!(cap_content(&joined)),
                                );
                            }
                            Some("tool_use") => {
                                let call_id = str_of(block, "id").unwrap_or_default();
                                let call = json!({
                                    "tool_call_id": call_id,
                                    "function_name": str_of(block, "name").unwrap_or_default(),
                                    "arguments": block.get("input").cloned()
                                        .unwrap_or(Value::Object(Map::new())),
                                });
                                let calls = step.entry("tool_calls").or_insert_with(|| json!([]));
                                if let Some(list) = calls.as_array_mut() {
                                    list.push(call.clone());
                                }
                                if !call_id.is_empty() {
                                    new_calls.push((call_id, call));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                for (call_id, _) in new_calls {
                    assembly.register_call(call_id, step_index);
                }
            }
            other => assembly.skip(other),
        }
    }

    if parsed_lines == 0 {
        return Err(format!(
            "{} carries no parseable session records",
            path.display()
        ));
    }

    let session_id = session_id.unwrap_or_else(|| {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_string())
    });

    let mut final_metrics = Map::new();
    final_metrics.insert("total_steps".to_string(), json!(assembly.steps.len()));
    if saw_tokens {
        final_metrics.insert("total_prompt_tokens".to_string(), json!(prompt_tokens));
        final_metrics.insert(
            "total_completion_tokens".to_string(),
            json!(completion_tokens),
        );
    }
    final_metrics.insert("extra".to_string(), json!({}));

    let mut extra = Map::new();
    extra.insert(
        "converter".to_string(),
        json!("openagents.gym.claude_session_to_atif.v1"),
    );
    if let Some(cwd) = cwd {
        extra.insert("cwd".to_string(), json!(cwd));
    }
    if let Some(branch) = git_branch {
        extra.insert("git_branch".to_string(), json!(branch));
    }
    if !assembly.skipped.is_empty() {
        extra.insert("skipped_records".to_string(), json!(assembly.skipped));
    }

    let mut agent = Map::new();
    agent.insert("name".to_string(), json!("claude-code"));
    if let Some(version) = cli_version {
        agent.insert("version".to_string(), json!(version));
    }
    if let Some(model) = model_name {
        agent.insert("model_name".to_string(), json!(model));
    }

    let mut document = json!({
        "schema_version": CONVERTER_SCHEMA_VERSION,
        "session_id": session_id,
        "trajectory_id": session_id,
        "agent": Value::Object(agent),
        "steps": assembly.steps,
        "final_metrics": Value::Object(final_metrics),
        "extra": Value::Object(extra),
    });
    fit_to_cap(&mut document);
    Ok(document)
}

// ---------------------------------------------------------------------------
// Codex rollouts: ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl
// ---------------------------------------------------------------------------

/// Convert a Codex rollout to ATIF.
///
/// Mapping decisions, stated once:
///
/// - `response_item`/`message` with role `user` becomes a user step; role
///   `assistant` an agent step; `developer` and `system` messages are harness
///   scaffolding and are skipped with counts.
/// - `function_call` and `custom_tool_call` open an agent step carrying the
///   call (`arguments` parsed from the JSON string when it is one, kept raw
///   otherwise; a custom tool's `input` becomes `{"input": ...}`); the paired
///   `*_output` attaches the observation by `call_id`.
/// - `reasoning` summaries are held and attached as `reasoning_content` to the
///   next agent step; encrypted reasoning bodies carry no readable text and
///   are counted, not invented.
/// - `event_msg`/`token_count` carries a cumulative `total_token_usage`; the
///   last one seen is the session total (`input_tokens` as prompt,
///   `output_tokens` as completion, cached and reasoning counts under
///   `final_metrics.extra`).
/// - `event_msg`/`patch_apply_end` is noted on the step that made the matching
///   call (`extra.patch_apply`), or counted at session level when no call
///   matches.
/// - `compacted` is a summarization marker: counted in `extra.compactions`.
/// - `session_meta` supplies the session id, cwd, and CLI version;
///   `turn_context` supplies the model name.
pub fn convert_codex_rollout(path: &Path) -> Result<Value, String> {
    let file =
        File::open(path).map_err(|error| format!("could not open {}: {error}", path.display()))?;
    let reader = BufReader::new(file);

    let mut assembly = Assembly::new();
    let mut session_id: Option<String> = None;
    let mut cwd: Option<String> = None;
    let mut cli_version: Option<String> = None;
    let mut model_name: Option<String> = None;
    let mut originator: Option<String> = None;
    let mut token_totals: Option<Value> = None;
    let mut pending_reasoning = String::new();
    let mut compactions = 0usize;
    let mut unmatched_patch_events = 0usize;
    let mut parsed_lines = 0usize;

    for line in reader.lines() {
        let line = line.map_err(|error| format!("read failed in {}: {error}", path.display()))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Value>(trimmed) else {
            assembly.skip("unparseable_line");
            continue;
        };
        parsed_lines += 1;

        let kind = str_of(&record, "type").unwrap_or_else(|| "untyped".to_string());
        let timestamp = str_of(&record, "timestamp").unwrap_or_default();
        let payload = record.get("payload").cloned().unwrap_or(Value::Null);

        match kind.as_str() {
            "session_meta" => {
                session_id = session_id.or_else(|| str_of(&payload, "id"));
                cwd = cwd.or_else(|| str_of(&payload, "cwd"));
                cli_version = cli_version.or_else(|| str_of(&payload, "cli_version"));
                originator = originator.or_else(|| str_of(&payload, "originator"));
            }
            "turn_context" => {
                model_name = model_name.or_else(|| str_of(&payload, "model"));
                cwd = cwd.or_else(|| str_of(&payload, "cwd"));
            }
            "compacted" => compactions += 1,
            "event_msg" => match str_of(&payload, "type").as_deref() {
                Some("token_count") => {
                    if let Some(info) = payload.get("info")
                        && let Some(totals) = info.get("total_token_usage")
                        && totals.is_object()
                    {
                        token_totals = Some(totals.clone());
                    }
                }
                Some("patch_apply_end") => {
                    let call_id = str_of(&payload, "call_id").unwrap_or_default();
                    let note = json!({
                        "success": payload.get("success").cloned().unwrap_or(Value::Null),
                        "status": payload.get("status").cloned().unwrap_or(Value::Null),
                    });
                    let attached = assembly
                        .call_index
                        .get(&call_id)
                        .copied()
                        .and_then(|index| assembly.steps.get_mut(index))
                        .and_then(Value::as_object_mut)
                        .map(|step| {
                            let extra = step
                                .entry("extra")
                                .or_insert_with(|| Value::Object(Map::new()));
                            if let Some(map) = extra.as_object_mut() {
                                map.insert("patch_apply".to_string(), note.clone());
                            }
                        })
                        .is_some();
                    if !attached {
                        unmatched_patch_events += 1;
                    }
                }
                _ => assembly.skip("event_msg"),
            },
            "response_item" => match str_of(&payload, "type").as_deref() {
                Some("message") => {
                    let role = str_of(&payload, "role").unwrap_or_default();
                    let text = payload.get("content").map(content_text).unwrap_or_default();
                    match role.as_str() {
                        "user" => {
                            if !text.is_empty() {
                                assembly.push_step(json!({
                                    "step_id": 0,
                                    "timestamp": timestamp,
                                    "source": "user",
                                    "message": cap_content(&text),
                                }));
                            }
                        }
                        "assistant" => {
                            let mut step = json!({
                                "step_id": 0,
                                "timestamp": timestamp,
                                "source": "agent",
                                "message": cap_content(&text),
                                "model_name": model_name.clone().unwrap_or_default(),
                            });
                            if !pending_reasoning.is_empty()
                                && let Some(map) = step.as_object_mut()
                            {
                                map.insert(
                                    "reasoning_content".to_string(),
                                    json!(cap_content(&std::mem::take(&mut pending_reasoning))),
                                );
                            }
                            assembly.push_step(step);
                        }
                        _ => assembly.skip(&format!("message_role_{role}")),
                    }
                }
                Some(call_kind @ ("function_call" | "custom_tool_call")) => {
                    let call_id = str_of(&payload, "call_id").unwrap_or_default();
                    let arguments = if call_kind == "function_call" {
                        let raw = str_of(&payload, "arguments").unwrap_or_default();
                        serde_json::from_str::<Value>(&raw)
                            .unwrap_or_else(|_| json!({"raw": cap_content(&raw)}))
                    } else {
                        json!({"input": cap_content(
                            &str_of(&payload, "input").unwrap_or_default()
                        )})
                    };
                    let mut step = json!({
                        "step_id": 0,
                        "timestamp": timestamp,
                        "source": "agent",
                        "message": "",
                        "model_name": model_name.clone().unwrap_or_default(),
                        "tool_calls": [{
                            "tool_call_id": call_id,
                            "function_name": str_of(&payload, "name").unwrap_or_default(),
                            "arguments": arguments,
                        }],
                    });
                    if !pending_reasoning.is_empty()
                        && let Some(map) = step.as_object_mut()
                    {
                        map.insert(
                            "reasoning_content".to_string(),
                            json!(cap_content(&std::mem::take(&mut pending_reasoning))),
                        );
                    }
                    let index = assembly.push_step(step);
                    if !call_id.is_empty() {
                        assembly.register_call(call_id, index);
                    }
                }
                Some("function_call_output" | "custom_tool_call_output") => {
                    let call_id = str_of(&payload, "call_id").unwrap_or_default();
                    let body = payload
                        .get("output")
                        .map(|output| match output {
                            Value::String(text) => text.clone(),
                            other => content_text(other),
                        })
                        .unwrap_or_default();
                    if !assembly.attach_result(&call_id, body, "completed") {
                        assembly.skip("orphan_tool_output");
                    }
                }
                Some("reasoning") => {
                    let summary = payload.get("summary").map(content_text).unwrap_or_default();
                    if summary.is_empty() {
                        assembly.skip("encrypted_reasoning");
                    } else {
                        if !pending_reasoning.is_empty() {
                            pending_reasoning.push('\n');
                        }
                        pending_reasoning.push_str(&summary);
                    }
                }
                other => assembly.skip(&format!("response_item_{}", other.unwrap_or("untyped"))),
            },
            other => assembly.skip(other),
        }
    }

    if parsed_lines == 0 {
        return Err(format!(
            "{} carries no parseable rollout records",
            path.display()
        ));
    }

    let session_id = session_id.unwrap_or_else(|| {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_string())
    });

    let mut final_metrics = Map::new();
    final_metrics.insert("total_steps".to_string(), json!(assembly.steps.len()));
    let mut metrics_extra = Map::new();
    if let Some(totals) = &token_totals {
        let read = |key: &str| totals.get(key).and_then(Value::as_u64);
        if let Some(prompt) = read("input_tokens") {
            final_metrics.insert("total_prompt_tokens".to_string(), json!(prompt));
        }
        if let Some(completion) = read("output_tokens") {
            final_metrics.insert("total_completion_tokens".to_string(), json!(completion));
        }
        if let Some(cached) = read("cached_input_tokens") {
            metrics_extra.insert("cached_input_tokens".to_string(), json!(cached));
        }
        if let Some(reasoning) = read("reasoning_output_tokens") {
            metrics_extra.insert("reasoning_output_tokens".to_string(), json!(reasoning));
        }
    }
    final_metrics.insert("extra".to_string(), Value::Object(metrics_extra));

    let mut extra = Map::new();
    extra.insert(
        "converter".to_string(),
        json!("openagents.gym.codex_rollout_to_atif.v1"),
    );
    if let Some(cwd) = cwd {
        extra.insert("cwd".to_string(), json!(cwd));
    }
    if let Some(originator) = originator {
        extra.insert("originator".to_string(), json!(originator));
    }
    if compactions > 0 {
        extra.insert("compactions".to_string(), json!(compactions));
    }
    if unmatched_patch_events > 0 {
        extra.insert(
            "unmatched_patch_apply_events".to_string(),
            json!(unmatched_patch_events),
        );
    }
    if !assembly.skipped.is_empty() {
        extra.insert("skipped_records".to_string(), json!(assembly.skipped));
    }

    let mut agent = Map::new();
    agent.insert("name".to_string(), json!("codex"));
    if let Some(version) = cli_version {
        agent.insert("version".to_string(), json!(version));
    }
    if let Some(model) = model_name {
        agent.insert("model_name".to_string(), json!(model));
    }

    let mut document = json!({
        "schema_version": CONVERTER_SCHEMA_VERSION,
        "session_id": session_id,
        "trajectory_id": session_id,
        "agent": Value::Object(agent),
        "steps": assembly.steps,
        "final_metrics": Value::Object(final_metrics),
        "extra": Value::Object(extra),
    });
    fit_to_cap(&mut document);
    Ok(document)
}

// ---------------------------------------------------------------------------
// The 10 MiB fit
// ---------------------------------------------------------------------------

/// Truncate a converted document to fit the server's ingest cap.
///
/// Steps are removed from the middle — the opening directive and the closing
/// outcome are the halves a distiller needs most — a quarter of the remainder
/// at a time until the serialized document fits [`FIT_TARGET_BYTES`]. The
/// elision is recorded in `extra.truncation` with the removed count and the
/// step ids at the seam, so a reader sees a marked cut rather than an
/// unexplained gap.
fn fit_to_cap(document: &mut Value) {
    let mut removed_total = 0usize;
    let mut seam: Option<(u64, u64)> = None;

    loop {
        let serialized = match serde_json::to_string(document) {
            Ok(text) => text.len(),
            Err(_) => return,
        };
        if serialized <= FIT_TARGET_BYTES {
            break;
        }
        let Some(steps) = document.get_mut("steps").and_then(Value::as_array_mut) else {
            return;
        };
        if steps.len() <= 2 {
            // Two steps that still blow the cap have already been content-capped;
            // there is nothing honest left to remove.
            break;
        }
        let remove = (steps.len() / 4).clamp(1, steps.len() - 2);
        let start = (steps.len() - remove) / 2;
        let step_id = |step: &Value| step.get("step_id").and_then(Value::as_u64).unwrap_or(0);
        let seam_before = step_id(&steps[start]);
        let seam_after = step_id(&steps[start + remove - 1]);
        seam = Some(match seam {
            Some((low, high)) => (low.min(seam_before), high.max(seam_after)),
            None => (seam_before, seam_after),
        });
        steps.drain(start..start + remove);
        removed_total += remove;
    }

    if removed_total == 0 {
        return;
    }
    let kept = document
        .get("steps")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if let Some(extra) = document.get_mut("extra").and_then(Value::as_object_mut) {
        extra.insert(
            "truncation".to_string(),
            json!({
                "reason": "server_cap_10mib",
                "removed_steps": removed_total,
                "kept_steps": kept,
                "removed_step_id_range": seam.map(|(low, high)| json!([low, high]))
                    .unwrap_or(Value::Null),
            }),
        );
    }
}
