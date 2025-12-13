//! File finding tool
//!
//! TOOL-020..023: Find files by name pattern

use crate::error::{ToolError, ToolResult};
use globset::{Glob, GlobMatcher};
use std::path::Path;
use std::time::Instant;
use walkdir::WalkDir;

/// Result of a find operation
#[derive(Debug, Clone)]
pub struct FindResult {
    /// Search root path
    pub root: String,
    /// Resolved absolute path
    pub resolved_root: String,
    /// Pattern searched for
    pub pattern: Option<String>,
    /// Glob pattern (if used)
    pub glob: Option<String>,
    /// Maximum results limit
    pub max_results: Option<usize>,
    /// Whether hidden files were included
    pub include_hidden: bool,
    /// Found file paths (relative to root)
    pub files: Vec<String>,
    /// Number of matches
    pub matches: usize,
    /// Whether results were truncated
    pub truncated: bool,
    /// Total entries visited
    pub entries_visited: usize,
    /// Directories visited
    pub directories_visited: usize,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// File finding tool
///
/// Finds files by name pattern or glob.
pub struct FindTool;

impl FindTool {
    /// Find files matching a substring pattern
    ///
    /// # Arguments
    /// * `path` - Directory to search in
    /// * `pattern` - Substring to match in filename (case-insensitive)
    /// * `max_results` - Maximum number of results
    ///
    /// # Returns
    /// FindResult with matching file paths
    pub fn find(
        path: impl AsRef<Path>,
        pattern: Option<&str>,
        max_results: Option<usize>,
    ) -> ToolResult<FindResult> {
        Self::find_with_options(path, pattern, None, max_results, false)
    }

    /// Find files with a glob pattern
    ///
    /// # Arguments
    /// * `path` - Directory to search in
    /// * `glob` - Glob pattern (e.g., "**/*.rs")
    /// * `max_results` - Maximum number of results
    pub fn find_glob(
        path: impl AsRef<Path>,
        glob: &str,
        max_results: Option<usize>,
    ) -> ToolResult<FindResult> {
        Self::find_with_options(path, None, Some(glob), max_results, false)
    }

    /// Find with all options
    pub fn find_with_options(
        path: impl AsRef<Path>,
        pattern: Option<&str>,
        glob: Option<&str>,
        max_results: Option<usize>,
        include_hidden: bool,
    ) -> ToolResult<FindResult> {
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

        if !path.is_dir() {
            return Err(ToolError::invalid_arguments(format!(
                "Path is not a directory: {}",
                path_str
            )));
        }

        let resolved_root = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string();

        // Compile glob matcher if provided
        let glob_matcher: Option<GlobMatcher> = if let Some(g) = glob {
            Some(
                Glob::new(g)
                    .map_err(|e| {
                        ToolError::invalid_arguments(format!("Invalid glob pattern: {}", e))
                    })?
                    .compile_matcher(),
            )
        } else {
            None
        };

        let pattern_lower = pattern.map(|p| p.to_lowercase());

        let mut files = Vec::new();
        let mut entries_visited = 0;
        let mut directories_visited = 0;
        let max = max_results.unwrap_or(usize::MAX);

        for entry in WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            entries_visited += 1;

            if entry.file_type().is_dir() {
                directories_visited += 1;
                continue;
            }

            let entry_path = entry.path();
            let relative = entry_path.strip_prefix(path).unwrap_or(entry_path);

            // Skip hidden unless requested (check relative path only)
            if !include_hidden && Self::is_hidden_relative(relative) {
                continue;
            }

            let relative = relative.to_string_lossy().to_string();

            // Check glob match
            if let Some(ref matcher) = glob_matcher {
                if !matcher.is_match(&relative) {
                    continue;
                }
            }

            // Check substring match
            if let Some(ref p) = pattern_lower {
                let filename = entry_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();

                if !filename.contains(p) {
                    continue;
                }
            }

            files.push(relative);

            if files.len() >= max {
                break;
            }
        }

        let truncated = files.len() >= max;
        let matches = files.len();

        Ok(FindResult {
            root: path_str,
            resolved_root,
            pattern: pattern.map(String::from),
            glob: glob.map(String::from),
            max_results,
            include_hidden,
            files,
            matches,
            truncated,
            entries_visited,
            directories_visited,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Check if a relative path contains hidden components
    fn is_hidden_relative(path: &Path) -> bool {
        for component in path.components() {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') {
                    return true;
                }
            }
        }
        false
    }

