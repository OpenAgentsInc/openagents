//! Unified thread entry component.

use maud::{Markup, html};
use crate::acp::organisms::{UserMessage, AssistantMessage, ToolCallCard};
use crate::acp::atoms::{ToolKind, ToolStatus};

/// Kind of thread entry with its data.
pub enum ThreadEntryKind {
    /// User message
    User {
        content: String,
        timestamp: Option<String>,
        checkpoint_sha: Option<String>,
    },
    /// Assistant message
    Assistant {
        content: String,
        timestamp: Option<String>,
        streaming: bool,
    },
    /// Tool call
    Tool {
        kind: ToolKind,
        label: String,
        status: ToolStatus,
        content: Option<String>,
    },
}

/// Unified thread entry that dispatches to specific components.
pub struct ThreadEntry {
    kind: ThreadEntryKind,
    entry_id: String,
    index: usize,
}

impl ThreadEntry {
    /// Create a new thread entry.
    pub fn new(kind: ThreadEntryKind, entry_id: impl Into<String>, index: usize) -> Self {
        Self {
            kind,
            entry_id: entry_id.into(),
            index,
        }
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        // Build the inner content based on kind, outside the html! macro
        let inner_content = match self.kind {
            ThreadEntryKind::User { content, timestamp, checkpoint_sha } => {
                let mut msg = UserMessage::new(&content, &self.entry_id);
                if let Some(ts) = timestamp {
                    msg = msg.timestamp(ts);
                }
                if let Some(sha) = checkpoint_sha {
                    msg = msg.checkpoint(sha);
                }
                msg.build()
            }
            ThreadEntryKind::Assistant { content, timestamp, streaming } => {
                let mut msg = AssistantMessage::new(&self.entry_id)
                    .text(&content);
                if let Some(ts) = timestamp {
                    msg = msg.timestamp(ts);
                }
                if streaming {
                    msg = msg.streaming();
                }
                msg.build()
            }
            ThreadEntryKind::Tool { kind, label, status, content } => {
                let mut card = ToolCallCard::new(kind, &label, &self.entry_id)
                    .status(status);
                if let Some(c) = content {
                    card = card.content(c);
                }
                card.build()
            }
        };

        html! {
            div
                class="thread-entry"
                data-entry-index=(self.index)
                data-entry-id=(self.entry_id)
            {
                (inner_content)
            }
        }
    }
}
