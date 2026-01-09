//! DSPy Pipeline wrappers for OANIX.
//!
//! This module provides pipeline structs that wrap the DSPy signatures
//! and can be used by the tick loop and state management.

use crate::dspy_lifecycle::{Complexity, IssueSelectionSignature};
use crate::dspy_situation::{PriorityAction, SituationAssessmentSignature, Urgency};
use anyhow::Result;
use dsrs::{example, LM, Predict, Predictor};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Situation Assessment Pipeline
// ============================================================================

/// Input for situation assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SituationInput {
    /// System state as JSON (from OanixManifest).
    pub system_state: String,
    /// Pending events as JSON array.
    pub pending_events: String,
    /// Recent decisions as JSON array.
    pub recent_history: String,
}

/// Result from situation assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SituationResult {
    /// Priority action to take.
    pub priority_action: PriorityAction,
    /// Urgency level.
    pub urgency: Urgency,
    /// Reasoning for the decision.
    pub reasoning: String,
    /// Confidence in this assessment (0.0-1.0).
    pub confidence: f32,
}

/// DSPy-powered situation assessment pipeline.
pub struct SituationPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for SituationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl SituationPipeline {
    /// Create a new situation pipeline using the global LM.
    pub fn new() -> Self {
        Self { lm: None }
    }

    /// Create a pipeline with a specific LM.
    pub fn with_lm(lm: Arc<LM>) -> Self {
        Self { lm: Some(lm) }
    }

    /// Helper to get string from prediction value.
    fn get_string(prediction: &dsrs::Prediction, key: &str) -> String {
        let val = prediction.get(key, None);
        if let Some(s) = val.as_str() {
            s.to_string()
        } else {
            val.to_string().trim_matches('"').to_string()
        }
    }

    /// Helper to get f32 from prediction value.
    fn get_f32(prediction: &dsrs::Prediction, key: &str) -> f32 {
        let val = prediction.get(key, None);
        if let Some(n) = val.as_f64() {
            n as f32
        } else if let Some(s) = val.as_str() {
            s.parse().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// Assess the current situation and recommend next action.
    pub async fn assess(&self, input: &SituationInput) -> Result<SituationResult> {
        // Check if we have an LM available (either local or global)
        if self.lm.is_none() && dsrs::GLOBAL_SETTINGS.read().unwrap().is_none() {
            return Err(anyhow::anyhow!("No LM available for DSPy assessment"));
        }

        let signature = SituationAssessmentSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "system_state": "input" => input.system_state.clone(),
            "pending_events": "input" => input.pending_events.clone(),
            "recent_history": "input" => input.recent_history.clone(),
        };

        let prediction = if let Some(lm) = &self.lm {
            predictor.forward_with_config(example, lm.clone()).await?
        } else {
            predictor.forward(example).await?
        };

        let action_str = Self::get_string(&prediction, "priority_action");
        let urgency_str = Self::get_string(&prediction, "urgency");

        Ok(SituationResult {
            priority_action: action_str.parse().unwrap_or(PriorityAction::AwaitUser),
            urgency: urgency_str.parse().unwrap_or(Urgency::Normal),
            reasoning: Self::get_string(&prediction, "reasoning"),
            confidence: Self::get_f32(&prediction, "confidence"),
        })
    }
}

// ============================================================================
// Issue Selection Pipeline
// ============================================================================

/// Input for issue selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSelectionInput {
    /// Available issues as JSON array.
    pub available_issues: String,
    /// Agent capabilities description.
    pub agent_capabilities: String,
    /// Current repository context.
    pub current_context: String,
}

/// Result from issue selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSelectionResult {
    /// Selected issue number as string.
    pub selected_issue: String,
    /// Rationale for selection.
    pub rationale: String,
    /// Estimated complexity.
    pub complexity: Complexity,
    /// Confidence in selection (0.0-1.0).
    pub confidence: f32,
}

/// DSPy-powered issue selection pipeline.
pub struct IssueSelectionPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for IssueSelectionPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl IssueSelectionPipeline {
    /// Create a new issue selection pipeline using the global LM.
    pub fn new() -> Self {
        Self { lm: None }
    }

    /// Create a pipeline with a specific LM.
    pub fn with_lm(lm: Arc<LM>) -> Self {
        Self { lm: Some(lm) }
    }

    /// Helper to get string from prediction value.
    fn get_string(prediction: &dsrs::Prediction, key: &str) -> String {
        let val = prediction.get(key, None);
        if let Some(s) = val.as_str() {
            s.to_string()
        } else {
            val.to_string().trim_matches('"').to_string()
        }
    }

    /// Helper to get f32 from prediction value.
    fn get_f32(prediction: &dsrs::Prediction, key: &str) -> f32 {
        let val = prediction.get(key, None);
        if let Some(n) = val.as_f64() {
            n as f32
        } else if let Some(s) = val.as_str() {
            s.parse().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// Select the best issue to work on.
    pub async fn select(&self, input: &IssueSelectionInput) -> Result<IssueSelectionResult> {
        // Check if we have an LM available (either local or global)
        if self.lm.is_none() && dsrs::GLOBAL_SETTINGS.read().unwrap().is_none() {
            return Err(anyhow::anyhow!("No LM available for DSPy issue selection"));
        }

        let signature = IssueSelectionSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "available_issues": "input" => input.available_issues.clone(),
            "agent_capabilities": "input" => input.agent_capabilities.clone(),
            "current_context": "input" => input.current_context.clone(),
        };

        let prediction = if let Some(lm) = &self.lm {
            predictor.forward_with_config(example, lm.clone()).await?
        } else {
            predictor.forward(example).await?
        };

        let complexity_str = Self::get_string(&prediction, "estimated_complexity");

        Ok(IssueSelectionResult {
            selected_issue: Self::get_string(&prediction, "selected_issue"),
            rationale: Self::get_string(&prediction, "rationale"),
            complexity: complexity_str.parse().unwrap_or(Complexity::Medium),
            confidence: Self::get_f32(&prediction, "confidence"),
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_situation_input_serialization() {
        let input = SituationInput {
            system_state: r#"{"hardware": {"cpu_cores": 8}}"#.to_string(),
            pending_events: "[]".to_string(),
            recent_history: "[]".to_string(),
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: SituationInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.system_state, input.system_state);
    }

    #[test]
    fn test_issue_selection_input_serialization() {
        let input = IssueSelectionInput {
            available_issues: r#"[{"number": 1, "title": "Fix bug"}]"#.to_string(),
            agent_capabilities: "[]".to_string(),
            current_context: "main branch".to_string(),
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: IssueSelectionInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.available_issues, input.available_issues);
    }

    #[test]
    fn test_situation_pipeline_creation() {
        let pipeline = SituationPipeline::new();
        assert!(pipeline.lm.is_none());
    }

    #[test]
    fn test_issue_selection_pipeline_creation() {
        let pipeline = IssueSelectionPipeline::new();
        assert!(pipeline.lm.is_none());
    }
}
