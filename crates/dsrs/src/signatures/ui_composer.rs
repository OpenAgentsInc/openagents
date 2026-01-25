//! Signature-driven UI composition for Effuse.

use dsrs_macros::Signature;
use serde::{Deserialize, Serialize};

/// Compose a UI tree or patch stream using the Effuse catalog.
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiComposerSignature {
    /// UI Composition: Generate a UITree or JSON patch stream using only
    /// the allowed component catalog. Output must be valid JSON.
    #[input]
    /// Catalog prompt describing allowed components and actions.
    pub catalog_prompt: String,

    #[input]
    /// Signature name for which UI is being composed.
    pub signature_name: String,

    #[input]
    /// Signature instruction describing the goal.
    pub signature_instruction: String,

    #[input]
    /// Signature input fields as JSON string.
    pub input_fields: String,

    #[input]
    /// Signature output fields as JSON string.
    pub output_fields: String,

    #[input]
    /// Layout constraints or UI hints.
    pub layout_constraints: String,

    #[input]
    /// Current UI tree as JSON (optional).
    pub current_tree: String,

    #[output]
    /// Full UI tree as JSON.
    pub ui_tree: String,

    #[output]
    /// JSON patch stream (newline-delimited) for incremental updates.
    pub ui_patches: String,
}
