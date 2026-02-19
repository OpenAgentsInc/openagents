//! CLI renderer for ACP streaming events.
//!
//! Formats the same ACP flow used by the desktop UI into concise CLI lines.

use std::collections::HashMap;
use std::io::Write;

use agent_client_protocol_schema as acp;
use serde_json::Value;

use crate::autopilot_loop::{DSPY_META_KEY, DspyStage};

const AI_PREFIX: &str = "[AI] ";
const THOUGHT_PREFIX: &str = "[THINK] ";
const DSPY_PREFIX: &str = "[DSPY] ";
const TOOL_PREFIX: &str = "[TOOL] ";

const MAX_LINE_LEN: usize = 120;
const MAX_FIELD_LEN: usize = 80;
const MAX_TOOL_OUTPUT: usize = 240;
const MAX_LIST_ITEMS: usize = 3;

#[derive(Debug)]
struct ToolMeta {
    name: String,
}

#[derive(Debug, Clone, Copy)]
enum TextKind {
    Message,
    Thought,
}

/// ACP stream renderer for CLI output.
pub struct CliAcpRenderer<W: Write> {
    writer: W,
    message_buf: String,
    thought_buf: String,
    tools: HashMap<String, ToolMeta>,
    ai_indent: String,
    thought_indent: String,
    dspy_indent: String,
    tool_indent: String,
}

