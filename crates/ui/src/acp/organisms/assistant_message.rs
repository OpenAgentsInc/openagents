//! Assistant message component.

use maud::{Markup, html, PreEscaped};
use crate::acp::atoms::{EntryKind, streaming_indicator};
use crate::acp::molecules::{MessageHeader, ThinkingBlock};
use crate::acp::styles::ACP_ASSISTANT_MESSAGE_CLASS;

/// Content chunk in an assistant message.
pub enum MessageChunk {
    /// Regular text/markdown content
    Text(String),
    /// Thinking/reasoning block
    Thinking(String),
}

/// Full assistant message with streaming support.
pub struct AssistantMessage {
    chunks: Vec<MessageChunk>,
    entry_id: String,
    timestamp: Option<String>,
    streaming: bool,
}

impl AssistantMessage {
    /// Create a new assistant message.
    pub fn new(entry_id: impl Into<String>) -> Self {
        Self {
            chunks: Vec::new(),
            entry_id: entry_id.into(),
            timestamp: None,
            streaming: false,
        }
    }

    /// Add a text chunk.
    pub fn text(mut self, content: impl Into<String>) -> Self {
        self.chunks.push(MessageChunk::Text(content.into()));
        self
    }

    /// Add a thinking block.
    pub fn thinking(mut self, content: impl Into<String>) -> Self {
        self.chunks.push(MessageChunk::Thinking(content.into()));
        self
    }

    /// Set chunks from a vector.
    pub fn chunks(mut self, chunks: Vec<MessageChunk>) -> Self {
        self.chunks = chunks;
        self
    }

    /// Set the timestamp.
    pub fn timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = Some(ts.into());
        self
    }

    /// Mark as currently streaming.
    pub fn streaming(mut self) -> Self {
        self.streaming = true;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        let mut header = MessageHeader::new(EntryKind::Assistant, &self.entry_id);
        if let Some(ts) = &self.timestamp {
            header = header.timestamp(ts);
        }

        html! {
            div class="mb-4" data-entry-id=(self.entry_id) {
                // Header
                (header.build())

                // Content
                div class=(ACP_ASSISTANT_MESSAGE_CLASS) {
                    @for (idx, chunk) in self.chunks.iter().enumerate() {
                        @match chunk {
                            MessageChunk::Text(content) => {
                                div class="text-sm text-foreground" {
                                    (PreEscaped(content))
                                }
                            }
                            MessageChunk::Thinking(content) => {
                                (ThinkingBlock::new(content, format!("{}-thinking-{}", self.entry_id, idx)).build())
                            }
                        }
                    }

                    // Streaming indicator
                    @if self.streaming {
                        div class="mt-2" {
                            (streaming_indicator(None))
                        }
                    }
                }
            }
        }
    }
}
