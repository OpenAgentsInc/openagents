//! Trajectory collection from local AI coding assistant logs

use super::{TrajectoryConfig, TrajectorySession};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Supported trajectory sources
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrajectorySource {
    /// Codex Code trajectories
    CodexCode,
    /// Cursor trajectories
    Cursor,
    /// Codex trajectories
    Codex,
}

impl TrajectorySource {
    /// Get standard log directory for this source
    pub fn default_log_dir(&self) -> Option<PathBuf> {
        match self {
            Self::CodexCode => {
                // Try multiple locations for Codex Code logs
                // 1. Check docs/logs/ in current directory (OpenAgents project structure)
                let docs_logs = PathBuf::from("docs/logs");
                if docs_logs.exists() && docs_logs.is_dir() {
                    return Some(docs_logs);
                }

                // 2. Fall back to ~/.codex/logs
                let home = std::env::var("HOME").ok()?;
                Some(PathBuf::from(home).join(".codex/logs"))
            }
            Self::Cursor => {
                let home = std::env::var("HOME").ok()?;
                Some(PathBuf::from(home).join(".cursor/logs"))
            }
            Self::Codex => {
                let home = std::env::var("HOME").ok()?;
                Some(PathBuf::from(home).join(".codex/logs"))
            }
        }
    }

    /// Get the identifier string for this source
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CodexCode => "codex",
            Self::Cursor => "cursor",
            Self::Codex => "codex",
        }
    }
}

impl FromStr for TrajectorySource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "codex" => Ok(Self::CodexCode),
            "cursor" => Ok(Self::Cursor),
            "codex" => Ok(Self::Codex),
            _ => Err(format!("Unknown trajectory source: {}", s)),
        }
    }
}

/// Result of a trajectory scan operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    /// Source that was scanned
    pub source: TrajectorySource,

    /// Directory that was scanned
    pub scanned_path: PathBuf,

    /// Sessions found
    pub sessions: Vec<TrajectorySession>,

    /// Total sessions found
    pub session_count: usize,

    /// Errors encountered during scan
    pub errors: Vec<String>,
}

/// Trajectory collector for scanning local AI assistant logs
pub struct TrajectoryCollector {
    config: TrajectoryConfig,
}

impl TrajectoryCollector {
    /// Create a new collector with the given configuration
    pub fn new(config: TrajectoryConfig) -> Self {
        Self { config }
    }

    /// Scan all configured sources for trajectories
    pub fn scan_all(&self) -> Result<Vec<ScanResult>> {
        let mut results = Vec::new();

        for source_str in &self.config.sources {
            if let Ok(source) = source_str.parse::<TrajectorySource>() {
                match self.scan_source(&source) {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        eprintln!("Error scanning {}: {}", source.as_str(), e);
                    }
                }
            }
        }

