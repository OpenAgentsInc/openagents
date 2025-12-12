//! Google Gemini provider implementation

use crate::{
    ChatOptions, ChatResponse, ChatStream, ContentPart, LlmError, LlmProvider, LlmResult, Message,
    ModelCapabilities, ModelInfo, ProviderConfig, Role, StopReason, StreamChunk, Usage,
};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
const DEFAULT_MODEL: &str = "gemini-1.5-flash";
const DEFAULT_MAX_TOKENS: u32 = 8192;

/// Google Gemini provider
pub struct GeminiProvider {
    client: Client,
    config: ProviderConfig,
}

impl GeminiProvider {
    /// Create a new Gemini provider
    pub fn new(config: ProviderConfig) -> LlmResult<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| LlmError::ConfigurationError(e.to_string()))?;

        Ok(Self { client, config })
    }

    /// Get the base URL
    fn base_url(&self) -> &str {
        self.config.base_url.as_deref().unwrap_or(GEMINI_API_URL)
    }

    /// Convert messages to Gemini format
    fn convert_messages(&self, messages: &[Message]) -> (Option<String>, Vec<GeminiContent>) {
        let mut system = None;
        let mut contents = Vec::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    let text = msg.text();
                    system = Some(match system {
                        Some(existing) => format!("{}\n\n{}", existing, text),
                        None => text,
                    });
                }
                Role::User => {
                    let parts = self.convert_content_to_parts(msg);
                    if !parts.is_empty() {
                        contents.push(GeminiContent {
                            role: "user".to_string(),
                            parts,
                        });
                    }
                }
                Role::Assistant => {
                    let parts = self.convert_content_to_parts(msg);
                    if !parts.is_empty() {
                        contents.push(GeminiContent {
                            role: "model".to_string(),
                            parts,
                        });
                    }
                }
                Role::Tool => {
                    // Tool results in Gemini use functionResponse
                    let text = msg.text();
                    contents.push(GeminiContent {
                        role: "user".to_string(),
                        parts: vec![GeminiPart::FunctionResponse {
                            function_response: GeminiFunctionResponse {
                                name: msg.name.clone().unwrap_or_else(|| "tool".to_string()),
                                response: GeminiFunctionResponseContent {
                                    name: msg.name.clone().unwrap_or_else(|| "tool".to_string()),
                                    content: text,
                                },
                            },
                        }],
                    });
                }
            }
        }

        (system, contents)
    }

    /// Convert message content to Gemini parts
    fn convert_content_to_parts(&self, msg: &Message) -> Vec<GeminiPart> {
        let mut parts = Vec::new();

        match &msg.content {
            crate::Content::Text(text) => {
                parts.push(GeminiPart::Text { text: text.clone() });
            }
            crate::Content::Parts(content_parts) => {
                for part in content_parts {
                    match part {
                        ContentPart::Text { text } => {
                            parts.push(GeminiPart::Text { text: text.clone() });
                        }
                        ContentPart::Image { source } => {
                            if let crate::ImageSource::Base64 { media_type, data } = source {
                                parts.push(GeminiPart::InlineData {
                                    inline_data: GeminiInlineData {
                                        mime_type: media_type.clone(),
                                        data: data.clone(),
                                    },
                                });
                            }
                        }
                        ContentPart::ToolUse { id: _, name, input } => {
                            parts.push(GeminiPart::FunctionCall {
                                function_call: GeminiFunctionCall {
                                    name: name.clone(),
                                    args: input.clone(),
                                },
                            });
                        }
                        _ => {}
                    }
                }
            }
        }

        parts
    }

    /// Convert tools to Gemini format
    fn convert_tools(&self, tools: &[crate::ToolDefinition]) -> Option<Vec<GeminiToolDeclaration>> {
        if tools.is_empty() {
            return None;
        }

        Some(vec![GeminiToolDeclaration {
            function_declarations: tools
                .iter()
                .map(|t| GeminiFunctionDeclaration {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                })
                .collect(),
        }])
    }

    /// Parse response into ChatResponse
    fn parse_response(&self, response: GeminiResponse, model: &str) -> LlmResult<ChatResponse> {
        let candidate = response
            .candidates
            .first()
            .ok_or_else(|| LlmError::InvalidRequest("No candidates in response".to_string()))?;

        let mut content_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for part in &candidate.content.parts {
            match part {
                GeminiPart::Text { text } => {
                    content_parts.push(ContentPart::Text { text: text.clone() });
                }
                GeminiPart::FunctionCall { function_call } => {
                    tool_calls.push(ContentPart::ToolUse {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: function_call.name.clone(),
                        input: function_call.args.clone(),
                    });
                }
                _ => {}
            }
        }

        // Combine text and tool calls
        content_parts.extend(tool_calls);

        let stop_reason = candidate.finish_reason.as_deref().map(|r| match r {
            "STOP" => StopReason::EndTurn,
            "MAX_TOKENS" => StopReason::MaxTokens,
            "SAFETY" => StopReason::ContentFilter,
            _ => StopReason::Unknown,
        });

        let usage = response.usage_metadata.map(|u| Usage {
            input_tokens: u.prompt_token_count.unwrap_or(0),
            output_tokens: u.candidates_token_count.unwrap_or(0),
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: u.cached_content_token_count.unwrap_or(0),
        }).unwrap_or_default();

        Ok(ChatResponse {
            id: response.model_version.unwrap_or_default(),
            model: model.to_string(),
            content: content_parts,
            stop_reason,
            usage,
        })
    }

    /// Parse error response
    fn parse_error(&self, _status: reqwest::StatusCode, body: &str) -> LlmError {
        if let Ok(error_response) = serde_json::from_str::<GeminiErrorResponse>(body) {
            let msg = error_response.error.message;
            let code = error_response.error.code;

            match code {
                401 | 403 => LlmError::AuthenticationError(msg),
                429 => LlmError::RateLimitError(msg),
                400 => LlmError::InvalidRequest(msg),
                404 => LlmError::ModelNotFound(msg),
                _ => LlmError::ProviderError {
                    provider: "gemini".to_string(),
                    message: msg,
                },
            }
        } else {
            LlmError::ProviderError {
                provider: "gemini".to_string(),
                message: body.to_string(),
            }
        }
    }
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn default_model(&self) -> &str {
        self.config
            .default_model
            .as_deref()
            .unwrap_or(DEFAULT_MODEL)
    }

    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        Ok(vec![
            ModelInfo {
                id: "gemini-1.5-pro".to_string(),
                name: "Gemini 1.5 Pro".to_string(),
                context_length: 2_000_000,
                capabilities: ModelCapabilities {
                    tool_use: true,
                    vision: true,
                    streaming: true,
                    thinking: false,
                    input_cost_per_mtok: Some(1.25),
                    output_cost_per_mtok: Some(5.0),
                },
            },
            ModelInfo {
                id: "gemini-1.5-flash".to_string(),
                name: "Gemini 1.5 Flash".to_string(),
                context_length: 1_000_000,
                capabilities: ModelCapabilities {
                    tool_use: true,
                    vision: true,
                    streaming: true,
                    thinking: false,
                    input_cost_per_mtok: Some(0.075),
                    output_cost_per_mtok: Some(0.30),
                },
            },
            ModelInfo {
                id: "gemini-2.0-flash-exp".to_string(),
                name: "Gemini 2.0 Flash (Experimental)".to_string(),
                context_length: 1_000_000,
                capabilities: ModelCapabilities {
                    tool_use: true,
                    vision: true,
                    streaming: true,
                    thinking: true,
                    input_cost_per_mtok: None,
                    output_cost_per_mtok: None,
                },
            },
        ])
    }

    async fn model_info(&self, model: &str) -> LlmResult<ModelInfo> {
        let models = self.list_models().await?;
        models
            .into_iter()
            .find(|m| m.id == model)
            .ok_or_else(|| LlmError::ModelNotFound(model.to_string()))
    }

    async fn chat(
        &self,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> LlmResult<ChatResponse> {
        let options = options.unwrap_or_default();
        let model = options
            .model
            .clone()
            .unwrap_or_else(|| self.default_model().to_string());

        let (system_instruction, contents) = self.convert_messages(messages);

        let mut generation_config = GeminiGenerationConfig::default();
        if let Some(temp) = options.temperature {
            generation_config.temperature = Some(temp);
        }
        if let Some(max) = options.max_tokens {
            generation_config.max_output_tokens = Some(max);
        } else {
            generation_config.max_output_tokens = Some(DEFAULT_MAX_TOKENS);
        }
        if let Some(p) = options.top_p {
            generation_config.top_p = Some(p);
        }

        let request = GeminiRequest {
            contents,
            system_instruction: system_instruction.or(options.system).map(|s| GeminiSystemInstruction {
                parts: vec![GeminiPart::Text { text: s }],
            }),
            tools: self.convert_tools(&options.tools),
            tool_config: if !options.tools.is_empty() {
                Some(GeminiToolConfig {
                    function_calling_config: GeminiFunctionCallingConfig {
                        mode: "AUTO".to_string(),
                    },
                })
            } else {
                None
            },
            generation_config: Some(generation_config),
        };

        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url(),
            model,
            self.config.api_key
        );

        let response = self.client.post(&url).json(&request).send().await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(self.parse_error(status, &body));
        }

        let gemini_response: GeminiResponse = serde_json::from_str(&body)?;
        self.parse_response(gemini_response, &model)
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> LlmResult<ChatStream> {
        let options = options.unwrap_or_default();
        let model = options
            .model
            .clone()
            .unwrap_or_else(|| self.default_model().to_string());

        let (system_instruction, contents) = self.convert_messages(messages);

        let mut generation_config = GeminiGenerationConfig::default();
        if let Some(temp) = options.temperature {
            generation_config.temperature = Some(temp);
        }
        if let Some(max) = options.max_tokens {
            generation_config.max_output_tokens = Some(max);
        } else {
            generation_config.max_output_tokens = Some(DEFAULT_MAX_TOKENS);
        }

        let request = GeminiRequest {
            contents,
            system_instruction: system_instruction.or(options.system).map(|s| GeminiSystemInstruction {
                parts: vec![GeminiPart::Text { text: s }],
            }),
            tools: self.convert_tools(&options.tools),
            tool_config: if !options.tools.is_empty() {
                Some(GeminiToolConfig {
                    function_calling_config: GeminiFunctionCallingConfig {
                        mode: "AUTO".to_string(),
                    },
                })
            } else {
                None
            },
            generation_config: Some(generation_config),
        };

        let url = format!(
            "{}/models/{}:streamGenerateContent?key={}&alt=sse",
            self.base_url(),
            model,
            self.config.api_key
        );

        let response = self.client.post(&url).json(&request).send().await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await?;
            return Err(self.parse_error(status, &body));
        }

        let stream = response.bytes_stream();
        let model_clone = model.clone();
        Ok(Box::pin(parse_sse_stream(stream, model_clone)))
    }

    async fn health_check(&self) -> LlmResult<bool> {
        let messages = vec![Message::user("Hi")];
        let options = ChatOptions::default().max_tokens(1);
        match self.chat(&messages, Some(options)).await {
            Ok(_) => Ok(true),
            Err(LlmError::AuthenticationError(_)) => Ok(false),
            Err(_) => Ok(false),
        }
    }
}

