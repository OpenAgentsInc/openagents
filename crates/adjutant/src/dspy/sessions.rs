//! Session tracking for autopilot runs.
//!
//! Records all decisions made during task execution and links them
//! to final outcomes, enabling the self-improving feedback loop.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// Unique identifier for an autopilot session.
pub type SessionId = String;

/// A single decision made during an autopilot session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    /// Type of decision: complexity, delegation, rlm_trigger
    pub decision_type: String,
    /// Summary of the input provided to the decision pipeline
    pub input_summary: String,
    /// The output from the decision pipeline
    pub output: serde_json::Value,
    /// Confidence at prediction time (0.0 - 1.0)
    pub predicted_confidence: f32,
    /// Timestamp when decision was made
    pub timestamp: DateTime<Utc>,
    /// Retrospective: was this decision correct? (set after outcome)
    pub was_correct: Option<bool>,
}

impl DecisionRecord {
    /// Create a new decision record.
    pub fn new(
        decision_type: impl Into<String>,
        input_summary: impl Into<String>,
        output: serde_json::Value,
        predicted_confidence: f32,
    ) -> Self {
        Self {
            decision_type: decision_type.into(),
            input_summary: input_summary.into(),
            output,
            predicted_confidence,
            timestamp: Utc::now(),
            was_correct: None,
        }
    }
}

/// Record of a verification attempt during the autopilot loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRecord {
    /// Which iteration this verification was for
    pub iteration: usize,
    /// Did verification pass?
    pub passed: bool,
    /// Result of cargo check (if run)
    pub cargo_check: Option<bool>,
    /// Result of cargo test (if run)
    pub cargo_test: Option<bool>,
    /// Reason for pass/fail
    pub reason: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl VerificationRecord {
    /// Create a new verification record.
    pub fn new(iteration: usize, passed: bool, reason: impl Into<String>) -> Self {
        Self {
            iteration,
            passed,
            cargo_check: None,
            cargo_test: None,
            reason: reason.into(),
            timestamp: Utc::now(),
        }
    }

    /// Set cargo check result.
    pub fn with_cargo_check(mut self, passed: bool) -> Self {
        self.cargo_check = Some(passed);
        self
    }

    /// Set cargo test result.
    pub fn with_cargo_test(mut self, passed: bool) -> Self {
        self.cargo_test = Some(passed);
        self
    }
}

/// Outcome of an autopilot session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SessionOutcome {
    /// Task completed successfully with verification
    Success {
        summary: String,
        modified_files: Vec<String>,
        verification_passed: bool,
    },
    /// Task failed definitively
    Failed {
        reason: String,
        error: Option<String>,
    },
    /// Reached max iterations without success
    MaxIterationsReached { last_summary: Option<String> },
    /// User interrupted the task
    UserInterrupted,
    /// System error occurred
    Error(String),
}

impl SessionOutcome {
    /// Check if this outcome represents success.
    pub fn is_success(&self) -> bool {
        matches!(self, SessionOutcome::Success { .. })
    }

    /// Check if this outcome represents a definitive failure.
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            SessionOutcome::Failed { .. } | SessionOutcome::MaxIterationsReached { .. }
        )
    }
}

/// Record of a complete autopilot session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotSession {
    /// Unique session ID (UUID)
    pub id: SessionId,
    /// Task ID (e.g., issue number or "adhoc")
    pub task_id: String,
    /// Task title
    pub task_title: String,
    /// Task description
    pub task_description: String,
    /// When the session started
    pub started_at: DateTime<Utc>,
    /// When the session ended (if completed)
    pub ended_at: Option<DateTime<Utc>>,
    /// All decisions made during this session
    pub decisions: Vec<DecisionRecord>,
    /// Outcome of the session
    pub outcome: Option<SessionOutcome>,
    /// Total iterations used
    pub iterations_used: usize,
    /// Verification results from each iteration
    pub verification_history: Vec<VerificationRecord>,
}

