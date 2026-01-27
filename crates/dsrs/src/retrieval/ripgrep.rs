//! Ripgrep-based text search backend.
//!
//! Uses regex patterns for fast text search across codebases.

use super::{RepoIndex, RetrievalConfig, RetrievalResult};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Command;

/// Ripgrep-based retrieval backend.
pub struct RipgrepIndex {
    /// Root path of the repository.
    repo_path: PathBuf,

    /// Whether to use case-insensitive matching.
    case_insensitive: bool,

    /// Whether to use regex mode.
    regex_mode: bool,

    /// File types to search (e.g., "rs", "ts").
    file_types: Vec<String>,
}

impl RipgrepIndex {
    /// Create a new ripgrep index for a repository.
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
            case_insensitive: false,
            regex_mode: false,
            file_types: vec![],
        }
    }

    /// Enable case-insensitive matching.
    pub fn with_case_insensitive(mut self, enabled: bool) -> Self {
        self.case_insensitive = enabled;
        self
    }

    /// Enable regex mode.
    pub fn with_regex(mut self, enabled: bool) -> Self {
        self.regex_mode = enabled;
        self
    }

    /// Filter to specific file types.
    pub fn with_file_types(mut self, types: Vec<String>) -> Self {
        self.file_types = types;
        self
    }

    /// Parse ripgrep JSON output into retrieval results.
    fn parse_rg_output(&self, output: &str, config: &RetrievalConfig) -> Vec<RetrievalResult> {
        let mut results = Vec::new();
        let mut current_file: Option<String> = None;
        let mut current_matches: Vec<(usize, String)> = Vec::new();

        for line in output.lines() {
            // Parse JSON output format
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                match json.get("type").and_then(|t| t.as_str()) {
                    Some("begin") => {
                        // New file
                        if let Some(path) = json["data"]["path"]["text"].as_str() {
                            // Flush previous file matches
                            if let Some(ref file) = current_file {
                                self.flush_matches(file, &current_matches, config, &mut results);
                            }
                            current_file = Some(path.to_string());
                            current_matches.clear();
                        }
                    }
                    Some("match") => {
                        // Match line
                        if let (Some(line_num), Some(text)) = (
                            json["data"]["line_number"].as_u64(),
                            json["data"]["lines"]["text"].as_str(),
                        ) {
                            current_matches.push((line_num as usize, text.to_string()));
                        }
                    }
                    Some("end") => {
                        // End of file
                        if let Some(ref file) = current_file {
                            self.flush_matches(file, &current_matches, config, &mut results);
                        }
                        current_file = None;
                        current_matches.clear();
                    }
                    _ => {}
                }
            }
        }

        // Handle non-JSON output (plain text mode)
        if results.is_empty() && !output.is_empty() {
            for line in output.lines() {
                // Format: path:line:content
                let parts: Vec<&str> = line.splitn(3, ':').collect();
                if parts.len() >= 3
                    && let Ok(line_num) = parts[1].parse::<usize>() {
                        results.push(
                            RetrievalResult::new(parts[0], line_num, line_num, parts[2])
                                .with_lane("ripgrep")
                                .with_score(1.0),
                        );
                    }
            }
        }

        // Limit to k results
        results.truncate(config.k);
        results
    }

    /// Flush accumulated matches for a file into results.
    fn flush_matches(
        &self,
        file: &str,
        matches: &[(usize, String)],
        config: &RetrievalConfig,
        results: &mut Vec<RetrievalResult>,
    ) {
        if matches.is_empty() {
            return;
        }

        // Group consecutive lines
        let mut groups: Vec<(usize, usize, Vec<String>)> = Vec::new();
        let context = config.context_lines;

        for (line_num, content) in matches {
            let can_merge = groups
                .last()
                .is_some_and(|(_, end, _)| *line_num <= *end + context + 1);

            if can_merge {
                let last = groups.last_mut().unwrap();
                last.1 = *line_num + context;
                last.2.push(content.clone());
            } else {
                groups.push((
                    line_num.saturating_sub(context),
                    *line_num + context,
                    vec![content.clone()],
                ));
            }
        }

        // Convert groups to results
        for (start, end, lines) in groups {
            let content = lines.join("");
            results.push(
                RetrievalResult::new(file, start.max(1), end, content)
                    .with_lane("ripgrep")
                    .with_score(1.0)
                    .with_metadata("match_count", lines.len().to_string()),
            );
        }
    }
}

#[async_trait]
impl RepoIndex for RipgrepIndex {
    async fn query(&self, query: &str, config: &RetrievalConfig) -> Result<Vec<RetrievalResult>> {
        let mut cmd = Command::new("rg");

        // JSON output for structured parsing
        cmd.arg("--json");

        // Context lines
        if config.context_lines > 0 {
            cmd.arg("-C").arg(config.context_lines.to_string());
        }

        // Case insensitivity
        if self.case_insensitive {
            cmd.arg("-i");
        }

        // Fixed string vs regex
        if !self.regex_mode {
            cmd.arg("-F");
        }

        // File types
        for ft in &self.file_types {
            cmd.arg("-t").arg(ft);
        }

        // Include patterns
        for pattern in &config.include_patterns {
            cmd.arg("-g").arg(pattern);
        }

        // Exclude patterns
        for pattern in &config.exclude_patterns {
            cmd.arg("-g").arg(format!("!{}", pattern));
        }

        // Max results (rg uses -m for max matches per file, but we'll filter after)
        cmd.arg("-m").arg("100");

        // Query and path
        cmd.arg(query);
        cmd.arg(&self.repo_path);

        let output = cmd.output().context("Failed to execute ripgrep")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(self.parse_rg_output(&stdout, config))
    }

    fn lane_name(&self) -> &str {
        "ripgrep"
    }

    fn supports_semantic(&self) -> bool {
        false
    }

    async fn is_available(&self) -> bool {
        Command::new("rg")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_ripgrep_availability() {
        let index = RipgrepIndex::new(env::current_dir().unwrap());
        // rg should be available in most dev environments
        let available = index.is_available().await;
        println!("Ripgrep available: {}", available);
    }

    #[test]
    fn test_retrieval_result_builder() {
        let result = RetrievalResult::new("src/main.rs", 10, 15, "fn main() {}")
            .with_score(0.95)
            .with_lane("ripgrep")
            .with_metadata("match_count", "3");

        assert_eq!(result.path, "src/main.rs");
        assert_eq!(result.start_line, 10);
        assert_eq!(result.end_line, 15);
        assert_eq!(result.score, 0.95);
        assert_eq!(result.lane, "ripgrep");
        assert_eq!(result.metadata.get("match_count"), Some(&"3".to_string()));
    }
}
