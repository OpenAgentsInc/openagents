//! JSONL parser for APM extraction
//!
//! Parses both:
//! - Claude Code session logs from `~/.claude/projects/<project>/*.jsonl`
//! - Autopilot session logs from `docs/logs/**/*.jsonl`
//!
//! Extracts APM metrics (messages + tool_calls / duration_minutes).

use crate::apm::{APMSource, SessionData};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Claude Code JSONL line
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCodeLine {
    #[serde(rename = "type")]
    pub line_type: String,
    pub data: Value,
}

/// Parse a Claude Code JSONL session file and extract APM data
///
/// # Arguments
/// * `path` - Path to the .jsonl file
///
/// # Returns
/// SessionData with message counts, tool calls, and timestamps
pub fn parse_claude_code_session(path: impl AsRef<Path>) -> Result<SessionData> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let mut messages = 0u32;
    let mut tool_calls = 0u32;
    let mut start_time: Option<DateTime<Utc>> = None;
    let mut end_time: Option<DateTime<Utc>> = None;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let parsed: ClaudeCodeLine = serde_json::from_str(line)
            .with_context(|| format!("Failed to parse JSONL line in {}", path.display()))?;

        match parsed.line_type.as_str() {
            "message" => {
                // Extract timestamp from message
                if let Some(timestamp_str) = parsed.data.get("timestamp").and_then(|t| t.as_str()) {
                    if let Ok(timestamp) = DateTime::parse_from_rfc3339(timestamp_str) {
                        let utc_timestamp = timestamp.with_timezone(&Utc);
                        if start_time.is_none() {
                            start_time = Some(utc_timestamp);
                        }
                        end_time = Some(utc_timestamp);
                    }
                }

                // Count this message
                messages += 1;
            }
            "tool_use" | "tool_call" => {
                // Extract timestamp
                if let Some(timestamp_str) = parsed.data.get("timestamp").and_then(|t| t.as_str()) {
                    if let Ok(timestamp) = DateTime::parse_from_rfc3339(timestamp_str) {
                        let utc_timestamp = timestamp.with_timezone(&Utc);
                        if start_time.is_none() {
                            start_time = Some(utc_timestamp);
                        }
                        end_time = Some(utc_timestamp);
                    }
                }

                tool_calls += 1;
            }
            "tool_result" => {
                // Update end time but don't count as action
                if let Some(timestamp_str) = parsed.data.get("timestamp").and_then(|t| t.as_str()) {
                    if let Ok(timestamp) = DateTime::parse_from_rfc3339(timestamp_str) {
                        end_time = Some(timestamp.with_timezone(&Utc));
                    }
                }
            }
            _ => {
                // Ignore other line types (init, status, etc.)
            }
        }
    }

    // Validate we have data
    let start = start_time.ok_or_else(|| {
        anyhow::anyhow!("No timestamps found in session file: {}", path.display())
    })?;
    let end = end_time.unwrap_or(start);

    Ok(SessionData {
        source: APMSource::ClaudeCode,
        start_time: start,
        end_time: end,
        messages,
        tool_calls,
    })
}