        Ok(results)
    }

    /// Scan a specific source for trajectories
    pub fn scan_source(&self, source: &TrajectorySource) -> Result<ScanResult> {
        let log_dir = source
            .default_log_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine log directory"))?;

        self.scan_directory(source, &log_dir)
    }

    /// Scan a specific directory for trajectories
    pub fn scan_directory(&self, source: &TrajectorySource, dir: &Path) -> Result<ScanResult> {
        let mut sessions = Vec::new();
        let mut errors = Vec::new();

        if !dir.exists() {
            return Ok(ScanResult {
                source: source.clone(),
                scanned_path: dir.to_path_buf(),
                sessions: Vec::new(),
                session_count: 0,
                errors: vec![format!("Directory does not exist: {}", dir.display())],
            });
        }

        // Recursively scan for .rlog files (Codex Code format)
        self.scan_directory_recursive(dir, source, &mut sessions, &mut errors);

        let session_count = sessions.len();

        Ok(ScanResult {
            source: source.clone(),
            scanned_path: dir.to_path_buf(),
            sessions,
            session_count,
            errors,
        })
    }

    /// Recursively scan directory for trajectory files
    fn scan_directory_recursive(
        &self,
        dir: &Path,
        source: &TrajectorySource,
        sessions: &mut Vec<TrajectorySession>,
        errors: &mut Vec<String>,
    ) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.is_dir() {
                    // Recurse into subdirectories (e.g., date-based: docs/logs/20251221/)
                    self.scan_directory_recursive(&path, source, sessions, errors);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rlog") {
                    match self.parse_trajectory_file(source, &path) {
                        Ok(Some(session)) => sessions.push(session),
                        Ok(None) => {} // File didn't meet quality threshold
                        Err(e) => errors.push(format!("{}: {}", path.display(), e)),
                    }
                }
            }
        }
    }

    /// Parse a trajectory file and extract session metadata
    fn parse_trajectory_file(
        &self,
        source: &TrajectorySource,
        path: &Path,
    ) -> Result<Option<TrajectorySession>> {
        // Read file content
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        // Parse header metadata (YAML frontmatter)
        let (metadata, log_content) = parse_rlog_header(&content);

        // Extract session ID - prefer metadata, fallback to filename
        let session_id = metadata
            .get("id")
            .cloned()
            .or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());

        // Parse token counts from metadata
        let token_count = metadata
            .get("tokens_total_in")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or_else(|| {
                // Fall back to word count estimation
                log_content.split_whitespace().count()
            });

        // Parse tool calls - count actual tool invocations
        let tool_calls = count_tool_calls(&log_content);

        // Extract git commits from tool outputs
        let (initial_commit, final_commit) = extract_git_commits(&log_content);

        // Try to detect CI/CD results
        let ci_passed = detect_ci_results(&log_content);

        // Extract session timestamps
        let (started_at, ended_at) = extract_session_times(&log_content, &metadata);

        // Quality score based on completeness
        let quality_score = self.calculate_quality_score(
            initial_commit.is_some(),
            final_commit.is_some(),
            token_count,
            tool_calls,
        );

        // Filter by minimum quality score
        if quality_score < self.config.min_quality_score {
            return Ok(None);
        }

        // Filter by CI signal requirement
        if self.config.require_ci_signal && ci_passed.is_none() {
            return Ok(None);
        }

        Ok(Some(TrajectorySession {
            session_id,
            source: source.as_str().to_string(),
            path: path.to_path_buf(),
            initial_commit,
            final_commit,
            ci_passed,
            started_at,
            ended_at,
            token_count,
            tool_calls,
            quality_score,
        }))
    }

    /// Calculate quality score for a trajectory
    fn calculate_quality_score(
        &self,
        has_initial_commit: bool,
        has_final_commit: bool,
        token_count: usize,
        tool_calls: usize,
    ) -> f64 {
        let mut score: f64 = 0.0;

        // Bonus for git commit correlation
        if has_initial_commit {
            score += 0.3;
        }
        if has_final_commit {
            score += 0.3;
        }

        // Bonus for meaningful length
        if token_count > 100 {
            score += 0.2;
        }
        if token_count > 1000 {
            score += 0.1;
        }

        // Bonus for tool usage (indicates actual work done)
        if tool_calls > 5 {
            score += 0.1;
        }

        score.min(1.0)
    }
}

