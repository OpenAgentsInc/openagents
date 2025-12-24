//! Thinking block story.

use maud::{Markup, html};
use ui::acp::molecules::ThinkingBlock;

pub fn thinking_block_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Thinking Block" }
        p class="text-sm text-muted-foreground mb-6" {
            "Collapsible block for displaying agent thinking content."
        }

        div class="space-y-4" {
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Collapsed" }
                (ThinkingBlock::new("Let me analyze this code to understand the architecture...", "entry-1")
                    .build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Expanded" }
                (ThinkingBlock::new("Let me analyze this code to understand the architecture. I'll look at the imports first, then trace through the main function to see how the components interact.", "entry-2")
                    .expanded()
                    .build())
            }
        }
    }
}
