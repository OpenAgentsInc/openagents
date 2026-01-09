//! DSPy Pipeline wrappers for Agent Orchestrator.
//!
//! This module provides pipeline structs that wrap the DSPy signatures
//! and can be used by the registry and background task manager.

use crate::dspy_delegation::{DelegationSignature, TargetAgent};
use anyhow::Result;
use dsrs::{example, Predict, Predictor, LM, GLOBAL_SETTINGS};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Delegation Pipeline
// ============================================================================

/// Input for delegation decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationInput {
    /// What needs to be done - the user's request or task.
    pub task_description: String,
    /// JSON object with agent capabilities, models, and permissions.
    pub available_agents: String,
    /// JSON object showing what each agent is currently doing.
    pub current_workload: String,
}

/// Result from delegation decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationResult {
    /// Which agent to delegate to.
    pub assigned_agent: TargetAgent,
    /// Refined, specific instructions for the assigned agent.
    pub task_refinement: String,
    /// Concrete list of what the agent should produce.
    pub expected_deliverables: String,
    /// Alternative agent if primary fails.
    pub fallback_agent: TargetAgent,
    /// Confidence in this delegation decision (0.0-1.0).
    pub confidence: f32,
}

impl Default for DelegationResult {
    fn default() -> Self {
        Self {
            assigned_agent: TargetAgent::Direct,
            task_refinement: String::new(),
            expected_deliverables: String::new(),
            fallback_agent: TargetAgent::Direct,
            confidence: 0.0,
        }
    }
}

/// DSPy-powered delegation pipeline.
pub struct DelegationPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for DelegationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl DelegationPipeline {
    /// Create a new delegation pipeline using the global LM.
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

    /// Decide which agent should handle a task.
    pub async fn delegate(&self, input: &DelegationInput) -> Result<DelegationResult> {
        // Check if we have an LM available (either local or global)
        if self.lm.is_none() && GLOBAL_SETTINGS.read().unwrap().is_none() {
            return Err(anyhow::anyhow!("No LM available for delegation"));
        }

        let signature = DelegationSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "task_description": "input" => input.task_description.clone(),
            "available_agents": "input" => input.available_agents.clone(),
            "current_workload": "input" => input.current_workload.clone(),
        };

        let prediction = if let Some(lm) = &self.lm {
            predictor.forward_with_config(example, lm.clone()).await?
        } else {
            predictor.forward(example).await?
        };

        let assigned_str = Self::get_string(&prediction, "assigned_agent");
        let fallback_str = Self::get_string(&prediction, "fallback_agent");

        Ok(DelegationResult {
            assigned_agent: assigned_str.parse().unwrap_or(TargetAgent::Direct),
            task_refinement: Self::get_string(&prediction, "task_refinement"),
            expected_deliverables: Self::get_string(&prediction, "expected_deliverables"),
            fallback_agent: fallback_str.parse().unwrap_or(TargetAgent::Direct),
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
    fn test_delegation_input_serialization() {
        let input = DelegationInput {
            task_description: "Find the auth code".to_string(),
            available_agents: r#"{"oracle": "available"}"#.to_string(),
            current_workload: "{}".to_string(),
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: DelegationInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.task_description, input.task_description);
    }

    #[test]
    fn test_delegation_result_default() {
        let result = DelegationResult::default();
        assert_eq!(result.assigned_agent, TargetAgent::Direct);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_delegation_pipeline_creation() {
        let pipeline = DelegationPipeline::new();
        assert!(pipeline.lm.is_none());
    }
}
