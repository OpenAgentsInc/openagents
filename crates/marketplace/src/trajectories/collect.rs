//! Trajectory collection from local AI coding assistant logs

use super::{TrajectorySession, TrajectoryConfig};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;

/// Supported trajectory sources
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrajectorySource {
    /// Claude Code trajectories
    ClaudeCode,
    /// Cursor trajectories
    Cursor,
    /// Codex trajectories
    Codex,
}

impl TrajectorySource {
    /// Get standard log directory for this source
    pub fn default_log_dir(&self) -> Option<PathBuf> {
        let home = std::env::var("HOME").ok()?;
        Some(match self {
            Self::ClaudeCode => PathBuf::from(home).join(".claude/logs"),
            Self::Cursor => PathBuf::from(home).join(".cursor/logs"),
            Self::Codex => PathBuf::from(home).join(".codex/logs"),
        })
    }

    /// Get the identifier string for this source
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude",
            Self::Cursor => "cursor",
            Self::Codex => "codex",
        }
    }

    /// Parse from string identifier
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(Self::ClaudeCode),
            "cursor" => Some(Self::Cursor),
            "codex" => Some(Self::Codex),
            _ => None,
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
            if let Some(source) = TrajectorySource::from_str(source_str) {
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
        let log_dir = source.default_log_dir()
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

        // Scan for .rlog files (Claude Code format)
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("rlog") {
                    match self.parse_trajectory_file(source, &path) {
                        Ok(Some(session)) => sessions.push(session),
                        Ok(None) => {}, // File didn't meet quality threshold
                        Err(e) => errors.push(format!("{}: {}", path.display(), e)),
                    }
                }
            }
        }

        let session_count = sessions.len();

        Ok(ScanResult {
            source: source.clone(),
            scanned_path: dir.to_path_buf(),
            sessions,
            session_count,
            errors,
        })
    }

    /// Parse a trajectory file and extract session metadata
    fn parse_trajectory_file(&self, source: &TrajectorySource, path: &Path) -> Result<Option<TrajectorySession>> {
        // Read file content
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        // Extract session ID from filename
        let session_id = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Parse basic metrics from content
        // This is a simplified parser - real implementation would parse structured logs
        let token_count = content.split_whitespace().count(); // Rough approximation
        let tool_calls = content.matches("Tool:").count(); // Count tool call markers

        // Try to extract git commits (simplified - would need actual parsing)
        let initial_commit = extract_commit_hash(&content, "initial");
        let final_commit = extract_commit_hash(&content, "final");

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
        if self.config.require_ci_signal {
            // Would need to check for CI/CD results in logs
            // Simplified: assume we don't have CI data for now
            return Ok(None);
        }

        Ok(Some(TrajectorySession {
            session_id,
            source: source.as_str().to_string(),
            path: path.to_path_buf(),
            initial_commit,
            final_commit,
            ci_passed: None, // Would be extracted from logs
            started_at: chrono::Utc::now(), // Would be extracted from logs
            ended_at: None,
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
        if has_initial_commit { score += 0.3; }
        if has_final_commit { score += 0.3; }

        // Bonus for meaningful length
        if token_count > 100 { score += 0.2; }
        if token_count > 1000 { score += 0.1; }

        // Bonus for tool usage (indicates actual work done)
        if tool_calls > 5 { score += 0.1; }

        score.min(1.0)
    }
}

/// Extract a git commit hash from content (simplified)
fn extract_commit_hash(content: &str, marker: &str) -> Option<String> {
    // This is a placeholder - real implementation would parse structured logs
    // Look for patterns like "commit: abc123..." or "git commit abc123"
    for line in content.lines() {
        if line.contains(marker) && line.contains("commit") {
            // Extract what looks like a commit hash (40 hex chars)
            for word in line.split_whitespace() {
                if word.len() == 40 && word.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Some(word.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_source_parsing() {
        assert_eq!(TrajectorySource::from_str("claude"), Some(TrajectorySource::ClaudeCode));
        assert_eq!(TrajectorySource::from_str("cursor"), Some(TrajectorySource::Cursor));
        assert_eq!(TrajectorySource::from_str("codex"), Some(TrajectorySource::Codex));
        assert_eq!(TrajectorySource::from_str("unknown"), None);
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
}
