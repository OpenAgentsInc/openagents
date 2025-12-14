//! Wrapper for the Write tool.

use crate::{PermissionRequest, Tool, ToolContext, ToolInfo, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;

/// Input for the Write tool.
#[derive(Debug, Deserialize)]
pub struct WriteInput {
    /// Path to the file to write.
    pub file_path: String,
    /// Content to write to the file.
    pub content: String,
}

/// Async wrapper for the Write tool.
#[derive(Debug)]
pub struct WriteToolWrapper;

#[async_trait]
impl Tool for WriteToolWrapper {
    type Input = WriteInput;

    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "write".to_string(),
            description: "Write content to a file, creating it if it doesn't exist or overwriting if it does.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The absolute path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    }
                },
                "required": ["file_path", "content"]
            }),
            tags: vec!["file".to_string(), "write".to_string()],
            requires_permission: true,
        }
    }

    fn check_permission(&self, input: &Self::Input, ctx: &ToolContext) -> Option<PermissionRequest> {
        let resolved = ctx.resolve_path(&input.file_path);
        let exists = resolved.exists();

        let action = if exists { "Overwrite" } else { "Create" };
        let title = format!("{} file: {}", action, input.file_path);

        Some(PermissionRequest::new(
            "file_write",
            title,
            format!(
                "{} file with {} bytes: {}",
                action,
                input.content.len(),
                resolved.display()
            ),
        ).with_patterns(vec![resolved.to_string_lossy().to_string()]))
    }

    async fn execute(&self, input: Self::Input, ctx: &ToolContext) -> ToolResult<ToolOutput> {
        ctx.check_cancelled()?;

        let resolved = ctx.resolve_path(&input.file_path);
        let content = input.content.clone();

        // Execute in blocking task since WriteTool is synchronous
        let result = tokio::task::spawn_blocking(move || {
            tools::WriteTool::write(&resolved, &content)
        })
        .await
        .map_err(|e| crate::ToolError::execution_failed(format!("Task join error: {}", e)))?
        .map_err(crate::ToolError::from)?;

        let metadata = serde_json::json!({
            "path": result.path,
            "bytes_written": result.bytes_written,
            "existed_before": result.existed_before,
            "previous_size": result.previous_size,
            "new_size": result.new_size,
        });

        let msg = if !result.existed_before {
            format!("Created file: {} ({} bytes)", result.path, result.bytes_written)
        } else {
            format!("Updated file: {} ({} bytes)", result.path, result.bytes_written)
        };

        Ok(ToolOutput::success(msg).with_metadata(metadata))
    }
}
