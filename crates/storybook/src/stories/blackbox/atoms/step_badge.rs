//! Step badge story.

use maud::{Markup, html};
use ui::blackbox::atoms::step_badge;

use super::shared::{code_block, item, row, section, section_title};

pub fn step_badge_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Step Badge" }
        p class="text-sm text-muted-foreground mb-6" { "Compact step indicator for log ordering." }

        (section_title("Variants"))
        (section(row(html! {
            (item("Step 1", step_badge(1)))
            (item("Step 42", step_badge(42)))
            (item("Step 125", step_badge(125)))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::blackbox::atoms::step_badge;

step_badge(1)
step_badge(42)"#))
    }
}
