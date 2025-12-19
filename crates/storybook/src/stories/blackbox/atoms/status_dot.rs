//! Status dot story.

use maud::{Markup, html};
use ui::blackbox::atoms::{StatusState, status_dot};

use super::shared::{code_block, item, row, section, section_title};

pub fn status_dot_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Status Dot" }
        p class="text-sm text-muted-foreground mb-6" { "Colored dot indicator for status states." }

        (section_title("Variants"))
        (section(row(html! {
            (item("Success", status_dot(StatusState::Success)))
            (item("Running", status_dot(StatusState::Running)))
            (item("Pending", status_dot(StatusState::Pending)))
            (item("Error", status_dot(StatusState::Error)))
            (item("Skipped", status_dot(StatusState::Skipped)))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::blackbox::atoms::{StatusState, status_dot};

status_dot(StatusState::Success)
status_dot(StatusState::Running)
status_dot(StatusState::Pending)
status_dot(StatusState::Error)
status_dot(StatusState::Skipped)"#))
    }
}
