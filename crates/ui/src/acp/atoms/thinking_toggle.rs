//! Thinking block expand/collapse toggle.

use maud::{Markup, html};

/// State of the thinking block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingState {
    /// Block is collapsed
    Collapsed,
    /// Block is expanded
    Expanded,
}

impl ThinkingState {
    /// Label text for the toggle.
    fn label(&self) -> &'static str {
        match self {
            ThinkingState::Collapsed => "Show thinking",
            ThinkingState::Expanded => "Hide thinking",
        }
    }

    /// Chevron icon direction.
    fn chevron(&self) -> &'static str {
        match self {
            ThinkingState::Collapsed => ">",
            ThinkingState::Expanded => "v",
        }
    }
}

/// Render a thinking toggle control.
///
/// # Arguments
/// * `state` - Current expansion state
/// * `entry_id` - Unique ID for the entry (for JS toggle behavior)
pub fn thinking_toggle(state: ThinkingState, entry_id: &str) -> Markup {
    html! {
        button
            type="button"
            class="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
            data-thinking-toggle=(entry_id)
            aria-expanded=(matches!(state, ThinkingState::Expanded))
        {
            span class="text-[10px] transition-transform" { (state.chevron()) }
            span { (state.label()) }
        }
    }
}
