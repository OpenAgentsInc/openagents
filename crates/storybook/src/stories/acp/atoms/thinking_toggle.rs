//! Thinking toggle story.

use maud::{Markup, html};
use ui::acp::atoms::{thinking_toggle, ThinkingState};

fn section_title(title: &str) -> Markup {
    html! { h2 class="text-base font-medium text-muted-foreground mt-8 mb-4" { (title) } }
}

fn section(content: Markup) -> Markup {
    html! { div class="p-4 border border-border bg-card mb-4" { (content) } }
}

fn row(content: Markup) -> Markup {
    html! { div class="flex gap-6 items-center flex-wrap" { (content) } }
}

fn item(label: &str, content: Markup) -> Markup {
    html! {
        div class="flex flex-col gap-2" {
            span class="text-xs text-muted-foreground" { (label) }
            (content)
        }
    }
}

fn code_block(code: &str) -> Markup {
    html! {
        pre class="text-xs bg-secondary border border-border p-4 overflow-x-auto text-muted-foreground" {
            code { (code) }
        }
    }
}

pub fn thinking_toggle_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Thinking Toggle"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Expand/collapse control for agent thinking blocks."
        }

        (section_title("States"))
        (section(row(html! {
            (item("Collapsed", thinking_toggle(ThinkingState::Collapsed, "demo-1")))
            (item("Expanded", thinking_toggle(ThinkingState::Expanded, "demo-2")))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::acp::atoms::{thinking_toggle, ThinkingState};

// Collapsed state
thinking_toggle(ThinkingState::Collapsed, "entry-id-123")

// Expanded state
thinking_toggle(ThinkingState::Expanded, "entry-id-123")"#))
    }
}
