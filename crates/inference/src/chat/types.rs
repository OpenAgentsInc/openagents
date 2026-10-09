//! OpenAI Chat Completions wire types: request, response, and stream
//! chunk. Unknown fields are kept in `extra` so a provider's own fields
//! (OpenRouter's `provider`, `top_k`) pass through the translation.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::ApiError;
use crate::item::ImageDetail;
use crate::openagents::{RequestOptions, ResponseInfo};
use crate::request::{ReasoningEffort, ServiceTier, Stop, ToolChoiceMode, Verbosity};
use crate::wire::{Extra, open_enum};

/// A message's content: a string or parts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatContent {
    Text(String),
    Parts(Vec<ChatPart>),
}

/// An image reference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<ImageDetail>,
}

/// An inline or uploaded file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FileRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

/// One content part.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
    InputAudio { input_audio: Value },
    File { file: FileRef },
    Refusal { refusal: String },
}

/// A function call the assistant made.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatToolCall {
    pub id: String,
    #[serde(rename = "type", default = "function_kind")]
    pub kind: String,
    pub function: ChatFunctionCall,
}

fn function_kind() -> String {
    "function".to_owned()
}

/// A call's name and JSON-encoded arguments.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatFunctionCall {
    pub name: String,
    pub arguments: String,
}

/// One message, by role. Closed: the legacy `function` role is refused.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum ChatMessage {
    System {
        content: ChatContent,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    Developer {
        content: ChatContent,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    User {
        content: ChatContent,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    Assistant {
        #[serde(default)]
        content: Option<ChatContent>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refusal: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ChatToolCall>>,
        /// Reasoning text (OpenRouter's convention).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    Tool {
        content: ChatContent,
        tool_call_id: String,
    },
}

/// A function definition.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatFunction {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// A tool: `{"type": "function", "function": {...}}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatTool {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ChatFunction,
}

/// `tool_choice`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatToolChoice {
    Mode(ToolChoiceMode),
    /// `{"type": "function", "function": {"name": ...}}`.
    Function(String),
    /// `{"type": "allowed_tools", "allowed_tools": {"mode", "tools"}}`.
    AllowedTools {
        mode: ToolChoiceMode,
        tools: Vec<String>,
    },
}

impl Serialize for ChatToolChoice {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let named =
            |name: &String| serde_json::json!({"type": "function", "function": {"name": name}});
        let value = match self {
            Self::Mode(mode) => return mode.serialize(serializer),
            Self::Function(name) => named(name),
            Self::AllowedTools { mode, tools } => serde_json::json!({
                "type": "allowed_tools",
                "allowed_tools": {"mode": mode, "tools": tools.iter().map(named).collect::<Vec<_>>()},
            }),
        };
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ChatToolChoice {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let value = Value::deserialize(deserializer)?;
        if value.is_string() {
            return serde_json::from_value(value)
                .map(Self::Mode)
                .map_err(D::Error::custom);
        }
        let name_of = |tool: &Value| -> Result<String, D::Error> {
            tool.pointer("/function/name")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| D::Error::custom("a named tool choice needs function.name"))
        };
        match value.get("type").and_then(Value::as_str) {
            Some("function") => Ok(Self::Function(name_of(&value)?)),
            Some("allowed_tools") => {
                let inner = value.get("allowed_tools").ok_or_else(|| {
                    D::Error::custom("allowed_tools needs an `allowed_tools` object")
                })?;
                let mode = match inner.get("mode") {
                    None | Some(Value::Null) => ToolChoiceMode::Auto,
                    Some(mode) => serde_json::from_value(mode.clone()).map_err(D::Error::custom)?,
                };
                let tools = inner
                    .get("tools")
                    .and_then(Value::as_array)
                    .ok_or_else(|| D::Error::custom("allowed_tools needs a `tools` list"))?
                    .iter()
                    .map(name_of)
                    .collect::<Result<_, _>>()?;
                Ok(Self::AllowedTools { mode, tools })
            }
            _ => Err(D::Error::custom(format!("unknown tool_choice {value}"))),
        }
    }
}

/// `response_format.json_schema`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatJsonSchema {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// `response_format`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFormat {
    Text,
    JsonObject,
    JsonSchema { json_schema: ChatJsonSchema },
}

/// `stream_options`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatStreamOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_usage: Option<bool>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A Chat Completions request.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub messages: Vec<ChatMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ChatTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ChatToolChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<Verbosity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<Stop>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<ChatStreamOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<ServiceTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openagents: Option<RequestOptions>,
    #[serde(flatten)]
    pub extra: Extra,
}

open_enum! {
    /// Why a choice stopped.
    pub enum FinishReason {
        Stop = "stop",
        Length = "length",
        ToolCalls = "tool_calls",
        ContentFilter = "content_filter",
        FunctionCall = "function_call",
        /// OpenRouter's value for a stream that failed after it began.
        Error = "error",
    }
}

/// The assistant's message in a non-streamed reply.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssistantMessage {
    #[serde(default = "assistant_role")]
    pub role: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub refusal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

fn assistant_role() -> String {
    "assistant".to_owned()
}

impl Default for AssistantMessage {
    fn default() -> Self {
        Self {
            role: assistant_role(),
            content: None,
            refusal: None,
            tool_calls: None,
            reasoning: None,
            extra: Extra::new(),
        }
    }
}

/// One choice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: AssistantMessage,
    #[serde(default)]
    pub finish_reason: Option<FinishReason>,
    #[serde(default)]
    pub logprobs: Option<Value>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `prompt_tokens_details`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptTokensDetails {
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `completion_tokens_details`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionTokensDetails {
    #[serde(default)]
    pub reasoning_tokens: u64,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Token usage.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<PromptTokensDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_tokens_details: Option<CompletionTokensDetails>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A non-streamed reply.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Completion {
    pub id: String,
    #[serde(default = "completion_object")]
    pub object: String,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub model: String,
    pub choices: Vec<Choice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<ChatUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<ServiceTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openagents: Option<ResponseInfo>,
    #[serde(flatten)]
    pub extra: Extra,
}

fn completion_object() -> String {
    "chat.completion".to_owned()
}

pub(crate) fn chunk_object() -> String {
    "chat.completion.chunk".to_owned()
}

/// A tool call fragment in a stream.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionCallDelta>,
}

/// A call's name (first fragment) and an arguments fragment.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FunctionCallDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// A chunk's delta.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Delta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
    /// Reasoning text (OpenRouter's convention).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// One choice in a chunk.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChunkChoice {
    pub index: u32,
    #[serde(default)]
    pub delta: Delta,
    #[serde(default)]
    pub finish_reason: Option<FinishReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<Value>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A streamed chunk.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatChunk {
    pub id: String,
    #[serde(default = "chunk_object")]
    pub object: String,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub choices: Vec<ChunkChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<ChatUsage>,
    /// A failure after the stream began.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openagents: Option<ResponseInfo>,
    #[serde(flatten)]
    pub extra: Extra,
}
