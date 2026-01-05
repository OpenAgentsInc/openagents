//! Context loading for RLM.
//!
//! Loads files and directories into a context variable that the model
//! can access and query in the REPL environment.

use std::path::Path;

use crate::error::{Result, RlmError};

/// Loaded context with metadata.
#[derive(Debug, Clone)]
pub struct Context {
    /// The full context content.
    pub content: String,
    /// Total character count.
    pub length: usize,
    /// Source description (file path, directory, etc.).
    pub source: String,
    /// Type of context (file, directory, text).
    pub context_type: ContextType,
    /// Number of files if loaded from directory.
    pub file_count: Option<usize>,
    /// Individual file entries (for directory loads).
    pub files: Vec<FileEntry>,
}

/// Type of context source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextType {
    /// Single file.
    File,
    /// Directory of files.
    Directory,
    /// Raw text input.
    Text,
}

impl std::fmt::Display for ContextType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextType::File => write!(f, "file"),
            ContextType::Directory => write!(f, "directory"),
            ContextType::Text => write!(f, "text"),
        }
    }
}

/// Entry for a file loaded from a directory.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Relative path from the root.
    pub path: String,
    /// Start index in the context string.
    pub start_index: usize,
    /// End index in the context string.
    pub end_index: usize,
    /// File size in characters.
    pub size: usize,
}

impl Context {
    /// Create a context from raw text.
    pub fn from_text(content: impl Into<String>) -> Self {
        let content = content.into();
        let length = content.len();
        Self {
            content,
            length,
            source: "text input".to_string(),
            context_type: ContextType::Text,
            file_count: None,
            files: vec![],
        }
    }

