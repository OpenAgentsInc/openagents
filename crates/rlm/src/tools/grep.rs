//! Pattern search tool for RLM environment.
//!
//! Searches for patterns across files in the repository, returning
//! SpanRefs for each match to enable provenance tracking.

use super::{RlmTool, ToolConfig, ToolError, ToolResult, get_current_commit};
use crate::span::SpanRef;
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

/// A single grep hit with SpanRef for provenance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrepHit {
    /// SpanRef pointing to the matching location.
    pub span: SpanRef,
    /// The matching line content.
    pub line: String,
    /// Preview context (lines before and after).
    pub preview: String,
    /// Match score (based on position, frequency).
    pub score: f32,
}

/// Pattern search tool.
///
/// Searches for regex patterns across files, returning matches with
/// SpanRefs for precise citation.
pub struct GrepTool {
    repo_root: PathBuf,
    config: ToolConfig,
}

impl GrepTool {
    /// Create a new GrepTool rooted at the given path.
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

    /// Search for a pattern in files matching the glob patterns.
    pub async fn search(
        &self,
        pattern: &str,
        globs: &[&str],
        max_hits: usize,
    ) -> ToolResult<Vec<GrepHit>> {
        let regex = Regex::new(pattern).map_err(|e| ToolError::InvalidPattern(e.to_string()))?;

        let commit = self
            .config
            .commit
            .clone()
            .or_else(|| get_current_commit(&self.repo_root));

        let mut hits = Vec::new();
        let files = self.collect_files(globs)?;

        for file_path in files {
            if hits.len() >= max_hits {
                break;
            }

            let relative_path = file_path
                .strip_prefix(&self.repo_root)
                .unwrap_or(&file_path)
                .to_string_lossy()
                .to_string();

            if let Ok(content) = fs::read_to_string(&file_path) {
                // Skip files that are too large
                if content.len() as u64 > self.config.max_file_size {
                    continue;
                }

                let file_hits = self.search_file(
                    &content,
                    &regex,
                    &relative_path,
                    commit.as_deref(),
                    max_hits - hits.len(),
                );
                hits.extend(file_hits);
            }
        }

        // Sort by score descending
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(hits)
    }

    /// Search within a single file's content.
    fn search_file(
        &self,
        content: &str,
        regex: &Regex,
        path: &str,
        commit: Option<&str>,
        max_hits: usize,
    ) -> Vec<GrepHit> {
        let lines: Vec<&str> = content.lines().collect();
        let mut hits = Vec::new();
        let mut byte_offset: u64 = 0;

        for (line_idx, line) in lines.iter().enumerate() {
            if hits.len() >= max_hits {
                break;
            }

            if regex.is_match(line) {
                let line_num = line_idx as u32 + 1;
                let line_bytes = line.len() as u64;

                // Build context preview (2 lines before/after)
                let context_start = line_idx.saturating_sub(2);
                let context_end = (line_idx + 3).min(lines.len());
                let preview: String = lines[context_start..context_end]
                    .iter()
                    .enumerate()
                    .map(|(i, l)| {
                        let num = context_start + i + 1;
                        if context_start + i == line_idx {
                            format!(">{:4}: {}", num, l)
                        } else {
                            format!(" {:4}: {}", num, l)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                let span = SpanRef::with_range(
                    SpanRef::generate_id(path, line_num, line_num),
                    path.to_string(),
                    line_num,
                    line_num,
                    byte_offset,
                    byte_offset + line_bytes,
                )
                .with_content(line);

                let span = if let Some(c) = commit {
                    span.with_commit(c)
                } else {
                    span
                };

                // Score based on match position (earlier = higher)
                let score = 1.0 - (line_idx as f32 / lines.len().max(1) as f32);

                hits.push(GrepHit {
                    span,
                    line: line.to_string(),
                    preview,
                    score,
                });
            }

            byte_offset += line.len() as u64 + 1; // +1 for newline
        }

        hits
    }

    /// Collect files matching glob patterns.
    fn collect_files(&self, globs: &[&str]) -> ToolResult<Vec<PathBuf>> {
        let mut files = Vec::new();

        for glob_pattern in globs {
            let full_pattern = self.repo_root.join(glob_pattern);
            let pattern_str = full_pattern.to_string_lossy();

            match glob::glob(&pattern_str) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        if entry.is_file() {
                            files.push(entry);
                        }
                    }
                }
                Err(e) => {
                    return Err(ToolError::InvalidPattern(format!(
                        "Invalid glob '{}': {}",
                        glob_pattern, e
                    )));
                }
            }
        }

        Ok(files)
    }
}

#[async_trait]
impl RlmTool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for a regex pattern across files. Returns matching lines with SpanRefs for citation."
    }

    fn args_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "paths": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Glob patterns for files to search (e.g., '**/*.rs')"
                },
                "max_hits": {
                    "type": "integer",
                    "description": "Maximum number of results to return",
                    "default": 20
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: Value) -> ToolResult<Value> {
        let pattern = args["pattern"]
            .as_str()
            .ok_or_else(|| ToolError::ParseError("Missing 'pattern' argument".to_string()))?;

        let paths: Vec<&str> = args["paths"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_else(|| vec!["**/*"]);

        let max_hits = args["max_hits"].as_u64().unwrap_or(20) as usize;

        let hits = self.search(pattern, &paths, max_hits).await?;

        Ok(json!({
            "hits": hits,
            "total": hits.len(),
            "truncated": hits.len() >= max_hits
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_grep_basic() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.rs");

        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "fn main() {{}}").unwrap();
        writeln!(file, "fn other() {{}}").unwrap();
        writeln!(file, "fn main_helper() {{}}").unwrap();

        let grep = GrepTool::new(temp.path().to_path_buf());
        let hits = grep.search("fn main", &["**/*.rs"], 10).await.unwrap();

        assert_eq!(hits.len(), 2); // main and main_helper
        assert!(hits[0].line.contains("main"));
    }

    #[tokio::test]
    async fn test_grep_context() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.txt");

        let mut file = fs::File::create(&file_path).unwrap();
        for i in 1..=10 {
            writeln!(file, "line {}", i).unwrap();
        }

        let grep = GrepTool::new(temp.path().to_path_buf());
        let hits = grep.search("line 5", &["**/*.txt"], 10).await.unwrap();

        assert_eq!(hits.len(), 1);
        assert!(hits[0].preview.contains("line 3"));
        assert!(hits[0].preview.contains("line 7"));
    }
}
