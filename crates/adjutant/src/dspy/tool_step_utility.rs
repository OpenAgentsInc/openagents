use dsrs::{Predict, Signature};

/// Tool Step Utility Evaluator.
#[Signature]
struct ToolStepUtilitySignature {
    /// Tool Step Utility Evaluator: Assess how useful the tool call was for the task.
    /// Use only the provided summaries and receipt; do not speculate.
    /// step_utility ranges from 0.0 (no value) to 1.0 (decisive progress).
    /// next_action_hint must be a short imperative (max 12 words).
    /// Always output machine-consumable fields.

    /// Name of the tool that executed (e.g., FileDiscovery, ContentReader)
    #[input]
    pub tool_name: String,

    /// Goal of this step in the overall task
    #[input]
    pub step_goal: String,

    /// Deterministic summary of the tool inputs
    #[input]
    pub inputs_summary: String,

    /// Deterministic summary of the tool outputs
    #[input]
    pub outputs_summary: String,

    /// JSON receipt for the tool call (hashes, latency, side effects)
    #[input]
    pub receipt: String,

    /// Utility score for this step (0.0 to 1.0)
    #[output]
    pub step_utility: f32,

    /// Whether the workflow should continue after this step
    #[output]
    pub should_continue: bool,

    /// Short hint for the next action
    #[output]
    pub next_action_hint: String,

    /// Confidence in the utility judgment (0.0 to 1.0)
    #[output]
    pub confidence: f32,
}

/// Build a predictor for the ToolStepUtilitySignature.
pub fn tool_step_utility_predict() -> Predict {
    Predict::new(ToolStepUtilitySignature::new())
}
