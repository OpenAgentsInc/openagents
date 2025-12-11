//! Sessions - Claude Code-compatible session storage
//!
//! Stores full conversation history for replayability, matching Claude Code's
//! `~/.claude/projects/<project-path>/<session-id>.jsonl` format.
//!
//! Each line is a complete JSON object with type, message, uuid, parentUuid, timestamp, sessionId.
//! Messages include full content (text + tool_use with ALL params).
//! Usage metrics with token counts and cache info.
//! Parent UUIDs for threading/conversation structure.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

// ============================================================================
// Usage Metrics
// ============================================================================

/// Usage metrics for tracking costs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageMetrics {
    #[serde(rename = "inputTokens", skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(rename = "outputTokens", skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(rename = "cacheReadInputTokens", skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u64>,
    #[serde(rename = "cacheCreationInputTokens", skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u64>,
    #[serde(rename = "totalCostUsd", skip_serializing_if = "Option::is_none")]
    pub total_cost_usd: Option<f64>,
}

impl UsageMetrics {
    /// Create new empty usage metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add usage from another metrics object.
    pub fn add(&mut self, other: &UsageMetrics) {
        if let Some(v) = other.input_tokens {
            *self.input_tokens.get_or_insert(0) += v;
        }
        if let Some(v) = other.output_tokens {
            *self.output_tokens.get_or_insert(0) += v;
        }
        if let Some(v) = other.cache_read_input_tokens {
            *self.cache_read_input_tokens.get_or_insert(0) += v;
        }
        if let Some(v) = other.cache_creation_input_tokens {
            *self.cache_creation_input_tokens.get_or_insert(0) += v;
        }
        if let Some(v) = other.total_cost_usd {
            *self.total_cost_usd.get_or_insert(0.0) += v;
        }
    }
}

// ============================================================================
// Content Blocks
// ============================================================================

/// Text content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: String,
}

impl TextBlock {
    pub fn new(text: String) -> Self {
        Self {
            block_type: "text".to_string(),
            text,
        }
    }
}

/// Tool use content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

impl ToolUseBlock {
    pub fn new(id: String, name: String, input: serde_json::Value) -> Self {
        Self {
            block_type: "tool_use".to_string(),
            id,
            name,
            input,
        }
    }
}

/// Tool result content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub tool_use_id: String,
    pub content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl ToolResultBlock {
    pub fn new(tool_use_id: String, content: serde_json::Value, is_error: Option<bool>) -> Self {
        Self {
            block_type: "tool_result".to_string(),
            tool_use_id,
            content,
            is_error,
        }
    }
}

/// Content block enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentBlock {
    Text(TextBlock),
    ToolUse(ToolUseBlock),
    ToolResult(ToolResultBlock),
    Unknown(serde_json::Value),
}

/// Message content - can be string or array of content blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl MessageContent {
    /// Extract text from message content.
    pub fn extract_text(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|block| {
                    if let ContentBlock::Text(TextBlock { text, .. }) = block {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// Count tool uses in content.
    pub fn count_tool_uses(&self) -> usize {
        match self {
            MessageContent::Text(_) => 0,
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter(|block| matches!(block, ContentBlock::ToolUse(_)))
                .count(),
        }
    }
}

// ============================================================================
// Session Entries
// ============================================================================

/// Session outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionOutcome {
    Success,
    Failure,
    Blocked,
    Cancelled,
}

/// User message in a session entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub role: String,
    pub content: MessageContent,
}

impl UserMessage {
    pub fn new(content: MessageContent) -> Self {
        Self {
            role: "user".to_string(),
            content,
        }
    }
}

/// Assistant message in a session entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub role: String,
    pub content: MessageContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

impl AssistantMessage {
    pub fn new(content: MessageContent) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
            model: None,
            id: None,
            stop_reason: None,
        }
    }
}

/// Tool result message in a session entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultMessage {
    pub role: String,
    pub content: Vec<ToolResultBlock>,
}

impl ToolResultMessage {
    pub fn new(content: Vec<ToolResultBlock>) -> Self {
        Self {
            role: "user".to_string(),
            content,
        }
    }
}

/// Session entry type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEntryType {
    SessionStart,
    User,
    Assistant,
    ToolResult,
    SessionEnd,
}

