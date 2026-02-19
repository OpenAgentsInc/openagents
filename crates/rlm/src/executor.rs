//! Execution environment trait for RLM.
//!
//! Defines the interface for code execution backends. Implementations include:
//! - `MockExecutor` - For testing
//! - `WasmExecutor` - WASM sandbox (future)
//! - `JsExecutor` - JavaScript via QuickJS in WASM (future)

use async_trait::async_trait;

use crate::error::Result;

/// Result of code execution.
#[derive(Debug, Clone, Default)]
pub struct ExecutionResult {
    /// Standard output from the execution.
    pub stdout: String,
    /// Standard error from the execution.
    pub stderr: String,
    /// Exit code (0 = success).
    pub exit_code: i32,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
}

impl ExecutionResult {
    /// Create a successful result with stdout.
    pub fn success(stdout: impl Into<String>) -> Self {
        Self {
            stdout: stdout.into(),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 0,
        }
    }

    /// Create an error result with stderr.
    pub fn error(stderr: impl Into<String>) -> Self {
        Self {
            stdout: String::new(),
            stderr: stderr.into(),
            exit_code: 1,
            duration_ms: 0,
        }
    }

    /// Check if execution was successful.
    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }

    /// Get the output (stdout if success, stderr if error).
    pub fn output(&self) -> &str {
        if self.is_success() {
            &self.stdout
        } else {
            &self.stderr
        }
    }
}

/// Capabilities of an execution environment.
#[derive(Debug, Clone, Default)]
pub struct ExecutorCapabilities {
    /// Supported programming languages.
    pub languages: Vec<String>,
    /// Whether filesystem access is available.
    pub has_filesystem: bool,
    /// Whether network access is available.
    pub has_network: bool,
    /// Maximum memory in bytes.
    pub max_memory_bytes: Option<u64>,
    /// Maximum execution time in milliseconds.
    pub max_time_ms: Option<u64>,
}

/// Trait for code execution environments.
///
/// Implementations must be Send + Sync for async usage.
#[async_trait]
pub trait ExecutionEnvironment: Send + Sync {
    /// Execute code and return the result.
    ///
    /// # Arguments
    ///
    /// * `code` - The code to execute
    ///
    /// # Returns
    ///
    /// The execution result including stdout, stderr, and exit code.
    async fn execute(&self, code: &str) -> Result<ExecutionResult>;

    /// Get the capabilities of this executor.
    fn capabilities(&self) -> ExecutorCapabilities {
        ExecutorCapabilities::default()
    }

    /// Check if the executor is ready to accept code.
    async fn is_ready(&self) -> bool {
        true
    }
}
