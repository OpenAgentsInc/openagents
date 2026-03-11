use crate::structured::{
    AppleFmGeneratedContent, AppleFmGenerationSchema, AppleFmStructuredType,
    AppleFmStructuredValueError,
};
use crate::transcript::{AppleFmTranscript, AppleFmTranscriptError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Default model identifier exposed by the retained Swift bridge.
pub const DEFAULT_APPLE_FM_MODEL_ID: &str = "apple-foundation-model";

/// Health endpoint path exposed by the retained Swift bridge.
pub const APPLE_FM_BRIDGE_HEALTH_PATH: &str = "/health";

/// Model-list endpoint path exposed by the retained Swift bridge.
pub const APPLE_FM_BRIDGE_MODELS_PATH: &str = "/v1/models";

/// Session-management endpoint path exposed by the retained Swift bridge.
pub const APPLE_FM_BRIDGE_SESSIONS_PATH: &str = "/v1/sessions";

/// Chat-completions endpoint path exposed by the retained Swift bridge.
pub const APPLE_FM_BRIDGE_CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";

/// Session-response streaming suffix exposed by the retained Swift bridge.
pub const APPLE_FM_BRIDGE_STREAM_SUFFIX: &str = "/stream";

/// Session-transcript export suffix exposed by the retained Swift bridge.
pub const APPLE_FM_BRIDGE_TRANSCRIPT_SUFFIX: &str = "/transcript";

/// Session-structured-response suffix exposed by the retained Swift bridge.
pub const APPLE_FM_BRIDGE_STRUCTURED_SUFFIX: &str = "/structured";

/// Typed system-model use cases exposed by Apple's Foundation Models surface.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleFmSystemLanguageModelUseCase {
    /// General-purpose text generation.
    #[default]
    General,
    /// Content tagging / classification.
    ContentTagging,
    /// Future or unknown bridge value.
    #[serde(other)]
    Unknown,
}

impl AppleFmSystemLanguageModelUseCase {
    /// Stable label used in logs and UI.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::General => "general",
            Self::ContentTagging => "content_tagging",
            Self::Unknown => "unknown",
        }
    }
}

/// Typed system-model guardrail modes exposed by Apple's Foundation Models surface.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleFmSystemLanguageModelGuardrails {
    /// Standard Apple safety defaults.
    #[default]
    Default,
    /// More permissive content-transformation mode.
    PermissiveContentTransformations,
    /// Future or unknown bridge value.
    #[serde(other)]
    Unknown,
}

impl AppleFmSystemLanguageModelGuardrails {
    /// Stable label used in logs and UI.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::PermissiveContentTransformations => "permissive_content_transformations",
            Self::Unknown => "unknown",
        }
    }
}

/// Typed reasons Apple's system model may be unavailable.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleFmSystemLanguageModelUnavailableReason {
    /// Apple Intelligence has not been enabled for the current system/user.
    AppleIntelligenceNotEnabled,
    /// The device does not satisfy Foundation Models requirements.
    DeviceNotEligible,
    /// The model has not finished downloading / preparing.
    ModelNotReady,
    /// Future or unknown bridge value.
    #[default]
    #[serde(other)]
    Unknown,
}

impl AppleFmSystemLanguageModelUnavailableReason {
    /// Stable label used in logs and UI.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::AppleIntelligenceNotEnabled => "apple_intelligence_not_enabled",
            Self::DeviceNotEligible => "device_not_eligible",
            Self::ModelNotReady => "model_not_ready",
            Self::Unknown => "unknown",
        }
    }
}

/// Rust-side equivalent of the Python SDK's `SystemLanguageModel`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmSystemLanguageModel {
    /// Stable Apple FM model identifier.
    pub id: String,
    /// Use-case specialization.
    #[serde(default)]
    pub use_case: AppleFmSystemLanguageModelUseCase,
    /// Guardrail mode.
    #[serde(default)]
    pub guardrails: AppleFmSystemLanguageModelGuardrails,
}

impl Default for AppleFmSystemLanguageModel {
    fn default() -> Self {
        Self {
            id: DEFAULT_APPLE_FM_MODEL_ID.to_string(),
            use_case: AppleFmSystemLanguageModelUseCase::General,
            guardrails: AppleFmSystemLanguageModelGuardrails::Default,
        }
    }
}

/// Typed availability/configuration truth for the current system model.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmSystemLanguageModelAvailability {
    /// System model configuration.
    pub model: AppleFmSystemLanguageModel,
    /// Whether the configured system model is available now.
    pub available: bool,
    /// Optional typed reason for unavailability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<AppleFmSystemLanguageModelUnavailableReason>,
    /// Human-readable availability detail from the bridge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability_message: Option<String>,
    /// Supported use cases the bridge currently exposes.
    #[serde(default)]
    pub supported_use_cases: Vec<AppleFmSystemLanguageModelUseCase>,
    /// Supported guardrail modes the bridge currently exposes.
    #[serde(default)]
    pub supported_guardrails: Vec<AppleFmSystemLanguageModelGuardrails>,
    /// Optional bridge version string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Optional platform string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// Whether Apple Silicon is required for this lane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple_silicon_required: Option<bool>,
    /// Whether Apple Intelligence is required for this lane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple_intelligence_required: Option<bool>,
}

impl AppleFmSystemLanguageModelAvailability {
    /// Returns whether the configured model is ready for requests.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.available
    }
}

/// Sampling mode families exposed by the Apple FM SDK.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleFmSamplingModeType {
    /// Deterministic highest-probability token selection.
    #[default]
    Greedy,
    /// Randomized sampling from constrained probability mass.
    Random,
}

/// Rust-side equivalent of the Python SDK's `SamplingMode`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleFmSamplingMode {
    /// Sampling mode discriminator.
    #[serde(rename = "mode")]
    pub mode_type: AppleFmSamplingModeType,
    /// Optional top-k candidate count for random sampling.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "top_k")]
    pub top: Option<u32>,
    /// Optional cumulative probability threshold for random sampling.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "top_p")]
    pub probability_threshold: Option<f64>,
    /// Optional random seed for reproducible random sampling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

impl AppleFmSamplingMode {
    /// Builds greedy deterministic sampling.
    #[must_use]
    pub const fn greedy() -> Self {
        Self {
            mode_type: AppleFmSamplingModeType::Greedy,
            top: None,
            probability_threshold: None,
            seed: None,
        }
    }

    /// Builds random sampling with optional top-k or probability-threshold constraints.
    pub fn random(
        top: Option<u32>,
        probability_threshold: Option<f64>,
        seed: Option<u64>,
    ) -> Result<Self, AppleFmGenerationOptionsValidationError> {
        let sampling = Self {
            mode_type: AppleFmSamplingModeType::Random,
            top,
            probability_threshold,
            seed,
        };
        sampling.validate()?;
        Ok(sampling)
    }

