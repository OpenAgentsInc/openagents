//! Issue Suggestion Pipeline for Adjutant.
//!
//! DSPy-powered pipeline for suggesting top issues to work on.
//! Integrates staleness filtering with LLM-based prioritization.

use super::get_planning_lm;
use super::issue_validation::validate_blocked_issue;
use super::staleness::{StalenessScore, filter_issues_for_suggestion};
use crate::manifest::IssueSummary;
use anyhow::Result;
use dsrs::signatures::{IssueSuggestionSignature, UnblockSuggestionSignature};
use dsrs::{GLOBAL_SETTINGS, LM, Predict, Prediction, Predictor, example};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

// ============================================================================
// Input/Output Types
// ============================================================================

/// Input for issue suggestion pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSuggestionInput {
    /// All available issues from the manifest
    pub issues: Vec<IssueSummary>,
    /// Current workspace context (project info, active directive)
    pub workspace_context: String,
    /// Recently completed/worked issue numbers
    pub recent_work: Vec<u32>,
    /// User preferences (optional)
    pub user_preferences: Option<serde_json::Value>,
}

/// A single issue suggestion with rationale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSuggestion {
    /// Issue number
    pub number: u32,
    /// Issue title
    pub title: String,
    /// Priority level
    pub priority: String,
    /// Rationale for suggesting this issue
    pub rationale: String,
    /// Estimated complexity (low/medium/high)
    pub complexity: String,
    /// Staleness score (0.0 = fresh, 1.0 = stale)
    pub staleness_score: f32,
}

/// Result from the issue suggestion pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSuggestionResult {
    /// Top suggested issues (up to 3)
    pub suggestions: Vec<IssueSuggestion>,
    /// Number of issues filtered out (stale/blocked)
    pub filtered_count: usize,
    /// Reasons for filtered issues
    pub filtered_reasons: Vec<(u32, String)>,
    /// Confidence in the suggestions (0.0-1.0)
    pub confidence: f32,
}

impl Default for IssueSuggestionResult {
    fn default() -> Self {
        Self {
            suggestions: Vec::new(),
            filtered_count: 0,
            filtered_reasons: Vec::new(),
            confidence: 0.0,
        }
    }
}

/// Result for unblock suggestion (when all issues are blocked).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnblockSuggestionResult {
    /// Issue to unblock
    pub issue_number: u32,
    /// Issue title
    pub title: String,
    /// Blocked reason
    pub blocked_reason: String,
    /// Why unblock this first
    pub unblock_rationale: String,
    /// How to unblock it
    pub unblock_strategy: String,
    /// Effort estimate
    pub estimated_effort: String,
    /// Cascade potential description
    pub cascade_potential: String,
    /// Total blocked issues
    pub total_blocked: usize,
}

// ============================================================================
// Pipeline
// ============================================================================

/// DSPy-powered issue suggestion pipeline.
///
/// Workflow:
/// 1. Filter out stale/blocked issues using staleness scoring
/// 2. Run DSPy prediction on remaining candidates
/// 3. Return top 3 suggestions with rationale
pub struct IssueSuggestionPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for IssueSuggestionPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl IssueSuggestionPipeline {
    /// Create a new pipeline using the global LM.
    pub fn new() -> Self {
        Self { lm: None }
    }

    /// Create a pipeline with a specific LM.
    pub fn with_lm(lm: Arc<LM>) -> Self {
        Self { lm: Some(lm) }
    }

