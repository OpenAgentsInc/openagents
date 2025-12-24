//! Checkpoint restore story.

use maud::{Markup, html};
use ui::acp::molecules::{CheckpointRestore, RestoreState};

pub fn checkpoint_restore_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Checkpoint Restore" }
        p class="text-sm text-muted-foreground mb-6" {
            "Checkpoint restore control with badge and action buttons."
        }

        div class="space-y-4" {
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Ready" }
                (CheckpointRestore::new("abc1234", "entry-1").build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Confirming" }
                (CheckpointRestore::new("def5678", "entry-2")
                    .state(RestoreState::Confirming)
                    .build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Restoring" }
                (CheckpointRestore::new("ghi9012", "entry-3")
                    .state(RestoreState::Restoring)
                    .build())
            }
        }
    }
}