/// Session start entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStartEntry {
    #[serde(rename = "type")]
    pub entry_type: SessionEntryType,
    pub uuid: String,
    pub timestamp: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "parentUuid")]
    pub parent_uuid: Option<String>,
    #[serde(rename = "taskId", skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(rename = "gitBranch", skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
}

/// User message entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessageEntry {
    #[serde(rename = "type")]
    pub entry_type: SessionEntryType,
    pub uuid: String,
    pub timestamp: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "parentUuid")]
    pub parent_uuid: Option<String>,
    pub message: UserMessage,
    #[serde(rename = "userType", skip_serializing_if = "Option::is_none")]
    pub user_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// Assistant message entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessageEntry {
    #[serde(rename = "type")]
    pub entry_type: SessionEntryType,
    pub uuid: String,
    pub timestamp: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "parentUuid")]
    pub parent_uuid: Option<String>,
    pub message: AssistantMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageMetrics>,
    #[serde(rename = "requestId", skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// Tool result entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultEntry {
    #[serde(rename = "type")]
    pub entry_type: SessionEntryType,
    pub uuid: String,
    pub timestamp: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "parentUuid")]
    pub parent_uuid: Option<String>,
    pub message: ToolResultMessage,
    #[serde(rename = "toolUseResult", skip_serializing_if = "Option::is_none")]
    pub tool_use_result: Option<serde_json::Value>,
}

/// Session end entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEndEntry {
    #[serde(rename = "type")]
    pub entry_type: SessionEntryType,
    pub uuid: String,
    pub timestamp: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "parentUuid")]
    pub parent_uuid: Option<String>,
    pub outcome: SessionOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(rename = "totalTurns")]
    pub total_turns: u32,
    #[serde(rename = "totalUsage", skip_serializing_if = "Option::is_none")]
    pub total_usage: Option<UsageMetrics>,
    #[serde(rename = "filesModified", skip_serializing_if = "Option::is_none")]
    pub files_modified: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commits: Option<Vec<String>>,
}

/// Union of all session entry types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEntry {
    SessionStart {
        uuid: String,
        timestamp: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "parentUuid")]
        parent_uuid: Option<String>,
        #[serde(rename = "taskId", skip_serializing_if = "Option::is_none")]
        task_id: Option<String>,
        cwd: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        provider: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        #[serde(rename = "gitBranch", skip_serializing_if = "Option::is_none")]
        git_branch: Option<String>,
    },
    User {
        uuid: String,
        timestamp: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "parentUuid")]
        parent_uuid: Option<String>,
        message: UserMessage,
        #[serde(rename = "userType", skip_serializing_if = "Option::is_none")]
        user_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },
    Assistant {
        uuid: String,
        timestamp: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "parentUuid")]
        parent_uuid: Option<String>,
        message: AssistantMessage,
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<UsageMetrics>,
        #[serde(rename = "requestId", skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
    ToolResult {
        uuid: String,
        timestamp: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "parentUuid")]
        parent_uuid: Option<String>,
        message: ToolResultMessage,
        #[serde(rename = "toolUseResult", skip_serializing_if = "Option::is_none")]
        tool_use_result: Option<serde_json::Value>,
    },
    SessionEnd {
        uuid: String,
        timestamp: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "parentUuid")]
        parent_uuid: Option<String>,
        outcome: SessionOutcome,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        #[serde(rename = "totalTurns")]
        total_turns: u32,
        #[serde(rename = "totalUsage", skip_serializing_if = "Option::is_none")]
        total_usage: Option<UsageMetrics>,
        #[serde(rename = "filesModified", skip_serializing_if = "Option::is_none")]
        files_modified: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        commits: Option<Vec<String>>,
    },
}

impl SessionEntry {
    /// Get the UUID of this entry.
    pub fn uuid(&self) -> &str {
        match self {
            SessionEntry::SessionStart { uuid, .. } => uuid,
            SessionEntry::User { uuid, .. } => uuid,
            SessionEntry::Assistant { uuid, .. } => uuid,
            SessionEntry::ToolResult { uuid, .. } => uuid,
            SessionEntry::SessionEnd { uuid, .. } => uuid,
        }
    }

