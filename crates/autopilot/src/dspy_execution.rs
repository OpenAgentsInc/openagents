//! DSPy-powered execution pipeline for autopilot.
//!
//! Uses typed signatures to decide how to execute plan steps:
//!
//! 1. **ExecutionStrategySignature** - Decide next action for a plan step
//! 2. **ToolSelectionSignature** - Choose the right tool for a task
//!
//! # Example
//!
//! ```rust,ignore
//! use autopilot::dspy_execution::{ExecutionPipeline, ExecutionInput};
//!
//! let pipeline = ExecutionPipeline::new();
//! let input = ExecutionInput {
//!     plan_step: "Add logout button to header".to_string(),
//!     current_file_state: Some("// header.rs contents...".to_string()),
//!     execution_history: "[]".to_string(),
//! };
//!
//! let result = pipeline.decide(&input).await?;
//! println!("Next action: {:?}", result.next_action);
//! ```

use dspy_rs::{example, LM, Predict, Predictor, Signature};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Signature Definitions
// ============================================================================

/// Execution strategy signature - decides how to implement a plan step.
#[Signature]
struct ExecutionStrategySignature {
    /// Execution Strategy: Decide the next action to implement a plan step.
    /// Consider the current state and execution history.

    /// Current step from the implementation plan
    #[input]
    plan_step: String,

    /// Current contents of the file being edited (if applicable)
    #[input]
    current_file_state: String,

    /// JSON array of previous tool calls and their results
    #[input]
    execution_history: String,

    /// Next action: EDIT_FILE, RUN_COMMAND, READ_FILE, or COMPLETE
    #[output]
    next_action: String,

    /// JSON object with parameters for the action
    #[output]
    action_params: String,

    /// Explanation of why this action was chosen
    #[output]
    reasoning: String,

    /// Estimated progress on this step (0.0-1.0)
    #[output]
    progress_estimate: f32,
}

/// Tool selection signature - chooses the right tool for a task.
#[Signature]
struct ToolSelectionSignature {
    /// Tool Selection: Choose the most appropriate tool for the current task.
    /// Consider available tools and recent context.

    /// Description of what needs to be done
    #[input]
    task_description: String,

    /// JSON array of available tool definitions
    #[input]
    available_tools: String,

    /// Recent tool call results for context
    #[input]
    recent_context: String,

    /// Name of the selected tool
    #[output]
    selected_tool: String,

    /// JSON object with tool parameters
    #[output]
    tool_params: String,

    /// What we expect this tool call to achieve
    #[output]
    expected_outcome: String,

    /// Alternative tool if primary fails
    #[output]
    fallback_tool: String,
}

/// Error recovery signature - handles execution failures.
#[Signature]
struct ErrorRecoverySignature {
    /// Error Recovery: Analyze execution failure and decide recovery strategy.

    /// The action that failed
    #[input]
    failed_action: String,

    /// Error message from the failure
    #[input]
    error_message: String,

    /// What we were trying to accomplish
    #[input]
    original_intent: String,

    /// Recovery strategy: RETRY, ALTERNATIVE, SKIP, or ABORT
    #[output]
    recovery_strategy: String,

    /// Modified action or alternative approach
    #[output]
    recovery_action: String,

    /// Why this recovery approach was chosen
    #[output]
    reasoning: String,

    /// Confidence in recovery success (0.0-1.0)
    #[output]
    confidence: f32,
}

// ============================================================================
// Pipeline Types
// ============================================================================

/// Possible next actions during execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionAction {
    /// Edit a file with specific changes
    EditFile,
    /// Run a shell command
    RunCommand,
    /// Read a file for context
    ReadFile,
    /// Step is complete
    Complete,
    /// Unknown or invalid action
    Unknown,
}

impl From<&str> for ExecutionAction {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "EDIT_FILE" | "EDIT" | "WRITE" => Self::EditFile,
            "RUN_COMMAND" | "RUN" | "COMMAND" | "BASH" => Self::RunCommand,
            "READ_FILE" | "READ" => Self::ReadFile,
            "COMPLETE" | "DONE" | "FINISHED" => Self::Complete,
            _ => Self::Unknown,
        }
    }
}

