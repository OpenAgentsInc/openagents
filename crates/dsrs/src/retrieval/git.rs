//! Git-based retrieval backend.
//!
//! Provides git-aware code navigation:
//! - Blame (who changed what)
//! - Log (commit history)
//! - Diff (recent changes)
//! - File history

use super::{RepoIndex, RetrievalConfig, RetrievalResult};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Command;

/// Git-based retrieval backend.
pub struct GitIndex {
    /// Root path of the repository.
    repo_path: PathBuf,

    /// Number of commits to consider.
    commit_limit: usize,

    /// Branch to search (default: current).
    branch: Option<String>,
}

impl GitIndex {
    /// Create a new git index for a repository.
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
            commit_limit: 100,
            branch: None,
        }
    }

    /// Set the commit limit.
    pub fn with_commit_limit(mut self, limit: usize) -> Self {
        self.commit_limit = limit;
        self
    }

    /// Set the branch to search.
    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// Search commit messages and changed files.
    fn search_commits(
        &self,
        query: &str,
        config: &RetrievalConfig,
    ) -> Result<Vec<RetrievalResult>> {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.repo_path);
        cmd.args([
            "log",
            "--all",
            "--pretty=format:%H|%s|%an|%ad",
            "--date=short",
            "--name-only",
            &format!("-{}", self.commit_limit),
            &format!("--grep={}", query),
        ]);

        let output = cmd.output().context("Failed to execute git log")?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut results = Vec::new();
        let mut current_commit: Option<(String, String, String, String)> = None;
        let mut current_files: Vec<String> = Vec::new();

        for line in stdout.lines() {
            if line.contains('|') {
                // Flush previous commit
                if let Some((hash, subject, author, date)) = current_commit.take() {
                    for file in &current_files {
                        results.push(
                            RetrievalResult::new(file, 1, 1, &subject)
                                .with_lane("git")
                                .with_score(0.8)
                                .with_metadata("commit", hash.clone())
                                .with_metadata("author", author.clone())
                                .with_metadata("date", date.clone()),
                        );
                    }
                }
                current_files.clear();

                // Parse new commit
                let parts: Vec<&str> = line.splitn(4, '|').collect();
                if parts.len() == 4 {
                    current_commit = Some((
                        parts[0].to_string(),
                        parts[1].to_string(),
                        parts[2].to_string(),
                        parts[3].to_string(),
                    ));
                }
            } else if !line.is_empty() {
                current_files.push(line.to_string());
            }
        }

        // Flush last commit
        if let Some((hash, subject, author, date)) = current_commit {
            for file in current_files {
                results.push(
                    RetrievalResult::new(&file, 1, 1, &subject)
                        .with_lane("git")
                        .with_score(0.8)
                        .with_metadata("commit", hash.clone())
                        .with_metadata("author", author.clone())
                        .with_metadata("date", date.clone()),
                );
            }
        }

        results.truncate(config.k);
        Ok(results)
    }

    /// Search git blame for author/recent changes.
    fn search_blame(&self, query: &str, config: &RetrievalConfig) -> Result<Vec<RetrievalResult>> {
        // First, find files containing the query
        let file_output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["grep", "-l", query])
            .output()
            .context("Failed to execute git grep")?;

        let stdout_str = String::from_utf8_lossy(&file_output.stdout).to_string();
        let files: Vec<&str> = stdout_str
            .lines()
            .take(10) // Limit files to process
            .collect();

        let mut results = Vec::new();

        for file in files {
            let blame_output = Command::new("git")
                .current_dir(&self.repo_path)
                .args(["blame", "--porcelain", file])
                .output()
                .context("Failed to execute git blame")?;

            let stdout = String::from_utf8_lossy(&blame_output.stdout);
            let mut current_line = 0;
            let mut current_author = String::new();
            let mut current_commit = String::new();

            for line in stdout.lines() {
                if line.starts_with("author ") {
                    current_author = line.strip_prefix("author ").unwrap_or("").to_string();
                } else if let Some(hash) = line.strip_prefix("author-time ") {
                    current_commit = hash.to_string();
                } else if line.starts_with('\t') {
                    current_line += 1;
                    let content = line.trim_start_matches('\t');
                    if content.to_lowercase().contains(&query.to_lowercase()) {
                        results.push(
                            RetrievalResult::new(file, current_line, current_line, content)
                                .with_lane("git")
                                .with_score(0.9)
                                .with_metadata("author", current_author.clone())
                                .with_metadata("commit", current_commit.clone()),
                        );
                    }
                }
            }
        }

        results.truncate(config.k);
        Ok(results)
    }

    /// Search recent diffs for changes.
    fn search_diffs(&self, query: &str, config: &RetrievalConfig) -> Result<Vec<RetrievalResult>> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args([
                "log",
                "-p",
                &format!("-{}", self.commit_limit.min(20)),
                &format!("-S{}", query), // Pickaxe: find commits that add/remove query
                "--pretty=format:COMMIT:%H",
            ])
            .output()
            .context("Failed to execute git log -p")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();
        let mut current_commit = String::new();
        let mut current_file = String::new();
        let mut in_diff = false;
        let mut line_num = 0;

        for line in stdout.lines() {
            if let Some(hash) = line.strip_prefix("COMMIT:") {
                current_commit = hash.to_string();
                in_diff = false;
            } else if let Some(file) = line.strip_prefix("+++ b/") {
                current_file = file.to_string();
                in_diff = true;
                line_num = 0;
            } else if line.starts_with("@@ ") {
                // Parse hunk header for line number
                if let Some(pos) = line.find('+') {
                    let rest = &line[pos + 1..];
                    if let Some(comma) = rest.find(',') {
                        line_num = rest[..comma].parse().unwrap_or(1);
                    } else if let Some(space) = rest.find(' ') {
                        line_num = rest[..space].parse().unwrap_or(1);
                    }
                }
            } else if in_diff && line.starts_with('+') && !line.starts_with("+++") {
                let content = line.trim_start_matches('+');
                if content.to_lowercase().contains(&query.to_lowercase()) {
                    results.push(
                        RetrievalResult::new(&current_file, line_num, line_num, content)
                            .with_lane("git")
                            .with_score(0.95)
                            .with_metadata("commit", current_commit.clone())
                            .with_metadata("change_type", "added".to_string()),
                    );
                }
                line_num += 1;
            } else if in_diff && !line.starts_with('-') {
                line_num += 1;
            }
        }

        results.truncate(config.k);
        Ok(results)
    }
}