    /// Validates sampling-mode semantics against the Python SDK contract.
    pub fn validate(&self) -> Result<(), AppleFmGenerationOptionsValidationError> {
        match self.mode_type {
            AppleFmSamplingModeType::Greedy => {
                if self.top.is_some() {
                    return Err(AppleFmGenerationOptionsValidationError::GreedyTopNotAllowed);
                }
                if self.probability_threshold.is_some() {
                    return Err(
                        AppleFmGenerationOptionsValidationError::GreedyProbabilityThresholdNotAllowed,
                    );
                }
                if self.seed.is_some() {
                    return Err(AppleFmGenerationOptionsValidationError::GreedySeedNotAllowed);
                }
            }
            AppleFmSamplingModeType::Random => {
                if self.top == Some(0) {
                    return Err(AppleFmGenerationOptionsValidationError::TopMustBePositive);
                }
                if self.top.is_some() && self.probability_threshold.is_some() {
                    return Err(
                        AppleFmGenerationOptionsValidationError::TopAndProbabilityThresholdConflict,
                    );
                }
                if let Some(probability_threshold) = self.probability_threshold
                    && !(0.0..=1.0).contains(&probability_threshold)
                {
                    return Err(
                        AppleFmGenerationOptionsValidationError::ProbabilityThresholdOutOfRange,
                    );
                }
            }
        }
        Ok(())
    }
}

/// Rust-side equivalent of the Python SDK's `GenerationOptions`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleFmGenerationOptions {
    /// Optional sampling strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling: Option<AppleFmSamplingMode>,
    /// Optional temperature override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Optional maximum response token limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_response_tokens: Option<u32>,
}

impl AppleFmGenerationOptions {
    /// Builds a validated generation-options value.
    pub fn new(
        sampling: Option<AppleFmSamplingMode>,
        temperature: Option<f64>,
        maximum_response_tokens: Option<u32>,
    ) -> Result<Self, AppleFmGenerationOptionsValidationError> {
        let options = Self {
            sampling,
            temperature,
            maximum_response_tokens,
        };
        options.validate()?;
        Ok(options)
    }

    /// Validates option semantics against the Python SDK contract.
    pub fn validate(&self) -> Result<(), AppleFmGenerationOptionsValidationError> {
        if let Some(temperature) = self.temperature
            && temperature < 0.0
        {
            return Err(AppleFmGenerationOptionsValidationError::NegativeTemperature);
        }
        if self.maximum_response_tokens == Some(0) {
            return Err(
                AppleFmGenerationOptionsValidationError::MaximumResponseTokensMustBePositive,
            );
        }
        if let Some(sampling) = self.sampling.as_ref() {
            sampling.validate()?;
        }
        Ok(())
    }
}

/// Validation failures for Apple FM generation options.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AppleFmGenerationOptionsValidationError {
    /// Temperature must be non-negative.
    #[error("'temperature' must be non-negative")]
    NegativeTemperature,
    /// Maximum response tokens must be positive.
    #[error("'maximum_response_tokens' must be positive")]
    MaximumResponseTokensMustBePositive,
    /// `top` and `probability_threshold` are mutually exclusive.
    #[error(
        "Cannot specify both 'top' and 'probability_threshold'. Choose one sampling constraint."
    )]
    TopAndProbabilityThresholdConflict,
    /// Top-k must be positive.
    #[error("'top' must be a positive integer")]
    TopMustBePositive,
    /// Probability threshold must stay inside 0.0...1.0.
    #[error("'probability_threshold' must be between 0.0 and 1.0")]
    ProbabilityThresholdOutOfRange,
    /// Greedy sampling does not accept top-k.
    #[error("greedy sampling does not accept 'top'")]
    GreedyTopNotAllowed,
    /// Greedy sampling does not accept probability-threshold sampling.
    #[error("greedy sampling does not accept 'probability_threshold'")]
    GreedyProbabilityThresholdNotAllowed,
    /// Greedy sampling does not accept a random seed.
    #[error("greedy sampling does not accept 'seed'")]
    GreedySeedNotAllowed,
}

/// Request/response chat message role used by the current bridge.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppleFmChatMessageRole {
    /// System instructions.
    System,
    /// User-authored prompt content.
    User,
    /// Assistant response content.
    Assistant,
    /// Tool-provided content.
    Tool,
}

/// Chat message carried by the current bridge contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmChatMessage {
    /// Message role.
    pub role: AppleFmChatMessageRole,
    /// Message content.
    pub content: String,
}

/// One-shot chat-completion request supported by the current bridge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleFmChatCompletionRequest {
    /// Optional requested model identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Ordered chat messages for the completion.
    pub messages: Vec<AppleFmChatMessage>,
    /// Optional temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Optional maximum response tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Typed generation options carried through the bridge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<AppleFmGenerationOptions>,
    /// Whether streaming is requested.
    pub stream: bool,
}

impl AppleFmChatCompletionRequest {
    /// Builds a one-shot user-prompt request against the current bridge shape.
    #[must_use]
    pub fn from_user_prompt(
        prompt: impl Into<String>,
        model: Option<impl Into<String>>,
        max_tokens: Option<u32>,
        temperature: Option<f64>,
    ) -> Self {
        let options = match (temperature, max_tokens) {
            (None, None) => None,
            _ => Some(AppleFmGenerationOptions {
                sampling: None,
                temperature,
                maximum_response_tokens: max_tokens,
            }),
        };
        Self::from_user_prompt_with_options(prompt, model, options)
    }

    /// Builds a one-shot user-prompt request with typed generation options.
    #[must_use]
    pub fn from_user_prompt_with_options(
        prompt: impl Into<String>,
        model: Option<impl Into<String>>,
        options: Option<AppleFmGenerationOptions>,
    ) -> Self {
        let (temperature, max_tokens) = match options.as_ref() {
            Some(options) => (options.temperature, options.maximum_response_tokens),
            None => (None, None),
        };
        Self {
            model: model.map(Into::into),
            messages: vec![AppleFmChatMessage {
                role: AppleFmChatMessageRole::User,
                content: prompt.into(),
            }],
            temperature,
            max_tokens,
            options,
            stream: false,
        }
    }

    /// Validates chat-completion generation options before transport.
    pub fn validate(&self) -> Result<(), AppleFmGenerationOptionsValidationError> {
        let options = self.normalized_generation_options();
        if let Some(options) = options.as_ref() {
            options.validate()?;
        }
        Ok(())
    }

