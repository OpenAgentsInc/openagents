//! Python executor that runs code via subprocess.

use std::io::Write;
use std::process::Command as ProcessCommand;
use std::time::Instant;

use async_trait::async_trait;
use tempfile::NamedTempFile;

use crate::error::{Result, RlmError};
use crate::executor::{ExecutionEnvironment, ExecutionResult, ExecutorCapabilities};

/// Python executor that runs code via subprocess.
///
/// Executes Python code by writing to a temporary file and running it
/// with the system Python interpreter.
pub struct PythonExecutor {
    /// Path to the Python binary (e.g., "python3", "/usr/bin/python3")
    python_binary: String,
    /// Timeout in seconds for execution
    timeout_secs: u64,
}

impl Default for PythonExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonExecutor {
    /// Create a new Python executor with default settings.
    ///
    /// Uses "python3" as the default interpreter.
    pub fn new() -> Self {
        Self {
            python_binary: "python3".to_string(),
            timeout_secs: 30,
        }
    }

    /// Create a Python executor with a custom binary path.
    pub fn with_binary(binary: impl Into<String>) -> Self {
        Self {
            python_binary: binary.into(),
            timeout_secs: 30,
        }
    }

    /// Set the execution timeout in seconds.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Check if Python is available on the system.
    pub fn is_available(&self) -> bool {
        ProcessCommand::new(&self.python_binary)
            .arg("--version")
            .output()
            .is_ok()
    }

    /// Get the Python version string.
    pub fn version(&self) -> Option<String> {
        ProcessCommand::new(&self.python_binary)
            .arg("--version")
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8(output.stdout)
                    .or_else(|_| String::from_utf8(output.stderr))
                    .ok()
            })
            .map(|s| s.trim().to_string())
    }
}

#[async_trait]
impl ExecutionEnvironment for PythonExecutor {
    async fn execute(&self, code: &str) -> Result<ExecutionResult> {
        let start = Instant::now();

        // Create a temporary file for the Python code
        let mut temp_file = NamedTempFile::new()
            .map_err(|e| RlmError::ExecutionError(format!("Failed to create temp file: {}", e)))?;

        // Write the code to the temp file
        temp_file
            .write_all(code.as_bytes())
            .map_err(|e| RlmError::ExecutionError(format!("Failed to write code: {}", e)))?;

        temp_file
            .flush()
            .map_err(|e| RlmError::ExecutionError(format!("Failed to flush: {}", e)))?;

        // Execute the Python script
        let output = ProcessCommand::new(&self.python_binary)
            .arg(temp_file.path())
            .output()
            .map_err(|e| RlmError::ExecutionError(format!("Failed to execute Python: {}", e)))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok(ExecutionResult {
            stdout,
            stderr,
            exit_code,
            duration_ms,
        })
    }

    fn capabilities(&self) -> ExecutorCapabilities {
        ExecutorCapabilities {
            languages: vec!["python".to_string()],
            has_filesystem: true,
            has_network: true,
            max_memory_bytes: None,
            max_time_ms: Some(self.timeout_secs * 1000),
        }
    }

    async fn is_ready(&self) -> bool {
        self.is_available()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_executor_creation() {
        let executor = PythonExecutor::new();
        assert_eq!(executor.python_binary, "python3");
        assert_eq!(executor.timeout_secs, 30);
    }

    #[test]
    fn test_python_executor_with_binary() {
        let executor = PythonExecutor::with_binary("/usr/bin/python3");
        assert_eq!(executor.python_binary, "/usr/bin/python3");
    }

    #[tokio::test]
    async fn test_python_executor_simple() {
        let executor = PythonExecutor::new();

        if !executor.is_available() {
            eprintln!("Skipping test: Python not available");
            return;
        }

        let result = executor.execute("print(2 + 2)").await.unwrap();
        assert_eq!(result.stdout.trim(), "4");
        assert_eq!(result.exit_code, 0);
    }

    #[tokio::test]
    async fn test_python_executor_multiline() {
        let executor = PythonExecutor::new();

        if !executor.is_available() {
            return;
        }

        let code = r#"
x = 10
y = 20
print(f"Sum: {x + y}")
"#;
        let result = executor.execute(code).await.unwrap();
        assert!(result.stdout.contains("Sum: 30"));
        assert_eq!(result.exit_code, 0);
    }

    #[tokio::test]
    async fn test_python_executor_error() {
        let executor = PythonExecutor::new();

        if !executor.is_available() {
            return;
        }

        let result = executor.execute("undefined_variable").await.unwrap();
        assert!(!result.stderr.is_empty());
        assert_ne!(result.exit_code, 0);
    }
}
