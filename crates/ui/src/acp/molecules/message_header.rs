//! Message header for user/assistant entries.

use maud::{Markup, html};
use crate::acp::atoms::{entry_marker, EntryKind};

/// Message header with entry marker, timestamp, and optional actions.
pub struct MessageHeader {
    kind: EntryKind,
    timestamp: Option<String>,
    editable: bool,
    entry_id: String,
}

impl MessageHeader {
    /// Create a new message header.
    pub fn new(kind: EntryKind, entry_id: impl Into<String>) -> Self {
        Self {
            kind,
            timestamp: None,
            editable: false,
            entry_id: entry_id.into(),
        }
    }

    /// Set the timestamp.
    pub fn timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = Some(ts.into());
        self
    }

    /// Mark as editable (shows edit button).
    pub fn editable(mut self) -> Self {
        self.editable = true;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        html! {
            div class="flex items-center gap-2 mb-2" {
                // Entry marker
                (entry_marker(self.kind))

                // Label
                span class="text-sm font-medium text-foreground" {
                    (self.kind.label())
                }

                // Timestamp
                @if let Some(ts) = &self.timestamp {
                    span class="text-xs text-muted-foreground font-mono" {
                        (ts)
                    }
                }

                // Spacer
                div class="flex-1" {}

                // Actions
                div class="flex gap-1" {
                    // Copy button
                    button
                        type="button"
                        class="p-1 text-muted-foreground hover:text-foreground"
                        data-copy-entry=(self.entry_id)
                        title="Copy"
                    {
                        "[c]"
                    }

                    // Edit button (for user messages)
                    @if self.editable {
                        button
                            type="button"
                            class="p-1 text-muted-foreground hover:text-foreground"
                            data-edit-entry=(self.entry_id)
                            title="Edit"
                        {
                            "[e]"
                        }
                    }
                }
            }
        }
    }
}