    /// Returns the normalized generation-options view across legacy and typed fields.
    #[must_use]
    pub fn normalized_generation_options(&self) -> Option<AppleFmGenerationOptions> {
        match self.options.clone() {
            Some(mut options) => {
                if options.temperature.is_none() {
                    options.temperature = self.temperature;
                }
                if options.maximum_response_tokens.is_none() {
                    options.maximum_response_tokens = self.max_tokens;
                }
                Some(options)
            }
            None => match (self.temperature, self.max_tokens) {
                (None, None) => None,
                _ => Some(AppleFmGenerationOptions {
                    sampling: None,
                    temperature: self.temperature,
                    maximum_response_tokens: self.max_tokens,
                }),
            },
        }
    }
}

/// Model-health payload returned by the current bridge.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmHealthResponse {
    /// Overall bridge health status.
    pub status: String,
    /// Whether the Apple system model is available for use.
    pub model_available: bool,
    /// Optional bridge version string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Optional platform string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// Human-readable availability detail from the bridge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability_message: Option<String>,
    /// Typed reason for unavailability if the model is not available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<AppleFmSystemLanguageModelUnavailableReason>,
    /// Default use-case configuration surfaced by the bridge.
    #[serde(default)]
    pub default_use_case: AppleFmSystemLanguageModelUseCase,
    /// Default guardrail configuration surfaced by the bridge.
    #[serde(default)]
    pub default_guardrails: AppleFmSystemLanguageModelGuardrails,
    /// Supported use cases the bridge currently exposes.
    #[serde(default)]
    pub supported_use_cases: Vec<AppleFmSystemLanguageModelUseCase>,
    /// Supported guardrail modes the bridge currently exposes.
    #[serde(default)]
    pub supported_guardrails: Vec<AppleFmSystemLanguageModelGuardrails>,
    /// Whether Apple Silicon is required for this lane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple_silicon_required: Option<bool>,
    /// Whether Apple Intelligence is required for this lane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple_intelligence_required: Option<bool>,
}

impl AppleFmHealthResponse {
    /// Reconstructs typed system-model availability truth from the health payload.
    #[must_use]
    pub fn system_model_availability(&self) -> AppleFmSystemLanguageModelAvailability {
        AppleFmSystemLanguageModelAvailability {
            model: AppleFmSystemLanguageModel {
                id: DEFAULT_APPLE_FM_MODEL_ID.to_string(),
                use_case: self.default_use_case,
                guardrails: self.default_guardrails,
            },
            available: self.model_available,
            unavailable_reason: self.unavailable_reason,
            availability_message: self.availability_message.clone(),
            supported_use_cases: self.supported_use_cases.clone(),
            supported_guardrails: self.supported_guardrails.clone(),
            version: self.version.clone(),
            platform: self.platform.clone(),
            apple_silicon_required: self.apple_silicon_required,
            apple_intelligence_required: self.apple_intelligence_required,
        }
    }
}

/// Model-list response returned by the current bridge.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmModelsResponse {
    /// Envelope kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    /// Available models.
    #[serde(default)]
    pub data: Vec<AppleFmModelInfo>,
}

impl AppleFmModelsResponse {
    /// Returns the ordered model identifiers exposed by the bridge.
    #[must_use]
    pub fn model_ids(&self) -> Vec<String> {
        self.data.iter().map(|model| model.id.clone()).collect()
    }
}

/// Individual model descriptor returned by the current bridge.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmModelInfo {
    /// Stable model identifier.
    pub id: String,
    /// Optional envelope kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    /// Optional created timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<i64>,
    /// Optional owner string.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "owned_by")]
    pub owned_by: Option<String>,
    /// Default use-case configuration for this system model.
    #[serde(default)]
    pub default_use_case: AppleFmSystemLanguageModelUseCase,
    /// Default guardrail configuration for this system model.
    #[serde(default)]
    pub default_guardrails: AppleFmSystemLanguageModelGuardrails,
    /// Supported use cases for this system model.
    #[serde(default)]
    pub supported_use_cases: Vec<AppleFmSystemLanguageModelUseCase>,
    /// Supported guardrail modes for this system model.
    #[serde(default)]
    pub supported_guardrails: Vec<AppleFmSystemLanguageModelGuardrails>,
    /// Current availability if the bridge includes it in model listing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available: Option<bool>,
    /// Current typed unavailability reason if the model is not available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<AppleFmSystemLanguageModelUnavailableReason>,
    /// Human-readable availability detail if the bridge includes it in model listing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability_message: Option<String>,
}

impl AppleFmModelInfo {
    /// Returns the typed model configuration described by this listing entry.
    #[must_use]
    pub fn system_model(&self) -> AppleFmSystemLanguageModel {
        AppleFmSystemLanguageModel {
            id: self.id.clone(),
            use_case: self.default_use_case,
            guardrails: self.default_guardrails,
        }
    }
}

/// Chat-completion response returned by the current bridge.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmChatCompletionResponse {
    /// Completion identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Envelope kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    /// Creation timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<i64>,
    /// Served model identifier.
    pub model: String,
    /// Completion choices.
    #[serde(default)]
    pub choices: Vec<AppleFmChatChoice>,
    /// Optional usage details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<AppleFmChatUsage>,
}

impl AppleFmChatCompletionResponse {
    /// Returns the first assistant text payload if one exists.
    #[must_use]
    pub fn first_text_content(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|choice| choice.message.content.as_deref())
    }

    /// Converts the response into a simplified completion result.
    #[must_use]
    pub fn completion_result(&self) -> AppleFmCompletionResult {
        AppleFmCompletionResult {
            model: self.model.clone(),
            output: self.first_text_content().unwrap_or_default().to_string(),
            prompt_tokens: self.usage.as_ref().and_then(|usage| usage.prompt_tokens),
            completion_tokens: self
                .usage
                .as_ref()
                .and_then(|usage| usage.completion_tokens),
            total_tokens: self.usage.as_ref().and_then(|usage| usage.total_tokens),
            usage: self.usage.clone(),
        }
    }
}

/// One completion choice returned by the current bridge.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmChatChoice {
    /// Choice index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    /// Assistant message payload.
    pub message: AppleFmChatResponseMessage,
    /// Terminal finish reason if provided by the bridge.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "finish_reason"
    )]
    pub finish_reason: Option<String>,
}