    /// Suggest top issues to work on.
    ///
    /// Returns Ok(Some(result)) with suggestions if candidates exist,
    /// or attempts unblock suggestion if all issues are blocked.
    pub async fn suggest(&self, input: &IssueSuggestionInput) -> Result<IssueSuggestionResult> {
        // Step 1: Filter out stale/blocked issues
        let (candidates, filtered) = filter_issues_for_suggestion(&input.issues);

        let filtered_count = filtered.len();
        let filtered_reasons: Vec<(u32, String)> = filtered
            .iter()
            .map(|(issue, reason)| (issue.number, reason.clone()))
            .collect();

        // If no candidates, return empty result (unblock suggestion handled separately)
        if candidates.is_empty() {
            return Ok(IssueSuggestionResult {
                suggestions: Vec::new(),
                filtered_count,
                filtered_reasons,
                confidence: 0.0,
            });
        }

        // If only 1-3 candidates, skip LLM and return them directly
        let candidate_count = candidates.len();
        if candidate_count <= 3 {
            let total_issues = candidate_count + filtered_count;
            let suggestions = candidates
                .into_iter()
                .map(|(issue, staleness)| IssueSuggestion {
                    number: issue.number,
                    title: issue.title.clone(),
                    priority: issue.priority.clone(),
                    rationale: format!("One of {} available issues", total_issues),
                    complexity: estimate_complexity(&issue),
                    staleness_score: staleness.score,
                })
                .collect();

            return Ok(IssueSuggestionResult {
                suggestions,
                filtered_count,
                filtered_reasons,
                confidence: 0.8,
            });
        }

        // Step 2: Prepare candidate data for LLM
        let candidate_json: Vec<serde_json::Value> = candidates
            .iter()
            .map(|(issue, staleness)| {
                json!({
                    "number": issue.number,
                    "title": issue.title,
                    "priority": issue.priority,
                    "issue_type": issue.issue_type,
                    "description": issue.description.as_ref().map(|d| truncate(d, 200)),
                    "staleness_score": staleness.score,
                })
            })
            .collect();

        // Step 3: Run DSPy prediction
        let lm = if let Some(lm) = &self.lm {
            lm.clone()
        } else if GLOBAL_SETTINGS.read().unwrap().is_some() {
            return self
                .suggest_with_global(input, &candidates, filtered_count, filtered_reasons)
                .await;
        } else {
            get_planning_lm().await?
        };

        let signature = IssueSuggestionSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "available_issues": "input" => serde_json::to_string(&candidate_json)?,
            "workspace_context": "input" => input.workspace_context.clone(),
            "recent_work": "input" => serde_json::to_string(&input.recent_work)?,
            "user_preferences": "input" => input.user_preferences.clone().map(|v| v.to_string()).unwrap_or_default(),
        };

        let prediction = predictor.forward_with_config(example, lm).await?;

        // Step 4: Parse suggestions from prediction
        let suggestions = parse_suggestions(&prediction, &candidates)?;
        let confidence = get_f32(&prediction, "confidence");

