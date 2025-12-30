//! Read Claude Code sessions from ~/.claude/projects/
//!
//! This module parses JSONL session files to display recent sessions
//! in the autopilot shell sidebar and load full conversation history.

use chrono::{DateTime, Local, TimeZone};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// A Claude Code session summary for display
#[derive(Debug, Clone)]
pub struct ClaudeSession {
    pub session_id: String,
    pub timestamp: DateTime<Local>,
    pub project: String,
    pub file_path: PathBuf,
}

/// A message from a Claude Code session
#[derive(Debug, Clone)]
pub struct SessionMessage {
    pub role: String,        // "user" or "assistant"
    pub content: String,     // text content
    pub is_tool_use: bool,   // whether this is a tool use block
    pub tool_name: Option<String>,
}

/// JSONL entry from Claude Code session files (metadata)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionEntry {
    session_id: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
}

/// JSONL entry for message content
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageEntry {
    #[serde(rename = "type")]
    entry_type: Option<String>,
    message: Option<MessageContent>,
}

#[derive(Debug, Deserialize)]
struct MessageContent {
    role: Option<String>,
    content: Option<Vec<ContentBlock>>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
    name: Option<String>,  // for tool_use blocks
}

/// List recent Claude Code sessions from ~/.claude/projects/
///
/// Scans all project directories for JSONL session files and extracts
/// session metadata. Returns up to 20 most recent unique sessions.
pub fn list_claude_sessions() -> Vec<ClaudeSession> {
    let projects_dir = match dirs::home_dir() {
        Some(home) => home.join(".claude").join("projects"),
        None => return Vec::new(),
    };

    if !projects_dir.exists() {
        return Vec::new();
    }

    let mut sessions: HashMap<String, ClaudeSession> = HashMap::new();

    // Scan all project directories
    if let Ok(entries) = fs::read_dir(&projects_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let project_dir = entry.path();
            if !project_dir.is_dir() {
                continue;
            }

            // Extract project name from directory name (e.g., "-Users-chris-code-openagents")
            let project_name = project_dir
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.replace('-', "/"))
                .unwrap_or_default();

            // Scan JSONL files in this project directory
            if let Ok(files) = fs::read_dir(&project_dir) {
                for file_entry in files.filter_map(|e| e.ok()) {
                    let file_path = file_entry.path();

                    // Only process .jsonl files
                    if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                        continue;
                    }

                    // Try to extract session from file
                    if let Some(session) = parse_session_file(&file_path, &project_name) {
                        // Deduplicate by session_id, keeping most recent
                        let entry = sessions.entry(session.session_id.clone()).or_insert(session.clone());
                        if session.timestamp > entry.timestamp {
                            *entry = session;
                        }
                    }
                }
            }
        }
    }

    // Convert to vec and sort by timestamp (most recent first)
    let mut result: Vec<ClaudeSession> = sessions.into_values().collect();
    result.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    // Limit to 20 sessions
    result.truncate(20);
    result
}

/// Parse a single JSONL session file to extract session metadata
fn parse_session_file(path: &PathBuf, project_name: &str) -> Option<ClaudeSession> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    // Get file modification time as fallback timestamp
    let file_mtime = fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| {
            let duration = t.duration_since(std::time::UNIX_EPOCH).ok()?;
            Local.timestamp_opt(duration.as_secs() as i64, 0).single()
        })
        .unwrap_or_else(Local::now);

    let mut session_id: Option<String> = None;
    let mut latest_timestamp = file_mtime;

    // Read first few lines to find session_id
    for line in reader.lines().take(10) {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        if let Ok(entry) = serde_json::from_str::<SessionEntry>(&line) {
            if session_id.is_none() {
                session_id = entry.session_id;
            }
            // Parse timestamp if present
            if let Some(ts) = entry.timestamp {
                if let Ok(parsed) = DateTime::parse_from_rfc3339(&ts) {
                    let local: DateTime<Local> = parsed.into();
                    if local > latest_timestamp {
                        latest_timestamp = local;
                    }
                }
            }
        }
    }

    // Use filename as session_id fallback (UUID.jsonl)
    let session_id = session_id.or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| s.len() == 36) // UUID length
            .map(|s| s.to_string())
    })?;

    Some(ClaudeSession {
        session_id,
        timestamp: latest_timestamp,
        project: project_name.to_string(),
        file_path: path.clone(),
    })
}

/// Find a session by ID and return its file path
pub fn find_session_file(session_id: &str) -> Option<PathBuf> {
    let sessions = list_claude_sessions();
    sessions
        .into_iter()
        .find(|s| s.session_id == session_id)
        .map(|s| s.file_path)
}

/// Load all messages from a Claude Code session JSONL file
pub fn load_session_messages(path: &PathBuf) -> Vec<SessionMessage> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let reader = BufReader::new(file);
    let mut messages = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        // Parse as generic JSON first to check type
        let entry: MessageEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Only process user and assistant messages
        let entry_type = match entry.entry_type.as_deref() {
            Some("user") | Some("assistant") => entry.entry_type.unwrap(),
            _ => continue,
        };

        // Extract content blocks
        if let Some(msg) = entry.message {
            if let Some(content) = msg.content {
                for block in content {
                    match block.block_type.as_str() {
                        "text" => {
                            if let Some(text) = block.text {
                                if !text.trim().is_empty() {
                                    messages.push(SessionMessage {
                                        role: entry_type.clone(),
                                        content: text,
                                        is_tool_use: false,
                                        tool_name: None,
                                    });
                                }
                            }
                        }
                        "tool_use" => {
                            messages.push(SessionMessage {
                                role: entry_type.clone(),
                                content: format!("Tool: {}", block.name.as_deref().unwrap_or("unknown")),
                                is_tool_use: true,
                                tool_name: block.name,
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    messages
}
