//! MechaCoder domain types
//!
//! Types for the MechaCoder screen - a flexible Terminal-Bench solver
//! that supports both FM (Apple Foundation Model) and CC (Claude Code) backends.

use crate::mechacoder::tb2_loader::{TB2Task, TB2TaskLoader};
use hillclimber::HillClimberBackend;
use std::path::PathBuf;

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
    /// Docker image for TB2 tasks
    pub docker_image: Option<String>,
    /// Agent timeout in seconds
    pub timeout_sec: Option<u64>,
    /// Path to TB2 task directory
    pub task_dir: Option<PathBuf>,
    /// Path to tests directory
    pub tests_dir: Option<PathBuf>,
    /// Task difficulty (easy, medium, hard)
    pub difficulty: Option<String>,
    /// Task category
    pub category: Option<String>,
    /// Memory limit (e.g., "2G")
    pub memory_limit: Option<String>,
    /// CPU limit
    pub cpu_limit: Option<u32>,
}

impl MechaTask {
    /// Check if this is a TB2 task (has Docker image)
    pub fn is_tb2_task(&self) -> bool {
        self.docker_image.is_some()
    }

    /// Get display label for the task
    pub fn display_label(&self) -> String {
        if let Some(ref diff) = self.difficulty {
            format!("{} [{}]", self.name, diff)
        } else {
            self.name.clone()
        }
    }
}

impl Default for MechaTask {
    fn default() -> Self {
        Self {
            id: "custom".to_string(),
            name: "Custom Task".to_string(),
            description: String::new(),
            verification_cmd: None,
            docker_image: None,
            timeout_sec: None,
            task_dir: None,
            tests_dir: None,
            difficulty: None,
            category: None,
            memory_limit: None,
            cpu_limit: None,
        }
    }
}

impl From<TB2Task> for MechaTask {
    fn from(tb2: TB2Task) -> Self {
        Self {
            id: tb2.id,
            name: tb2.name,
            description: tb2.instruction,
            verification_cmd: Some("bash /tests/test.sh".to_string()),
            docker_image: Some(tb2.config.environment.docker_image),
            timeout_sec: Some(tb2.config.agent.timeout_sec as u64),
            task_dir: Some(tb2.task_dir),
            tests_dir: Some(tb2.tests_dir),
            difficulty: Some(tb2.config.metadata.difficulty),
            category: Some(tb2.config.metadata.category),
            memory_limit: Some(tb2.config.environment.memory),
            cpu_limit: Some(tb2.config.environment.cpus),
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
    /// Use tbench binary for TB2 tasks (vs legacy DockerRunner)
    /// Default: true - use Harbor's tbench for ATIF trajectory output
    pub use_tbench: bool,
    /// Model override (e.g., "claude-opus-4-5-20251101")
    pub model_override: Option<String>,
}

impl MechaSession {
    pub fn new(backend: HillClimberBackend) -> Self {
        Self {
            backend,
            max_turns: 30,
            use_tbench: true, // Default to tbench for ATIF output
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
    use super::{MechaTask, TB2TaskLoader};

    /// Load all available TB2 tasks
    pub fn load_tb2_tasks() -> Vec<MechaTask> {
        let loader = TB2TaskLoader::new_default();
        if !loader.is_available() {
            tracing::warn!(
                target: "mechacoder",
                "TB2 directory not found, using fallback tasks"
            );
            return vec![regex_log_fallback()];
        }

        let summaries = loader.discover_tasks();
        let mut tasks: Vec<MechaTask> = summaries
            .iter()
            .filter_map(|summary| {
                loader
                    .load_task(&summary.id)
                    .ok()
                    .map(MechaTask::from)
            })
            .collect();

        // If no TB2 tasks found, use fallback
        if tasks.is_empty() {
            tasks.push(regex_log_fallback());
        }

        tasks
    }

    /// Fallback regex-log task when TB2 directory not available
    pub fn regex_log_fallback() -> MechaTask {
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
            docker_image: None,
            timeout_sec: Some(900),
            task_dir: None,
            tests_dir: None,
            difficulty: Some("medium".to_string()),
            category: Some("data-processing".to_string()),
            memory_limit: Some("2G".to_string()),
            cpu_limit: Some(1),
        }
    }

    /// Get all available tasks (TB2 + fallbacks)
    pub fn all_tasks() -> Vec<MechaTask> {
        load_tb2_tasks()
    }

    /// Get the default task (first TB2 task or fallback)
    pub fn default_task() -> MechaTask {
        load_tb2_tasks().into_iter().next().unwrap_or_else(regex_log_fallback)
    }
}
