//! Claude Code JSONL to Recorder format converter
//!
//! Converts Claude Code session files (`.jsonl`) to Recorder format (`.rlog`).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConvertError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error at line {line}: {message}")]
    JsonParse { line: usize, message: String },

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid event type: {0}")]
    InvalidEventType(String),
}

/// Options for conversion
#[derive(Debug, Clone)]
pub struct ConvertOptions {
    /// Include thinking blocks in output
    pub include_thinking: bool,
    /// Include signature on thinking blocks
    pub include_signature: bool,
    /// Include file-history-snapshot events as comments
    pub include_snapshots: bool,
    /// Include queue-operation events as comments
    pub include_queue_ops: bool,
    /// Include raw Claude Code JSONL events as comments
    pub include_raw_events: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            include_thinking: true,
            include_signature: true,
            include_snapshots: true,
            include_queue_ops: true,
            // Changed from true to false to prevent PII/secret leakage
            // Raw events may contain sensitive data that should not be shared
            include_raw_events: false,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserEvent {
    uuid: Option<String>,
    parent_uuid: Option<String>,
    session_id: Option<String>,
    version: Option<String>,
    git_branch: Option<String>,
    slug: Option<String>,
    cwd: Option<String>,
    timestamp: Option<String>,
    message: Option<UserMessage>,
    #[serde(default)]
    todos: Vec<TodoItem>,
}

#[derive(Debug, Deserialize)]
struct UserMessage {
    role: Option<String>,
    content: MessageContent,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssistantEvent {
    uuid: Option<String>,
    parent_uuid: Option<String>,
    session_id: Option<String>,
    timestamp: Option<String>,
    message: Option<AssistantMessage>,
    tool_use_result: Option<ToolUseResult>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssistantMessage {
    model: Option<String>,
    id: Option<String>,
    content: Option<Vec<ContentBlock>>,
    usage: Option<TokenUsage>,
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum ContentBlock {
    Thinking {
        thinking: String,
        signature: Option<String>,
    },
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: ToolResultContent,
        #[serde(default)]
        is_error: bool,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum ToolResultContent {
    Text(String),
    Blocks(Vec<ToolResultBlock>),
}

#[derive(Debug, Deserialize, Serialize)]
struct ToolResultBlock {
    #[serde(rename = "type")]
    block_type: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolUseResult {
    stdout: Option<String>,
    stderr: Option<String>,
    #[serde(default)]
    interrupted: bool,
    #[serde(default)]
    is_image: bool,
    status: Option<String>,
    prompt: Option<String>,
    agent_id: Option<String>,
    content: Option<Vec<ToolResultBlock>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileSnapshotEvent {
    message_id: Option<String>,
    snapshot: Option<SnapshotData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotData {
    tracked_file_backups: Option<HashMap<String, Value>>,
    timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QueueOperationEvent {
    operation: Option<String>,
    content: Option<String>,
    timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TodoItem {
    content: Option<String>,
    status: Option<String>,
}

/// Session metadata extracted during conversion
#[derive(Debug, Default)]
struct SessionMeta {
    session_id: Option<String>,
    client_version: Option<String>,
    git_branch: Option<String>,
    slug: Option<String>,
    cwd: Option<String>,
    model: Option<String>,
    tokens_in: u64,
    tokens_out: u64,
    tokens_cached: u64,
    tokens_cache_create: u64,
    first_timestamp: Option<String>,
}

/// Convert a Claude Code JSONL file to Recorder format
///
/// **Memory usage**: Loads the entire file into memory. For very large JSONL files (>100MB),
/// consider processing in batches. The conversion accumulates output lines before writing the
/// final result.
pub fn convert_file(
    path: &Path,
    repo_sha: &str,
    options: &ConvertOptions,
) -> Result<String, ConvertError> {
    let content = std::fs::read_to_string(path)?;
    convert_content(&content, repo_sha, options)
}

/// Convert Claude Code JSONL content to Recorder format
pub fn convert_content(
    content: &str,
    repo_sha: &str,
    options: &ConvertOptions,
) -> Result<String, ConvertError> {
    let mut output = Vec::new();
    let mut meta = SessionMeta::default();
    let mut lines_output = Vec::new();

    // Parse each line
    for (line_num, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let event: Value = serde_json::from_str(line).map_err(|e| ConvertError::JsonParse {
            line: line_num + 1,
            message: e.to_string(),
        })?;

        // Extract event type
        let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");

        if options.include_raw_events {
            lines_output.push(format!("# claude: {}", line));
        }

        match event_type {
            "user" => {
                let user_event: UserEvent =
                    serde_json::from_value(event).map_err(|e| ConvertError::JsonParse {
                        line: line_num + 1,
                        message: e.to_string(),
                    })?;

                // Extract session metadata from first user event
                if meta.session_id.is_none() {
                    meta.session_id = user_event.session_id.clone();
                    meta.client_version = user_event.version.clone();
                    meta.git_branch = user_event.git_branch.clone();
                    meta.slug = user_event.slug.clone();
                    meta.cwd = user_event.cwd.clone();
                    meta.first_timestamp = user_event.timestamp.clone();
                }

                // Convert user message
                if let Some(msg) = &user_event.message {
                    let content_str = match &msg.content {
                        MessageContent::Text(t) => t.clone(),
                        MessageContent::Blocks(blocks) => {
                            // Extract text from all block types
                            blocks
                                .iter()
                                .filter_map(|b| match b {
                                    ContentBlock::Text { text } => Some(text.clone()),
                                    ContentBlock::ToolResult { content, .. } => {
                                        Some(extract_tool_result_text(content))
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                    };

                    if !content_str.is_empty() {
                        let mut line = format_user_line(&content_str);

                        // Add metadata
                        if let Some(ts) = &user_event.timestamp {
                            line.push_str(&format!(" ts={}", ts));
                        }
                        if let Some(id) = &user_event.uuid {
                            line.push_str(&format!(" id={}", truncate_uuid(id)));
                            append_meta_json(&mut line, "id_full", id);
                        }
                        if let Some(parent) = &user_event.parent_uuid {
                            line.push_str(&format!(" parent={}", truncate_uuid(parent)));
                            append_meta_json(&mut line, "parent_full", parent);
                        }
                        if let Some(session) = &user_event.session_id {
                            append_meta_json(&mut line, "session", session);
                        }
                        if let Some(role) = &msg.role {
                            append_meta_json(&mut line, "role", role);
                        }

                        lines_output.push(line);
                    }
                }

                // Convert todos if present
                if !user_event.todos.is_empty() {
                    let todos_line = format_todos_line(&user_event.todos);
                    lines_output.push(todos_line);
                }
            }

            "assistant" => {
                let assistant_event: AssistantEvent =
                    serde_json::from_value(event).map_err(|e| ConvertError::JsonParse {
                        line: line_num + 1,
                        message: e.to_string(),
                    })?;

                if let Some(msg) = &assistant_event.message {
                    // Extract model
                    if meta.model.is_none() {
                        meta.model = msg.model.clone();
                    }

                    // Track tokens
                    if let Some(usage) = &msg.usage {
                        if let Some(t) = usage.input_tokens {
                            meta.tokens_in += t;
                        }
                        if let Some(t) = usage.output_tokens {
                            meta.tokens_out += t;
                        }
                        if let Some(t) = usage.cache_read_input_tokens {
                            meta.tokens_cached += t;
                        }
                        if let Some(t) = usage.cache_creation_input_tokens {
                            meta.tokens_cache_create += t;
                        }
                    }

                    // Process content blocks
                    if let Some(blocks) = &msg.content {
                        for block in blocks {
                            match block {
                                ContentBlock::Thinking {
                                    thinking,
                                    signature,
                                } => {
                                    if options.include_thinking {
                                        let mut line = format_thinking_line(thinking);

                                        if options.include_signature
                                            && let Some(sig) = signature
                                        {
                                            let short_sig = if sig.len() > 20 {
                                                format!("{}...", &sig[..20])
                                            } else {
                                                sig.clone()
                                            };
                                            line.push_str(&format!(" sig={}", short_sig));
                                        }

                                        append_assistant_meta(&mut line, &assistant_event, msg);

                                        lines_output.push(line);
                                    }
                                }

                                ContentBlock::Text { text } => {
                                    let mut line = format_agent_line(text);
                                    append_assistant_meta(&mut line, &assistant_event, msg);

                                    lines_output.push(line);
                                }

                                ContentBlock::ToolUse { id, name, input } => {
                                    let mut line = format_tool_start_line(id, name, input);
                                    append_meta_json(&mut line, "id_full", id);
                                    append_meta_json(&mut line, "input", input);
                                    append_assistant_meta(&mut line, &assistant_event, msg);
                                    lines_output.push(line);
                                }

                                ContentBlock::ToolResult {
                                    tool_use_id,
                                    content,
                                    is_error,
                                } => {
                                    let result_text = extract_tool_result_text(content);
                                    let status = if *is_error { "[error]" } else { "[ok]" };
                                    let mut line = format!(
                                        "o: id={} → {} {}",
                                        truncate_uuid(tool_use_id),
                                        status,
                                        truncate_content(&result_text, 100)
                                    );
                                    append_meta_json(&mut line, "id_full", tool_use_id);
                                    append_meta_json(&mut line, "result_full", content);
                                    append_assistant_meta(&mut line, &assistant_event, msg);
                                    lines_output.push(line);
                                }

                                ContentBlock::Unknown => {}
                            }
                        }
                    }
                }

                // Handle tool_use_result for tool completions
                if let Some(result) = &assistant_event.tool_use_result {
                    // This is typically attached to tool result messages
                    if result.interrupted {
                        // Mark the last tool as interrupted if present
                        if let Some(last) = lines_output.last_mut()
                            && (last.starts_with("t!:") || last.starts_with("t:"))
                        {
                            last.push_str(" interrupted");
                        }
                    }

                    let mut meta_line = String::from("# tool-use-result");
                    let mut has_meta = false;

                    if let Some(status) = &result.status {
                        append_meta_json(&mut meta_line, "status", status);
                        has_meta = true;
                    }
                    if let Some(prompt) = &result.prompt {
                        append_meta_json(&mut meta_line, "prompt", prompt);
                        has_meta = true;
                    }
                    if let Some(agent_id) = &result.agent_id {
                        append_meta_json(&mut meta_line, "agent_id", agent_id);
                        has_meta = true;
                    }
                    if let Some(stdout) = &result.stdout {
                        append_meta_json(&mut meta_line, "stdout", stdout);
                        has_meta = true;
                    }
                    if let Some(stderr) = &result.stderr {
                        append_meta_json(&mut meta_line, "stderr", stderr);
                        has_meta = true;
                    }
                    if result.is_image {
                        meta_line.push_str(" is_image=true");
                        has_meta = true;
                    }
                    if let Some(content) = &result.content {
                        append_meta_json(&mut meta_line, "content", content);
                        has_meta = true;
                    }

                    if has_meta {
                        lines_output.push(meta_line);
                    }
                }
            }

            "file-history-snapshot" => {
                if options.include_snapshots {
                    let snapshot: FileSnapshotEvent =
                        serde_json::from_value(event).map_err(|e| ConvertError::JsonParse {
                            line: line_num + 1,
                            message: e.to_string(),
                        })?;

                    let file_count = snapshot
                        .snapshot
                        .as_ref()
                        .and_then(|s| s.tracked_file_backups.as_ref())
                        .map(|m| m.len())
                        .unwrap_or(0);

                    let msg_id = snapshot.message_id.as_deref().unwrap_or("unknown");
                    let mut line = format!(
                        "# file-snapshot: {} files={}",
                        truncate_uuid(msg_id),
                        file_count
                    );
                    if let Some(ts) = snapshot
                        .snapshot
                        .as_ref()
                        .and_then(|s| s.timestamp.as_ref())
                    {
                        line.push_str(&format!(" ts={}", ts));
                    }
                    lines_output.push(line);
                }
            }

            "queue-operation" => {
                if options.include_queue_ops {
                    let queue_op: QueueOperationEvent =
                        serde_json::from_value(event).map_err(|e| ConvertError::JsonParse {
                            line: line_num + 1,
                            message: e.to_string(),
                        })?;

                    let op = queue_op.operation.as_deref().unwrap_or("unknown");
                    let content = queue_op
                        .content
                        .as_ref()
                        .map(|c| truncate_content(c, 50))
                        .unwrap_or_default();

                    let mut line = format!("# queue: {} \"{}\"", op, content);
                    if let Some(ts) = &queue_op.timestamp {
                        line.push_str(&format!(" ts={}", ts));
                    }
                    lines_output.push(line);
                }
            }

            _ => {
                // Skip unknown event types
            }
        }
    }

    // Build header
    let session_id = meta.session_id.clone().unwrap_or_else(|| "unknown".into());

    output.push("---".to_string());
    output.push("format: rlog/1".to_string());
    output.push(format!("id: {}", session_id));
    output.push(format!("repo_sha: {}", repo_sha));

    if let Some(v) = &meta.client_version {
        output.push(format!("client_version: \"{}\"", v));
    }
    if let Some(s) = &meta.slug {
        output.push(format!("slug: {}", s));
    }
    if let Some(b) = &meta.git_branch {
        output.push(format!("branch: {}", b));
    }
    if let Some(m) = &meta.model {
        output.push(format!("model: {}", m));
    }
    if let Some(c) = &meta.cwd {
        output.push(format!("cwd: {}", c));
    }
    if meta.tokens_in > 0 {
        output.push(format!("tokens_total_in: {}", meta.tokens_in));
    }
    if meta.tokens_out > 0 {
        output.push(format!("tokens_total_out: {}", meta.tokens_out));
    }
    if meta.tokens_cached > 0 {
        output.push(format!("tokens_cached: {}", meta.tokens_cached));
    }
    if meta.tokens_cache_create > 0 {
        output.push(format!("tokens_cache_create: {}", meta.tokens_cache_create));
    }

    output.push("---".to_string());
    output.push(String::new());

    // Add @start lifecycle event
    if let Some(ts) = &meta.first_timestamp {
        output.push(format!(
            "@start id={} ts={}",
            truncate_uuid(&session_id),
            ts
        ));
    }

    // Add body lines
    output.extend(lines_output);

    // Add @end with token summary
    output.push(format!(
        "@end tokens_in={} tokens_out={}",
        meta.tokens_in, meta.tokens_out
    ));

    Ok(output.join("\n"))
}

fn format_user_line(content: &str) -> String {
    let first_line = content.lines().next().unwrap_or("");
    let truncated = truncate_content(first_line, 200);
    format!("u: {}", truncated)
}

fn format_agent_line(content: &str) -> String {
    let first_line = content.lines().next().unwrap_or("");
    let truncated = truncate_content(first_line, 200);
    format!("a: {}", truncated)
}

fn format_thinking_line(content: &str) -> String {
    let first_line = content.lines().next().unwrap_or("");
    let truncated = truncate_content(first_line, 150);
    format!("th: {}", truncated)
}

fn format_tool_start_line(id: &str, name: &str, input: &Value) -> String {
    // Extract key args based on tool type
    let args_summary = match name {
        "Read" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|p| format!("file_path={}", p))
            .unwrap_or_default(),
        "Bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .map(|c| format!("cmd=\"{}\"", truncate_content(c, 50)))
            .unwrap_or_default(),
        "Edit" | "Write" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|p| format!("file_path={}", p))
            .unwrap_or_default(),
        "Glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|p| format!("pattern=\"{}\"", p))
            .unwrap_or_default(),
        "Grep" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|p| format!("pattern=\"{}\"", truncate_content(p, 30)))
            .unwrap_or_default(),
        "Task" => input
            .get("description")
            .and_then(|v| v.as_str())
            .map(|d| format!("desc=\"{}\"", truncate_content(d, 40)))
            .unwrap_or_default(),
        "TodoWrite" => "todos".to_string(),
        _ => String::new(),
    };

    format!(
        "t!:{} id={} {} → [running]",
        name,
        truncate_uuid(id),
        args_summary
    )
}

fn append_meta_json<T: serde::Serialize>(line: &mut String, key: &str, value: &T) {
    let encoded = serde_json::to_string(value).unwrap_or_else(|_| "\"<invalid>\"".to_string());
    line.push(' ');
    line.push_str(key);
    line.push('=');
    line.push_str(&encoded);
}

fn append_assistant_meta(line: &mut String, event: &AssistantEvent, msg: &AssistantMessage) {
    if let Some(ts) = &event.timestamp {
        line.push_str(&format!(" ts={}", ts));
    }
    if let Some(parent) = &event.parent_uuid {
        line.push_str(&format!(" parent={}", truncate_uuid(parent)));
        append_meta_json(line, "parent_full", parent);
    }
    if let Some(uuid) = &event.uuid {
        append_meta_json(line, "uuid", uuid);
    }
    if let Some(session) = &event.session_id {
        append_meta_json(line, "session", session);
    }
    if let Some(message_id) = &msg.id {
        append_meta_json(line, "message_id", message_id);
    }
    if let Some(stop) = &msg.stop_reason {
        append_meta_json(line, "stop", stop);
    }
    if let Some(model) = &msg.model {
        append_meta_json(line, "model", model);
    }
    if let Some(usage) = &msg.usage {
        if let Some(t) = usage.input_tokens {
            line.push_str(&format!(" tokens_in={}", t));
        }
        if let Some(t) = usage.output_tokens {
            line.push_str(&format!(" tokens_out={}", t));
        }
        if let Some(t) = usage.cache_read_input_tokens {
            line.push_str(&format!(" tokens_cached={}", t));
        }
        if let Some(t) = usage.cache_creation_input_tokens {
            line.push_str(&format!(" tokens_cache_create={}", t));
        }
    }
}

fn format_todos_line(todos: &[TodoItem]) -> String {
    let items: Vec<String> = todos
        .iter()
        .filter_map(|t| {
            let status = t.status.as_deref().unwrap_or("pending");
            let content = t.content.as_deref().unwrap_or("");
            if content.is_empty() {
                None
            } else {
                Some(format!("[{}] {}", status, truncate_content(content, 30)))
            }
        })
        .collect();

    format!("td: {}", items.join(" "))
}

fn extract_tool_result_text(content: &ToolResultContent) -> String {
    match content {
        ToolResultContent::Text(t) => t.clone(),
        ToolResultContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match (&b.block_type, &b.text) {
                (Some(block_type), Some(text)) => Some(format!("[{}] {}", block_type, text)),
                (Some(block_type), None) => Some(format!("[{}]", block_type)),
                (None, Some(text)) => Some(text.clone()),
                (None, None) => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn truncate_uuid(uuid: &str) -> &str {
    if uuid.len() > 8 { &uuid[..8] } else { uuid }
}

fn truncate_content(s: &str, max_len: usize) -> String {
    // Remove newlines and limit length
    let cleaned: String = s
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();

    if cleaned.chars().count() <= max_len {
        cleaned
    } else {
        let truncated: String = cleaned.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_minimal_session() {
        let jsonl = r#"{"type":"user","uuid":"abc123","sessionId":"sess_1","version":"2.0.71","gitBranch":"main","slug":"test-session","cwd":"/test","timestamp":"2025-12-19T20:00:00Z","message":{"role":"user","content":"Hello world"},"todos":[]}
{"type":"assistant","uuid":"def456","parentUuid":"abc123","timestamp":"2025-12-19T20:00:01Z","message":{"model":"claude-opus","content":[{"type":"text","text":"Hi there!"}],"usage":{"input_tokens":10,"output_tokens":5}}}"#;

        let result = convert_content(jsonl, "abc123", &ConvertOptions::default()).unwrap();

        assert!(result.contains("format: rlog/1"));
        assert!(result.contains("id: sess_1"));
        assert!(result.contains("client_version: \"2.0.71\""));
        assert!(result.contains("slug: test-session"));
        assert!(result.contains("u: Hello world"));
        assert!(result.contains("a: Hi there!"));
        assert!(
            result.contains("tokens_total_in: 10"),
            "Missing tokens_total_in"
        );
        assert!(
            result.contains("tokens_total_out: 5"),
            "Missing tokens_total_out"
        );
    }

    #[test]
    fn test_convert_with_thinking() {
        let jsonl = r#"{"type":"user","uuid":"abc123","sessionId":"sess_1","message":{"role":"user","content":"Think about this"},"todos":[]}
{"type":"assistant","uuid":"def456","parentUuid":"abc123","timestamp":"2025-12-19T20:00:01Z","message":{"content":[{"type":"thinking","thinking":"Let me analyze this carefully...","signature":"SIG123"}]}}"#;

        let result = convert_content(jsonl, "abc123", &ConvertOptions::default()).unwrap();

        assert!(result.contains("th: Let me analyze this carefully..."));
        assert!(result.contains("sig=SIG123"));
    }

    #[test]
    fn test_convert_with_tool_calls() {
        let jsonl = r#"{"type":"user","uuid":"abc123","sessionId":"sess_1","message":{"role":"user","content":"Read a file"},"todos":[]}
{"type":"assistant","uuid":"def456","message":{"content":[{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"src/lib.rs"}}]}}"#;

        let result = convert_content(jsonl, "abc123", &ConvertOptions::default()).unwrap();

        assert!(result.contains("t!:Read id=toolu_1 file_path=src/lib.rs"));
    }

    #[test]
    fn test_truncate_uuid() {
        assert_eq!(truncate_uuid("abc123def456"), "abc123de");
        assert_eq!(truncate_uuid("short"), "short");
    }

    #[test]
    fn test_truncate_content() {
        let long = "a".repeat(100);
        let truncated = truncate_content(&long, 20);
        assert_eq!(truncated.len(), 20);
        assert!(truncated.ends_with("..."));
    }
}
