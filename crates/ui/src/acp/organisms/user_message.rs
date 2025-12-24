//! User message component.

use maud::{Markup, html, PreEscaped};
use crate::acp::atoms::EntryKind;
use crate::acp::molecules::{MessageHeader, CheckpointRestore, EntryActions};
use crate::acp::styles::ACP_USER_MESSAGE_CLASS;

/// Full user message with optional checkpoint.
pub struct UserMessage {
    content: String,
    entry_id: String,
    timestamp: Option<String>,
    checkpoint_sha: Option<String>,
    editing: bool,
}

impl UserMessage {
    /// Create a new user message.
    pub fn new(content: impl Into<String>, entry_id: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            entry_id: entry_id.into(),
            timestamp: None,
            checkpoint_sha: None,
            editing: false,
        }
    }

    /// Set the timestamp.
    pub fn timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = Some(ts.into());
        self
    }

    /// Set checkpoint SHA for restore functionality.
    pub fn checkpoint(mut self, sha: impl Into<String>) -> Self {
        self.checkpoint_sha = Some(sha.into());
        self
    }

    /// Mark as currently being edited.
    pub fn editing(mut self) -> Self {
        self.editing = true;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        let mut header = MessageHeader::new(EntryKind::User, &self.entry_id).editable();
        if let Some(ts) = &self.timestamp {
            header = header.timestamp(ts);
        }

        html! {
            div class="mb-4" data-entry-id=(self.entry_id) {
                // Header
                (header.build())

                // Content
                div class=(ACP_USER_MESSAGE_CLASS) {
                    @if self.editing {
                        // Edit mode: textarea
                        textarea
                            class="w-full bg-transparent text-foreground resize-none focus:outline-none"
                            rows="3"
                            data-edit-content=(self.entry_id)
                        {
                            (self.content)
                        }
                        div class="mt-2 flex gap-2" {
                            button
                                type="button"
                                class="px-3 py-1 text-xs bg-primary text-primary-foreground"
                                data-save-edit=(self.entry_id)
                            {
                                "Save & Regenerate"
                            }
                            (EntryActions::for_editing(&self.entry_id).build())
                        }
                    } @else {
                        // Display mode
                        div class="text-sm text-foreground whitespace-pre-wrap" {
                            (PreEscaped(&self.content))
                        }
                    }
                }

                // Checkpoint restore (if available)
                @if let Some(sha) = &self.checkpoint_sha {
                    div class="mt-2" {
                        (CheckpointRestore::new(sha, &self.entry_id).build())
                    }
                }
            }
        }
    }
}
