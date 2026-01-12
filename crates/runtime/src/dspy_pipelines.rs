//! DSPy Pipeline wrappers for Runtime tool management.
//!
//! This module provides pipeline structs that wrap the DSPy signatures
//! and can be used for intelligent tool selection and interpretation.

use crate::dspy_tools::{
    ToolChainPlanningSignature, ToolResultInterpretationSignature, ToolSelectionSignature,
    ToolSuccess,
};
use anyhow::Result;
use dsrs::{GLOBAL_SETTINGS, LM, Predict, Predictor, example};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Tool Selection Pipeline
// ============================================================================

/// Input for tool selection decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSelectionInput {
    /// What needs to be done - the user's request or task.
    pub task_description: String,
    /// JSON array of tool definitions with name, description, and parameters.
    pub available_tools: String,
    /// Recent tool results and conversation context.
    pub context: String,
}

/// Result from tool selection decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSelectionResult {
    /// Name of the tool to use.
    pub selected_tool: String,
    /// JSON parameters to pass to the tool.
    pub tool_params: String,
    /// What the tool should produce.
    pub expected_outcome: String,
    /// Alternative tool if primary fails.
    pub fallback_tool: String,
    /// Confidence in this selection (0.0-1.0).
    pub confidence: f32,
}

impl Default for ToolSelectionResult {
    fn default() -> Self {
        Self {
            selected_tool: String::new(),
            tool_params: "{}".to_string(),
            expected_outcome: String::new(),
            fallback_tool: String::new(),
            confidence: 0.0,
        }
    }
}

/// DSPy-powered tool selection pipeline.
pub struct ToolSelectionPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for ToolSelectionPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolSelectionPipeline {
    /// Create a new tool selection pipeline using the global LM.
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

    /// Select the best tool for a task.
    pub async fn select(&self, input: &ToolSelectionInput) -> Result<ToolSelectionResult> {
        // Check if we have an LM available (either local or global)
        if self.lm.is_none() && GLOBAL_SETTINGS.read().unwrap().is_none() {
            return Err(anyhow::anyhow!("No LM available for tool selection"));
        }

        let signature = ToolSelectionSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "task_description": "input" => input.task_description.clone(),
            "available_tools": "input" => input.available_tools.clone(),
            "context": "input" => input.context.clone(),
        };

        let prediction = if let Some(lm) = &self.lm {
            predictor.forward_with_config(example, lm.clone()).await?
        } else {
            predictor.forward(example).await?
        };

        Ok(ToolSelectionResult {
            selected_tool: Self::get_string(&prediction, "selected_tool"),
            tool_params: Self::get_string(&prediction, "tool_params"),
            expected_outcome: Self::get_string(&prediction, "expected_outcome"),
            fallback_tool: Self::get_string(&prediction, "fallback_tool"),
            confidence: Self::get_f32(&prediction, "confidence"),
        })
    }
}

// ============================================================================
// Tool Interpretation Pipeline
// ============================================================================

/// Input for tool result interpretation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInterpretationInput {
    /// Name of the tool that was called.
    pub tool_name: String,
    /// Raw output from the tool.
    pub tool_output: String,
    /// What we were trying to accomplish.
    pub original_intent: String,
}

/// Result from tool interpretation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInterpretationResult {
    /// Whether the tool succeeded (YES, PARTIAL, NO).
    pub success: ToolSuccess,
    /// Key information extracted from the output.
    pub extracted_info: String,
    /// What to do next based on the result.
    pub next_steps: String,
    /// If failed, explanation and suggested fixes.
    pub error_analysis: String,
    /// Confidence in this interpretation (0.0-1.0).
    pub confidence: f32,
}

impl Default for ToolInterpretationResult {
    fn default() -> Self {
        Self {
            success: ToolSuccess::No,
            extracted_info: String::new(),
            next_steps: String::new(),
            error_analysis: String::new(),
            confidence: 0.0,
        }
    }
}

/// DSPy-powered tool result interpretation pipeline.
pub struct ToolInterpretationPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for ToolInterpretationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolInterpretationPipeline {
    /// Create a new tool interpretation pipeline using the global LM.
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

    /// Interpret a tool result.
    pub async fn interpret(
        &self,
        input: &ToolInterpretationInput,
    ) -> Result<ToolInterpretationResult> {
        // Check if we have an LM available (either local or global)
        if self.lm.is_none() && GLOBAL_SETTINGS.read().unwrap().is_none() {
            return Err(anyhow::anyhow!("No LM available for tool interpretation"));
        }

        let signature = ToolResultInterpretationSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "tool_name": "input" => input.tool_name.clone(),
            "tool_output": "input" => input.tool_output.clone(),
            "original_intent": "input" => input.original_intent.clone(),
        };

