//! Grep/search tool
//!
//! TOOL-020..023: Pattern search in files

use crate::error::{ToolError, ToolResult};
use regex::Regex;
use std::fs;
use std::path::Path;
use std::time::Instant;
use walkdir::WalkDir;

/// A single match result
#[derive(Debug, Clone)]
pub struct GrepMatch {
    /// File path relative to search root
    pub file: String,
    /// Line number (1-indexed)
    pub line: usize,
    /// Line content
    pub text: String,
}

/// Result of a grep search
#[derive(Debug, Clone)]
pub struct GrepResult {
    /// Pattern searched for
    pub pattern: String,
    /// Search root path
    pub path: String,
    /// Resolved absolute path
    pub resolved_path: String,
    /// Whether search was case-insensitive
    pub ignore_case: bool,
    /// Maximum results limit
    pub max_results: Option<usize>,
    /// All matches found
    pub matches: Vec<GrepMatch>,
    /// Total files searched
    pub files_searched: usize,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Whether results were truncated
    pub truncated: bool,
}

/// Grep search tool
///
/// TOOL-020: Search file contents
/// TOOL-021: Support regex patterns
/// TOOL-022: Case-insensitive option
/// TOOL-023: Result limiting
pub struct GrepTool;

impl GrepTool {
    /// Search for a pattern in files
    ///
    /// # Arguments
    /// * `pattern` - Regex pattern to search for
    /// * `path` - Directory or file to search in
    /// * `max_results` - Maximum number of matches to return
    ///
    /// # Returns
    /// GrepResult with all matches and statistics
    pub fn search(
        pattern: &str,
        path: impl AsRef<Path>,
        max_results: Option<usize>,
    ) -> ToolResult<GrepResult> {
        Self::search_with_options(pattern, path, false, max_results)
    }

    /// Search with additional options
    pub fn search_with_options(
        pattern: &str,
        path: impl AsRef<Path>,
        ignore_case: bool,
        max_results: Option<usize>,
    ) -> ToolResult<GrepResult> {
        let start = Instant::now();
        let path_str = path.as_ref().to_string_lossy().to_string();

        // Expand ~ to home directory
        let expanded = shellexpand::tilde(&path_str).to_string();
        let path = Path::new(&expanded);

        // Check if path exists
        if !path.exists() {
            return Err(ToolError::not_found(format!(
                "Path not found: {}",
                path_str
            )));
        }

        // Compile regex
        let regex_pattern = if ignore_case {
            format!("(?i){}", pattern)
        } else {
            pattern.to_string()
        };

        let regex = Regex::new(&regex_pattern)
            .map_err(|e| ToolError::invalid_arguments(format!("Invalid regex pattern: {}", e)))?;

        let resolved_path = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string();

        let mut matches = Vec::new();
        let mut files_searched = 0;
        let max = max_results.unwrap_or(usize::MAX);

        // Search files
        if path.is_file() {
            files_searched = 1;
            Self::search_file(path, &path_str, &regex, &mut matches, max)?;
        } else {
            for entry in WalkDir::new(path)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let entry_path = entry.path();
                let relative_path = entry_path.strip_prefix(path).unwrap_or(entry_path);

                // Skip hidden files and directories (check relative path only)
                if Self::is_hidden_or_ignored(relative_path) {
                    continue;
                }

                // Skip binary files
                if Self::is_likely_binary(entry_path) {
                    continue;
                }

                files_searched += 1;
                let relative = relative_path.to_string_lossy().to_string();

                Self::search_file(entry_path, &relative, &regex, &mut matches, max)?;

                if matches.len() >= max {
                    break;
                }
            }
        }

        let truncated = matches.len() >= max;

