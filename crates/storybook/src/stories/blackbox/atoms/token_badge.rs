//! Token badge story.

use maud::{Markup, html};
use ui::blackbox::atoms::token_badge;

use super::shared::{code_block, item, row, section, section_title};

pub fn token_badge_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Token Badge" }
        p class="text-sm text-muted-foreground mb-6" { "Prompt/completion token counts with optional cached totals." }

        (section_title("Variants"))
        (section(row(html! {
            (item("Small", token_badge(520, 80, None)))
            (item("Large", token_badge(2400, 62, None)))
            (item("With cached", token_badge(12400, 890, Some(8000))))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::blackbox::atoms::token_badge;

token_badge(520, 80, None)
token_badge(2400, 62, None)
token_badge(12400, 890, Some(8000))"#))
    }
}