    /// Get the session ID of this entry.
    pub fn session_id(&self) -> &str {
        match self {
            SessionEntry::SessionStart { session_id, .. } => session_id,
            SessionEntry::User { session_id, .. } => session_id,
            SessionEntry::Assistant { session_id, .. } => session_id,
            SessionEntry::ToolResult { session_id, .. } => session_id,
            SessionEntry::SessionEnd { session_id, .. } => session_id,
        }
    }

    /// Get the timestamp of this entry.
    pub fn timestamp(&self) -> &str {
        match self {
            SessionEntry::SessionStart { timestamp, .. } => timestamp,
            SessionEntry::User { timestamp, .. } => timestamp,
            SessionEntry::Assistant { timestamp, .. } => timestamp,
            SessionEntry::ToolResult { timestamp, .. } => timestamp,
            SessionEntry::SessionEnd { timestamp, .. } => timestamp,
        }
    }

    /// Check if this is a session start entry.
    pub fn is_session_start(&self) -> bool {
        matches!(self, SessionEntry::SessionStart { .. })
    }

    /// Check if this is a session end entry.
    pub fn is_session_end(&self) -> bool {
        matches!(self, SessionEntry::SessionEnd { .. })
    }

    /// Check if this is a user entry.
    pub fn is_user(&self) -> bool {
        matches!(self, SessionEntry::User { .. })
    }

    /// Check if this is an assistant entry.
    pub fn is_assistant(&self) -> bool {
        matches!(self, SessionEntry::Assistant { .. })
    }
}

// ============================================================================
// Session Metadata
// ============================================================================

/// Session metadata for quick lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "taskId", skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(rename = "startedAt")]
    pub started_at: String,
    #[serde(rename = "endedAt", skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<SessionOutcome>,
    #[serde(rename = "totalTurns")]
    pub total_turns: u32,
    #[serde(rename = "totalUsage", skip_serializing_if = "Option::is_none")]
    pub total_usage: Option<UsageMetrics>,
    #[serde(rename = "filesModified", skip_serializing_if = "Option::is_none")]
    pub files_modified: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commits: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub cwd: String,
    #[serde(rename = "firstUserMessage", skip_serializing_if = "Option::is_none")]
    pub first_user_message: Option<String>,
}

impl SessionMetadata {
    /// Create metadata from session entries.
    pub fn from_entries(session_id: String, entries: &[SessionEntry]) -> Option<Self> {
        let start_entry = entries.iter().find(|e| e.is_session_start())?;
        let end_entry = entries.iter().find(|e| e.is_session_end());
        let first_user = entries.iter().find(|e| e.is_user());

        let (task_id, started_at, model, cwd) = match start_entry {
            SessionEntry::SessionStart {
                task_id,
                timestamp,
                model,
                cwd,
                ..
            } => (task_id.clone(), timestamp.clone(), model.clone(), cwd.clone()),
            _ => return None,
        };

        let (ended_at, outcome, total_turns_from_end, total_usage, files_modified, commits) =
            match end_entry {
                Some(SessionEntry::SessionEnd {
                    timestamp,
                    outcome,
                    total_turns,
                    total_usage,
                    files_modified,
                    commits,
                    ..
                }) => (
                    Some(timestamp.clone()),
                    Some(outcome.clone()),
                    Some(*total_turns),
                    total_usage.clone(),
                    files_modified.clone(),
                    commits.clone(),
                ),
                _ => (None, None, None, None, None, None),
            };

        let first_user_message = match first_user {
            Some(SessionEntry::User { message, .. }) => Some(message.content.extract_text()),
            _ => None,
        };

        let total_turns = total_turns_from_end.unwrap_or_else(|| {
            entries.iter().filter(|e| e.is_assistant()).count() as u32
        });

        Some(Self {
            session_id,
            task_id,
            started_at,
            ended_at,
            outcome,
            total_turns,
            total_usage,
            files_modified,
            commits,
            model,
            cwd,
            first_user_message,
        })
    }
}

// ============================================================================
// Active Session
// ============================================================================

/// An active session being recorded.
#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub session_id: String,
    pub file_path: String,
    pub task_id: Option<String>,
    pub started_at: String,
    pub last_uuid: Option<String>,
    pub turn_count: u32,
    pub cumulative_usage: UsageMetrics,
    pub files_modified: HashSet<String>,
}

