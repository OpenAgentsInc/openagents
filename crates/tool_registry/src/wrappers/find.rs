//! Wrapper for the Find (glob) tool.

use crate::{PermissionRequest, Tool, ToolContext, ToolInfo, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;

/// Input for the Find tool.
#[derive(Debug, Deserialize)]
pub struct FindInput {
    /// The glob pattern to match files.
    pub pattern: String,
    /// The directory to search in.
    #[serde(default)]
    pub path: Option<String>,
    /// Maximum number of results.
    #[serde(default)]
    pub max_results: Option<usize>,
    /// Whether to respect .gitignore and VCS ignore files.
    #[serde(default = "default_respect_gitignore")]
    pub respect_gitignore: bool,
}

fn default_respect_gitignore() -> bool {
    true
}

/// Async wrapper for the Find/Glob tool.
#[derive(Debug)]
pub struct FindToolWrapper;

#[async_trait]
impl Tool for FindToolWrapper {
    type Input = FindInput;

    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "glob".to_string(),
            description:
                "Find files matching a glob pattern. Returns a list of matching file paths."
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The glob pattern to match (e.g., '**/*.rs', 'src/**/*.ts')"
                    },
                    "path": {
                        "type": "string",
                        "description": "The directory to search in (defaults to current directory)"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return"
                    },
                    "respect_gitignore": {
                        "type": "boolean",
                        "description": "Whether to honor .gitignore and VCS ignore files",
                        "default": true
                    }
                },
                "required": ["pattern"]
            }),
            tags: vec!["search".to_string(), "glob".to_string(), "find".to_string()],
            requires_permission: false,
        }
    }

    fn check_permission(
        &self,
        input: &Self::Input,
        ctx: &ToolContext,
    ) -> Option<PermissionRequest> {
        // Only require permission if searching outside working directory
        if let Some(path) = &input.path {
            let resolved = ctx.resolve_path(path);
            if !ctx.is_path_allowed(&resolved) {
                return Some(
                    PermissionRequest::new(
                        "file_search",
                        format!("Find files in: {}", path),
                        format!(
                            "Allow searching outside working directory: {}",
                            resolved.display()
                        ),
                    )
                    .with_patterns(vec![resolved.to_string_lossy().to_string()]),
                );
            }
        }
        None
    }

    async fn execute(&self, input: Self::Input, ctx: &ToolContext) -> ToolResult<ToolOutput> {
        ctx.check_cancelled()?;

        let path = input
            .path
            .as_ref()
            .map(|p| ctx.resolve_path(p))
            .unwrap_or_else(|| ctx.working_dir.clone());
        let pattern = input.pattern.clone();
        let max_results = input.max_results;
        let respect_gitignore = input.respect_gitignore;

        // Execute in blocking task since FindTool is synchronous
        // Use find_glob for glob patterns
        let result = tokio::task::spawn_blocking(move || {
            tools::FindTool::find_with_options(
                &path,
                None,
                Some(&pattern),
                max_results,
                false,
                respect_gitignore,
            )
        })
        .await
        .map_err(|e| crate::ToolError::execution_failed(format!("Task join error: {}", e)))?
        .map_err(crate::ToolError::from)?;

        let metadata = serde_json::json!({
            "matches": result.matches,
            "entries_visited": result.entries_visited,
            "truncated": result.truncated,
        });

        // Format output as newline-separated paths
        let output = result.files.join("\n");

        let summary = if result.files.is_empty() {
            "No files found".to_string()
        } else {
            output
        };

        Ok(ToolOutput::success_with_data(summary, result.files).with_metadata(metadata))
    }
}
