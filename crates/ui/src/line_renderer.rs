//! Line renderer for converting rlog lines to formatted UI components.
//!
//! Parses streaming output lines and renders appropriate organism components.

use lazy_static::lazy_static;
use maud::{Markup, html};
use regex::Regex;

use crate::recorder::molecules::ResultType;
use crate::recorder::organisms::{
    AgentLine, McpLine, PlanLine, QuestionLine, RecallLine, SkillLine, SubagentLine,
    ThinkingLine, TodoLine, TodoStatus, ToolLine, UserLine, phase_line,
};

lazy_static! {
    // Line type detection patterns (matching recorder format)
    // Note: Allow optional space after colon (e.g., "t!: Read" or "t!:Read")
    static ref RE_USER: Regex = Regex::new(r"^u:\s*(.*)$").unwrap();
    static ref RE_AGENT: Regex = Regex::new(r"^a:\s*(.*)$").unwrap();
    static ref RE_AGENT_BULLET: Regex = Regex::new(r"^●AGENT\s*$").unwrap();  // Agent marker line
    static ref RE_TOOL: Regex = Regex::new(r"^t:\s*(\w+)\s*(.*)$").unwrap();
    static ref RE_TOOL_START: Regex = Regex::new(r"^t!:\s*(\w+)\s*(.*)$").unwrap();
    static ref RE_OBSERVATION: Regex = Regex::new(r"^o:\s*(.*)$").unwrap();
    static ref RE_SKILL: Regex = Regex::new(r"^s:\s*(\S+)\s+(.*)$").unwrap();
    static ref RE_PLAN: Regex = Regex::new(r"^p:\s*(\w+)\s*(.*)$").unwrap();
    static ref RE_RECALL: Regex = Regex::new(r"^r:\s*(.*)$").unwrap();
    static ref RE_SUBAGENT: Regex = Regex::new(r"^x:\s*(\w+)\s*(.*)$").unwrap();
    static ref RE_MCP: Regex = Regex::new(r"^c:\s*(\w+)\.(\w+)\s*(.*)$").unwrap();
    static ref RE_QUESTION: Regex = Regex::new(r"^q:\s*(.*)$").unwrap();
    static ref RE_THINKING: Regex = Regex::new(r"^th:\s*(.*)$").unwrap();
    static ref RE_TODOS: Regex = Regex::new(r"^td:\s*(.*)$").unwrap();
    static ref RE_LIFECYCLE: Regex = Regex::new(r"^@(\w+)(.*)$").unwrap();
    static ref RE_PHASE: Regex = Regex::new(r"^@phase\s+(\w+)(.*)$").unwrap();
    static ref RE_COMMENT: Regex = Regex::new(r"^#\s*(.*)$").unwrap();

    // Metadata extraction
    static ref RE_STEP: Regex = Regex::new(r"\bstep=(\d+)").unwrap();
    static ref RE_CALL_ID: Regex = Regex::new(r"\bid=(\w+)").unwrap();
    static ref RE_LATENCY: Regex = Regex::new(r"\blatency_ms=(\d+)").unwrap();
    static ref RE_RESULT: Regex = Regex::new(r"→\s*(.+)$").unwrap();

    // Todo item parsing
    static ref RE_TODO_ITEM: Regex = Regex::new(r"\[(pending|in_progress|completed)\]\s*([^\[]+)").unwrap();
}

