//! File reading tool
//!
//! TOOL-001..004: Read file contents with pagination

use crate::error::{ToolError, ToolResult};
use std::fs;
use std::path::Path;

/// Maximum lines to read by default
pub const MAX_LINES: usize = 2000;

/// Maximum characters per line before truncation
pub const MAX_LINE_LENGTH: usize = 2000;

/// Result of reading a file
#[derive(Debug, Clone)]
pub struct ReadResult {
    /// The file content (with line numbers if text)
    pub text: String,
    /// Resolved absolute path
    pub path: String,
    /// File size in bytes
    pub size_bytes: u64,
    /// Total lines in file (for text files)
    pub total_lines: Option<usize>,
    /// Starting line number (1-indexed)
    pub start_line: Option<usize>,
    /// Ending line number
    pub end_line: Option<usize>,
    /// Number of lines returned
    pub lines_returned: Option<usize>,
    /// Lines remaining after this read
    pub remaining_lines: Option<usize>,
    /// Number of truncated lines
    pub truncated_lines: usize,
    /// Whether output was truncated
    pub truncated: bool,
    /// MIME type (for binary files)
    pub mime_type: Option<String>,
}

/// File reading tool
///
/// TOOL-001: Read file contents
/// TOOL-002: Support line range reading
/// TOOL-003: Handle binary files
/// TOOL-004: Return file metadata
pub struct ReadTool;

impl ReadTool {
    /// Read a file's contents
    ///
    /// # Arguments
    /// * `path` - Path to the file (supports ~ expansion)
    /// * `offset` - Starting line number (1-indexed, default: 1)
    /// * `limit` - Maximum lines to read (default: MAX_LINES)
    ///
    /// # Returns
    /// ReadResult with content and metadata
    pub fn read(
        path: impl AsRef<Path>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> ToolResult<ReadResult> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        // Expand ~ to home directory
        let expanded = shellexpand::tilde(&path_str).to_string();
        let path = Path::new(&expanded);

        // Check if file exists
        if !path.exists() {
            return Err(ToolError::not_found(format!(
                "File not found: {}",
                path_str
            )));
        }

        // Get file metadata
        let metadata = fs::metadata(path)
            .map_err(|e| ToolError::io_error(format!("Failed to read metadata: {}", e)))?;

        if metadata.is_dir() {
            return Err(ToolError::invalid_arguments(format!(
                "Path is a directory, not a file: {}",
                path_str
            )));
        }

        let size_bytes = metadata.len();
        let resolved_path = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string();

        // Check if it's likely a binary file
        if Self::is_likely_binary(path) {
            return Self::read_binary(path, &resolved_path, size_bytes);
        }

        // Read as text
        Self::read_text(path, &resolved_path, size_bytes, offset, limit)
    }

    /// Read a text file with line numbers
    fn read_text(
        path: &Path,
        resolved_path: &str,
        size_bytes: u64,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> ToolResult<ReadResult> {
        let content = fs::read_to_string(path)
            .map_err(|e| ToolError::io_error(format!("Failed to read file: {}", e)))?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let offset = offset.unwrap_or(1).max(1);
        let limit = limit.unwrap_or(MAX_LINES);

        // Convert to 0-indexed
        let start_idx = offset.saturating_sub(1);
        let end_idx = (start_idx + limit).min(total_lines);

        let mut output = String::new();
        let mut truncated_count = 0;

        for (idx, line) in lines
            .iter()
            .enumerate()
            .skip(start_idx)
            .take(end_idx - start_idx)
        {
            let line_num = idx + 1;
            let line_num_width = total_lines.to_string().len();

            // Truncate long lines
            let display_line = if line.len() > MAX_LINE_LENGTH {
                truncated_count += 1;
                format!("{}...", &line[..MAX_LINE_LENGTH - 3])
            } else {
                line.to_string()
            };

            output.push_str(&format!(
                "{:>width$}\t{}\n",
                line_num,
                display_line,
                width = line_num_width
            ));
        }

        let lines_returned = end_idx - start_idx;
        let remaining = total_lines.saturating_sub(end_idx);

        Ok(ReadResult {
            text: output,
            path: resolved_path.to_string(),
            size_bytes,
            total_lines: Some(total_lines),
            start_line: Some(offset),
            end_line: Some(start_idx + lines_returned),
            lines_returned: Some(lines_returned),
            remaining_lines: Some(remaining),
            truncated_lines: truncated_count,
            truncated: remaining > 0 || truncated_count > 0,
            mime_type: None,
        })
    }

    /// Read a binary file (return base64 for images, error for others)
    fn read_binary(path: &Path, resolved_path: &str, size_bytes: u64) -> ToolResult<ReadResult> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Only handle image files
        let mime_type = match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            _ => {
                return Err(ToolError::invalid_arguments(format!(
                    "Cannot read binary file: {}",
                    path.display()
                )));
            }
        };

