use maud::{Markup, html};

use super::super::atoms::cost_badge;

pub fn cost_accumulator(total: f64, delta: Option<f64>) -> Markup {
    html! {
        div class="inline-flex items-center gap-2 text-xs" {
            span class="text-muted-foreground" { "Session cost:" }
            (cost_badge(total))
            @if let Some(d) = delta {
                span class="text-green text-xs" {
                    "\u{2191}$" (format!("{:.4}", d))
                }
            }
        }
    }
}
