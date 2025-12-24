//! Permission dialog component for tool authorization.

use maud::{Markup, html};
use crate::acp::atoms::{tool_icon, ToolKind};
use crate::acp::molecules::PermissionBar;
use crate::acp::styles::ACP_CARD_CLASS;

/// Full permission request dialog.
pub struct PermissionDialog {
    tool_kind: ToolKind,
    tool_name: String,
    description: String,
    details: Option<String>,
    entry_id: String,
}

impl PermissionDialog {
    /// Create a new permission dialog.
    pub fn new(
        tool_kind: ToolKind,
        tool_name: impl Into<String>,
        description: impl Into<String>,
        entry_id: impl Into<String>,
    ) -> Self {
        Self {
            tool_kind,
            tool_name: tool_name.into(),
            description: description.into(),
            details: None,
            entry_id: entry_id.into(),
        }
    }

    /// Add additional details.
    pub fn details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        html! {
            div
                class={ (ACP_CARD_CLASS) " border-yellow" }
                data-entry-id=(self.entry_id)
            {
                // Header
                div class="px-4 py-3 border-b border-border flex items-center gap-3" {
                    // Warning icon
                    span class="text-lg text-yellow" { "[!]" }

                    div {
                        h3 class="text-sm font-medium text-foreground" {
                            "Permission Required"
                        }
                        p class="text-xs text-muted-foreground" {
                            "Claude wants to use a tool"
                        }
                    }
                }

                // Tool info
                div class="px-4 py-3 border-b border-border" {
                    div class="flex items-center gap-2 mb-2" {
                        (tool_icon(self.tool_kind))
                        span class="text-sm font-medium text-foreground" {
                            (self.tool_name)
                        }
                    }

                    p class="text-sm text-muted-foreground" {
                        (self.description)
                    }

                    @if let Some(details) = &self.details {
                        pre class="mt-2 text-xs font-mono text-muted-foreground bg-secondary p-2 overflow-x-auto" {
                            (details)
                        }
                    }
                }

                // Permission bar
                (PermissionBar::new().build())
            }
        }
    }
}
