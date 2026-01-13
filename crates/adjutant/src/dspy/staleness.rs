//! Issue staleness checking for Adjutant.
//!
//! Provides:
//! - DSPy signature for LLM-based staleness evaluation
//! - Multi-factor staleness scoring for issue suggestion filtering
//! - Helpers to update staleness timestamps

use crate::manifest::IssueSummary;
use anyhow::Result;
use chrono::{DateTime, Utc};
use dsrs::{GLOBAL_SETTINGS, Predict, Prediction, Predictor, Signature, example};
use serde::{Deserialize, Serialize};
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

// ============================================================================
// Multi-Factor Staleness Scoring (for Issue Suggestions)
// ============================================================================

/// Multi-factor staleness score for filtering issue suggestions.
///
/// This is different from `StalenessResult` which uses an LLM to evaluate
/// issue relevance. This score is computed locally and used to filter
/// issues before presenting suggestions to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StalenessScore {
    /// Overall staleness score (0.0 = fresh, 1.0 = very stale)
    pub score: f32,
    /// Individual factor contributions
    pub factors: StalenessFactors,
    /// Whether the issue should be filtered out from suggestions
    pub exclude_from_suggestions: bool,
    /// Reason for exclusion (if applicable)
    pub exclusion_reason: Option<String>,
}

/// Individual staleness factors with their contributions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StalenessFactors {
    /// Days since last update
    pub days_since_update: Option<u32>,
    /// No `last_checked` date
    pub never_checked: bool,
    /// Missing required fields
    pub missing_fields: Vec<String>,
    /// Blocked without clear unblock path
    pub unclear_blocker: bool,
}

/// Compute a multi-factor staleness score for an issue.
///
/// Factors and weights:
/// - Days since update (14+ days): weight 0.3
/// - Never checked (no last_checked): weight 0.2
/// - Missing required fields: weight 0.2
/// - Blocked without clear unblock path: weight 0.3
///
/// Issues with score >= 0.7 or is_blocked == true are excluded from suggestions.
pub fn compute_staleness_score(issue: &IssueSummary) -> StalenessScore {
    let mut score = 0.0f32;
    let mut factors = StalenessFactors::default();

    let now = Utc::now();

    // Factor 1: Days since update (14+ days = stale)
    if let Some(updated_at) = &issue.updated_at {
        if let Ok(dt) = DateTime::parse_from_rfc3339(updated_at) {
            let days = now.signed_duration_since(dt.with_timezone(&Utc)).num_days();
            factors.days_since_update = Some(days as u32);
            if days >= 14 {
                // Score increases linearly from 0 to 0.3 for days 14-42
                score += 0.3 * ((days as f32 - 14.0) / 28.0).min(1.0);
            }
        }
    } else if let Some(created_at) = &issue.created_at {
        // Use created_at if updated_at is missing
        if let Ok(dt) = DateTime::parse_from_rfc3339(created_at) {
            let days = now.signed_duration_since(dt.with_timezone(&Utc)).num_days();
            factors.days_since_update = Some(days as u32);
            if days >= 14 {
                score += 0.3 * ((days as f32 - 14.0) / 28.0).min(1.0);
            }
        }
    }

    // Factor 2: Never checked
    if issue.last_checked.is_none() {
        factors.never_checked = true;
        score += 0.2;
    }

    // Factor 3: Missing required fields
    if issue.description.is_none()
        || issue
            .description
            .as_ref()
            .map(|d| d.trim().is_empty())
            .unwrap_or(true)
    {
        factors.missing_fields.push("description".to_string());
        score += 0.1;
    }
    if issue.priority.is_empty() || issue.priority == "unknown" {
        factors.missing_fields.push("priority".to_string());
        score += 0.1;
    }
    if issue.issue_type.is_none() {
        factors.missing_fields.push("issue_type".to_string());
        score += 0.05;
    }

    // Factor 4: Blocked without clear unblock path
    if issue.is_blocked {
        let has_clear_path = issue
            .blocked_reason
            .as_ref()
            .map(|r| {
                let r_lower = r.to_lowercase();
                r_lower.contains("waiting")
                    || r_lower.contains("depends on")
                    || r_lower.contains("after")
                    || r_lower.contains("blocked by #")
            })
            .unwrap_or(false);

        if !has_clear_path {
            factors.unclear_blocker = true;
            score += 0.3;
        }
    }

    // Determine exclusion
    let exclude = score >= 0.7 || issue.is_blocked;
    let reason = if issue.is_blocked {
        Some(
            issue
                .blocked_reason
                .clone()
                .unwrap_or_else(|| "Blocked".to_string()),
        )
    } else if score >= 0.7 {
        Some("Too stale - needs triage".to_string())
    } else {
        None
    };

    StalenessScore {
        score: score.min(1.0),
        factors,
        exclude_from_suggestions: exclude,
        exclusion_reason: reason,
    }
}

/// Filter issues for suggestion, returning (candidates, filtered_out).
///
/// Candidates are sorted by freshness (lowest staleness score first).
pub fn filter_issues_for_suggestion(
    issues: &[IssueSummary],
) -> (
    Vec<(IssueSummary, StalenessScore)>,
    Vec<(IssueSummary, String)>,
) {
    let mut candidates = Vec::new();
    let mut filtered = Vec::new();

    for issue in issues {
        // Skip non-open issues
        if issue.status != "open" {
            continue;
        }

        let staleness = compute_staleness_score(issue);

        if staleness.exclude_from_suggestions {
            let reason = staleness
                .exclusion_reason
                .unwrap_or_else(|| "Filtered".to_string());
            filtered.push((issue.clone(), reason));
        } else {
            candidates.push((issue.clone(), staleness));
        }
    }

    // Sort candidates by staleness score (freshest first)
    candidates.sort_by(|a, b| {
        a.1.score
            .partial_cmp(&b.1.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    (candidates, filtered)
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