/// Chat message returned in a completion choice.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmChatResponseMessage {
    /// Message role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<AppleFmChatMessageRole>,
    /// Optional assistant content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Usage data returned by the current bridge.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmChatUsage {
    /// Prompt token count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u64>,
    /// Completion token count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u64>,
    /// Total token count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
    /// Truthful prompt-token count with exact-vs-estimated status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_detail: Option<AppleFmUsageMeasurement>,
    /// Truthful completion-token count with exact-vs-estimated status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_tokens_detail: Option<AppleFmUsageMeasurement>,
    /// Truthful total-token count with exact-vs-estimated status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens_detail: Option<AppleFmUsageMeasurement>,
}

impl AppleFmChatUsage {
    /// Returns the best available prompt-token count, exact or estimated.
    #[must_use]
    pub fn prompt_tokens_best_effort(&self) -> Option<u64> {
        self.prompt_tokens.or_else(|| {
            self.prompt_tokens_detail
                .as_ref()
                .map(|detail| detail.value)
        })
    }

    /// Returns the best available completion-token count, exact or estimated.
    #[must_use]
    pub fn completion_tokens_best_effort(&self) -> Option<u64> {
        self.completion_tokens.or_else(|| {
            self.completion_tokens_detail
                .as_ref()
                .map(|detail| detail.value)
        })
    }

    /// Returns the best available total-token count, exact or estimated.
    #[must_use]
    pub fn total_tokens_best_effort(&self) -> Option<u64> {
        self.total_tokens
            .or_else(|| self.total_tokens_detail.as_ref().map(|detail| detail.value))
    }
}

/// Whether a usage value is exact or estimated.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleFmUsageTruth {
    /// Value is exact and authoritative.
    Exact,
    /// Value is an estimate only.
    #[default]
    Estimated,
}

/// A truthful usage measurement with exact-versus-estimated metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmUsageMeasurement {
    /// Numeric measurement value.
    pub value: u64,
    /// Truthfulness of the measurement.
    #[serde(default)]
    pub truth: AppleFmUsageTruth,
}

/// Structured error payload returned by the current bridge.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmErrorResponse {
    /// Error details.
    pub error: AppleFmErrorDetail,
}

/// Typed Apple FM error families aligned to the documented Python SDK surface.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleFmErrorCode {
    /// Prompt/history exceeded the available context window.
    ExceededContextWindowSize,
    /// Required on-device assets are unavailable.
    AssetsUnavailable,
    /// Guardrails blocked generation or continuation.
    GuardrailViolation,
    /// The requested guide or schema constraint is unsupported.
    UnsupportedGuide,
    /// The requested language or locale is unsupported.
    UnsupportedLanguageOrLocale,
    /// Output decoding failed.
    DecodingFailure,
    /// Runtime rate limiting blocked the request.
    RateLimited,
    /// Overlapping requests hit session concurrency limits.
    ConcurrentRequests,
    /// The model explicitly refused the request.
    Refusal,
    /// The provided generation schema was invalid.
    InvalidGenerationSchema,
    /// A registered tool call failed.
    ToolCallFailed,
    /// The request payload itself was invalid.
    InvalidRequest,
    /// The bridge hit an internal server failure.
    ServerError,
    /// Future or unknown bridge value.
    #[default]
    Unknown,
}

impl AppleFmErrorCode {
    /// Stable label used in logs, receipts, and wire payloads.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::ExceededContextWindowSize => "exceeded_context_window_size",
            Self::AssetsUnavailable => "assets_unavailable",
            Self::GuardrailViolation => "guardrail_violation",
            Self::UnsupportedGuide => "unsupported_guide",
            Self::UnsupportedLanguageOrLocale => "unsupported_language_or_locale",
            Self::DecodingFailure => "decoding_failure",
            Self::RateLimited => "rate_limited",
            Self::ConcurrentRequests => "concurrent_requests",
            Self::Refusal => "refusal",
            Self::InvalidGenerationSchema => "invalid_generation_schema",
            Self::ToolCallFailed => "tool_call_failed",
            Self::InvalidRequest => "invalid_request",
            Self::ServerError => "server_error",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    fn parse(label: &str) -> Self {
        match label {
            "exceeded_context_window_size" | "exceededContextWindowSize" => {
                Self::ExceededContextWindowSize
            }
            "assets_unavailable" | "assetsUnavailable" | "model_unavailable" => {
                Self::AssetsUnavailable
            }
            "guardrail_violation" | "guardrailViolation" => Self::GuardrailViolation,
            "unsupported_guide" | "unsupportedGuide" => Self::UnsupportedGuide,
            "unsupported_language_or_locale" | "unsupportedLanguageOrLocale" => {
                Self::UnsupportedLanguageOrLocale
            }
            "decoding_failure" | "decodingFailure" => Self::DecodingFailure,
            "rate_limited" | "rateLimited" => Self::RateLimited,
            "concurrent_requests" | "concurrentRequests" => Self::ConcurrentRequests,
            "refusal" => Self::Refusal,
            "invalid_generation_schema" | "invalid_schema" => Self::InvalidGenerationSchema,
            "tool_call_failed" => Self::ToolCallFailed,
            "invalid_request" | "invalid_request_error" => Self::InvalidRequest,
            "server_error" | "request_failed" | "error" => Self::ServerError,
            _ => Self::Unknown,
        }
    }
}

/// Error details returned by the current bridge.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmErrorDetail {
    /// Human-readable error message.
    pub message: String,
    /// Error type string.
    pub r#type: String,
    /// Optional machine-readable code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Optional failed tool name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// Optional underlying tool error detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underlying_error: Option<String>,
    /// Optional platform failure reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    /// Optional platform recovery suggestion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_suggestion: Option<String>,
    /// Optional lower-level debug detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug_description: Option<String>,
    /// Optional refusal explanation surfaced by Foundation Models.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_explanation: Option<String>,
}

impl AppleFmErrorDetail {
    /// Returns the typed error family reconstructed from the bridge payload.
    #[must_use]
    pub fn kind(&self) -> AppleFmErrorCode {
        self.code
            .as_deref()
            .map(AppleFmErrorCode::parse)
            .filter(|kind| *kind != AppleFmErrorCode::Unknown)
            .unwrap_or_else(|| AppleFmErrorCode::parse(self.r#type.as_str()))
    }
}

/// Simplified one-shot completion result derived from the bridge response.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppleFmCompletionResult {
    /// Served model identifier.
    pub model: String,
    /// First assistant text payload.
    pub output: String,
    /// Prompt token count if the bridge reported one.
    pub prompt_tokens: Option<u64>,
    /// Completion token count if the bridge reported one.
    pub completion_tokens: Option<u64>,
    /// Total token count if the bridge reported one.
    pub total_tokens: Option<u64>,
    /// Full usage payload including estimated-vs-exact truth.
    pub usage: Option<AppleFmChatUsage>,
}

/// First-class plain-text generation request for the reusable Rust lane.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleFmTextGenerationRequest {
    /// Optional requested model identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Plain user prompt.
    pub prompt: String,
    /// Typed generation options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<AppleFmGenerationOptions>,
}