impl<W: Write> CliAcpRenderer<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            message_buf: String::new(),
            thought_buf: String::new(),
            tools: HashMap::new(),
            ai_indent: " ".repeat(AI_PREFIX.len()),
            thought_indent: " ".repeat(THOUGHT_PREFIX.len()),
            dspy_indent: " ".repeat(DSPY_PREFIX.len()),
            tool_indent: " ".repeat(TOOL_PREFIX.len()),
        }
    }

    pub fn handle_notification(&mut self, notification: acp::SessionNotification) {
        match notification.update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                self.handle_chunk(chunk, TextKind::Message);
            }
            acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                self.handle_chunk(chunk, TextKind::Thought);
            }
            acp::SessionUpdate::Plan(plan) => {
                self.flush_buffers();
                self.render_plan(plan);
            }
            acp::SessionUpdate::ToolCall(call) => {
                self.flush_buffers();
                self.render_tool_start(call);
            }
            acp::SessionUpdate::ToolCallUpdate(update) => {
                self.flush_buffers();
                self.render_tool_update(update);
            }
            _ => {}
        }
    }

    pub fn finish(&mut self) {
        self.flush_buffers();
    }

    pub fn into_inner(self) -> W {
        self.writer
    }

    fn handle_chunk(&mut self, chunk: acp::ContentChunk, kind: TextKind) {
        if let Some(stage) = Self::dspy_stage(&chunk.content) {
            self.flush_buffers();
            self.render_stage(stage);
            return;
        }
        if let Some(text) = Self::content_text(&chunk.content) {
            self.append_text(kind, &text);
        }
    }

    fn append_text(&mut self, kind: TextKind, text: &str) {
        let buffer = match kind {
            TextKind::Message => &mut self.message_buf,
            TextKind::Thought => &mut self.thought_buf,
        };
        buffer.push_str(text);
        self.flush_buffer(kind, false);
    }

    fn flush_buffers(&mut self) {
        self.flush_buffer(TextKind::Message, true);
        self.flush_buffer(TextKind::Thought, true);
    }

    fn flush_buffer(&mut self, kind: TextKind, force: bool) {
        let prefix = match kind {
            TextKind::Message => AI_PREFIX,
            TextKind::Thought => THOUGHT_PREFIX,
        };
        let indent = match kind {
            TextKind::Message => self.ai_indent.clone(),
            TextKind::Thought => self.thought_indent.clone(),
        };

        loop {
            let line = {
                let buffer = match kind {
                    TextKind::Message => &mut self.message_buf,
                    TextKind::Thought => &mut self.thought_buf,
                };

                if let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].to_string();
                    buffer.drain(..=pos);
                    Some(line)
                } else if buffer.len() >= MAX_LINE_LEN {
                    let (head, tail) = split_at_boundary(buffer, MAX_LINE_LEN);
                    *buffer = tail;
                    Some(head)
                } else if force && !buffer.trim().is_empty() {
                    let line = buffer.trim().to_string();
                    buffer.clear();
                    Some(line)
                } else {
                    None
                }
            };

            if let Some(line) = line {
                self.write_wrapped_line(prefix, &indent, line);
                continue;
            }
            break;
        }
    }

    fn write_wrapped_line(&mut self, prefix: &str, indent: &str, line: String) {
        if line.trim().is_empty() {
            return;
        }
        let mut first = true;
        let mut remaining = line;
        loop {
            let (head, tail) = if remaining.len() > MAX_LINE_LEN {
                split_at_boundary(&remaining, MAX_LINE_LEN)
            } else {
                (remaining, String::new())
            };
            let label = if first { prefix } else { indent };
            let _ = writeln!(self.writer, "{}{}", label, head.trim());
            first = false;
            if tail.is_empty() {
                break;
            }
            remaining = tail;
        }
        let _ = self.writer.flush();
    }

    fn render_stage(&mut self, stage: DspyStage) {
        let indent = self.dspy_indent.clone();
        match stage {
            DspyStage::EnvironmentAssessment {
                system_info,
                workspace,
                active_directive,
                open_issues,
                compute_backends,
                priority_action,
                urgency,
                reasoning,
            } => {
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!(
                        "environment: {} | workspace: {}",
                        truncate(&system_info, MAX_FIELD_LEN),
                        truncate(&workspace, MAX_FIELD_LEN)
                    ),
                );
                if let Some(directive) = active_directive {
                    self.write_line(
                        DSPY_PREFIX,
                        &indent,
                        format!("directive: {}", truncate(&directive, MAX_FIELD_LEN)),
                    );
                }
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!(
                        "issues: {} | compute: {}",
                        open_issues,
                        summarize_list(&compute_backends, MAX_LIST_ITEMS)
                    ),
                );
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!(
                        "priority: {} (urgency: {})",
                        truncate(&priority_action, MAX_FIELD_LEN),
                        truncate(&urgency, MAX_FIELD_LEN)
                    ),
                );
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!("reasoning: {}", truncate(&reasoning, MAX_FIELD_LEN)),
                );
            }
            DspyStage::Planning {
                analysis,
                files_to_modify,
                implementation_steps,
                test_strategy,
                complexity,
                confidence,
            } => {
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!(
                        "planning: complexity={} confidence={:.0}%",
                        complexity,
                        confidence * 100.0
                    ),
                );
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!("analysis: {}", truncate(&analysis, MAX_FIELD_LEN)),
                );
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!(
                        "files: {}",
                        summarize_list(&files_to_modify, MAX_LIST_ITEMS)
                    ),
                );
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!(
                        "steps: {}",
                        summarize_list(&implementation_steps, MAX_LIST_ITEMS)
                    ),
                );
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!("tests: {}", truncate(&test_strategy, MAX_FIELD_LEN)),
                );
            }
            DspyStage::TodoList { tasks } => {
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!("todo list: {} tasks", tasks.len()),
                );
                for task in tasks {
                    let status = todo_status_marker(task.status);
                    self.write_line(
                        DSPY_PREFIX,
                        &indent,
                        format!(
                            "{} {}. {}",
                            status,
                            task.index,
                            truncate(&task.description, MAX_FIELD_LEN)
                        ),
                    );
                }
            }
            DspyStage::ExecutingTask {
                task_index,
                total_tasks,
                task_description,
            } => {
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!(
                        "execute: {}/{} {}",
                        task_index,
                        total_tasks,
                        truncate(&task_description, MAX_FIELD_LEN)
                    ),
                );
            }
            DspyStage::TaskComplete {
                task_index,
                success,
            } => {
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!(
                        "task {}: {}",
                        task_index,
                        if success { "ok" } else { "failed" }
                    ),
                );
            }
            DspyStage::Complete {
                total_tasks,
                successful,
                failed,
            } => {
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!(
                        "complete: {} total, {} ok, {} failed",
                        total_tasks, successful, failed
                    ),
                );
            }
            DspyStage::IssueSuggestions {
                suggestions,
                filtered_count,
                confidence,
                await_selection,
            } => {
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!(
                        "issue suggestions: {} options, confidence={:.0}%",
                        suggestions.len(),
                        confidence * 100.0
                    ),
                );
                for (i, s) in suggestions.iter().enumerate() {
                    self.write_line(
                        DSPY_PREFIX,
                        &indent,
                        format!(
                            "  {}. [#{}] {} ({}) - {}",
                            i + 1,
                            s.number,
                            truncate(&s.title, 40),
                            s.priority,
                            s.complexity
                        ),
                    );
                }
                if filtered_count > 0 {
                    self.write_line(
                        DSPY_PREFIX,
                        &indent,
                        format!("  [{} issues filtered]", filtered_count),
                    );
                }
                if await_selection {
                    self.write_line(DSPY_PREFIX, &indent, "  awaiting selection...".to_string());
                }
            }
            DspyStage::IssueSelected {
                number,
                title,
                selection_method,
            } => {
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!(
                        "selected: #{} {} ({})",
                        number,
                        truncate(&title, MAX_FIELD_LEN),
                        selection_method
                    ),
                );
            }
            DspyStage::UnblockSuggestion {
                issue_number,
                title,
                blocked_reason,
                unblock_rationale,
                unblock_strategy,
                estimated_effort,
                other_blocked_count,
            } => {
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!(
                        "unblock suggestion: #{} {} (effort: {})",
                        issue_number,
                        truncate(&title, 40),
                        estimated_effort
                    ),
                );
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!("  blocked: {}", truncate(&blocked_reason, MAX_FIELD_LEN)),
                );
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!(
                        "  rationale: {}",
                        truncate(&unblock_rationale, MAX_FIELD_LEN)
                    ),
                );
                self.write_line(
                    DSPY_PREFIX,
                    &indent,
                    format!("  strategy: {}", truncate(&unblock_strategy, MAX_FIELD_LEN)),
                );
                if other_blocked_count > 0 {
                    self.write_line(
                        DSPY_PREFIX,
                        &indent,
                        format!("  [{} other issues also blocked]", other_blocked_count),
                    );
                }
            }
        }
    }

    fn render_plan(&mut self, plan: acp::Plan) {
        if plan.entries.is_empty() {
            return;
        }
        let indent = self.dspy_indent.clone();
        self.write_line(
            DSPY_PREFIX,
            &indent,
            format!("todo list update: {} items", plan.entries.len()),
        );
        for (idx, entry) in plan.entries.iter().enumerate() {
            let status = match entry.status {
                acp::PlanEntryStatus::Pending => "[ ]",
                acp::PlanEntryStatus::InProgress => "[~]",
                acp::PlanEntryStatus::Completed => "[x]",
                _ => "[?]",
            };
            self.write_line(
                DSPY_PREFIX,
                &indent,
                format!(
                    "{} {}. {}",
                    status,
                    idx + 1,
                    truncate(&entry.content, MAX_FIELD_LEN)
                ),
            );
        }
    }

    fn render_tool_start(&mut self, call: acp::ToolCall) {
        let tool_id = call.tool_call_id.to_string();
        let input = call
            .raw_input
            .as_ref()
            .map(format_tool_input)
            .unwrap_or_default();
        self.tools.insert(
            tool_id.clone(),
            ToolMeta {
                name: call.title.clone(),
            },
        );
        let details = if input.is_empty() {
            call.title.clone()
        } else {
            format!("{}: {}", call.title, input)
        };
        let indent = self.tool_indent.clone();
        self.write_line(TOOL_PREFIX, &indent, format!("start {}", details));
    }

    fn render_tool_update(&mut self, update: acp::ToolCallUpdate) {
        if let Some(meta) = update.meta.as_ref() {
            if let Some(progress) = meta.get(autopilot_core::ACP_TOOL_PROGRESS_META_KEY) {
                if let Some(elapsed_secs) = progress.as_f64() {
                    let tool_name = meta
                        .get(autopilot_core::ACP_TOOL_NAME_META_KEY)
                        .and_then(|name| name.as_str())
                        .unwrap_or("tool");
                    let indent = self.tool_indent.clone();
                    self.write_line(
                        TOOL_PREFIX,
                        &indent,
                        format!("progress {} {:.1}s", tool_name, elapsed_secs),
                    );
                    return;
                }
            }
        }

        let status = update
            .fields
            .status
            .unwrap_or(acp::ToolCallStatus::InProgress);
        let is_done = matches!(
            status,
            acp::ToolCallStatus::Completed | acp::ToolCallStatus::Failed
        );
        if !is_done {
            return;
        }

        let tool_id = update.tool_call_id.to_string();
        let tool_name = update
            .meta
            .as_ref()
            .and_then(|meta| meta.get(autopilot_core::ACP_TOOL_NAME_META_KEY))
            .and_then(|value| value.as_str())
            .map(|name| name.to_string())
            .or_else(|| self.tools.get(&tool_id).map(|meta| meta.name.clone()))
            .unwrap_or_else(|| tool_id.clone());
        let status_label = if matches!(status, acp::ToolCallStatus::Failed) {
            "failed"
        } else {
            "ok"
        };
        let output = update
            .fields
            .raw_output
            .as_ref()
            .and_then(format_tool_output)
            .unwrap_or_else(|| status_label.to_string());
        let indent = self.tool_indent.clone();
        self.write_line(
            TOOL_PREFIX,
            &indent,
            format!("done {} {}", tool_name, status_label),
        );
        if !output.trim().is_empty() {
            self.write_line(
                TOOL_PREFIX,
                &indent,
                format!("output {}", truncate(&output, MAX_TOOL_OUTPUT)),
            );
        }
        self.tools.remove(&tool_id);
    }

    fn write_line(&mut self, prefix: &str, indent: &str, line: String) {
        if line.trim().is_empty() {
            return;
        }
        let mut first = true;
        let mut remaining = line;
        loop {
            let (head, tail) = if remaining.len() > MAX_LINE_LEN {
                split_at_boundary(&remaining, MAX_LINE_LEN)
            } else {
                (remaining, String::new())
            };
            let label = if first { prefix } else { indent };
            let _ = writeln!(self.writer, "{}{}", label, head.trim());
            first = false;
            if tail.is_empty() {
                break;
            }
            remaining = tail;
        }
        let _ = self.writer.flush();
    }

    fn dspy_stage(content: &acp::ContentBlock) -> Option<DspyStage> {
        let meta = match content {
            acp::ContentBlock::Text(text) => text.meta.as_ref(),
            _ => None,
        }?;
        let stage_value = meta.get(DSPY_META_KEY)?;
        serde_json::from_value(stage_value.clone()).ok()
    }

    fn content_text(content: &acp::ContentBlock) -> Option<String> {
        match content {
            acp::ContentBlock::Text(text) => Some(text.text.clone()),
            _ => None,
        }
    }
}

