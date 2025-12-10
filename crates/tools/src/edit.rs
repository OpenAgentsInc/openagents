//! File editing tool
//!
//! TOOL-010..013: Text replacement with diff output

use crate::error::{ToolError, ToolResult};
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::Path;

/// Result of editing a file
#[derive(Debug, Clone)]
pub struct EditResult {
    /// Resolved absolute path
    pub path: String,
    /// Unified diff showing the change
    pub diff: String,
    /// Original content length
    pub old_length: usize,
    /// New content length
    pub new_length: usize,
    /// Size delta (positive = grew, negative = shrunk)
    pub delta: i64,
    /// Number of lines added
    pub lines_added: usize,
    /// Number of lines removed
    pub lines_removed: usize,
    /// Number of replacements made
    pub replacements: usize,
}

/// File editing tool
///
/// Performs text replacement with validation and diff output.
pub struct EditTool;

impl EditTool {
    /// Edit a file by replacing text
    ///
    /// # Arguments
    /// * `path` - Path to the file
    /// * `old_text` - Text to find and replace
    /// * `new_text` - Replacement text
    /// * `replace_all` - If true, replace all occurrences; if false, require unique match
    ///
    /// # Returns
    /// EditResult with diff and statistics
    ///
    /// # Errors
    /// - `MissingOldText` if old_text not found
    /// - `NotUnique` if old_text matches multiple times and replace_all is false
    /// - `Unchanged` if old_text equals new_text
    pub fn edit(
        path: impl AsRef<Path>,
        old_text: &str,
        new_text: &str,
        replace_all: bool,
    ) -> ToolResult<EditResult> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        // Expand ~ to home directory
        let expanded = shellexpand::tilde(&path_str).to_string();
        let path = Path::new(&expanded);

        // Check if file exists
        if !path.exists() {
            return Err(ToolError::not_found(format!("File not found: {}", path_str)));
        }

        // Read current content
        let content = fs::read_to_string(path)
            .map_err(|e| ToolError::io_error(format!("Failed to read file: {}", e)))?;

        // Check for unchanged edit
        if old_text == new_text {
            return Err(ToolError::unchanged(
                "old_text and new_text are identical - no change would be made",
            ));
        }

        // Count occurrences
        let occurrences = content.matches(old_text).count();

        if occurrences == 0 {
            return Err(ToolError::missing_old_text(format!(
                "Text to replace not found in file. Searched for: {:?}",
                Self::truncate_for_error(old_text, 100)
            )));
        }

        if occurrences > 1 && !replace_all {
            return Err(ToolError::not_unique(format!(
                "Found {} occurrences of old_text. Use replace_all=true to replace all, or provide more context to make it unique.",
                occurrences
            )));
        }

        // Perform replacement
        let new_content = if replace_all {
            content.replace(old_text, new_text)
        } else {
            content.replacen(old_text, new_text, 1)
        };

        // Generate diff
        let diff = Self::generate_diff(&content, &new_content, &path_str);

        // Count line changes
        let (lines_added, lines_removed) = Self::count_line_changes(&content, &new_content);

        // Write the file
        fs::write(path, &new_content)
            .map_err(|e| ToolError::io_error(format!("Failed to write file: {}", e)))?;

        let resolved_path = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string();