/// Render a raw line as a formatted UI component.
/// Returns `None` if the line doesn't match any known pattern.
pub fn render_line(raw: &str) -> Option<Markup> {
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return None;
    }

    // Extract common metadata
    let step = RE_STEP.captures(trimmed).and_then(|c| c[1].parse().ok());
    let _call_id = RE_CALL_ID.captures(trimmed).map(|c| c[1].to_string());
    let _latency = RE_LATENCY.captures(trimmed).and_then(|c| c[1].parse::<u32>().ok());
    let result_str = RE_RESULT.captures(trimmed).map(|c| c[1].to_string());

    // User message
    if let Some(caps) = RE_USER.captures(trimmed) {
        let message = caps.get(1).map_or("", |m| m.as_str());
        let mut line = UserLine::new(message);
        if let Some(s) = step {
            line = line.step(s);
        }
        return Some(line.build());
    }

    // Agent message
    if let Some(caps) = RE_AGENT.captures(trimmed) {
        let message = caps.get(1).map_or("", |m| m.as_str());
        let mut line = AgentLine::new(message);
        if let Some(s) = step {
            line = line.step(s);
        }
        return Some(line.build());
    }

    // Tool start (streaming)
    if let Some(caps) = RE_TOOL_START.captures(trimmed) {
        let tool_name = caps.get(1).map_or("", |m| m.as_str());
        let args = caps.get(2).map_or("", |m| m.as_str()).trim();
        let line = ToolLine::new(tool_name, args, ResultType::Pending);
        return Some(line.build());
    }

    // Tool call
    if let Some(caps) = RE_TOOL.captures(trimmed) {
        let tool_name = caps.get(1).map_or("", |m| m.as_str());
        let args = caps.get(2).map_or("", |m| m.as_str()).trim();
        let result = match &result_str {
            Some(r) if r.contains("[ok]") || r.contains("ok") => ResultType::Ok,
            Some(r) if r.contains("[error]") || r.contains("error") || r.contains("[err") => {
                ResultType::Error(r.clone())
            }
            Some(_) => ResultType::Ok, // Default to Ok for other results
            None => ResultType::Pending,
        };
        let mut line = ToolLine::new(tool_name, args, result);
        if let Some(s) = step {
            line = line.step(s);
        }
        return Some(line.build());
    }

    // Observation (tool output)
    if let Some(caps) = RE_OBSERVATION.captures(trimmed) {
        let content = caps.get(1).map_or("", |m| m.as_str());
        // Parse the observation format: id=XXX → [ok/error] → content
        let (status_class, display_content) = if content.contains("→ [ok]") {
            ("text-green", content.replace("→ [ok]", "✓").replace("→ ", " "))
        } else if content.contains("→ [error]") {
            ("text-red", content.replace("→ [error]", "✗").replace("→ ", " "))
        } else {
            ("text-muted-foreground", content.to_string())
        };
        return Some(html! {
            div class="px-3 py-1 pl-6 text-xs font-mono border-l border-border ml-3" {
                span class=(status_class) { (display_content) }
            }
        });
    }

    // Thinking block
    if let Some(caps) = RE_THINKING.captures(trimmed) {
        let content = caps.get(1).map_or("", |m| m.as_str());
        let mut line = ThinkingLine::new(content);
        if let Some(s) = step {
            line = line.step(s);
        }
        return Some(line.build());
    }

    // Todos
    if RE_TODOS.is_match(trimmed) {
        let mut line = TodoLine::new();
        for caps in RE_TODO_ITEM.captures_iter(trimmed) {
            let status_str = caps.get(1).map_or("", |m| m.as_str());
            let content = caps.get(2).map_or("", |m| m.as_str()).trim();
            let status = match status_str {
                "completed" => TodoStatus::Completed,
                "in_progress" => TodoStatus::InProgress,
                _ => TodoStatus::Pending,
            };
            line = line.add_item(content, status);
        }
        if let Some(s) = step {
            line = line.step(s);
        }
        return Some(line.build());
    }

    // Skill invocation
    if let Some(caps) = RE_SKILL.captures(trimmed) {
        let skill_name = caps.get(1).map_or("", |m| m.as_str());
        let rest = caps.get(2).map_or("", |m| m.as_str());
        let result = result_str
            .map(|r| {
                if r.contains("[ok]") {
                    ResultType::Ok
                } else if r.contains("[error]") || r.contains("[err") {
                    ResultType::Error(r)
                } else {
                    ResultType::Ok
                }
            })
            .unwrap_or(ResultType::Pending);
        let mut line = SkillLine::new(skill_name, result);
        if let Some(s) = step {
            line = line.step(s);
        }
        // Try to extract args from rest
        if !rest.is_empty() && !rest.starts_with('→') {
            let args = rest.split('→').next().unwrap_or("").trim();
            if !args.is_empty() {
                line = line.args(args);
            }
        }
        return Some(line.build());
    }

    // Plan action - just capture the content
    if let Some(caps) = RE_PLAN.captures(trimmed) {
        let action = caps.get(1).map_or("", |m| m.as_str());
        let rest = caps.get(2).map_or("", |m| m.as_str());
        // Combine action and rest for content
        let content = if rest.is_empty() {
            action.to_string()
        } else {
            format!("{} {}", action, rest.trim())
        };
        let mut line = PlanLine::new(&content);
        if let Some(s) = step {
            line = line.step(s);
        }
        return Some(line.build());
    }

    // MCP call - server.method combined
    if let Some(caps) = RE_MCP.captures(trimmed) {
        let server = caps.get(1).map_or("", |m| m.as_str());
        let method = caps.get(2).map_or("", |m| m.as_str());
        let args = caps.get(3).map_or("", |m| m.as_str()).trim();
        let server_method = format!("{}.{}", server, method);
        let result = result_str
            .map(|r| {
                if r.contains("[ok]") {
                    ResultType::Ok
                } else if r.contains("[error]") || r.contains("[err") {
                    ResultType::Error(r)
                } else {
                    ResultType::Ok
                }
            })
            .unwrap_or(ResultType::Pending);
        let mut line = McpLine::new(&server_method, args, result);
        if let Some(s) = step {
            line = line.step(s);
        }
        return Some(line.build());
    }

    // Subagent
    if let Some(caps) = RE_SUBAGENT.captures(trimmed) {
        let agent_type = caps.get(1).map_or("", |m| m.as_str());
        let rest = caps.get(2).map_or("", |m| m.as_str());
        // Extract task from quotes
        let task = if rest.contains('"') {
            rest.split('"').nth(1).unwrap_or("")
        } else {
            rest.split('→').next().unwrap_or("").trim()
        };
        let mut line = SubagentLine::new(agent_type, task);
        if let Some(s) = step {
            line = line.step(s);
        }
        // If there's a result, add it as summary
        if let Some(ref r) = result_str {
            line = line.summary(r);
        }
        return Some(line.build());
    }

    // Question
    if let Some(caps) = RE_QUESTION.captures(trimmed) {
        let content = caps.get(1).map_or("", |m| m.as_str());
        // Parse question and selected answer
        let (question, selected) = if content.contains("→") {
            let parts: Vec<&str> = content.splitn(2, '→').collect();
            let q = parts[0].trim().trim_matches('"');
            let sel = parts.get(1).map(|s| s.trim().to_string());
            (q, sel)
        } else {
            (content.trim_matches('"'), None)
        };
        let mut line = QuestionLine::new(question);
        if let Some(sel) = selected {
            line = line.selected(&sel);
        }
        if let Some(s) = step {
            line = line.step(s);
        }
        return Some(line.build());
    }

    // Recall/memory
    if let Some(caps) = RE_RECALL.captures(trimmed) {
        let query = caps.get(1).map_or("", |m| m.as_str());
        let (q, _results) = if query.contains("→") {
            let parts: Vec<&str> = query.splitn(2, '→').collect();
            (parts[0].trim(), parts.get(1).map(|s| *s))
        } else {
            (query, None)
        };
        let mut line = RecallLine::new(vec![q]);
        if let Some(s) = step {
            line = line.step(s);
        }
        return Some(line.build());
    }

    // Comment - render as subtle text
    if let Some(caps) = RE_COMMENT.captures(trimmed) {
        let content = caps.get(1).map_or("", |m| m.as_str());
        return Some(html! {
            div class="px-3 py-1 text-xs text-muted-foreground italic" {
                "# " (content)
            }
        });
    }

    // Phase
    if let Some(caps) = RE_PHASE.captures(trimmed) {
        let phase_name = caps.get(1).map_or("", |m| m.as_str());
        return Some(phase_line(phase_name));
    }

    // Lifecycle event - render as simple labeled block
    if let Some(caps) = RE_LIFECYCLE.captures(trimmed) {
        let event = caps.get(1).map_or("", |m| m.as_str());
        let details = caps.get(2).map_or("", |m| m.as_str()).trim();
        let color = match event {
            "start" | "resume" | "end" => "text-green",
            "checkpoint" => "text-blue",
            "pause" => "text-yellow",
            _ => "text-muted-foreground",
        };
        return Some(html! {
            div class="bg-card border border-border border-l-2 border-l-green mb-2" {
                div class="flex items-center gap-2 px-3 py-2" {
                    span class="text-xs text-muted-foreground" { "@" }
                    span class={ "text-xs font-semibold uppercase " (color) } { (event) }
                    @if !details.is_empty() {
                        span class="text-xs text-muted-foreground" { (details) }
                    }
                }
            }
        });
    }

    // No match - return None to fall back to raw display
    None
}

