//! Streaming indicator story.

use maud::{Markup, html};
use ui::acp::atoms::streaming_indicator;

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

pub fn streaming_indicator_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Streaming Indicator"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Animated indicator showing content is being generated."
        }

        (section_title("Variants"))
        (section(row(html! {
            (item("Default", streaming_indicator(None)))
            (item("Custom Label", streaming_indicator(Some("Thinking..."))))
            (item("Custom Label", streaming_indicator(Some("Processing..."))))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::acp::atoms::streaming_indicator;

// Default label
streaming_indicator(None)
// → "Generating..."

// Custom label
streaming_indicator(Some("Thinking..."))"#))
    }
}
