//! Issue validation pipeline for Adjutant.
//!
//! Validates whether an issue is still accurate before the agent starts work.
//! This prevents wasting time on stale or already-addressed issues.

use anyhow::Result;
use dsrs::signatures::IssueValidationSignature;
use dsrs::{GLOBAL_SETTINGS, Predict, Prediction, Predictor, example};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::get_planning_lm;

// ============================================================================
// Validation Types
// ============================================================================

/// Status of issue validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationStatus {
    /// Issue is still valid and accurate
    Valid,
    /// Recent commits appear to have addressed this issue
    AlreadyAddressed,
    /// Issue description is outdated/stale
    Stale,
    /// Issue exists but description needs revision
    NeedsUpdate,
}

impl ValidationStatus {
    /// Parse from string (from LLM output).
    pub fn from_str(s: &str) -> Self {
        let s = s.to_uppercase();
        if s.contains("ALREADY_ADDRESSED") || s.contains("ALREADY ADDRESSED") {
            Self::AlreadyAddressed
        } else if s.contains("STALE") {
            Self::Stale
        } else if s.contains("NEEDS_UPDATE") || s.contains("NEEDS UPDATE") {
            Self::NeedsUpdate
        } else {
            Self::Valid
        }
    }

    /// Human-readable label for display.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Valid => "Valid",
            Self::AlreadyAddressed => "Already Addressed",
            Self::Stale => "Stale",
            Self::NeedsUpdate => "Needs Update",
        }
    }
}

/// Input for issue validation.
#[derive(Debug, Clone)]
pub struct IssueValidationInput {
    pub issue_number: u32,
    pub issue_title: String,
    pub issue_description: Option<String>,
    pub blocked_reason: Option<String>,
    pub workspace_root: PathBuf,
}

/// Result of issue validation.
#[derive(Debug, Clone)]
pub struct IssueValidationResult {
    pub is_valid: bool,
    pub status: ValidationStatus,
    pub reason: String,
    pub confidence: f32,
}

// ============================================================================
// Validation Pipeline
// ============================================================================

/// Pipeline for validating issues before work starts.
pub struct IssueValidationPipeline {
    /// Optional custom LM (uses global if not set)
    #[allow(dead_code)]
    lm: Option<std::sync::Arc<dsrs::LM>>,
}

impl Default for IssueValidationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl IssueValidationPipeline {
    /// Create a new pipeline with default settings.
    pub fn new() -> Self {
        Self { lm: None }
    }

    /// Validate an issue before starting work.
    ///
    /// Returns validation result indicating if the issue is still accurate
    /// and worth working on.
    pub async fn validate(&self, input: &IssueValidationInput) -> Result<IssueValidationResult> {
        // Gather git context
        let recent_commits = get_recent_commits(&input.workspace_root, 20)?;
        let changed_files = get_changed_files(&input.workspace_root)?;

        tracing::debug!(
            "Issue validation context for #{}: blocked_reason={:?}, commits:\n{}\nchanged_files:\n{}",
            input.issue_number,
            input.blocked_reason,
            recent_commits,
            changed_files
        );

        // Build signature and predictor
        let signature = IssueValidationSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "issue_title": "input" => input.issue_title.clone(),
            "issue_description": "input" => input.issue_description.clone().unwrap_or_default(),
            "blocked_reason": "input" => input.blocked_reason.clone().unwrap_or_else(|| "None".to_string()),
            "recent_commits": "input" => recent_commits,
            "changed_files": "input" => changed_files,
        };

        // Use global LM if configured, otherwise auto-detect
        let prediction = if GLOBAL_SETTINGS.read().unwrap().is_some() {
            predictor.forward(example).await?
        } else {
            let lm = get_planning_lm().await?;
            predictor.forward_with_config(example, lm).await?
        };

        // Debug: log all prediction keys and values
        tracing::debug!(
            "Prediction keys: {:?}, values: {:?}",
            prediction.keys(),
            prediction.values()
        );

        // Parse results with improved handling
        let is_valid = get_bool(&prediction, "is_valid");
        let status_str = get_string(&prediction, "validation_status");
        let status = ValidationStatus::from_str(&status_str);
        let reason = get_string(&prediction, "reason");
        let confidence = get_f32(&prediction, "confidence");

        // If reason is empty but status indicates invalid, derive reason from status
        let reason = if reason.is_empty() && !is_valid {
            match status {
                ValidationStatus::AlreadyAddressed => {
                    "Issue appears to have been addressed by recent commits.".to_string()
                }
                ValidationStatus::Stale => "Issue description appears outdated.".to_string(),
                ValidationStatus::NeedsUpdate => {
                    "Issue needs description update before work can begin.".to_string()
                }
                ValidationStatus::Valid => {
                    "Validation returned invalid but no reason provided.".to_string()
                }
            }
        } else {
            reason
        };

