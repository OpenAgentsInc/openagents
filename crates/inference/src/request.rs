//! The Open Responses request (`CreateResponseBody`) and the pieces it
//! shares with the response object: tools, `tool_choice`, text format, and
//! reasoning settings.
//!
//! [`CreateResponse`] is also the gateway's internal request: a Chat
//! Completions request is translated onto it (see [`crate::chat`]), so a
//! request routes the same way whichever API it arrived on.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::ApiError;
use crate::item::{Item, Message, Role};
use crate::openagents::RequestOptions;
use crate::wire::{Extra, open_enum, tagged_union};

/// A function tool.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FunctionTool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A tool the model may call. Function tools are the only standard kind;
/// hosted tools are prefixed extensions and stay [`Tool::Unknown`].
#[derive(Clone, Debug, PartialEq)]
pub enum Tool {
    Function(FunctionTool),
    Unknown(Value),
}

tagged_union!(Tool { "function" => Function });

/// `tool_choice`'s simple modes. Closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoiceMode {
    None,
    Auto,
    Required,
}

/// `tool_choice`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolChoice {
    /// `"none"`, `"auto"`, or `"required"`.
    Mode(ToolChoiceMode),
    /// `{"type": "function", "name": ...}`: call this function.
    Function { name: String },
    /// `{"type": "allowed_tools", "mode": ..., "tools": [...]}`: only these
    /// functions may be called, a hard constraint the server enforces.
    AllowedTools {
        mode: ToolChoiceMode,
        tools: Vec<String>,
    },
}

impl Default for ToolChoice {
    fn default() -> Self {
        Self::Mode(ToolChoiceMode::Auto)
    }
}

impl Serialize for ToolChoice {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value = match self {
            Self::Mode(mode) => return mode.serialize(serializer),
            Self::Function { name } => serde_json::json!({"type": "function", "name": name}),
            Self::AllowedTools { mode, tools } => serde_json::json!({
                "type": "allowed_tools",
                "mode": mode,
                "tools": tools
                    .iter()
                    .map(|name| serde_json::json!({"type": "function", "name": name}))
                    .collect::<Vec<_>>(),
            }),
        };
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ToolChoice {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let value = Value::deserialize(deserializer)?;
        if value.is_string() {
            return serde_json::from_value(value)
                .map(Self::Mode)
                .map_err(D::Error::custom);
        }
        let kind = value.get("type").and_then(Value::as_str);
        let name_of = |tool: &Value| -> Result<String, D::Error> {
            match (tool.get("type").and_then(Value::as_str), tool.get("name")) {
                (Some("function"), Some(Value::String(name))) => Ok(name.clone()),
                _ => Err(D::Error::custom(
                    "allowed_tools entries must be {\"type\": \"function\", \"name\": ...}",
                )),
            }
        };
        match kind {
            Some("function") => Ok(Self::Function {
                name: name_of(&value)?,
            }),
            Some("allowed_tools") => {
                let mode = match value.get("mode") {
                    None | Some(Value::Null) => ToolChoiceMode::Auto,
                    Some(mode) => serde_json::from_value(mode.clone()).map_err(D::Error::custom)?,
                };
                let tools = value
                    .get("tools")
                    .and_then(Value::as_array)
                    .ok_or_else(|| D::Error::custom("allowed_tools needs a `tools` list"))?
                    .iter()
                    .map(name_of)
                    .collect::<Result<_, _>>()?;
                Ok(Self::AllowedTools { mode, tools })
            }
            _ => Err(D::Error::custom(format!(
                "unknown tool_choice {value}; expected a mode, a function, or allowed_tools"
            ))),
        }
    }
}

/// `json_schema` output format.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct JsonSchemaFormat {
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// An empty body for formats that carry nothing but their `type`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Empty {
    #[serde(flatten)]
    pub extra: Extra,
}

/// `text.format`.
///
/// `json_object` is in the spec's response schema but missing from its
/// request schema (`TextFormatParam`); we accept it on requests too, as
/// OpenAI does.
#[derive(Clone, Debug, PartialEq)]
pub enum TextFormat {
    Text(Empty),
    JsonObject(Empty),
    JsonSchema(JsonSchemaFormat),
    Unknown(Value),
}

tagged_union!(TextFormat {
    "text" => Text,
    "json_object" => JsonObject,
    "json_schema" => JsonSchema,
});

