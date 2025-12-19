//! Timestamp badge story.

use maud::{Markup, html};
use ui::{timestamp_badge_elapsed, timestamp_badge_wall};

use super::shared::{code_block, item, row, section, section_title};

pub fn timestamp_badge_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Timestamp Badge" }
        p class="text-sm text-muted-foreground mb-6" { "Elapsed and wall clock time badges." }

        (section_title("Variants"))
        (section(row(html! {
            (item("Elapsed", timestamp_badge_elapsed(0, 15, 23)))
            (item("Elapsed (4h)", timestamp_badge_elapsed(4, 0, 0)))
            (item("Wall clock", timestamp_badge_wall("03:21:08Z")))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::{timestamp_badge_elapsed, timestamp_badge_wall};

timestamp_badge_elapsed(0, 15, 23)
timestamp_badge_wall("03:21:08Z")"#))
    }
}
