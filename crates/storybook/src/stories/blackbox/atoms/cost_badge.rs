//! Cost badge story.

use maud::{Markup, html};
use ui::blackbox::atoms::cost_badge;

use super::shared::{code_block, item, row, section, section_title};

pub fn cost_badge_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Cost Badge" }
        p class="text-sm text-muted-foreground mb-6" { "Cost indicator with threshold-based coloring." }

        (section_title("Variants"))
        (section(row(html! {
            (item("Low (<$0.01)", cost_badge(0.0018)))
            (item("Medium ($0.01-0.10)", cost_badge(0.0456)))
            (item("High (>$0.10)", cost_badge(0.2345)))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::blackbox::atoms::cost_badge;

cost_badge(0.0018)
cost_badge(0.0456)
cost_badge(0.2345)"#))
    }
}
