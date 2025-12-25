//! Tooltip HUD story.

use maud::{Markup, html};

use super::shared::{code_block, section, section_title, story_header};

fn tooltip_cell(label: &str, tooltip_class: &str, arrow_class: &str, text: &str) -> Markup {
    let tooltip_class = format!("absolute {} border border-border bg-card px-2 py-1 text-xs", tooltip_class);
    let arrow_class = format!("absolute {} w-2 h-2 border border-border bg-card", arrow_class);
    html! {
        div class="flex flex-col gap-2" {
            span class="text-xs text-muted-foreground" { (label) }
            div class="relative h-28 border border-border bg-secondary" {
                div class="absolute left-1/2 top-1/2 w-6 h-6 -translate-x-1/2 -translate-y-1/2 border border-border bg-card" {}
                div class=(tooltip_class) { (text) }
                div class=(arrow_class) {}
            }
        }
    }
}

pub fn tooltip_story() -> Markup {
    html! {
        (story_header(
            "Tooltip",
            "Hover overlays with positional variants and width constraints."
        ))

        (section_title("Positions"))
        (section(html! {
            div class="grid gap-4 md:grid-cols-2" {
                (tooltip_cell(
                    "Top",
                    "left-1/2 -translate-x-1/2 top-3",
                    "left-1/2 -translate-x-1/2 top-8",
                    "Top tooltip"
                ))
                (tooltip_cell(
                    "Bottom",
                    "left-1/2 -translate-x-1/2 bottom-3",
                    "left-1/2 -translate-x-1/2 bottom-8",
                    "Bottom tooltip"
                ))
                (tooltip_cell(
                    "Left",
                    "left-3 top-1/2 -translate-y-1/2",
                    "left-8 top-1/2 -translate-y-1/2",
                    "Left tooltip"
                ))
                (tooltip_cell(
                    "Right",
                    "right-3 top-1/2 -translate-y-1/2",
                    "right-8 top-1/2 -translate-y-1/2",
                    "Right tooltip"
                ))
            }
        }))

        (section_title("Auto selection"))
        (section(html! {
            div class="grid gap-4 md:grid-cols-2" {
                (tooltip_cell(
                    "Auto prefers top",
                    "left-1/2 -translate-x-1/2 top-3",
                    "left-1/2 -translate-x-1/2 top-8",
                    "Auto top"
                ))
                (tooltip_cell(
                    "Auto fallback bottom",
                    "left-1/2 -translate-x-1/2 bottom-3",
                    "left-1/2 -translate-x-1/2 bottom-8",
                    "Auto bottom"
                ))
            }
        }))

        (section_title("Max width and wrap"))
        (section(html! {
            div class="border border-border bg-secondary p-4" {
                div class="relative h-24" {
                    div class="absolute left-1/2 top-1/2 w-6 h-6 -translate-x-1/2 -translate-y-1/2 border border-border bg-card" {}
                    div class="absolute left-1/2 -translate-x-1/2 top-3 border border-border bg-card px-2 py-1 text-xs max-w-xs" {
                        "This tooltip uses a narrow width and wraps to multiple lines for long content."
                    }
                    div class="absolute left-1/2 -translate-x-1/2 top-8 w-2 h-2 border border-border bg-card" {}
                }
            }
        }))

        (section_title("Delay state"))
        (section(html! {
            div class="border border-border bg-secondary p-4" {
                div class="text-xs text-muted-foreground" { "Delay: waiting for hover threshold" }
                div class="mt-2 text-xs text-muted-foreground" { "Visible after N frames" }
            }
        }))

        (section_title("Usage"))
        (code_block(r#"use wgpui::components::hud::{Tooltip, TooltipPosition};
use wgpui::Bounds;

let tooltip = Tooltip::new("Helpful hint")
    .position(TooltipPosition::Top)
    .target(Bounds::new(120.0, 80.0, 50.0, 20.0))
    .delay(20)
    .max_width(240.0);
"#))
    }
}
