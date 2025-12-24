//! Model selector story.

use maud::{Markup, html};
use ui::acp::molecules::ModelSelector;

pub fn model_selector_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Model Selector" }
        p class="text-sm text-muted-foreground mb-6" {
            "Dropdown for selecting the AI model."
        }

        div class="space-y-4" {
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Sonnet 4" }
                (ModelSelector::new("claude-sonnet-4-20250514", "session-1").build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Opus 4" }
                (ModelSelector::new("claude-opus-4-20250514", "session-2").build())
            }
        }
    }
}
