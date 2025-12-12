//! File-based session storage with JSONL format and compaction
//!
//! Sessions are stored as JSONL files where each line is a `SessionEntry`.
//! Compaction summarizes older messages to reduce context size.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use thiserror::Error;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};

use crate::{
    generate_session_id, generate_uuid, timestamp, ActiveSession, AssistantMessage,
    EndSessionOptions, LogAssistantOptions, MessageContent, SessionEntry,
    SessionMetadata, SessionOutcome, StartSessionOptions, ToolResultBlock,
    ToolResultMessage, UsageMetrics, UserMessage,
};

/// Errors for file store operations
#[derive(Error, Debug)]
pub enum FileStoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Session not found: {0}")]
    NotFound(String),

    #[error("Invalid session state: {0}")]
    InvalidState(String),
}

/// Result type for file store operations
pub type FileStoreResult<T> = Result<T, FileStoreError>;

/// File-based session store using JSONL format
///
/// Each session is stored as a `.jsonl` file where each line is a JSON-encoded
/// `SessionEntry`. Files are stored in a configurable sessions directory.
pub struct FileSessionStore {
    /// Base directory for session files
    sessions_dir: PathBuf,
}

impl FileSessionStore {
    /// Create a new file session store
    pub fn new(sessions_dir: impl Into<PathBuf>) -> Self {
        Self {
            sessions_dir: sessions_dir.into(),
        }
    }