/// Parse SSE stream from Gemini
fn parse_sse_stream(
    stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = LlmResult<StreamChunk>> + Send {
    let started = false;
    let model_for_start = model.clone();

    futures::stream::unfold(
        (stream.boxed(), String::new(), started, model_for_start),
        |(mut stream, mut buffer, mut started, model)| async move {
            loop {
                // Look for complete SSE data
                if let Some(data_start) = buffer.find("data: ") {
                    let offset = data_start + 6;
                    if let Some(newline) = buffer[offset..].find('\n') {
                        let data = buffer[offset..offset + newline].trim().to_string();
                        buffer = buffer[offset + newline + 1..].to_string();

                        if data == "[DONE]" {
                            continue;
                        }

                        if let Ok(response) = serde_json::from_str::<GeminiResponse>(&data) {
                            // Emit start chunk once
                            if !started {
                                started = true;
                                return Some((
                                    Ok(StreamChunk::Start {
                                        id: response.model_version.clone().unwrap_or_default(),
                                        model: model.clone(),
                                    }),
                                    (stream, buffer, started, model),
                                ));
                            }

                            // Extract text from response
                            if let Some(candidate) = response.candidates.first() {
                                for part in &candidate.content.parts {
                                    if let GeminiPart::Text { text } = part {
                                        return Some((
                                            Ok(StreamChunk::Text(text.clone())),
                                            (stream, buffer, started, model),
                                        ));
                                    }
                                }

                                // Check for done
                                if candidate.finish_reason.is_some() {
                                    let usage = response.usage_metadata.map(|u| Usage {
                                        input_tokens: u.prompt_token_count.unwrap_or(0),
                                        output_tokens: u.candidates_token_count.unwrap_or(0),
                                        cache_creation_input_tokens: 0,
                                        cache_read_input_tokens: 0,
                                    }).unwrap_or_default();

                                    return Some((
                                        Ok(StreamChunk::Done {
                                            stop_reason: Some(StopReason::EndTurn),
                                            usage,
                                        }),
                                        (stream, buffer, started, model),
                                    ));
                                }
                            }
                        }
                        continue;
                    }
                }

                // Need more data
                match stream.next().await {
                    Some(Ok(bytes)) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    Some(Err(e)) => {
                        return Some((Err(LlmError::from(e)), (stream, buffer, started, model)));
                    }
                    None => {
                        return None;
                    }
                }
            }
        },
    )
}

// Gemini API types

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiToolDeclaration>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_config: Option<GeminiToolConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
}

