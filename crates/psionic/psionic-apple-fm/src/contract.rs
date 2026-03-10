use serde::{Deserialize, Serialize};

/// Default model identifier exposed by the retained Swift bridge.
pub const DEFAULT_APPLE_FM_MODEL_ID: &str = "apple-foundation-model";

/// Health endpoint path exposed by the retained Swift bridge.
pub const APPLE_FM_BRIDGE_HEALTH_PATH: &str = "/health";

/// Model-list endpoint path exposed by the retained Swift bridge.
pub const APPLE_FM_BRIDGE_MODELS_PATH: &str = "/v1/models";

/// Chat-completions endpoint path exposed by the retained Swift bridge.
pub const APPLE_FM_BRIDGE_CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";

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
    /// Whether Apple Silicon is required for this lane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple_silicon_required: Option<bool>,
    /// Whether Apple Intelligence is required for this lane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple_intelligence_required: Option<bool>,
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

#[cfg(test)]
mod tests {
    use super::{
        AppleFmChatCompletionRequest, AppleFmChatCompletionResponse, AppleFmChatMessageRole,
        AppleFmHealthResponse, AppleFmModelsResponse,
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
                "apple_silicon_required":true,
                "apple_intelligence_required":true
            }"#,
        )
        .expect("decode health response");

        assert!(response.model_available);
        assert_eq!(response.version.as_deref(), Some("1.0.0"));
        assert_eq!(response.platform.as_deref(), Some("macOS"));
    }
}
