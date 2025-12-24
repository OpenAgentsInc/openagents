//! Tool call status badge for ACP tool calls.

use maud::{Markup, html};

/// Status of a tool call execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolStatus {
    /// Tool is currently running
    Running,
    /// Tool completed successfully
    Success,
    /// Tool failed with an error
    Error(String),
    /// Waiting for user confirmation
    WaitingForConfirmation,
}

impl ToolStatus {
    /// CSS class for the status indicator.
    fn class(&self) -> &'static str {
        match self {
            ToolStatus::Running => "text-blue",
            ToolStatus::Success => "text-green",
            ToolStatus::Error(_) => "text-red",
            ToolStatus::WaitingForConfirmation => "text-yellow",
        }
    }

    /// Status icon character.
    fn icon(&self) -> &'static str {
        match self {
            ToolStatus::Running => "*",
            ToolStatus::Success => "+",
            ToolStatus::Error(_) => "x",
            ToolStatus::WaitingForConfirmation => "?",
        }
    }

    /// Human-readable label.
    fn label(&self) -> &'static str {
        match self {
            ToolStatus::Running => "Running...",
            ToolStatus::Success => "Success",
            ToolStatus::Error(_) => "Error",
            ToolStatus::WaitingForConfirmation => "Waiting...",
        }
    }

    /// Additional animation class for running state.
    fn animation_class(&self) -> &'static str {
        match self {
            ToolStatus::Running => "animate-pulse",
            _ => "",
        }
    }
}

/// Render a tool status badge.
pub fn tool_status_badge(status: &ToolStatus) -> Markup {
    let error_msg = match status {
        ToolStatus::Error(msg) => Some(msg.as_str()),
        _ => None,
    };

    html! {
        span
            class={
                "inline-flex items-center gap-1 text-xs font-mono "
                (status.class()) " "
                (status.animation_class())
            }
            title=[error_msg]
        {
            span class="leading-none" { (status.icon()) }
            span { (status.label()) }
        }
    }
}