    /// Create with default sessions directory (~/.openagents/sessions)
    pub fn default_dir() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self::new(PathBuf::from(home).join(".openagents/sessions"))
    }

    /// Ensure sessions directory exists
    pub async fn init(&self) -> FileStoreResult<()> {
        fs::create_dir_all(&self.sessions_dir).await?;
        Ok(())
    }

    /// Get the file path for a session
    fn session_path(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{}.jsonl", session_id))
    }

    /// Start a new session
    pub async fn start_session(
        &self,
        options: StartSessionOptions,
    ) -> FileStoreResult<ActiveSession> {
        let session_id = options.session_id.unwrap_or_else(generate_session_id);
        let uuid = generate_uuid();
        let now = timestamp();

        let entry = SessionEntry::SessionStart {
            uuid: uuid.clone(),
            timestamp: now.clone(),
            session_id: session_id.clone(),
            parent_uuid: None,
            task_id: options.task_id.clone(),
            cwd: options.cwd.clone().unwrap_or_else(|| ".".to_string()),
            model: options.model,
            provider: options.provider,
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            git_branch: options.git_branch,
        };

        let file_path = self.session_path(&session_id);
        self.append_entry(&file_path, &entry).await?;

        Ok(ActiveSession {
            session_id: session_id.clone(),
            file_path: file_path.to_string_lossy().to_string(),
            task_id: options.task_id,
            started_at: now,
            last_uuid: Some(uuid),
            turn_count: 0,
            cumulative_usage: UsageMetrics::new(),
            files_modified: HashSet::new(),
        })
    }

    /// Log a user message
    pub async fn log_user_message(
        &self,
        session: &mut ActiveSession,
        content: MessageContent,
        user_type: Option<String>,
    ) -> FileStoreResult<()> {
        let uuid = generate_uuid();

        let entry = SessionEntry::User {
            uuid: uuid.clone(),
            timestamp: timestamp(),
            session_id: session.session_id.clone(),
            parent_uuid: session.last_uuid.clone(),
            message: UserMessage::new(content),
            user_type,
            cwd: None,
        };

        let file_path = self.session_path(&session.session_id);
        self.append_entry(&file_path, &entry).await?;

        session.last_uuid = Some(uuid);
        Ok(())
    }

    /// Log an assistant message
    pub async fn log_assistant_message(
        &self,
        session: &mut ActiveSession,
        content: MessageContent,
        options: LogAssistantOptions,
    ) -> FileStoreResult<()> {
        let uuid = generate_uuid();

        let mut message = AssistantMessage::new(content);
        message.model = options.model;
        message.id = options.message_id;
        message.stop_reason = options.stop_reason;

        let entry = SessionEntry::Assistant {
            uuid: uuid.clone(),
            timestamp: timestamp(),
            session_id: session.session_id.clone(),
            parent_uuid: session.last_uuid.clone(),
            message,
            usage: options.usage.clone(),
            request_id: options.request_id,
        };

        let file_path = self.session_path(&session.session_id);
        self.append_entry(&file_path, &entry).await?;

        if let Some(usage) = &options.usage {
            session.cumulative_usage.add(usage);
        }

        session.last_uuid = Some(uuid);
        session.turn_count += 1;
        Ok(())
    }

    /// Log a tool result
    pub async fn log_tool_result(
        &self,
        session: &mut ActiveSession,
        tool_use_id: String,
        result: serde_json::Value,
        is_error: Option<bool>,
    ) -> FileStoreResult<()> {
        let uuid = generate_uuid();

        let block = ToolResultBlock::new(tool_use_id, result.clone(), is_error);

        let entry = SessionEntry::ToolResult {
            uuid: uuid.clone(),
            timestamp: timestamp(),
            session_id: session.session_id.clone(),
            parent_uuid: session.last_uuid.clone(),
            message: ToolResultMessage::new(vec![block]),
            tool_use_result: Some(result),
        };

        let file_path = self.session_path(&session.session_id);
        self.append_entry(&file_path, &entry).await?;

        session.last_uuid = Some(uuid);
        Ok(())
    }

    /// End a session
    pub async fn end_session(
        &self,
        session: &ActiveSession,
        outcome: SessionOutcome,
        options: EndSessionOptions,
    ) -> FileStoreResult<()> {
        let entry = SessionEntry::SessionEnd {
            uuid: generate_uuid(),
            timestamp: timestamp(),
            session_id: session.session_id.clone(),
            parent_uuid: session.last_uuid.clone(),
            outcome,
            reason: options.reason,
            total_turns: session.turn_count,
            total_usage: Some(session.cumulative_usage.clone()),
            files_modified: Some(session.files_modified.iter().cloned().collect()),
            commits: options.commits,
        };

        let file_path = self.session_path(&session.session_id);
        self.append_entry(&file_path, &entry).await
    }

    /// Load all entries for a session
    pub async fn load_session(&self, session_id: &str) -> FileStoreResult<Vec<SessionEntry>> {
        let file_path = self.session_path(session_id);

        if !file_path.exists() {
            return Err(FileStoreError::NotFound(session_id.to_string()));
        }

        let file = File::open(&file_path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        let mut entries = Vec::new();
        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }
            let entry: SessionEntry = serde_json::from_str(&line)?;
            entries.push(entry);
        }

        Ok(entries)
    }

    /// Resume an existing session
    pub async fn resume_session(&self, session_id: &str) -> FileStoreResult<ActiveSession> {
        let entries = self.load_session(session_id).await?;

        if entries.is_empty() {
            return Err(FileStoreError::NotFound(session_id.to_string()));
        }

        // Find session start
        let start = entries
            .iter()
            .find(|e| e.is_session_start())
            .ok_or_else(|| FileStoreError::InvalidState("No session start entry".to_string()))?;

        // Check not already ended
        if entries.iter().any(|e| e.is_session_end()) {
            return Err(FileStoreError::InvalidState(
                "Session already ended".to_string(),
            ));
        }

        // Extract info from start
        let (task_id, started_at, _cwd) = match start {
            SessionEntry::SessionStart {
                task_id,
                timestamp,
                cwd,
                ..
            } => (task_id.clone(), timestamp.clone(), cwd.clone()),
            _ => unreachable!(),
        };

        // Calculate cumulative state
        let mut turn_count = 0u32;
        let mut cumulative_usage = UsageMetrics::new();
        let files_modified = HashSet::new();
        let mut last_uuid = None;

        for entry in &entries {
            match entry {
                SessionEntry::SessionStart { uuid, .. } => {
                    last_uuid = Some(uuid.clone());
                }
                SessionEntry::User { uuid, .. } => {
                    last_uuid = Some(uuid.clone());
                }
                SessionEntry::Assistant { uuid, usage, .. } => {
                    last_uuid = Some(uuid.clone());
                    turn_count += 1;
                    if let Some(u) = usage {
                        cumulative_usage.add(u);
                    }
                }
                SessionEntry::ToolResult { uuid, .. } => {
                    last_uuid = Some(uuid.clone());
                }
                SessionEntry::SessionEnd { .. } => {}
            }
        }

        Ok(ActiveSession {
            session_id: session_id.to_string(),
            file_path: self.session_path(session_id).to_string_lossy().to_string(),
            task_id,
            started_at,
            last_uuid,
            turn_count,
            cumulative_usage,
            files_modified,
        })
    }

    /// List all session IDs (sorted newest first)
    pub async fn list_sessions(&self) -> FileStoreResult<Vec<String>> {
        let mut sessions = Vec::new();

        let mut read_dir = fs::read_dir(&self.sessions_dir).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "jsonl") {
                if let Some(stem) = path.file_stem() {
                    sessions.push(stem.to_string_lossy().to_string());
                }
            }
        }

        // Sort newest first
        sessions.sort();
        sessions.reverse();

        Ok(sessions)
    }

    /// Get session metadata without loading all entries
    pub async fn get_session_metadata(
        &self,
        session_id: &str,
    ) -> FileStoreResult<SessionMetadata> {
        let entries = self.load_session(session_id).await?;
        SessionMetadata::from_entries(session_id.to_string(), &entries)
            .ok_or_else(|| FileStoreError::InvalidState("Invalid session format".to_string()))
    }

    /// Search sessions by content
    pub async fn search_sessions(&self, term: &str) -> FileStoreResult<Vec<SessionMetadata>> {
        let lower_term = term.to_lowercase();
        let mut results = Vec::new();

        for session_id in self.list_sessions().await? {
            let entries = match self.load_session(&session_id).await {
                Ok(e) => e,
                Err(_) => continue,
            };

            let mut found = false;
            for entry in &entries {
                let text = match entry {
                    SessionEntry::User { message, .. } => message.content.extract_text(),
                    SessionEntry::Assistant { message, .. } => message.content.extract_text(),
                    _ => String::new(),
                };
                if text.to_lowercase().contains(&lower_term) {
                    found = true;
                    break;
                }
            }

            if found {
                if let Some(metadata) = SessionMetadata::from_entries(session_id, &entries) {
                    results.push(metadata);
                }
            }
        }

        Ok(results)
    }

    /// Compact a session by summarizing older messages
    ///
    /// This creates a compaction entry that summarizes messages before
    /// `keep_last_n` turns, reducing context size while preserving history.
    pub async fn compact_session(
        &self,
        session_id: &str,
        summary: &str,
        keep_last_n: usize,
    ) -> FileStoreResult<()> {
        let entries = self.load_session(session_id).await?;

        // Find turns to keep
        let assistant_indices: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| if e.is_assistant() { Some(i) } else { None })
            .collect();

        if assistant_indices.len() <= keep_last_n {
            // Nothing to compact
            return Ok(());
        }

        // Find the index of the first entry to keep
        let keep_from_turn = assistant_indices.len() - keep_last_n;
        let keep_from_index = assistant_indices[keep_from_turn];

        // Create compaction entry
        let compaction_entry = CompactionEntry {
            entry_type: "compaction".to_string(),
            uuid: generate_uuid(),
            timestamp: timestamp(),
            session_id: session_id.to_string(),
            summary: summary.to_string(),
            compacted_turns: keep_from_turn as u32,
            first_kept_index: keep_from_index,
        };

        // Rewrite session file
        let file_path = self.session_path(session_id);
        let temp_path = file_path.with_extension("jsonl.tmp");

        {
            let file = File::create(&temp_path).await?;
            let mut writer = BufWriter::new(file);

            // Write session start
            if let Some(start) = entries.first() {
                let json = serde_json::to_string(start)?;
                writer.write_all(json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }

            // Write compaction entry
            let compaction_json = serde_json::to_string(&compaction_entry)?;
            writer.write_all(compaction_json.as_bytes()).await?;
            writer.write_all(b"\n").await?;

            // Write kept entries (skip session start, start from keep_from_index)
            for entry in entries.iter().skip(keep_from_index) {
                if entry.is_session_start() {
                    continue;
                }
                let json = serde_json::to_string(entry)?;
                writer.write_all(json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }

            writer.flush().await?;
        }

        // Atomic rename
        fs::rename(&temp_path, &file_path).await?;

        Ok(())
    }

    /// Append a single entry to a session file
    async fn append_entry(&self, path: &Path, entry: &SessionEntry) -> FileStoreResult<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;

        let mut writer = BufWriter::new(file);
        let json = serde_json::to_string(entry)?;
        writer.write_all(json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        Ok(())
    }
}

