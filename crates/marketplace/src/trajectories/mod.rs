//! Trajectory contribution system for the data marketplace
//!
//! This module enables developers to contribute their AI coding trajectories
//! to the marketplace, receiving Bitcoin payments for valuable training data.
//!
//! # Overview
//!
//! Developer trajectories captured from Codex Code, Cursor, Codex, and other
//! AI coding assistants contain valuable training signal:
//!
//! - Initial state: Git commit hash (real environment, no simulation)
//! - Task trajectory: Tool calls, thinking blocks, user interactions
//! - Reward signal: Final commit + CI/CD results (build success, tests pass)
//! - Task instructions: Inferred from commit messages or auto-generated
//!
//! This eliminates the need for synthetic environment generation, artificial
//! task construction, and simulated rewards. Real developer workflows provide
//! genuine signal for training next-generation coding models.
//!
//! # Privacy & Security
//!
//! All data is processed locally before contribution:
//!
//! - Secret redaction: API keys, tokens, passwords, private keys
//! - PII anonymization: Usernames, emails, identifying information
//! - Path sanitization: Absolute paths replaced with relative
//! - User control: Review and approve each contribution
//! - Opt-in only: Never auto-contribute without explicit consent
//!
//! # Modules
//!
//! - `collect`: Scan local sources for trajectory data
//! - `redact`: Open-source secret redaction
//! - `anonymize`: PII and identifying information removal
//! - `contribute`: Submit redacted trajectories to marketplace
//! - `validate`: Quality scoring and completeness checks
//! - `rewards`: Payment calculation for contributions

pub mod anonymize;
pub mod collect;
pub mod contribute;
pub mod redact;
pub mod rewards;
pub mod validate;

pub use anonymize::{AnonymizationResult, Anonymizer};
pub use collect::{ScanResult, TrajectoryCollector, TrajectorySource};
pub use contribute::{
    ContributionClient, ContributionConfig, ContributionEarning, ContributionRecord,
    ContributionRequest, ContributionStatus,
};
pub use redact::{RedactionEngine, RedactionLevel, RedactionResult};
pub use rewards::{RewardCalculator, RewardInfo};
pub use validate::{QualityScore, ValidationResult};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a trajectory session collected from local sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectorySession {
    /// Unique session identifier
    pub session_id: String,

    /// Source of the trajectory (codex, cursor, codex, etc.)
    pub source: String,

    /// Local file path to original trajectory log
    pub path: PathBuf,

    /// Git commit hash at session start (initial state)
    pub initial_commit: Option<String>,

    /// Git commit hash at session end (final state)
    pub final_commit: Option<String>,

    /// CI/CD result (reward signal)
    pub ci_passed: Option<bool>,

    /// Session start timestamp
    pub started_at: chrono::DateTime<chrono::Utc>,

    /// Session end timestamp
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Total token count (size metric)
    pub token_count: usize,

    /// Total tool calls (complexity metric)
    pub tool_calls: usize,

    /// Quality score (0.0 - 1.0)
    pub quality_score: f64,
}

/// Configuration for trajectory contribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryConfig {
    /// Sources to scan (codex, cursor, codex)
    pub sources: Vec<String>,

    /// Auto-contribute without manual review
    pub auto_contribute: bool,

    /// Minimum quality score to contribute
    pub min_quality_score: f64,

    /// Only contribute if CI/CD result available
    pub require_ci_signal: bool,

    /// Redaction level (standard, strict, paranoid)
    pub redaction_level: String,

    /// Redact file paths
    pub redact_file_paths: bool,

    /// Redact usernames
    pub redact_usernames: bool,

    /// Keep repository names (useful context)
    pub keep_repo_names: bool,

    /// Custom patterns to redact
    pub custom_patterns: Vec<String>,

    /// Minimum reward in sats to bother contributing
    pub min_reward_sats: u64,
}

impl Default for TrajectoryConfig {
    fn default() -> Self {
        Self {
            sources: vec!["codex".to_string()],
            auto_contribute: false,
            min_quality_score: 0.1, // Low threshold for scanning
            require_ci_signal: false,
            redaction_level: "standard".to_string(),
            redact_file_paths: true,
            redact_usernames: true,
            keep_repo_names: false,
            custom_patterns: Vec::new(),
            min_reward_sats: 10,
        }
    }
}
