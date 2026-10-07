use std::collections::{BTreeMap, VecDeque};

use psionic_models::{
    AttnResConfig, AttnResCpuReferenceModel, AttnResDiagnosticsSnapshot, AttnResExecutionError,
    AttnResModelDescriptor, TokenId, TokenSequence,
};
use psionic_runtime::select_argmax_token;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Dedicated served product identifier for bounded AttnRes text generation.
pub const ATTNRES_TEXT_GENERATION_PRODUCT_ID: &str = "psionic.attnres_text_generation";

/// Explicit request contract for the local AttnRes text-generation lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttnResTextGenerationRequest {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier. Must be `psionic.attnres_text_generation`.
    pub product_id: String,
    /// Optional explicit AttnRes model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_model_id: Option<String>,
    /// Prompt tokens supplied to the bounded reference model.
    pub prompt_tokens: TokenSequence,
    /// Maximum number of new tokens to decode.
    pub max_new_tokens: u32,
    /// Optional stop tokens that terminate generation after the matching token is emitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop_tokens: Vec<TokenId>,
    /// Explicit environment refs carried into request truth.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_refs: Vec<String>,
}

impl AttnResTextGenerationRequest {
    /// Creates a bounded AttnRes text-generation request.
    #[must_use]
    pub fn new(
        request_id: impl Into<String>,
        prompt_tokens: TokenSequence,
        max_new_tokens: u32,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(ATTNRES_TEXT_GENERATION_PRODUCT_ID),
            requested_model_id: None,
            prompt_tokens,
            max_new_tokens,
            stop_tokens: Vec::new(),
            environment_refs: Vec::new(),
        }
    }

    /// Pins the request to one explicit AttnRes model.
    #[must_use]
    pub fn with_requested_model_id(mut self, requested_model_id: impl Into<String>) -> Self {
        self.requested_model_id = Some(requested_model_id.into());
        self
    }

    /// Installs an explicit stop-token set.
    #[must_use]
    pub fn with_stop_tokens(mut self, stop_tokens: Vec<TokenId>) -> Self {
        let mut stop_tokens = stop_tokens;
        stop_tokens.sort_unstable();
        stop_tokens.dedup();
        self.stop_tokens = stop_tokens;
        self
    }

    /// Carries environment refs into the request truth.
    #[must_use]
    pub fn with_environment_refs(mut self, environment_refs: Vec<String>) -> Self {
        let mut environment_refs = environment_refs;
        environment_refs.sort();
        environment_refs.dedup();
        self.environment_refs = environment_refs;
        self
    }

    /// Returns a stable digest over the request payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_attnres_text_generation_request|", self)
    }
}

/// Typed in-band refusal reason for the bounded AttnRes served lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttnResTextGenerationRefusalReason {
    /// The prompt carried no tokens.
    EmptyPrompt,
    /// The request asked to decode zero tokens.
    ZeroDecodeBudget,
}

/// Explicit refusal response for the bounded AttnRes served lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTextGenerationRefusal {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Served model descriptor that evaluated the request.
    pub model_descriptor: AttnResModelDescriptor,
    /// Stable refusal reason.
    pub reason: AttnResTextGenerationRefusalReason,
    /// Human-readable refusal detail.
    pub detail: String,
}

/// One generated AttnRes token step plus the shared routing diagnostics truth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTextGenerationStep {
    /// Zero-based decode index.
    pub decode_index: u32,
    /// Token count visible to the model before this decode step.
    pub context_token_count: u32,
    /// Generated token id.
    pub generated_token: u32,
    /// Selected logit value for the emitted token.
    pub selected_logit: f32,
    /// Stable digest over the last-position logits.
    pub logit_digest: String,
    /// Stable digest over the shared routing diagnostics snapshot.
    pub diagnostics_digest: String,
    /// Shared routing diagnostics snapshot.
    pub diagnostics: AttnResDiagnosticsSnapshot,
}