/// Recovery strategy for failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    /// Retry the same action
    Retry,
    /// Try an alternative approach
    Alternative,
    /// Skip this step and continue
    Skip,
    /// Abort the execution
    Abort,
}

impl From<&str> for RecoveryStrategy {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "RETRY" => Self::Retry,
            "ALTERNATIVE" | "ALT" => Self::Alternative,
            "SKIP" => Self::Skip,
            "ABORT" | "FAIL" | "STOP" => Self::Abort,
            _ => Self::Retry, // Default to retry
        }
    }
}

/// Input to the execution pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionInput {
    /// Current step from the plan
    pub plan_step: String,
    /// Current file contents (if editing)
    pub current_file_state: Option<String>,
    /// Previous tool calls and results (JSON)
    pub execution_history: String,
}

/// Result from execution decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionDecision {
    /// What action to take next
    pub next_action: ExecutionAction,
    /// Parameters for the action (JSON)
    pub action_params: serde_json::Value,
    /// Explanation of the decision
    pub reasoning: String,
    /// Estimated progress (0.0-1.0)
    pub progress_estimate: f32,
}

/// Input for tool selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSelectionInput {
    /// What needs to be done
    pub task_description: String,
    /// Available tools (as JSON)
    pub available_tools: String,
    /// Recent context from tool calls
    pub recent_context: String,
}

/// Result from tool selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSelectionResult {
    /// Selected tool name
    pub selected_tool: String,
    /// Tool parameters (JSON)
    pub tool_params: serde_json::Value,
    /// Expected outcome
    pub expected_outcome: String,
    /// Fallback tool name
    pub fallback_tool: Option<String>,
}

/// Input for error recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryInput {
    /// Action that failed
    pub failed_action: String,
    /// Error message
    pub error_message: String,
    /// Original intent
    pub original_intent: String,
}

/// Result from error recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryResult {
    /// Recovery strategy
    pub strategy: RecoveryStrategy,
    /// Recovery action details
    pub recovery_action: String,
    /// Reasoning
    pub reasoning: String,
    /// Confidence in recovery
    pub confidence: f32,
}

// ============================================================================
// Execution Pipeline
// ============================================================================

/// DSPy-powered execution pipeline.
pub struct ExecutionPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for ExecutionPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionPipeline {
    /// Create a new execution pipeline using the global LM.
    pub fn new() -> Self {
        Self { lm: None }
    }

    /// Create a pipeline with a specific LM.
    pub fn with_lm(lm: Arc<LM>) -> Self {
        Self { lm: Some(lm) }
    }

    /// Helper to get string from prediction value.
    fn get_string(prediction: &dspy_rs::Prediction, key: &str) -> String {
        let val = prediction.get(key, None);
        if let Some(s) = val.as_str() {
            s.to_string()
        } else {
            val.to_string().trim_matches('"').to_string()
        }
    }

