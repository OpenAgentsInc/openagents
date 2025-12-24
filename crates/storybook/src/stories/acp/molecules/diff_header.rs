//! Diff header story.

use maud::{Markup, html};
use ui::acp::molecules::DiffHeader;

pub fn diff_header_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Diff Header" }
        p class="text-sm text-muted-foreground mb-6" {
            "Header for diff content showing file path and change stats."
        }

        div class="space-y-4" {
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Rust File" }
                (DiffHeader::new("src/main.rs")
                    .additions(15)
                    .deletions(3)
                    .build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "TypeScript File" }
                (DiffHeader::new("src/components/App.tsx")
                    .additions(42)
                    .deletions(18)
                    .build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Config File" }
                (DiffHeader::new("Cargo.toml")
                    .additions(2)
                    .deletions(0)
                    .build())
            }
        }
    }
}
