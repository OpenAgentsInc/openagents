//! Tool execution framework
//!
//! Provides tool definitions and execution with abort support, streaming output,
//! and proper truncation - ported from pi-mono's tool implementations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::error::{PiError, PiResult};

/// Maximum output size before truncation (50KB like pi-mono)
pub const MAX_OUTPUT_BYTES: usize = 50 * 1024;

/// Maximum lines in output
pub const MAX_OUTPUT_LINES: usize = 2000;

/// Default tool timeout in seconds
pub const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Tool definition for LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Result of tool execution
#[derive(Debug, Clone)]
pub struct ToolOutput {
    /// Output content (text or formatted)
    pub content: String,
    /// Whether this is an error result
    pub is_error: bool,
    /// Truncation info if output was truncated
    pub truncation: Option<TruncationInfo>,
    /// Files that were modified (for session tracking)
    pub files_modified: Vec<PathBuf>,
}

/// Information about output truncation
#[derive(Debug, Clone)]
pub struct TruncationInfo {
    /// Total bytes before truncation
    pub total_bytes: usize,
    /// Total lines before truncation
    pub total_lines: usize,
    /// Bytes returned
    pub output_bytes: usize,
    /// Lines returned
    pub output_lines: usize,
    /// Path to full output file (if saved)
    pub full_output_path: Option<PathBuf>,
    /// Whether truncated by bytes or lines
    pub truncated_by: TruncatedBy,
}

#[derive(Debug, Clone, Copy)]
pub enum TruncatedBy {
    Lines,
    Bytes,
}

/// Trait for tool implementations
#[async_trait]
pub trait PiTool: Send + Sync {
    /// Tool name (must be unique)
    fn name(&self) -> &str;

    /// Tool definition for LLM
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool
    async fn execute(
        &self,
        input: Value,
        cwd: &Path,
        cancel: CancellationToken,
    ) -> PiResult<ToolOutput>;
}

/// Registry of available tools
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn PiTool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create a new registry with default tools
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };

        // Register default tools
        registry.register(Arc::new(BashTool));
        registry.register(Arc::new(ReadTool));
        registry.register(Arc::new(WriteTool));
        registry.register(Arc::new(EditTool));

        registry
    }

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn PiTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn PiTool>> {
        self.tools.get(name).cloned()
    }

    /// Get all tool definitions
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Execute a tool by name
    pub async fn execute(
        &self,
        name: &str,
        input: Value,
        cwd: &Path,
        cancel: CancellationToken,
    ) -> PiResult<ToolOutput> {
        let tool = self.get(name).ok_or_else(|| PiError::ToolNotFound(name.to_string()))?;
        tool.execute(input, cwd, cancel).await
    }
}

// ============================================================================
// Bash Tool - Command execution with streaming and abort
// ============================================================================

pub struct BashTool;

