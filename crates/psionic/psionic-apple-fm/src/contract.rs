use serde::{Deserialize, Serialize};

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
        Self {
            model: model.map(Into::into),
            messages: vec![AppleFmChatMessage {
                role: AppleFmChatMessageRole::User,
                content: prompt.into(),
            }],
            temperature,
            max_tokens,
            stream: false,
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
}

/// Structured error payload returned by the current bridge.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmErrorResponse {
    /// Error details.
    pub error: AppleFmErrorDetail,
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
}

/// Session-scoped tool metadata carried ahead of real tool-calling support.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmSessionToolMetadata {
    /// Stable tool name.
    pub name: String,
    /// Optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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

/// Session-creation / session-restore request.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmSessionCreateRequest {
    /// Optional system instructions for the new session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Optional model configuration. Defaults to the Apple system default model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<AppleFmSystemLanguageModel>,
    /// Tool metadata registered for this session.
    #[serde(default)]
    pub tools: Vec<AppleFmSessionToolMetadata>,
    /// Optional transcript JSON used to restore a session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_json: Option<String>,
}

impl AppleFmSessionCreateRequest {
    /// Builds a session-restore request from a transcript snapshot.
    #[must_use]
    pub fn from_transcript_json(
        transcript_json: impl Into<String>,
        model: Option<AppleFmSystemLanguageModel>,
        tools: Vec<AppleFmSessionToolMetadata>,
    ) -> Self {
        Self {
            instructions: None,
            model,
            tools,
            transcript_json: Some(transcript_json.into()),
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
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmSessionRespondRequest {
    /// User-authored prompt content.
    pub prompt: String,
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

#[cfg(test)]
mod tests {
    use super::{
        AppleFmChatCompletionRequest, AppleFmChatCompletionResponse, AppleFmChatMessageRole,
        AppleFmHealthResponse, AppleFmModelsResponse, AppleFmSessionCreateRequest,
        AppleFmSessionToolMetadata, AppleFmSystemLanguageModel,
        AppleFmSystemLanguageModelGuardrails, AppleFmSystemLanguageModelUnavailableReason,
        AppleFmSystemLanguageModelUseCase, DEFAULT_APPLE_FM_MODEL_ID,
    };

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
        assert!(!request.stream);
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
                "usage":{"prompt_tokens":12,"completion_tokens":5,"total_tokens":17}
            }"#,
        )
        .expect("decode completion response");

        let result = response.completion_result();
        assert_eq!(result.model, "apple-foundation-model");
        assert_eq!(result.output, "hello from bridge");
        assert_eq!(result.prompt_tokens, Some(12));
        assert_eq!(result.completion_tokens, Some(5));
        assert_eq!(result.total_tokens, Some(17));
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
            vec![AppleFmSessionToolMetadata {
                name: "search".to_string(),
                description: Some("Search the local index".to_string()),
            }],
        );

        assert!(request.instructions.is_none());
        assert_eq!(
            request.transcript_json.as_deref(),
            Some("{\"type\":\"FoundationModels.Transcript\"}")
        );
        assert_eq!(request.model, Some(AppleFmSystemLanguageModel::default()));
        assert_eq!(request.tools.len(), 1);
        assert_eq!(request.tools[0].name, "search");
    }
}
