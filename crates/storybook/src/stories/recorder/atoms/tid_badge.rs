//! TID badge story.

use maud::{Markup, html};
use ui::recorder::atoms::tid_badge;

use super::shared::{code_block, item, row, section, section_title};

pub fn tid_badge_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "TID Badge" }
        p class="text-sm text-muted-foreground mb-6" { "Thread ID badge with color mapping." }

        (section_title("Variants"))
        (section(row(html! {
            (item("Main (tid:1)", tid_badge(1)))
            (item("Thread 2", tid_badge(2)))
            (item("Thread 3", tid_badge(3)))
            (item("Thread 4", tid_badge(4)))
            (item("Thread 5", tid_badge(5)))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::recorder::atoms::tid_badge;

tid_badge(1)
tid_badge(2)
tid_badge(3)"#))
    }
}
