//! Guidance router signature for the first Guidance Module step.

use dsrs_macros::Signature;
use serde::{Deserialize, Serialize};

/// Guidance router signature - decide the first guidance response for a user message.
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidanceRouterSignature {
    /// Guidance Router: choose a route and produce a concise, task-focused response.
    /// Routes: respond | understand | plan.
    /// Use the user message, goal intent, and context to decide the route.
    /// Response must be a single sentence, no markdown, no questions, no quotes,
    /// and never mention being an AI. Keep it under 140 characters.
    #[input]
    pub user_message: String,

    #[input]
    /// Guidance goal intent (what we are trying to achieve overall).
    pub goal_intent: String,

    #[input]
    /// Optional context label (e.g. "full_auto").
    pub context: String,

    #[output]
    /// Routing decision: respond | understand | plan.
    pub route: String,

    #[output]
    /// Single-line guidance response to show in the UI.
    pub response: String,
}
