//! Wrapper for the Read tool.

use crate::{PermissionRequest, Tool, ToolContext, ToolInfo, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;

/// Input for the Read tool.
#[derive(Debug, Deserialize)]
pub struct ReadInput {
    /// Path to the file to read.
    pub file_path: String,
    /// Optional starting line number (1-indexed).
    #[serde(default)]
    pub offset: Option<usize>,
    /// Optional maximum lines to read.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Async wrapper for the Read tool.
#[derive(Debug)]
pub struct ReadToolWrapper;

#[async_trait]
impl Tool for ReadToolWrapper {
    type Input = ReadInput;

    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "read".to_string(),
            description: "Read the contents of a file. Returns the file content with line numbers.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The absolute path to the file to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "The line number to start reading from (1-indexed)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "The number of lines to read"
                    }
                },
                "required": ["file_path"]
            }),
            tags: vec!["file".to_string(), "read".to_string()],
            requires_permission: false,
        }
    }

    fn check_permission(&self, input: &Self::Input, ctx: &ToolContext) -> Option<PermissionRequest> {
        let resolved = ctx.resolve_path(&input.file_path);

        // Check if path is outside working directory
        if !ctx.is_path_allowed(&resolved) {
            return Some(PermissionRequest::new(
                "file_read",
                format!("Read file: {}", input.file_path),
                format!("Allow reading file outside working directory: {}", resolved.display()),
            ).with_patterns(vec![resolved.to_string_lossy().to_string()]));
        }

        None
    }

    async fn execute(&self, input: Self::Input, ctx: &ToolContext) -> ToolResult<ToolOutput> {
        ctx.check_cancelled()?;

        let resolved = ctx.resolve_path(&input.file_path);

        // Execute in blocking task since ReadTool is synchronous
        let result = tokio::task::spawn_blocking(move || {
            tools::ReadTool::read(&resolved, input.offset, input.limit)
        })
        .await
        .map_err(|e| crate::ToolError::execution_failed(format!("Task join error: {}", e)))?
        .map_err(crate::ToolError::from)?;

        let metadata = serde_json::json!({
            "path": result.path,
            "size_bytes": result.size_bytes,
            "total_lines": result.total_lines,
            "start_line": result.start_line,
            "end_line": result.end_line,
            "lines_returned": result.lines_returned,
            "remaining_lines": result.remaining_lines,
            "truncated": result.truncated,
        });

        Ok(ToolOutput::success(result.text).with_metadata(metadata))
    }
}