impl ActiveSession {
    /// Create a new active session.
    pub fn new(session_id: String, file_path: String, task_id: Option<String>) -> Self {
        Self {
            session_id,
            file_path,
            task_id,
            started_at: timestamp(),
            last_uuid: None,
            turn_count: 0,
            cumulative_usage: UsageMetrics::new(),
            files_modified: HashSet::new(),
        }
    }

    /// Track a modified file.
    pub fn track_file_modified(&mut self, file_path: &str) {
        self.files_modified.insert(file_path.to_string());
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a UUID.
pub fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

/// Generate a session ID.
pub fn generate_session_id() -> String {
    let now = Utc::now();
    let iso = now.format("%Y-%m-%dT%H-%M-%S").to_string();
    let random = base36_random(6);
    format!("session-{}-{}", iso, random)
}

/// Get current timestamp in ISO format.
pub fn timestamp() -> String {
    Utc::now().to_rfc3339()
}

/// Generate random base36 string.
fn base36_random(len: usize) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos() as u64;

    let mut seed = nanos;
    let mut result = Vec::with_capacity(len);
    for _ in 0..len {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        result.push(CHARS[(seed % 36) as usize]);
    }
    String::from_utf8(result).unwrap()
}

/// Extract text from message content (standalone function).
pub fn extract_text(content: &MessageContent) -> String {
    content.extract_text()
}

/// Count tool uses in message content (standalone function).
pub fn count_tool_uses(content: &MessageContent) -> usize {
    content.count_tool_uses()
}

// ============================================================================
// Session Store
// ============================================================================

/// Error type for session store operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStoreError {
    NotFound(String),
    ParseError(String),
    WriteError(String),
    InvalidState(String),
}

impl std::fmt::Display for SessionStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionStoreError::NotFound(msg) => write!(f, "Not found: {}", msg),
            SessionStoreError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            SessionStoreError::WriteError(msg) => write!(f, "Write error: {}", msg),
            SessionStoreError::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
        }
    }
}

impl std::error::Error for SessionStoreError {}

/// In-memory session store.
#[derive(Debug, Default)]
pub struct SessionStore {
    sessions: HashMap<String, Vec<SessionEntry>>,
}

impl SessionStore {
    /// Create a new empty session store.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Start a new session.
    pub fn start_session(&mut self, options: StartSessionOptions) -> ActiveSession {
        let session_id = options.session_id.unwrap_or_else(generate_session_id);
        let uuid = generate_uuid();
        let now = timestamp();

        let entry = SessionEntry::SessionStart {
            uuid: uuid.clone(),
            timestamp: now.clone(),
            session_id: session_id.clone(),
            parent_uuid: None,
            task_id: options.task_id.clone(),
            cwd: options.cwd.unwrap_or_else(|| ".".to_string()),
            model: options.model,
            provider: options.provider,
            version: Some("1.0.0".to_string()),
            git_branch: options.git_branch,
        };

        self.sessions
            .entry(session_id.clone())
            .or_default()
            .push(entry);

        ActiveSession {
            session_id: session_id.clone(),
            file_path: format!("{}.jsonl", session_id),
            task_id: options.task_id,
            started_at: now,
            last_uuid: Some(uuid),
            turn_count: 0,
            cumulative_usage: UsageMetrics::new(),
            files_modified: HashSet::new(),
        }
    }

    /// Log a user message.
    pub fn log_user_message(
        &mut self,
        session: &mut ActiveSession,
        content: MessageContent,
        user_type: Option<String>,
    ) {
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

        self.sessions
            .entry(session.session_id.clone())
            .or_default()
            .push(entry);

        session.last_uuid = Some(uuid);
    }

    /// Log an assistant message.
    pub fn log_assistant_message(
        &mut self,
        session: &mut ActiveSession,
        content: MessageContent,
        options: LogAssistantOptions,
    ) {
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

        self.sessions
            .entry(session.session_id.clone())
            .or_default()
            .push(entry);

        if let Some(usage) = &options.usage {
            session.cumulative_usage.add(usage);
        }

        session.last_uuid = Some(uuid);
        session.turn_count += 1;
    }

