//! Free Analysis Pipeline
//!
//! Provides free repository analysis for the demo funnel.
//! Uses ComputeBuyer to request analysis jobs from the marketplace.

use std::collections::HashMap;
use std::time::SystemTime;

use compute::domain::{IndexType, RepoIndexRequest};

use crate::compute::{ComputeBuyer, ComputeBuyerConfig};
use crate::github::models::ConnectedRepo;

pub mod runner;
pub mod trial;

pub use runner::{AnalysisReport, FreeAnalysisRunner};
pub use trial::{TrialRun, TrialRunConfig, TrialRunResult};

/// Language detected in a repository
#[derive(Debug, Clone)]
pub struct DetectedLanguage {
    /// Language name
    pub name: String,
    /// Percentage of codebase (0-100)
    pub percentage: f32,
    /// Number of files
    pub file_count: u32,
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheck {
    /// Check name (e.g., "cargo check", "npm build")
    pub name: String,
    /// Whether the check passed
    pub passed: bool,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Error message if failed
    pub error: Option<String>,
    /// Warnings detected
    pub warnings: Vec<String>,
}

impl HealthCheck {
    /// Create a passed health check
    pub fn passed(name: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            name: name.into(),
            passed: true,
            duration_ms,
            error: None,
            warnings: Vec::new(),
        }
    }

    /// Create a failed health check
    pub fn failed(name: impl Into<String>, duration_ms: u64, error: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: false,
            duration_ms,
            error: Some(error.into()),
            warnings: Vec::new(),
        }
    }

    /// Add warnings
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }
}

/// Recommendation for the repository
#[derive(Debug, Clone)]
pub struct Recommendation {
    /// Recommendation category
    pub category: RecommendationCategory,
    /// Priority level
    pub priority: Priority,
    /// Short title
    pub title: String,
    /// Detailed description
    pub description: String,
    /// Estimated effort (optional)
    pub effort: Option<String>,
}

/// Category of recommendation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecommendationCategory {
    /// Code quality improvements
    Quality,
    /// Security recommendations
    Security,
    /// Performance optimizations
    Performance,
    /// Documentation improvements
    Documentation,
    /// Test coverage
    Testing,
    /// Dependency updates
    Dependencies,
    /// Configuration improvements
    Configuration,
}

impl RecommendationCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecommendationCategory::Quality => "quality",
            RecommendationCategory::Security => "security",
            RecommendationCategory::Performance => "performance",
            RecommendationCategory::Documentation => "documentation",
            RecommendationCategory::Testing => "testing",
            RecommendationCategory::Dependencies => "dependencies",
            RecommendationCategory::Configuration => "configuration",
        }
    }
}

/// Priority level
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Low => "low",
            Priority::Medium => "medium",
            Priority::High => "high",
            Priority::Critical => "critical",
        }
    }
}

/// Repository statistics
#[derive(Debug, Clone, Default)]
pub struct RepoStats {
    /// Total number of files
    pub total_files: u32,
    /// Total lines of code
    pub total_lines: u64,
    /// Number of symbols (functions, classes, etc.)
    pub symbol_count: u32,
    /// Number of dependencies
    pub dependency_count: u32,
    /// Test file count
    pub test_file_count: u32,
    /// Documentation file count
    pub doc_file_count: u32,
}

/// Suggested issue for the repository
#[derive(Debug, Clone)]
pub struct SuggestedIssue {
    /// Issue title
    pub title: String,
    /// Issue body/description
    pub body: String,
    /// Suggested labels
    pub labels: Vec<String>,
    /// Estimated complexity
    pub complexity: IssueComplexity,
    /// Affected files
    pub affected_files: Vec<String>,
}

/// Issue complexity
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueComplexity {
    /// Simple fix, good for first-time contributors
    GoodFirstIssue,
    /// Moderate complexity
    Medium,
    /// Complex, requires deep understanding
    Complex,
}

impl IssueComplexity {
    pub fn as_str(&self) -> &'static str {
        match self {
            IssueComplexity::GoodFirstIssue => "good-first-issue",
            IssueComplexity::Medium => "medium",
            IssueComplexity::Complex => "complex",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_passed() {
        let check = HealthCheck::passed("cargo check", 1500);
        assert!(check.passed);
        assert!(check.error.is_none());
    }

    #[test]
    fn test_health_check_failed() {
        let check = HealthCheck::failed("npm build", 500, "Missing dependency");
        assert!(!check.passed);
        assert_eq!(check.error, Some("Missing dependency".to_string()));
    }

    #[test]
    fn test_health_check_with_warnings() {
        let check = HealthCheck::passed("cargo build", 2000)
            .with_warnings(vec!["unused import".to_string()]);
        assert!(check.passed);
        assert_eq!(check.warnings.len(), 1);
    }

    #[test]
    fn test_recommendation_category() {
        assert_eq!(RecommendationCategory::Security.as_str(), "security");
        assert_eq!(RecommendationCategory::Quality.as_str(), "quality");
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Medium);
        assert!(Priority::Medium > Priority::Low);
    }

    #[test]
    fn test_issue_complexity() {
        assert_eq!(
            IssueComplexity::GoodFirstIssue.as_str(),
            "good-first-issue"
        );
    }
}
