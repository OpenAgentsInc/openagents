//! MechaCoder domain types
//!
//! Types for the MechaCoder screen - a flexible Terminal-Bench solver
//! that supports both FM (Apple Foundation Model) and CC (Claude Code) backends.

use hillclimber::HillClimberBackend;

/// Status of the MechaCoder session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MechaStatus {
    #[default]
    Idle,
    /// Generating tests using TestGen
    GeneratingTests,
    /// Running solver iteration
    Running,
    /// Waiting for user input
    WaitingInput,
    /// Successfully solved the task
    Solved,
    /// Failed to solve
    Failed,
}

impl MechaStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::GeneratingTests => "Generating Tests...",
            Self::Running => "Running...",
            Self::WaitingInput => "Waiting for Input",
            Self::Solved => "Solved!",
            Self::Failed => "Failed",
        }
    }

    pub fn is_busy(&self) -> bool {
        matches!(self, Self::GeneratingTests | Self::Running)
    }
}

/// Task definition for MechaCoder
#[derive(Debug, Clone)]
pub struct MechaTask {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Optional verification command
    pub verification_cmd: Option<String>,
}

impl Default for MechaTask {
    fn default() -> Self {
        Self {
            id: "custom".to_string(),
            name: "Custom Task".to_string(),
            description: String::new(),
            verification_cmd: None,
        }
    }
}

/// MechaCoder session state
#[derive(Debug, Clone, Default)]
pub struct MechaSession {
    /// Current status
    pub status: MechaStatus,
    /// Selected backend
    pub backend: HillClimberBackend,
    /// Current task
    pub task: MechaTask,
    /// Current turn/iteration
    pub turn: u32,
    /// Maximum turns allowed
    pub max_turns: u32,
    /// Tests passing
    pub tests_passed: u32,
    /// Total tests
    pub tests_total: u32,
    /// Best progress achieved (0.0-1.0)
    pub best_progress: f64,
    /// Current solution (if any)
    pub solution: Option<String>,
    /// Error message (if any)
    pub error: Option<String>,
    /// API cost so far (USD)
    pub cost_usd: f64,
    /// ATIF trajectory session ID
    pub session_id: Option<String>,
}

impl MechaSession {
    pub fn new(backend: HillClimberBackend) -> Self {
        Self {
            backend,
            max_turns: 30,
            ..Default::default()
        }
    }

    pub fn progress_percent(&self) -> f64 {
        self.best_progress * 100.0
    }

    pub fn backend_label(&self) -> &'static str {
        match self.backend {
            HillClimberBackend::FM => "FM (Local)",
            HillClimberBackend::CC => "CC (Claude)",
        }
    }
}

/// Type of log entry for streaming display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    /// Info message
    Info,
    /// Progress update
    Progress,
    /// Tool use (CC mode)
    Tool,
    /// Agent thinking/reasoning
    Thinking,
    /// Test result
    TestResult,
    /// Success
    Success,
    /// Error
    Error,
}

impl LogKind {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Info => "i",
            Self::Progress => ">",
            Self::Tool => "T",
            Self::Thinking => "?",
            Self::TestResult => "#",
            Self::Success => "+",
            Self::Error => "!",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Progress => "PROG",
            Self::Tool => "TOOL",
            Self::Thinking => "THINK",
            Self::TestResult => "TEST",
            Self::Success => "OK",
            Self::Error => "ERR",
        }
    }
}

/// A log entry for the streaming display
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub kind: LogKind,
    pub message: String,
    /// Optional details (expanded on click)
    pub details: Option<String>,
}

impl LogEntry {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            kind: LogKind::Info,
            message: message.into(),
            details: None,
        }
    }

    pub fn progress(message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            kind: LogKind::Progress,
            message: message.into(),
            details: None,
        }
    }

    pub fn tool(tool_name: impl Into<String>, details: Option<String>) -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            kind: LogKind::Tool,
            message: tool_name.into(),
            details,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            kind: LogKind::Error,
            message: message.into(),
            details: None,
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            kind: LogKind::Success,
            message: message.into(),
            details: None,
        }
    }
}

/// Events from the MechaCoder runner
#[derive(Debug, Clone)]
pub enum MechaEvent {
    /// Session started
    Started { backend: HillClimberBackend },
    /// Turn started
    TurnStart { turn: u32, max_turns: u32 },
    /// Tool being used (CC mode)
    ToolUse { tool: String, elapsed_secs: f64 },
    /// Verification result
    VerifyResult { passed: u32, total: u32, progress: f64 },
    /// Session completed
    Completed { passed: bool, turns: u32, cost_usd: f64 },
    /// Error occurred
    Error { message: String },
}

/// Built-in Terminal-Bench tasks
pub mod tasks {
    use super::MechaTask;

    pub fn regex_log() -> MechaTask {
        MechaTask {
            id: "regex-log".to_string(),
            name: "Regex Log Parser".to_string(),
            description: r#"Write a regex pattern that captures dates from log lines.

Requirements:
- Match dates in YYYY-MM-DD format
- Only match on lines containing an IPv4 address
- Capture the LAST date if multiple dates exist on a line
- Respect word boundaries (don't match abc2023-10-15)
- Allow February up to 29 days

Output: Write the regex to /app/regex.txt"#.to_string(),
            verification_cmd: Some("pytest -v".to_string()),
        }
    }

    pub fn all_tasks() -> Vec<MechaTask> {
        vec![
            regex_log(),
        ]
    }
}
