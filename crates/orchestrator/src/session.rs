//! Session management for the orchestrator

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Session is running
    Running,
    /// Session is paused
    Paused,
    /// Session completed successfully
    Completed,
    /// Session failed
    Failed,
    /// Session was cancelled
    Cancelled,
}

impl Default for SessionState {
    fn default() -> Self {
        SessionState::Running
    }
}

/// A single orchestrator session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session ID
    pub id: String,
    /// Session state
    pub state: SessionState,
    /// When the session started
    pub started_at: DateTime<Utc>,
    /// When the session ended (if completed)
    pub ended_at: Option<DateTime<Utc>>,
    /// Working directory
    pub working_dir: PathBuf,
    /// Branch being worked on
    pub branch: Option<String>,
    /// Tasks completed this session
    pub tasks_completed: usize,
    /// Tasks failed this session
    pub tasks_failed: usize,
    /// Total tool calls made
    pub tool_calls: usize,
    /// Total tokens used
    pub tokens_used: TokenUsage,
    /// Session configuration
    pub config: SessionConfig,
    /// Last error (if any)
    pub last_error: Option<String>,
    /// Session metadata
    pub metadata: serde_json::Value,
}

/// Token usage tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input tokens
    pub input_tokens: u64,
    /// Output tokens
    pub output_tokens: u64,
    /// Cache write tokens
    pub cache_write_tokens: u64,
    /// Cache read tokens
    pub cache_read_tokens: u64,
}

impl TokenUsage {
    /// Add usage from an LLM response
    pub fn add(&mut self, usage: &llm::Usage) {
        self.input_tokens += usage.input_tokens as u64;
        self.output_tokens += usage.output_tokens as u64;
        self.cache_write_tokens += usage.cache_creation_input_tokens as u64;
        self.cache_read_tokens += usage.cache_read_input_tokens as u64;
    }

    /// Total tokens used
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }

    /// Estimate cost in USD
    pub fn estimate_cost_usd(&self) -> f64 {
        // Claude 3.5 Sonnet pricing
        let input_cost = (self.input_tokens as f64 / 1_000_000.0) * 3.0;
        let output_cost = (self.output_tokens as f64 / 1_000_000.0) * 15.0;
        let cache_write_cost = (self.cache_write_tokens as f64 / 1_000_000.0) * 3.75;
        let cache_read_cost = (self.cache_read_tokens as f64 / 1_000_000.0) * 0.30;
        input_cost + output_cost + cache_write_cost + cache_read_cost
    }
}

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Maximum tasks to complete per session
    pub max_tasks: Option<usize>,
    /// Maximum time to run (in seconds)
    pub max_duration_secs: Option<u64>,
    /// Maximum tokens to use
    pub max_tokens: Option<u64>,
    /// Enable safe mode (no destructive operations)
    pub safe_mode: bool,
    /// Enable dry run (don't actually execute tools)
    pub dry_run: bool,
    /// Model to use
    pub model: String,
    /// Temperature
    pub temperature: Option<f32>,
    /// Auto-commit changes
    pub auto_commit: bool,
    /// Auto-push commits
    pub auto_push: bool,
    /// Verification strictness (0.0 - 1.0)
    pub verification_strictness: f32,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_tasks: None,
            max_duration_secs: None,
            max_tokens: None,
            safe_mode: true,
            dry_run: false,
            model: ai::Model::default().id().to_string(),
            temperature: None,
            auto_commit: true,
            auto_push: false,
            verification_strictness: 0.7,
        }
    }
}

