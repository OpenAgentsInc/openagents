use maud::{Markup, html};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    Auto,
    Plan,
    Chat,
}

impl SessionMode {
    fn label(&self) -> &'static str {
        match self {
            SessionMode::Auto => "auto",
            SessionMode::Plan => "plan",
            SessionMode::Chat => "chat",
        }
    }

    fn accent_class(&self) -> &'static str {
        match self {
            SessionMode::Auto => "text-green",
            SessionMode::Plan => "text-blue",
            SessionMode::Chat => "text-muted-foreground",
        }
    }
}

pub fn mode_indicator(mode: SessionMode) -> Markup {
    html! {
        div class="inline-flex items-center gap-2 border border-border bg-secondary px-3 py-1" {
            span class="text-xs text-muted-foreground tracking-widest" { "MODE:" }
            span class={ "text-xs font-semibold " (mode.accent_class()) } { (mode.label()) }
        }
    }
}
