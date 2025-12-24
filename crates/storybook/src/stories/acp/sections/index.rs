//! ACP sections index story.

use maud::{Markup, html};
use ui::acp::atoms::AgentMode;
use ui::acp::sections::*;

fn section_title(title: &str) -> Markup {
    html! { h2 class="text-base font-medium text-muted-foreground mt-8 mb-4" { (title) } }
}

fn section(content: Markup) -> Markup {
    html! { div class="p-4 border border-border bg-card mb-4" { (content) } }
}

pub fn sections_index_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "ACP Sections"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Page-level layout components for ACP thread views."
        }

        // Thread Header
        (section_title("Thread Header"))
        (section(html! {
            (ThreadHeader::new("session-abc123")
                .mode(AgentMode::Code)
                .model("claude-sonnet-4-20250514")
                .connection_status(ConnectionStatus::Connected)
                .build())
        }))

        (section(html! {
            (ThreadHeader::new("session-def456")
                .mode(AgentMode::Plan)
                .model("claude-opus-4")
                .connection_status(ConnectionStatus::Connecting)
                .build())
        }))

        // Thread Feedback
        (section_title("Thread Feedback"))
        (section(html! {
            (ThreadFeedback::new("session-1").build())
        }))

        // Message Editor
        (section_title("Message Editor"))
        (section(html! {
            (MessageEditor::new("session-1")
                .placeholder("Ask Claude anything...")
                .build())
        }))

        (section(html! {
            (MessageEditor::new("session-2")
                .disabled()
                .placeholder("Waiting for response...")
                .build())
        }))

        // Note about ThreadView
        (section_title("Thread View"))
        p class="text-sm text-muted-foreground" {
            "The full ThreadView component combines all sections. See the "
            a href="/stories/acp/demo" class="text-cyan hover:underline" { "Demo" }
            " for a complete example."
        }
    }
}
