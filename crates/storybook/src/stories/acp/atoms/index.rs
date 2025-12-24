//! ACP atoms index story.

use maud::{Markup, html};
use ui::acp::atoms::*;

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

pub fn atoms_index_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "ACP Atoms"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Simple, single-purpose UI elements for ACP components."
        }

        // Tool Icons
        (section_title("Tool Icons"))
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

        // Tool Status Badges
        (section_title("Tool Status Badges"))
        (section(row(html! {
            (item("Running", tool_status_badge(&ToolStatus::Running)))
            (item("Success", tool_status_badge(&ToolStatus::Success)))
            (item("Error", tool_status_badge(&ToolStatus::Error("Failed to read file".to_string()))))
            (item("Waiting", tool_status_badge(&ToolStatus::WaitingForConfirmation)))
        })))

        // Permission Buttons
        (section_title("Permission Buttons"))
        (section(row(html! {
            (item("Allow Once", permission_button(PermissionKind::AllowOnce, None)))
            (item("Allow Always", permission_button(PermissionKind::AllowAlways, None)))
            (item("Reject Once", permission_button(PermissionKind::RejectOnce, None)))
            (item("Reject Always", permission_button(PermissionKind::RejectAlways, None)))
        })))

        // Mode Badges
        (section_title("Mode Badges"))
        (section(row(html! {
            (item("Plan", mode_badge(&AgentMode::Plan)))
            (item("Code", mode_badge(&AgentMode::Code)))
            (item("Ask", mode_badge(&AgentMode::Ask)))
            (item("Custom", mode_badge(&AgentMode::Custom("research".to_string()))))
        })))

        // Model Badges
        (section_title("Model Badges"))
        (section(row(html! {
            (item("Compact", model_badge("claude-sonnet-4-20250514", true)))
            (item("Full", model_badge("claude-sonnet-4-20250514", false)))
        })))

        // Entry Markers
        (section_title("Entry Markers"))
        (section(row(html! {
            (item("User", entry_marker(EntryKind::User)))
            (item("Assistant", entry_marker(EntryKind::Assistant)))
            (item("Tool", entry_marker(EntryKind::Tool)))
        })))

        // Other Atoms
        (section_title("Other Atoms"))
        (section(row(html! {
            (item("Thinking Toggle", thinking_toggle(ThinkingState::Collapsed, "demo")))
            (item("Checkpoint", checkpoint_badge("abc1234567890def")))
            (item("Thumbs Up", feedback_button(FeedbackKind::ThumbsUp, FeedbackState::Inactive)))
            (item("Thumbs Down", feedback_button(FeedbackKind::ThumbsDown, FeedbackState::Active)))
            (item("Keybinding", keybinding_hint("y", "allow")))
            (item("Streaming", streaming_indicator(None)))
        })))
    }
}
