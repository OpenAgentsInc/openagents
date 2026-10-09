//! The Open Responses response object (`ResponseResource`) and usage.
//!
//! Decoding is lenient where upstreams differ from the spec: a missing
//! field the spec marks required takes its zero value, and unknown fields
//! are kept in `extra`. Encoding always writes every field the spec
//! requires, with `null` for the nullable ones, so what we serve validates.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::ResponseError;
use crate::item::{ContentPart, Item, ItemStatus, MessageContent};
use crate::openagents::ResponseInfo;
use crate::request::{
    CreateResponse, ReasoningConfig, ServiceTier, TextConfig, TextFormat, Tool, ToolChoice,
    Truncation,
};
use crate::wire::{Extra, open_enum};

open_enum! {
    /// A response's lifecycle state.
    pub enum ResponseStatus {
        Queued = "queued",
        InProgress = "in_progress",
        Completed = "completed",
        Incomplete = "incomplete",
        Failed = "failed",
        Cancelled = "cancelled",
    }
}

impl Default for ResponseStatus {
    fn default() -> Self {
        Self::InProgress
    }
}

impl ResponseStatus {
    /// Whether no further events follow this status.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Incomplete | Self::Failed | Self::Cancelled
        )
    }
}

open_enum! {
    /// Why a response stopped early.
    pub enum IncompleteReason {
        MaxOutputTokens = "max_output_tokens",
        ContentFilter = "content_filter",
    }
}

/// `incomplete_details`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IncompleteDetails {
    pub reason: IncompleteReason,
    #[serde(flatten)]
    pub extra: Extra,
}

impl IncompleteDetails {
    #[must_use]
    pub fn new(reason: IncompleteReason) -> Self {
        Self {
            reason,
            extra: Extra::new(),
        }
    }
}

/// Input token detail.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputTokensDetails {
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Output token detail.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputTokensDetails {
    #[serde(default)]
    pub reasoning_tokens: u64,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Token usage.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub input_tokens_details: InputTokensDetails,
    #[serde(default)]
    pub output_tokens_details: OutputTokensDetails,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Usage {
    /// Usage from counts, with `total_tokens` their sum.
    #[must_use]
    pub fn new(input: u64, cached: u64, output: u64, reasoning: u64) -> Self {
        Self {
            input_tokens: input,
            output_tokens: output,
            total_tokens: input + output,
            input_tokens_details: InputTokensDetails {
                cached_tokens: cached,
                extra: Extra::new(),
            },
            output_tokens_details: OutputTokensDetails {
                reasoning_tokens: reasoning,
                extra: Extra::new(),
            },
            extra: Extra::new(),
        }
    }
}

fn object_response() -> String {
    "response".to_owned()
}
fn one() -> f64 {
    1.0
}
fn yes() -> bool {
    true
}

/// A response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub id: String,
    #[serde(default = "object_response")]
    pub object: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub completed_at: Option<u64>,
    #[serde(default)]
    pub status: ResponseStatus,
    #[serde(default)]
    pub incomplete_details: Option<IncompleteDetails>,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub previous_response_id: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub output: Vec<Item>,
    #[serde(default)]
    pub error: Option<ResponseError>,
    #[serde(default, serialize_with = "tools_out")]
    pub tools: Vec<Tool>,
    #[serde(default)]
    pub tool_choice: ToolChoice,
    #[serde(default)]
    pub truncation: Truncation,
    #[serde(default = "yes")]
    pub parallel_tool_calls: bool,
    #[serde(default)]
    pub text: TextConfig,
    #[serde(default = "one")]
    pub top_p: f64,
    #[serde(default)]
    pub presence_penalty: f64,
    #[serde(default)]
    pub frequency_penalty: f64,
    #[serde(default)]
    pub top_logprobs: u64,
    #[serde(default = "one")]
    pub temperature: f64,
    #[serde(default)]
    pub reasoning: Option<ReasoningConfig>,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(default)]
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub max_tool_calls: Option<u64>,
    #[serde(default)]
    pub store: bool,
    #[serde(default)]
    pub background: bool,
    #[serde(default = "default_tier")]
    pub service_tier: ServiceTier,
    #[serde(default)]
    pub metadata: Map<String, Value>,
    #[serde(default)]
    pub safety_identifier: Option<String>,
    #[serde(default)]
    pub prompt_cache_key: Option<String>,
    /// Extension: which model and upstream answered, attempts, and cost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openagents: Option<ResponseInfo>,
    #[serde(flatten)]
    pub extra: Extra,
}

fn default_tier() -> ServiceTier {
    ServiceTier::Default
}