/// Parse an Autopilot JSONL session file and extract APM data
///
/// Autopilot JSONL format has timestamp at top level:
/// ```json
/// {"type":"user","message":{...},"timestamp":"2025-12-22T10:00:00Z"}
/// {"type":"assistant","message":{...},"timestamp":"2025-12-22T10:00:05Z"}
/// ```
///
/// # Arguments
/// * `path` - Path to the .jsonl file
///
/// # Returns
/// SessionData with message counts, tool calls, and timestamps
pub fn parse_autopilot_session(path: impl AsRef<Path>) -> Result<SessionData> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let mut user_messages = 0u32;
    let mut assistant_messages = 0u32;
    let mut tool_calls = 0u32;
    let mut start_time: Option<DateTime<Utc>> = None;
    let mut end_time: Option<DateTime<Utc>> = None;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let parsed: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue, // Skip malformed lines
        };

        // Extract timestamp (at top level in autopilot format)
        if let Some(timestamp_str) = parsed.get("timestamp").and_then(|t| t.as_str()) {
            if let Ok(timestamp) = DateTime::parse_from_rfc3339(timestamp_str) {
                let utc_timestamp = timestamp.with_timezone(&Utc);
                if start_time.is_none() {
                    start_time = Some(utc_timestamp);
                }
                end_time = Some(utc_timestamp);
            }
        }

        // Count by type
        if let Some(line_type) = parsed.get("type").and_then(|t| t.as_str()) {
            match line_type {
                "user" => user_messages += 1,
                "assistant" => assistant_messages += 1,
                "system" => {
                    // System messages (init, status) don't count as actions
                }
                "result" => {
                    // Result messages update end time but don't count
                }
                "tool_use" | "tool_progress" => {
                    tool_calls += 1;
                }
                _ => {}
            }
        }
    }

    // Total messages = user + assistant
    let messages = user_messages + assistant_messages;

    // Validate we have data
    let start = start_time.ok_or_else(|| {
        anyhow::anyhow!("No timestamps found in session file: {}", path.display())
    })?;
    let end = end_time.unwrap_or(start);

    Ok(SessionData {
        source: APMSource::Autopilot,
        start_time: start,
        end_time: end,
        messages,
        tool_calls,
    })
}

/// Find all Autopilot JSONL files in the logs directory
///
/// # Arguments
/// * `logs_dir` - Path to the logs directory (e.g., "docs/logs")
///
/// # Returns
/// Vector of paths to .jsonl autopilot session files
pub fn find_autopilot_sessions(logs_dir: impl AsRef<Path>) -> Result<Vec<std::path::PathBuf>> {
    let logs_dir = logs_dir.as_ref();
    if !logs_dir.exists() {
        anyhow::bail!("Autopilot logs directory not found: {}", logs_dir.display());
    }

    let mut sessions = Vec::new();

    // Walk through date directories
    for date_entry in std::fs::read_dir(logs_dir)? {
        let date_entry = date_entry?;
        let date_path = date_entry.path();

        if !date_path.is_dir() {
            continue;
        }

        // Find .jsonl files in each date directory
        for entry in std::fs::read_dir(&date_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                sessions.push(path);
            }
        }
    }

    Ok(sessions)
}

