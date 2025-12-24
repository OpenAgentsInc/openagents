//! Permission bar story.

use maud::{Markup, html};
use ui::acp::molecules::PermissionBar;

pub fn permission_bar_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Permission Bar" }
        p class="text-sm text-muted-foreground mb-6" {
            "Bar with permission action buttons and keybinding hints."
        }

        div class="space-y-4" {
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Default" }
                (PermissionBar::new().build())
            }
        }
    }
}