impl Response {
    /// An in-progress response echoing a request's settings, as
    /// `response.created` carries it.
    #[must_use]
    pub fn from_request(
        id: impl Into<String>,
        created_at: u64,
        model: impl Into<String>,
        request: &CreateResponse,
    ) -> Self {
        Self {
            id: id.into(),
            object: object_response(),
            created_at,
            completed_at: None,
            status: ResponseStatus::InProgress,
            incomplete_details: None,
            model: model.into(),
            previous_response_id: request.previous_response_id.clone(),
            instructions: request.instructions.clone(),
            output: Vec::new(),
            error: None,
            tools: request.tools.clone().unwrap_or_default(),
            tool_choice: request.tool_choice.clone().unwrap_or_default(),
            truncation: request.truncation.unwrap_or_default(),
            parallel_tool_calls: request.parallel_tool_calls.unwrap_or(true),
            text: request.text.clone().unwrap_or_else(|| TextConfig {
                format: Some(TextFormat::text()),
                ..TextConfig::default()
            }),
            top_p: request.top_p.unwrap_or(1.0),
            presence_penalty: request.presence_penalty.unwrap_or(0.0),
            frequency_penalty: request.frequency_penalty.unwrap_or(0.0),
            top_logprobs: request.top_logprobs.unwrap_or(0),
            temperature: request.temperature.unwrap_or(1.0),
            reasoning: request.reasoning.clone(),
            usage: None,
            max_output_tokens: request.max_output_tokens,
            max_tool_calls: request.max_tool_calls,
            store: request.store.unwrap_or(false),
            background: request.background.unwrap_or(false),
            service_tier: request.service_tier.clone().unwrap_or_else(default_tier),
            metadata: request.metadata.clone().unwrap_or_default(),
            safety_identifier: request.safety_identifier.clone(),
            prompt_cache_key: request.prompt_cache_key.clone(),
            openagents: None,
            extra: Extra::new(),
        }
    }

    /// All `output_text` of assistant messages, in order.
    #[must_use]
    pub fn output_text(&self) -> String {
        let mut out = String::new();
        for item in &self.output {
            let Item::Message(message) = item else {
                continue;
            };
            match &message.content {
                MessageContent::Text(text) => out.push_str(text),
                MessageContent::Parts(parts) => {
                    for part in parts {
                        if let ContentPart::OutputText(text) = part {
                            out.push_str(&text.text);
                        }
                    }
                }
            }
        }
        out
    }

    /// Checks the fields the spec constrains beyond their types: a
    /// terminal status has a matching detail object, output items carry
    /// `id` and `status`, and only the last item may be `incomplete`.
    pub fn validate(&self) -> Result<(), String> {
        match self.status {
            ResponseStatus::Incomplete if self.incomplete_details.is_none() => {
                return Err("an incomplete response needs incomplete_details".into());
            }
            ResponseStatus::Failed if self.error.is_none() => {
                return Err("a failed response needs error".into());
            }
            _ => {}
        }
        let last = self.output.len().saturating_sub(1);
        for (index, item) in self.output.iter().enumerate() {
            if matches!(item, Item::Unknown(_)) {
                continue;
            }
            if item.id().is_none() {
                return Err(format!("output[{index}] has no id"));
            }
            match item.status() {
                None => return Err(format!("output[{index}] has no status")),
                Some(ItemStatus::Incomplete) if index != last => {
                    return Err(format!("output[{index}] is incomplete but not last"));
                }
                Some(ItemStatus::Incomplete) if self.status != ResponseStatus::Incomplete => {
                    return Err(format!(
                        "output[{index}] is incomplete but the response is {}",
                        self.status
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// A response's tools as the spec's `ResponseResource` requires them: a
/// function tool always carries `description`, `parameters`, and `strict`
/// (`null` when the request left them out). A request's tools are sent
/// upstream as the caller wrote them; only the response fills the gaps.
fn tools_out<S: serde::Serializer>(tools: &[Tool], serializer: S) -> Result<S::Ok, S::Error> {
    use serde::ser::{Error, SerializeSeq};
    let mut seq = serializer.serialize_seq(Some(tools.len()))?;
    for tool in tools {
        let mut value = serde_json::to_value(tool).map_err(S::Error::custom)?;
        if let (Tool::Function(_), Some(fields)) = (tool, value.as_object_mut()) {
            for key in ["description", "parameters", "strict"] {
                fields.entry(key).or_insert(Value::Null);
            }
        }
        seq.serialize_element(&value)?;
    }
    seq.end()
}

#[cfg(test)]
mod tools_tests {
    use super::*;

    #[test]
    fn a_responses_function_tools_carry_every_required_field() {
        let request: CreateResponse = serde_json::from_value(serde_json::json!({
            "model": "m",
            "tools": [{"type": "function", "name": "get_weather"}]
        }))
        .unwrap();
        let response = Response::from_request("resp_1", 1, "m", &request);
        let value = serde_json::to_value(&response).unwrap();
        let tool = &value["tools"][0];
        assert_eq!(tool["type"], "function");
        for key in ["description", "parameters", "strict"] {
            assert!(tool.get(key).is_some_and(Value::is_null), "{key}: {tool}");
        }
        // The request itself still goes upstream as written.
        let sent = serde_json::to_value(&request).unwrap();
        assert!(sent["tools"][0].get("strict").is_none());
    }
}
