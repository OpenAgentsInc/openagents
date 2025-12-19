use maud::{Markup, html};

use super::{SessionHeader, SessionStats, ToolIndex};
use super::super::molecules::{PlanPhase, SessionMode, budget_meter, cost_accumulator, mode_indicator, phase_indicator};

#[allow(clippy::too_many_arguments)]
pub fn session_sidebar(
    header: SessionHeader,
    mode: SessionMode,
    phase: Option<PlanPhase>,
    budget_spent: f64,
    budget_total: f64,
    cost_total: f64,
    cost_delta: Option<f64>,
    stats: SessionStats,
    tool_index: ToolIndex,
) -> Markup {
    html! {
        div class="flex flex-col gap-4 w-72 p-4 bg-background border-r border-border h-full overflow-y-auto" {
            (header.build())

            div class="flex flex-col gap-2" {
                (mode_indicator(mode))
                @if let Some(p) = phase {
                    (phase_indicator(p))
                }
            }

            div class="bg-card border border-border p-3" {
                div class="mb-2" {
                    (budget_meter(budget_spent, budget_total))
                }
                (cost_accumulator(cost_total, cost_delta))
            }

            (stats.build())

            (tool_index.build())
        }
    }
}
