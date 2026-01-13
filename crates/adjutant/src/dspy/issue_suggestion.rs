//! Issue Suggestion Pipeline for Adjutant.
//!
//! DSPy-powered pipeline for suggesting top issues to work on.
//! Integrates staleness filtering with LLM-based prioritization.

use super::staleness::{filter_issues_for_suggestion, StalenessScore};
use super::get_planning_lm;
use crate::manifest::IssueSummary;
use anyhow::Result;
use dsrs::{example, Predict, Prediction, Predictor, LM, GLOBAL_SETTINGS};
use dsrs::signatures::IssueSuggestionSignature;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
    pub async fn suggest(&self, input: &IssueSuggestionInput) -> Result<IssueSuggestionResult> {
        // Step 1: Filter out stale/blocked issues
        let (candidates, filtered) = filter_issues_for_suggestion(&input.issues);

        let filtered_count = filtered.len();
        let filtered_reasons: Vec<(u32, String)> = filtered
            .iter()
            .map(|(issue, reason)| (issue.number, reason.clone()))
            .collect();

        // If no candidates, return empty result
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
            return self.suggest_with_global(input, &candidates, filtered_count, filtered_reasons).await;
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
        let number = item.get("number")
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
                    (IssueSummary {
                        number,
                        title: item.get("title")
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
                    }, StalenessScore {
                        score: 0.0,
                        factors: Default::default(),
                        exclude_from_suggestions: false,
                        exclusion_reason: None,
                    })
                })
            });

        suggestions.push(IssueSuggestion {
            number: issue.number,
            title: issue.title,
            priority: issue.priority,
            rationale: item.get("rationale")
                .and_then(|r| r.as_str())
                .unwrap_or("Suggested by priority ranking")
                .to_string(),
            complexity: item.get("complexity")
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
