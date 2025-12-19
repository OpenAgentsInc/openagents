use maud::{Markup, html};

use super::super::molecules::{LineMeta, budget_meter};

pub enum LifecycleEvent {
    Start {
        id: String,
        budget: f64,
        duration: String,
    },
    Checkpoint {
        hour: u8,
        tokens: u64,
        cost: f64,
        budget_total: f64,
    },
    Pause {
        reason: String,
    },
    Resume,
    End {
        summary: String,
        issues_completed: u32,
        prs_merged: u32,
        cost: f64,
        duration: String,
    },
}

pub fn lifecycle_line(
    event: LifecycleEvent,
    step: Option<u32>,
    elapsed: Option<(u8, u8, u8)>,
) -> Markup {
    let mut meta = LineMeta::new();
    if let Some(s) = step {
        meta = meta.step(s);
    }
    if let Some((h, m, s)) = elapsed {
        meta = meta.elapsed(h, m, s);
    }

    let (event_name, color, content) = match &event {
        LifecycleEvent::Start {
            id,
            budget,
            duration,
        } => (
            "START",
            "text-green",
            html! {
                div class="pl-6 text-xs text-muted-foreground" {
                    "id=" (id) " budget=$" (format!("{:.0}", budget)) " duration=" (duration)
                }
            },
        ),
        LifecycleEvent::Checkpoint {
            hour,
            tokens,
            cost,
            budget_total,
        } => (
            "CHECKPOINT",
            "text-blue",
            html! {
                div class="pl-6 text-xs text-muted-foreground" {
                    "hour=" (*hour) " tokens=" (tokens) " cost=$" (format!("{:.2}", cost))
                }
                div class="px-3 py-3 border-t border-border mt-2" {
                    div class="text-xs text-muted-foreground mb-2" {
                        "Progress: " (*hour) "/12 hours"
                    }
                    (budget_meter(*cost, *budget_total))
                }
            },
        ),
        LifecycleEvent::Pause { reason } => (
            "PAUSE",
            "text-yellow",
            html! {
                div class="pl-6 text-xs text-muted-foreground" {
                    "reason=\"" (reason) "\""
                }
            },
        ),
        LifecycleEvent::Resume => ("RESUME", "text-green", html! {}),
        LifecycleEvent::End {
            summary,
            issues_completed,
            prs_merged,
            cost,
            duration,
        } => (
            "END",
            "text-green",
            html! {
                div class="pl-6 text-xs text-muted-foreground" {
                    "summary=\"" (summary) "\""
                }
                div class="px-3 py-3 border-t border-border mt-2" {
                    div class="grid grid-cols-[auto,1fr] gap-x-4 gap-y-1 text-xs" {
                        span class="text-muted-foreground" { "duration:" }
                        span class="text-muted-foreground" { (duration) }
                        span class="text-muted-foreground" { "issues_completed:" }
                        span class="text-muted-foreground" { (issues_completed) }
                        span class="text-muted-foreground" { "prs_merged:" }
                        span class="text-muted-foreground" { (prs_merged) }
                        span class="text-muted-foreground" { "cost:" }
                        span class="text-green" { "$" (format!("{:.2}", cost)) }
                    }
                }
            },
        ),
    };

    html! {
        div class={ "bg-card border border-border border-l-2 " (color) " mb-2" } {
            div class="flex items-center gap-2 px-3 py-2" {
                span class="text-xs text-muted-foreground" { "@" }
                span class={ "text-xs font-semibold " (color) } { (event_name) }
                span class="flex-1" {}
                (meta.build())
            }
            (content)
        }
    }
}
