//! Issue staleness checking for Adjutant.
//!
//! Provides a DSPy signature and helpers to evaluate whether an issue
//! is still relevant given recent codebase changes.

use crate::manifest::IssueSummary;
use anyhow::Result;
use chrono::{DateTime, Utc};
use dsrs::{GLOBAL_SETTINGS, Predict, Prediction, Predictor, Signature, example};
use std::path::Path;
use std::process::Command;

use super::get_planning_lm;

// ============================================================================
// Staleness Check Signature
// ============================================================================

/// Staleness Check - Evaluate if an issue is still relevant.
#[Signature]
struct StalenessCheckSignature {
    /// Issue Staleness Evaluator: Given an issue and recent git activity,
    /// determine if the issue is still relevant and actionable.
    /// Consider whether recent commits may have addressed the issue,
    /// changed related files, or made the issue obsolete.
    /// Output recommendation: proceed (work on it), close (no longer needed),
    /// needs_update (issue description outdated), or blocked (dependencies missing).

    /// Title of the issue
    #[input]
    pub issue_title: String,

    /// Description of the issue
    #[input]
    pub issue_description: String,

    /// Type of issue (bug, feature, task)
    #[input]
    pub issue_type: String,

    /// Recent git commits since the issue was created
    #[input]
    pub recent_commits: String,

    /// Files that have changed recently
    #[input]
    pub changed_files: String,

    /// Is this issue still relevant and actionable?
    #[output]
    pub is_relevant: bool,

    /// Brief explanation of relevance assessment
    #[output]
    pub reason: String,

    /// Recommended action: proceed, close, needs_update, or blocked
    #[output]
    pub recommendation: String,
}

// ============================================================================
// Staleness Check Logic
// ============================================================================

/// Check if an issue needs a staleness check.
///
/// An issue needs staleness checking if:
/// 1. `created_at` > 7 days ago, AND
/// 2. (`last_checked` is null OR `last_checked` > 24 hours ago)
pub fn needs_staleness_check(issue: &IssueSummary) -> bool {
    let now = Utc::now();

    // Check if created > 7 days ago
    let created = issue
        .created_at
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let is_old = created
        .map(|c| now.signed_duration_since(c).num_days() > 7)
        .unwrap_or(false);

    if !is_old {
        return false;
    }

    // Check if last_checked > 24 hours ago (or never checked)
    let last_checked = issue
        .last_checked
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    last_checked
        .map(|lc| now.signed_duration_since(lc).num_hours() > 24)
        .unwrap_or(true) // Never checked = needs check
}

/// Result of a staleness check.
#[derive(Debug, Clone)]
pub struct StalenessResult {
    pub is_relevant: bool,
    pub reason: String,
    pub recommendation: String,
}

/// Check if an issue is still relevant given recent codebase changes.
pub async fn check_issue_staleness(
    issue: &IssueSummary,
    workspace_root: &Path,
) -> Result<StalenessResult> {
    // Get recent commits
    let commits = get_recent_commits(workspace_root, 20)?;
    let changed_files = get_changed_files_summary(workspace_root)?;

    let signature = StalenessCheckSignature::new();
    let predictor = Predict::new(signature);

    let example = example! {
        "issue_title": "input" => issue.title.clone(),
        "issue_description": "input" => issue.description.clone().unwrap_or_default(),
        "issue_type": "input" => issue.issue_type.clone().unwrap_or_else(|| "task".to_string()),
        "recent_commits": "input" => commits,
        "changed_files": "input" => changed_files,
    };

    // Use global LM if configured, otherwise auto-detect
    let prediction = if GLOBAL_SETTINGS.read().unwrap().is_some() {
        predictor.forward(example).await?
    } else {
        let lm = get_planning_lm().await?;
        predictor.forward_with_config(example, lm).await?
    };

    Ok(StalenessResult {
        is_relevant: get_bool(&prediction, "is_relevant"),
        reason: get_string(&prediction, "reason"),
        recommendation: get_string(&prediction, "recommendation"),
    })
}

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

