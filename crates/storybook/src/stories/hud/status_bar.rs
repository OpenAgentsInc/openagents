//! Status bar HUD story.

use maud::{Markup, html};

use super::shared::{code_block, section, section_title, story_header};

fn status_bar_preview(position: &str, left: Markup, center: Markup, right: Markup) -> Markup {
    let bar_class = if position == "top" {
        "absolute top-0 left-0 right-0"
    } else {
        "absolute bottom-0 left-0 right-0"
    };

    html! {
        div class="relative h-24 border border-border bg-secondary" {
            div class=(bar_class) {
                div class="grid grid-cols-3 items-center h-8 px-3 border border-border bg-card text-xs" {
                    div class="text-left" { (left) }
                    div class="text-center" { (center) }
                    div class="text-right" { (right) }
                }
            }
        }
    }
}

fn tag(label: &str) -> Markup {
    html! {
        span class="border border-border bg-card px-2 py-1 text-xs" { (label) }
    }
}

pub fn status_bar_story() -> Markup {
    html! {
        (story_header(
            "Status Bar",
            "Persistent bar with left, center, and right aligned items."
        ))

        (section_title("Top position"))
        (section(status_bar_preview(
            "top",
            html! { (tag("Mode: Plan")) (tag("Branch: main")) },
            html! { (tag("File: app.rs")) },
            html! { (tag("Model: Claude")) (tag("Status: Online")) }
        )))

        (section_title("Bottom position"))
        (section(status_bar_preview(
            "bottom",
            html! { (tag("Mode: Act")) (tag("Workspace: openagents")) },
            html! { (tag("Line: 142")) },
            html! { (tag("Status: Busy")) }
        )))

        (section_title("Alignment and content types"))
        (section(html! {
            div class="grid gap-4 md:grid-cols-2" {
                div class="border border-border bg-secondary p-4" {
                    div class="text-xs text-muted-foreground mb-2" { "Left aligned" }
                    div class="flex gap-2 flex-wrap" {
                        (tag("Text: Ready"))
                        (tag("Mode: Plan"))
                    }
                }
                div class="border border-border bg-secondary p-4" {
                    div class="text-xs text-muted-foreground mb-2" { "Center aligned" }
                    div class="flex gap-2 flex-wrap" {
                        (tag("Model: Claude"))
                        (tag("Custom: CPU 32%"))
                    }
                }
                div class="border border-border bg-secondary p-4" {
                    div class="text-xs text-muted-foreground mb-2" { "Right aligned" }
                    div class="flex gap-2 flex-wrap" {
                        (tag("Status: Online"))
                        (tag("Latency: 120ms"))
                    }
                }
                div class="border border-border bg-secondary p-4" {
                    div class="text-xs text-muted-foreground mb-2" { "Mixed" }
                    div class="flex gap-2 flex-wrap" {
                        (tag("Text: main.rs"))
                        (tag("Mode: Act"))
                        (tag("Model: Local"))
                    }
                }
            }
        }))

        (section_title("Usage"))
        (code_block(r#"use wgpui::components::hud::{StatusBar, StatusBarPosition, StatusItem};
use wgpui::components::atoms::{Mode, Model, Status};

let bar = StatusBar::new()
    .position(StatusBarPosition::Bottom)
    .items(vec![
        StatusItem::mode("mode", Mode::Plan).left(),
        StatusItem::text("file", "main.rs").center(),
        StatusItem::model("model", Model::Claude).right(),
        StatusItem::status("status", Status::Online).right(),
    ]);
"#))
    }
}