fn split_at_boundary(text: &str, max_len: usize) -> (String, String) {
    if text.len() <= max_len {
        return (text.to_string(), String::new());
    }
    let mut split = None;
    for (idx, ch) in text.char_indices() {
        if idx >= max_len {
            break;
        }
        if ch.is_whitespace() {
            split = Some(idx);
        }
    }
    let idx = split.unwrap_or(max_len);
    let (head, tail) = text.split_at(idx);
    (head.trim_end().to_string(), tail.trim_start().to_string())
}

fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }
    let keep_len = max_len.saturating_sub(3).max(1);
    let mut end = 0;
    for (idx, ch) in text.char_indices() {
        if idx >= keep_len {
            break;
        }
        end = idx + ch.len_utf8();
    }
    if end == 0 {
        end = text
            .char_indices()
            .next()
            .map(|(idx, ch)| idx + ch.len_utf8())
            .unwrap_or(0);
    }
    let mut truncated = text[..end].to_string();
    truncated.push_str("...");
    truncated
}

fn summarize_list(items: &[String], max_items: usize) -> String {
    if items.is_empty() {
        return "none".to_string();
    }
    let mut parts: Vec<String> = items.iter().take(max_items).cloned().collect();
    let remaining = items.len().saturating_sub(max_items);
    if remaining > 0 {
        parts.push(format!("+{} more", remaining));
    }
    parts.join(", ")
}

