use serde::{Deserialize, Serialize};

use crate::messages::InputMessage;
use crate::tools::{ToolChoice, ToolDefinition};

/// A conversation and its prompt configuration.
///
/// Sampling parameters and transport options belong to the caller. Thinking
/// mode is enabled by default.
#[derive(Debug, Clone)]
pub struct Conversation {
    /// Messages in conversation order.
    pub messages: Vec<InputMessage>,
    /// Whether the next assistant turn uses thinking mode.
    ///
    /// Rendering uses it to select the thinking marker that opens the next
    /// assistant turn: [`ThinkingStart`] when `true` and [`ThinkingEnd`]
    /// otherwise. Rendering writes the reasoning effort prompt only when it is
    /// `true`, and V4.1 rendering includes the reasoning content of assistant
    /// messages only when it is `true`.
    ///
    /// [`ThinkingStart`]: crate::special_tokens::SpecialToken::ThinkingStart
    /// [`ThinkingEnd`]: crate::special_tokens::SpecialToken::ThinkingEnd
    pub thinking_mode: bool,
    /// Available client tools, in declaration order.
    pub tools: Vec<ToolDefinition>,
    /// Tool selection after protocol-specific normalization.
    pub tool_choice: ToolChoice,
    /// Reasoning effort for the reasoning effort prompt, which rendering
    /// writes only in thinking mode.
    ///
    /// `None` and [`ReasoningEffort::Xhigh`] render the same as
    /// [`ReasoningEffort::High`]. V4 rendering writes no reasoning effort prompt
    /// for [`ReasoningEffort::Low`].
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Requested answer format used during prompt rendering.
    pub response_format: ResponseFormat,
}

impl Default for Conversation {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            thinking_mode: true,
            tools: Vec::new(),
            tool_choice: ToolChoice::Auto,
            reasoning_effort: None,
            response_format: ResponseFormat::Text,
        }
    }
}

/// The requested answer format.
///
/// Rendering describes `JsonObject` and `JsonSchema` in the prompt. Constrained
/// decoding, which enforces a format during inference, belongs to the caller.
#[derive(Debug, Clone, Default)]
pub enum ResponseFormat {
    #[default]
    Text,
    JsonObject,
    /// A JSON Schema the answer must follow, rendered into the prompt.
    JsonSchema(serde_json::Value),
    /// A regular expression the answer must match. Rendering does not write it
    /// into the prompt.
    Regex(String),
}

/// Protocol-independent reasoning effort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Low,
    High,
    Xhigh,
    Max,
}