impl AppleFmTextGenerationRequest {
    /// Validates generation options before transport.
    pub fn validate(&self) -> Result<(), AppleFmGenerationOptionsValidationError> {
        if let Some(options) = self.options.as_ref() {
            options.validate()?;
        }
        Ok(())
    }

    /// Converts this request into the compatibility chat-completion shape.
    #[must_use]
    pub fn into_chat_completion_request(self) -> AppleFmChatCompletionRequest {
        AppleFmChatCompletionRequest::from_user_prompt_with_options(
            self.prompt,
            self.model,
            self.options,
        )
    }
}

/// First-class plain-text generation response for the reusable Rust lane.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmTextGenerationResponse {
    /// Served model identifier.
    pub model: String,
    /// First assistant text payload.
    pub output: String,
    /// Optional usage details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<AppleFmChatUsage>,
}

impl AppleFmTextGenerationResponse {
    /// Converts the response into the simplified completion result.
    #[must_use]
    pub fn completion_result(&self) -> AppleFmCompletionResult {
        AppleFmCompletionResult {
            model: self.model.clone(),
            output: self.output.clone(),
            prompt_tokens: self.usage.as_ref().and_then(|usage| usage.prompt_tokens),
            completion_tokens: self
                .usage
                .as_ref()
                .and_then(|usage| usage.completion_tokens),
            total_tokens: self.usage.as_ref().and_then(|usage| usage.total_tokens),
            usage: self.usage.clone(),
        }
    }
}

/// Snapshot-versus-terminal stream event kinds for Apple FM text streaming.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleFmTextStreamEventKind {
    /// Intermediate full response snapshot.
    #[default]
    Snapshot,
    /// Terminal completion snapshot with final session/usage state.
    Completed,
}

/// Stream event payload yielded by the Apple FM text streaming transport.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmTextStreamEvent {
    /// Stream event kind.
    pub kind: AppleFmTextStreamEventKind,
    /// Served model identifier.
    pub model: String,
    /// Full response snapshot so far.
    pub output: String,
    /// Optional final session state on terminal completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<AppleFmSession>,
    /// Optional usage details on terminal completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<AppleFmChatUsage>,
}

impl AppleFmTextStreamEvent {
    /// Returns whether the event is terminal.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self.kind, AppleFmTextStreamEventKind::Completed)
    }
}

/// Tool metadata surfaced in session state and transcript history.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmSessionToolMetadata {
    /// Stable tool name.
    pub name: String,
    /// Optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Full tool definition registered for active Apple FM sessions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleFmToolDefinition {
    /// Stable tool name.
    pub name: String,
    /// Optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Structured schema for tool arguments.
    pub arguments_schema: AppleFmGenerationSchema,
}

impl AppleFmToolDefinition {
    /// Builds a tool definition from explicit schema input.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        description: Option<impl Into<String>>,
        arguments_schema: AppleFmGenerationSchema,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.map(Into::into),
            arguments_schema,
        }
    }

    /// Builds a tool definition from a typed Rust schema.
    pub fn typed<T>(
        name: impl Into<String>,
        description: Option<impl Into<String>>,
    ) -> Result<Self, AppleFmStructuredValueError>
    where
        T: AppleFmStructuredType,
    {
        Ok(Self::new(
            name,
            description,
            AppleFmGenerationSchema::from_type::<T>()?,
        ))
    }

    /// Drops callback-only detail and returns user-facing metadata.
    #[must_use]
    pub fn metadata(&self) -> AppleFmSessionToolMetadata {
        AppleFmSessionToolMetadata {
            name: self.name.clone(),
            description: self.description.clone(),
        }
    }
}

/// Session-scoped loopback callback configuration for tool execution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleFmToolCallbackConfiguration {
    /// Absolute callback URL the bridge should POST tool calls to.
    pub url: String,
    /// Stable opaque token identifying the Rust-side tool registry for this session.
    pub session_token: String,
}

/// Callback request emitted by the Swift bridge when Apple FM invokes a tool.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleFmToolCallRequest {
    /// Stable opaque session token identifying the Rust-side tool registry.
    pub session_token: String,
    /// The invoked tool's stable name.
    pub tool_name: String,
    /// Structured tool arguments decoded by Apple's guided-generation surface.
    pub arguments: AppleFmGeneratedContent,
}

/// Successful tool-call response returned by the Rust-side callback runtime.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleFmToolCallResponse {
    /// Tool output returned to the model.
    pub output: String,
}

/// Typed tool-call failure surfaced by the reusable Apple FM lane.
#[derive(Clone, Debug, Error, PartialEq, Serialize, Deserialize)]
#[error("tool '{tool_name}' failed: {underlying_error}")]
pub struct AppleFmToolCallError {
    /// Stable failed tool name.
    pub tool_name: String,
    /// Underlying tool failure detail.
    pub underlying_error: String,
}

impl AppleFmToolCallError {
    /// Builds a typed tool-call failure.
    #[must_use]
    pub fn new(tool_name: impl Into<String>, underlying_error: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            underlying_error: underlying_error.into(),
        }
    }
}

/// Reusable Apple FM session state.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmSession {
    /// Stable bridge session identifier.
    pub id: String,
    /// Optional system instructions bound to this session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Session-bound model configuration.
    pub model: AppleFmSystemLanguageModel,
    /// Registered tool metadata.
    #[serde(default)]
    pub tools: Vec<AppleFmSessionToolMetadata>,
    /// Whether a request is currently queued or executing for this session.
    #[serde(default)]
    pub is_responding: bool,
    /// Serialized transcript snapshot if the bridge included one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_json: Option<String>,
}

impl AppleFmSession {
    /// Decodes the typed transcript snapshot if the bridge included one.
    pub fn transcript(&self) -> Result<Option<AppleFmTranscript>, AppleFmTranscriptError> {
        self.transcript_json
            .as_deref()
            .map(AppleFmTranscript::from_json_str)
            .transpose()
    }
}

/// Session-creation / session-restore request.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleFmSessionCreateRequest {
    /// Optional system instructions for the new session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Optional model configuration. Defaults to the Apple system default model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<AppleFmSystemLanguageModel>,
    /// Active tool definitions registered for this session.
    #[serde(default)]
    pub tools: Vec<AppleFmToolDefinition>,
    /// Optional loopback callback configuration for tool execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_callback: Option<AppleFmToolCallbackConfiguration>,
    /// Optional transcript JSON used to restore a session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_json: Option<String>,
    /// Optional typed transcript dictionary used to restore a session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<AppleFmTranscript>,
}

