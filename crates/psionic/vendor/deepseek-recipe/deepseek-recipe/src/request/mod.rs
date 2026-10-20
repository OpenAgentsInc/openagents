//! Shared request data and the conversion contract for protocol adapters.

use deepseek_recipe_core::conversation::{Conversation, ReasoningEffort};
use deepseek_recipe_core::tools::ToolDefinition;

use crate::stream::ParsingOptions;

pub use crate::protocol::output_constraint::resolve_output_constraint;
pub use crate::protocol::reasoning::{RequestReasoningEffort, ThinkingType, resolve_thinking_mode};
pub use options::{
    ConversionOptions, DEFAULT_MAX_TOKENS_UPPER_BOUND, ModelVersion, ReasoningContentBehavior,
    WebSearchBehavior,
};

mod options;

/// Inference parameters extracted from a protocol request.
///
/// Protocol adapters always set `temperature` and `top_p` to their resolved
/// values. Other unspecified parameters remain `None`.
/// [`ConversationRequest::resolve_max_tokens`] supplies the default
/// `max_tokens` after encoding; the caller chooses the other defaults.
#[derive(Debug, Default)]
pub struct InferenceOptions {
    /// Maximum number of output tokens, from 1 to the `max_tokens` upper bound
    /// of the request's thinking mode; see
    /// [`ConversionOptions::resolve_max_tokens_upper_bound`]. `None` when the
    /// request sets none; [`ConversationRequest::resolve_max_tokens`] supplies
    /// the default after the prompt is encoded.
    pub max_tokens: Option<u32>,
    /// Sampling temperature: 1.0 in thinking mode, otherwise the requested
    /// value or 1.0. Protocol adapters always set it.
    pub temperature: Option<f32>,
    /// Final `top_p` for the target model version; see
    /// [`ModelVersion::resolve_top_p`].
    pub top_p: Option<f32>,
    /// Whether the request asks for the log probability of each output token.
    /// The caller then supplies log probabilities in
    /// [`InferenceChunk::Tokens`](crate::stream::InferenceChunk::Tokens).
    pub logprobs: bool,
    /// Number of most likely alternatives reported per output token, in
    /// `[0, 20]`.
    pub top_logprobs: Option<u8>,
    /// Sampling seed in `[0, 2^63)`. Unspecified seeds are chosen by the caller.
    pub seed: Option<u64>,
    /// The constraint that constrained decoding would enforce on the output:
    /// the tool call block of a request with tools and a tool choice other than
    /// `none`, otherwise the JSON, JSON Schema, or regex of its response format.
    /// `None` when neither applies.
    /// Whether the backend applies it is the caller's decision.
    pub output_constraint: Option<OutputConstraint>,
}

/// A constraint on the model output that the backend can enforce by
/// constrained decoding.
///
/// [`Self::kind`] and [`Self::schema`] give the constraint as a kind name
/// and a constraint string.
#[derive(Debug, Clone, PartialEq)]
pub enum OutputConstraint {
    /// Any JSON value, or a JSON document that matches `schema`. In thinking
    /// mode, reasoning content precedes the JSON;
    /// [`Conversation::thinking_mode`] records the mode.
    Json { schema: Option<serde_json::Value> },
    /// Output that matches a regular expression.
    Regex(String),
    /// A tool call block whose calls use `tools`; with `required`, at least one
    /// call.
    ToolCalls {
        tools: Vec<ToolDefinition>,
        required: bool,
    },
}

impl OutputConstraint {
    /// Return the name of the constraint kind: `json`, `regex`, `tool_call`,
    /// or `tool_call_required`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Json { .. } => "json",
            Self::Regex(_) => "regex",
            Self::ToolCalls {
                required: false, ..
            } => "tool_call",
            Self::ToolCalls { required: true, .. } => "tool_call_required",
        }
    }

    /// Return the constraint string: the schema as compact JSON, the regular
    /// expression, the tool list as a compact JSON array of Chat Completions
    /// function tools with `defer_loading: true` on deferred tools, or an
    /// empty string for any JSON value.
    pub fn schema(&self) -> String {
        match self {
            Self::Json { schema: None } => String::new(),
            Self::Json {
                schema: Some(schema),
            } => schema.to_string(),
            Self::Regex(regex) => regex.clone(),
            Self::ToolCalls { tools, .. } => {
                let tools: Vec<serde_json::Value> = tools
                    .iter()
                    .map(|tool| {
                        let mut definition = serde_json::json!({
                            "type": "function",
                            "function": {
                                "name": tool.name,
                                "description": tool.description,
                                "parameters": tool.parameters,
                                "strict": tool.strict,
                            },
                        });
                        if tool.defer_loading {
                            definition["defer_loading"] = serde_json::json!(true);
                        }
                        definition
                    })
                    .collect();
                serde_json::Value::Array(tools).to_string()
            }
        }
    }
}

