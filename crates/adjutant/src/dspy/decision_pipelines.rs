//! Decision Pipeline wrappers for Adjutant routing.
//!
//! Pipeline wrappers for decision-related DSPy signatures:
//! - ComplexityPipeline: Classify task complexity
//! - DelegationPipeline: Decide whether to delegate
//! - RlmTriggerPipeline: Decide whether to use RLM

use super::get_planning_lm;
use anyhow::Result;
use dsrs::{example, Predict, Prediction, Predictor, Signature, LM, GLOBAL_SETTINGS};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Signatures (defined here because #[Signature] generates private structs)
// ============================================================================

/// Complexity Classification - Determine task complexity level.
#[Signature]
struct ComplexityClassificationSignature {
    /// Complexity Classifier: Analyze the task and classify its complexity level.
    /// Consider the scope of changes, number of files, architectural impact, and risk.
    /// Output one of: Low, Medium, High, VeryHigh.
    /// - Low: Simple single-file edit, minimal risk
    /// - Medium: Multi-file edit, moderate scope
    /// - High: Complex refactoring, many files, architectural changes
    /// - VeryHigh: Massive scope, system-wide changes, high risk

    /// Description of the task to classify
    #[input]
    pub task_description: String,

    /// Number of files likely to be affected
    #[input]
    pub file_count: String,

    /// Estimated token count for context
    #[input]
    pub estimated_tokens: String,

    /// Keywords found in task (refactor, migrate, rewrite, etc.)
    #[input]
    pub keywords: String,

    /// Complexity level: Low, Medium, High, or VeryHigh
    #[output]
    pub complexity: String,

    /// Explanation of the classification reasoning
    #[output]
    pub reasoning: String,

    /// Confidence in this classification (0.0 to 1.0)
    #[output]
    pub confidence: f32,
}

/// Delegation Decision - Determine whether to delegate task execution.
#[Signature]
struct DelegationDecisionSignature {
    /// Delegation Decider: Determine if this task should be delegated to another executor.
    /// Consider task complexity, context size, and available capabilities.
    /// Output should_delegate as true if the task requires external help.
    /// For delegation_target, choose one of: codex_code, rlm, local_tools.
    /// - codex_code: Complex multi-file tasks, architectural work
    /// - rlm: Large context analysis, recursive investigation
    /// - local_tools: Simple edits, small scope tasks

    /// Description of the task
    #[input]
    pub task_description: String,

    /// Classified complexity level (Low/Medium/High/VeryHigh)
    #[input]
    pub complexity: String,

    /// Number of files involved
    #[input]
    pub file_count: String,

    /// Estimated token count for context
    #[input]
    pub estimated_tokens: String,

    /// Whether to delegate this task to another executor
    #[output]
    pub should_delegate: bool,

    /// Target executor if delegating: codex_code, rlm, or local_tools
    #[output]
    pub delegation_target: String,

    /// Explanation of the delegation decision
    #[output]
    pub reasoning: String,

    /// Confidence in this decision (0.0 to 1.0)
    #[output]
    pub confidence: f32,
}

/// RLM Trigger Decision - Determine whether to use Recursive Language Model.
#[Signature]
struct RlmTriggerSignature {
    /// RLM Trigger: Decide if this task benefits from Recursive Language Model analysis.
    /// RLM is good for: deep code analysis, recursive investigation, security audits,
    /// comprehensive reviews, finding all occurrences, understanding complex systems.
    /// RLM is overkill for: simple edits, single-file changes, well-scoped tasks.
    /// Output use_rlm as true if RLM would significantly improve results.

    /// Description of the task
    #[input]
    pub task_description: String,

    /// Classified complexity level (Low/Medium/High/VeryHigh)
    #[input]
    pub complexity: String,

    /// Estimated token count for context
    #[input]
    pub estimated_tokens: String,

    /// Whether to use RLM for this task
    #[output]
    pub use_rlm: bool,

    /// Explanation of the RLM decision
    #[output]
    pub reasoning: String,

    /// Confidence in this decision (0.0 to 1.0)
    #[output]
    pub confidence: f32,
}

// ============================================================================
// Complexity Pipeline
// ============================================================================

/// Input for complexity classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityInput {
    /// Description of the task
    pub task_description: String,
    /// Number of files involved
    pub file_count: u32,
    /// Estimated token count
    pub estimated_tokens: usize,
    /// Keywords found in task
    pub keywords: Vec<String>,
}

/// Result from complexity classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityResult {
    /// Complexity level: Low, Medium, High, VeryHigh
    pub complexity: String,
    /// Explanation of classification
    pub reasoning: String,
    /// Confidence in classification (0.0-1.0)
    pub confidence: f32,
}

impl Default for ComplexityResult {
    fn default() -> Self {
        Self {
            complexity: "Medium".to_string(),
            reasoning: String::new(),
            confidence: 0.0,
        }
    }
}

/// DSPy-powered complexity classification pipeline.
pub struct ComplexityPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for ComplexityPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl ComplexityPipeline {
    /// Create a new complexity pipeline using the global LM.
    pub fn new() -> Self {
        Self { lm: None }
    }

