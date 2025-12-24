//! Model badge story.

use maud::{Markup, html};
use ui::acp::atoms::model_badge;

fn section_title(title: &str) -> Markup {
    html! { h2 class="text-base font-medium text-muted-foreground mt-8 mb-4" { (title) } }
}

fn section(content: Markup) -> Markup {
    html! { div class="p-4 border border-border bg-card mb-4" { (content) } }
}

fn row(content: Markup) -> Markup {
    html! { div class="flex gap-4 items-center flex-wrap" { (content) } }
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

pub fn model_badge_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Model Badge"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Model identifier badges with compact and full display modes."
        }

        (section_title("Display Modes"))
        (section(row(html! {
            (item("Compact (sonnet)", model_badge("claude-sonnet-4-20250514", true)))
            (item("Full", model_badge("claude-sonnet-4-20250514", false)))
        })))

        (section_title("Different Models"))
        (section(row(html! {
            (item("Opus", model_badge("claude-opus-4-20250514", true)))
            (item("Sonnet", model_badge("claude-sonnet-4-20250514", true)))
            (item("Haiku", model_badge("claude-haiku-20250514", true)))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::acp::atoms::model_badge;

// Compact display (extracts short name)
model_badge("claude-sonnet-4-20250514", true)
// → "sonnet-4"

// Full display
model_badge("claude-sonnet-4-20250514", false)
// → "claude-sonnet-4-20250514""#))
    }
}