    /// List directory contents (like ls)
    pub fn list(
        path: impl AsRef<Path>,
        recursive: bool,
        include_hidden: bool,
        max_results: Option<usize>,
    ) -> ToolResult<FindResult> {
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

        if !path.is_dir() {
            return Err(ToolError::invalid_arguments(format!(
                "Path is not a directory: {}",
                path_str
            )));
        }

        let resolved_root = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string();

        let mut files = Vec::new();
        let mut entries_visited = 0;
        let mut directories_visited = 0;
        let max = max_results.unwrap_or(usize::MAX);

        let walker = if recursive {
            WalkDir::new(path).follow_links(false)
        } else {
            WalkDir::new(path).follow_links(false).max_depth(1)
        };

        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            // Skip the root directory itself
            if entry.path() == path {
                continue;
            }

            entries_visited += 1;

            let relative_path = entry.path().strip_prefix(path).unwrap_or(entry.path());

            // Skip hidden unless requested (check relative path only)
            if !include_hidden && Self::is_hidden_relative(relative_path) {
                continue;
            }

            let is_dir = entry.file_type().is_dir();
            if is_dir {
                directories_visited += 1;
            }

            let mut relative = relative_path.to_string_lossy().to_string();

            // Append / to directories
            if is_dir {
                relative.push('/');
            }

            files.push(relative);

            if files.len() >= max {
                break;
            }
        }

        // Sort alphabetically
        files.sort();

        let truncated = files.len() >= max;
        let matches = files.len();

        Ok(FindResult {
            root: path_str,
            resolved_root,
            pattern: None,
            glob: None,
            max_results,
            include_hidden,
            files,
            matches,
            truncated,
            entries_visited,
            directories_visited,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_structure(dir: &TempDir) {
        fs::write(dir.path().join("file1.txt"), "").unwrap();
        fs::write(dir.path().join("file2.rs"), "").unwrap();
        fs::write(dir.path().join(".hidden"), "").unwrap();

        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("nested.txt"), "").unwrap();
        fs::write(subdir.join("nested.rs"), "").unwrap();
    }

    #[test]
    fn test_find_all() {
        let dir = TempDir::new().unwrap();
        create_test_structure(&dir);

        let result = FindTool::find(dir.path(), None, None).unwrap();
        assert_eq!(result.matches, 4); // All non-hidden files
    }

    #[test]
    fn test_find_by_pattern() {
        let dir = TempDir::new().unwrap();
        create_test_structure(&dir);

        let result = FindTool::find(dir.path(), Some("txt"), None).unwrap();
        assert_eq!(result.matches, 2); // file1.txt and nested.txt
    }

    #[test]
    fn test_find_by_glob() {
        let dir = TempDir::new().unwrap();
        create_test_structure(&dir);

        let result = FindTool::find_glob(dir.path(), "**/*.rs", None).unwrap();
        assert_eq!(result.matches, 2); // file2.rs and nested.rs
    }

    #[test]
    fn test_find_with_max() {
        let dir = TempDir::new().unwrap();
        create_test_structure(&dir);

        let result = FindTool::find(dir.path(), None, Some(2)).unwrap();
        assert_eq!(result.matches, 2);
        assert!(result.truncated);
    }

    #[test]
    fn test_find_include_hidden() {
        let dir = TempDir::new().unwrap();
        create_test_structure(&dir);

        let result = FindTool::find_with_options(dir.path(), None, None, None, true).unwrap();
        assert_eq!(result.matches, 5); // Including .hidden
    }

    #[test]
    fn test_list_non_recursive() {
        let dir = TempDir::new().unwrap();
        create_test_structure(&dir);

        let result = FindTool::list(dir.path(), false, false, None).unwrap();
        // file1.txt, file2.rs, subdir/
        assert_eq!(result.matches, 3);
    }

    #[test]
    fn test_list_recursive() {
        let dir = TempDir::new().unwrap();
        create_test_structure(&dir);

        let result = FindTool::list(dir.path(), true, false, None).unwrap();
        // All files and directories except hidden
        assert!(result.matches >= 5);
    }

    #[test]
    fn test_nonexistent_path() {
        let result = FindTool::find("/nonexistent/path", None, None);
        assert!(result.is_err());
    }
}
