//! Mode badge story.

use maud::{Markup, html};
use ui::acp::atoms::{mode_badge, AgentMode};

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

pub fn mode_badge_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Mode Badge"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Agent mode indicators showing the current operating mode."
        }

        (section_title("Standard Modes"))
        (section(row(html! {
            (item("Plan", mode_badge(&AgentMode::Plan)))
            (item("Code", mode_badge(&AgentMode::Code)))
            (item("Ask", mode_badge(&AgentMode::Ask)))
        })))

        (section_title("Custom Modes"))
        (section(row(html! {
            (item("Research", mode_badge(&AgentMode::Custom("research".to_string()))))
            (item("Debug", mode_badge(&AgentMode::Custom("debug".to_string()))))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::acp::atoms::{mode_badge, AgentMode};

// Standard modes
mode_badge(&AgentMode::Plan)
mode_badge(&AgentMode::Code)
mode_badge(&AgentMode::Ask)

// Custom mode
mode_badge(&AgentMode::Custom("research".to_string()))"#))
    }
}