/// Find all Claude Code session JSONL files for a project
///
/// # Arguments
/// * `project_name` - Name of the project
///
/// # Returns
/// Vector of paths to .jsonl session files
pub fn find_claude_code_sessions(project_name: &str) -> Result<Vec<std::path::PathBuf>> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;
    let project_dir = std::path::PathBuf::from(home)
        .join(".claude")
        .join("projects")
        .join(project_name);

    if !project_dir.exists() {
        anyhow::bail!(
            "Claude Code project directory not found: {}",
            project_dir.display()
        );
    }

    let mut sessions = Vec::new();
    for entry in std::fs::read_dir(&project_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            sessions.push(path);
        }
    }

    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_empty_session() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("empty.jsonl");
        std::fs::write(&file_path, "").unwrap();

        let result = parse_claude_code_session(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_session_with_messages_and_tools() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("session.jsonl");

        let mut file = std::fs::File::create(&file_path).unwrap();

        // Write some sample JSONL lines
        writeln!(
            file,
            r#"{{"type":"message","data":{{"timestamp":"2025-12-22T12:00:00Z","role":"user","content":"hello"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"message","data":{{"timestamp":"2025-12-22T12:00:10Z","role":"assistant","content":"hi"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"tool_use","data":{{"timestamp":"2025-12-22T12:00:20Z","tool":"Read","params":{{}}}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"tool_result","data":{{"timestamp":"2025-12-22T12:00:25Z","success":true}}}}"#
        )
        .unwrap();
        drop(file);

        let session = parse_claude_code_session(&file_path).unwrap();

        assert_eq!(session.source, APMSource::ClaudeCode);
        assert_eq!(session.messages, 2);
        assert_eq!(session.tool_calls, 1);
        assert_eq!(session.actions(), 3);

        // Duration should be 25 seconds = ~0.417 minutes
        let duration = session.duration_minutes();
        assert!((duration - 0.416667).abs() < 0.01);

        // APM should be 3 actions / 0.417 minutes ≈ 7.2
        let apm = session.apm().unwrap();
        assert!((apm - 7.2).abs() < 0.5);
    }

    #[test]
    fn test_parse_session_ignores_empty_lines() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("session.jsonl");

        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "").unwrap();
        writeln!(
            file,
            r#"{{"type":"message","data":{{"timestamp":"2025-12-22T12:00:00Z","role":"user","content":"hello"}}}}"#
        )
        .unwrap();
        writeln!(file, "   ").unwrap();
        writeln!(
            file,
            r#"{{"type":"message","data":{{"timestamp":"2025-12-22T12:00:10Z","role":"assistant","content":"hi"}}}}"#
        )
        .unwrap();
        drop(file);

        let session = parse_claude_code_session(&file_path).unwrap();
        assert_eq!(session.messages, 2);
    }

    #[test]
    fn test_parse_session_with_tool_call_variant() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("session.jsonl");

        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(
            file,
            r#"{{"type":"message","data":{{"timestamp":"2025-12-22T12:00:00Z","role":"user","content":"test"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"tool_call","data":{{"timestamp":"2025-12-22T12:00:10Z","tool":"Bash"}}}}"#
        )
        .unwrap();
        drop(file);

        let session = parse_claude_code_session(&file_path).unwrap();
        assert_eq!(session.messages, 1);
        assert_eq!(session.tool_calls, 1);
    }

    #[test]
    fn test_parse_autopilot_session() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("session.jsonl");

        let mut file = std::fs::File::create(&file_path).unwrap();

        // Write autopilot format JSONL (timestamp at top level)
        writeln!(
            file,
            r#"{{"type":"user","message":{{"content":"fix bug"}},"timestamp":"2025-12-22T10:00:00Z"}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"assistant","message":{{"content":"I'll investigate"}},"timestamp":"2025-12-22T10:00:05Z"}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"tool_use","tool":"Read","input":{{"file":"test.rs"}},"timestamp":"2025-12-22T10:00:10Z"}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"result","message":{{"success":true}},"timestamp":"2025-12-22T10:00:30Z"}}"#
        )
        .unwrap();
        drop(file);

        let session = parse_autopilot_session(&file_path).unwrap();

        assert_eq!(session.source, APMSource::Autopilot);
        assert_eq!(session.messages, 2); // user + assistant
        assert_eq!(session.tool_calls, 1);
        assert_eq!(session.actions(), 3);

        // Duration should be 30 seconds = 0.5 minutes
        let duration = session.duration_minutes();
        assert!((duration - 0.5).abs() < 0.01);

        // APM should be 3 actions / 0.5 minutes = 6.0
        let apm = session.apm().unwrap();
        assert!((apm - 6.0).abs() < 0.1);
    }

    #[test]
    fn test_parse_autopilot_empty_session() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("empty.jsonl");
        std::fs::write(&file_path, "").unwrap();

        let result = parse_autopilot_session(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_autopilot_skips_system_messages() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("session.jsonl");

        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(
            file,
            r#"{{"type":"system","message":{{"session_id":"abc"}},"timestamp":"2025-12-22T10:00:00Z"}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"user","message":{{"content":"test"}},"timestamp":"2025-12-22T10:00:05Z"}}"#
        )
        .unwrap();
        drop(file);

        let session = parse_autopilot_session(&file_path).unwrap();
        // System messages shouldn't count
        assert_eq!(session.messages, 1);
        assert_eq!(session.tool_calls, 0);
    }
}
