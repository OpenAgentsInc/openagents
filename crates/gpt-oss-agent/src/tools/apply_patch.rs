//! File modification tool
//!
//! Apply patches and edits to files.

use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;

use super::{Tool, ToolResult};

/// Apply patch tool for file modifications
pub struct ApplyPatchTool {
    workspace_root: PathBuf,
}

impl ApplyPatchTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[derive(Debug, Deserialize)]
struct ApplyPatchParams {
    file_path: String,
    patch: String,
}

#[async_trait]
impl Tool for ApplyPatchTool {
    async fn execute(&self, params: serde_json::Value) -> crate::Result<ToolResult> {
        let params: ApplyPatchParams = serde_json::from_value(params)?;

        let result = self.apply_patch(&params.file_path, &params.patch).await;

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
        "apply_patch"
    }

    fn description(&self) -> &str {
        "Apply a patch to modify a file"
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to modify"
                },
                "patch": {
                    "type": "string",
                    "description": "Unified diff patch to apply"
                }
            },
            "required": ["file_path", "patch"]
        })
    }
}

impl ApplyPatchTool {
    async fn apply_patch(&self, file_path: &str, patch: &str) -> crate::Result<String> {
        let full_path = self.workspace_root.join(file_path);

        // Ensure the path is within the workspace
        let canonical_workspace = self
            .workspace_root
            .canonicalize()
            .map_err(|e| crate::GptOssAgentError::ToolError(format!("Invalid workspace: {}", e)))?;

        // For path validation, we need to handle the case where the file doesn't exist yet.
        // We do this by canonicalizing the parent directory (which must exist) and checking
        // that the final path would be within the workspace.
        let canonical_path = if full_path.exists() {
            full_path
                .canonicalize()
                .map_err(|e| crate::GptOssAgentError::ToolError(format!("Invalid path: {}", e)))?
        } else {
            // File doesn't exist - canonicalize the parent and append the filename
            let parent = full_path.parent().ok_or_else(|| {
                crate::GptOssAgentError::ToolError(
                    "Invalid file path: no parent directory".to_string(),
                )
            })?;

            let canonical_parent = parent.canonicalize().map_err(|e| {
                crate::GptOssAgentError::ToolError(format!("Invalid parent directory: {}", e))
            })?;

            let file_name = full_path.file_name().ok_or_else(|| {
                crate::GptOssAgentError::ToolError("Invalid file path: no file name".to_string())
            })?;

            canonical_parent.join(file_name)
        };

        if !canonical_path.starts_with(&canonical_workspace) {
            return Err(crate::GptOssAgentError::ToolError(
                "File path is outside workspace".to_string(),
            ));
        }

        // For now, write the patch to a temp file and use `patch` command
        use std::io::Write;
        let mut patch_file = tempfile::NamedTempFile::new().map_err(|e| {
            crate::GptOssAgentError::ToolError(format!("Failed to create temp file: {}", e))
        })?;

        patch_file.write_all(patch.as_bytes()).map_err(|e| {
            crate::GptOssAgentError::ToolError(format!("Failed to write patch: {}", e))
        })?;

        let output = tokio::process::Command::new("patch")
            .arg(&full_path)
            .arg(patch_file.path())
            .output()
            .await
            .map_err(|e| {
                crate::GptOssAgentError::ToolError(format!("Failed to execute patch: {}", e))
            })?;

        if output.status.success() {
            Ok(format!("Successfully applied patch to {}", file_path))
        } else {
            Err(crate::GptOssAgentError::ToolError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }
}
