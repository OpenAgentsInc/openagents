//! Directory traversal tool for RLM environment.
//!
//! Lists files matching glob patterns, providing metadata useful
//! for routing decisions.

use super::{RlmTool, ToolConfig, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

/// Information about a file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileInfo {
    /// File path relative to repository root.
    pub path: String,
    /// File size in bytes.
    pub size: u64,
    /// Detected language (based on extension).
    pub language: Option<String>,
    /// Number of lines (if computed).
    pub lines: Option<u32>,
    /// Whether the file is binary.
    pub is_binary: bool,
}

impl FileInfo {
    /// Detect language from file extension.
    pub fn detect_language(path: &str) -> Option<String> {
        let ext = path.rsplit('.').next()?;
        let lang = match ext.to_lowercase().as_str() {
            "rs" => "rust",
            "py" => "python",
            "js" => "javascript",
            "ts" => "typescript",
            "tsx" | "jsx" => "typescript-react",
            "go" => "go",
            "java" => "java",
            "c" | "h" => "c",
            "cpp" | "hpp" | "cc" | "cxx" => "cpp",
            "rb" => "ruby",
            "php" => "php",
            "swift" => "swift",
            "kt" | "kts" => "kotlin",
            "scala" => "scala",
            "cs" => "csharp",
            "fs" | "fsx" => "fsharp",
            "hs" => "haskell",
            "ml" | "mli" => "ocaml",
            "ex" | "exs" => "elixir",
            "erl" | "hrl" => "erlang",
            "clj" | "cljs" => "clojure",
            "lua" => "lua",
            "r" => "r",
            "jl" => "julia",
            "dart" => "dart",
            "zig" => "zig",
            "nim" => "nim",
            "v" => "vlang",
            "cr" => "crystal",
            "sh" | "bash" | "zsh" => "shell",
            "ps1" => "powershell",
            "sql" => "sql",
            "html" | "htm" => "html",
            "css" | "scss" | "sass" | "less" => "css",
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            "xml" => "xml",
            "md" | "markdown" => "markdown",
            "txt" => "text",
            _ => return None,
        };
        Some(lang.to_string())
    }

    /// Check if a file appears to be binary.
    pub fn is_binary_content(content: &[u8]) -> bool {
        // Check first 8KB for null bytes (common binary indicator)
        let check_len = content.len().min(8192);
        content[..check_len].contains(&0)
    }
}

/// Directory traversal tool.
///
/// Lists files matching glob patterns with metadata useful for
/// routing and navigation decisions.
pub struct ListFilesTool {
    repo_root: PathBuf,
    config: ToolConfig,
}

impl ListFilesTool {
    /// Create a new ListFilesTool rooted at the given path.
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

    /// List files matching a glob pattern.
    pub async fn list(&self, glob_pattern: &str) -> ToolResult<Vec<FileInfo>> {
        let full_pattern = self.repo_root.join(glob_pattern);
        let pattern_str = full_pattern.to_string_lossy();

        let entries = glob::glob(&pattern_str).map_err(|e| {
            ToolError::InvalidPattern(format!("Invalid glob '{}': {}", glob_pattern, e))
        })?;

        let mut files = Vec::new();

        for entry in entries {
            if files.len() >= self.config.max_results {
                break;
            }

            let path = entry.map_err(|e| ToolError::Io(e.into_error()))?;

            if !path.is_file() {
                continue;
            }

            let relative_path = path
                .strip_prefix(&self.repo_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            let metadata = fs::metadata(&path).map_err(ToolError::Io)?;
            let size = metadata.len();

            // Detect if binary by reading first bytes
            let is_binary = if size > 0 {
                let mut file = fs::File::open(&path).map_err(ToolError::Io)?;
                let mut buffer = vec![0u8; 8192.min(size as usize)];
                use std::io::Read;
                let _ = file.read(&mut buffer);
                FileInfo::is_binary_content(&buffer)
            } else {
                false
            };

            // Count lines for text files under size limit
            let lines = if !is_binary && size < self.config.max_file_size {
                fs::read_to_string(&path)
                    .ok()
                    .map(|content| content.lines().count() as u32)
            } else {
                None
            };

            let language = FileInfo::detect_language(&relative_path);

            files.push(FileInfo {
                path: relative_path,
                size,
                language,
                lines,
                is_binary,
            });
        }

        // Sort by path for deterministic output
        files.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(files)
    }

    /// List files by language.
    pub async fn list_by_language(&self, language: &str) -> ToolResult<Vec<FileInfo>> {
        // Map language to glob patterns
        let patterns: Vec<&str> = match language.to_lowercase().as_str() {
            "rust" => vec!["**/*.rs"],
            "python" => vec!["**/*.py"],
            "javascript" => vec!["**/*.js", "**/*.mjs", "**/*.cjs"],
            "typescript" => vec!["**/*.ts", "**/*.tsx"],
            "go" => vec!["**/*.go"],
            "java" => vec!["**/*.java"],
            "c" => vec!["**/*.c", "**/*.h"],
            "cpp" => vec!["**/*.cpp", "**/*.hpp", "**/*.cc", "**/*.cxx"],
            "markdown" => vec!["**/*.md", "**/*.markdown"],
            _ => {
                return Err(ToolError::InvalidPattern(format!(
                    "Unknown language: {}",
                    language
                )));
            }
        };

        let mut all_files = Vec::new();
        for pattern in patterns {
            let files = self.list(pattern).await?;
            all_files.extend(files);
        }

        // Deduplicate
        all_files.sort_by(|a, b| a.path.cmp(&b.path));
        all_files.dedup_by(|a, b| a.path == b.path);

        Ok(all_files)
    }

    /// Get summary statistics about the repository.
    pub async fn summary(&self) -> ToolResult<Value> {
        let all_files = self.list("**/*").await?;

        let mut by_language: std::collections::HashMap<String, (usize, u64)> =
            std::collections::HashMap::new();

        let mut total_lines = 0u64;
        let mut total_size = 0u64;
        let binary_count = all_files.iter().filter(|f| f.is_binary).count();

        for file in &all_files {
            total_size += file.size;
            if let Some(lines) = file.lines {
                total_lines += lines as u64;
            }

            let lang = file.language.clone().unwrap_or_else(|| "other".to_string());
            let entry = by_language.entry(lang).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += file.size;
        }

        Ok(json!({
            "total_files": all_files.len(),
            "total_size_bytes": total_size,
            "total_lines": total_lines,
            "binary_files": binary_count,
            "by_language": by_language.into_iter()
                .map(|(lang, (count, size))| json!({
                    "language": lang,
                    "count": count,
                    "size_bytes": size
                }))
                .collect::<Vec<_>>()
        }))
    }
}

#[async_trait]
impl RlmTool for ListFilesTool {
    fn name(&self) -> &str {
        "list_files"
    }