impl AppleFmSessionCreateRequest {
    /// Builds a session-restore request from a transcript snapshot.
    #[must_use]
    pub fn from_transcript_json(
        transcript_json: impl Into<String>,
        model: Option<AppleFmSystemLanguageModel>,
        tools: Vec<AppleFmToolDefinition>,
    ) -> Self {
        Self {
            instructions: None,
            model,
            tools,
            tool_callback: None,
            transcript_json: Some(transcript_json.into()),
            transcript: None,
        }
    }

    /// Builds a session-restore request from a typed transcript snapshot.
    #[must_use]
    pub fn from_transcript(
        transcript: AppleFmTranscript,
        model: Option<AppleFmSystemLanguageModel>,
        tools: Vec<AppleFmToolDefinition>,
    ) -> Self {
        Self {
            instructions: None,
            model,
            tools,
            tool_callback: None,
            transcript_json: None,
            transcript: Some(transcript),
        }
    }

    /// Resolves the typed transcript requested for restore, if any.
    pub fn normalized_transcript(
        &self,
    ) -> Result<Option<AppleFmTranscript>, AppleFmTranscriptError> {
        match (self.transcript.as_ref(), self.transcript_json.as_deref()) {
            (Some(transcript), Some(transcript_json)) => {
                let parsed = AppleFmTranscript::from_json_str(transcript_json)?;
                if parsed != *transcript {
                    return Err(AppleFmTranscriptError::ConflictingInputs);
                }
                transcript.validate()?;
                Ok(Some(transcript.clone()))
            }
            (Some(transcript), None) => {
                transcript.validate()?;
                Ok(Some(transcript.clone()))
            }
            (None, Some(transcript_json)) => {
                AppleFmTranscript::from_json_str(transcript_json).map(Some)
            }
            (None, None) => Ok(None),
        }
    }
}

/// Session-creation response.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmSessionCreateResponse {
    /// Created session state.
    pub session: AppleFmSession,
}

/// Session-scoped prompt request.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleFmSessionRespondRequest {
    /// User-authored prompt content.
    pub prompt: String,
    /// Typed generation options for this response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<AppleFmGenerationOptions>,
}

impl AppleFmSessionRespondRequest {
    /// Validates the request generation options.
    pub fn validate(&self) -> Result<(), AppleFmGenerationOptionsValidationError> {
        if let Some(options) = self.options.as_ref() {
            options.validate()?;
        }
        Ok(())
    }
}

/// Session-scoped response result.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmSessionRespondResponse {
    /// Updated session state after the response completes.
    pub session: AppleFmSession,
    /// Served model identifier.
    pub model: String,
    /// First assistant text payload.
    pub output: String,
    /// Optional usage details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<AppleFmChatUsage>,
}

/// One-shot structured-generation request supported by the reusable client surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleFmStructuredGenerationRequest {
    /// Optional requested model identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// User-authored prompt content.
    pub prompt: String,
    /// Structured generation schema.
    pub schema: AppleFmGenerationSchema,
    /// Typed generation options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<AppleFmGenerationOptions>,
}

impl AppleFmStructuredGenerationRequest {
    /// Validates options and schema shape before transport.
    pub fn validate(&self) -> Result<(), AppleFmStructuredValueError> {
        self.schema.validate()?;
        if let Some(options) = self.options.as_ref() {
            options
                .validate()
                .map_err(|error| AppleFmStructuredValueError::OptionsValidation {
                    error: error.to_string(),
                })?;
        }
        Ok(())
    }
}

/// One-shot structured-generation result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleFmStructuredGenerationResponse {
    /// Served model identifier.
    pub model: String,
    /// Structured generated content.
    pub content: AppleFmGeneratedContent,
    /// Optional usage details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<AppleFmChatUsage>,
}

/// Session-scoped structured-generation request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleFmSessionStructuredGenerationRequest {
    /// User-authored prompt content.
    pub prompt: String,
    /// Structured generation schema.
    pub schema: AppleFmGenerationSchema,
    /// Typed generation options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<AppleFmGenerationOptions>,
}

impl AppleFmSessionStructuredGenerationRequest {
    /// Validates options and schema shape before transport.
    pub fn validate(&self) -> Result<(), AppleFmStructuredValueError> {
        self.schema.validate()?;
        if let Some(options) = self.options.as_ref() {
            options
                .validate()
                .map_err(|error| AppleFmStructuredValueError::OptionsValidation {
                    error: error.to_string(),
                })?;
        }
        Ok(())
    }
}

