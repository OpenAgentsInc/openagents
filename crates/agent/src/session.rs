//! Session management for agent runs
//!
//! Sessions are stored as JSONL files with events for each action.
//! This allows for crash recovery and replay.

use crate::error::{AgentError, AgentResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

// ============================================================================
// Session Event Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolContent {
    Text { text: String },
    Image { data: String, mime_type: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub tool_call_id: String,
    pub name: String,
    pub result: ToolResultContent,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultContent {
    pub content: Vec<ToolContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTurn {
    pub role: TurnRole,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_results: Option<Vec<ToolResult>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnRole {
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    SessionStart {
        timestamp: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        config: SessionConfig,
    },
    UserMessage {
        timestamp: String,
        content: String,
    },
    Turn {
        timestamp: String,
        turn: AgentTurn,
    },
    Message {
        timestamp: String,
        message: ChatMessage,
    },
    SessionEnd {
        timestamp: String,
        #[serde(rename = "totalTurns")]
        total_turns: u32,
        #[serde(rename = "finalMessage")]
        final_message: Option<String>,
    },
    LogTrimmed {
        timestamp: String,
        dropped: u32,
        kept: u32,
        reason: String,
    },
}

// ============================================================================
// Session
// ============================================================================

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub config: SessionConfig,
    pub messages: Vec<ChatMessage>,
    pub turns: Vec<AgentTurn>,
    pub user_message: String,
}

impl Session {
    /// Create a new session
    pub fn new(config: SessionConfig, user_message: impl Into<String>) -> Self {
        Self {
            id: generate_session_id(),
            config,
            messages: Vec::new(),
            turns: Vec::new(),
            user_message: user_message.into(),
        }
    }

    /// Create a session with a specific ID
    pub fn with_id(
        id: impl Into<String>,
        config: SessionConfig,
        user_message: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            config,
            messages: Vec::new(),
            turns: Vec::new(),
            user_message: user_message.into(),
        }
    }
}

// ============================================================================
// Session I/O
// ============================================================================

/// Configuration for session log trimming
#[derive(Debug, Clone)]
pub struct TrimConfig {
    /// Maximum size in bytes before trimming
    pub max_bytes: u64,
    /// Maximum number of lines before trimming
    pub max_lines: usize,
    /// Number of lines to keep at the start (preserve session_start)
    pub keep_head: usize,
    /// Number of lines to keep at the end (preserve recent events)
    pub keep_tail: usize,
}

impl Default for TrimConfig {
    fn default() -> Self {
        Self {
            max_bytes: 10 * 1024 * 1024, // 10 MB
            max_lines: 10_000,
            keep_head: 10,
            keep_tail: 500,
        }
    }
}

/// Append an event to the session log
pub fn append_event(session_path: &Path, event: &SessionEvent) -> AgentResult<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(session_path)?;

    let line = serde_json::to_string(event)? + "\n";
    file.write_all(line.as_bytes())?;

    // Check if trimming is needed
    maybe_trim_session_log(session_path, &TrimConfig::default())?;

    Ok(())
}

/// Write session start event
pub fn write_session_start(session_path: &Path, session: &Session) -> AgentResult<()> {
    append_event(
        session_path,
        &SessionEvent::SessionStart {
            timestamp: timestamp(),
            session_id: session.id.clone(),
            config: session.config.clone(),
        },
    )
}

/// Write user message event
pub fn write_user_message(session_path: &Path, content: impl Into<String>) -> AgentResult<()> {
    append_event(
        session_path,
        &SessionEvent::UserMessage {
            timestamp: timestamp(),
            content: content.into(),
        },
    )
}

/// Write turn event
pub fn write_turn(session_path: &Path, turn: &AgentTurn) -> AgentResult<()> {
    append_event(
        session_path,
        &SessionEvent::Turn {
            timestamp: timestamp(),
            turn: turn.clone(),
        },
    )
}

/// Write message event
pub fn write_message(session_path: &Path, message: &ChatMessage) -> AgentResult<()> {
    append_event(
        session_path,
        &SessionEvent::Message {
            timestamp: timestamp(),
            message: message.clone(),
        },
    )
}

/// Write session end event
pub fn write_session_end(
    session_path: &Path,
    total_turns: u32,
    final_message: Option<String>,
) -> AgentResult<()> {
    append_event(
        session_path,
        &SessionEvent::SessionEnd {
            timestamp: timestamp(),
            total_turns,
            final_message,
        },
    )
}

/// Load a session from a JSONL file
pub fn load_session(session_path: &Path) -> AgentResult<Session> {
    if !session_path.exists() {
        return Err(AgentError::Session(format!(
            "Session file not found: {}",
            session_path.display()
        )));
    }

    let file = fs::File::open(session_path)?;
    let reader = BufReader::new(file);

    let mut session: Option<Session> = None;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let event: SessionEvent = serde_json::from_str(&line).map_err(|e| {
            AgentError::Session(format!("Invalid JSON in session file: {}", e))
        })?;

        match event {
            SessionEvent::SessionStart {
                session_id,
                config,
                ..
            } => {
                session = Some(Session {
                    id: session_id,
                    config,
                    messages: Vec::new(),
                    turns: Vec::new(),
                    user_message: String::new(),
                });
            }
            SessionEvent::UserMessage { content, .. } => {
                if let Some(ref mut s) = session {
                    s.user_message = content;
                }
            }
            SessionEvent::Message { message, .. } => {
                if let Some(ref mut s) = session {
                    s.messages.push(message);
                }
            }
            SessionEvent::Turn { turn, .. } => {
                if let Some(ref mut s) = session {
                    s.turns.push(turn);
                }
            }
            SessionEvent::LogTrimmed { .. } | SessionEvent::SessionEnd { .. } => {
                // No action needed
            }
        }
    }

    session.ok_or_else(|| AgentError::Session("No session_start event found".to_string()))
}