impl AttnResTextGenerationStep {
    /// Returns a stable digest over the step payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_attnres_text_generation_step|", self)
    }
}

/// Completed response for the bounded AttnRes served lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTextGenerationResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Served model descriptor.
    pub model_descriptor: AttnResModelDescriptor,
    /// Original prompt tokens.
    pub prompt_tokens: TokenSequence,
    /// Tokens generated by this request.
    pub generated_tokens: TokenSequence,
    /// Final full sequence after decoding.
    pub full_sequence: TokenSequence,
    /// Per-token generation steps and diagnostics.
    pub steps: Vec<AttnResTextGenerationStep>,
    /// Explicit environment refs attached to the request.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_refs: Vec<String>,
}

impl AttnResTextGenerationResponse {
    /// Returns a stable digest over the response payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_attnres_text_generation_response|", self)
    }
}

/// Served outcome for one AttnRes text-generation request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AttnResTextGenerationOutcome {
    /// The request completed normally.
    Completed {
        /// Completed response payload.
        response: AttnResTextGenerationResponse,
    },
    /// The request was refused explicitly.
    Refused {
        /// Typed refusal payload.
        refusal: AttnResTextGenerationRefusal,
    },
}

/// Terminal event emitted by the local AttnRes generation stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTextGenerationTerminalEvent {
    /// Final outcome for the request.
    pub outcome: AttnResTextGenerationOutcome,
}

/// Typed event emitted by the pull-driven AttnRes generation stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AttnResTextGenerationStreamEvent {
    /// One generated token step.
    Step {
        /// Step payload.
        step: AttnResTextGenerationStep,
    },
    /// Terminal completion or refusal.
    Terminal {
        /// Terminal payload.
        terminal: AttnResTextGenerationTerminalEvent,
    },
}

/// Typed request-validation or execution error for the AttnRes served lane.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AttnResTextGenerationServiceError {
    /// The request targeted a different product family.
    #[error("unsupported AttnRes served product `{product_id}`")]
    UnsupportedProduct {
        /// Product identifier supplied by the caller.
        product_id: String,
    },
    /// The request named an AttnRes model that is not registered.
    #[error("unknown AttnRes model `{model_id}`")]
    UnknownModel {
        /// Requested model identifier.
        model_id: String,
    },
    /// The model produced an empty logit vector.
    #[error("attnres model `{model_id}` produced an empty last-position logit vector")]
    EmptyLogits {
        /// Served model identifier.
        model_id: String,
    },
    /// The shared AttnRes runtime truth failed during execution.
    #[error(transparent)]
    Execution(#[from] AttnResExecutionError),
}

/// Pull-driven local stream for bounded AttnRes text generation.
#[derive(Clone, Debug, Default)]
pub struct LocalAttnResTextGenerationStream {
    events: VecDeque<AttnResTextGenerationStreamEvent>,
}

impl LocalAttnResTextGenerationStream {
    fn from_events(events: Vec<AttnResTextGenerationStreamEvent>) -> Self {
        Self {
            events: VecDeque::from(events),
        }
    }

    /// Returns the next typed stream event.
    pub fn next_event(&mut self) -> Option<AttnResTextGenerationStreamEvent> {
        self.events.pop_front()
    }
}

/// Local reference implementation of the bounded AttnRes served lane.
#[derive(Clone, Debug)]
pub struct LocalAttnResTextGenerationService {
    models: BTreeMap<String, AttnResCpuReferenceModel>,
    default_model_id: String,
}

impl Default for LocalAttnResTextGenerationService {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalAttnResTextGenerationService {
    /// Creates the default in-process AttnRes text-generation service.
    #[must_use]
    pub fn new() -> Self {
        let config = AttnResConfig::new(8, 4, 2)
            .with_num_heads(2)
            .with_d_ff(16)
            .with_vocab_size(8);
        let model = AttnResCpuReferenceModel::seeded("attnres-reference-text", "v0", config)
            .expect("default AttnRes serve fixture should build");
        let model_id = model.descriptor().model.model_id.clone();
        let mut models = BTreeMap::new();
        models.insert(model_id.clone(), model);
        Self {
            models,
            default_model_id: model_id,
        }
    }