    /// Log a tool result.
    pub fn log_tool_result(
        &mut self,
        session: &mut ActiveSession,
        tool_use_id: String,
        result: serde_json::Value,
        is_error: Option<bool>,
    ) {
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

        self.sessions
            .entry(session.session_id.clone())
            .or_default()
            .push(entry);

        session.last_uuid = Some(uuid);
    }

    /// End a session.
    pub fn end_session(
        &mut self,
        session: &ActiveSession,
        outcome: SessionOutcome,
        options: EndSessionOptions,
    ) {
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

        self.sessions
            .entry(session.session_id.clone())
            .or_default()
            .push(entry);
    }

    /// Load a session by ID.
    pub fn load_session(&self, session_id: &str) -> Option<&Vec<SessionEntry>> {
        self.sessions.get(session_id)
    }

    /// List all session IDs.
    pub fn list_sessions(&self) -> Vec<&String> {
        let mut ids: Vec<_> = self.sessions.keys().collect();
        ids.sort();
        ids.reverse();
        ids
    }

    /// Get session metadata.
    pub fn get_session_metadata(&self, session_id: &str) -> Option<SessionMetadata> {
        let entries = self.sessions.get(session_id)?;
        SessionMetadata::from_entries(session_id.to_string(), entries)
    }

    /// Search sessions by text content.
    pub fn search_sessions(&self, term: &str) -> Vec<SessionMetadata> {
        let lower_term = term.to_lowercase();
        let mut results = Vec::new();

        for (session_id, entries) in &self.sessions {
            let mut found = false;
            for entry in entries {
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
                if let Some(metadata) = SessionMetadata::from_entries(session_id.clone(), entries) {
                    results.push(metadata);
                }
            }
        }

        results
    }

    /// Find sessions by task ID.
    pub fn find_sessions_by_task(&self, task_id: &str) -> Vec<SessionMetadata> {
        let mut results = Vec::new();

        for (session_id, entries) in &self.sessions {
            if let Some(metadata) = SessionMetadata::from_entries(session_id.clone(), entries) {
                if metadata.task_id.as_deref() == Some(task_id) {
                    results.push(metadata);
                }
            }
        }

        results
    }

    /// Get session count.
    pub fn count(&self) -> usize {
        self.sessions.len()
    }
}

/// Options for starting a session.
#[derive(Debug, Clone, Default)]
pub struct StartSessionOptions {
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
}

/// Options for logging an assistant message.
#[derive(Debug, Clone, Default)]
pub struct LogAssistantOptions {
    pub model: Option<String>,
    pub message_id: Option<String>,
    pub usage: Option<UsageMetrics>,
    pub request_id: Option<String>,
    pub stop_reason: Option<String>,
}

/// Options for ending a session.
#[derive(Debug, Clone, Default)]
pub struct EndSessionOptions {
    pub reason: Option<String>,
    pub commits: Option<Vec<String>>,
}