#[async_trait]
impl PiTool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "bash".to_string(),
            description: "Execute a bash command in the current working directory. Returns stdout and stderr combined. Output is truncated to last 2000 lines or 50KB (whichever is hit first).".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute"
                    },
                    "timeout": {
                        "type": "number",
                        "description": "Optional timeout in seconds (default: 120)"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(
        &self,
        input: Value,
        cwd: &Path,
        cancel: CancellationToken,
    ) -> PiResult<ToolOutput> {
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PiError::tool("bash", "Missing 'command' parameter"))?;

        let timeout_secs = input
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_SECS);

        debug!(command = %command, cwd = %cwd.display(), "Executing bash command");

        // Spawn the process
        let mut child = Command::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| PiError::tool("bash", format!("Failed to spawn: {}", e)))?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        // Collect output with timeout and cancellation
        let timeout = tokio::time::Duration::from_secs(timeout_secs);

        let output_future = async {
            let mut combined = Vec::new();
            let mut stdout_reader = BufReader::new(stdout);
            let mut stderr_reader = BufReader::new(stderr);

            let mut stdout_buf = Vec::new();
            let mut stderr_buf = Vec::new();

            // Read both streams
            tokio::try_join!(
                async {
                    stdout_reader.read_to_end(&mut stdout_buf).await
                },
                async {
                    stderr_reader.read_to_end(&mut stderr_buf).await
                }
            ).map_err(|e| PiError::tool("bash", format!("Read error: {}", e)))?;

            // Combine stdout and stderr
            combined.extend_from_slice(&stdout_buf);
            if !stderr_buf.is_empty() {
                if !combined.is_empty() {
                    combined.push(b'\n');
                }
                combined.extend_from_slice(&stderr_buf);
            }

            Ok::<_, PiError>(combined)
        };

        let result = tokio::select! {
            _ = cancel.cancelled() => {
                // Kill the process
                let _ = child.kill().await;
                return Err(PiError::Cancelled);
            }
            _ = tokio::time::sleep(timeout) => {
                // Kill on timeout
                let _ = child.kill().await;
                return Ok(ToolOutput {
                    content: format!("Command timed out after {} seconds", timeout_secs),
                    is_error: true,
                    truncation: None,
                    files_modified: vec![],
                });
            }
            output = output_future => output?,
        };

        // Wait for exit status
        let status = child.wait().await
            .map_err(|e| PiError::tool("bash", format!("Wait error: {}", e)))?;

        // Truncate output (tail truncation like pi-mono)
        let (content, truncation) = truncate_tail(&result);

        let mut output_text = content;
        if !status.success() {
            if let Some(code) = status.code() {
                output_text.push_str(&format!("\n\nCommand exited with code {}", code));
            }
        }

        if let Some(ref trunc) = truncation {
            output_text.push_str(&format!(
                "\n\n[Output truncated. Showing last {} lines of {}]",
                trunc.output_lines, trunc.total_lines
            ));
        }

        Ok(ToolOutput {
            content: output_text,
            is_error: !status.success(),
            truncation,
            files_modified: vec![],
        })
    }
}

// ============================================================================
// Read Tool - File reading with pagination
// ============================================================================

pub struct ReadTool;

#[async_trait]
impl PiTool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read".to_string(),
            description: "Read a file's contents. Supports text files with line numbers and images (returned as base64). Output is truncated to 2000 lines.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read (relative to working directory)"
                    },
                    "offset": {
                        "type": "number",
                        "description": "Line number to start from (1-indexed, default: 1)"
                    },
                    "limit": {
                        "type": "number",
                        "description": "Maximum lines to read (default: 2000)"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(
        &self,
        input: Value,
        cwd: &Path,
        cancel: CancellationToken,
    ) -> PiResult<ToolOutput> {
        if cancel.is_cancelled() {
            return Err(PiError::Cancelled);
        }

        let path_str = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PiError::tool("read", "Missing 'path' parameter"))?;

        let offset = input.get("offset").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
        let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(MAX_OUTPUT_LINES as u64) as usize;

        // Resolve path
        let path = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else {
            cwd.join(path_str)
        };

        // Expand ~ if present
        let path_str = path.to_string_lossy();
        let expanded = shellexpand::tilde(&path_str).to_string();
        let path = PathBuf::from(expanded);

        debug!(path = %path.display(), "Reading file");

        if !path.exists() {
            return Ok(ToolOutput {
                content: format!("File not found: {}", path.display()),
                is_error: true,
                truncation: None,
                files_modified: vec![],
            });
        }

        if path.is_dir() {
            return Ok(ToolOutput {
                content: format!("Path is a directory: {}", path.display()),
                is_error: true,
                truncation: None,
                files_modified: vec![],
            });
        }

        // Check if it's an image
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if matches!(extension.to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp") {
            // For images, we'd return base64 - for now just indicate it's an image
            let content = tokio::fs::read(&path).await
                .map_err(|e| PiError::tool("read", format!("Failed to read: {}", e)))?;

            let base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &content);
            return Ok(ToolOutput {
                content: format!("[Image file: {} ({} bytes)]\nBase64: {}", path.display(), content.len(), &base64[..100.min(base64.len())]),
                is_error: false,
                truncation: None,
                files_modified: vec![],
            });
        }

        // Read text file
        let content = tokio::fs::read_to_string(&path).await
            .map_err(|e| PiError::tool("read", format!("Failed to read: {}", e)))?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // Apply offset and limit (1-indexed)
        let start = (offset.saturating_sub(1)).min(lines.len());
        let end = (start + limit).min(lines.len());
        let selected_lines = &lines[start..end];

        // Format with line numbers
        let mut output = String::new();
        for (i, line) in selected_lines.iter().enumerate() {
            let line_num = start + i + 1;
            // Truncate long lines
            let display_line = if line.len() > 2000 {
                format!("{}...[truncated]", &line[..2000])
            } else {
                line.to_string()
            };
            output.push_str(&format!("{:>6}\t{}\n", line_num, display_line));
        }

        let truncation = if end < total_lines || start > 0 {
            Some(TruncationInfo {
                total_bytes: content.len(),
                total_lines,
                output_bytes: output.len(),
                output_lines: selected_lines.len(),
                full_output_path: None,
                truncated_by: TruncatedBy::Lines,
            })
        } else {
            None
        };

        if truncation.is_some() {
            output.push_str(&format!(
                "\n[Showing lines {}-{} of {}]",
                start + 1,
                end,
                total_lines
            ));
        }

        Ok(ToolOutput {
            content: output,
            is_error: false,
            truncation,
            files_modified: vec![],
        })
    }
}