    /// Registers one additional AttnRes model.
    #[must_use]
    pub fn with_model(mut self, model: AttnResCpuReferenceModel) -> Self {
        let model_id = model.descriptor().model.model_id.clone();
        self.models.insert(model_id.clone(), model);
        if self.models.len() == 1 {
            self.default_model_id = model_id;
        }
        self
    }

    /// Executes one request through the bounded AttnRes text-generation surface.
    pub fn execute(
        &self,
        request: &AttnResTextGenerationRequest,
    ) -> Result<AttnResTextGenerationOutcome, AttnResTextGenerationServiceError> {
        self.validate_product(request)?;
        let model = self.resolve_model(request)?;
        self.execute_with_model(model, request)
    }

    /// Starts a pull-driven AttnRes text-generation stream.
    pub fn execute_stream(
        &self,
        request: &AttnResTextGenerationRequest,
    ) -> Result<LocalAttnResTextGenerationStream, AttnResTextGenerationServiceError> {
        let outcome = self.execute(request)?;
        Ok(LocalAttnResTextGenerationStream::from_events(
            stream_events_for_outcome(outcome),
        ))
    }

    fn validate_product(
        &self,
        request: &AttnResTextGenerationRequest,
    ) -> Result<(), AttnResTextGenerationServiceError> {
        if request.product_id == ATTNRES_TEXT_GENERATION_PRODUCT_ID {
            Ok(())
        } else {
            Err(AttnResTextGenerationServiceError::UnsupportedProduct {
                product_id: request.product_id.clone(),
            })
        }
    }

    fn resolve_model(
        &self,
        request: &AttnResTextGenerationRequest,
    ) -> Result<&AttnResCpuReferenceModel, AttnResTextGenerationServiceError> {
        let requested_model_id = request
            .requested_model_id
            .as_deref()
            .unwrap_or(self.default_model_id.as_str());
        self.models.get(requested_model_id).ok_or_else(|| {
            AttnResTextGenerationServiceError::UnknownModel {
                model_id: requested_model_id.to_string(),
            }
        })
    }

