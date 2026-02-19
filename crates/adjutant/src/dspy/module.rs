//! AdjutantModule - Composite dsrs module for 3-phase tiered execution.
//!
//! Implements Module, Evaluator, and Optimizable traits for MIPROv2 optimization.

use anyhow::Result;
use bon::Builder;
use dsrs::{
    Evaluator, Example, Module, Optimizable, Predict, Prediction, Predictor, Signature, example,
};
use indexmap::IndexMap;

use super::{combined_metric, get_execution_lm, get_planning_lm};

// ============================================================================
// Signatures (defined here because #[Signature] generates private structs)
// ============================================================================

/// Subtask Planning Signature - breaks a task into atomic subtasks.
#[Signature]
struct SubtaskPlanningSignature {
    /// Task Planner: Break the given task into concrete, atomic subtasks.
    /// Output ONLY valid JSON with a list of subtasks.
    /// Each subtask must have: id, action (read/edit/bash), target (file path), instruction.
    /// Keep subtasks atomic and focused. Order logically: read before edit, edit before test.
    /// Maximum 5 subtasks per task.

    /// Title of the task to accomplish
    #[input]
    pub task_title: String,

    /// Detailed description of what needs to be done
    #[input]
    pub task_description: String,

    /// Context handle or summary reference for large contexts
    #[input]
    pub context_handle: String,

    /// Repository context including relevant file contents
    #[input]
    pub context: String,

    /// JSON array of subtasks, each with id, action, target, instruction fields
    #[output]
    pub subtasks: String,

    /// Brief explanation of the planning approach
    #[output]
    pub reasoning: String,

    /// Confidence in the plan (0.0 to 1.0)
    #[output]
    pub confidence: f32,
}

/// Subtask Execution Signature - executes a single subtask.
#[Signature]
struct SubtaskExecutionSignature {
    /// Subtask Executor: Perform the given action and return the result.
    /// For "edit" actions: output JSON with old_string (exact text to find) and new_string (replacement).
    /// For "bash" actions: output JSON with command field.
    /// For "read" actions: summarize what you learned about the file.
    /// Be precise with old_string - it must match exactly what's in the file.

    /// Action type: read, edit, or bash
    #[input]
    pub action: String,

    /// Target file path (empty for bash actions)
    #[input]
    pub target: String,

    /// Instruction describing what to do
    #[input]
    pub instruction: String,

    /// Current file content (for read/edit actions)
    #[input]
    pub file_context: String,

    /// JSON result: {old_string, new_string} for edit, {command} for bash, or summary for read
    #[output]
    pub result: String,

    /// Explanation of what was done
    #[output]
    pub reasoning: String,

    /// Whether the action completed successfully
    #[output]
    pub success: bool,
}

/// Result Synthesis Signature - synthesizes subtask results into final outcome.
#[Signature]
struct ResultSynthesisSignature {
    /// Synthesis Agent: Given subtask results, determine if the overall task succeeded.
    /// Provide a concise but informative summary of what was accomplished or what failed.
    /// Output success as true only if all critical subtasks completed successfully.

    /// Original task title
    #[input]
    pub task_title: String,

    /// Formatted results from all subtasks (success/failure with details)
    #[input]
    pub subtask_results: String,

    /// Overall success: true if task is complete
    #[output]
    pub success: bool,

    /// Brief description of what was accomplished or what failed
    #[output]
    pub summary: String,

    /// JSON array of files that were modified
    #[output]
    pub modified_files: String,

    /// Confidence in the result assessment (0.0 to 1.0)
    #[output]
    pub confidence: f32,
}

// ============================================================================
// AdjutantModule
// ============================================================================

/// Composite module for Adjutant's 3-phase tiered execution.
///
/// Phases:
/// 1. Planning: Break task into atomic subtasks (GLM 4.7)
/// 2. Execution: Execute each subtask (Qwen-3-32B)
/// 3. Synthesis: Combine results into final outcome (GLM 4.7)
#[derive(Builder)]
pub struct AdjutantModule {
    #[builder(default = Predict::new(SubtaskPlanningSignature::new()))]
    pub planner: Predict,

    #[builder(default = Predict::new(SubtaskExecutionSignature::new()))]
    pub executor: Predict,

    #[builder(default = Predict::new(ResultSynthesisSignature::new()))]
    pub synthesizer: Predict,
}

impl Default for AdjutantModule {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl AdjutantModule {
    /// Create a new AdjutantModule with default predictors.
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute the planning phase.
    ///
    /// Takes task info and returns planned subtasks.
    pub async fn plan(
        &self,
        task_title: &str,
        task_description: &str,
        context_handle: &str,
        context: &str,
    ) -> Result<Prediction> {
        let input = example! {
            "task_title": "input" => task_title.to_string(),
            "task_description": "input" => task_description.to_string(),
            "context_handle": "input" => context_handle.to_string(),
            "context": "input" => context.to_string(),
        };

        // Use planning LM (GLM 4.7)
        let lm = get_planning_lm().await?;
        self.planner.forward_with_config(input, lm).await
    }

