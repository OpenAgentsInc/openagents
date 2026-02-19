//! Guidance module decision signature.
//!
//! This signature powers the Guidance Modules pipeline that decides what to do
//! after a Full Auto turn completes.

use dsrs_macros::Signature;
use serde::{Deserialize, Serialize};

/// Guidance decision signature - decide whether to continue, pause, stop, or review.
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidanceDecisionSignature {
    /// Guidance goal intent for the run.
    #[input]
    pub goal_intent: String,

    #[input]
    /// Optional success criteria (JSON array or text).
    pub goal_success_criteria: String,

    #[input]
    /// Turn summary and context (JSON or text).
    pub summary: String,

    #[input]
    /// Current state (JSON or text).
    pub state: String,

    #[output]
    /// Action: continue | pause | stop | review.
    pub action: String,

    #[output]
    /// Next input prompt if action == continue.
    pub next_input: String,

    #[output]
    /// Brief justification for the action.
    pub reason: String,

    #[output]
    /// Confidence in the decision (0.0-1.0).
    pub confidence: f32,
}
