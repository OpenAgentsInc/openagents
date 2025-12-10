//! File writing tool
//!
//! TOOL-010..013: Create and overwrite files

use crate::error::{ToolError, ToolResult};
use std::fs;
use std::path::Path;
use std::time::Instant;

/// Result of writing a file
#[derive(Debug, Clone)]
pub struct WriteResult {
    /// Resolved absolute path
    pub path: String,
    /// Bytes written
    pub bytes_written: usize,
    /// Whether file existed before
    pub existed_before: bool,
    /// Previous file size (if existed)
    pub previous_size: Option<u64>,
    /// New file size
    pub new_size: u64,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// File writing tool
///
/// TOOL-010: Create new files
/// TOOL-011: Overwrite existing files
/// TOOL-012: Create parent directories
/// TOOL-013: Return write metadata
pub struct WriteTool;

impl WriteTool {
    /// Write content to a file
    ///
    /// Creates the file if it doesn't exist, overwrites if it does.
    /// Creates parent directories as needed.
    ///
    /// # Arguments
    /// * `path` - Path to the file (supports ~ expansion)
    /// * `content` - Content to write
    ///
    /// # Returns
    /// WriteResult with metadata about the operation
    pub fn write(path: impl AsRef<Path>, content: &str) -> ToolResult<WriteResult> {
        let start = Instant::now();
        let path_str = path.as_ref().to_string_lossy().to_string();

        // Expand ~ to home directory
        let expanded = shellexpand::tilde(&path_str).to_string();
        let path = Path::new(&expanded);

        // Check if file exists and get previous size
        let existed_before = path.exists();
        let previous_size = if existed_before {
            fs::metadata(path).ok().map(|m| m.len())
        } else {
            None
        };

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| ToolError::io_error(format!("Failed to create directory: {}", e)))?;
            }
        }

        // Write the file
        let bytes = content.as_bytes();
        fs::write(path, bytes)
            .map_err(|e| ToolError::io_error(format!("Failed to write file: {}", e)))?;

        // Get new file size
        let new_size = fs::metadata(path)
            .map(|m| m.len())
            .unwrap_or(bytes.len() as u64);

        let resolved_path = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string();

        Ok(WriteResult {
            path: resolved_path,
            bytes_written: bytes.len(),
            existed_before,
            previous_size,
            new_size,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Append content to a file
    ///
    /// Creates the file if it doesn't exist.
    pub fn append(path: impl AsRef<Path>, content: &str) -> ToolResult<WriteResult> {
        let start = Instant::now();
        let path_str = path.as_ref().to_string_lossy().to_string();

        // Expand ~ to home directory
        let expanded = shellexpand::tilde(&path_str).to_string();
        let path = Path::new(&expanded);

        // Check if file exists and get previous size
        let existed_before = path.exists();
        let previous_size = if existed_before {
            fs::metadata(path).ok().map(|m| m.len())
        } else {
            None
        };

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| ToolError::io_error(format!("Failed to create directory: {}", e)))?;
            }
        }

        // Read existing content
        let existing = if existed_before {
            fs::read_to_string(path).unwrap_or_default()
        } else {
            String::new()
        };

        // Write combined content
        let combined = format!("{}{}", existing, content);
        let bytes = combined.as_bytes();
        fs::write(path, bytes)
            .map_err(|e| ToolError::io_error(format!("Failed to write file: {}", e)))?;

        let new_size = fs::metadata(path)
            .map(|m| m.len())
            .unwrap_or(bytes.len() as u64);

        let resolved_path = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string();

        Ok(WriteResult {
            path: resolved_path,
            bytes_written: content.len(),
            existed_before,
            previous_size,
            new_size,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_write_new_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");

        let result = WriteTool::write(&path, "hello world").unwrap();
        assert!(!result.existed_before);
        assert_eq!(result.bytes_written, 11);
        assert_eq!(result.new_size, 11);

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_overwrite_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");

        // Create initial file
        fs::write(&path, "original").unwrap();

        let result = WriteTool::write(&path, "new content").unwrap();
        assert!(result.existed_before);
        assert_eq!(result.previous_size, Some(8));
        assert_eq!(result.new_size, 11);

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "new content");
    }

    #[test]
    fn test_create_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a/b/c/test.txt");

        let result = WriteTool::write(&path, "nested").unwrap();
        assert!(!result.existed_before);

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "nested");
    }

    #[test]
    fn test_append() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");

        WriteTool::write(&path, "hello").unwrap();
        WriteTool::append(&path, " world").unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello world");
    }
}
