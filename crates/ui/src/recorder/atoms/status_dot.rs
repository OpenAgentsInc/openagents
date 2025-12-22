use maud::{Markup, html};

/// Status indicator dot with color variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusState {
    Success,
    Running,
    Pending,
    Error,
    Skipped,
}

impl StatusState {
    fn class(&self) -> &'static str {
        match self {
            StatusState::Success => "text-green",
            StatusState::Running => "text-blue",
            StatusState::Pending => "text-yellow",
            StatusState::Error => "text-red",
            StatusState::Skipped => "text-muted-foreground opacity-60",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            StatusState::Success => "success",
            StatusState::Running => "running",
            StatusState::Pending => "pending",
            StatusState::Error => "error",
            StatusState::Skipped => "skipped",
        }
    }

    fn dot_char(&self) -> &'static str {
        match self {
            StatusState::Skipped => "\u{25CB}", // ○
            _ => "\u{25CF}",                    // ●
        }
    }
}

pub fn status_dot(state: StatusState) -> Markup {
    html! {
        span title=(state.label()) class={ "text-xs leading-none " (state.class()) } {
            (state.dot_char())
        }
    }
}