    /// Execute a single subtask.
    ///
    /// Takes subtask details and returns execution result.
    pub async fn execute_subtask(
        &self,
        action: &str,
        target: &str,
        instruction: &str,
        file_context: &str,
    ) -> Result<Prediction> {
        let input = example! {
            "action": "input" => action.to_string(),
            "target": "input" => target.to_string(),
            "instruction": "input" => instruction.to_string(),
            "file_context": "input" => file_context.to_string(),
        };

        // Use execution LM (Qwen-3-32B)
        let lm = get_execution_lm().await?;
        self.executor.forward_with_config(input, lm).await
    }

    /// Execute the synthesis phase.
    ///
    /// Takes task title and subtask results, returns final verdict.
    pub async fn synthesize(&self, task_title: &str, subtask_results: &str) -> Result<Prediction> {
        let input = example! {
            "task_title": "input" => task_title.to_string(),
            "subtask_results": "input" => subtask_results.to_string(),
        };

        // Use planning LM for synthesis (GLM 4.7)
        let lm = get_planning_lm().await?;
        self.synthesizer.forward_with_config(input, lm).await
    }
}

// ============================================================================
// Module Implementation
// ============================================================================

impl Module for AdjutantModule {
    /// Forward pass through all three phases.
    ///
    /// Input example should contain:
    /// - task_title: Title of the task
    /// - task_description: Detailed description
    /// - context: Repository context
    ///
    /// Returns prediction with:
    /// - subtasks: JSON array of planned subtasks
    /// - success: Overall success boolean
    /// - summary: Summary of what was accomplished
    /// - modified_files: JSON array of modified files
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        // Phase 1: Planning
        let task_title = inputs
            .data
            .get("task_title")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let task_description = inputs
            .data
            .get("task_description")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let context = inputs
            .data
            .get("context")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let context_handle = inputs
            .data
            .get("context_handle")
            .and_then(|v| v.as_str())
            .unwrap_or("inline");

        let plan_result = self
            .plan(task_title, task_description, context_handle, context)
            .await?;

        let subtasks = plan_result.get("subtasks", None);

        // Phase 2: Execution (simulate - in real usage, we'd iterate subtasks)
        // For module evaluation, we focus on planning + synthesis quality
        let subtask_results = format!(
            "Subtasks planned: {}. (Execution would happen here in full pipeline)",
            subtasks
        );

        // Phase 3: Synthesis
        let synthesis_result = self.synthesize(task_title, &subtask_results).await?;

        // Combine results
        let mut combined_data = plan_result.data.clone();
        for (key, value) in synthesis_result.data {
            combined_data.insert(key, value);
        }

        Ok(Prediction::new(
            combined_data,
            plan_result.lm_usage + synthesis_result.lm_usage,
        ))
    }
}

// ============================================================================
// Evaluator Implementation
// ============================================================================

impl Evaluator for AdjutantModule {
    /// Evaluate the quality of a prediction using combined metrics.
    async fn metric(&self, example: &Example, prediction: &Prediction) -> f32 {
        combined_metric(example, prediction)
    }
}

// ============================================================================
// Optimizable Implementation
// ============================================================================

impl Optimizable for AdjutantModule {
    fn parameters(&mut self) -> IndexMap<String, &mut dyn Optimizable> {
        let mut params = IndexMap::new();
        params.insert(
            "planner".to_string(),
            &mut self.planner as &mut dyn Optimizable,
        );
        params.insert(
            "executor".to_string(),
            &mut self.executor as &mut dyn Optimizable,
        );
        params.insert(
            "synthesizer".to_string(),
            &mut self.synthesizer as &mut dyn Optimizable,
        );
        params
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsrs::MetaSignature;

    #[test]
    fn test_module_builder() {
        let _module = AdjutantModule::builder().build();
    }

    #[test]
    fn test_module_default() {
        let _module = AdjutantModule::new();
    }

    #[test]
    fn test_optimizable_parameters() {
        let mut module = AdjutantModule::new();
        let params = module.parameters();
        assert!(params.contains_key("planner"));
        assert!(params.contains_key("executor"));
        assert!(params.contains_key("synthesizer"));
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_planning_signature_metadata() {
        let sig = SubtaskPlanningSignature::new();
        let instruction = sig.instruction();
        assert!(instruction.contains("Task Planner"));
        assert!(instruction.contains("subtasks"));
    }

    #[test]
    fn test_execution_signature_metadata() {
        let sig = SubtaskExecutionSignature::new();
        let instruction = sig.instruction();
        assert!(instruction.contains("Subtask Executor"));
    }

    #[test]
    fn test_synthesis_signature_metadata() {
        let sig = ResultSynthesisSignature::new();
        let instruction = sig.instruction();
        assert!(instruction.contains("Synthesis Agent"));
    }
}
