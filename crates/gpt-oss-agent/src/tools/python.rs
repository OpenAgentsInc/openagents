//! Python code execution tool
//!
//! Docker-based sandboxed Python execution, inspired by GPT-OSS's PythonTool.

use async_trait::async_trait;
use serde::Deserialize;

use super::{Tool, ToolResult};

/// Python code execution tool
pub struct PythonTool {
    docker_available: bool,
}

impl Default for PythonTool {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonTool {
    pub fn new() -> Self {
        // Check if Docker is available
        let docker_available = std::process::Command::new("docker")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        Self { docker_available }
    }
}

#[derive(Debug, Deserialize)]
struct PythonParams {
    code: String,
}

#[async_trait]
impl Tool for PythonTool {
    async fn execute(&self, params: serde_json::Value) -> crate::Result<ToolResult> {
        let params: PythonParams = serde_json::from_value(params)?;

        if !self.docker_available {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "Docker is not available. Python execution requires Docker.".to_string(),
                ),
            });
        }

        let result = self.execute_python(&params.code).await;

        match result {
            Ok(output) => Ok(ToolResult {
                success: true,
                output,
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
            }),
        }
    }

    fn name(&self) -> &str {
        "python"
    }

    fn description(&self) -> &str {
        "Execute Python code in a sandboxed Docker container"
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "Python code to execute"
                }
            },
            "required": ["code"]
        })
    }
}

impl PythonTool {
    async fn execute_python(&self, code: &str) -> crate::Result<String> {
        // Create a temporary file with the Python code
        use std::io::Write;
        let mut temp_file = tempfile::NamedTempFile::new().map_err(|e| {
            crate::GptOssAgentError::ToolError(format!("Failed to create temp file: {}", e))
        })?;

        temp_file.write_all(code.as_bytes()).map_err(|e| {
            crate::GptOssAgentError::ToolError(format!("Failed to write code: {}", e))
        })?;

        // Execute in Docker container
        let output = tokio::process::Command::new("docker")
            .args([
                "run",
                "--rm",
                "-i",
                "--network=none",
                "--memory=512m",
                "--cpus=1",
                "python:3.11-slim",
                "python",
                "-c",
                code,
            ])
            .output()
            .await
            .map_err(|e| {
                crate::GptOssAgentError::ToolError(format!("Failed to execute Docker: {}", e))
            })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(crate::GptOssAgentError::ToolError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }
}