    fn description(&self) -> &str {
        "List files matching a glob pattern. Returns file paths with size and language metadata."
    }

    fn args_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to match files (e.g., '**/*.rs', 'src/**/*')",
                    "default": "**/*"
                },
                "language": {
                    "type": "string",
                    "description": "Filter by language (e.g., 'rust', 'python', 'typescript')"
                }
            }
        })
    }

    async fn execute(&self, args: Value) -> ToolResult<Value> {
        if let Some(language) = args["language"].as_str() {
            let files = self.list_by_language(language).await?;
            return Ok(json!({
                "files": files,
                "count": files.len()
            }));
        }

        let glob = args["glob"].as_str().unwrap_or("**/*");

        let files = self.list(glob).await?;

        Ok(json!({
            "files": files,
            "count": files.len()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_list_files_basic() {
        let temp = TempDir::new().unwrap();

        // Create some files
        fs::create_dir_all(temp.path().join("src")).unwrap();
        fs::File::create(temp.path().join("src/main.rs"))
            .unwrap()
            .write_all(b"fn main() {}\n")
            .unwrap();
        fs::File::create(temp.path().join("src/lib.rs"))
            .unwrap()
            .write_all(b"pub fn hello() {}\n")
            .unwrap();
        fs::File::create(temp.path().join("README.md"))
            .unwrap()
            .write_all(b"# Test\n")
            .unwrap();

        let tool = ListFilesTool::new(temp.path().to_path_buf());
        let files = tool.list("**/*.rs").await.unwrap();

        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.path.contains("main.rs")));
        assert!(files.iter().any(|f| f.path.contains("lib.rs")));
    }

    #[tokio::test]
    async fn test_language_detection() {
        assert_eq!(
            FileInfo::detect_language("foo.rs"),
            Some("rust".to_string())
        );
        assert_eq!(
            FileInfo::detect_language("bar.py"),
            Some("python".to_string())
        );
        assert_eq!(
            FileInfo::detect_language("baz.tsx"),
            Some("typescript-react".to_string())
        );
        assert_eq!(FileInfo::detect_language("unknown.xyz"), None);
    }

    #[tokio::test]
    async fn test_binary_detection() {
        // Text content
        let text = b"Hello, world!\nThis is text.\n";
        assert!(!FileInfo::is_binary_content(text));

        // Binary content (contains null bytes)
        let binary = b"\x00\x01\x02\x03";
        assert!(FileInfo::is_binary_content(binary));
    }

    #[tokio::test]
    async fn test_list_by_language() {
        let temp = TempDir::new().unwrap();

        fs::create_dir_all(temp.path().join("src")).unwrap();
        fs::File::create(temp.path().join("src/main.rs")).unwrap();
        fs::File::create(temp.path().join("src/lib.rs")).unwrap();
        fs::File::create(temp.path().join("script.py")).unwrap();

        let tool = ListFilesTool::new(temp.path().to_path_buf());

        let rust_files = tool.list_by_language("rust").await.unwrap();
        assert_eq!(rust_files.len(), 2);

        let python_files = tool.list_by_language("python").await.unwrap();
        assert_eq!(python_files.len(), 1);
    }
}
