//! Line range reading tool for RLM environment.
//!
//! Reads specific line ranges from files, returning content with
//! SpanRefs for provenance tracking.

use super::{RlmTool, ToolConfig, ToolError, ToolResult, get_current_commit};
use crate::span::SpanRef;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

/// Result of reading a line range.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadResult {
    /// SpanRef for the read content.
    pub span: SpanRef,
    /// The content of the specified lines.
    pub content: String,
    /// Total lines in the file.
    pub total_lines: u32,
    /// Whether the range was truncated.
    pub truncated: bool,
}

/// Line range reading tool.
///
/// Reads specific line ranges from files, useful for examining
/// code sections identified by grep or other tools.
pub struct ReadLinesTool {
    repo_root: PathBuf,
    config: ToolConfig,
}

impl ReadLinesTool {
    /// Create a new ReadLinesTool rooted at the given path.
    pub fn new(repo_root: PathBuf) -> Self {
        Self {
            repo_root,
            config: ToolConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(repo_root: PathBuf, config: ToolConfig) -> Self {
        Self { repo_root, config }
    }

    /// Read lines from a file.
    ///
    /// Lines are 1-indexed, inclusive on both ends.
    pub async fn read(&self, path: &str, start_line: u32, end_line: u32) -> ToolResult<ReadResult> {
        let file_path = self.repo_root.join(path);

        if !file_path.exists() {
            return Err(ToolError::PathNotFound(path.to_string()));
        }

        let content = fs::read_to_string(&file_path).map_err(ToolError::Io)?;

        // Check file size
        if content.len() as u64 > self.config.max_file_size {
            return Err(ToolError::ExecutionError(format!(
                "File too large: {} bytes (max {})",
                content.len(),
                self.config.max_file_size
            )));
        }

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len() as u32;

        // Clamp line numbers to valid range
        let start = (start_line.max(1) - 1) as usize;
        let end = (end_line.min(total_lines) as usize).min(lines.len());

        // Check for maximum lines to return
        let max_lines = self.config.max_results;
        let truncated = end - start > max_lines;
        let actual_end = if truncated { start + max_lines } else { end };

        let selected_lines = &lines[start..actual_end];
        let selected_content = selected_lines.join("\n");

        // Calculate byte offsets
        let mut start_byte: u64 = 0;
        for line in lines.iter().take(start) {
            start_byte += line.len() as u64 + 1;
        }

        let mut end_byte = start_byte;
        for line in selected_lines {
            end_byte += line.len() as u64 + 1;
        }

        let commit = self
            .config
            .commit
            .clone()
            .or_else(|| get_current_commit(&self.repo_root));

        let span = SpanRef::with_range(
            SpanRef::generate_id(path, start_line, end_line),
            path.to_string(),
            start_line,
            actual_end as u32,
            start_byte,
            end_byte,
        )
        .with_content(&selected_content);

        let span = if let Some(c) = commit {
            span.with_commit(c)
        } else {
            span
        };

        Ok(ReadResult {
            span,
            content: selected_content,
            total_lines,
            truncated,
        })
    }

    /// Read an entire file.
    pub async fn read_all(&self, path: &str) -> ToolResult<ReadResult> {
        self.read(path, 1, u32::MAX).await
    }

    /// Read lines from an existing SpanRef (expand context).
    pub async fn read_span(&self, span: &SpanRef) -> ToolResult<ReadResult> {
        self.read(&span.path, span.start_line, span.end_line).await
    }

    /// Read with context around specified lines.
    pub async fn read_with_context(
        &self,
        path: &str,
        center_line: u32,
        context_lines: u32,
    ) -> ToolResult<ReadResult> {
        let start = center_line.saturating_sub(context_lines);
        let end = center_line.saturating_add(context_lines);
        self.read(path, start, end).await
    }
}

#[async_trait]
impl RlmTool for ReadLinesTool {
    fn name(&self) -> &str {
        "read_lines"
    }

    fn description(&self) -> &str {
        "Read specific line ranges from a file. Returns content with a SpanRef for citation."
    }

    fn args_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path relative to repository root"
                },
                "start_line": {
                    "type": "integer",
                    "description": "Starting line number (1-indexed, inclusive)",
                    "default": 1
                },
                "end_line": {
                    "type": "integer",
                    "description": "Ending line number (1-indexed, inclusive)",
                    "default": 100
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> ToolResult<Value> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| ToolError::ParseError("Missing 'path' argument".to_string()))?;

        let start_line = args["start_line"].as_u64().unwrap_or(1) as u32;

        let end_line = args["end_line"].as_u64().unwrap_or(100) as u32;

        let result = self.read(path, start_line, end_line).await?;

        Ok(json!(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_read_lines_basic() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.txt");

        let mut file = fs::File::create(&file_path).unwrap();
        for i in 1..=10 {
            writeln!(file, "line {}", i).unwrap();
        }

        let reader = ReadLinesTool::new(temp.path().to_path_buf());
        let result = reader.read("test.txt", 3, 5).await.unwrap();

        assert_eq!(result.span.start_line, 3);
        assert_eq!(result.span.end_line, 5);
        assert!(result.content.contains("line 3"));
        assert!(result.content.contains("line 5"));
        assert!(!result.content.contains("line 2"));
        assert!(!result.content.contains("line 6"));
    }

    #[tokio::test]
    async fn test_read_lines_bounds() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.txt");

        let mut file = fs::File::create(&file_path).unwrap();
        for i in 1..=5 {
            writeln!(file, "line {}", i).unwrap();
        }

        let reader = ReadLinesTool::new(temp.path().to_path_buf());

        // Request beyond file bounds
        let result = reader.read("test.txt", 1, 100).await.unwrap();
        assert_eq!(result.total_lines, 5);
        assert!(result.content.contains("line 5"));
    }

    #[tokio::test]
    async fn test_read_with_context() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.txt");

        let mut file = fs::File::create(&file_path).unwrap();
        for i in 1..=20 {
            writeln!(file, "line {}", i).unwrap();
        }

        let reader = ReadLinesTool::new(temp.path().to_path_buf());
        let result = reader.read_with_context("test.txt", 10, 2).await.unwrap();

        assert!(result.content.contains("line 8"));
        assert!(result.content.contains("line 10"));
        assert!(result.content.contains("line 12"));
    }

    #[tokio::test]
    async fn test_read_missing_file() {
        let temp = TempDir::new().unwrap();
        let reader = ReadLinesTool::new(temp.path().to_path_buf());

        let result = reader.read("nonexistent.txt", 1, 10).await;
        assert!(matches!(result, Err(ToolError::PathNotFound(_))));
    }
}