impl AutopilotSession {
    /// Create a new session for a task.
    pub fn new(
        task_id: impl Into<String>,
        task_title: impl Into<String>,
        task_description: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            task_id: task_id.into(),
            task_title: task_title.into(),
            task_description: task_description.into(),
            started_at: Utc::now(),
            ended_at: None,
            decisions: Vec::new(),
            outcome: None,
            iterations_used: 0,
            verification_history: Vec::new(),
        }
    }

    /// Record a decision made during this session.
    pub fn record_decision(&mut self, decision: DecisionRecord) {
        self.decisions.push(decision);
    }

    /// Record a verification result.
    pub fn record_verification(&mut self, verification: VerificationRecord) {
        self.verification_history.push(verification);
    }

    /// Complete the session with an outcome.
    pub fn complete(&mut self, outcome: SessionOutcome, iterations: usize) {
        self.ended_at = Some(Utc::now());
        self.outcome = Some(outcome);
        self.iterations_used = iterations;
    }

    /// Get duration of the session in seconds.
    pub fn duration_secs(&self) -> Option<i64> {
        self.ended_at
            .map(|end| (end - self.started_at).num_seconds())
    }

    /// Get decisions of a specific type.
    pub fn decisions_of_type(&self, decision_type: &str) -> Vec<&DecisionRecord> {
        self.decisions
            .iter()
            .filter(|d| d.decision_type == decision_type)
            .collect()
    }
}

/// Index of sessions for quick access.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionIndex {
    /// Total sessions recorded
    pub total_sessions: usize,
    /// Sessions that succeeded
    pub success_count: usize,
    /// Sessions that failed
    pub failed_count: usize,
    /// Sessions that were interrupted
    pub interrupted_count: usize,
    /// Recent session IDs (last 100)
    pub recent_sessions: VecDeque<SessionId>,
    /// Last optimization timestamp
    pub last_optimization: Option<DateTime<Utc>>,
    /// Last updated
    pub updated_at: DateTime<Utc>,
}

impl SessionIndex {
    /// Maximum recent sessions to track.
    const MAX_RECENT: usize = 100;

    /// Create a new empty index.
    pub fn new() -> Self {
        Self {
            updated_at: Utc::now(),
            ..Default::default()
        }
    }

    /// Add a session to the index.
    pub fn add_session(&mut self, session_id: &str, outcome: &SessionOutcome) {
        self.total_sessions += 1;

        match outcome {
            SessionOutcome::Success { .. } => self.success_count += 1,
            SessionOutcome::Failed { .. } | SessionOutcome::MaxIterationsReached { .. } => {
                self.failed_count += 1
            }
            SessionOutcome::UserInterrupted => self.interrupted_count += 1,
            SessionOutcome::Error(_) => self.failed_count += 1,
        }

        self.recent_sessions.push_front(session_id.to_string());
        if self.recent_sessions.len() > Self::MAX_RECENT {
            self.recent_sessions.pop_back();
        }

        self.updated_at = Utc::now();
    }

    /// Get success rate.
    pub fn success_rate(&self) -> f32 {
        if self.total_sessions == 0 {
            0.0
        } else {
            self.success_count as f32 / self.total_sessions as f32
        }
    }

    /// Record an optimization run.
    pub fn record_optimization(&mut self) {
        self.last_optimization = Some(Utc::now());
        self.updated_at = Utc::now();
    }
}

/// Persistent storage for autopilot sessions.
pub struct SessionStore {
    /// Base path: ~/.openagents/adjutant/sessions/
    base_path: PathBuf,
    /// In-memory index for fast lookups
    index: SessionIndex,
    /// Current active session (if any)
    active_session: Option<AutopilotSession>,
}

impl SessionStore {
    /// Open or create a session store.
    pub fn open() -> anyhow::Result<Self> {
        let base_path = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("No home directory"))?
            .join(".openagents")
            .join("adjutant")
            .join("sessions");

        fs::create_dir_all(&base_path)?;

        let index_path = base_path.join("index.json");
        let index = if index_path.exists() {
            let content = fs::read_to_string(&index_path)?;
            serde_json::from_str(&content).unwrap_or_else(|_| SessionIndex::new())
        } else {
            SessionIndex::new()
        };