    /// Helper to get f32 from prediction value.
    fn get_f32(prediction: &dspy_rs::Prediction, key: &str) -> f32 {
        let val = prediction.get(key, None);
        if let Some(n) = val.as_f64() {
            n as f32
        } else if let Some(s) = val.as_str() {
            s.parse().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// Parse JSON value from string.
    fn parse_json(s: &str) -> serde_json::Value {
        serde_json::from_str(s).unwrap_or(serde_json::Value::Null)
    }

    /// Decide next execution action for a plan step.
    pub async fn decide(&self, input: &ExecutionInput) -> anyhow::Result<ExecutionDecision> {
        let executor = Predict::new(ExecutionStrategySignature::new());

        let file_state = input
            .current_file_state
            .clone()
            .unwrap_or_else(|| "No file currently open".to_string());

        let example = example! {
            "plan_step": "input" => input.plan_step.clone(),
            "current_file_state": "input" => file_state,
            "execution_history": "input" => input.execution_history.clone(),
        };

        let prediction = if let Some(lm) = &self.lm {
            executor.forward_with_config(example, lm.clone()).await?
        } else {
            executor.forward(example).await?
        };

        let action_str = Self::get_string(&prediction, "next_action");
        let params_str = Self::get_string(&prediction, "action_params");

        Ok(ExecutionDecision {
            next_action: ExecutionAction::from(action_str.as_str()),
            action_params: Self::parse_json(&params_str),
            reasoning: Self::get_string(&prediction, "reasoning"),
            progress_estimate: Self::get_f32(&prediction, "progress_estimate"),
        })
    }

    /// Select appropriate tool for a task.
    pub async fn select_tool(&self, input: &ToolSelectionInput) -> anyhow::Result<ToolSelectionResult> {
        let selector = Predict::new(ToolSelectionSignature::new());

        let example = example! {
            "task_description": "input" => input.task_description.clone(),
            "available_tools": "input" => input.available_tools.clone(),
            "recent_context": "input" => input.recent_context.clone(),
        };

        let prediction = if let Some(lm) = &self.lm {
            selector.forward_with_config(example, lm.clone()).await?
        } else {
            selector.forward(example).await?
        };

        let fallback = Self::get_string(&prediction, "fallback_tool");
        let fallback_tool = if fallback.is_empty() || fallback == "null" || fallback == "none" {
            None
        } else {
            Some(fallback)
        };

        Ok(ToolSelectionResult {
            selected_tool: Self::get_string(&prediction, "selected_tool"),
            tool_params: Self::parse_json(&Self::get_string(&prediction, "tool_params")),
            expected_outcome: Self::get_string(&prediction, "expected_outcome"),
            fallback_tool,
        })
    }

    /// Handle execution failure and decide recovery.
    pub async fn recover(&self, input: &RecoveryInput) -> anyhow::Result<RecoveryResult> {
        let recoverer = Predict::new(ErrorRecoverySignature::new());

        let example = example! {
            "failed_action": "input" => input.failed_action.clone(),
            "error_message": "input" => input.error_message.clone(),
            "original_intent": "input" => input.original_intent.clone(),
        };

        let prediction = if let Some(lm) = &self.lm {
            recoverer.forward_with_config(example, lm.clone()).await?
        } else {
            recoverer.forward(example).await?
        };

        let strategy_str = Self::get_string(&prediction, "recovery_strategy");

        Ok(RecoveryResult {
            strategy: RecoveryStrategy::from(strategy_str.as_str()),
            recovery_action: Self::get_string(&prediction, "recovery_action"),
            reasoning: Self::get_string(&prediction, "reasoning"),
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
    fn test_execution_action_parsing() {
        assert_eq!(ExecutionAction::from("EDIT_FILE"), ExecutionAction::EditFile);
        assert_eq!(ExecutionAction::from("edit"), ExecutionAction::EditFile);
        assert_eq!(ExecutionAction::from("RUN_COMMAND"), ExecutionAction::RunCommand);
        assert_eq!(ExecutionAction::from("bash"), ExecutionAction::RunCommand);
        assert_eq!(ExecutionAction::from("READ_FILE"), ExecutionAction::ReadFile);
        assert_eq!(ExecutionAction::from("COMPLETE"), ExecutionAction::Complete);
        assert_eq!(ExecutionAction::from("done"), ExecutionAction::Complete);
        assert_eq!(ExecutionAction::from("invalid"), ExecutionAction::Unknown);
    }

    #[test]
    fn test_recovery_strategy_parsing() {
        assert_eq!(RecoveryStrategy::from("RETRY"), RecoveryStrategy::Retry);
        assert_eq!(RecoveryStrategy::from("ALTERNATIVE"), RecoveryStrategy::Alternative);
        assert_eq!(RecoveryStrategy::from("SKIP"), RecoveryStrategy::Skip);
        assert_eq!(RecoveryStrategy::from("ABORT"), RecoveryStrategy::Abort);
        assert_eq!(RecoveryStrategy::from("unknown"), RecoveryStrategy::Retry);
    }

    #[test]
    fn test_execution_input_serialization() {
        let input = ExecutionInput {
            plan_step: "Add button".to_string(),
            current_file_state: Some("// code".to_string()),
            execution_history: "[]".to_string(),
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: ExecutionInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.plan_step, input.plan_step);
    }

    #[test]
    fn test_json_parsing() {
        let valid = r#"{"file": "test.rs", "line": 10}"#;
        let parsed = ExecutionPipeline::parse_json(valid);
        assert_eq!(parsed["file"], "test.rs");

        let invalid = "not json";
        let parsed = ExecutionPipeline::parse_json(invalid);
        assert!(parsed.is_null());
    }
}
