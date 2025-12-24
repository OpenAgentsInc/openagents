//! Tool call header component.

use maud::{Markup, html};
use crate::acp::atoms::{tool_icon, tool_status_badge, ToolKind, ToolStatus};
use crate::acp::styles::ACP_HEADER_CLASS;

/// Tool call header with icon, label, and status.
pub struct ToolHeader {
    kind: ToolKind,
    label: String,
    status: Option<ToolStatus>,
}

impl ToolHeader {
    /// Create a new tool header.
    pub fn new(kind: ToolKind, label: impl Into<String>) -> Self {
        Self {
            kind,
            label: label.into(),
            status: None,
        }
    }

    /// Set the tool status.
    pub fn status(mut self, status: ToolStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        html! {
            div class=(ACP_HEADER_CLASS) {
                // Tool icon
                (tool_icon(self.kind))

                // Tool label
                span class="text-sm font-medium text-foreground flex-1" {
                    (self.label)
                }

                // Status badge (if present)
                @if let Some(status) = &self.status {
                    (tool_status_badge(status))
                }
            }
        }
    }
}
