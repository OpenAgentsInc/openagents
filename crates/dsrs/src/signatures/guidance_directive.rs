//! Guidance directive signature for producing Codex directives from repo intel.

use dsrs_macros::Signature;
use serde::{Deserialize, Serialize};

/// Guidance directive signature - produce an imperative Codex directive.
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidanceDirectiveSignature {
    /// Guidance directive: produce a single, imperative directive for Codex.
    /// Use repo intel + task summary + goal intent.
    /// Output must be one sentence, no questions, no markdown, no quotes,
    /// and avoid mentioning being an AI. Keep it under 200 characters.
    #[input]
    pub goal_intent: String,

    #[input]
    /// Repo intel summary (git history, status, docs, issues).
    pub repo_intel: String,

    #[input]
    /// Task understanding + plan summary.
    pub task_summary: String,

    #[output]
    /// Imperative directive to send to Codex.
    pub directive: String,

    #[output]
    /// Short rationale (optional, may be empty).
    pub rationale: String,
}
