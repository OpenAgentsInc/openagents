use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request to GPT-OSS completions API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssRequest {
    pub model: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(default)]
    pub stream: bool,
    /// JSON schema to constrain output (converted to GBNF grammar by llama-server)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub json_schema: Option<serde_json::Value>,
}

impl GptOssRequest {
    /// Create a new request with just model and prompt
    pub fn new(model: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            prompt: prompt.into(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            json_schema: None,
        }
    }

    /// Set JSON schema constraint for structured output
    pub fn with_json_schema(mut self, schema: serde_json::Value) -> Self {
        self.json_schema = Some(schema);
        self
    }
}

/// Response from GPT-OSS Responses API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssResponse {
    pub id: String,
    pub model: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageStats>,
}

/// Usage statistics from API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

/// OpenAI-compatible streaming chunk (completions API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssStreamChunk {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub choices: Vec<CompletionChoice>,
}

/// Choice in a completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionChoice {
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub text: String,
    pub finish_reason: Option<String>,
}

impl GptOssStreamChunk {
    /// Get the text delta from this chunk
    pub fn delta(&self) -> &str {
        self.choices.first().map(|c| c.text.as_str()).unwrap_or("")
    }

    /// Check if this is the final chunk
    pub fn is_done(&self) -> bool {
        self.choices
            .first()
            .and_then(|c| c.finish_reason.as_ref())
            .is_some()
    }
}

/// Request to GPT-OSS Responses API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssResponsesRequest {
    pub model: String,
    pub input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<GptOssToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<GptOssToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<GptOssReasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(default)]
    pub stream: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl GptOssResponsesRequest {
    pub fn new(model: impl Into<String>, input: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            input: serde_json::Value::String(input.into()),
            tools: None,
            tool_choice: None,
            reasoning: None,
            max_output_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            extra: HashMap::new(),
        }
    }

    pub fn with_tools(mut self, tools: Vec<GptOssToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn with_tool_choice(mut self, choice: GptOssToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }

    pub fn with_reasoning_effort(mut self, effort: GptOssReasoningEffort) -> Self {
        self.reasoning = Some(GptOssReasoning { effort });
        self
    }
}

/// Responses API response payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssResponsesResponse {
    pub id: String,
    pub model: String,
    #[serde(default)]
    pub output: Vec<GptOssResponsesOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<GptOssResponsesUsage>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl GptOssResponsesResponse {
    pub fn output_text(&self) -> String {
        let mut text = String::new();
        for output in &self.output {
            if let Some(content) = &output.content {
                for part in content {
                    if matches!(part.content_type.as_str(), "output_text" | "text")
                        && let Some(chunk) = &part.text
                    {
                        text.push_str(chunk);
                    }
                }
            }
        }
        text
    }

    pub fn tool_calls(&self) -> Vec<GptOssToolCall> {
        let mut calls = Vec::new();
        for output in &self.output {
            if output.output_type == "tool_call"
                && let Some(name) = output.name.clone()
            {
                calls.push(GptOssToolCall {
                    id: output.id.clone(),
                    name,
                    arguments: output
                        .arguments
                        .clone()
                        .unwrap_or_else(|| serde_json::Value::Null),
                });
            }

            if let Some(content) = &output.content {
                for part in content {
                    if part.content_type == "tool_call"
                        && let Some(name) = part.name.clone()
                    {
                        calls.push(GptOssToolCall {
                            id: output.id.clone(),
                            name,
                            arguments: part
                                .arguments
                                .clone()
                                .unwrap_or_else(|| serde_json::Value::Null),
                        });
                    }
                }
            }
        }
        calls
    }
}

/// Responses API output item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssResponsesOutput {
    #[serde(rename = "type")]
    pub output_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<GptOssResponsesContent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Responses API output content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssResponsesContent {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Responses API usage payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssResponsesUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
}

/// Responses API tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: GptOssToolFunction,
}

/// Tool function metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssToolFunction {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

/// Tool choice configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GptOssToolChoice {
    Mode(String),
    Named {
        #[serde(rename = "type")]
        tool_type: String,
        function: GptOssToolChoiceFunction,
    },
}

/// Tool choice function selector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssToolChoiceFunction {
    pub name: String,
}

/// Reasoning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssReasoning {
    pub effort: GptOssReasoningEffort,
}

/// Reasoning effort levels
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GptOssReasoningEffort {
    Low,
    Medium,
    High,
}

/// Normalized tool call extracted from responses output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssToolCall {
    pub id: Option<String>,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssModelInfo {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_context_length")]
    pub context_length: usize,
}

impl GptOssModelInfo {
    /// Get display name (falls back to id if name not set)
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

fn default_context_length() -> usize {
    8192
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

// ============ Chat Completions API types ============

/// Chat message for chat completions API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Request to chat completions API with structured output support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    /// Response format for structured output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    #[serde(default)]
    pub stream: bool,
}

/// Response format specification for structured output
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResponseFormat {
    /// Plain text output
    #[serde(rename = "text")]
    Text,
    /// JSON object output (model chooses structure)
    #[serde(rename = "json_object")]
    JsonObject,
    /// JSON schema constrained output
    #[serde(rename = "json_schema")]
    JsonSchema { json_schema: JsonSchemaSpec },
}

/// JSON schema specification for structured output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchemaSpec {
    /// Optional name for the schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The JSON schema itself
    pub schema: serde_json::Value,
    /// Whether to enforce strict schema adherence
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// Response from chat completions API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageStats>,
}

/// Choice in chat completions response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: usize,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

impl ChatCompletionsResponse {
    /// Get the content of the first choice
    pub fn content(&self) -> &str {
        self.choices
            .first()
            .map(|c| c.message.content.as_str())
            .unwrap_or("")
    }
}