        Ok(GrepResult {
            pattern: pattern.to_string(),
            path: path_str,
            resolved_path,
            ignore_case,
            max_results,
            matches,
            files_searched,
            duration_ms: start.elapsed().as_millis() as u64,
            truncated,
        })
    }

    /// Search a single file for matches
    fn search_file(
        path: &Path,
        relative_path: &str,
        regex: &Regex,
        matches: &mut Vec<GrepMatch>,
        max: usize,
    ) -> ToolResult<()> {
        // Try to read as text
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Ok(()), // Skip files that can't be read as text
        };

        for (line_num, line) in content.lines().enumerate() {
            if matches.len() >= max {
                break;
            }

            if regex.is_match(line) {
                matches.push(GrepMatch {
                    file: relative_path.to_string(),
                    line: line_num + 1,
                    text: line.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Check if a path should be ignored (hidden or common ignore patterns)
    fn is_hidden_or_ignored(path: &Path) -> bool {
        for component in path.components() {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') {
                    return true;
                }
                // Common ignore patterns
                if matches!(
                    name_str.as_ref(),
                    "node_modules" | "target" | "dist" | "build" | "__pycache__" | ".git"
                ) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a file is likely binary
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
                | "o"
                | "a"
                | "mp3"
                | "mp4"
                | "wav"
                | "avi"
                | "mov"
                | "ttf"
                | "otf"
                | "woff"
                | "woff2"
                | "lock"
                | "sum" // package lock files
        )
    }

    /// Search for literal text (not regex)
    pub fn search_literal(
        text: &str,
        path: impl AsRef<Path>,
        max_results: Option<usize>,
    ) -> ToolResult<GrepResult> {
        // Escape regex special characters
        let escaped = regex::escape(text);
        Self::search(&escaped, path, max_results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_files(dir: &TempDir) {
        let file1 = dir.path().join("file1.txt");
        fs::write(&file1, "hello world\nfoo bar\nhello again\n").unwrap();

        let file2 = dir.path().join("file2.txt");
        fs::write(&file2, "another file\nwith hello\nand more\n").unwrap();

        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();

        let file3 = subdir.join("file3.txt");
        fs::write(&file3, "nested hello\n").unwrap();
    }

    #[test]
    fn test_basic_search() {
        let dir = TempDir::new().unwrap();
        create_test_files(&dir);

        let result = GrepTool::search("hello", dir.path(), None).unwrap();
        assert_eq!(result.matches.len(), 4); // 2 in file1, 1 in file2, 1 in file3
        assert!(result.files_searched >= 3);
    }

    #[test]
    fn test_max_results() {
        let dir = TempDir::new().unwrap();
        create_test_files(&dir);

        let result = GrepTool::search("hello", dir.path(), Some(2)).unwrap();
        assert_eq!(result.matches.len(), 2);
        assert!(result.truncated);
    }

    #[test]
    fn test_case_insensitive() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "Hello HELLO hello\n").unwrap();

        let result = GrepTool::search_with_options("HELLO", dir.path(), true, None).unwrap();
        assert_eq!(result.matches.len(), 1); // One line matches
    }

    #[test]
    fn test_regex_pattern() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "foo123\nfoo456\nbar789\n").unwrap();

        let result = GrepTool::search(r"foo\d+", dir.path(), None).unwrap();
        assert_eq!(result.matches.len(), 2);
    }

    #[test]
    fn test_literal_search() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "test (parens)\ntest [brackets]\n").unwrap();

        let result = GrepTool::search_literal("(parens)", dir.path(), None).unwrap();
        assert_eq!(result.matches.len(), 1);
    }

    #[test]
    fn test_single_file_search() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "line 1\nline 2\nline 3\n").unwrap();

        let result = GrepTool::search("line", &file, None).unwrap();
        assert_eq!(result.matches.len(), 3);
        assert_eq!(result.files_searched, 1);
    }

    #[test]
    fn test_nonexistent_path() {
        let result = GrepTool::search("pattern", "/nonexistent/path", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_regex() {
        let dir = TempDir::new().unwrap();
        let result = GrepTool::search("[invalid", dir.path(), None);
        assert!(result.is_err());
    }
}