/// Render a line with OOB swap wrapper for HTMX.
/// Returns HTML that can be broadcast via WebSocket.
pub fn render_line_oob(raw: &str) -> String {
    if let Some(markup) = render_line(raw) {
        format!(
            r#"<div id="chat-content-formatted" hx-swap-oob="beforeend">{}</div>"#,
            markup.into_string()
        )
    } else {
        // For unrecognized lines, still show them in formatted view as plain text
        let escaped = html_escape(raw);
        format!(
            r#"<div id="chat-content-formatted" hx-swap-oob="beforeend"><div class="px-3 py-1 text-xs text-muted-foreground font-mono">{}</div></div>"#,
            escaped
        )
    }
}

/// Escape HTML special characters.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_user_line() {
        let result = render_line("u: Hello world");
        assert!(result.is_some());
        let html = result.unwrap().into_string();
        assert!(html.contains("Hello world"));
    }

    #[test]
    fn test_render_agent_line() {
        let result = render_line("a: I can help with that");
        assert!(result.is_some());
        let html = result.unwrap().into_string();
        assert!(html.contains("I can help with that"));
    }

    #[test]
    fn test_render_tool_line() {
        let result = render_line("t:Read file.txt → [ok]");
        assert!(result.is_some());
        let html = result.unwrap().into_string();
        assert!(html.contains("Read"));
    }

    #[test]
    fn test_render_thinking_line() {
        let result = render_line("th: Let me analyze this problem...");
        assert!(result.is_some());
        let html = result.unwrap().into_string();
        assert!(html.contains("Let me analyze"));
    }

    #[test]
    fn test_render_todos_line() {
        let result = render_line("td: [pending] Task 1 [completed] Task 2 [in_progress] Task 3");
        assert!(result.is_some());
        let html = result.unwrap().into_string();
        assert!(html.contains("Task 1"));
        assert!(html.contains("Task 2"));
        assert!(html.contains("Task 3"));
    }

    #[test]
    fn test_unknown_line_returns_none() {
        let result = render_line("this is just random text");
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_line_returns_none() {
        let result = render_line("");
        assert!(result.is_none());
    }

    #[test]
    fn test_render_tool_start_no_space() {
        // Actual rlog format from autopilot
        let result = render_line(r#"t!:Bash id=f83iWdeR cmd="cargo autopilot issue ready" → [running]"#);
        assert!(result.is_some(), "Should match tool start pattern");
        let html = result.unwrap().into_string();
        assert!(html.contains("Bash"), "Should contain tool name");
    }

    #[test]
    fn test_render_tool_start_with_space() {
        let result = render_line("t!: Read id=Mxnh8Wcj file.txt → [running]");
        assert!(result.is_some(), "Should match tool start pattern with space");
        let html = result.unwrap().into_string();
        assert!(html.contains("Read"), "Should contain tool name");
    }

    #[test]
    fn test_render_mcp_line() {
        let result = render_line("c:issues.issue_ready → [ok]");
        assert!(result.is_some(), "Should match MCP pattern");
        let html = result.unwrap().into_string();
        assert!(html.contains("issues.issue_ready") || html.contains("issue_ready"), "Should contain server.method");
    }

    #[test]
    fn test_render_lifecycle_start() {
        let result = render_line("@start id=808ab419 ts=2025-12-22T08:08:07Z");
        assert!(result.is_some(), "Should match lifecycle pattern");
        let html = result.unwrap().into_string();
        assert!(html.contains("start"), "Should contain lifecycle event");
    }

    #[test]
    fn test_render_observation_ok() {
        let result = render_line("o: id=f83iWdeR → [ok] → Next ready issue:");
        assert!(result.is_some(), "Should match observation pattern");
        let html = result.unwrap().into_string();
        assert!(html.contains("text-green") || html.contains("✓"), "Should show success status");
    }

    #[test]
    fn test_render_observation_error() {
        let result = render_line("o: id=kEHy1gSY → [error] File content exceeds maximum");
        assert!(result.is_some(), "Should match observation pattern");
        let html = result.unwrap().into_string();
        assert!(html.contains("text-red") || html.contains("✗"), "Should show error status");
    }

    #[test]
    fn test_render_lifecycle_init() {
        let result = render_line("@init model=claude-sonnet-4-5-20250929");
        assert!(result.is_some(), "Should match lifecycle pattern");
        let html = result.unwrap().into_string();
        assert!(html.contains("init"), "Should contain lifecycle event");
    }

    #[test]
    fn test_render_agent_line_actual() {
        // Test actual rlog format
        let result = render_line("a: I'll call `issue_ready` to get the first issue tokens_in=2 tokens_out=3");
        assert!(result.is_some(), "Should match agent pattern");
        let html = result.unwrap().into_string();
        assert!(html.contains("issue_ready"), "Should contain agent message");
    }
}
