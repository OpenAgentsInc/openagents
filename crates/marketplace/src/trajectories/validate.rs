//! Trajectory quality validation
//!
//! Comprehensive quality scoring system for trajectory contributions with:
//!
//! - **Completeness scoring** (40% weight): Git commit correlation (initial → final)
//! - **Complexity scoring** (30% weight): Token count and tool call metrics
//! - **Reward signal scoring** (30% weight): CI/CD results as ground truth
//!
//! Quality scores range from 0.0 (unusable) to 1.0 (perfect). The validation
//! pipeline enforces minimum quality thresholds to ensure only valuable
//! trajectories are accepted into the marketplace.
//!
//! # Integration Points
//!
//! - Git commits extracted by `collect::extract_git_commits()`
//! - CI/CD signals detected by `collect::detect_ci_results()`
//! - Token counts and tool calls tracked during collection
//! - Enforced in `contribute::submit()` before submission
//! - Used by `RewardCalculator` for payment calculation

use super::TrajectorySession;
use serde::{Deserialize, Serialize};

/// Quality score for a trajectory (0.0 - 1.0)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct QualityScore(pub f64);

impl QualityScore {
    /// Create a new quality score
    pub fn new(score: f64) -> Self {
        Self(score.clamp(0.0, 1.0))
    }

    /// Get the raw score value
    pub fn value(&self) -> f64 {
        self.0
    }

    /// Check if score meets minimum threshold
    pub fn meets_threshold(&self, threshold: f64) -> bool {
        self.0 >= threshold
    }
}

/// Result of trajectory validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Overall quality score
    pub quality_score: QualityScore,

    /// Individual dimension scores
    pub completeness_score: f64,
    pub complexity_score: f64,
    pub reward_signal_score: f64,

    /// Validation passed
    pub passed: bool,

    /// Reasons for failure (if any)
    pub failure_reasons: Vec<String>,
}

/// Validate a trajectory session
pub fn validate_trajectory(session: &TrajectorySession, min_quality: f64) -> ValidationResult {
    let mut failure_reasons = Vec::new();

    // Completeness: does it have git commits?
    let completeness_score = calculate_completeness(session);
    if completeness_score < 0.3 {
        failure_reasons.push("Missing git commit correlation".to_string());
    }

    // Complexity: token count and tool calls
    let complexity_score = calculate_complexity(session);
    if complexity_score < 0.2 {
        failure_reasons.push("Trajectory too simple (low token/tool count)".to_string());
    }

    // Reward signal: CI/CD results
    let reward_signal_score = calculate_reward_signal(session);

    // Overall quality score (weighted average)
    let quality = QualityScore::new(
        completeness_score * 0.4 + complexity_score * 0.3 + reward_signal_score * 0.3,
    );

    let passed = quality.meets_threshold(min_quality) && failure_reasons.is_empty();

    ValidationResult {
        quality_score: quality,
        completeness_score,
        complexity_score,
        reward_signal_score,
        passed,
        failure_reasons,
    }
}

/// Calculate completeness score
fn calculate_completeness(session: &TrajectorySession) -> f64 {
    let mut score = 0.0;

    if session.initial_commit.is_some() {
        score += 0.4;
    }
    if session.final_commit.is_some() {
        score += 0.4;
    }
    if session.ended_at.is_some() {
        score += 0.2;
    }

    score
}

/// Calculate complexity score
fn calculate_complexity(session: &TrajectorySession) -> f64 {
    let mut score = 0.0;

    // Token count contributes up to 0.5
    if session.token_count > 100 {
        score += 0.2;
    }
    if session.token_count > 500 {
        score += 0.2;
    }
    if session.token_count > 2000 {
        score += 0.1;
    }

    // Tool calls contribute up to 0.5
    if session.tool_calls > 3 {
        score += 0.2;
    }
    if session.tool_calls > 10 {
        score += 0.2;
    }
    if session.tool_calls > 20 {
        score += 0.1;
    }

    score
}

/// Calculate reward signal score
fn calculate_reward_signal(session: &TrajectorySession) -> f64 {
    match session.ci_passed {
        Some(true) => 1.0,  // CI passed = perfect reward signal
        Some(false) => 0.3, // CI failed = still valuable (negative signal)
        None => 0.0,        // No CI data = no reward signal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_high_quality_trajectory() {
        let session = TrajectorySession {
            session_id: "test".to_string(),
            source: "codex".to_string(),
            path: "/tmp/test.rlog".into(),
            initial_commit: Some("abc123".to_string()),
            final_commit: Some("def456".to_string()),
            ci_passed: Some(true),
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            token_count: 3000,
            tool_calls: 25,
            quality_score: 0.0, // Will be calculated
        };

        let result = validate_trajectory(&session, 0.7);
        assert!(result.passed);
        assert!(result.quality_score.value() >= 0.7);
    }

    #[test]
    fn test_low_quality_trajectory() {
        let session = TrajectorySession {
            session_id: "test".to_string(),
            source: "codex".to_string(),
            path: "/tmp/test.rlog".into(),
            initial_commit: None,
            final_commit: None,
            ci_passed: None,
            started_at: Utc::now(),
            ended_at: None,
            token_count: 50,
            tool_calls: 1,
            quality_score: 0.0,
        };

        let result = validate_trajectory(&session, 0.5);
        assert!(!result.passed);
        assert!(!result.failure_reasons.is_empty());
    }
}