        Ok(IssueSuggestionResult {
            suggestions,
            filtered_count,
            filtered_reasons,
            confidence,
        })
    }

    async fn suggest_with_global(
        &self,
        input: &IssueSuggestionInput,
        candidates: &[(IssueSummary, StalenessScore)],
        filtered_count: usize,
        filtered_reasons: Vec<(u32, String)>,
    ) -> Result<IssueSuggestionResult> {
        let candidate_json: Vec<serde_json::Value> = candidates
            .iter()
            .map(|(issue, staleness)| {
                json!({
                    "number": issue.number,
                    "title": issue.title,
                    "priority": issue.priority,
                    "issue_type": issue.issue_type,
                    "description": issue.description.as_ref().map(|d| truncate(d, 200)),
                    "staleness_score": staleness.score,
                })
            })
            .collect();

        let signature = IssueSuggestionSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "available_issues": "input" => serde_json::to_string(&candidate_json)?,
            "workspace_context": "input" => input.workspace_context.clone(),
            "recent_work": "input" => serde_json::to_string(&input.recent_work)?,
            "user_preferences": "input" => input.user_preferences.clone().map(|v| v.to_string()).unwrap_or_default(),
        };

        let prediction = predictor.forward(example).await?;

        let suggestions = parse_suggestions(&prediction, candidates)?;
        let confidence = get_f32(&prediction, "confidence");

        Ok(IssueSuggestionResult {
            suggestions,
            filtered_count,
            filtered_reasons,
            confidence,
        })
    }

    /// Suggest which blocked issue to unblock first.
    ///
    /// Call this when all issues are blocked (suggest() returns empty suggestions).
    ///
    /// This function validates the suggested issue against recent commits before
    /// returning. If the blocked_reason is stale (already addressed by recent commits),
    /// it will try the next candidate.
    pub async fn suggest_unblock(
        &self,
        issues: &[IssueSummary],
        workspace_context: &str,
        workspace_root: Option<&Path>,
    ) -> Result<Option<UnblockSuggestionResult>> {
        // Track excluded issues (those that failed validation)
        let mut excluded: Vec<u32> = Vec::new();

        self.suggest_unblock_internal(issues, workspace_context, workspace_root, &mut excluded)
            .await
    }

    /// Internal implementation with exclusion list for retry logic.
    ///
    /// Uses Box::pin for the recursive call to avoid infinite-sized futures.
    fn suggest_unblock_internal<'a>(
        &'a self,
        issues: &'a [IssueSummary],
        workspace_context: &'a str,
        workspace_root: Option<&'a Path>,
        excluded: &'a mut Vec<u32>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<UnblockSuggestionResult>>> + Send + 'a>> {
        Box::pin(async move {
        // Get blocked issues, excluding already-validated-as-stale ones
        let blocked: Vec<_> = issues
            .iter()
            .filter(|i| i.is_blocked && i.status == "open" && !excluded.contains(&i.number))
            .collect();

        if blocked.is_empty() {
            tracing::info!("No more blocked issues to consider (all excluded or none exist)");
            return Ok(None);
        }

        // Get recent commits for context
        let recent_commits = workspace_root
            .map(|root| get_recent_commits(root, 20))
            .transpose()?
            .unwrap_or_else(|| "No commit history available".to_string());

        // Prepare blocked issues JSON
        let blocked_json: Vec<serde_json::Value> = blocked
            .iter()
            .map(|issue| {
                json!({
                    "number": issue.number,
                    "title": issue.title,
                    "blocked_reason": issue.blocked_reason.as_deref().unwrap_or("Unknown"),
                    "priority": issue.priority,
                    "issue_type": issue.issue_type,
                })
            })
            .collect();

        // Try LLM prediction, fallback to heuristic if no LLM available
        let prediction_result = self
            .try_lm_unblock_prediction(&blocked_json, workspace_context, &recent_commits)
            .await;

        let unblock_result = match prediction_result {
            Ok(prediction) => self.parse_unblock_result(&prediction, &blocked)?,
            Err(_) => {
                // Fallback to heuristic selection when LLM is unavailable
                Some(self.heuristic_unblock_selection(&blocked))
            }
        };

        // Validate the result before returning
        if let Some(ref result) = unblock_result {
            if let Some(root) = workspace_root {
                tracing::info!(
                    "Validating issue #{} blocked_reason: '{}' against recent commits at {:?}",
                    result.issue_number,
                    result.blocked_reason,
                    root
                );

                // Validate that the blocked_reason is still accurate
                let validation = validate_blocked_issue(
                    result.issue_number,
                    &result.title,
                    &result.blocked_reason,
                    root,
                )
                .await;

                match validation {
                    Ok(v) => {
                        tracing::info!(
                            "Validation result for #{}: is_valid={}, status={:?}, reason='{}', confidence={}",
                            result.issue_number,
                            v.is_valid,
                            v.status,
                            v.reason,
                            v.confidence
                        );

                        if !v.is_valid {
                            tracing::info!(
                                "Issue #{} blocked_reason is stale - trying next candidate",
                                result.issue_number
                            );
                            // Exclude this issue and try next candidate
                            excluded.push(result.issue_number);
                            return self
                                .suggest_unblock_internal(
                                    issues,
                                    workspace_context,
                                    workspace_root,
                                    excluded,
                                )
                                .await;
                        }
                    }
                    Err(e) => {
                        // Validation failed (e.g., no LLM available), proceed anyway
                        tracing::warn!(
                            "Could not validate issue #{}: {}. Proceeding anyway.",
                            result.issue_number,
                            e
                        );
                    }
                }
            } else {
                tracing::warn!(
                    "No workspace_root provided - skipping validation for issue #{}",
                    result.issue_number
                );
            }
        }

        Ok(unblock_result)
        })
    }

    /// Try to get LLM prediction for unblock suggestion.
    async fn try_lm_unblock_prediction(
        &self,
        blocked_json: &[serde_json::Value],
        workspace_context: &str,
        recent_commits: &str,
    ) -> Result<Prediction> {
        let lm = if let Some(lm) = &self.lm {
            lm.clone()
        } else if GLOBAL_SETTINGS.read().unwrap().is_some() {
            let signature = UnblockSuggestionSignature::new();
            let predictor = Predict::new(signature);
            let example = example! {
                "blocked_issues": "input" => serde_json::to_string(blocked_json)?,
                "workspace_context": "input" => workspace_context.to_string(),
                "recent_commits": "input" => recent_commits.to_string(),
            };
            return predictor.forward(example).await;
        } else {
            get_planning_lm().await?
        };

        let signature = UnblockSuggestionSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "blocked_issues": "input" => serde_json::to_string(blocked_json)?,
            "workspace_context": "input" => workspace_context.to_string(),
            "recent_commits": "input" => recent_commits.to_string(),
        };

        predictor.forward_with_config(example, lm).await
    }

    /// Heuristic selection when LLM is unavailable.
    fn heuristic_unblock_selection(&self, blocked: &[&IssueSummary]) -> UnblockSuggestionResult {
        // Sort by priority (high > medium > low) and pick first
        let mut sorted: Vec<_> = blocked.iter().copied().collect();
        sorted.sort_by(|a, b| {
            let priority_order = |p: &str| match p.to_lowercase().as_str() {
                "critical" => 0,
                "high" => 1,
                "medium" => 2,
                "low" => 3,
                _ => 4,
            };
            priority_order(&a.priority).cmp(&priority_order(&b.priority))
        });

        let selected = sorted.first().copied().unwrap_or(blocked[0]);

        UnblockSuggestionResult {
            issue_number: selected.number,
            title: selected.title.clone(),
            blocked_reason: selected.blocked_reason.clone().unwrap_or_else(|| "Unknown".to_string()),
            unblock_rationale: format!(
                "Selected as highest priority blocked issue ({} priority). Consider breaking it into smaller tasks or addressing the blocker directly.",
                selected.priority
            ),
            unblock_strategy: "Review the blocked reason and identify the smallest actionable step to make progress.".to_string(),
            estimated_effort: estimate_effort_from_blocked_reason(selected.blocked_reason.as_deref()),
            cascade_potential: format!("{} other issues may be unblocked", blocked.len() - 1),
            total_blocked: blocked.len(),
        }
    }

    fn parse_unblock_result(
        &self,
        prediction: &Prediction,
        blocked: &[&IssueSummary],
    ) -> Result<Option<UnblockSuggestionResult>> {
        let selected_number = get_u32(prediction, "selected_issue_number");

        // Find the selected issue
        let selected = blocked
            .iter()
            .find(|i| i.number == selected_number)
            .or_else(|| blocked.first())
            .map(|i| *i);

        let Some(issue) = selected else {
            return Ok(None);
        };

        Ok(Some(UnblockSuggestionResult {
            issue_number: issue.number,
            title: issue.title.clone(),
            blocked_reason: issue
                .blocked_reason
                .clone()
                .unwrap_or_else(|| "Unknown".to_string()),
            unblock_rationale: get_string(prediction, "unblock_rationale"),
            unblock_strategy: get_string(prediction, "unblock_strategy"),
            estimated_effort: get_string(prediction, "estimated_effort"),
            cascade_potential: get_string(prediction, "cascade_potential"),
            total_blocked: blocked.len(),
        }))
    }
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

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse suggestions from LLM prediction.
fn parse_suggestions(
    prediction: &Prediction,
    candidates: &[(IssueSummary, StalenessScore)],
) -> Result<Vec<IssueSuggestion>> {
    let suggestions_val = prediction.get("suggestions", None);

    // Try to parse as JSON array
    let parsed: Vec<serde_json::Value> = if let Some(s) = suggestions_val.as_str() {
        serde_json::from_str(s).unwrap_or_default()
    } else if suggestions_val.is_array() {
        suggestions_val.as_array().unwrap_or(&vec![]).clone()
    } else {
        Vec::new()
    };

    let mut suggestions = Vec::new();

    for item in parsed.into_iter().take(3) {
        let number = item
            .get("number")
            .and_then(|n| n.as_u64())
            .map(|n| n as u32)
            .unwrap_or(0);

        // Find the candidate to get staleness score
        let (issue, staleness) = candidates
            .iter()
            .find(|(i, _)| i.number == number)
            .cloned()
            .unwrap_or_else(|| {
                // Fallback to first candidate if number not found
                candidates.first().cloned().unwrap_or_else(|| {
                    (
                        IssueSummary {
                            number,
                            title: item
                                .get("title")
                                .and_then(|t| t.as_str())
                                .unwrap_or("Unknown")
                                .to_string(),
                            description: None,
                            issue_type: None,
                            status: "open".to_string(),
                            priority: "medium".to_string(),
                            is_blocked: false,
                            blocked_reason: None,
                            created_at: None,
                            updated_at: None,
                            last_checked: None,
                        },
                        StalenessScore {
                            score: 0.0,
                            factors: Default::default(),
                            exclude_from_suggestions: false,
                            exclusion_reason: None,
                        },
                    )
                })
            });

        suggestions.push(IssueSuggestion {
            number: issue.number,
            title: issue.title,
            priority: issue.priority,
            rationale: item
                .get("rationale")
                .and_then(|r| r.as_str())
                .unwrap_or("Suggested by priority ranking")
                .to_string(),
            complexity: item
                .get("complexity")
                .and_then(|c| c.as_str())
                .unwrap_or("medium")
                .to_string(),
            staleness_score: staleness.score,
        });
    }

    // If no suggestions parsed, fall back to top candidates
    if suggestions.is_empty() && !candidates.is_empty() {
        for (issue, staleness) in candidates.iter().take(3) {
            suggestions.push(IssueSuggestion {
                number: issue.number,
                title: issue.title.clone(),
                priority: issue.priority.clone(),
                rationale: "Top candidate by staleness score".to_string(),
                complexity: estimate_complexity(issue),
                staleness_score: staleness.score,
            });
        }
    }

    Ok(suggestions)
}