#[async_trait]
impl RepoIndex for GitIndex {
    async fn query(&self, query: &str, config: &RetrievalConfig) -> Result<Vec<RetrievalResult>> {
        // Try multiple git search strategies
        let mut results = Vec::new();

        // 1. Search commit messages
        if let Ok(commit_results) = self.search_commits(query, config) {
            results.extend(commit_results);
        }

        // 2. Search blame (if we have room for more results)
        if results.len() < config.k
            && let Ok(blame_results) = self.search_blame(query, config)
        {
            results.extend(blame_results);
        }

        // 3. Search diffs (if still need more)
        if results.len() < config.k
            && let Ok(diff_results) = self.search_diffs(query, config)
        {
            results.extend(diff_results);
        }

        // Deduplicate by path+line
        let mut seen = std::collections::HashSet::new();
        results.retain(|r| seen.insert(format!("{}:{}", r.path, r.start_line)));

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results.truncate(config.k);
        Ok(results)
    }

    fn lane_name(&self) -> &str {
        "git"
    }

    fn supports_semantic(&self) -> bool {
        false
    }

    async fn is_available(&self) -> bool {
        Command::new("git")
            .current_dir(&self.repo_path)
            .args(["status"])
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
    async fn test_git_availability() {
        let index = GitIndex::new(env::current_dir().unwrap());
        let available = index.is_available().await;
        println!("Git available: {}", available);
    }

    #[test]
    fn test_git_index_builder() {
        let index = GitIndex::new(".").with_commit_limit(50).with_branch("main");

        assert_eq!(index.commit_limit, 50);
        assert_eq!(index.branch, Some("main".to_string()));
    }
}