    /// Create a pipeline with a specific LM.
    pub fn with_lm(lm: Arc<LM>) -> Self {
        Self { lm: Some(lm) }
    }

    /// Classify task complexity.
    pub async fn classify(&self, input: &ComplexityInput) -> Result<ComplexityResult> {
        // Get LM - either use provided one or auto-detect
        let lm = if let Some(lm) = &self.lm {
            lm.clone()
        } else if GLOBAL_SETTINGS.read().unwrap().is_some() {
            // Use global settings
            return self.classify_with_global(input).await;
        } else {
            // Auto-detect provider
            get_planning_lm().await?
        };

        let signature = ComplexityClassificationSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "task_description": "input" => input.task_description.clone(),
            "file_count": "input" => input.file_count.to_string(),
            "estimated_tokens": "input" => input.estimated_tokens.to_string(),
            "keywords": "input" => input.keywords.join(", "),
        };

        let prediction = predictor.forward_with_config(example, lm).await?;

        Ok(ComplexityResult {
            complexity: get_string(&prediction, "complexity"),
            reasoning: get_string(&prediction, "reasoning"),
            confidence: get_f32(&prediction, "confidence"),
        })
    }

    async fn classify_with_global(&self, input: &ComplexityInput) -> Result<ComplexityResult> {
        let signature = ComplexityClassificationSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "task_description": "input" => input.task_description.clone(),
            "file_count": "input" => input.file_count.to_string(),
            "estimated_tokens": "input" => input.estimated_tokens.to_string(),
            "keywords": "input" => input.keywords.join(", "),
        };

        let prediction = predictor.forward(example).await?;

        Ok(ComplexityResult {
            complexity: get_string(&prediction, "complexity"),
            reasoning: get_string(&prediction, "reasoning"),
            confidence: get_f32(&prediction, "confidence"),
        })
    }
}

// ============================================================================
// Delegation Pipeline
// ============================================================================

/// Input for delegation decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationInput {
    /// Description of the task
    pub task_description: String,
    /// Complexity level (from ComplexityPipeline)
    pub complexity: String,
    /// Number of files involved
    pub file_count: u32,
    /// Estimated token count
    pub estimated_tokens: usize,
}

/// Result from delegation decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationResult {
    /// Whether to delegate this task
    pub should_delegate: bool,
    /// Target executor: codex_code, rlm, or local_tools
    pub delegation_target: String,
    /// Explanation of decision
    pub reasoning: String,
    /// Confidence in decision (0.0-1.0)
    pub confidence: f32,
}

impl Default for DelegationResult {
    fn default() -> Self {
        Self {
            should_delegate: false,
            delegation_target: "local_tools".to_string(),
            reasoning: String::new(),
            confidence: 0.0,
        }
    }
}

/// DSPy-powered delegation decision pipeline.
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

    /// Decide whether to delegate task execution.
    pub async fn decide(&self, input: &DelegationInput) -> Result<DelegationResult> {
        // Get LM - either use provided one or auto-detect
        let lm = if let Some(lm) = &self.lm {
            lm.clone()
        } else if GLOBAL_SETTINGS.read().unwrap().is_some() {
            return self.decide_with_global(input).await;
        } else {
            get_planning_lm().await?
        };

        let signature = DelegationDecisionSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "task_description": "input" => input.task_description.clone(),
            "complexity": "input" => input.complexity.clone(),
            "file_count": "input" => input.file_count.to_string(),
            "estimated_tokens": "input" => input.estimated_tokens.to_string(),
        };

        let prediction = predictor.forward_with_config(example, lm).await?;

        Ok(DelegationResult {
            should_delegate: get_bool(&prediction, "should_delegate"),
            delegation_target: get_string(&prediction, "delegation_target"),
            reasoning: get_string(&prediction, "reasoning"),
            confidence: get_f32(&prediction, "confidence"),
        })
    }

    async fn decide_with_global(&self, input: &DelegationInput) -> Result<DelegationResult> {
        let signature = DelegationDecisionSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "task_description": "input" => input.task_description.clone(),
            "complexity": "input" => input.complexity.clone(),
            "file_count": "input" => input.file_count.to_string(),
            "estimated_tokens": "input" => input.estimated_tokens.to_string(),
        };

        let prediction = predictor.forward(example).await?;

        Ok(DelegationResult {
            should_delegate: get_bool(&prediction, "should_delegate"),
            delegation_target: get_string(&prediction, "delegation_target"),
            reasoning: get_string(&prediction, "reasoning"),
            confidence: get_f32(&prediction, "confidence"),
        })
    }
}

// ============================================================================
// RLM Trigger Pipeline
// ============================================================================

/// Input for RLM trigger decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmTriggerInput {
    /// Description of the task
    pub task_description: String,
    /// Complexity level (from ComplexityPipeline)
    pub complexity: String,
    /// Estimated token count
    pub estimated_tokens: usize,
}

/// Result from RLM trigger decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmTriggerResult {
    /// Whether to use RLM
    pub use_rlm: bool,
    /// Explanation of decision
    pub reasoning: String,
    /// Confidence in decision (0.0-1.0)
    pub confidence: f32,
}