/// Session-scoped structured-generation result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleFmSessionStructuredGenerationResponse {
    /// Updated session state after the response completes.
    pub session: AppleFmSession,
    /// Served model identifier.
    pub model: String,
    /// Structured generated content.
    pub content: AppleFmGeneratedContent,
    /// Optional usage details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<AppleFmChatUsage>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        AppleFmChatCompletionRequest, AppleFmChatCompletionResponse, AppleFmChatMessageRole,
        AppleFmGenerationOptions, AppleFmGenerationOptionsValidationError, AppleFmHealthResponse,
        AppleFmModelsResponse, AppleFmSamplingMode, AppleFmSamplingModeType,
        AppleFmSessionCreateRequest, AppleFmSessionRespondRequest,
        AppleFmStructuredGenerationRequest, AppleFmSystemLanguageModel,
        AppleFmSystemLanguageModelGuardrails, AppleFmSystemLanguageModelUnavailableReason,
        AppleFmSystemLanguageModelUseCase, AppleFmTextGenerationRequest, AppleFmToolDefinition,
        AppleFmUsageTruth, DEFAULT_APPLE_FM_MODEL_ID,
    };
    use crate::structured::{AppleFmGenerationSchema, AppleFmStructuredValueError};
    use crate::transcript::{
        APPLE_FM_TRANSCRIPT_TYPE, AppleFmTranscript, AppleFmTranscriptContent,
        AppleFmTranscriptEntry, AppleFmTranscriptError, AppleFmTranscriptPayload,
    };

    fn search_tool_definition() -> AppleFmToolDefinition {
        AppleFmToolDefinition::new(
            "search",
            Some("Search the local index"),
            AppleFmGenerationSchema::from_json_str(
                r#"{"type":"object","properties":{"query":{"type":"string"}}}"#,
            )
            .expect("tool schema"),
        )
    }

    #[test]
    fn user_prompt_builder_matches_bridge_shape() {
        let request = AppleFmChatCompletionRequest::from_user_prompt(
            "hello",
            Some("apple-foundation-model"),
            Some(256),
            Some(0.2),
        );

        assert_eq!(request.model.as_deref(), Some("apple-foundation-model"));
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, AppleFmChatMessageRole::User);
        assert_eq!(request.messages[0].content, "hello");
        assert_eq!(request.max_tokens, Some(256));
        assert_eq!(request.temperature, Some(0.2));
        assert_eq!(
            request.options,
            Some(AppleFmGenerationOptions {
                sampling: None,
                temperature: Some(0.2),
                maximum_response_tokens: Some(256),
            })
        );
        assert!(!request.stream);
    }

    #[test]
    fn generation_options_validate_python_semantics() {
        let greedy = AppleFmSamplingMode::greedy();
        assert!(greedy.validate().is_ok());

        let random_top =
            AppleFmSamplingMode::random(Some(50), None, Some(42)).expect("valid top-k");
        assert_eq!(random_top.mode_type, AppleFmSamplingModeType::Random);
        assert_eq!(random_top.top, Some(50));
        assert_eq!(random_top.seed, Some(42));

        let random_threshold =
            AppleFmSamplingMode::random(None, Some(0.9), Some(7)).expect("valid top-p");
        assert_eq!(random_threshold.probability_threshold, Some(0.9));

        let conflict = AppleFmSamplingMode::random(Some(10), Some(0.8), None)
            .expect_err("top and threshold should conflict");
        assert_eq!(
            conflict,
            AppleFmGenerationOptionsValidationError::TopAndProbabilityThresholdConflict
        );

        let zero_top =
            AppleFmSamplingMode::random(Some(0), None, None).expect_err("top zero should fail");
        assert_eq!(
            zero_top,
            AppleFmGenerationOptionsValidationError::TopMustBePositive
        );

        let negative_temperature = AppleFmGenerationOptions::new(None, Some(-0.1), None)
            .expect_err("negative temperature should fail");
        assert_eq!(
            negative_temperature,
            AppleFmGenerationOptionsValidationError::NegativeTemperature
        );

        let zero_limit = AppleFmGenerationOptions::new(None, None, Some(0))
            .expect_err("zero maximum_response_tokens should fail");
        assert_eq!(
            zero_limit,
            AppleFmGenerationOptionsValidationError::MaximumResponseTokensMustBePositive
        );
    }

    #[test]
    fn models_response_collects_ids() {
        let response: AppleFmModelsResponse = serde_json::from_str(
            r#"{"object":"list","data":[{"id":"apple-foundation-model"},{"id":"apple-foundation-model-alt"}]}"#,
        )
        .expect("decode model list");

        assert_eq!(
            response.model_ids(),
            vec![
                "apple-foundation-model".to_string(),
                "apple-foundation-model-alt".to_string(),
            ]
        );
    }

    #[test]
    fn completion_response_extracts_first_text_result() {
        let response: AppleFmChatCompletionResponse = serde_json::from_str(
            r#"{
                "model":"apple-foundation-model",
                "choices":[{"message":{"role":"assistant","content":"hello from bridge"}}],
                "usage":{
                    "prompt_tokens":12,
                    "completion_tokens":5,
                    "total_tokens":17,
                    "prompt_tokens_detail":{"value":12,"truth":"exact"},
                    "completion_tokens_detail":{"value":5,"truth":"estimated"},
                    "total_tokens_detail":{"value":17,"truth":"estimated"}
                }
            }"#,
        )
        .expect("decode completion response");

        let result = response.completion_result();
        assert_eq!(result.model, "apple-foundation-model");
        assert_eq!(result.output, "hello from bridge");
        assert_eq!(result.prompt_tokens, Some(12));
        assert_eq!(result.completion_tokens, Some(5));
        assert_eq!(result.total_tokens, Some(17));
        assert_eq!(
            response
                .usage
                .as_ref()
                .and_then(|usage| usage.prompt_tokens_detail.as_ref())
                .map(|detail| detail.truth),
            Some(AppleFmUsageTruth::Exact)
        );
        assert_eq!(
            response
                .usage
                .as_ref()
                .and_then(|usage| usage.completion_tokens_detail.as_ref())
                .map(|detail| detail.truth),
            Some(AppleFmUsageTruth::Estimated)
        );
    }

    #[test]
    fn health_response_decodes_optional_fields() {
        let response: AppleFmHealthResponse = serde_json::from_str(
            r#"{
                "status":"ok",
                "model_available":true,
                "version":"1.0.0",
                "platform":"macOS",
                "availability_message":"Foundation Models is available",
                "default_use_case":"general",
                "default_guardrails":"default",
                "supported_use_cases":["general","content_tagging"],
                "supported_guardrails":["default","permissive_content_transformations"],
                "apple_silicon_required":true,
                "apple_intelligence_required":true
            }"#,
        )
        .expect("decode health response");

        assert!(response.model_available);
        assert_eq!(response.version.as_deref(), Some("1.0.0"));
        assert_eq!(response.platform.as_deref(), Some("macOS"));
        assert_eq!(
            response.availability_message.as_deref(),
            Some("Foundation Models is available")
        );
        assert_eq!(
            response.default_use_case,
            AppleFmSystemLanguageModelUseCase::General
        );
        assert_eq!(
            response.default_guardrails,
            AppleFmSystemLanguageModelGuardrails::Default
        );
        assert_eq!(
            response.supported_use_cases,
            vec![
                AppleFmSystemLanguageModelUseCase::General,
                AppleFmSystemLanguageModelUseCase::ContentTagging,
            ]
        );
        assert_eq!(
            response.supported_guardrails,
            vec![
                AppleFmSystemLanguageModelGuardrails::Default,
                AppleFmSystemLanguageModelGuardrails::PermissiveContentTransformations,
            ]
        );
        assert_eq!(response.apple_silicon_required, Some(true));
        assert_eq!(response.apple_intelligence_required, Some(true));

        let system_model = response.system_model_availability();
        assert_eq!(system_model.model.id, DEFAULT_APPLE_FM_MODEL_ID);
        assert_eq!(
            system_model.model.use_case,
            AppleFmSystemLanguageModelUseCase::General
        );
        assert_eq!(
            system_model.model.guardrails,
            AppleFmSystemLanguageModelGuardrails::Default
        );
        assert!(system_model.is_ready());
    }

    #[test]
    fn unknown_system_model_enums_decode_to_unknown() {
        let response: AppleFmHealthResponse = serde_json::from_str(
            r#"{
                "status":"degraded",
                "model_available":false,
                "availability_message":"future system state",
                "unavailable_reason":"future_reason",
                "default_use_case":"future_use_case",
                "default_guardrails":"future_guardrails",
                "supported_use_cases":["general","future_use_case"],
                "supported_guardrails":["default","future_guardrails"]
            }"#,
        )
        .expect("decode unknown health enums");

        assert_eq!(
            response.unavailable_reason,
            Some(AppleFmSystemLanguageModelUnavailableReason::Unknown)
        );
        assert_eq!(
            response.default_use_case,
            AppleFmSystemLanguageModelUseCase::Unknown
        );
        assert_eq!(
            response.default_guardrails,
            AppleFmSystemLanguageModelGuardrails::Unknown
        );
        assert_eq!(
            response.supported_use_cases,
            vec![
                AppleFmSystemLanguageModelUseCase::General,
                AppleFmSystemLanguageModelUseCase::Unknown,
            ]
        );
        assert_eq!(
            response.supported_guardrails,
            vec![
                AppleFmSystemLanguageModelGuardrails::Default,
                AppleFmSystemLanguageModelGuardrails::Unknown,
            ]
        );
    }

    #[test]
    fn session_restore_request_preserves_transcript_model_and_tools() {
        let request = AppleFmSessionCreateRequest::from_transcript_json(
            "{\"type\":\"FoundationModels.Transcript\"}",
            Some(AppleFmSystemLanguageModel::default()),
            vec![search_tool_definition()],
        );

        assert!(request.instructions.is_none());
        assert_eq!(
            request.transcript_json.as_deref(),
            Some("{\"type\":\"FoundationModels.Transcript\"}")
        );
        assert_eq!(request.model, Some(AppleFmSystemLanguageModel::default()));
        assert_eq!(request.tools.len(), 1);
        assert_eq!(request.tools[0].name, "search");
        assert!(request.transcript.is_none());
    }

    #[test]
    fn typed_session_restore_request_preserves_transcript_model_and_tools() {
        let transcript = AppleFmTranscript {
            transcript_type: APPLE_FM_TRANSCRIPT_TYPE.to_string(),
            transcript: AppleFmTranscriptPayload {
                entries: vec![AppleFmTranscriptEntry {
                    id: Some("entry-1".to_string()),
                    role: "user".to_string(),
                    contents: vec![AppleFmTranscriptContent {
                        content_type: "text".to_string(),
                        id: Some("content-1".to_string()),
                        extra: [(
                            "text".to_string(),
                            serde_json::Value::String("hello".to_string()),
                        )]
                        .into_iter()
                        .collect(),
                    }],
                    extra: Default::default(),
                }],
            },
            ..AppleFmTranscript::default()
        };
        let request = AppleFmSessionCreateRequest::from_transcript(
            transcript.clone(),
            Some(AppleFmSystemLanguageModel::default()),
            vec![],
        );

        assert!(request.transcript_json.is_none());
        assert_eq!(request.transcript, Some(transcript.clone()));
        assert_eq!(
            request
                .normalized_transcript()
                .expect("normalize transcript"),
            Some(transcript)
        );
    }

    #[test]
    fn session_restore_request_rejects_conflicting_transcript_inputs() {
        let request = AppleFmSessionCreateRequest {
            instructions: None,
            model: None,
            tools: vec![],
            tool_callback: None,
            transcript_json: Some(
                r#"{"version":1,"type":"FoundationModels.Transcript","transcript":{"entries":[]}}"#
                    .to_string(),
            ),
            transcript: Some(AppleFmTranscript {
                transcript_type: APPLE_FM_TRANSCRIPT_TYPE.to_string(),
                transcript: AppleFmTranscriptPayload {
                    entries: vec![AppleFmTranscriptEntry {
                        id: Some("entry-1".to_string()),
                        role: "user".to_string(),
                        contents: vec![],
                        extra: Default::default(),
                    }],
                },
                ..AppleFmTranscript::default()
            }),
        };

        let error = request
            .normalized_transcript()
            .expect_err("conflicting transcript inputs should fail");
        assert_eq!(error, AppleFmTranscriptError::ConflictingInputs);
    }

    #[test]
    fn text_generation_request_converts_to_chat_completion_contract() {
        let request = AppleFmTextGenerationRequest {
            model: Some("apple-foundation-model".to_string()),
            prompt: "hello".to_string(),
            options: Some(
                AppleFmGenerationOptions::new(
                    Some(AppleFmSamplingMode::greedy()),
                    Some(0.3),
                    Some(128),
                )
                .expect("valid generation options"),
            ),
        };

        request.validate().expect("request should validate");
        let chat_request = request.into_chat_completion_request();
        assert_eq!(chat_request.messages.len(), 1);
        assert_eq!(chat_request.messages[0].role, AppleFmChatMessageRole::User);
        assert_eq!(
            chat_request
                .options
                .as_ref()
                .and_then(|options| options.temperature),
            Some(0.3)
        );
        assert_eq!(chat_request.max_tokens, Some(128));
    }

    #[test]
    fn session_respond_request_validates_options() {
        let request = AppleFmSessionRespondRequest {
            prompt: "hello".to_string(),
            options: Some(AppleFmGenerationOptions {
                sampling: Some(AppleFmSamplingMode {
                    mode_type: AppleFmSamplingModeType::Greedy,
                    top: Some(5),
                    probability_threshold: None,
                    seed: None,
                }),
                temperature: None,
                maximum_response_tokens: None,
            }),
        };

        let error = request
            .validate()
            .expect_err("invalid greedy top should fail");
        assert_eq!(
            error,
            AppleFmGenerationOptionsValidationError::GreedyTopNotAllowed
        );
    }

    #[test]
    fn structured_generation_request_validates_schema_and_options() {
        let request = AppleFmStructuredGenerationRequest {
            model: None,
            prompt: "classify this".to_string(),
            schema: AppleFmGenerationSchema::from_json_str(
                r#"{"type":"object","properties":{"label":{"enum":["a","b"]}}}"#,
            )
            .expect("valid schema"),
            options: Some(AppleFmGenerationOptions {
                sampling: Some(AppleFmSamplingMode {
                    mode_type: AppleFmSamplingModeType::Greedy,
                    top: Some(3),
                    probability_threshold: None,
                    seed: None,
                }),
                temperature: None,
                maximum_response_tokens: None,
            }),
        };

        let error = request
            .validate()
            .expect_err("invalid structured options should fail");
        assert_eq!(
            error,
            AppleFmStructuredValueError::OptionsValidation {
                error: "greedy sampling does not accept 'top'".to_string()
            }
        );
    }

    #[test]
    fn generation_schema_rejects_non_object_schema() {
        let error = AppleFmGenerationSchema::new(serde_json::json!([]))
            .expect_err("non-object schema should fail");
        assert_eq!(error, AppleFmStructuredValueError::InvalidSchemaRoot);
    }
}
