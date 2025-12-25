//! Notifications HUD story.

use maud::{Markup, html};

use super::shared::{code_block, section, section_title, story_header};

fn notification_card(
    level: &str,
    accent_class: &str,
    title: &str,
    message: Option<&str>,
    dismissible: bool,
    progress: Option<u8>,
) -> Markup {
    let card_class = format!("border border-border {} bg-card p-3", accent_class);
    html! {
        div class=(card_class) {
            div class="flex items-start justify-between gap-4" {
                div {
                    div class="text-sm text-foreground" { (title) }
                    @if let Some(text) = message {
                        div class="text-xs text-muted-foreground mt-1" { (text) }
                    }
                    div class="text-xs text-muted-foreground mt-2" { (level) }
                }
                @if dismissible {
                    div class="text-xs text-muted-foreground" { "x" }
                }
            }
            @if let Some(percent) = progress {
                div class="mt-2 h-1 bg-secondary" {
                    div class="h-1 bg-muted" style={"width: " (percent) "%"} {}
                }
            }
        }
    }
}

fn position_preview(label: &str, pos_class: &str) -> Markup {
    let container_class = format!("absolute {}", pos_class);
    html! {
        div class="flex flex-col gap-2" {
            span class="text-xs text-muted-foreground" { (label) }
            div class="relative h-24 border border-border bg-secondary" {
                div class=(container_class) {
                    div class="border border-border bg-card px-2 py-1 text-xs text-muted-foreground" {
                        "Toast"
                    }
                }
            }
        }
    }
}

pub fn notifications_story() -> Markup {
    html! {
        (story_header(
            "Notifications",
            "Toast system with levels, positions, and dismiss behavior."
        ))

        (section_title("Levels"))
        (section(html! {
            div class="grid gap-4 md:grid-cols-2" {
                (notification_card(
                    "Info",
                    "border-l-2 border-cyan-400",
                    "Sync started",
                    Some("Fetching latest state"),
                    true,
                    Some(70)
                ))
                (notification_card(
                    "Success",
                    "border-l-2 border-green-400",
                    "Saved",
                    Some("Changes persisted"),
                    true,
                    Some(100)
                ))
                (notification_card(
                    "Warning",
                    "border-l-2 border-yellow-400",
                    "Low balance",
                    Some("Add funds to continue"),
                    false,
                    None
                ))
                (notification_card(
                    "Error",
                    "border-l-2 border-red-400",
                    "Connection failed",
                    Some("Retry in a moment"),
                    true,
                    None
                ))
            }
        }))

        (section_title("Positions"))
        (section(html! {
            div class="grid gap-4 md:grid-cols-3" {
                (position_preview("Top left", "top-2 left-2"))
                (position_preview("Top center", "top-2 left-1/2 -translate-x-1/2"))
                (position_preview("Top right", "top-2 right-2"))
                (position_preview("Bottom left", "bottom-2 left-2"))
                (position_preview("Bottom center", "bottom-2 left-1/2 -translate-x-1/2"))
                (position_preview("Bottom right", "bottom-2 right-2"))
            }
        }))

        (section_title("Stacking and max visible"))
        (section(html! {
            div class="border border-border bg-secondary p-4" {
                div class="flex flex-col gap-2 max-w-sm" {
                    (notification_card(
                        "Info",
                        "border-l-2 border-cyan-400",
                        "Queued",
                        Some("Job added"),
                        true,
                        Some(60)
                    ))
                    (notification_card(
                        "Success",
                        "border-l-2 border-green-400",
                        "Completed",
                        Some("Task finished"),
                        true,
                        Some(100)
                    ))
                    (notification_card(
                        "Warning",
                        "border-l-2 border-yellow-400",
                        "Throttle",
                        Some("Rate limit near"),
                        false,
                        None
                    ))
                    div class="text-xs text-muted-foreground border border-border bg-card px-3 py-2" {
                        "Hidden: 1 more notification"
                    }
                }
            }
        }))

        (section_title("Usage"))
        (code_block(r#"use wgpui::components::hud::{Notification, NotificationLevel, Notifications};
use std::time::Duration;

let mut notifs = Notifications::new();
notifs.push(
    Notification::new(1, "Sync started")
        .message("Fetching latest state")
        .level(NotificationLevel::Info)
        .duration(Duration::from_secs(5))
);
"#))
    }
}