impl Default for RlmTriggerResult {
    fn default() -> Self {
        Self {
            use_rlm: false,
            reasoning: String::new(),
            confidence: 0.0,
        }
    }
}

/// DSPy-powered RLM trigger decision pipeline.
pub struct RlmTriggerPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for RlmTriggerPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl RlmTriggerPipeline {
    /// Create a new RLM trigger pipeline using the global LM.
    pub fn new() -> Self {
        Self { lm: None }
    }

    /// Create a pipeline with a specific LM.
    pub fn with_lm(lm: Arc<LM>) -> Self {
        Self { lm: Some(lm) }
    }

    /// Decide whether to use RLM for this task.
    pub async fn should_trigger(&self, input: &RlmTriggerInput) -> Result<RlmTriggerResult> {
        // Get LM - either use provided one or auto-detect
        let lm = if let Some(lm) = &self.lm {
            lm.clone()
        } else if GLOBAL_SETTINGS.read().unwrap().is_some() {
            return self.should_trigger_with_global(input).await;
        } else {
            get_planning_lm().await?
        };

        let signature = RlmTriggerSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "task_description": "input" => input.task_description.clone(),
            "complexity": "input" => input.complexity.clone(),
            "estimated_tokens": "input" => input.estimated_tokens.to_string(),
        };

        let prediction = predictor.forward_with_config(example, lm).await?;

        Ok(RlmTriggerResult {
            use_rlm: get_bool(&prediction, "use_rlm"),
            reasoning: get_string(&prediction, "reasoning"),
            confidence: get_f32(&prediction, "confidence"),
        })
    }

    async fn should_trigger_with_global(&self, input: &RlmTriggerInput) -> Result<RlmTriggerResult> {
        let signature = RlmTriggerSignature::new();
        let predictor = Predict::new(signature);

        let example = example! {
            "task_description": "input" => input.task_description.clone(),
            "complexity": "input" => input.complexity.clone(),
            "estimated_tokens": "input" => input.estimated_tokens.to_string(),
        };

        let prediction = predictor.forward(example).await?;

        Ok(RlmTriggerResult {
            use_rlm: get_bool(&prediction, "use_rlm"),
            reasoning: get_string(&prediction, "reasoning"),
            confidence: get_f32(&prediction, "confidence"),
        })
    }
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

/// Helper to get bool from prediction value.
fn get_bool(prediction: &Prediction, key: &str) -> bool {
    let val = prediction.get(key, None);
    if let Some(b) = val.as_bool() {
        b
    } else if let Some(s) = val.as_str() {
        s.to_lowercase() == "true" || s == "1"
    } else {
        false
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use dsrs::MetaSignature;

    #[test]
    fn test_complexity_signature_metadata() {
        let sig = ComplexityClassificationSignature::new();
        let instruction = sig.instruction();
        assert!(instruction.contains("Complexity Classifier"));
    }

    #[test]
    fn test_delegation_signature_metadata() {
        let sig = DelegationDecisionSignature::new();
        let instruction = sig.instruction();
        assert!(instruction.contains("Delegation Decider"));
    }

    #[test]
    fn test_rlm_trigger_signature_metadata() {
        let sig = RlmTriggerSignature::new();
        let instruction = sig.instruction();
        assert!(instruction.contains("RLM Trigger"));
    }

    #[test]
    fn test_complexity_input_serialization() {
        let input = ComplexityInput {
            task_description: "Refactor auth module".to_string(),
            file_count: 5,
            estimated_tokens: 10000,
            keywords: vec!["refactor".to_string(), "auth".to_string()],
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: ComplexityInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.task_description, input.task_description);
    }

    #[test]
    fn test_complexity_result_default() {
        let result = ComplexityResult::default();
        assert_eq!(result.complexity, "Medium");
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_delegation_input_serialization() {
        let input = DelegationInput {
            task_description: "Fix login bug".to_string(),
            complexity: "Low".to_string(),
            file_count: 2,
            estimated_tokens: 5000,
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: DelegationInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.complexity, input.complexity);
    }

    #[test]
    fn test_delegation_result_default() {
        let result = DelegationResult::default();
        assert!(!result.should_delegate);
        assert_eq!(result.delegation_target, "local_tools");
    }

    #[test]
    fn test_rlm_trigger_input_serialization() {
        let input = RlmTriggerInput {
            task_description: "Analyze security vulnerabilities".to_string(),
            complexity: "High".to_string(),
            estimated_tokens: 50000,
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: RlmTriggerInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.task_description, input.task_description);
    }

    #[test]
    fn test_rlm_trigger_result_default() {
        let result = RlmTriggerResult::default();
        assert!(!result.use_rlm);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_complexity_pipeline_creation() {
        let pipeline = ComplexityPipeline::new();
        assert!(pipeline.lm.is_none());
    }

    #[test]
    fn test_delegation_pipeline_creation() {
        let pipeline = DelegationPipeline::new();
        assert!(pipeline.lm.is_none());
    }

    #[test]
    fn test_rlm_trigger_pipeline_creation() {
        let pipeline = RlmTriggerPipeline::new();
        assert!(pipeline.lm.is_none());
    }
}
