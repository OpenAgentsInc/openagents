//! Wrapper for the Read tool.

use crate::{PermissionRequest, Tool, ToolContext, ToolInfo, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use tools::MAX_LINES as READ_MAX_LINES;

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
            description: "Read the contents of a file. Returns the file content with line numbers."
                .to_string(),
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

    fn check_permission(
        &self,
        input: &Self::Input,
        ctx: &ToolContext,
    ) -> Option<PermissionRequest> {
        let resolved = ctx.resolve_path(&input.file_path);

        // Check if path is outside working directory
        if !ctx.is_path_allowed(&resolved) {
            return Some(
                PermissionRequest::new(
                    "file_read",
                    format!("Read file: {}", input.file_path),
                    format!(
                        "Allow reading file outside working directory: {}",
                        resolved.display()
                    ),
                )
                .with_patterns(vec![resolved.to_string_lossy().to_string()]),
            );
        }

        None
    }

    async fn execute(&self, input: Self::Input, ctx: &ToolContext) -> ToolResult<ToolOutput> {
        ctx.check_cancelled()?;

        let offset = input.offset.unwrap_or(1);
        if offset == 0 {
            return Err(crate::ToolError::invalid_input(
                "offset must be >= 1 for line-based reads",
            ));
        }

        let limit = input
            .limit
            .filter(|l| *l > 0)
            .map(|l| l.min(READ_MAX_LINES));

        let resolved = ctx.resolve_path(&input.file_path);

        // Execute in blocking task since ReadTool is synchronous
        let result = tokio::task::spawn_blocking(move || {
            tools::ReadTool::read(&resolved, Some(offset), limit)
        })
        .await
        .map_err(|e| crate::ToolError::execution_failed(format!("Task join error: {}", e)))?
        .map_err(crate::ToolError::from)?;

        let has_more = result.remaining_lines.unwrap_or(0) > 0;
        let next_offset = if has_more {
            result.end_line.map(|line| line + 1)
        } else {
            None
        };

        let metadata = serde_json::json!({
            "path": result.path,
            "size_bytes": result.size_bytes,
            "total_lines": result.total_lines,
            "start_line": result.start_line,
            "end_line": result.end_line,
            "lines_returned": result.lines_returned,
            "remaining_lines": result.remaining_lines,
            "truncated": result.truncated,
            "applied_limit": limit.unwrap_or(READ_MAX_LINES),
            "has_more": has_more,
            "next_offset": next_offset,
        });

        let mut content = result.text;
        if has_more {
            if let Some(next) = next_offset {
                content.push_str(&format!(
                    "\n[truncated] Use offset={} to continue reading. Limit capped at {} lines.",
                    next,
                    limit.unwrap_or(READ_MAX_LINES)
                ));
            }
        }

        Ok(ToolOutput::success(content).with_metadata(metadata))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn read_partial_sets_next_offset_and_note() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sample.txt");
        std::fs::write(&path, "a\nb\nc\nd\ne\n").unwrap();

        let wrapper = ReadToolWrapper;
        let ctx = ToolContext::new(dir.path());
        let input = ReadInput {
            file_path: path.to_string_lossy().to_string(),
            offset: None,
            limit: Some(2),
        };

        let output = wrapper.execute(input, &ctx).await.unwrap();
        assert!(output.success);
        assert!(output.content.contains("[truncated]"));
        assert!(output.metadata["has_more"].as_bool().unwrap());
        assert_eq!(output.metadata["next_offset"], 3);
    }

    #[tokio::test]
    async fn read_rejects_zero_offset() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sample.txt");
        std::fs::write(&path, "a\n").unwrap();

        let wrapper = ReadToolWrapper;
        let ctx = ToolContext::new(dir.path());
        let input = ReadInput {
            file_path: path.to_string_lossy().to_string(),
            offset: Some(0),
            limit: Some(1),
        };

        let result = wrapper.execute(input, &ctx).await;
        assert!(matches!(result, Err(crate::ToolError::InvalidInput(_))));
    }
}