/// Parse YAML frontmatter from rlog file
/// Returns (metadata map, remaining log content)
fn parse_rlog_header(content: &str) -> (std::collections::HashMap<String, String>, String) {
    use std::collections::HashMap;

    let mut metadata = HashMap::new();
    let mut in_header = false;
    let mut log_start_idx = 0;

    // Look for YAML frontmatter (--- ... ---)
    for (idx, line) in content.lines().enumerate() {
        if line.trim() == "---" {
            if !in_header {
                in_header = true;
            } else {
                // End of header
                log_start_idx = idx + 1;
                break;
            }
        } else if in_header {
            // Parse key: value pairs
            if let Some((key, value)) = line.split_once(':') {
                metadata.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }

    let log_content = content
        .lines()
        .skip(log_start_idx)
        .collect::<Vec<_>>()
        .join("\n");

    (metadata, log_content)
}

/// Count actual tool calls from log content
fn count_tool_calls(content: &str) -> usize {
    // Count lines starting with "t!:" which indicate tool invocations
    content
        .lines()
        .filter(|line| line.trim_start().starts_with("t!:"))
        .count()
}

/// Extract git commits from bash tool outputs
fn extract_git_commits(content: &str) -> (Option<String>, Option<String>) {
    let mut all_commits = Vec::new();

    // Pattern 1: [branch commit_hash] message (from git commit output)
    // Example: "[main 40fc4f2e7] Add CLI commands"
    let commit_pattern1 = regex::Regex::new(r"\[[\w/-]+\s+([0-9a-f]{7,40})\]").unwrap();

    // Pattern 2: commit hash directly (from git log, git show)
    // Example: "commit 40fc4f2e7123abc..."
    let commit_pattern2 = regex::Regex::new(r"^commit\s+([0-9a-f]{40})").unwrap();

    // Pattern 3: git push range output (hash1..hash2)
    // Example: "   7663f22b1..816574027  main -> main"
    let commit_pattern3 = regex::Regex::new(r"([0-9a-f]{7,9})\.\.([0-9a-f]{7,9})").unwrap();

    for line in content.lines() {
        // Try pattern 1 first (most reliable - from git commit)
        if let Some(caps) = commit_pattern1.captures(line) {
            if let Some(hash) = caps.get(1) {
                all_commits.push(hash.as_str().to_string());
            }
        }
        // Try pattern 2 (git log format)
        else if let Some(caps) = commit_pattern2.captures(line) {
            if let Some(hash) = caps.get(1) {
                all_commits.push(hash.as_str().to_string());
            }
        }
        // Try pattern 3 (git push output)
        else if let Some(caps) = commit_pattern3.captures(line) {
            // Add both hashes from the range
            if let Some(from_hash) = caps.get(1) {
                all_commits.push(from_hash.as_str().to_string());
            }
            if let Some(to_hash) = caps.get(2) {
                all_commits.push(to_hash.as_str().to_string());
            }
        }
        // Pattern 4: specific contexts to avoid false positives
        else if line.contains("git") && line.contains("commit") {
            for word in line.split_whitespace() {
                if word.len() >= 7
                    && word.len() <= 40
                    && word.chars().all(|c| c.is_ascii_hexdigit())
                {
                    all_commits.push(word.to_string());
                }
            }
        }
    }

    // If we have commits, use first as initial, last as final
    match all_commits.len() {
        0 => (None, None),
        1 => (None, Some(all_commits[0].clone())), // Only final commit
        _ => {
            let initial = all_commits.first().cloned();
            let final_commit = all_commits.last().cloned();
            (initial, final_commit)
        }
    }
}

/// Detect CI/CD results from log content
fn detect_ci_results(content: &str) -> Option<bool> {
    let mut has_test_run = false;
    let mut tests_passed = false;
    let mut has_build = false;
    let mut build_passed = false;

    for line in content.lines() {
        let lower = line.to_lowercase();

        // Detect test execution
        if lower.contains("cargo test") || lower.contains("npm test") || lower.contains("pytest") {
            has_test_run = true;
        }

        // Detect test success
        if lower.contains("test result: ok") || lower.contains("all tests passed") {
            tests_passed = true;
        }

        // Detect test failure
        if lower.contains("test result: failed") || lower.contains("failures:") {
            tests_passed = false;
        }

        // Detect build execution
        if lower.contains("cargo build") || lower.contains("npm build") || lower.contains("make") {
            has_build = true;
        }

        // Detect build success
        if lower.contains("finished") && (lower.contains("dev") || lower.contains("release")) {
            build_passed = true;
        }

        // Check exit codes from bash
        if line.contains("Exit code") {
            if line.contains("Exit code 0") {
                build_passed = true;
            } else if line.contains("Exit code 1") || line.contains("Exit code 101") {
                tests_passed = false;
                build_passed = false;
            }
        }
    }

    // Return CI result if we have evidence
    if has_test_run {
        Some(tests_passed)
    } else if has_build {
        Some(build_passed)
    } else {
        None
    }
}

/// Extract session start and end timestamps
fn extract_session_times(
    content: &str,
    _metadata: &std::collections::HashMap<String, String>,
) -> (
    chrono::DateTime<chrono::Utc>,
    Option<chrono::DateTime<chrono::Utc>>,
) {
    use chrono::{DateTime, Utc};

    // Try to parse from @start and @end markers
    let mut start_time = None;
    let mut end_time = None;

    for line in content.lines() {
        // @start id=... ts=2025-12-20T01:13:33Z
        if line.starts_with("@start") {
            if let Some(ts_part) = line.split("ts=").nth(1) {
                if let Some(ts_str) = ts_part.split_whitespace().next() {
                    if let Ok(dt) = DateTime::parse_from_rfc3339(ts_str) {
                        start_time = Some(dt.with_timezone(&Utc));
                    }
                }
            }
        }

        // @end tokens_in=... tokens_out=... cost_usd=...
        if line.starts_with("@end") {
            // For end time, we'll use "now" as a proxy since it's not in the log
            end_time = Some(Utc::now());
        }
    }

    // Fall back to file modification time or current time
    let start = start_time.unwrap_or_else(Utc::now);

    (start, end_time)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_source_parsing() {
        assert_eq!(
            TrajectorySource::from_str("codex"),
            Ok(TrajectorySource::CodexCode)
        );
        assert_eq!(
            TrajectorySource::from_str("cursor"),
            Ok(TrajectorySource::Cursor)
        );
        assert_eq!(
            TrajectorySource::from_str("codex"),
            Ok(TrajectorySource::Codex)
        );
        assert!(TrajectorySource::from_str("unknown").is_err());
    }

    #[test]
    fn test_quality_score_calculation() {
        let collector = TrajectoryCollector::new(TrajectoryConfig::default());

        // High quality: has commits, lots of tokens, many tool calls
        let score = collector.calculate_quality_score(true, true, 2000, 10);
        assert!(score >= 0.9);

        // Low quality: no commits, few tokens, no tool calls
        let score = collector.calculate_quality_score(false, false, 50, 0);
        assert!(score < 0.5);
    }

    #[test]
    fn test_parse_rlog_header() {
        let content = r#"---
format: rlog/1
id: 303cc83b-1f79-40e2-91ac-95138e826d77
repo_sha: eb6bb683c
branch: main
model: codex-sonnet-4-5-20250929
tokens_total_in: 598
tokens_total_out: 6406
---

@start id=303cc83b ts=2025-12-20T01:13:33Z
Some log content here
"#;

        let (metadata, log_content) = parse_rlog_header(content);

        assert_eq!(
            metadata.get("id"),
            Some(&"303cc83b-1f79-40e2-91ac-95138e826d77".to_string())
        );
        assert_eq!(metadata.get("tokens_total_in"), Some(&"598".to_string()));
        assert_eq!(
            metadata.get("model"),
            Some(&"codex-sonnet-4-5-20250929".to_string())
        );
        assert!(log_content.contains("@start"));
        assert!(log_content.contains("Some log content"));
    }

    #[test]
    fn test_count_tool_calls() {
        let content = r#"
@start id=abc ts=2025-12-20T01:13:33Z
a: I'll implement this feature
t!:Read id=toolu_01 file_path=/some/path → [running]
o: id=toolu_01 → [ok] File contents here
t!:Edit id=toolu_02 file_path=/some/path → [running]
o: id=toolu_02 → [ok] File updated
t!:Bash id=toolu_03 cmd="cargo build" → [running]
o: id=toolu_03 → [ok] Finished build
"#;

        let count = count_tool_calls(content);
        assert_eq!(count, 3); // Read, Edit, Bash
    }

    #[test]
    fn test_extract_git_commits() {
        let content = r#"
t!:Bash id=abc cmd="git status" → [running]
o: id=abc → [ok] On branch main
t!:Bash id=def cmd="git commit -m 'test'" → [running]
o: id=def → [ok] [main 40fc4f2e7] Add CLI commands
t!:Bash id=ghi cmd="git push origin main" → [running]
o: id=ghi → [ok] To github.com:user/repo.git
   7663f22b1..816574027  main -> main
"#;

        let (initial, final_commit) = extract_git_commits(content);

        // Should extract both commits
        assert!(initial.is_some());
        assert!(final_commit.is_some());

        // Should extract the hashes
        let initial_hash = initial.unwrap();
        let final_hash = final_commit.unwrap();

        // Both should be valid hex strings
        assert!(initial_hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(final_hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_extract_single_commit() {
        let content = r#"
t!:Bash id=abc cmd="git commit -m 'Add feature'" → [running]
o: id=abc → [ok] [main 40fc4f2e7] Add feature
"#;

        let (initial, final_commit) = extract_git_commits(content);

        // With only one commit, initial should be None, final should be set
        assert!(initial.is_none());
        assert_eq!(final_commit, Some("40fc4f2e7".to_string()));
    }

    #[test]
    fn test_detect_ci_results_tests_passed() {
        let content = r#"
t!:Bash id=abc cmd="cargo test" → [running]
o: id=abc → [ok] running 5 tests
o: id=abc → [ok] test result: ok. 5 passed; 0 failed; 0 ignored
"#;

        let result = detect_ci_results(content);
        assert_eq!(result, Some(true));
    }

    #[test]
    fn test_detect_ci_results_tests_failed() {
        let content = r#"
t!:Bash id=abc cmd="cargo test" → [running]
o: id=abc → [error] Exit code 101
o: id=abc → [ok] test result: FAILED. 3 passed; 2 failed
"#;

        let result = detect_ci_results(content);
        assert_eq!(result, Some(false));
    }

    #[test]
    fn test_detect_ci_results_build_passed() {
        let content = r#"
t!:Bash id=abc cmd="cargo build --release" → [running]
o: id=abc → [ok]    Compiling my-crate v0.1.0
o: id=abc → [ok]     Finished release [optimized] target(s) in 5.23s
"#;

        let result = detect_ci_results(content);
        assert_eq!(result, Some(true));
    }

    #[test]
    fn test_detect_ci_results_no_ci() {
        let content = r#"
t!:Read id=abc file_path=/some/file → [running]
o: id=abc → [ok] File contents
t!:Edit id=def file_path=/some/file → [running]
o: id=def → [ok] File updated
"#;

        let result = detect_ci_results(content);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_session_times() {
        use std::collections::HashMap;

        let content = r#"
@start id=303cc83b ts=2025-12-20T01:13:33Z
@init model=codex-sonnet-4-5-20250929
Some content here
@end tokens_in=598 tokens_out=6406 cost_usd=0.4758
"#;

        let metadata = HashMap::new();
        let (start, end) = extract_session_times(content, &metadata);

        // Should parse start time correctly
        assert_eq!(start.to_rfc3339(), "2025-12-20T01:13:33+00:00");

        // Should detect end marker
        assert!(end.is_some());
    }
}
