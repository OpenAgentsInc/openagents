//! Full Auto decision signature for app-server driven automation.

use dsrs_macros::Signature;
use serde::{Deserialize, Serialize};

/// Full Auto decision signature - decide whether to continue, pause, stop, or review.
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullAutoDecisionSignature {
    /// Full Auto Decision: Decide what to do after a Codex turn completes.
    /// Return a single action and, if continuing, the next user input to send.
    #[input]
    /// Codex thread id.
    pub thread_id: String,

    #[input]
    /// Completed turn id.
    pub turn_id: String,

    #[input]
    /// Turn status (completed, interrupted, failed).
    pub last_turn_status: String,

    #[input]
    /// Turn error details, or empty string if none.
    pub turn_error: String,

    #[input]
    /// Latest plan snapshot (JSON or text).
    pub turn_plan: String,

    #[input]
    /// Latest diff summary (JSON or text).
    pub diff_summary: String,

    #[input]
    /// Latest token usage summary (JSON or text).
    pub token_usage: String,

    #[input]
    /// Pending approval summary (JSON or text).
    pub pending_approvals: String,

    #[input]
    /// Pending tool input summary (JSON or text).
    pub pending_tool_inputs: String,

    #[input]
    /// Recent Full Auto decisions (JSON or text).
    pub recent_actions: String,

    #[input]
    /// Recent compaction events (JSON or text).
    pub compaction_events: String,

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
