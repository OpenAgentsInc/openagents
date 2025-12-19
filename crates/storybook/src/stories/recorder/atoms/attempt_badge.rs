//! Attempt badge story.

use maud::{Markup, html};
use ui::recorder::atoms::attempt_badge;

use super::shared::{code_block, item, row, section, section_title};

pub fn attempt_badge_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Attempt Badge" }
        p class="text-sm text-muted-foreground mb-6" { "Retry count for attempts." }

        (section_title("Variants"))
        (section(row(html! {
            (item("Retry 2/3", attempt_badge(2, 3)))
            (item("Retry 3/3", attempt_badge(3, 3)))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::recorder::atoms::attempt_badge;

attempt_badge(2, 3)
attempt_badge(3, 3)"#))
    }
}