// ============================================================================
// Write Tool - File writing with directory creation
// ============================================================================

pub struct WriteTool;

#[async_trait]
impl PiTool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write".to_string(),
            description: "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Creates parent directories as needed.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    async fn execute(
        &self,
        input: Value,
        cwd: &Path,
        cancel: CancellationToken,
    ) -> PiResult<ToolOutput> {
        if cancel.is_cancelled() {
            return Err(PiError::Cancelled);
        }

        let path_str = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PiError::tool("write", "Missing 'path' parameter"))?;

        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PiError::tool("write", "Missing 'content' parameter"))?;

        // Resolve path
        let path = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else {
            cwd.join(path_str)
        };

        debug!(path = %path.display(), bytes = content.len(), "Writing file");

        // Create parent directories
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent).await
                    .map_err(|e| PiError::tool("write", format!("Failed to create directory: {}", e)))?;
            }
        }

        // Write file
        tokio::fs::write(&path, content).await
            .map_err(|e| PiError::tool("write", format!("Failed to write: {}", e)))?;

        Ok(ToolOutput {
            content: format!("Wrote {} bytes to {}", content.len(), path.display()),
            is_error: false,
            truncation: None,
            files_modified: vec![path],
        })
    }
}

// ============================================================================
// Edit Tool - Search and replace with diff
// ============================================================================

pub struct EditTool;

