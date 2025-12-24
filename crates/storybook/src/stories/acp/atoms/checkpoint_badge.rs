//! Checkpoint badge story.

use maud::{Markup, html};
use ui::acp::atoms::checkpoint_badge;

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

pub fn checkpoint_badge_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Checkpoint Badge"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Indicator for restorable checkpoint state with Git SHA."
        }

        (section_title("Examples"))
        (section(row(html! {
            (item("Short SHA", checkpoint_badge("abc1234")))
            (item("Full SHA (truncated)", checkpoint_badge("abc1234567890def1234567890")))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::acp::atoms::checkpoint_badge;

// Shows first 7 characters of SHA
checkpoint_badge("abc1234567890def")
// → "abc1234""#))
    }
}
