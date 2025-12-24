//! Tool header story.

use maud::{Markup, html};
use ui::acp::atoms::{ToolKind, ToolStatus};
use ui::acp::molecules::ToolHeader;

pub fn tool_header_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Tool Header" }
        p class="text-sm text-muted-foreground mb-6" {
            "Header component for tool calls showing icon, label, and status."
        }

        div class="space-y-4" {
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Read Tool - Running" }
                (ToolHeader::new(ToolKind::Read, "Read src/main.rs")
                    .status(ToolStatus::Running)
                    .build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Edit Tool - Success" }
                (ToolHeader::new(ToolKind::Edit, "Edit src/lib.rs")
                    .status(ToolStatus::Success)
                    .build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Execute Tool - Error" }
                (ToolHeader::new(ToolKind::Execute, "cargo build")
                    .status(ToolStatus::Error("exit 1".to_string()))
                    .build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Search Tool - Waiting" }
                (ToolHeader::new(ToolKind::Search, "Search for 'config'")
                    .status(ToolStatus::WaitingForConfirmation)
                    .build())
            }
        }
    }
}
