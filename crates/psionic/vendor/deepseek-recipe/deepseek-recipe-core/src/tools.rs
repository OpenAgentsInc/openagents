//! Client tool definitions used when constructing a conversation.

/// The definition of a client tool, which the caller executes after receiving
/// a tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDefinition {
    /// Function name.
    pub name: String,
    /// Human-readable purpose of the function.
    pub description: Option<String>,
    /// JSON Schema describing the input object.
    pub parameters: serde_json::Value,
    /// Requested strict argument validation. Enforcement belongs to the caller.
    pub strict: Option<bool>,
    /// Omit this definition from the initial tool prompt. Loading it through
    /// a system message requires the V4.1 prompt encoding.
    pub defer_loading: bool,
}

/// Normalized tool selection for a rendered conversation.
///
/// Rendering includes the tool prompt only when the conversation defines at
/// least one tool. Without the tool prompt, rendering also omits the reasoning
/// content of historical assistant messages.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ToolChoice {
    /// Rendering includes the tool prompt, and the model can answer or call a
    /// tool.
    #[default]
    Auto,
    /// Rendering omits the tool prompt and, as a consequence, the reasoning
    /// content of historical assistant messages.
    None,
    /// Rendering includes the tool prompt and, when at least one tool is
    /// defined, starts the assistant output inside a tool-call block.
    Required,
}