        Ok(EditResult {
            path: resolved_path,
            diff,
            old_length: content.len(),
            new_length: new_content.len(),
            delta: new_content.len() as i64 - content.len() as i64,
            lines_added,
            lines_removed,
            replacements: if replace_all { occurrences } else { 1 },
        })
    }

    /// Generate a unified diff between old and new content
    fn generate_diff(old: &str, new: &str, filename: &str) -> String {
        let diff = TextDiff::from_lines(old, new);
        let mut output = String::new();

        output.push_str(&format!("--- a/{}\n", filename));
        output.push_str(&format!("+++ b/{}\n", filename));

        for (idx, group) in diff.grouped_ops(3).iter().enumerate() {
            if idx > 0 {
                output.push_str("\n");
            }

            // Calculate hunk range
            let mut old_start = 0;
            let mut old_count = 0;
            let mut new_start = 0;
            let mut new_count = 0;

            for op in group {
                match op {
                    similar::DiffOp::Equal {
                        old_index,
                        new_index,
                        len,
                    } => {
                        if old_start == 0 {
                            old_start = old_index + 1;
                            new_start = new_index + 1;
                        }
                        old_count += len;
                        new_count += len;
                    }
                    similar::DiffOp::Delete {
                        old_index,
                        old_len,
                        new_index,
                    } => {
                        if old_start == 0 {
                            old_start = old_index + 1;
                            new_start = new_index + 1;
                        }
                        old_count += old_len;
                    }
                    similar::DiffOp::Insert {
                        old_index,
                        new_index,
                        new_len,
                    } => {
                        if old_start == 0 {
                            old_start = old_index + 1;
                            new_start = new_index + 1;
                        }
                        new_count += new_len;
                    }
                    similar::DiffOp::Replace {
                        old_index,
                        old_len,
                        new_index,
                        new_len,
                    } => {
                        if old_start == 0 {
                            old_start = old_index + 1;
                            new_start = new_index + 1;
                        }
                        old_count += old_len;
                        new_count += new_len;
                    }
                }
            }

            output.push_str(&format!(
                "@@ -{},{} +{},{} @@\n",
                old_start, old_count, new_start, new_count
            ));

            // Output changes using iter_changes
            for op in group {
                for change in diff.iter_changes(op) {
                    let sign = match change.tag() {
                        ChangeTag::Equal => " ",
                        ChangeTag::Delete => "-",
                        ChangeTag::Insert => "+",
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

    /// Count lines added and removed
    fn count_line_changes(old: &str, new: &str) -> (usize, usize) {
        let diff = TextDiff::from_lines(old, new);
        let mut added = 0;
        let mut removed = 0;

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Insert => added += 1,
                ChangeTag::Delete => removed += 1,
                ChangeTag::Equal => {}
            }
        }

        (added, removed)
    }

    /// Truncate text for error messages
    fn truncate_for_error(text: &str, max_len: usize) -> String {
        if text.len() <= max_len {
            text.to_string()
        } else {
            format!("{}...", &text[..max_len])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_simple_edit() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "hello world").unwrap();

        let result = EditTool::edit(file.path(), "world", "rust", false).unwrap();
        assert_eq!(result.replacements, 1);

        let content = fs::read_to_string(file.path()).unwrap();
        assert_eq!(content.trim(), "hello rust");
    }

    #[test]
    fn test_replace_all() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "foo bar foo baz foo").unwrap();

        let result = EditTool::edit(file.path(), "foo", "qux", true).unwrap();
        assert_eq!(result.replacements, 3);

        let content = fs::read_to_string(file.path()).unwrap();
        assert_eq!(content.trim(), "qux bar qux baz qux");
    }

    #[test]
    fn test_not_unique_error() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "foo bar foo").unwrap();

        let result = EditTool::edit(file.path(), "foo", "baz", false);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().reason,
            crate::error::ToolErrorReason::NotUnique
        ));
    }

    #[test]
    fn test_missing_text_error() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "hello world").unwrap();

        let result = EditTool::edit(file.path(), "nonexistent", "replacement", false);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().reason,
            crate::error::ToolErrorReason::MissingOldText
        ));
    }

    #[test]
    fn test_unchanged_error() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "hello world").unwrap();

        let result = EditTool::edit(file.path(), "hello", "hello", false);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().reason,
            crate::error::ToolErrorReason::Unchanged
        ));
    }

    #[test]
    fn test_multiline_edit() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2").unwrap();
        writeln!(file, "line 3").unwrap();

        let result = EditTool::edit(file.path(), "line 2\n", "new line\nextra line\n", false).unwrap();
        assert_eq!(result.lines_added, 2);
        assert_eq!(result.lines_removed, 1);

        let content = fs::read_to_string(file.path()).unwrap();
        assert!(content.contains("new line"));
        assert!(content.contains("extra line"));
    }

    #[test]
    fn test_diff_generation() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "hello world").unwrap();

        let result = EditTool::edit(file.path(), "world", "rust", false).unwrap();
        assert!(result.diff.contains("-hello world"));
        assert!(result.diff.contains("+hello rust"));
    }
}
