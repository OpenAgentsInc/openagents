//! Entry marker story.

use maud::{Markup, html};
use ui::acp::atoms::{entry_marker, EntryKind};

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

pub fn entry_marker_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Entry Marker"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Visual markers for thread entry types."
        }

        (section_title("Entry Types"))
        (section(row(html! {
            (item("User", entry_marker(EntryKind::User)))
            (item("Assistant", entry_marker(EntryKind::Assistant)))
            (item("Tool", entry_marker(EntryKind::Tool)))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::acp::atoms::{entry_marker, EntryKind};

entry_marker(EntryKind::User)
entry_marker(EntryKind::Assistant)
entry_marker(EntryKind::Tool)"#))
    }
}
