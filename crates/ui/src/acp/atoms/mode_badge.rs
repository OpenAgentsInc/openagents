//! Agent mode indicator badge.

use maud::{Markup, html};

/// Agent operating mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentMode {
    /// Planning mode - agent is designing approach
    Plan,
    /// Code mode - agent is implementing
    Code,
    /// Ask mode - agent is gathering information
    Ask,
    /// Custom mode with arbitrary name
    Custom(String),
}

impl AgentMode {
    /// Display name for the mode.
    pub fn label(&self) -> &str {
        match self {
            AgentMode::Plan => "Plan",
            AgentMode::Code => "Code",
            AgentMode::Ask => "Ask",
            AgentMode::Custom(name) => name,
        }
    }

    /// CSS class for mode-specific styling.
    fn class(&self) -> &'static str {
        match self {
            AgentMode::Plan => "text-magenta",
            AgentMode::Code => "text-green",
            AgentMode::Ask => "text-cyan",
            AgentMode::Custom(_) => "text-yellow",
        }
    }

    /// Icon for the mode.
    fn icon(&self) -> &'static str {
        match self {
            AgentMode::Plan => "[P]",
            AgentMode::Code => "[C]",
            AgentMode::Ask => "[?]",
            AgentMode::Custom(_) => "[*]",
        }
    }
}

/// Render an agent mode badge.
pub fn mode_badge(mode: &AgentMode) -> Markup {
    html! {
        span
            class={
                "inline-flex items-center gap-1 px-2 py-0.5 text-xs font-mono "
                "bg-secondary border border-border "
                (mode.class())
            }
        {
            span class="text-xs" { (mode.icon()) }
            span { (mode.label()) }
        }
    }
}