/// Estimate complexity based on issue metadata.
fn estimate_complexity(issue: &IssueSummary) -> String {
    let desc_len = issue.description.as_ref().map(|d| d.len()).unwrap_or(0);
    let title_lower = issue.title.to_lowercase();

    // High complexity indicators
    if title_lower.contains("refactor")
        || title_lower.contains("rewrite")
        || title_lower.contains("migrate")
        || title_lower.contains("architecture")
        || desc_len > 1000
    {
        return "high".to_string();
    }

    // Low complexity indicators
    if title_lower.contains("typo")
        || title_lower.contains("rename")
        || title_lower.contains("update")
        || title_lower.contains("fix")
        || desc_len < 200
    {
        return "low".to_string();
    }

    "medium".to_string()
}

/// Estimate effort from blocked reason text.
fn estimate_effort_from_blocked_reason(reason: Option<&str>) -> String {
    let reason = reason.unwrap_or("").to_lowercase();

    // High effort indicators
    if reason.contains("extensive")
        || reason.contains("major")
        || reason.contains("large")
        || reason.contains("complete rewrite")
        || reason.contains("significant")
    {
        return "high".to_string();
    }

    // Low effort indicators
    if reason.contains("simple")
        || reason.contains("small")
        || reason.contains("minor")
        || reason.contains("quick")
    {
        return "low".to_string();
    }

    "medium".to_string()
}