    fn execute_with_model(
        &self,
        model: &AttnResCpuReferenceModel,
        request: &AttnResTextGenerationRequest,
    ) -> Result<AttnResTextGenerationOutcome, AttnResTextGenerationServiceError> {
        if request.prompt_tokens.is_empty() {
            return Ok(AttnResTextGenerationOutcome::Refused {
                refusal: AttnResTextGenerationRefusal {
                    request_id: request.request_id.clone(),
                    product_id: request.product_id.clone(),
                    model_descriptor: model.descriptor().clone(),
                    reason: AttnResTextGenerationRefusalReason::EmptyPrompt,
                    detail: String::from(
                        "attnres text generation requires at least one prompt token",
                    ),
                },
            });
        }
        if request.max_new_tokens == 0 {
            return Ok(AttnResTextGenerationOutcome::Refused {
                refusal: AttnResTextGenerationRefusal {
                    request_id: request.request_id.clone(),
                    product_id: request.product_id.clone(),
                    model_descriptor: model.descriptor().clone(),
                    reason: AttnResTextGenerationRefusalReason::ZeroDecodeBudget,
                    detail: String::from(
                        "attnres text generation requires a strictly positive decode budget",
                    ),
                },
            });
        }

        let mut full_sequence = request.prompt_tokens.clone();
        let mut generated_tokens = TokenSequence::default();
        let mut steps = Vec::with_capacity(request.max_new_tokens as usize);

        for decode_index in 0..request.max_new_tokens {
            let context_token_count = full_sequence.len() as u32;
            let batch = [full_sequence.clone()];
            let (_, diagnostics) = model.forward_hidden_with_diagnostics(&batch)?;
            let logits = model.forward(&batch)?;
            let last_logits = last_position_logits(&logits).ok_or_else(|| {
                AttnResTextGenerationServiceError::EmptyLogits {
                    model_id: model.descriptor().model.model_id.clone(),
                }
            })?;
            let generated_token = select_argmax_token(last_logits).ok_or_else(|| {
                AttnResTextGenerationServiceError::EmptyLogits {
                    model_id: model.descriptor().model.model_id.clone(),
                }
            })?;
            let selected_logit = last_logits[generated_token as usize];
            let generated_token = TokenId(generated_token);
            let step = AttnResTextGenerationStep {
                decode_index,
                context_token_count,
                generated_token: generated_token.as_u32(),
                selected_logit,
                logit_digest: digest_f32_slice(b"psionic_attnres_last_logits|", last_logits),
                diagnostics_digest: stable_digest(
                    b"psionic_attnres_generation_diagnostics|",
                    &diagnostics,
                ),
                diagnostics,
            };
            generated_tokens.push(generated_token);
            full_sequence.push(generated_token);
            let should_stop = request.stop_tokens.binary_search(&generated_token).is_ok();
            steps.push(step);
            if should_stop {
                break;
            }
        }

        Ok(AttnResTextGenerationOutcome::Completed {
            response: AttnResTextGenerationResponse {
                request_id: request.request_id.clone(),
                product_id: request.product_id.clone(),
                model_descriptor: model.descriptor().clone(),
                prompt_tokens: request.prompt_tokens.clone(),
                generated_tokens,
                full_sequence,
                steps,
                environment_refs: request.environment_refs.clone(),
            },
        })
    }
}

fn stream_events_for_outcome(
    outcome: AttnResTextGenerationOutcome,
) -> Vec<AttnResTextGenerationStreamEvent> {
    let mut events = Vec::new();
    if let AttnResTextGenerationOutcome::Completed { response } = &outcome {
        for step in &response.steps {
            events.push(AttnResTextGenerationStreamEvent::Step { step: step.clone() });
        }
    }
    events.push(AttnResTextGenerationStreamEvent::Terminal {
        terminal: AttnResTextGenerationTerminalEvent { outcome },
    });
    events
}

fn last_position_logits(logits: &psionic_models::AttnResTensor3) -> Option<&[f32]> {
    let width = logits.width();
    let sequence_length = logits.sequence_length();
    (width > 0 && sequence_length > 0).then(|| {
        let last_position = sequence_length - 1;
        let offset = last_position * width;
        &logits.values()[offset..offset + width]
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let bytes = serde_json::to_vec(value).expect("stable digest serialization should succeed");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn digest_f32_slice(prefix: &[u8], values: &[f32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    for value in values {
        hasher.update(value.to_bits().to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{
        ATTNRES_TEXT_GENERATION_PRODUCT_ID, AttnResTextGenerationOutcome,
        AttnResTextGenerationRefusalReason, AttnResTextGenerationRequest,
        AttnResTextGenerationServiceError, AttnResTextGenerationStreamEvent,
        LocalAttnResTextGenerationService,
    };
    use psionic_models::{TokenId, TokenSequence};

    fn request(tokens: &[u32], max_new_tokens: u32) -> AttnResTextGenerationRequest {
        AttnResTextGenerationRequest::new(
            "attnres-request",
            TokenSequence::new(tokens.iter().copied().map(TokenId).collect::<Vec<_>>()),
            max_new_tokens,
        )
    }

    #[test]
    fn service_completes_request_and_emits_shared_diagnostics() -> Result<(), Box<dyn Error>> {
        let service = LocalAttnResTextGenerationService::new();
        let outcome = service.execute(&request(&[0, 1, 2], 2))?;

        match outcome {
            AttnResTextGenerationOutcome::Completed { response } => {
                assert_eq!(response.product_id, ATTNRES_TEXT_GENERATION_PRODUCT_ID);
                assert_eq!(response.prompt_tokens.len(), 3);
                assert_eq!(response.generated_tokens.len(), 2);
                assert_eq!(response.full_sequence.len(), 5);
                assert_eq!(response.steps.len(), 2);
                assert_eq!(
                    response.steps.first().map(|step| step.context_token_count),
                    Some(3)
                );
                assert_eq!(
                    response.steps.last().map(|step| step.context_token_count),
                    Some(4)
                );
                assert_eq!(
                    response.steps[0].diagnostics.sublayers.len(),
                    response.model_descriptor.config.num_layers
                );
                assert!(!response.steps[0].diagnostics_digest.is_empty());
                assert!(!response.stable_digest().is_empty());
            }
            AttnResTextGenerationOutcome::Refused { refusal } => {
                panic!("request should not be refused: {}", refusal.detail);
            }
        }
        Ok(())
    }

    #[test]
    fn service_refuses_empty_prompt_after_model_resolution() -> Result<(), Box<dyn Error>> {
        let service = LocalAttnResTextGenerationService::new();
        let outcome = service.execute(&request(&[], 1))?;

        match outcome {
            AttnResTextGenerationOutcome::Completed { .. } => {
                panic!("empty prompt should not complete");
            }
            AttnResTextGenerationOutcome::Refused { refusal } => {
                assert_eq!(
                    refusal.reason,
                    AttnResTextGenerationRefusalReason::EmptyPrompt
                );
            }
        }
        Ok(())
    }

    #[test]
    fn stream_surfaces_step_and_terminal_events() -> Result<(), Box<dyn Error>> {
        let service = LocalAttnResTextGenerationService::new();
        let mut stream = service.execute_stream(&request(&[0, 1, 2], 1))?;

        let Some(AttnResTextGenerationStreamEvent::Step { step }) = stream.next_event() else {
            panic!("first event should be a generation step");
        };
        assert_eq!(step.context_token_count, 3);
        let Some(AttnResTextGenerationStreamEvent::Terminal { .. }) = stream.next_event() else {
            panic!("second event should be terminal");
        };
        assert!(stream.next_event().is_none());
        Ok(())
    }

    #[test]
    fn stop_tokens_terminate_after_matching_generation() -> Result<(), Box<dyn Error>> {
        let service = LocalAttnResTextGenerationService::new();
        let initial = service.execute(&request(&[0, 1, 2], 1))?;
        let stop_token = match initial {
            AttnResTextGenerationOutcome::Completed { response } => {
                response.steps[0].generated_token
            }
            AttnResTextGenerationOutcome::Refused { refusal } => {
                panic!("request should not be refused: {}", refusal.detail);
            }
        };

        let request = request(&[0, 1, 2], 4).with_stop_tokens(vec![TokenId(stop_token)]);
        let outcome = service.execute(&request)?;
        match outcome {
            AttnResTextGenerationOutcome::Completed { response } => {
                assert_eq!(response.steps.len(), 1);
                assert_eq!(response.generated_tokens.len(), 1);
            }
            AttnResTextGenerationOutcome::Refused { refusal } => {
                panic!("request should not be refused: {}", refusal.detail);
            }
        }
        Ok(())
    }

    #[test]
    fn service_rejects_wrong_product() {
        let service = LocalAttnResTextGenerationService::new();
        let mut request = request(&[0, 1, 2], 1);
        request.product_id = String::from("psionic.text_generation");

        let error = service
            .execute(&request)
            .expect_err("wrong product should fail before execution");
        assert_eq!(
            error,
            AttnResTextGenerationServiceError::UnsupportedProduct {
                product_id: String::from("psionic.text_generation"),
            }
        );
    }
}
