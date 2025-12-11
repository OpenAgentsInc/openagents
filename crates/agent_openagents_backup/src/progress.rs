//! Progress File Infrastructure
//!
//! Enables cross-session coordination by reading/writing progress.md.
//! Following Anthropic's "Effective Harnesses for Long-Running Agents" pattern:
//! - Write orientation summary after understanding repo state
//! - Track current session work (subtasks, files, tests)
//! - Leave instructions for next session

use crate::types::{
    get_progress_path, ApiUsage, ClaudeCodeSessionMetadata, InitScriptResult, NextSession,
    Orientation, SessionProgress, WorkProgress,
};
use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Compress output for progress file
fn compress_output(output: &str, max_length: usize) -> String {
    let condensed: String = output.split_whitespace().collect::<Vec<_>>().join(" ");
    if condensed.len() <= max_length {
        condensed
    } else {
        format!("{}...", &condensed[..max_length])
    }
}

/// Write progress file for next session to read
pub fn write_progress(openagents_dir: &str, progress: &SessionProgress) {
    let progress_path = get_progress_path(openagents_dir);
    let markdown = format_progress_markdown(progress);

    // Ensure directory exists
    if let Some(parent) = Path::new(&progress_path).parent() {
        let _ = fs::create_dir_all(parent);
    }

    let _ = fs::write(&progress_path, markdown);
}

/// Format SessionProgress as markdown
pub fn format_progress_markdown(progress: &SessionProgress) -> String {
    let mut lines = vec![
        "# Session Progress".to_string(),
        String::new(),
        "## Session Info".to_string(),
        format!("- **Session ID**: {}", progress.session_id),
        format!("- **Started**: {}", progress.started_at),
        format!("- **Task**: {} - {}", progress.task_id, progress.task_title),
        String::new(),
        "## Orientation".to_string(),
        format!("- **Repo State**: {}", progress.orientation.repo_state),
        format!(
            "- **Tests Passing at Start**: {}",
            if progress.orientation.tests_passing_at_start { "Yes" } else { "No" }
        ),
    ];

    if let Some(ref init) = progress.orientation.init_script {
        let status = if !init.ran {
            "Not Found"
        } else if init.success {
            "Success"
        } else {
            "Failed"
        };
        lines.push(format!("- **Init Script**: {}", status));

        if let Some(ref output) = init.output {
            lines.push(format!("- **Init Output**: {}", compress_output(output, 500)));
        }
    }

    if let Some(ref summary) = progress.orientation.previous_session_summary {
        lines.push(format!("- **Previous Session**: {}", summary));
    }

    lines.extend(vec![
        String::new(),
        "## Work Done".to_string(),
        format!(
            "- **Subtasks Completed**: {}",
            if progress.work.subtasks_completed.is_empty() {
                "None".to_string()
            } else {
                progress.work.subtasks_completed.join(", ")
            }
        ),
        format!(
            "- **Subtasks In Progress**: {}",
            if progress.work.subtasks_in_progress.is_empty() {
                "None".to_string()
            } else {
                progress.work.subtasks_in_progress.join(", ")
            }
        ),
        format!(
            "- **Files Modified**: {}",
            if progress.work.files_modified.is_empty() {
                "None".to_string()
            } else {
                progress.work.files_modified.join(", ")
            }
        ),
        format!("- Tests Run: {}", if progress.work.tests_run { "Yes" } else { "No" }),
        format!(
            "- Tests Passing After Work: {}",
            if progress.work.tests_passing_after_work { "Yes" } else { "No" }
        ),
        format!("- E2E Run: {}", if progress.work.e2e_run { "Yes" } else { "No" }),
        format!(
            "- E2E Passing After Work: {}",
            if progress.work.e2e_passing_after_work { "Yes" } else { "No" }
        ),
    ]);

    // Add Claude Code session metadata if present
    if let Some(ref cc) = progress.work.claude_code_session {
        lines.push(String::new());
        lines.push("### Claude Code Session".to_string());

        if let Some(ref session_id) = cc.session_id {
            lines.push(format!("- **Session ID**: {}", session_id));
        }

        if let Some(ref forked_from) = cc.forked_from_session_id {
            lines.push(format!("- **Forked From**: {}", forked_from));
        }

        if let Some(ref tools) = cc.tools_used {
            let tools_str: Vec<String> = tools
                .iter()
                .map(|(tool, count)| format!("{}({})", tool, count))
                .collect();
            lines.push(format!("- **Tools Used**: {}", tools_str.join(", ")));
        }

        if let Some(ref summary) = cc.summary {
            lines.push(format!("- **Summary**: {}", summary));
        }

        if let Some(ref usage) = cc.usage {
            let mut parts = Vec::new();
            if let Some(input) = usage.input_tokens {
                parts.push(format!("{} in", input));
            }
            if let Some(output) = usage.output_tokens {
                parts.push(format!("{} out", output));
            }
            if let Some(cache_read) = usage.cache_read_input_tokens {
                parts.push(format!("{} cache hits", cache_read));
            }
            if let Some(cache_write) = usage.cache_creation_input_tokens {
                parts.push(format!("{} cache writes", cache_write));
            }
            if !parts.is_empty() {
                lines.push(format!("- **Token Usage**: {}", parts.join(", ")));
            }
        }

        if let Some(cost) = cc.total_cost_usd {
            lines.push(format!("- **Cost**: ${:.4} USD", cost));
        }
    }

    lines.push(String::new());
    lines.push("## Next Session Should".to_string());

    if progress.next_session.suggested_next_steps.is_empty() {
        lines.push("- Continue with next task".to_string());
    } else {
        for step in &progress.next_session.suggested_next_steps {
            lines.push(format!("- {}", step));
        }
    }

    if let Some(ref blockers) = progress.next_session.blockers {
        if !blockers.is_empty() {
            lines.push(String::new());
            lines.push("### Blockers".to_string());
            for blocker in blockers {
                lines.push(format!("- {}", blocker));
            }
        }
    }

    if let Some(ref notes) = progress.next_session.notes {
        lines.push(String::new());
        lines.push("### Notes".to_string());
        lines.push(notes.clone());
    }

    lines.push(String::new());
    lines.push("---".to_string());
    lines.push(format!(
        "Completed: {}",
        progress.completed_at.as_deref().unwrap_or("In Progress")
    ));

    lines.join("\n")
}