fn todo_status_marker(status: crate::autopilot_loop::TodoStatus) -> &'static str {
    match status {
        crate::autopilot_loop::TodoStatus::Pending => "[ ]",
        crate::autopilot_loop::TodoStatus::InProgress => "[~]",
        crate::autopilot_loop::TodoStatus::Complete => "[x]",
        crate::autopilot_loop::TodoStatus::Failed => "[!]",
    }
}

fn format_tool_input(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut parts = Vec::new();
            for key in keys {
                if parts.len() >= MAX_LIST_ITEMS {
                    break;
                }
                let val = map.get(key).unwrap_or(&Value::Null);
                let formatted = match val {
                    Value::String(s) => truncate(s, MAX_FIELD_LEN),
                    _ => truncate(&val.to_string(), MAX_FIELD_LEN),
                };
                parts.push(format!("{}={}", key, formatted));
            }
            let remaining = map.len().saturating_sub(parts.len());
            if remaining > 0 {
                parts.push(format!("+{} more", remaining));
            }
            parts.join(" ")
        }
        Value::String(s) => truncate(s, MAX_FIELD_LEN),
        _ => truncate(&value.to_string(), MAX_FIELD_LEN),
    }
}

fn format_tool_output(value: &Value) -> Option<String> {
    if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
        return Some(truncate(content, MAX_TOOL_OUTPUT));
    }
    if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
        return Some(truncate(&format!("error: {}", error), MAX_TOOL_OUTPUT));
    }
    let stdout = value.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
    let stderr = value.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
    if !stdout.is_empty() || !stderr.is_empty() {
        let combined = if !stderr.is_empty() {
            format!("{}\nstderr: {}", stdout, stderr)
        } else {
            stdout.to_string()
        };
        return Some(truncate(&combined, MAX_TOOL_OUTPUT));
    }
    Some(truncate(&value.to_string(), MAX_TOOL_OUTPUT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn notif(update: acp::SessionUpdate) -> acp::SessionNotification {
        acp::SessionNotification::new(acp::SessionId::new("test"), update)
    }

    #[test]
    fn renders_tool_start_and_done() {
        let mut renderer = CliAcpRenderer::new(Vec::new());
        let tool_call = acp::ToolCall::new(acp::ToolCallId::new("tool-1"), "bash")
            .raw_input(json!({"command": "ls -la"}));
        renderer.handle_notification(notif(acp::SessionUpdate::ToolCall(tool_call)));

        let mut fields = acp::ToolCallUpdateFields::new();
        fields = fields
            .status(acp::ToolCallStatus::Completed)
            .raw_output(json!({"content": "ok"}));
        let update = acp::ToolCallUpdate::new(acp::ToolCallId::new("tool-1"), fields);
        renderer.handle_notification(notif(acp::SessionUpdate::ToolCallUpdate(update)));
        renderer.finish();

        let output = String::from_utf8(renderer.into_inner()).expect("utf8");
        assert!(output.contains("[TOOL] start bash"));
        assert!(output.contains("command=ls -la"));
        assert!(output.contains("[TOOL] done bash ok"));
        assert!(output.contains("[TOOL] output ok"));
    }

    #[test]
    fn renders_plan_entries() {
        let mut renderer = CliAcpRenderer::new(Vec::new());
        let plan = acp::Plan::new(vec![
            acp::PlanEntry::new(
                "First task",
                acp::PlanEntryPriority::Medium,
                acp::PlanEntryStatus::Pending,
            ),
            acp::PlanEntry::new(
                "Second task",
                acp::PlanEntryPriority::Medium,
                acp::PlanEntryStatus::Completed,
            ),
        ]);
        renderer.handle_notification(notif(acp::SessionUpdate::Plan(plan)));
        renderer.finish();

        let output = String::from_utf8(renderer.into_inner()).expect("utf8");
        assert!(output.contains("todo list update: 2 items"));
        assert!(output.contains("[ ] 1."));
        assert!(output.contains("[x] 2."));
    }

    #[test]
    fn renders_dspy_stage_summary() {
        let mut renderer = CliAcpRenderer::new(Vec::new());
        let stage = DspyStage::Planning {
            analysis: "Check files".to_string(),
            files_to_modify: vec!["src/lib.rs".to_string()],
            implementation_steps: vec!["Update logic".to_string()],
            test_strategy: "cargo test".to_string(),
            complexity: "Medium".to_string(),
            confidence: 0.82,
        };
        let mut meta = acp::Meta::new();
        meta.insert(
            DSPY_META_KEY.to_string(),
            serde_json::to_value(stage).unwrap(),
        );
        let chunk = acp::ContentChunk::new(acp::ContentBlock::Text(
            acp::TextContent::new("Planning").meta(meta),
        ));
        renderer.handle_notification(notif(acp::SessionUpdate::AgentThoughtChunk(chunk)));
        renderer.finish();

        let output = String::from_utf8(renderer.into_inner()).expect("utf8");
        assert!(output.contains("planning: complexity=Medium"));
        assert!(output.contains("files: src/lib.rs"));
    }
}