        // Derive is_valid from status if they conflict (status is more reliable)
        let is_valid = match status {
            ValidationStatus::Valid => true,
            ValidationStatus::AlreadyAddressed
            | ValidationStatus::Stale
            | ValidationStatus::NeedsUpdate => false,
        };

        tracing::debug!(
            "Parsed validation: is_valid={}, status={:?}, reason={}",
            is_valid,
            status,
            reason
        );

        Ok(IssueValidationResult {
            is_valid,
            status,
            reason,
            confidence,
        })
    }
}

// ============================================================================
// Git Context Helpers
// ============================================================================

/// Get recent git commits from the workspace.
fn get_recent_commits(workspace_root: &Path, count: usize) -> Result<String> {
    let output = Command::new("git")
        .args(["log", "--oneline", &format!("-{}", count), "--no-decorate"])
        .current_dir(workspace_root)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Ok("No recent commits available".to_string())
    }
}

/// Get files changed in recent commits.
fn get_changed_files(workspace_root: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD~20..HEAD"])
        .current_dir(workspace_root)
        .output()?;

    if output.status.success() {
        let files = String::from_utf8_lossy(&output.stdout);
        // Deduplicate
        let unique: std::collections::HashSet<&str> = files.lines().collect();
        let summary: Vec<&str> = unique.into_iter().take(50).collect();
        Ok(summary.join("\n"))
    } else {
        Ok("No file change history available".to_string())
    }
}

// ============================================================================
// Prediction Helpers
// ============================================================================

/// Helper to get string from prediction value.
fn get_string(prediction: &Prediction, key: &str) -> String {
    let val = prediction.get(key, None);
    if let Some(s) = val.as_str() {
        s.trim().to_string()
    } else if val.is_null() {
        String::new()
    } else {
        // Handle JSON values - strip quotes and clean up
        let s = val.to_string();
        s.trim().trim_matches('"').trim_matches('\\').to_string()
    }
}

/// Helper to get bool from prediction value.
fn get_bool(prediction: &Prediction, key: &str) -> bool {
    let val = prediction.get(key, None);
    if let Some(b) = val.as_bool() {
        b
    } else if let Some(s) = val.as_str() {
        let s = s.trim().to_lowercase();
        s == "true" || s == "1" || s == "yes" || s == "valid"
    } else if val.is_null() {
        // Default to true if not specified (proceed with issue)
        true
    } else {
        // Try parsing the string representation
        let s = val.to_string().trim().to_lowercase();
        s == "true" || s == "1" || s == "yes" || s == "valid"
    }
}

/// Helper to get f32 from prediction value.
fn get_f32(prediction: &Prediction, key: &str) -> f32 {
    let val = prediction.get(key, None);
    if let Some(n) = val.as_f64() {
        n as f32
    } else if let Some(s) = val.as_str() {
        s.parse().unwrap_or(0.5)
    } else {
        0.5
    }
}

// ============================================================================
// Convenience Helper for Blocked Issue Validation
// ============================================================================

/// Validate that a blocked issue's blocked_reason is still accurate.
///
/// This is a convenience function that creates a pipeline and validates
/// whether recent commits have resolved the blocker described in blocked_reason.
///
/// Returns `is_valid=false` if the blocked_reason describes a state that
/// no longer exists (e.g., "crate has no source files" when source files
/// were recently added).
pub async fn validate_blocked_issue(
    issue_number: u32,
    title: &str,
    blocked_reason: &str,
    workspace_root: &Path,
) -> Result<IssueValidationResult> {
    let pipeline = IssueValidationPipeline::new();
    pipeline
        .validate(&IssueValidationInput {
            issue_number,
            issue_title: title.to_string(),
            issue_description: None,
            blocked_reason: Some(blocked_reason.to_string()),
            workspace_root: workspace_root.to_path_buf(),
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_status_parsing() {
        assert_eq!(
            ValidationStatus::from_str("ALREADY_ADDRESSED"),
            ValidationStatus::AlreadyAddressed
        );
        assert_eq!(
            ValidationStatus::from_str("already addressed"),
            ValidationStatus::AlreadyAddressed
        );
        assert_eq!(ValidationStatus::from_str("STALE"), ValidationStatus::Stale);
        assert_eq!(
            ValidationStatus::from_str("NEEDS_UPDATE"),
            ValidationStatus::NeedsUpdate
        );
        assert_eq!(ValidationStatus::from_str("VALID"), ValidationStatus::Valid);
        assert_eq!(
            ValidationStatus::from_str("something else"),
            ValidationStatus::Valid
        );
    }

    #[test]
    fn test_validation_status_labels() {
        assert_eq!(ValidationStatus::Valid.label(), "Valid");
        assert_eq!(
            ValidationStatus::AlreadyAddressed.label(),
            "Already Addressed"
        );
        assert_eq!(ValidationStatus::Stale.label(), "Stale");
        assert_eq!(ValidationStatus::NeedsUpdate.label(), "Needs Update");
    }
}