/// Truncate a string to max_len characters.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Helper to get f32 from prediction value.
fn get_f32(prediction: &Prediction, key: &str) -> f32 {
    let val = prediction.get(key, None);
    if let Some(n) = val.as_f64() {
        n as f32
    } else if let Some(s) = val.as_str() {
        s.parse().unwrap_or(0.0)
    } else {
        0.0
    }
}

/// Helper to get u32 from prediction value.
fn get_u32(prediction: &Prediction, key: &str) -> u32 {
    let val = prediction.get(key, None);
    if let Some(n) = val.as_u64() {
        n as u32
    } else if let Some(n) = val.as_i64() {
        n as u32
    } else if let Some(s) = val.as_str() {
        s.parse().unwrap_or(0)
    } else {
        0
    }
}

/// Helper to get String from prediction value.
fn get_string(prediction: &Prediction, key: &str) -> String {
    let val = prediction.get(key, None);
    if let Some(s) = val.as_str() {
        s.to_string()
    } else {
        val.to_string().trim_matches('"').to_string()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_complexity() {
        let mut issue = IssueSummary {
            number: 1,
            title: "Fix typo in README".to_string(),
            description: Some("Small fix".to_string()),
            issue_type: Some("bug".to_string()),
            status: "open".to_string(),
            priority: "low".to_string(),
            is_blocked: false,
            blocked_reason: None,
            created_at: None,
            updated_at: None,
            last_checked: None,
        };

        assert_eq!(estimate_complexity(&issue), "low");

        issue.title = "Refactor authentication module".to_string();
        assert_eq!(estimate_complexity(&issue), "high");

        issue.title = "Add dark mode toggle".to_string();
        issue.description = Some("Medium sized feature".repeat(50));
        assert_eq!(estimate_complexity(&issue), "medium");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_pipeline_creation() {
        let pipeline = IssueSuggestionPipeline::new();
        assert!(pipeline.lm.is_none());
    }

    #[test]
    fn test_result_default() {
        let result = IssueSuggestionResult::default();
        assert!(result.suggestions.is_empty());
        assert_eq!(result.filtered_count, 0);
        assert_eq!(result.confidence, 0.0);
    }
}