#[async_trait]
impl PiTool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "edit".to_string(),
            description: "Edit a file by replacing text. The old_text must match exactly (including whitespace). Fails if the text appears multiple times or not at all.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to edit"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "Text to find and replace (must match exactly)"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "Replacement text"
                    }
                },
                "required": ["path", "old_text", "new_text"]
            }),
        }
    }

    async fn execute(
        &self,
        input: Value,
        cwd: &Path,
        cancel: CancellationToken,
    ) -> PiResult<ToolOutput> {
        if cancel.is_cancelled() {
            return Err(PiError::Cancelled);
        }

        let path_str = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PiError::tool("edit", "Missing 'path' parameter"))?;

        let old_text = input
            .get("old_text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PiError::tool("edit", "Missing 'old_text' parameter"))?;

        let new_text = input
            .get("new_text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PiError::tool("edit", "Missing 'new_text' parameter"))?;

        // Resolve path
        let path = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else {
            cwd.join(path_str)
        };

        debug!(path = %path.display(), "Editing file");

        if !path.exists() {
            return Ok(ToolOutput {
                content: format!("File not found: {}", path.display()),
                is_error: true,
                truncation: None,
                files_modified: vec![],
            });
        }

        // Read current content
        let content = tokio::fs::read_to_string(&path).await
            .map_err(|e| PiError::tool("edit", format!("Failed to read: {}", e)))?;

        // Check if old_text is same as new_text
        if old_text == new_text {
            return Ok(ToolOutput {
                content: "old_text and new_text are identical - no change would be made".to_string(),
                is_error: true,
                truncation: None,
                files_modified: vec![],
            });
        }

        // Count occurrences
        let occurrences = content.matches(old_text).count();

        if occurrences == 0 {
            return Ok(ToolOutput {
                content: format!("Text not found in file: {:?}", old_text),
                is_error: true,
                truncation: None,
                files_modified: vec![],
            });
        }

        if occurrences > 1 {
            return Ok(ToolOutput {
                content: format!(
                    "Text appears {} times in file. Edit requires unique match. Provide more context.",
                    occurrences
                ),
                is_error: true,
                truncation: None,
                files_modified: vec![],
            });
        }

        // Perform replacement
        let new_content = content.replacen(old_text, new_text, 1);

        // Generate diff
        let diff = generate_diff(&content, &new_content, &path);

        // Write new content
        tokio::fs::write(&path, &new_content).await
            .map_err(|e| PiError::tool("edit", format!("Failed to write: {}", e)))?;

        Ok(ToolOutput {
            content: format!("Edited {}:\n{}", path.display(), diff),
            is_error: false,
            truncation: None,
            files_modified: vec![path],
        })
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Truncate output from the tail (keep last N lines/bytes)
fn truncate_tail(bytes: &[u8]) -> (String, Option<TruncationInfo>) {
    let content = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    let total_bytes = bytes.len();

    // Check if truncation needed
    if total_bytes <= MAX_OUTPUT_BYTES && total_lines <= MAX_OUTPUT_LINES {
        return (content.to_string(), None);
    }

    // Truncate by lines first
    let truncated_by;
    let output_lines;

    if total_lines > MAX_OUTPUT_LINES {
        output_lines = MAX_OUTPUT_LINES;
        truncated_by = TruncatedBy::Lines;
    } else {
        // Truncate by bytes
        output_lines = total_lines;
        truncated_by = TruncatedBy::Bytes;
    }

    // Take last N lines
    let start_line = total_lines.saturating_sub(output_lines);
    let kept_lines = &lines[start_line..];
    let output = kept_lines.join("\n");

    // Further truncate by bytes if needed
    let final_output = if output.len() > MAX_OUTPUT_BYTES {
        output[output.len() - MAX_OUTPUT_BYTES..].to_string()
    } else {
        output
    };

    let truncation = TruncationInfo {
        total_bytes,
        total_lines,
        output_bytes: final_output.len(),
        output_lines: kept_lines.len(),
        full_output_path: None,
        truncated_by,
    };

    (final_output, Some(truncation))
}

/// Generate a unified diff between old and new content
fn generate_diff(old: &str, new: &str, path: &Path) -> String {
    let diff = similar::TextDiff::from_lines(old, new);

    let mut output = String::new();
    output.push_str(&format!("--- {}\n", path.display()));
    output.push_str(&format!("+++ {}\n", path.display()));

    for (idx, group) in diff.grouped_ops(3).iter().enumerate() {
        if idx > 0 {
            output.push_str("...\n");
        }

        for op in group {
            for change in diff.iter_changes(op) {
                let sign = match change.tag() {
                    similar::ChangeTag::Delete => "-",
                    similar::ChangeTag::Insert => "+",
                    similar::ChangeTag::Equal => " ",
                };
                output.push_str(sign);
                output.push_str(change.value());
                if change.missing_newline() {
                    output.push_str("\n\\ No newline at end of file\n");
                }
            }
        }
    }

    output
}