/// Get a summary of recently changed files.
fn get_changed_files_summary(workspace_root: &Path) -> Result<String> {
    // Get files changed in the last 50 commits
    let output = Command::new("git")
        .args(["diff", "--stat", "--name-only", "HEAD~50..HEAD"])
        .current_dir(workspace_root)
        .output()?;

    if output.status.success() {
        let files = String::from_utf8_lossy(&output.stdout);
        // Deduplicate and summarize
        let unique_files: std::collections::HashSet<&str> = files.lines().collect();
        let summary: Vec<&str> = unique_files.into_iter().take(50).collect();
        Ok(summary.join("\n"))
    } else {
        Ok("No file change history available".to_string())
    }
}

/// Update the last_checked timestamp for an issue in the issues.json file.
pub fn update_issue_last_checked(issue_number: u32, workspace_root: &Path) -> Result<()> {
    use std::fs;

    let issues_path = workspace_root.join(".openagents/issues.json");
    if !issues_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&issues_path)?;
    let mut issues: Vec<serde_json::Value> = serde_json::from_str(&content)?;

    let now = Utc::now().to_rfc3339();

    for issue in &mut issues {
        if let Some(num) = issue.get("number").and_then(|n| n.as_u64()) {
            if num as u32 == issue_number {
                issue["last_checked"] = serde_json::Value::String(now.clone());
                break;
            }
        }
    }

    let updated_content = serde_json::to_string_pretty(&issues)?;
    fs::write(&issues_path, updated_content)?;

    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Helper to get string from prediction value.
fn get_string(prediction: &Prediction, key: &str) -> String {
    let val = prediction.get(key, None);
    if let Some(s) = val.as_str() {
        s.to_string()
    } else {
        val.to_string().trim_matches('"').to_string()
    }
}

/// Helper to get bool from prediction value.
fn get_bool(prediction: &Prediction, key: &str) -> bool {
    let val = prediction.get(key, None);
    if let Some(b) = val.as_bool() {
        b
    } else if let Some(s) = val.as_str() {
        s.eq_ignore_ascii_case("true") || s == "1"
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_staleness_check_new_issue() {
        let issue = IssueSummary {
            number: 1,
            title: "Test".to_string(),
            description: None,
            issue_type: None,
            status: "open".to_string(),
            priority: "medium".to_string(),
            is_blocked: false,
            blocked_reason: None,
            created_at: Some(Utc::now().to_rfc3339()),
            updated_at: None,
            last_checked: None,
        };

        // New issue (created now) should NOT need staleness check
        assert!(!needs_staleness_check(&issue));
    }

    #[test]
    fn test_needs_staleness_check_old_issue() {
        use chrono::Duration;

        let old_date = Utc::now() - Duration::days(10);
        let issue = IssueSummary {
            number: 1,
            title: "Test".to_string(),
            description: None,
            issue_type: None,
            status: "open".to_string(),
            priority: "medium".to_string(),
            is_blocked: false,
            blocked_reason: None,
            created_at: Some(old_date.to_rfc3339()),
            updated_at: None,
            last_checked: None,
        };

        // Old issue (10 days ago, never checked) should need staleness check
        assert!(needs_staleness_check(&issue));
    }

    #[test]
    fn test_needs_staleness_check_recently_checked() {
        use chrono::Duration;

        let old_date = Utc::now() - Duration::days(10);
        let recent_check = Utc::now() - Duration::hours(12);
        let issue = IssueSummary {
            number: 1,
            title: "Test".to_string(),
            description: None,
            issue_type: None,
            status: "open".to_string(),
            priority: "medium".to_string(),
            is_blocked: false,
            blocked_reason: None,
            created_at: Some(old_date.to_rfc3339()),
            updated_at: None,
            last_checked: Some(recent_check.to_rfc3339()),
        };

        // Old issue but recently checked (12h ago) should NOT need staleness check
        assert!(!needs_staleness_check(&issue));
    }
}
