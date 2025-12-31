//! Replay bundle format for demo publishing
//!
//! Converts JSONL session logs into publishable replay bundles with:
//! - Timeline of events
//! - Metadata (duration, cost, tests)
//! - Receipts (CI status, files changed)
//! - Redacted content (secrets removed)

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::logger::LogEntry;

/// A publishable replay bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBundle {
    pub version: String,
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub metadata: ReplayMetadata,
    pub timeline: Vec<TimelineEvent>,
    pub receipts: ReplayReceipts,
}

/// Metadata about the autopilot run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayMetadata {
    pub issue_url: Option<String>,
    pub pr_url: Option<String>,
    pub duration_seconds: u64,
    pub playback_speed: f32,
    pub demo_duration_seconds: u64,
    pub model: String,
    pub cost_usd: Option<f64>,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
}

/// An event in the timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    /// Timestamp in milliseconds from start
    pub t: u64,

    /// Event type
    #[serde(rename = "type")]
    pub event_type: String,

    /// Optional tool name for tool calls
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,

    /// Event data
    pub data: serde_json::Value,
}

/// Results and verification info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayReceipts {
    pub tests_run: Option<usize>,
    pub tests_passed: Option<usize>,
    pub ci_status: Option<String>,
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

impl ReplayBundle {
    /// Load a replay bundle from a JSONL session log
    pub fn from_jsonl(path: &Path) -> Result<Self> {
        let file = fs::File::open(path).context("Failed to open session log")?;

        let reader = BufReader::new(file);
        let mut entries: Vec<LogEntry> = Vec::new();

        for line in reader.lines() {
            let line = line.context("Failed to read line from log")?;
            if line.trim().is_empty() {
                continue;
            }

            let entry: LogEntry =
                serde_json::from_str(&line).context("Failed to parse log entry")?;
            entries.push(entry);
        }

        if entries.is_empty() {
            anyhow::bail!("Session log is empty");
        }

        Self::from_entries(entries, path)
    }

    /// Convert log entries to a replay bundle
    fn from_entries(entries: Vec<LogEntry>, _log_path: &Path) -> Result<Self> {
        let session_id = entries
            .first()
            .map(|e| e.session_id.clone())
            .ok_or_else(|| anyhow::anyhow!("No entries"))?;

        // Parse timestamps
        let start_time = entries
            .first()
            .and_then(|e| DateTime::parse_from_rfc3339(&e.timestamp).ok())
            .ok_or_else(|| anyhow::anyhow!("Invalid start timestamp"))?
            .with_timezone(&Utc);

        let end_time = entries
            .last()
            .and_then(|e| DateTime::parse_from_rfc3339(&e.timestamp).ok())
            .ok_or_else(|| anyhow::anyhow!("Invalid end timestamp"))?
            .with_timezone(&Utc);

        let duration_seconds = (end_time - start_time).num_seconds() as u64;

        // Build timeline
        let mut timeline = Vec::new();

        for entry in &entries {
            let timestamp = DateTime::parse_from_rfc3339(&entry.timestamp)
                .context("Invalid timestamp")?
                .with_timezone(&Utc);

            let t_ms = (timestamp - start_time).num_milliseconds() as u64;

            let event = match entry.event_type.as_str() {
                "tool_use" => {
                    let tool = entry
                        .data
                        .get("tool")
                        .and_then(|t| t.as_str())
                        .map(String::from);

                    TimelineEvent {
                        t: t_ms,
                        event_type: "tool_call".to_string(),
                        tool,
                        data: entry.data.clone(),
                    }
                }
                "tool_result" => {
                    let tool = entry
                        .data
                        .get("tool")
                        .and_then(|t| t.as_str())
                        .map(String::from);

                    TimelineEvent {
                        t: t_ms,
                        event_type: "tool_result".to_string(),
                        tool,
                        data: entry.data.clone(),
                    }
                }
                "assistant" => TimelineEvent {
                    t: t_ms,
                    event_type: "assistant".to_string(),
                    tool: None,
                    data: entry.data.clone(),
                },
                "phase_start" => TimelineEvent {
                    t: t_ms,
                    event_type: "phase_start".to_string(),
                    tool: None,
                    data: entry.data.clone(),
                },
                "phase_end" => TimelineEvent {
                    t: t_ms,
                    event_type: "phase_end".to_string(),
                    tool: None,
                    data: entry.data.clone(),
                },
                _ => continue,
            };

            timeline.push(event);
        }

        // Extract metadata
        let model = entries
            .iter()
            .find_map(|e| {
                if e.event_type == "phase_start" && e.phase == "planning" {
                    e.data
                        .get("model")
                        .and_then(|m| m.as_str())
                        .map(String::from)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "claude-sonnet-4-5-20250929".to_string());

        // Calculate default playback speed (2x for demo)
        let playback_speed = 2.0;
        let demo_duration_seconds = (duration_seconds as f32 / playback_speed) as u64;

        // Extract receipts from verification results
        let receipts = extract_receipts(&entries);

        Ok(ReplayBundle {
            version: "1.0".to_string(),
            id: format!("replay_{}", session_id),
            created_at: start_time,
            metadata: ReplayMetadata {
                issue_url: None,
                pr_url: None,
                duration_seconds,
                playback_speed,
                demo_duration_seconds,
                model,
                cost_usd: None,
                tokens_in: None,
                tokens_out: None,
            },
            timeline,
            receipts,
        })
    }

    /// Save replay bundle to JSON file
    pub fn save(&self, output_path: &Path) -> Result<()> {
        let json =
            serde_json::to_string_pretty(self).context("Failed to serialize replay bundle")?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).context("Failed to create output directory")?;
        }

        fs::write(output_path, json).context("Failed to write replay bundle")?;

        Ok(())
    }