#[derive(Debug, Serialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum GeminiPart {
    Text {
        text: String,
    },
    InlineData {
        #[serde(rename = "inlineData")]
        inline_data: GeminiInlineData,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GeminiFunctionResponse,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiInlineData {
    mime_type: String,
    data: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiFunctionResponse {
    name: String,
    response: GeminiFunctionResponseContent,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiFunctionResponseContent {
    name: String,
    content: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiToolDeclaration {
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Serialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiToolConfig {
    function_calling_config: GeminiFunctionCallingConfig,
}

#[derive(Debug, Serialize)]
struct GeminiFunctionCallingConfig {
    mode: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    usage_metadata: Option<GeminiUsageMetadata>,
    #[serde(default)]
    model_version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCandidate {
    content: GeminiContent,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiUsageMetadata {
    #[serde(default)]
    prompt_token_count: Option<u32>,
    #[serde(default)]
    candidates_token_count: Option<u32>,
    #[serde(default)]
    cached_content_token_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorResponse {
    error: GeminiError,
}

#[derive(Debug, Deserialize)]
struct GeminiError {
    code: u16,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_messages() {
        let config = ProviderConfig::new("test-key");
        let provider = GeminiProvider::new(config).unwrap();

        let messages = vec![
            Message::system("You are helpful"),
            Message::user("Hello"),
            Message::assistant("Hi there!"),
        ];

        let (system, contents) = provider.convert_messages(&messages);
        assert_eq!(system, Some("You are helpful".to_string()));
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0].role, "user");
        assert_eq!(contents[1].role, "model");
    }
}
