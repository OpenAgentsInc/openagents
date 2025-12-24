//! Mode selector dropdown.

use maud::{Markup, html};
use crate::acp::atoms::{mode_badge, AgentMode};

/// Mode selector with current mode and available options.
pub struct ModeSelector {
    current: AgentMode,
    available: Vec<AgentMode>,
    session_id: String,
}

impl ModeSelector {
    /// Create a new mode selector.
    pub fn new(current: AgentMode, session_id: impl Into<String>) -> Self {
        Self {
            current,
            available: vec![
                AgentMode::Plan,
                AgentMode::Code,
                AgentMode::Ask,
            ],
            session_id: session_id.into(),
        }
    }

    /// Set available modes.
    pub fn available(mut self, modes: Vec<AgentMode>) -> Self {
        self.available = modes;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        html! {
            details class="relative group" {
                summary class="cursor-pointer list-none" {
                    (mode_badge(&self.current))
                }

                // Dropdown menu
                div class="absolute top-full left-0 mt-1 z-50 bg-popover border border-border min-w-[120px]" {
                    @for mode in &self.available {
                        button
                            type="button"
                            class={
                                "block w-full text-left px-3 py-2 text-sm hover:bg-accent "
                                @if mode == &self.current { "bg-accent" }
                            }
                            data-session-id=(self.session_id)
                            data-mode=(mode.label())
                        {
                            (mode.label())
                        }
                    }
                }
            }
        }
    }
}
