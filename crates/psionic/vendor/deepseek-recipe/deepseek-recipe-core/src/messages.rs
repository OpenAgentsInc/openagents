use crate::multimodal::ImageSource;
use crate::prompt::Prompt;
use crate::tools::ToolDefinition;

/// A message in the conversation supplied for prompt rendering.
///
/// System, user, and tool content is a [`Prompt`]: its text is plain text
/// except for explicit special tokens, such as the placeholders of a message's
/// image sources. Assistant content, reasoning content, and latest reminder
/// content are strings. Rendering inserts each grounding-token spelling found
/// in assistant content or reasoning content as an explicit special token;
/// all other text of these strings is plain text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMessage {
    System {
        content: Prompt,
        /// Image sources in the order of the placeholders in `content`.
        image_sources: Vec<ImageSource>,
        /// Tool definitions introduced by this message. V4.1 renders them after
        /// the content; V4 ignores them.
        tools: Option<Vec<ToolDefinition>>,
    },
    User {
        content: Prompt,
        /// Image sources in the order of the placeholders in `content`.
        image_sources: Vec<ImageSource>,
    },
    Assistant {
        content: String,
        reasoning_content: Option<String>,
        /// Tool calls of this message. V4 renders `Some` with an empty vector
        /// as an empty tool-call block; V4.1 renders it the same as `None`.
        tool_calls: Option<Vec<ToolCall>>,
        /// Whether rendering continues this message as a prefix continuation
        /// instead of starting a new assistant turn. The caller must set it
        /// only on the final message; Chat Completions conversion rejects
        /// other positions.
        prefix: bool,
    },
    Tool {
        content: Prompt,
        /// Image sources in the order of the placeholders in `content`.
        image_sources: Vec<ImageSource>,
        /// ID of the answered tool call.
        ///
        /// V4 sorts the messages that follow an assistant message with tool
        /// calls, one per call, by this ID, and a result without an ID sorts
        /// first. V4.1 sorts the results with an ID by the order of the calls
        /// of the preceding assistant message with tool calls, and a result
        /// without an ID keeps its position.
        tool_call_id: Option<String>,
    },
    LatestReminder {
        content: String,
    },
}

/// A historical tool call and its arguments serialized as JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}
