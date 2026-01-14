use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Canonical plan format emitted by all planning signatures.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct PlanIR {
    /// High-level analysis of the task.
    pub analysis: String,

    /// Ordered list of steps to execute.
    pub steps: Vec<PlanStep>,

    /// How to verify completion.
    pub verification_strategy: VerificationStrategy,

    /// Overall task complexity.
    pub complexity: Complexity,

    /// Planner confidence (0.0-1.0).
    pub confidence: f32,
}

/// A single step in the plan.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct PlanStep {
    /// Unique identifier (e.g., "step-1", "step-2a").
    pub id: String,

    /// Human-readable description.
    pub description: String,

    /// What this step achieves.
    pub intent: StepIntent,

    /// Files expected to be touched.
    pub target_files: Vec<String>,

    /// Step IDs this depends on (for parallel execution).
    pub depends_on: Vec<String>,

    /// Max iterations for this step (per-step loop budget).
    pub max_iterations: u8,
}

/// Classification of what a step does.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepIntent {
    /// Read/search to understand (file_read, ripgrep, lsp).
    Investigate,
    /// Edit files (file_edit).
    Modify,
    /// Run tests/build (shell).
    Verify,
    /// Combine results from prior steps.
    Synthesize,
}

/// How to verify task completion.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct VerificationStrategy {
    /// Commands to run (e.g., ["cargo check", "cargo test"]).
    pub commands: Vec<String>,

    /// What constitutes success.
    pub success_criteria: String,

    /// Max verification retries.
    pub max_retries: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Complexity {
    Low,
    Medium,
    High,
    VeryHigh,
}
