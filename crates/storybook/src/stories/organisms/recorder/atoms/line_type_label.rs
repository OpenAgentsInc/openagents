//! Line type label story.

use maud::{Markup, html};
use ui::recorder::atoms::{LineType, StatusState, line_type_label, status_dot};

use super::shared::{code_block, item, row, section, section_title};

pub fn line_type_label_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Line Type Label" }
        p class="text-sm text-muted-foreground mb-6" { "Uppercase label for log line types." }

        (section_title("Variants"))
        (section(html! {
            (row(html! {
                (item("User", html! { (status_dot(StatusState::Success)) " " (line_type_label(LineType::User)) }))
                (item("Agent", html! { (status_dot(StatusState::Success)) " " (line_type_label(LineType::Agent)) }))
                (item("Tool", html! { (status_dot(StatusState::Running)) " " (line_type_label(LineType::Tool)) }))
                (item("Observation", html! { (status_dot(StatusState::Success)) " " (line_type_label(LineType::Observation)) }))
            }))
            (row(html! {
                (item("Plan", html! { (status_dot(StatusState::Pending)) " " (line_type_label(LineType::Plan)) }))
                (item("Mode", html! { (status_dot(StatusState::Success)) " " (line_type_label(LineType::Mode)) }))
                (item("Recall", html! { (status_dot(StatusState::Success)) " " (line_type_label(LineType::Recall)) }))
                (item("Subagent", html! { (status_dot(StatusState::Running)) " " (line_type_label(LineType::Subagent)) }))
            }))
            (row(html! {
                (item("MCP", html! { (status_dot(StatusState::Pending)) " " (line_type_label(LineType::Mcp)) }))
                (item("Question", html! { (status_dot(StatusState::Pending)) " " (line_type_label(LineType::Question)) }))
                (item("Comment", line_type_label(LineType::Comment)))
                (item("Lifecycle", line_type_label(LineType::Lifecycle)))
                (item("Phase", line_type_label(LineType::Phase)))
                (item("Skill", line_type_label(LineType::Skill)))
            }))
        }))

        (section_title("Usage"))
        (code_block(r#"use ui::recorder::atoms::{LineType, StatusState, line_type_label, status_dot};

line_type_label(LineType::User)
line_type_label(LineType::Tool)

html! {
    (status_dot(StatusState::Running))
    " "
    (line_type_label(LineType::Mcp))
}"#))
    }
}