/// Read the previous session's progress file if it exists
pub fn read_progress(openagents_dir: &str) -> Option<SessionProgress> {
    let progress_path = get_progress_path(openagents_dir);
    let content = fs::read_to_string(&progress_path).ok()?;
    parse_progress_markdown(&content)
}

/// Check if a progress file exists
pub fn progress_exists(openagents_dir: &str) -> bool {
    let progress_path = get_progress_path(openagents_dir);
    Path::new(&progress_path).exists()
}

/// Get a summary of the previous session for context bridging
pub fn get_previous_session_summary(openagents_dir: &str) -> Option<String> {
    let progress = read_progress(openagents_dir)?;
    let mut parts = Vec::new();

    if !progress.task_id.is_empty() && !progress.task_title.is_empty() {
        parts.push(format!("Previous task: {} - {}", progress.task_id, progress.task_title));
    }

    if !progress.work.subtasks_completed.is_empty() {
        parts.push(format!("Completed: {}", progress.work.subtasks_completed.join(", ")));
    }

    if !progress.work.subtasks_in_progress.is_empty() {
        parts.push(format!("In progress: {}", progress.work.subtasks_in_progress.join(", ")));
    }

    if let Some(ref blockers) = progress.next_session.blockers {
        if !blockers.is_empty() {
            parts.push(format!("Blockers: {}", blockers.join(", ")));
        }
    }

    if !progress.next_session.suggested_next_steps.is_empty() {
        parts.push(format!(
            "Next steps: {}",
            progress.next_session.suggested_next_steps.join("; ")
        ));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

/// Create an empty SessionProgress object
pub fn create_empty_progress(session_id: &str, task_id: &str, task_title: &str) -> SessionProgress {
    SessionProgress {
        session_id: session_id.to_string(),
        started_at: Utc::now().to_rfc3339(),
        task_id: task_id.to_string(),
        task_title: task_title.to_string(),
        orientation: Orientation::default(),
        work: WorkProgress::default(),
        next_session: NextSession::default(),
        completed_at: None,
    }
}

/// Parse a markdown progress file back into SessionProgress
pub fn parse_progress_markdown(markdown: &str) -> Option<SessionProgress> {
    let mut progress = SessionProgress {
        session_id: String::new(),
        started_at: String::new(),
        task_id: String::new(),
        task_title: String::new(),
        orientation: Orientation::default(),
        work: WorkProgress::default(),
        next_session: NextSession::default(),
        completed_at: None,
    };

    let lines: Vec<&str> = markdown.lines().collect();
    let mut current_section = "";

    for line in lines {
        let trimmed = line.trim();

        // Detect section headers
        if trimmed.starts_with("### ") {
            current_section = trimmed.strip_prefix("### ").unwrap_or("").to_lowercase().leak();
            continue;
        }
        if trimmed.starts_with("## ") {
            current_section = trimmed.strip_prefix("## ").unwrap_or("").to_lowercase().leak();
            continue;
        }

        // Parse key-value pairs
        if trimmed.starts_with("- **") || (trimmed.starts_with("- ") && trimmed.contains(':')) {
            if let Some((key, value)) = parse_key_value(trimmed) {
                apply_key_value(&mut progress, current_section, &key, &value);
            }
            continue;
        }

        // Parse list items in "next session should" section
        if current_section == "next session should" && trimmed.starts_with("- ") {
            let step = trimmed.strip_prefix("- ").unwrap_or("");
            progress.next_session.suggested_next_steps.push(step.to_string());
            continue;
        }

        // Parse blockers
        if current_section == "blockers" && trimmed.starts_with("- ") {
            let blocker = trimmed.strip_prefix("- ").unwrap_or("");
            if progress.next_session.blockers.is_none() {
                progress.next_session.blockers = Some(Vec::new());
            }
            if let Some(ref mut blockers) = progress.next_session.blockers {
                blockers.push(blocker.to_string());
            }
            continue;
        }

        // Parse completed timestamp
        if trimmed.starts_with("Completed:") {
            let value = trimmed.strip_prefix("Completed:").unwrap_or("").trim();
            if value != "In Progress" {
                progress.completed_at = Some(value.to_string());
            }
        }
    }

    Some(progress)
}

fn parse_key_value(line: &str) -> Option<(String, String)> {
    // Match "- **Key**: Value" or "- Key: Value"
    let content = line.strip_prefix("- ").unwrap_or(line);
    let content = content.strip_prefix("**").unwrap_or(content);

    if let Some(colon_pos) = content.find(':') {
        let key = content[..colon_pos].trim_end_matches("**").to_lowercase();
        let value = content[colon_pos + 1..].trim().to_string();
        Some((key, value))
    } else {
        None
    }
}

fn apply_key_value(progress: &mut SessionProgress, section: &str, key: &str, value: &str) {
    match section {
        "session info" => match key {
            "session id" => progress.session_id = value.to_string(),
            "started" => progress.started_at = value.to_string(),
            "task" => {
                if let Some(dash_pos) = value.find(" - ") {
                    progress.task_id = value[..dash_pos].to_string();
                    progress.task_title = value[dash_pos + 3..].to_string();
                }
            }
            _ => {}
        },
        "orientation" => match key {
            "repo state" => progress.orientation.repo_state = value.to_string(),
            "tests passing at start" => {
                progress.orientation.tests_passing_at_start = value.to_lowercase() == "yes";
            }
            "previous session" => {
                progress.orientation.previous_session_summary = Some(value.to_string());
            }
            "init script" => {
                let init = progress.orientation.init_script.get_or_insert(InitScriptResult::default());
                let lower = value.to_lowercase();
                if lower.contains("success") {
                    init.ran = true;
                    init.success = true;
                } else if lower.contains("failed") {
                    init.ran = true;
                    init.success = false;
                } else {
                    init.ran = false;
                    init.success = true;
                }
            }
            "init output" => {
                let init = progress.orientation.init_script.get_or_insert(InitScriptResult::default());
                init.output = Some(value.to_string());
            }
            _ => {}
        },
        "work done" => match key {
            "subtasks completed" => {
                progress.work.subtasks_completed = parse_list(value);
            }
            "subtasks in progress" => {
                progress.work.subtasks_in_progress = parse_list(value);
            }
            "files modified" => {
                progress.work.files_modified = parse_list(value);
            }
            "tests run" => {
                progress.work.tests_run = value.to_lowercase() == "yes";
            }
            "tests passing after work" => {
                progress.work.tests_passing_after_work = value.to_lowercase() == "yes";
            }
            "e2e run" => {
                progress.work.e2e_run = value.to_lowercase() == "yes";
            }
            "e2e passing after work" => {
                progress.work.e2e_passing_after_work = value.to_lowercase() == "yes";
            }
            _ => {}
        },
        "claude code session" => {
            let cc = progress.work.claude_code_session.get_or_insert(ClaudeCodeSessionMetadata::default());
            match key {
                "session id" => cc.session_id = Some(value.to_string()),
                "forked from" => cc.forked_from_session_id = Some(value.to_string()),
                "tools used" => cc.tools_used = Some(parse_tool_counts(value)),
                "summary" => cc.summary = Some(value.to_string()),
                "token usage" => cc.usage = Some(parse_token_usage(value)),
                "cost" => {
                    let cost_str = value.replace(|c: char| !c.is_ascii_digit() && c != '.', "");
                    if let Ok(cost) = cost_str.parse::<f64>() {
                        cc.total_cost_usd = Some(cost);
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn parse_list(value: &str) -> Vec<String> {
    if value.to_lowercase() == "none" || value.is_empty() {
        return Vec::new();
    }
    value
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_tool_counts(value: &str) -> HashMap<String, u32> {
    let mut result = HashMap::new();
    for entry in value.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        // Parse "tool(count)" format
        if let Some(paren_pos) = entry.find('(') {
            if entry.ends_with(')') {
                let tool = entry[..paren_pos].to_string();
                let count_str = &entry[paren_pos + 1..entry.len() - 1];
                if let Ok(count) = count_str.parse::<u32>() {
                    result.insert(tool, count);
                }
            }
        }
    }
    result
}

fn parse_token_usage(value: &str) -> ApiUsage {
    let mut usage = ApiUsage::default();

    // Match patterns like "10000 in" or "5000 out"
    for part in value.split(',').map(|s| s.trim()) {
        let words: Vec<&str> = part.split_whitespace().collect();
        if words.len() >= 2 {
            let num_str = words[0].replace(',', "");
            if let Ok(num) = num_str.parse::<u64>() {
                let label = words[1..].join(" ");
                if label.starts_with("in") {
                    usage.input_tokens = Some(num);
                } else if label.starts_with("out") {
                    usage.output_tokens = Some(num);
                } else if label.contains("cache hits") {
                    usage.cache_read_input_tokens = Some(num);
                } else if label.contains("cache writes") {
                    usage.cache_creation_input_tokens = Some(num);
                }
            }
        }
    }

    usage
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_output() {
        assert_eq!(compress_output("hello world", 100), "hello world");
        assert_eq!(compress_output("a b c d e", 5), "a b c...");
    }

    #[test]
    fn test_parse_list() {
        assert_eq!(parse_list("None"), Vec::<String>::new());
        assert_eq!(parse_list(""), Vec::<String>::new());
        assert_eq!(parse_list("a, b, c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_tool_counts() {
        let result = parse_tool_counts("read(5), write(3)");
        assert_eq!(result.get("read"), Some(&5));
        assert_eq!(result.get("write"), Some(&3));
    }

    #[test]
    fn test_parse_token_usage() {
        let usage = parse_token_usage("10000 in, 5000 out");
        assert_eq!(usage.input_tokens, Some(10000));
        assert_eq!(usage.output_tokens, Some(5000));
    }

    #[test]
    fn test_create_empty_progress() {
        let progress = create_empty_progress("session-1", "task-1", "Fix bug");
        assert_eq!(progress.session_id, "session-1");
        assert_eq!(progress.task_id, "task-1");
        assert_eq!(progress.task_title, "Fix bug");
        assert!(progress.work.subtasks_completed.is_empty());
    }

    #[test]
    fn test_format_and_parse_roundtrip() {
        let progress = SessionProgress {
            session_id: "test-session".to_string(),
            started_at: "2025-01-01T00:00:00Z".to_string(),
            task_id: "task-123".to_string(),
            task_title: "Test Task".to_string(),
            orientation: Orientation {
                repo_state: "clean".to_string(),
                tests_passing_at_start: true,
                ..Default::default()
            },
            work: WorkProgress {
                subtasks_completed: vec!["sub-1".to_string()],
                tests_run: true,
                tests_passing_after_work: true,
                ..Default::default()
            },
            next_session: NextSession {
                suggested_next_steps: vec!["Continue work".to_string()],
                ..Default::default()
            },
            completed_at: None,
        };

        let markdown = format_progress_markdown(&progress);
        let parsed = parse_progress_markdown(&markdown).unwrap();

        assert_eq!(parsed.session_id, "test-session");
        assert_eq!(parsed.task_id, "task-123");
        assert_eq!(parsed.orientation.repo_state, "clean");
        assert!(parsed.orientation.tests_passing_at_start);
        assert_eq!(parsed.work.subtasks_completed, vec!["sub-1"]);
    }
}
