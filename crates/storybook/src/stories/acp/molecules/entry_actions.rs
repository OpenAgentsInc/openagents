//! Entry actions story.

use maud::{Markup, html};
use ui::acp::molecules::EntryActions;

pub fn entry_actions_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Entry Actions" }
        p class="text-sm text-muted-foreground mb-6" {
            "Action buttons for thread entries (copy, regenerate, cancel)."
        }

        div class="space-y-4" {
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "User Actions (Copy + Regenerate)" }
                (EntryActions::for_user("entry-1").build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Assistant Actions (Copy only)" }
                (EntryActions::for_assistant("entry-2").build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Editing Actions (Cancel)" }
                (EntryActions::for_editing("entry-3").build())
            }
        }
    }
}
