//! DSPy-aware LM provider backed by lm-router.

use std::sync::Arc;

use lm_router::{DspyBackend, DspyRoutingPolicy, DspySignatureInfo, LmRouter};
use rig::OneOrMany;
use rig::completion::{CompletionError, CompletionRequest, CompletionResponse, Usage};
use rig::message::{AssistantContent, Text};

use crate::core::lm::usage::LmUsage;
use crate::core::signature::MetaSignature;

use super::CompletionProvider;

/// Result from a DSPy-aware lm-router completion.
pub struct LmRouterCompletion {
    pub response: CompletionResponse<()>,
    pub usage: LmUsage,
}

/// DSPy-aware LM wrapper around `lm-router`.
#[derive(Clone)]
pub struct LmRouterLM {
    backend: DspyBackend,
    max_tokens: usize,
    temperature: f32,
}

impl LmRouterLM {
    /// Create a new router-backed LM with a default model.
    pub fn new(router: Arc<LmRouter>, default_model: impl Into<String>) -> Self {
        let policy = DspyRoutingPolicy::new(default_model);
        Self {
            backend: DspyBackend::new(router, policy),
            max_tokens: 512,
            temperature: 0.7,
        }
    }

    /// Override the cheap model used for simple signatures.
    pub fn with_cheap_model(mut self, model: impl Into<String>) -> Self {
        self = self.update_policy(|policy| policy.with_cheap_model(model));
        self
    }

    /// Override routing for a specific signature name.
    pub fn with_signature_override(
        mut self,
        signature: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        self = self.update_policy(|policy| policy.with_signature_override(signature, model));
        self
    }

    /// Configure thresholds for simple signatures.
    pub fn with_simple_limits(
        mut self,
        max_input_fields: usize,
        max_output_fields: usize,
        max_instruction_chars: usize,
    ) -> Self {
        self = self.update_policy(|policy| {
            policy.with_simple_limits(max_input_fields, max_output_fields, max_instruction_chars)
        });
        self
    }

    /// Treat chain-of-thought signatures as complex or not.
    pub fn with_cot_as_complex(mut self, enabled: bool) -> Self {
        self = self.update_policy(|policy| policy.with_cot_as_complex(enabled));
        self
    }

    /// Set the default max tokens for requests.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set the default temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Access the routing policy.
    pub fn policy(&self) -> &DspyRoutingPolicy {
        self.backend.policy()
    }

    /// Access the underlying router.
    pub fn router(&self) -> &Arc<LmRouter> {
        self.backend.router()
    }

    /// Execute a completion with optional signature metadata.
    pub async fn completion_with_signature(
        &self,
        signature: Option<&dyn MetaSignature>,
        request: CompletionRequest,
    ) -> Result<LmRouterCompletion, CompletionError> {
        let prompt = build_prompt_from_request(&request);
        let max_tokens = request
            .max_tokens
            .map(|t| t as usize)
            .unwrap_or(self.max_tokens);
        let _temperature = request.temperature.unwrap_or(self.temperature as f64);

        let signature_info = signature.map(signature_to_info);
        let response = self
            .backend
            .complete(&prompt, max_tokens, signature_info.as_ref())
            .await
            .map_err(|e| CompletionError::ProviderError(e.to_string()))?;

        let usage = router_usage_to_dsrs(&response);
        let completion = CompletionResponse {
            choice: OneOrMany::one(AssistantContent::Text(Text {
                text: response.text,
            })),
            usage: Usage {
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
            },
            raw_response: (),
        };

        Ok(LmRouterCompletion {
            response: completion,
            usage,
        })
    }

    fn update_policy<F>(self, updater: F) -> Self
    where
        F: FnOnce(DspyRoutingPolicy) -> DspyRoutingPolicy,
    {
        let policy = updater(self.backend.policy().clone());
        Self {
            backend: self.backend.clone().with_policy(policy),
            ..self
        }
    }
}

impl CompletionProvider for LmRouterLM {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        Ok(self
            .completion_with_signature(None, request)
            .await?
            .response)
    }
}

fn build_prompt_from_request(request: &CompletionRequest) -> String {
    let mut parts = Vec::new();

    if let Some(preamble) = &request.preamble {
        parts.push(format!("System: {}", preamble));
    }

    for msg in request.chat_history.iter() {
        match msg {
            rig::message::Message::User { content } => {
                for c in content.iter() {
                    if let rig::message::UserContent::Text(text) = c {
                        parts.push(format!("User: {}", text.text));
                    }
                }
            }
            rig::message::Message::Assistant { content, .. } => {
                for c in content.iter() {
                    if let rig::message::AssistantContent::Text(text) = c {
                        parts.push(format!("Assistant: {}", text.text));
                    }
                }
            }
        }
    }

    parts.join("\n\n")
}

fn signature_to_info(signature: &dyn MetaSignature) -> DspySignatureInfo {
    let instruction = signature.instruction();
    let instruction_lower = instruction.to_lowercase();
    let has_cot = instruction_lower.contains("chain-of-thought")
        || instruction_lower.contains("step-by-step")
        || instruction_lower.contains("think step");

    let input_fields = signature
        .input_fields()
        .as_object()
        .map(|o| o.len())
        .unwrap_or(0);
    let output_fields = signature
        .output_fields()
        .as_object()
        .map(|o| o.len())
        .unwrap_or(0);

    DspySignatureInfo::new(
        signature.signature_name(),
        input_fields,
        output_fields,
        instruction.len(),
        has_cot,
    )
}

fn router_usage_to_dsrs(response: &lm_router::LmResponse) -> LmUsage {
    let cost_msats = response.usage.cost_sats.unwrap_or(0).saturating_mul(1000);
    LmUsage {
        prompt_tokens: response.usage.prompt_tokens as u64,
        completion_tokens: response.usage.completion_tokens as u64,
        total_tokens: response.usage.total_tokens as u64,
        cost_msats,
        provider_usage: None,
    }
}
