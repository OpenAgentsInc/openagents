//! Wrapper for the Bash tool.

use crate::{PermissionRequest, Tool, ToolContext, ToolInfo, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;

/// Input for the Bash tool.
#[derive(Debug, Deserialize)]
pub struct BashInput {
    /// The command to execute.
    pub command: String,
    /// Optional timeout in milliseconds.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Optional working directory.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Optional description of what the command does.
    #[serde(default)]
    pub description: Option<String>,
}

/// Async wrapper for the Bash tool.
#[derive(Debug)]
pub struct BashToolWrapper;

#[async_trait]
impl Tool for BashToolWrapper {
    type Input = BashInput;

    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "bash".to_string(),
            description: "Execute a bash command. Returns stdout, stderr, and exit code."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Optional timeout in milliseconds (max 600000)"
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Optional working directory for the command"
                    },
                    "description": {
                        "type": "string",
                        "description": "Short description of what this command does"
                    }
                },
                "required": ["command"]
            }),
            tags: vec!["shell".to_string(), "execute".to_string()],
            requires_permission: true,
        }
    }

    fn check_permission(
        &self,
        input: &Self::Input,
        _ctx: &ToolContext,
    ) -> Option<PermissionRequest> {
        // If command targets an external directory, prompt specifically for that.
        if let Some(cwd) = &input.cwd {
            let resolved: PathBuf = _ctx.resolve_path(cwd);
            if !_ctx.is_path_allowed(&resolved) {
                return Some(
                    PermissionRequest::new(
                        "external_directory",
                        "Access external directory",
                        format!("Run command in external directory: {}", resolved.display()),
                    )
                    .with_patterns(vec![resolved.display().to_string()]),
                );
            }
        }

        // All bash commands require permission
        let desc = input
            .description
            .as_deref()
            .unwrap_or("Execute shell command");
        Some(
            PermissionRequest::new(
                "bash",
                desc,
                format!("Execute command: {}", truncate_command(&input.command, 100)),
            )
            .with_patterns(vec![input.command.clone()]),
        )
    }

    async fn execute(&self, input: Self::Input, ctx: &ToolContext) -> ToolResult<ToolOutput> {
        ctx.check_cancelled()?;

        let cwd = input
            .cwd
            .clone()
            .or_else(|| ctx.working_dir.to_str().map(|s| s.to_string()));
        let timeout = input.timeout;
        let command = input.command.clone();

        // Execute in blocking task since BashTool is synchronous
        let result = tokio::task::spawn_blocking(move || {
            tools::BashTool::execute_with_options(&command, timeout, cwd.as_deref())
        })
        .await
        .map_err(|e| crate::ToolError::execution_failed(format!("Task join error: {}", e)))?
        .map_err(crate::ToolError::from)?;

        let metadata = serde_json::json!({
            "exit_code": result.exit_code,
            "success": result.success,
            "duration_ms": result.duration_ms,
            "truncated": result.truncated,
            "timed_out": result.timed_out,
        });

        let content = if result.success {
            result.output
        } else {
            format!(
                "Command failed (exit {}): {}",
                result
                    .exit_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "signal".to_string()),
                result.output
            )
        };

        if result.success {
            Ok(ToolOutput::success(content).with_metadata(metadata))
        } else {
            Ok(ToolOutput::failure(content).with_metadata(metadata))
        }
    }
}

fn truncate_command(cmd: &str, max_len: usize) -> String {
    if cmd.len() <= max_len {
        cmd.to_string()
    } else {
        format!("{}...", &cmd[..max_len - 3])
    }
}