/// Get the path for a session file
pub fn get_session_path(sessions_dir: &Path, session_id: &str) -> PathBuf {
    sessions_dir.join(format!("{}.jsonl", session_id))
}

/// List all session IDs in a directory
pub fn list_sessions(sessions_dir: &Path) -> AgentResult<Vec<String>> {
    if !sessions_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for entry in fs::read_dir(sessions_dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "jsonl" {
                if let Some(stem) = path.file_stem() {
                    if let Some(name) = stem.to_str() {
                        sessions.push(name.to_string());
                    }
                }
            }
        }
    }

    sessions.sort();
    sessions.reverse();
    Ok(sessions)
}

// ============================================================================
// Helpers
// ============================================================================

fn generate_session_id() -> String {
    let now = Utc::now();
    let date = now.format("%Y%m%d").to_string();
    let time = now.format("%H%M%S").to_string();
    let rand: String = (0..4)
        .map(|_| {
            let idx = rand::random::<usize>() % 36;
            if idx < 10 {
                (b'0' + idx as u8) as char
            } else {
                (b'a' + (idx - 10) as u8) as char
            }
        })
        .collect();
    format!("session-{}-{}-{}", date, time, rand)
}

fn timestamp() -> String {
    Utc::now().to_rfc3339()
}

/// Trim a session log if it exceeds size limits
fn maybe_trim_session_log(session_path: &Path, config: &TrimConfig) -> AgentResult<()> {
    let metadata = fs::metadata(session_path)?;

    // Check if trimming is needed based on file size
    if metadata.len() < config.max_bytes {
        return Ok(());
    }

    let content = fs::read_to_string(session_path)?;
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();

    // Check if trimming is needed based on line count
    if lines.len() <= config.max_lines {
        return Ok(());
    }

    // Trim the log, keeping head and tail
    let total_keep = config.keep_head + config.keep_tail;
    if lines.len() <= total_keep {
        return Ok(());
    }

    let dropped = lines.len() - total_keep;
    let mut new_lines: Vec<String> = Vec::with_capacity(total_keep + 1);

    // Keep head
    new_lines.extend(lines.iter().take(config.keep_head).map(|s| s.to_string()));

    // Add trim marker
    let trim_event = SessionEvent::LogTrimmed {
        timestamp: timestamp(),
        dropped: dropped as u32,
        kept: total_keep as u32,
        reason: "exceeded size limit".to_string(),
    };
    new_lines.push(serde_json::to_string(&trim_event).unwrap_or_default());

    // Keep tail
    new_lines.extend(lines.iter().skip(lines.len() - config.keep_tail).map(|s| s.to_string()));

    let new_content = new_lines.join("\n") + "\n";
    fs::write(session_path, new_content)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_session_creation() {
        let config = SessionConfig {
            model: Some("claude-3".to_string()),
            ..Default::default()
        };
        let session = Session::new(config, "Hello, world!");

        assert!(session.id.starts_with("session-"));
        assert_eq!(session.user_message, "Hello, world!");
        assert!(session.turns.is_empty());
        assert!(session.messages.is_empty());
    }

    #[test]
    fn test_session_roundtrip() -> AgentResult<()> {
        let dir = tempdir()?;
        let session_path = dir.path().join("test-session.jsonl");

        // Create and write session
        let config = SessionConfig {
            model: Some("claude-3".to_string()),
            max_turns: Some(10),
            ..Default::default()
        };
        let session = Session::with_id("test-123", config, "Test message");

        write_session_start(&session_path, &session)?;
        write_user_message(&session_path, "Hello!")?;
        write_message(
            &session_path,
            &ChatMessage {
                role: MessageRole::Assistant,
                content: "Hi there!".to_string(),
                tool_call_id: None,
                name: None,
            },
        )?;
        write_session_end(&session_path, 1, Some("Goodbye".to_string()))?;

        // Load and verify
        let loaded = load_session(&session_path)?;

        assert_eq!(loaded.id, "test-123");
        assert_eq!(loaded.config.model, Some("claude-3".to_string()));
        assert_eq!(loaded.user_message, "Hello!");
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].content, "Hi there!");

        Ok(())
    }

    #[test]
    fn test_list_sessions() -> AgentResult<()> {
        let dir = tempdir()?;

        // Create some session files
        fs::write(dir.path().join("session-a.jsonl"), "")?;
        fs::write(dir.path().join("session-b.jsonl"), "")?;
        fs::write(dir.path().join("session-c.jsonl"), "")?;
        fs::write(dir.path().join("not-a-session.txt"), "")?;

        let sessions = list_sessions(dir.path())?;

        assert_eq!(sessions.len(), 3);
        // Sorted in reverse order
        assert_eq!(sessions[0], "session-c");
        assert_eq!(sessions[1], "session-b");
        assert_eq!(sessions[2], "session-a");

        Ok(())
    }

    #[test]
    fn test_session_event_serialization() {
        let event = SessionEvent::SessionStart {
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            session_id: "sess-123".to_string(),
            config: SessionConfig::default(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"session_start\""));
        assert!(json.contains("\"sessionId\":\"sess-123\""));

        // Roundtrip
        let parsed: SessionEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            SessionEvent::SessionStart { session_id, .. } => {
                assert_eq!(session_id, "sess-123");
            }
            _ => panic!("Wrong event type"),
        }
    }
}