        let prediction = if let Some(lm) = &self.lm {
            predictor.forward_with_config(example, lm.clone()).await?
        } else {
            predictor.forward(example).await?
        };

        let success_str = Self::get_string(&prediction, "success");

        Ok(ToolInterpretationResult {
            success: success_str.parse().unwrap_or(ToolSuccess::No),
            extracted_info: Self::get_string(&prediction, "extracted_info"),
            next_steps: Self::get_string(&prediction, "next_steps"),
            error_analysis: Self::get_string(&prediction, "error_analysis"),
            confidence: Self::get_f32(&prediction, "confidence"),
        })
    }
}

// ============================================================================
// Tool Chain Pipeline
// ============================================================================

/// Input for tool chain planning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChainInput {
    /// What we're trying to accomplish.
    pub goal: String,
    /// JSON array of tool definitions.
    pub available_tools: String,
    /// Time, cost, and resource constraints as JSON.
    pub constraints: String,
}

/// Result from tool chain planning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChainResult {
    /// JSON array of tool calls in execution order.
    pub tool_sequence: String,
    /// JSON object mapping step_id to required prior step_ids.
    pub dependencies: String,
    /// JSON array of groups of step_ids that can run in parallel.
    pub parallelizable: String,
    /// Confidence in this plan (0.0-1.0).
    pub confidence: f32,
}

impl Default for ToolChainResult {
    fn default() -> Self {
        Self {
            tool_sequence: "[]".to_string(),
            dependencies: "{}".to_string(),
            parallelizable: "[]".to_string(),
            confidence: 0.0,
        }
    }
}

/// DSPy-powered tool chain planning pipeline.
pub struct ToolChainPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for ToolChainPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolChainPipeline {
    /// Create a new tool chain planning pipeline using the global LM.
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

    /// Plan a multi-tool sequence.
    pub async fn plan(&self, input: &ToolChainInput) -> Result<ToolChainResult> {
        // Check if we have an LM available (either local or global)
        if self.lm.is_none() && GLOBAL_SETTINGS.read().unwrap().is_none() {
            return Err(anyhow::anyhow!("No LM available for tool chain planning"));
        }

        let signature = ToolChainPlanningSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "goal": "input" => input.goal.clone(),
            "available_tools": "input" => input.available_tools.clone(),
            "constraints": "input" => input.constraints.clone(),
        };

        let prediction = if let Some(lm) = &self.lm {
            predictor.forward_with_config(example, lm.clone()).await?
        } else {
            predictor.forward(example).await?
        };

        Ok(ToolChainResult {
            tool_sequence: Self::get_string(&prediction, "tool_sequence"),
            dependencies: Self::get_string(&prediction, "dependencies"),
            parallelizable: Self::get_string(&prediction, "parallelizable"),
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
    fn test_tool_selection_input_serialization() {
        let input = ToolSelectionInput {
            task_description: "Read a file".to_string(),
            available_tools: r#"[{"name": "Read"}]"#.to_string(),
            context: "".to_string(),
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: ToolSelectionInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.task_description, input.task_description);
    }

    #[test]
    fn test_tool_selection_result_default() {
        let result = ToolSelectionResult::default();
        assert!(result.selected_tool.is_empty());
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_tool_interpretation_input_serialization() {
        let input = ToolInterpretationInput {
            tool_name: "Read".to_string(),
            tool_output: "file contents".to_string(),
            original_intent: "Read the config".to_string(),
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: ToolInterpretationInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tool_name, input.tool_name);
    }

    #[test]
    fn test_tool_interpretation_result_default() {
        let result = ToolInterpretationResult::default();
        assert_eq!(result.success, ToolSuccess::No);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_tool_chain_input_serialization() {
        let input = ToolChainInput {
            goal: "Refactor code".to_string(),
            available_tools: r#"[{"name": "Read"}, {"name": "Write"}]"#.to_string(),
            constraints: r#"{"max_time_ms": 60000}"#.to_string(),
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: ToolChainInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.goal, input.goal);
    }

    #[test]
    fn test_tool_chain_result_default() {
        let result = ToolChainResult::default();
        assert_eq!(result.tool_sequence, "[]");
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_tool_selection_pipeline_creation() {
        let pipeline = ToolSelectionPipeline::new();
        assert!(pipeline.lm.is_none());
    }

    #[test]
    fn test_tool_interpretation_pipeline_creation() {
        let pipeline = ToolInterpretationPipeline::new();
        assert!(pipeline.lm.is_none());
    }

    #[test]
    fn test_tool_chain_pipeline_creation() {
        let pipeline = ToolChainPipeline::new();
        assert!(pipeline.lm.is_none());
    }
}