impl TextFormat {
    /// Plain text.
    #[must_use]
    pub fn text() -> Self {
        Self::Text(Empty::default())
    }
}

/// Output verbosity. Closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verbosity {
    Low,
    Medium,
    High,
}

/// `text`: output format and verbosity.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TextConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<TextFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<Verbosity>,
    #[serde(flatten)]
    pub extra: Extra,
}

open_enum! {
    /// Reasoning effort. The spec names five; upstreams take others
    /// (OpenAI's `minimal`), which pass through.
    pub enum ReasoningEffort {
        None = "none",
        Minimal = "minimal",
        Low = "low",
        Medium = "medium",
        High = "high",
        Xhigh = "xhigh",
    }
}

/// Reasoning summary setting. Closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningSummary {
    Concise,
    Detailed,
    Auto,
}

/// `reasoning`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReasoningConfig {
    #[serde(default)]
    pub effort: Option<ReasoningEffort>,
    #[serde(default)]
    pub summary: Option<ReasoningSummary>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Truncation policy. Closed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Truncation {
    Auto,
    #[default]
    Disabled,
}

open_enum! {
    /// Service tier. Servers may define their own.
    pub enum ServiceTier {
        Auto = "auto",
        Default = "default",
        Flex = "flex",
        Priority = "priority",
    }
}

open_enum! {
    /// Extra output to include.
    pub enum Include {
        ReasoningEncryptedContent = "reasoning.encrypted_content",
        OutputTextLogprobs = "message.output_text.logprobs",
    }
}

/// `stream_options`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StreamOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_obfuscation: Option<bool>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `input`: a bare string (one user message) or a list of items.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Input {
    Text(String),
    Items(Vec<Item>),
}

impl Input {
    /// The input as items, a bare string becoming one user message.
    #[must_use]
    pub fn into_items(self) -> Vec<Item> {
        match self {
            Self::Text(text) => vec![Item::Message(Message::text(Role::User, text))],
            Self::Items(items) => items,
        }
    }
}

/// Stop sequences: one string or several. Not in Open Responses; carried
/// for Chat Completions parity (see the crate README).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Stop {
    One(String),
    Many(Vec<String>),
}

/// An Open Responses request. Every field is optional on the wire.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Input>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<Include>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<TextConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<ServiceTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    /// Extension: stop sequences (Chat Completions parity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<Stop>,
    /// Extension: sampling seed (Chat Completions parity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    /// Extension: the caller's end-user id (Chat Completions parity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Extension: routing, privacy, payer, and price limits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openagents: Option<RequestOptions>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl CreateResponse {
    /// Whether the caller asked for a stream.
    #[must_use]
    pub fn is_stream(&self) -> bool {
        self.stream.unwrap_or(false)
    }

    /// The input as items (empty when absent).
    #[must_use]
    pub fn input_items(&self) -> Vec<Item> {
        self.input
            .clone()
            .map(Input::into_items)
            .unwrap_or_default()
    }

    /// The stateless rule the gateway serves until stored responses exist:
    /// `store: true` and `previous_response_id` are refused.
    pub fn require_stateless(&self) -> Result<(), ApiError> {
        if self.store == Some(true) {
            return Err(ApiError::invalid_request(
                "store",
                "Stored responses are not available yet. Send `store: false` and the whole conversation.",
            ));
        }
        if self.previous_response_id.is_some() {
            return Err(ApiError::invalid_request(
                "previous_response_id",
                "Continuing a stored response is not available yet. Send the whole conversation in `input`.",
            ));
        }
        Ok(())
    }

    /// Checks a function call's name against `tool_choice` and the tool
    /// list: the hard constraint the spec puts on `allowed_tools` (and on
    /// `none` and a named function).
    #[must_use]
    pub fn call_allowed(&self, name: &str) -> bool {
        let declared = self
            .tools
            .as_deref()
            .unwrap_or_default()
            .iter()
            .any(|tool| matches!(tool, Tool::Function(function) if function.name == name));
        match &self.tool_choice {
            Some(ToolChoice::Mode(ToolChoiceMode::None)) => false,
            Some(ToolChoice::Function { name: only }) => only == name,
            Some(ToolChoice::AllowedTools { mode, tools }) => {
                *mode != ToolChoiceMode::None && tools.iter().any(|allowed| allowed == name)
            }
            Some(ToolChoice::Mode(_)) | None => declared,
        }
    }
}