/// Compaction entry for summarized history
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompactionEntry {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub uuid: String,
    pub timestamp: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub summary: String,
    #[serde(rename = "compactedTurns")]
    pub compacted_turns: u32,
    #[serde(rename = "firstKeptIndex")]
    pub first_kept_index: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, FileSessionStore) {
        let temp_dir = TempDir::new().unwrap();
        let store = FileSessionStore::new(temp_dir.path());
        store.init().await.unwrap();
        (temp_dir, store)
    }

    #[tokio::test]
    async fn test_start_session() {
        let (_temp, store) = setup().await;

        let session = store
            .start_session(StartSessionOptions {
                task_id: Some("test-task".to_string()),
                model: Some("claude-sonnet".to_string()),
                cwd: Some("/test".to_string()),
                ..Default::default()
            })
            .await
            .unwrap();

        assert!(session.session_id.starts_with("session-"));
        assert_eq!(session.task_id, Some("test-task".to_string()));
    }

    #[tokio::test]
    async fn test_log_messages() {
        let (_temp, store) = setup().await;

        let mut session = store
            .start_session(StartSessionOptions::default())
            .await
            .unwrap();

        store
            .log_user_message(
                &mut session,
                MessageContent::Text("Hello".to_string()),
                None,
            )
            .await
            .unwrap();

        store
            .log_assistant_message(
                &mut session,
                MessageContent::Text("Hi there!".to_string()),
                LogAssistantOptions {
                    usage: Some(UsageMetrics {
                        input_tokens: Some(10),
                        output_tokens: Some(5),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(session.turn_count, 1);
        assert_eq!(session.cumulative_usage.input_tokens, Some(10));

        let entries = store.load_session(&session.session_id).await.unwrap();
        assert_eq!(entries.len(), 3); // start + user + assistant
    }

    #[tokio::test]
    async fn test_end_session() {
        let (_temp, store) = setup().await;

        let mut session = store
            .start_session(StartSessionOptions::default())
            .await
            .unwrap();

        session.track_file_modified("test.rs");

        store
            .end_session(&session, SessionOutcome::Success, EndSessionOptions::default())
            .await
            .unwrap();

        let entries = store.load_session(&session.session_id).await.unwrap();
        assert!(entries.last().unwrap().is_session_end());
    }

    #[tokio::test]
    async fn test_resume_session() {
        let (_temp, store) = setup().await;

        let mut session = store
            .start_session(StartSessionOptions {
                task_id: Some("resume-test".to_string()),
                ..Default::default()
            })
            .await
            .unwrap();

        store
            .log_user_message(&mut session, MessageContent::Text("Hello".to_string()), None)
            .await
            .unwrap();

        store
            .log_assistant_message(
                &mut session,
                MessageContent::Text("Hi!".to_string()),
                LogAssistantOptions {
                    usage: Some(UsageMetrics {
                        input_tokens: Some(100),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let session_id = session.session_id.clone();

        // Resume the session
        let resumed = store.resume_session(&session_id).await.unwrap();
        assert_eq!(resumed.session_id, session_id);
        assert_eq!(resumed.turn_count, 1);
        assert_eq!(resumed.cumulative_usage.input_tokens, Some(100));
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let (_temp, store) = setup().await;

        for _ in 0..3 {
            let session = store
                .start_session(StartSessionOptions::default())
                .await
                .unwrap();
            store
                .end_session(&session, SessionOutcome::Success, EndSessionOptions::default())
                .await
                .unwrap();
        }

        let sessions = store.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 3);
    }

    #[tokio::test]
    async fn test_compaction() {
        let (_temp, store) = setup().await;

        let mut session = store
            .start_session(StartSessionOptions::default())
            .await
            .unwrap();

        // Add 5 turns
        for i in 0..5 {
            store
                .log_user_message(
                    &mut session,
                    MessageContent::Text(format!("Message {}", i)),
                    None,
                )
                .await
                .unwrap();

            store
                .log_assistant_message(
                    &mut session,
                    MessageContent::Text(format!("Response {}", i)),
                    LogAssistantOptions::default(),
                )
                .await
                .unwrap();
        }

        let entries_before = store.load_session(&session.session_id).await.unwrap();
        assert_eq!(entries_before.len(), 11); // start + 5*(user + assistant)

        // Compact, keep last 2 turns
        store
            .compact_session(&session.session_id, "Summary of earlier messages", 2)
            .await
            .unwrap();

        // After compaction, we should have fewer entries
        // session_start + compaction + 2 turns * 2 messages = 6 entries
        // But we keep all messages from keep_from_index, so we need to count more carefully
        let file_path = store.session_path(&session.session_id);
        let content = fs::read_to_string(&file_path).await.unwrap();
        let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();

        // Should have: start, compaction, then kept entries
        assert!(lines.len() < entries_before.len());
        assert!(content.contains("compaction"));
        assert!(content.contains("Summary of earlier messages"));
    }
}