    /// Load replay bundle from JSON file
    pub fn load(path: &Path) -> Result<Self> {
        let json = fs::read_to_string(path).context("Failed to read replay bundle")?;

        let bundle: ReplayBundle =
            serde_json::from_str(&json).context("Failed to parse replay bundle")?;

        Ok(bundle)
    }
}

/// Extract receipts from log entries
fn extract_receipts(entries: &[LogEntry]) -> ReplayReceipts {
    let mut tests_run = None;
    let mut tests_passed = None;
    let mut ci_status = None;
    let mut files_changed = 0;
    let lines_added = 0;
    let lines_removed = 0;

    for entry in entries {
        if entry.event_type == "result" && entry.phase == "verification" {
            // Extract verification results
            if let Some(checks) = entry.data.get("checks").and_then(|c| c.as_object()) {
                if let Some(tests) = checks.get("tests_passing") {
                    tests_run = tests
                        .get("total")
                        .and_then(|t| t.as_u64())
                        .map(|n| n as usize);
                    tests_passed = tests
                        .get("passed")
                        .and_then(|t| t.as_u64())
                        .map(|n| n as usize);
                }

                if let Some(ci) = checks.get("ci_status") {
                    ci_status = ci.get("status").and_then(|s| s.as_str()).map(String::from);
                }
            }
        }

        if entry.event_type == "tool_result" {
            // Count file edits
            if let Some(tool) = entry.data.get("tool").and_then(|t| t.as_str()) {
                if tool == "Edit" || tool == "Write" {
                    files_changed += 1;
                }
            }
        }
    }

    ReplayReceipts {
        tests_run,
        tests_passed,
        ci_status,
        files_changed,
        lines_added,
        lines_removed,
    }
}

/// Redact secrets and PII from replay bundle
pub fn redact_replay(bundle: &mut ReplayBundle) -> Result<()> {
    for event in &mut bundle.timeline {
        redact_value(&mut event.data);
    }

    Ok(())
}

/// Redact secrets from a JSON value
fn redact_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) => {
            *s = redact_string(s);
        }
        serde_json::Value::Object(obj) => {
            for (key, val) in obj.iter_mut() {
                // Redact known secret fields
                if is_secret_field(key) {
                    *val = serde_json::Value::String("[REDACTED]".to_string());
                } else {
                    redact_value(val);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                redact_value(item);
            }
        }
        _ => {}
    }
}

/// Check if a field name indicates a secret
fn is_secret_field(key: &str) -> bool {
    let lower = key.to_lowercase();
    lower.contains("token")
        || lower.contains("key")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("auth")
}

/// Redact secrets from a string
fn redact_string(s: &str) -> String {
    // API keys
    let s = regex::Regex::new(r"sk-[a-zA-Z0-9]{48}")
        .unwrap()
        .replace_all(s, "sk-[REDACTED]");

    // GitHub tokens
    let s = regex::Regex::new(r"gh[ps]_[a-zA-Z0-9]{36}")
        .unwrap()
        .replace_all(&s, "gh_[REDACTED]");

    // Replace home directories
    let s = if let Ok(home) = std::env::var("HOME") {
        s.replace(&home, "~")
    } else {
        s.to_string()
    };

    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_api_key() {
        let input = "Using API key sk-abc123def456ghi789jkl012mno345pqr678stu901vwx234";
        let output = redact_string(input);
        assert!(output.contains("sk-[REDACTED]"));
        assert!(!output.contains("abc123"));
    }

    #[test]
    fn test_redact_home_path() {
        unsafe {
            std::env::set_var("HOME", "/home/testuser");
        }
        let input = "/home/testuser/projects/myrepo";
        let output = redact_string(input);
        assert_eq!(output, "~/projects/myrepo");
    }

    #[test]
    fn test_is_secret_field() {
        assert!(is_secret_field("api_key"));
        assert!(is_secret_field("github_token"));
        assert!(is_secret_field("secret"));
        assert!(is_secret_field("password"));
        assert!(!is_secret_field("username"));
        assert!(!is_secret_field("email"));
    }
}
