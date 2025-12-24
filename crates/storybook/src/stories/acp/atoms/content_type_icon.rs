//! Content type icon story.

use maud::{Markup, html};
use ui::acp::atoms::{content_type_icon, ContentType};

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
        div class="flex flex-col gap-2 items-center" {
            (content)
            span class="text-xs text-muted-foreground" { (label) }
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

pub fn content_type_icon_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Content Type Icon"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Icons for tool call content types."
        }

        (section_title("Content Types"))
        (section(row(html! {
            (item("Content Block", content_type_icon(ContentType::ContentBlock)))
            (item("Diff", content_type_icon(ContentType::Diff)))
            (item("Terminal", content_type_icon(ContentType::Terminal)))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::acp::atoms::{content_type_icon, ContentType};

content_type_icon(ContentType::ContentBlock)
content_type_icon(ContentType::Diff)
content_type_icon(ContentType::Terminal)"#))
    }
}
