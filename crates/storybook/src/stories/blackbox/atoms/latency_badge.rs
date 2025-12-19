//! Latency badge story.

use maud::{Markup, html};
use ui::blackbox::atoms::latency_badge;

use super::shared::{code_block, item, row, section, section_title};

pub fn latency_badge_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Latency Badge" }
        p class="text-sm text-muted-foreground mb-6" { "Latency indicator with threshold-based coloring." }

        (section_title("Variants"))
        (section(row(html! {
            (item("Fast (<1s)", latency_badge(340)))
            (item("Medium (1-5s)", latency_badge(2500)))
            (item("Slow (>5s)", latency_badge(8400)))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::blackbox::atoms::latency_badge;

latency_badge(340)
latency_badge(2500)
latency_badge(8400)"#))
    }
}
