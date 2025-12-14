//! Wrapper for the Edit tool.

use crate::{PermissionRequest, Tool, ToolContext, ToolInfo, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;

/// Input for the Edit tool.
#[derive(Debug, Deserialize)]
pub struct EditInput {
    /// Path to the file to edit.
    pub file_path: String,
    /// The text to find and replace.
    pub old_string: String,
    /// The replacement text.
    pub new_string: String,
    /// If true, replace all occurrences.
    #[serde(default)]
    pub replace_all: bool,
}

/// Async wrapper for the Edit tool.
#[derive(Debug)]
pub struct EditToolWrapper;

#[async_trait]
impl Tool for EditToolWrapper {
    type Input = EditInput;

    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "edit".to_string(),
            description: "Perform a search-and-replace edit on a file. The old_string must match exactly.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The absolute path to the file to edit"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "The text to find and replace (must match exactly)"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "The replacement text"
                    },
                    "replace_all": {
                        "type": "boolean",
                        "description": "If true, replace all occurrences; otherwise only replace if unique"
                    }
                },
                "required": ["file_path", "old_string", "new_string"]
            }),
            tags: vec!["file".to_string(), "edit".to_string()],
            requires_permission: true,
        }
    }

    fn check_permission(&self, input: &Self::Input, ctx: &ToolContext) -> Option<PermissionRequest> {
        let resolved = ctx.resolve_path(&input.file_path);

        let old_preview = truncate_string(&input.old_string, 50);
        let new_preview = truncate_string(&input.new_string, 50);

        Some(PermissionRequest::new(
            "file_edit",
            format!("Edit file: {}", input.file_path),
            format!(
                "Replace \"{}\" with \"{}\" in {}",
                old_preview,
                new_preview,
                resolved.display()
            ),
        ).with_patterns(vec![resolved.to_string_lossy().to_string()]))
    }

    fn validate(&self, input: &Self::Input, _ctx: &ToolContext) -> ToolResult<()> {
        if input.old_string == input.new_string {
            return Err(crate::ToolError::invalid_input(
                "old_string and new_string are identical - no change would occur"
            ));
        }
        Ok(())
    }

    async fn execute(&self, input: Self::Input, ctx: &ToolContext) -> ToolResult<ToolOutput> {
        ctx.check_cancelled()?;

        let resolved = ctx.resolve_path(&input.file_path);
        let old_string = input.old_string.clone();
        let new_string = input.new_string.clone();
        let replace_all = input.replace_all;

        // Execute in blocking task since EditTool is synchronous
        let result = tokio::task::spawn_blocking(move || {
            tools::EditTool::edit(&resolved, &old_string, &new_string, replace_all)
        })
        .await
        .map_err(|e| crate::ToolError::execution_failed(format!("Task join error: {}", e)))?
        .map_err(crate::ToolError::from)?;

        let metadata = serde_json::json!({
            "path": result.path,
            "replacements": result.replacements,
        });

        let msg = format!(
            "Edited {}: {} replacement(s) made",
            result.path,
            result.replacements
        );

        Ok(ToolOutput::success(msg).with_metadata(metadata))
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
