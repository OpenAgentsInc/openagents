//! Tool status badge story.

use maud::{Markup, html};
use ui::acp::atoms::{tool_status_badge, ToolStatus};

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

pub fn tool_status_badge_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Tool Status Badge"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Status indicators for tool call execution state."
        }

        (section_title("Status States"))
        (section(row(html! {
            (item("Running", tool_status_badge(&ToolStatus::Running)))
            (item("Success", tool_status_badge(&ToolStatus::Success)))
            (item("Error", tool_status_badge(&ToolStatus::Error("File not found".to_string()))))
            (item("Waiting", tool_status_badge(&ToolStatus::WaitingForConfirmation)))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::acp::atoms::{tool_status_badge, ToolStatus};

// Running state (animates)
tool_status_badge(&ToolStatus::Running)

// Success state
tool_status_badge(&ToolStatus::Success)

// Error state with message
tool_status_badge(&ToolStatus::Error("Permission denied".to_string()))

// Waiting for user confirmation
tool_status_badge(&ToolStatus::WaitingForConfirmation)"#))
    }
}