/// A conversation with inference parameters and output parsing options.
///
/// Protocol adapters produce this representation. The caller encodes the
/// conversation with the prompt encoding of [`Self::model_version`], calls
/// [`Self::resolve_max_tokens`], and constructs the inference request.
#[derive(Debug)]
pub struct ConversationRequest {
    pub conversation: Conversation,
    pub inference_options: InferenceOptions,
    pub parsing_options: ParsingOptions,
    /// Model version resolved during conversion from the request's model name.
    pub model_version: ModelVersion,
    pub stream: bool,
}

/// The default `max_tokens` outside thinking mode, which
/// [`ConversationRequest::resolve_max_tokens`] supplies when the request sets
/// none: 8192.
pub const DEFAULT_NON_THINKING_MAX_TOKENS: u32 = 8 * 1024;
/// The default `max_tokens` in thinking mode, which
/// [`ConversationRequest::resolve_max_tokens`] supplies when the request sets
/// none and the reasoning effort is not [`ReasoningEffort::Max`]: 65536.
pub const DEFAULT_THINKING_MAX_TOKENS: u32 = 64 * 1024;
/// The default `max_tokens` in thinking mode with [`ReasoningEffort::Max`],
/// which [`ConversationRequest::resolve_max_tokens`] supplies when the request
/// sets none: 131072.
pub const DEFAULT_MAX_EFFORT_MAX_TOKENS: u32 = 128 * 1024;

impl ConversationRequest {
    /// Construct a request with default inference, parsing, and transport options.
    pub fn new(conversation: Conversation, model_version: ModelVersion) -> Self {
        Self {
            conversation,
            inference_options: InferenceOptions::default(),
            parsing_options: ParsingOptions::default(),
            model_version,
            stream: false,
        }
    }

    /// Resolve the `max_tokens` to send to the backend from the encoded prompt
    /// length and the context limit of the model.
    ///
    /// `prompt_tokens` is the number of tokens in the encoded prompt, with each
    /// image counted at its image token count instead of as one placeholder;
    /// `context_limit` is the number of tokens the model's context holds. A
    /// requested `max_tokens` is kept. An unspecified one becomes
    /// [`DEFAULT_NON_THINKING_MAX_TOKENS`] outside thinking mode,
    /// [`DEFAULT_MAX_EFFORT_MAX_TOKENS`] in thinking mode with
    /// [`ReasoningEffort::Max`], and [`DEFAULT_THINKING_MAX_TOKENS`] in thinking
    /// mode otherwise. The result is lowered to the context left after the
    /// prompt and raised to at least 1.
    ///
    /// Call it after encoding the prompt; conversion cannot count prompt
    /// tokens.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::BadRequest`] when `prompt_tokens` plus the
    /// requested `max_tokens` exceeds `context_limit`, or, when the request
    /// sets no `max_tokens`, when `prompt_tokens` alone exceeds it. The detail
    /// is `This model's maximum context length is {context_limit} tokens.
    /// However, you requested {total} tokens ({prompt_tokens} in the messages,
    /// {max_tokens} in the completion). Please reduce the length of the
    /// messages or completion.`, with `{max_tokens}` 0 when the request sets
    /// none.
    pub fn resolve_max_tokens(
        &self,
        prompt_tokens: u32,
        context_limit: u32,
    ) -> Result<u32, ConversionError> {
        let requested = self.inference_options.max_tokens.unwrap_or(0);
        let total = prompt_tokens.saturating_add(requested);
        if total > context_limit {
            return Err(ConversionError::bad_request(format!(
                "This model's maximum context length is {context_limit} tokens. However, you \
                 requested {total} tokens ({prompt_tokens} in the messages, {requested} in the \
                 completion). Please reduce the length of the messages or completion."
            )));
        }
        let default = if !self.conversation.thinking_mode {
            DEFAULT_NON_THINKING_MAX_TOKENS
        } else if self.conversation.reasoning_effort == Some(ReasoningEffort::Max) {
            DEFAULT_MAX_EFFORT_MAX_TOKENS
        } else {
            DEFAULT_THINKING_MAX_TOKENS
        };
        let remaining = context_limit - prompt_tokens;
        Ok(self
            .inference_options
            .max_tokens
            .unwrap_or(default)
            .min(remaining)
            .max(1))
    }
}

/// A failure while converting protocol input into a conversation request.
#[derive(Debug)]
pub enum ConversionError {
    /// The input is invalid or uses an unsupported feature.
    BadRequest { detail: String },
    /// An internal conversion operation failed.
    Internal { detail: String },
}

impl ConversionError {
    pub fn bad_request(detail: impl Into<String>) -> Self {
        ConversionError::BadRequest {
            detail: detail.into(),
        }
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        ConversionError::Internal {
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadRequest { detail } | Self::Internal { detail } => f.write_str(detail),
        }
    }
}

impl std::error::Error for ConversionError {}