    /// Load context from a file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(RlmError::ContextError(format!(
                "File not found: {}",
                path.display()
            )));
        }

        if !path.is_file() {
            return Err(RlmError::ContextError(format!(
                "Not a file: {}",
                path.display()
            )));
        }

        let content = std::fs::read_to_string(path).map_err(|e| {
            RlmError::ContextError(format!("Failed to read file {}: {}", path.display(), e))
        })?;

        let length = content.len();

        Ok(Self {
            content,
            length,
            source: path.display().to_string(),
            context_type: ContextType::File,
            file_count: Some(1),
            files: vec![FileEntry {
                path: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                start_index: 0,
                end_index: length,
                size: length,
            }],
        })
    }

    /// Load context from a directory.
    ///
    /// Recursively reads all text files and concatenates them with markers.
    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(RlmError::ContextError(format!(
                "Directory not found: {}",
                path.display()
            )));
        }

        if !path.is_dir() {
            return Err(RlmError::ContextError(format!(
                "Not a directory: {}",
                path.display()
            )));
        }

        let mut content = String::new();
        let mut files = Vec::new();
        let mut file_count = 0;

        Self::load_directory_recursive(path, path, &mut content, &mut files, &mut file_count)?;

        let length = content.len();

        Ok(Self {
            content,
            length,
            source: path.display().to_string(),
            context_type: ContextType::Directory,
            file_count: Some(file_count),
            files,
        })
    }

    /// Recursively load files from a directory.
    fn load_directory_recursive(
        root: &Path,
        current: &Path,
        content: &mut String,
        files: &mut Vec<FileEntry>,
        file_count: &mut usize,
    ) -> Result<()> {
        let entries = std::fs::read_dir(current).map_err(|e| {
            RlmError::ContextError(format!("Failed to read directory {}: {}", current.display(), e))
        })?;

        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();

        // Sort for deterministic ordering
        paths.sort();

        for entry_path in paths {
            // Skip hidden files and common non-text directories
            let file_name = entry_path.file_name().unwrap_or_default().to_string_lossy();
            if file_name.starts_with('.')
                || file_name == "node_modules"
                || file_name == "target"
                || file_name == "__pycache__"
                || file_name == ".git"
            {
                continue;
            }

            if entry_path.is_dir() {
                Self::load_directory_recursive(root, &entry_path, content, files, file_count)?;
            } else if entry_path.is_file() {
                // Check if it's a text file by extension
                if Self::is_text_file(&entry_path) {
                    match std::fs::read_to_string(&entry_path) {
                        Ok(file_content) => {
                            let relative_path = entry_path
                                .strip_prefix(root)
                                .unwrap_or(&entry_path)
                                .display()
                                .to_string();

                            let start_index = content.len();

                            // Add file header
                            content.push_str(&format!("\n### FILE: {} ###\n", relative_path));
                            content.push_str(&file_content);
                            content.push_str("\n### END FILE ###\n");

                            let end_index = content.len();
                            let size = end_index - start_index;

                            files.push(FileEntry {
                                path: relative_path,
                                start_index,
                                end_index,
                                size,
                            });

                            *file_count += 1;
                        }
                        Err(_) => {
                            // Skip binary or unreadable files
                            continue;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a file is likely a text file based on extension.
    fn is_text_file(path: &Path) -> bool {
        let text_extensions = [
            "rs", "py", "js", "ts", "tsx", "jsx", "json", "toml", "yaml", "yml",
            "md", "txt", "html", "css", "scss", "sass", "less", "xml", "svg",
            "sh", "bash", "zsh", "fish", "c", "cpp", "h", "hpp", "go", "java",
            "kt", "swift", "rb", "php", "sql", "graphql", "proto", "dockerfile",
            "makefile", "cmake", "gradle", "properties", "env", "gitignore",
            "editorconfig", "prettierrc", "eslintrc", "lock",
        ];

        // Files without extension that are typically text
        let text_filenames = [
            "Makefile", "Dockerfile", "Cargo.toml", "package.json", "README",
            "LICENSE", "CHANGELOG", "Gemfile", "Rakefile", ".gitignore",
            ".env", ".env.example",
        ];

        if let Some(name) = path.file_name() {
            let name_str = name.to_string_lossy().to_lowercase();
            if text_filenames.iter().any(|f| name_str == f.to_lowercase()) {
                return true;
            }
        }

        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            return text_extensions.contains(&ext_str.as_str());
        }

        false
    }

    /// Generate a summary of the context for the system prompt.
    pub fn summary(&self) -> String {
        let mut summary = format!(
            "Context loaded from {} ({}):\n",
            self.source, self.context_type
        );
        summary.push_str(&format!("- Total length: {} characters\n", self.length));

        if let Some(count) = self.file_count {
            summary.push_str(&format!("- Files: {}\n", count));
        }

        if !self.files.is_empty() && self.files.len() <= 20 {
            summary.push_str("- File list:\n");
            for file in &self.files {
                summary.push_str(&format!(
                    "  - {} ({} chars, index {}..{})\n",
                    file.path, file.size, file.start_index, file.end_index
                ));
            }
        } else if !self.files.is_empty() {
            summary.push_str(&format!(
                "- Files: {} (use get_fragments() to list)\n",
                self.files.len()
            ));
        }

        summary
    }

    /// Get a slice of the context.
    pub fn slice(&self, start: usize, end: usize) -> &str {
        let start = start.min(self.length);
        let end = end.min(self.length);
        &self.content[start..end]
    }

    /// Search for a pattern in the context and return matches with surrounding context.
    pub fn search(&self, pattern: &str, max_results: usize, window: usize) -> Vec<SearchResult> {
        let pattern_lower = pattern.to_lowercase();
        let content_lower = self.content.to_lowercase();

        let mut results = Vec::new();
        let mut search_start = 0;

        while let Some(pos) = content_lower[search_start..].find(&pattern_lower) {
            let absolute_pos = search_start + pos;
            let context_start = absolute_pos.saturating_sub(window);
            let context_end = (absolute_pos + pattern.len() + window).min(self.length);

            results.push(SearchResult {
                position: absolute_pos,
                context: self.content[context_start..context_end].to_string(),
                file: self.find_file_for_position(absolute_pos),
            });

            if results.len() >= max_results {
                break;
            }

            search_start = absolute_pos + 1;
        }

        results
    }

    /// Find which file contains a given position.
    fn find_file_for_position(&self, pos: usize) -> Option<String> {
        for file in &self.files {
            if pos >= file.start_index && pos < file.end_index {
                return Some(file.path.clone());
            }
        }
        None
    }
}

/// Result from a context search.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Position in the context where the match was found.
    pub position: usize,
    /// Surrounding context around the match.
    pub context: String,
    /// File containing the match (if from directory).
    pub file: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_from_text() {
        let ctx = Context::from_text("Hello, world!");
        assert_eq!(ctx.length, 13);
        assert_eq!(ctx.context_type, ContextType::Text);
    }

    #[test]
    fn test_context_slice() {
        let ctx = Context::from_text("Hello, world!");
        assert_eq!(ctx.slice(0, 5), "Hello");
        assert_eq!(ctx.slice(7, 12), "world");
    }

    #[test]
    fn test_context_search() {
        let ctx = Context::from_text("The quick brown fox jumps over the lazy dog. The fox is quick.");
        let results = ctx.search("fox", 10, 10);
        assert_eq!(results.len(), 2);
        assert!(results[0].context.contains("fox"));
    }

    #[test]
    fn test_is_text_file() {
        assert!(Context::is_text_file(Path::new("test.rs")));
        assert!(Context::is_text_file(Path::new("test.py")));
        assert!(Context::is_text_file(Path::new("Makefile")));
        assert!(!Context::is_text_file(Path::new("test.png")));
        assert!(!Context::is_text_file(Path::new("test.exe")));
    }
}