/// Default sessions directory.
pub const DEFAULT_SESSIONS_DIR: &str = ".openagents/sessions";

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_uuid() {
        let uuid = generate_uuid();
        assert_eq!(uuid.len(), 36);
        assert!(uuid.contains('-'));
    }

    #[test]
    fn test_generate_session_id() {
        let id = generate_session_id();
        assert!(id.starts_with("session-"));
    }

    #[test]
    fn test_timestamp() {
        let ts = timestamp();
        assert!(ts.contains("T"));
        assert!(ts.contains("-"));
    }

    #[test]
    fn test_usage_metrics_add() {
        let mut m1 = UsageMetrics::new();
        m1.input_tokens = Some(100);
        m1.output_tokens = Some(50);

        let m2 = UsageMetrics {
            input_tokens: Some(200),
            output_tokens: Some(100),
            total_cost_usd: Some(0.01),
            ..Default::default()
        };

        m1.add(&m2);
        assert_eq!(m1.input_tokens, Some(300));
        assert_eq!(m1.output_tokens, Some(150));
        assert_eq!(m1.total_cost_usd, Some(0.01));
    }

    #[test]
    fn test_message_content_extract_text() {
        let text_content = MessageContent::Text("Hello, world!".to_string());
        assert_eq!(text_content.extract_text(), "Hello, world!");

        let block_content = MessageContent::Blocks(vec![
            ContentBlock::Text(TextBlock::new("First".to_string())),
            ContentBlock::Text(TextBlock::new("Second".to_string())),
        ]);
        assert_eq!(block_content.extract_text(), "First\nSecond");
    }

    #[test]
    fn test_message_content_count_tool_uses() {
        let text_content = MessageContent::Text("No tools".to_string());
        assert_eq!(text_content.count_tool_uses(), 0);

        let block_content = MessageContent::Blocks(vec![
            ContentBlock::Text(TextBlock::new("Text".to_string())),
            ContentBlock::ToolUse(ToolUseBlock::new(
                "id1".to_string(),
                "bash".to_string(),
                serde_json::json!({"command": "ls"}),
            )),
            ContentBlock::ToolUse(ToolUseBlock::new(
                "id2".to_string(),
                "read".to_string(),
                serde_json::json!({"path": "/tmp"}),
            )),
        ]);
        assert_eq!(block_content.count_tool_uses(), 2);
    }

    #[test]
    fn test_session_store_basic() {
        let mut store = SessionStore::new();
        assert_eq!(store.count(), 0);

        let session = store.start_session(StartSessionOptions {
            task_id: Some("task-1".to_string()),
            model: Some("fm".to_string()),
            ..Default::default()
        });

        assert!(session.session_id.starts_with("session-"));
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn test_session_store_log_messages() {
        let mut store = SessionStore::new();

        let mut session = store.start_session(StartSessionOptions::default());

        store.log_user_message(
            &mut session,
            MessageContent::Text("Hello".to_string()),
            None,
        );

        store.log_assistant_message(
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
        );

        assert_eq!(session.turn_count, 1);
        assert_eq!(session.cumulative_usage.input_tokens, Some(10));

        let entries = store.load_session(&session.session_id).unwrap();
        assert_eq!(entries.len(), 3); // start + user + assistant
    }

    #[test]
    fn test_session_store_tool_result() {
        let mut store = SessionStore::new();
        let mut session = store.start_session(StartSessionOptions::default());

        store.log_tool_result(
            &mut session,
            "tool-use-123".to_string(),
            serde_json::json!({"output": "success"}),
            None,
        );

        let entries = store.load_session(&session.session_id).unwrap();
        assert_eq!(entries.len(), 2); // start + tool_result
    }

    #[test]
    fn test_session_store_end_session() {
        let mut store = SessionStore::new();
        let mut session = store.start_session(StartSessionOptions {
            task_id: Some("test-task".to_string()),
            ..Default::default()
        });

        session.track_file_modified("src/lib.rs");
        session.track_file_modified("Cargo.toml");

        store.log_assistant_message(
            &mut session,
            MessageContent::Text("Done!".to_string()),
            LogAssistantOptions::default(),
        );

        store.end_session(
            &session,
            SessionOutcome::Success,
            EndSessionOptions {
                commits: Some(vec!["abc123".to_string()]),
                ..Default::default()
            },
        );

        let entries = store.load_session(&session.session_id).unwrap();
        let end_entry = entries.last().unwrap();

        match end_entry {
            SessionEntry::SessionEnd {
                outcome,
                files_modified,
                commits,
                ..
            } => {
                assert_eq!(*outcome, SessionOutcome::Success);
                assert_eq!(files_modified.as_ref().unwrap().len(), 2);
                assert_eq!(commits.as_ref().unwrap(), &vec!["abc123".to_string()]);
            }
            _ => panic!("Expected SessionEnd entry"),
        }
    }

    #[test]
    fn test_session_metadata() {
        let mut store = SessionStore::new();
        let mut session = store.start_session(StartSessionOptions {
            task_id: Some("meta-test".to_string()),
            model: Some("fm".to_string()),
            cwd: Some("/test".to_string()),
            ..Default::default()
        });

        store.log_user_message(
            &mut session,
            MessageContent::Text("First message".to_string()),
            None,
        );

        store.log_assistant_message(
            &mut session,
            MessageContent::Text("Response".to_string()),
            LogAssistantOptions::default(),
        );

        store.end_session(&session, SessionOutcome::Success, EndSessionOptions::default());

        let metadata = store.get_session_metadata(&session.session_id).unwrap();
        assert_eq!(metadata.task_id, Some("meta-test".to_string()));
        assert_eq!(metadata.model, Some("fm".to_string()));
        assert_eq!(metadata.cwd, "/test");
        assert_eq!(metadata.outcome, Some(SessionOutcome::Success));
        assert_eq!(metadata.first_user_message, Some("First message".to_string()));
    }

    #[test]
    fn test_search_sessions() {
        let mut store = SessionStore::new();

        let mut s1 = store.start_session(StartSessionOptions::default());
        store.log_user_message(
            &mut s1,
            MessageContent::Text("Fix the authentication bug".to_string()),
            None,
        );
        store.end_session(&s1, SessionOutcome::Success, EndSessionOptions::default());

        let mut s2 = store.start_session(StartSessionOptions::default());
        store.log_user_message(
            &mut s2,
            MessageContent::Text("Add new feature".to_string()),
            None,
        );
        store.end_session(&s2, SessionOutcome::Success, EndSessionOptions::default());

        let results = store.search_sessions("authentication");
        assert_eq!(results.len(), 1);

        let results = store.search_sessions("fix");
        assert_eq!(results.len(), 1);

        let results = store.search_sessions("something else");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_find_sessions_by_task() {
        let mut store = SessionStore::new();

        for i in 0..3 {
            let task_id = if i < 2 {
                Some("shared-task".to_string())
            } else {
                Some("other-task".to_string())
            };
            let session = store.start_session(StartSessionOptions {
                task_id,
                ..Default::default()
            });
            store.end_session(&session, SessionOutcome::Success, EndSessionOptions::default());
        }

        let results = store.find_sessions_by_task("shared-task");
        assert_eq!(results.len(), 2);

        let results = store.find_sessions_by_task("other-task");
        assert_eq!(results.len(), 1);

        let results = store.find_sessions_by_task("nonexistent");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_session_entry_accessors() {
        let entry = SessionEntry::User {
            uuid: "test-uuid".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            session_id: "test-session".to_string(),
            parent_uuid: None,
            message: UserMessage::new(MessageContent::Text("test".to_string())),
            user_type: None,
            cwd: None,
        };

        assert_eq!(entry.uuid(), "test-uuid");
        assert_eq!(entry.session_id(), "test-session");
        assert_eq!(entry.timestamp(), "2024-01-01T00:00:00Z");
        assert!(entry.is_user());
        assert!(!entry.is_assistant());
        assert!(!entry.is_session_start());
        assert!(!entry.is_session_end());
    }

    #[test]
    fn test_session_outcome_serialization() {
        let json = serde_json::to_string(&SessionOutcome::Success).unwrap();
        assert_eq!(json, "\"success\"");

        let json = serde_json::to_string(&SessionOutcome::Failure).unwrap();
        assert_eq!(json, "\"failure\"");

        let json = serde_json::to_string(&SessionOutcome::Blocked).unwrap();
        assert_eq!(json, "\"blocked\"");

        let json = serde_json::to_string(&SessionOutcome::Cancelled).unwrap();
        assert_eq!(json, "\"cancelled\"");
    }

    #[test]
    fn test_text_block() {
        let block = TextBlock::new("Hello".to_string());
        assert_eq!(block.block_type, "text");
        assert_eq!(block.text, "Hello");

        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"text\""));
    }

    #[test]
    fn test_tool_use_block() {
        let block = ToolUseBlock::new(
            "tool-123".to_string(),
            "bash".to_string(),
            serde_json::json!({"command": "ls"}),
        );
        assert_eq!(block.block_type, "tool_use");
        assert_eq!(block.id, "tool-123");
        assert_eq!(block.name, "bash");
    }

    #[test]
    fn test_tool_result_block() {
        let block = ToolResultBlock::new(
            "tool-123".to_string(),
            serde_json::json!("output"),
            Some(false),
        );
        assert_eq!(block.block_type, "tool_result");
        assert_eq!(block.tool_use_id, "tool-123");
        assert_eq!(block.is_error, Some(false));
    }

    #[test]
    fn test_active_session() {
        let mut session = ActiveSession::new(
            "test-session".to_string(),
            "test-session.jsonl".to_string(),
            Some("task-1".to_string()),
        );

        assert_eq!(session.turn_count, 0);
        assert!(session.files_modified.is_empty());

        session.track_file_modified("file1.rs");
        session.track_file_modified("file2.rs");
        session.track_file_modified("file1.rs"); // Duplicate

        assert_eq!(session.files_modified.len(), 2);
    }
}