        Ok(Self {
            base_path,
            index,
            active_session: None,
        })
    }

    /// Start a new session.
    pub fn start_session(
        &mut self,
        task_id: impl Into<String>,
        task_title: impl Into<String>,
        task_description: impl Into<String>,
    ) -> SessionId {
        let session = AutopilotSession::new(task_id, task_title, task_description);
        let session_id = session.id.clone();
        self.active_session = Some(session);
        session_id
    }

    /// Get the active session ID.
    pub fn active_session_id(&self) -> Option<&str> {
        self.active_session.as_ref().map(|s| s.id.as_str())
    }

    /// Record a decision in the active session.
    pub fn record_decision(&mut self, decision: DecisionRecord) {
        if let Some(ref mut session) = self.active_session {
            session.record_decision(decision);
        }
    }

    /// Record a verification result in the active session.
    pub fn record_verification(&mut self, verification: VerificationRecord) {
        if let Some(ref mut session) = self.active_session {
            session.record_verification(verification);
        }
    }

    /// Complete the active session with an outcome.
    pub fn complete_session(
        &mut self,
        outcome: SessionOutcome,
        iterations: usize,
    ) -> anyhow::Result<SessionId> {
        let mut session = self
            .active_session
            .take()
            .ok_or_else(|| anyhow::anyhow!("No active session"))?;

        session.complete(outcome.clone(), iterations);

        // Update index
        self.index.add_session(&session.id, &outcome);

        // Save session to disk
        self.save_session(&session)?;

        // Save updated index
        self.save_index()?;

        Ok(session.id)
    }

    /// Save a session to disk.
    fn save_session(&self, session: &AutopilotSession) -> anyhow::Result<()> {
        let date = session.started_at.format("%Y/%m").to_string();
        let dir = self.base_path.join(&date);
        fs::create_dir_all(&dir)?;

        let path = dir.join(format!("{}.json", session.id));
        let content = serde_json::to_string_pretty(session)?;
        fs::write(path, content)?;

        Ok(())
    }

    /// Save the index to disk.
    fn save_index(&self) -> anyhow::Result<()> {
        let path = self.base_path.join("index.json");
        let content = serde_json::to_string_pretty(&self.index)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Get a session by ID.
    pub fn get_session(&self, session_id: &str) -> anyhow::Result<Option<AutopilotSession>> {
        // Check active session first
        if let Some(ref session) = self.active_session {
            if session.id == session_id {
                return Ok(Some(session.clone()));
            }
        }

        // Search in recent sessions
        for recent_id in &self.index.recent_sessions {
            if recent_id == session_id {
                return self.load_session(session_id);
            }
        }

        // Try to load directly
        self.load_session(session_id)
    }

    /// Load a session from disk.
    fn load_session(&self, session_id: &str) -> anyhow::Result<Option<AutopilotSession>> {
        // We need to search through date directories
        // For efficiency, check recent months first
        let now = Utc::now();
        for months_back in 0..12 {
            let date = now - chrono::Duration::days(months_back * 30);
            let date_str = date.format("%Y/%m").to_string();
            let path = self
                .base_path
                .join(&date_str)
                .join(format!("{}.json", session_id));

            if path.exists() {
                let content = fs::read_to_string(&path)?;
                let session: AutopilotSession = serde_json::from_str(&content)?;
                return Ok(Some(session));
            }
        }

        Ok(None)
    }

    /// Get recent sessions.
    pub fn get_recent_sessions(&self, limit: usize) -> anyhow::Result<Vec<AutopilotSession>> {
        let mut sessions = Vec::new();
        let limit = limit.min(self.index.recent_sessions.len());

        for session_id in self.index.recent_sessions.iter().take(limit) {
            if let Some(session) = self.load_session(session_id)? {
                sessions.push(session);
            }
        }

        Ok(sessions)
    }

    /// Get the session index.
    pub fn index(&self) -> &SessionIndex {
        &self.index
    }

    /// Get mutable reference to index.
    pub fn index_mut(&mut self) -> &mut SessionIndex {
        &mut self.index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = AutopilotSession::new("test-1", "Test Task", "A test task description");
        assert!(!session.id.is_empty());
        assert_eq!(session.task_id, "test-1");
        assert!(session.decisions.is_empty());
        assert!(session.outcome.is_none());
    }

    #[test]
    fn test_decision_record() {
        let decision = DecisionRecord::new(
            "complexity",
            "task: fix bug",
            serde_json::json!({"complexity": "Medium", "confidence": 0.85}),
            0.85,
        );
        assert_eq!(decision.decision_type, "complexity");
        assert!(decision.was_correct.is_none());
    }

    #[test]
    fn test_session_index() {
        let mut index = SessionIndex::new();
        assert_eq!(index.success_rate(), 0.0);

        index.add_session(
            "s1",
            &SessionOutcome::Success {
                summary: "done".into(),
                modified_files: vec![],
                verification_passed: true,
            },
        );
        assert_eq!(index.success_count, 1);
        assert_eq!(index.success_rate(), 1.0);

        index.add_session(
            "s2",
            &SessionOutcome::Failed {
                reason: "error".into(),
                error: None,
            },
        );
        assert_eq!(index.failed_count, 1);
        assert_eq!(index.success_rate(), 0.5);
    }

    #[test]
    fn test_verification_record() {
        let verification = VerificationRecord::new(1, true, "All tests passed")
            .with_cargo_check(true)
            .with_cargo_test(true);
        assert!(verification.passed);
        assert_eq!(verification.cargo_check, Some(true));
        assert_eq!(verification.cargo_test, Some(true));
    }
}
