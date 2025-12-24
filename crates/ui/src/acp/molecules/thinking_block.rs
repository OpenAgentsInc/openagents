//! Collapsible thinking block component.

use maud::{Markup, html, PreEscaped};
use crate::acp::atoms::{thinking_toggle, ThinkingState};

/// Collapsible block for agent thinking/reasoning content.
pub struct ThinkingBlock {
    content: String,
    entry_id: String,
    initially_expanded: bool,
}

impl ThinkingBlock {
    /// Create a new thinking block.
    pub fn new(content: impl Into<String>, entry_id: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            entry_id: entry_id.into(),
            initially_expanded: false,
        }
    }

    /// Start expanded.
    pub fn expanded(mut self) -> Self {
        self.initially_expanded = true;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        let state = if self.initially_expanded {
            ThinkingState::Expanded
        } else {
            ThinkingState::Collapsed
        };

        html! {
            details
                class="bg-secondary/50 border border-border mb-2"
                open[self.initially_expanded]
            {
                summary class="px-3 py-2 cursor-pointer list-none" {
                    (thinking_toggle(state, &self.entry_id))
                }

                div class="px-3 py-2 text-sm text-muted-foreground border-t border-border" {
                    // Content (rendered as-is, could be markdown)
                    (PreEscaped(&self.content))
                }
            }
        }
    }
}
