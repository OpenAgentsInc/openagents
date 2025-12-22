//! Claude Code JSONL parser for APM extraction
//!
//! Parses Claude Code session logs from ~/.claude/projects/<project>/*.jsonl
//! and extracts APM metrics (messages + tool_calls / duration_minutes).

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
                if let Some(timestamp_str) = parsed.data.get("timestamp").and_then(|t| t.as_str())
                {
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
                if let Some(timestamp_str) = parsed.data.get("timestamp").and_then(|t| t.as_str())
                {
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
                if let Some(timestamp_str) = parsed.data.get("timestamp").and_then(|t| t.as_str())
                {
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
        anyhow::anyhow!(
            "No timestamps found in session file: {}",
            path.display()
        )
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
}
