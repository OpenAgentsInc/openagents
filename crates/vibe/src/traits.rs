//! Backend traits for the Vibe IDE
//!
//! These traits abstract the filesystem, terminal, and job backends
//! so the IDE can work with different implementations (OANIX, mock, etc.)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::VibeError;

/// Directory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    /// Entry name
    pub name: String,
    /// Whether this is a directory
    pub is_dir: bool,
    /// Size in bytes
    pub size: u64,
}

/// Filesystem trait for the IDE
#[async_trait]
pub trait IdeFs: Send + Sync {
    /// Read a file's contents
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, VibeError>;

    /// Write data to a file
    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), VibeError>;

    /// Read directory contents
    async fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>, VibeError>;

    /// Remove a file
    async fn remove_file(&self, path: &str) -> Result<(), VibeError>;

    /// Create directories recursively
    async fn create_dir_all(&self, path: &str) -> Result<(), VibeError>;

    /// Check if path exists
    async fn exists(&self, path: &str) -> Result<bool, VibeError>;
}

/// Handle to an open terminal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalHandle(pub Uuid);

/// Terminal backend trait
#[async_trait]
pub trait TerminalBackend: Send + Sync {
    /// Open a new terminal
    async fn open(&self, name: &str) -> Result<TerminalHandle, VibeError>;

    /// Write data to a terminal
    async fn write(&self, handle: TerminalHandle, data: &[u8]) -> Result<(), VibeError>;

    /// Close a terminal
    async fn close(&self, handle: TerminalHandle) -> Result<(), VibeError>;

    /// Subscribe to terminal output
    fn subscribe(&self, handle: TerminalHandle, callback: Box<dyn Fn(Vec<u8>) + Send + Sync>);
}

/// Job identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub Uuid);

/// Job specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSpec {
    /// Type of job
    pub kind: JobKind,
    /// Working directory
    pub cwd: Option<String>,
}

/// Kind of job to run
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JobKind {
    /// Scaffold a full-stack app from a prompt
    ScaffoldFullStack { prompt: String },
    /// Refactor a file or region
    Refactor {
        path: String,
        instructions: String,
    },
    /// Add an API endpoint
    AddEndpoint {
        path: String,
        method: String,
    },
    /// Generate tests
    GenerateTests { scope: TestScope },
    /// Run TerminalBench task
    TerminalBench { task_id: String },
    /// Run a shell command
    Shell { command: String },
}

/// Scope for test generation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestScope {
    /// Backend only
    BackendOnly,
    /// Frontend only
    FrontendOnly,
    /// Full stack
    FullStack,
}

/// Job status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum JobStatus {
    /// Job is waiting to run
    Pending,
    /// Job is currently running
    Running { progress: Option<f32> },
    /// Job completed successfully
    Succeeded { output: String },
    /// Job failed
    Failed { error: String },
}

/// Log entry from a job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Log level
    pub level: LogLevel,
    /// Message
    pub message: String,
}

/// Log level
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Job backend trait
#[async_trait]
pub trait JobBackend: Send + Sync {
    /// Submit a new job
    async fn submit(&self, spec: JobSpec) -> Result<JobId, VibeError>;

    /// Get job status
    async fn status(&self, id: JobId) -> Result<JobStatus, VibeError>;

    /// Get job logs
    async fn logs(&self, id: JobId) -> Result<Vec<LogEntry>, VibeError>;

    /// Cancel a running job
    async fn cancel(&self, id: JobId) -> Result<(), VibeError>;
}
