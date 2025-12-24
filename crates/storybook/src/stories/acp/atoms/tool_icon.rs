//! Tool icon story.

use maud::{Markup, html};
use ui::acp::atoms::{tool_icon, ToolKind};

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

pub fn tool_icon_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Tool Icon"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Icons representing different tool kinds in ACP tool calls."
        }

        (section_title("All Tool Kinds"))
        (section(row(html! {
            (item("Read", tool_icon(ToolKind::Read)))
            (item("Edit", tool_icon(ToolKind::Edit)))
            (item("Delete", tool_icon(ToolKind::Delete)))
            (item("Execute", tool_icon(ToolKind::Execute)))
            (item("Search", tool_icon(ToolKind::Search)))
            (item("Think", tool_icon(ToolKind::Think)))
            (item("Fetch", tool_icon(ToolKind::Fetch)))
            (item("SwitchMode", tool_icon(ToolKind::SwitchMode)))
            (item("Other", tool_icon(ToolKind::Other)))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::acp::atoms::{tool_icon, ToolKind};

// Render a tool icon
tool_icon(ToolKind::Execute)

// Available kinds:
// - ToolKind::Read     (file read)
// - ToolKind::Edit     (file edit)
// - ToolKind::Delete   (file delete)
// - ToolKind::Execute  (command execution)
// - ToolKind::Search   (search operation)
// - ToolKind::Think    (agent thinking)
// - ToolKind::Fetch    (web fetch)
// - ToolKind::SwitchMode (mode switch)
// - ToolKind::Other    (generic tool)"#))
    }
}
