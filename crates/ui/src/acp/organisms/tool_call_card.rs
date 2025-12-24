//! Generic tool call card component.

use maud::{Markup, html, PreEscaped};
use crate::acp::atoms::{ToolKind, ToolStatus};
use crate::acp::molecules::{ToolHeader, PermissionBar};
use crate::acp::styles::{ACP_TOOL_CALL_CLASS, ACP_CONTENT_CLASS};

/// Generic tool call card with collapsible content.
pub struct ToolCallCard {
    kind: ToolKind,
    label: String,
    status: ToolStatus,
    content: Option<String>,
    entry_id: String,
    initially_expanded: bool,
    waiting_for_permission: bool,
}

impl ToolCallCard {
    /// Create a new tool call card.
    pub fn new(kind: ToolKind, label: impl Into<String>, entry_id: impl Into<String>) -> Self {
        Self {
            kind,
            label: label.into(),
            status: ToolStatus::Running,
            content: None,
            entry_id: entry_id.into(),
            initially_expanded: false,
            waiting_for_permission: false,
        }
    }

    /// Set the status.
    pub fn status(mut self, status: ToolStatus) -> Self {
        self.status = status;
        self
    }

    /// Set the content.
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Start expanded.
    pub fn expanded(mut self) -> Self {
        self.initially_expanded = true;
        self
    }

    /// Mark as waiting for permission.
    pub fn waiting_for_permission(mut self) -> Self {
        self.waiting_for_permission = true;
        self.status = ToolStatus::WaitingForConfirmation;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        html! {
            div class=(ACP_TOOL_CALL_CLASS) data-entry-id=(self.entry_id) {
                // Collapsible structure
                details open[self.initially_expanded || self.waiting_for_permission] {
                    summary class="cursor-pointer list-none" {
                        (ToolHeader::new(self.kind, &self.label)
                            .status(self.status.clone())
                            .build())
                    }

                    // Content area
                    @if let Some(content) = &self.content {
                        div class=(ACP_CONTENT_CLASS) {
                            pre class="whitespace-pre-wrap text-xs font-mono" {
                                (PreEscaped(content))
                            }
                        }
                    }

                    // Permission bar
                    @if self.waiting_for_permission {
                        (PermissionBar::new().build())
                    }
                }
            }
        }
    }
}
