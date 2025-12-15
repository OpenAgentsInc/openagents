//! Wrapper for the Grep tool.

use crate::{PermissionRequest, Tool, ToolContext, ToolInfo, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Input for the Grep tool.
#[derive(Debug, Deserialize)]
pub struct GrepInput {
    /// The regex pattern to search for.
    pub pattern: String,
    /// The path to search in (file or directory).
    #[serde(default)]
    pub path: Option<String>,
    /// Maximum number of results.
    #[serde(default)]
    pub max_results: Option<usize>,
    /// Case insensitive search.
    #[serde(default)]
    pub ignore_case: bool,
    /// Whether to respect .gitignore and VCS ignore files.
    #[serde(default = "default_respect_gitignore")]
    pub respect_gitignore: bool,
}

fn default_respect_gitignore() -> bool {
    true
}

/// A serializable match result for output.
#[derive(Debug, Clone, Serialize)]
pub struct GrepMatchOutput {
    pub file: String,
    pub line: usize,
    pub text: String,
}

/// Async wrapper for the Grep tool.
#[derive(Debug)]
pub struct GrepToolWrapper;

#[async_trait]
impl Tool for GrepToolWrapper {
    type Input = GrepInput;

    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "grep".to_string(),
            description: "Search for a pattern in files using regex. Returns matching lines with file paths and line numbers.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The regex pattern to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "The file or directory to search in (defaults to current directory)"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of matches to return"
                    },
                    "ignore_case": {
                        "type": "boolean",
                        "description": "If true, search is case insensitive"
                    },
                    "respect_gitignore": {
                        "type": "boolean",
                        "description": "Whether to honor .gitignore and VCS ignore files",
                        "default": true
                    }
                },
                "required": ["pattern"]
            }),
            tags: vec!["search".to_string(), "grep".to_string()],
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
                        format!("Search in: {}", path),
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
        let ignore_case = input.ignore_case;
        let respect_gitignore = input.respect_gitignore;

        // Execute in blocking task since GrepTool is synchronous
        let result = tokio::task::spawn_blocking(move || {
            tools::GrepTool::search_with_options(
                &pattern,
                &path,
                ignore_case,
                max_results,
                respect_gitignore,
            )
        })
        .await
        .map_err(|e| crate::ToolError::execution_failed(format!("Task join error: {}", e)))?
        .map_err(crate::ToolError::from)?;

        let metadata = serde_json::json!({
            "matches_count": result.matches.len(),
            "files_searched": result.files_searched,
            "truncated": result.truncated,
        });

        // Convert to serializable output
        let matches_output: Vec<GrepMatchOutput> = result
            .matches
            .iter()
            .map(|m| GrepMatchOutput {
                file: m.file.clone(),
                line: m.line,
                text: m.text.clone(),
            })
            .collect();

        // Format output as text
        let mut output = String::new();
        for m in &matches_output {
            output.push_str(&format!("{}:{}:{}\n", m.file, m.line, m.text));
        }

        if matches_output.is_empty() {
            output = "No matches found".to_string();
        }

        Ok(ToolOutput::success_with_data(output, matches_output).with_metadata(metadata))
    }
}