impl Session {
    /// Create a new session
    pub fn new(working_dir: PathBuf, config: SessionConfig) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            state: SessionState::Running,
            started_at: Utc::now(),
            ended_at: None,
            working_dir,
            branch: None,
            tasks_completed: 0,
            tasks_failed: 0,
            tool_calls: 0,
            tokens_used: TokenUsage::default(),
            config,
            last_error: None,
            metadata: serde_json::json!({}),
        }
    }

    /// Set the branch
    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// Check if the session should continue
    pub fn should_continue(&self) -> bool {
        if self.state != SessionState::Running {
            return false;
        }

        // Check max tasks
        if let Some(max) = self.config.max_tasks {
            if self.tasks_completed >= max {
                return false;
            }
        }

        // Check max duration
        if let Some(max_secs) = self.config.max_duration_secs {
            let duration = Utc::now().signed_duration_since(self.started_at);
            if duration.num_seconds() as u64 >= max_secs {
                return false;
            }
        }

        // Check max tokens
        if let Some(max) = self.config.max_tokens {
            if self.tokens_used.total() >= max {
                return false;
            }
        }

        true
    }

    /// Mark a task as completed
    pub fn record_task_completed(&mut self) {
        self.tasks_completed += 1;
    }

    /// Mark a task as failed
    pub fn record_task_failed(&mut self, error: &str) {
        self.tasks_failed += 1;
        self.last_error = Some(error.to_string());
    }

    /// Record a tool call
    pub fn record_tool_call(&mut self) {
        self.tool_calls += 1;
    }

    /// Record token usage
    pub fn record_tokens(&mut self, usage: &llm::Usage) {
        self.tokens_used.add(usage);
    }

    /// Pause the session
    pub fn pause(&mut self) {
        self.state = SessionState::Paused;
    }

    /// Resume the session
    pub fn resume(&mut self) {
        if self.state == SessionState::Paused {
            self.state = SessionState::Running;
        }
    }

    /// Complete the session
    pub fn complete(&mut self) {
        self.state = SessionState::Completed;
        self.ended_at = Some(Utc::now());
    }

    /// Fail the session
    pub fn fail(&mut self, error: &str) {
        self.state = SessionState::Failed;
        self.ended_at = Some(Utc::now());
        self.last_error = Some(error.to_string());
    }

    /// Cancel the session
    pub fn cancel(&mut self) {
        self.state = SessionState::Cancelled;
        self.ended_at = Some(Utc::now());
    }

    /// Get session duration
    pub fn duration_secs(&self) -> u64 {
        let end = self.ended_at.unwrap_or_else(Utc::now);
        end.signed_duration_since(self.started_at).num_seconds() as u64
    }

    /// Get session summary
    pub fn summary(&self) -> SessionSummary {
        SessionSummary {
            id: self.id.clone(),
            state: self.state,
            duration_secs: self.duration_secs(),
            tasks_completed: self.tasks_completed,
            tasks_failed: self.tasks_failed,
            tool_calls: self.tool_calls,
            tokens_used: self.tokens_used.total(),
            estimated_cost_usd: self.tokens_used.estimate_cost_usd(),
        }
    }
}

/// Summary of a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub state: SessionState,
    pub duration_secs: u64,
    pub tasks_completed: usize,
    pub tasks_failed: usize,
    pub tool_calls: usize,
    pub tokens_used: u64,
    pub estimated_cost_usd: f64,
}

/// Session store for persistence
#[async_trait::async_trait]
pub trait SessionStore: Send + Sync {
    /// Save a session
    async fn save(&self, session: &Session) -> crate::OrchestratorResult<()>;

    /// Load a session by ID
    async fn load(&self, id: &str) -> crate::OrchestratorResult<Option<Session>>;

    /// List recent sessions
    async fn list_recent(&self, limit: usize) -> crate::OrchestratorResult<Vec<SessionSummary>>;

    /// Delete a session
    async fn delete(&self, id: &str) -> crate::OrchestratorResult<()>;
}

/// In-memory session store (for testing)
pub struct InMemorySessionStore {
    sessions: tokio::sync::Mutex<std::collections::HashMap<String, Session>>,
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self {
            sessions: tokio::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for InMemorySessionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SessionStore for InMemorySessionStore {
    async fn save(&self, session: &Session) -> crate::OrchestratorResult<()> {
        self.sessions
            .lock()
            .await
            .insert(session.id.clone(), session.clone());
        Ok(())
    }

    async fn load(&self, id: &str) -> crate::OrchestratorResult<Option<Session>> {
        Ok(self.sessions.lock().await.get(id).cloned())
    }

    async fn list_recent(&self, limit: usize) -> crate::OrchestratorResult<Vec<SessionSummary>> {
        let sessions = self.sessions.lock().await;
        let mut summaries: Vec<_> = sessions.values().map(|s| s.summary()).collect();
        summaries.sort_by(|a, b| b.id.cmp(&a.id)); // Sort by ID (which includes timestamp)
        summaries.truncate(limit);
        Ok(summaries)
    }

    async fn delete(&self, id: &str) -> crate::OrchestratorResult<()> {
        self.sessions.lock().await.remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = Session::new(PathBuf::from("/tmp"), SessionConfig::default());
        assert_eq!(session.state, SessionState::Running);
        assert_eq!(session.tasks_completed, 0);
    }

    #[test]
    fn test_should_continue() {
        let mut config = SessionConfig::default();
        config.max_tasks = Some(5);

        let mut session = Session::new(PathBuf::from("/tmp"), config);
        assert!(session.should_continue());

        for _ in 0..5 {
            session.record_task_completed();
        }
        assert!(!session.should_continue());
    }

    #[test]
    fn test_token_usage() {
        let mut usage = TokenUsage::default();
        usage.add(&llm::Usage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        });

        assert_eq!(usage.total(), 1500);
        assert!(usage.estimate_cost_usd() > 0.0);
    }
}