        // For now, just return metadata - actual base64 encoding would be added for full impl
        Ok(ReadResult {
            text: format!("[Binary file: {} ({} bytes)]", mime_type, size_bytes),
            path: resolved_path.to_string(),
            size_bytes,
            total_lines: None,
            start_line: None,
            end_line: None,
            lines_returned: None,
            remaining_lines: None,
            truncated_lines: 0,
            truncated: false,
            mime_type: Some(mime_type.to_string()),
        })
    }

    /// Check if a file is likely binary based on extension
    fn is_likely_binary(path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        matches!(
            ext.as_str(),
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "webp"
                | "ico"
                | "bmp"
                | "pdf"
                | "doc"
                | "docx"
                | "xls"
                | "xlsx"
                | "ppt"
                | "pptx"
                | "zip"
                | "tar"
                | "gz"
                | "bz2"
                | "7z"
                | "rar"
                | "exe"
                | "dll"
                | "so"
                | "dylib"
                | "mp3"
                | "mp4"
                | "wav"
                | "avi"
                | "mov"
                | "ttf"
                | "otf"
                | "woff"
                | "woff2"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2").unwrap();
        writeln!(file, "line 3").unwrap();

        let result = ReadTool::read(file.path(), None, None).unwrap();
        assert!(result.text.contains("line 1"));
        assert!(result.text.contains("line 2"));
        assert!(result.text.contains("line 3"));
        assert_eq!(result.total_lines, Some(3));
        assert_eq!(result.lines_returned, Some(3));
    }

    #[test]
    fn test_read_with_offset() {
        let mut file = NamedTempFile::new().unwrap();
        for i in 1..=10 {
            writeln!(file, "line {}", i).unwrap();
        }

        let result = ReadTool::read(file.path(), Some(5), Some(3)).unwrap();
        assert!(result.text.contains("line 5"));
        assert!(result.text.contains("line 6"));
        assert!(result.text.contains("line 7"));
        assert!(!result.text.contains("line 4"));
        assert!(!result.text.contains("line 8"));
        assert_eq!(result.start_line, Some(5));
        assert_eq!(result.lines_returned, Some(3));
        assert_eq!(result.remaining_lines, Some(3)); // lines 8, 9, 10
    }

    #[test]
    fn test_read_nonexistent() {
        let result = ReadTool::read("/nonexistent/path/file.txt", None, None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().reason,
            crate::error::ToolErrorReason::NotFound
        ));
    }

    #[test]
    fn test_read_directory() {
        let result = ReadTool::read("/tmp", None, None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().reason,
            crate::error::ToolErrorReason::InvalidArguments
        ));
    }

    #[test]
    fn test_line_truncation() {
        let mut file = NamedTempFile::new().unwrap();
        let long_line = "x".repeat(3000);
        writeln!(file, "{}", long_line).unwrap();

        let result = ReadTool::read(file.path(), None, None).unwrap();
        assert!(result.text.len() < 3000);
        assert!(result.text.contains("..."));
        assert_eq!(result.truncated_lines, 1);
    }
}
