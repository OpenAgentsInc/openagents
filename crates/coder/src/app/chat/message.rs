use wgpui::markdown::MarkdownDocument;

/// Message role in the conversation.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MessageRole {
    User,
    Assistant,
    AssistantThought,
}

/// Metadata about a message response.
#[derive(Clone, Default)]
pub(crate) struct MessageMetadata {
    /// Model used to generate response.
    pub(crate) model: Option<String>,
    /// Input tokens.
    pub(crate) input_tokens: Option<u64>,
    /// Output tokens.
    pub(crate) output_tokens: Option<u64>,
    /// Generation time in milliseconds.
    pub(crate) duration_ms: Option<u64>,
    /// Cost in millisatoshis (if applicable).
    pub(crate) cost_msats: Option<u64>,
}

/// A chat message.
pub(crate) struct ChatMessage {
    pub(crate) role: MessageRole,
    pub(crate) content: String,
    /// Parsed markdown document for assistant messages.
    pub(crate) document: Option<MarkdownDocument>,
    pub(crate) uuid: Option<String>,
    /// Response metadata (model, tokens, timing).
    pub(crate) metadata: Option<MessageMetadata>,
}
