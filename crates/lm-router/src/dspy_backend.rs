//! DSPy-aware routing helpers for lm-router.
//!
//! This module adds lightweight DSPy-specific routing primitives without
//! depending on DSPy itself. Callers provide signature metadata to guide
//! model selection.

use std::collections::HashMap;
use std::sync::Arc;

use crate::{LmResponse, LmRouter, Result};

/// Metadata about a DSPy signature used for routing decisions.
#[derive(Debug, Clone)]
pub struct DspySignatureInfo {
    /// Fully-qualified or user-provided signature name.
    pub name: String,
    /// Number of input fields in the signature.
    pub input_fields: usize,
    /// Number of output fields in the signature.
    pub output_fields: usize,
    /// Length of the instruction text in characters.
    pub instruction_len: usize,
    /// Whether the signature is expected to use chain-of-thought.
    pub has_chain_of_thought: bool,
}

impl DspySignatureInfo {
    /// Create a new signature metadata record.
    pub fn new(
        name: impl Into<String>,
        input_fields: usize,
        output_fields: usize,
        instruction_len: usize,
        has_chain_of_thought: bool,
    ) -> Self {
        Self {
            name: name.into(),
            input_fields,
            output_fields,
            instruction_len,
            has_chain_of_thought,
        }
    }

    /// Return the short signature name (last path segment).
    pub fn short_name(&self) -> &str {
        self.name.rsplit("::").next().unwrap_or(&self.name)
    }
}

/// Routing policy for DSPy signatures.
#[derive(Debug, Clone)]
pub struct DspyRoutingPolicy {
    /// Default model for all signatures.
    pub default_model: String,
    /// Optional cheaper model for simple signatures.
    pub cheap_model: Option<String>,
    /// Maximum number of input fields to be considered simple.
    pub max_input_fields: usize,
    /// Maximum number of output fields to be considered simple.
    pub max_output_fields: usize,
    /// Maximum instruction length to be considered simple.
    pub max_instruction_chars: usize,
    /// Treat chain-of-thought signatures as complex.
    pub treat_cot_as_complex: bool,
    /// Signature-specific model overrides.
    pub signature_overrides: HashMap<String, String>,
}

impl DspyRoutingPolicy {
    /// Create a new routing policy with sensible defaults.
    pub fn new(default_model: impl Into<String>) -> Self {
        Self {
            default_model: default_model.into(),
            cheap_model: None,
            max_input_fields: 4,
            max_output_fields: 4,
            max_instruction_chars: 400,
            treat_cot_as_complex: true,
            signature_overrides: HashMap::new(),
        }
    }

    /// Set the cheap model for simple signatures.
    pub fn with_cheap_model(mut self, model: impl Into<String>) -> Self {
        self.cheap_model = Some(model.into());
        self
    }

    /// Override routing for a specific signature name.
    pub fn with_signature_override(
        mut self,
        signature: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        self.signature_overrides
            .insert(signature.into(), model.into());
        self
    }

    /// Set complexity thresholds for simple signatures.
    pub fn with_simple_limits(
        mut self,
        max_input_fields: usize,
        max_output_fields: usize,
        max_instruction_chars: usize,
    ) -> Self {
        self.max_input_fields = max_input_fields;
        self.max_output_fields = max_output_fields;
        self.max_instruction_chars = max_instruction_chars;
        self
    }

    /// Configure whether chain-of-thought signatures should be treated as complex.
    pub fn with_cot_as_complex(mut self, enabled: bool) -> Self {
        self.treat_cot_as_complex = enabled;
        self
    }

    /// Choose a model for the provided signature metadata.
    pub fn model_for(&self, signature: Option<&DspySignatureInfo>) -> &str {
        if let Some(sig) = signature {
            if let Some(model) = self.signature_overrides.get(&sig.name) {
                return model;
            }
            if let Some(model) = self.signature_overrides.get(sig.short_name()) {
                return model;
            }

            if let Some(cheap) = &self.cheap_model
                && self.is_simple_signature(sig)
            {
                return cheap;
            }
        }

        &self.default_model
    }

    fn is_simple_signature(&self, signature: &DspySignatureInfo) -> bool {
        if self.treat_cot_as_complex && signature.has_chain_of_thought {
            return false;
        }

        signature.input_fields <= self.max_input_fields
            && signature.output_fields <= self.max_output_fields
            && signature.instruction_len <= self.max_instruction_chars
    }
}

/// DSPy-aware backend wrapper around [`LmRouter`].
#[derive(Clone)]
pub struct DspyBackend {
    router: Arc<LmRouter>,
    policy: DspyRoutingPolicy,
}

impl DspyBackend {
    /// Create a new DSPy backend wrapper.
    pub fn new(router: Arc<LmRouter>, policy: DspyRoutingPolicy) -> Self {
        Self { router, policy }
    }

    /// Update the routing policy.
    pub fn with_policy(mut self, policy: DspyRoutingPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Access the routing policy.
    pub fn policy(&self) -> &DspyRoutingPolicy {
        &self.policy
    }

    /// Access the underlying router.
    pub fn router(&self) -> &Arc<LmRouter> {
        &self.router
    }

    /// Complete a prompt through the router with DSPy-aware routing.
    pub async fn complete(
        &self,
        prompt: &str,
        max_tokens: usize,
        signature: Option<&DspySignatureInfo>,
    ) -> Result<LmResponse> {
        let model = self.policy.model_for(signature);
        self.router.complete(model, prompt, max_tokens).await
    }
}
