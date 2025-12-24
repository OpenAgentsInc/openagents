//! ACP molecules index story.

use maud::{Markup, html};
use ui::acp::atoms::{ToolKind, ToolStatus, AgentMode, EntryKind};
use ui::acp::molecules::*;

fn section_title(title: &str) -> Markup {
    html! { h2 class="text-base font-medium text-muted-foreground mt-8 mb-4" { (title) } }
}

fn section(content: Markup) -> Markup {
    html! { div class="p-4 border border-border bg-card mb-4" { (content) } }
}

pub fn molecules_index_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "ACP Molecules"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Compositions of atoms for ACP components."
        }

        // Tool Header
        (section_title("Tool Header"))
        (section(html! {
            div class="space-y-2" {
                (ToolHeader::new(ToolKind::Execute, "Running command").status(ToolStatus::Running).build())
                (ToolHeader::new(ToolKind::Edit, "Editing file.rs").status(ToolStatus::Success).build())
                (ToolHeader::new(ToolKind::Read, "Reading config").status(ToolStatus::Error("Not found".to_string())).build())
            }
        }))

        // Permission Bar
        (section_title("Permission Bar"))
        (section(html! {
            div class="max-w-md" {
                (PermissionBar::new().build())
            }
        }))

        // Mode Selector
        (section_title("Mode Selector"))
        (section(html! {
            div class="flex gap-4" {
                (ModeSelector::new(AgentMode::Code, "session-1").build())
                (ModeSelector::new(AgentMode::Plan, "session-2").build())
            }
        }))

        // Model Selector
        (section_title("Model Selector"))
        (section(html! {
            (ModelSelector::new("claude-sonnet-4-20250514", "session-1").build())
        }))

        // Message Header
        (section_title("Message Header"))
        (section(html! {
            div class="space-y-4" {
                (MessageHeader::new(EntryKind::User, "entry-1").timestamp("10:30 AM").editable().build())
                (MessageHeader::new(EntryKind::Assistant, "entry-2").timestamp("10:30 AM").build())
            }
        }))

        // Thinking Block
        (section_title("Thinking Block"))
        (section(html! {
            (ThinkingBlock::new("I need to consider the best approach here. Let me analyze the requirements...", "thinking-1").build())
        }))

        // Diff Header
        (section_title("Diff Header"))
        (section(html! {
            (DiffHeader::new("src/lib.rs").additions(15).deletions(3).build())
        }))

        // Terminal Header
        (section_title("Terminal Header"))
        (section(html! {
            div class="space-y-2" {
                (TerminalHeader::new("cargo build --release").working_dir("/home/user/project").exit_status(ExitStatus::Success).build())
                (TerminalHeader::new("npm install").exit_status(ExitStatus::Failed(1)).build())
            }
        }))

        // Checkpoint Restore
        (section_title("Checkpoint Restore"))
        (section(html! {
            div class="flex gap-4" {
                (CheckpointRestore::new("abc1234", "entry-1").build())
                (CheckpointRestore::new("def5678", "entry-2").state(RestoreState::Confirming).build())
            }
        }))

        // Entry Actions
        (section_title("Entry Actions"))
        (section(html! {
            div class="flex gap-6" {
                div {
                    p class="text-xs text-muted-foreground mb-2" { "User actions" }
                    (EntryActions::for_user("entry-1").build())
                }
                div {
                    p class="text-xs text-muted-foreground mb-2" { "Assistant actions" }
                    (EntryActions::for_assistant("entry-2").build())
                }
            }
        }))
    }
}
