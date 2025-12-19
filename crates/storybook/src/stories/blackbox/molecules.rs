//! BlackBox molecule components story.

use maud::{Markup, html};
use ui::blackbox::atoms::{CallType, LineType, StatusState};
use ui::blackbox::molecules::{
    LineHeader, LineMeta, PlanPhase, ResultType, SessionMode, budget_meter, cost_accumulator,
    metrics_footer, mode_indicator, phase_indicator, result_display,
};

use super::shared::{code_block, item, row, section, section_title};

pub fn blackbox_molecules_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "BlackBox Molecules" }
        p class="text-sm text-muted-foreground mb-6" {
            "Composed components that combine atoms into functional units."
        }

        (section_title("LineMeta"))
        (section(row(html! {
            (item("Full", LineMeta::new()
                .step(42)
                .elapsed(0, 15, 23)
                .call_id("call_47", CallType::Tool)
                .latency(340)
                .build()))
            (item("Step + Time", LineMeta::new()
                .step(8)
                .elapsed(0, 1, 0)
                .build()))
            (item("With MCP call", LineMeta::new()
                .step(13)
                .call_id("call_4", CallType::Mcp)
                .build()))
        })))

        (section_title("LineHeader"))
        (section(html! {
            div class="flex flex-col gap-2" {
                div class="bg-card border border-border px-2" {
                    (LineHeader::new(StatusState::Success, LineType::Tool)
                        .name("read")
                        .meta(LineMeta::new()
                            .step(42)
                            .elapsed(0, 15, 23)
                            .call_id("call_42", CallType::Tool)
                            .latency(340))
                        .build())
                }
                div class="bg-card border border-border px-2" {
                    (LineHeader::new(StatusState::Success, LineType::Agent)
                        .meta(LineMeta::new().step(46).elapsed(0, 9, 18))
                        .build())
                }
                div class="bg-card border border-border px-2" {
                    (LineHeader::new(StatusState::Running, LineType::Subagent)
                        .name("explore")
                        .meta(LineMeta::new().step(24).elapsed(0, 4, 45).call_id("sub_1", CallType::Subagent))
                        .build())
                }
                div class="bg-card border border-border px-2" {
                    (LineHeader::new(StatusState::Pending, LineType::Mcp)
                        .name("github.issues")
                        .meta(LineMeta::new().step(13).call_id("call_4", CallType::Mcp))
                        .build())
                }
            }
        }))

        (section_title("ResultDisplay"))
        (section(html! {
            (row(html! {
                (item("Ok", result_display(ResultType::Ok)))
                (item("Lines count", result_display(ResultType::Count { count: 186, unit: "lines".to_string() })))
                (item("Files count", result_display(ResultType::Count { count: 8, unit: "files".to_string() })))
            }))
            (row(html! {
                (item("Error", result_display(ResultType::Error("permission denied".to_string()))))
                (item("Pending", result_display(ResultType::Pending)))
                (item("Blob", result_display(ResultType::Blob {
                    sha256: "a1b2c3d4".to_string(),
                    bytes: 12847,
                    mime: Some("text/markdown".to_string()),
                })))
            }))
        }))

        (section_title("ModeIndicator"))
        (section(row(html! {
            (item("Auto", mode_indicator(SessionMode::Auto)))
            (item("Plan", mode_indicator(SessionMode::Plan)))
            (item("Chat", mode_indicator(SessionMode::Chat)))
        })))

        (section_title("PhaseIndicator"))
        (section(html! {
            div class="flex flex-col gap-4" {
                (item("At Explore phase", phase_indicator(PlanPhase::Explore)))
                (item("At Design phase", phase_indicator(PlanPhase::Design)))
                (item("At Review phase", phase_indicator(PlanPhase::Review)))
                (item("At Final phase", phase_indicator(PlanPhase::Final)))
                (item("At Exit phase", phase_indicator(PlanPhase::Exit)))
            }
        }))

        (section_title("BudgetMeter"))
        (section(html! {
            div class="flex flex-col gap-3" {
                (item("Low usage (20%)", budget_meter(10.0, 50.0)))
                (item("Medium usage (60%)", budget_meter(30.0, 50.0)))
                (item("High usage (85%)", budget_meter(42.5, 50.0)))
            }
        }))

        (section_title("CostAccumulator"))
        (section(row(html! {
            (item("Without delta", cost_accumulator(12.47, None)))
            (item("With delta", cost_accumulator(12.49, Some(0.02))))
            (item("Large cost", cost_accumulator(42.17, Some(0.15))))
        })))

        (section_title("MetricsFooter"))
        (section(html! {
            div class="flex flex-col gap-3" {
                (item("Small response", metrics_footer(520, 80, None, 0.00045)))
                (item("Large response with cache", metrics_footer(12400, 890, Some(8000), 0.0234)))
            }
        }))

        (section_title("Usage"))
        (code_block(r#"use ui::blackbox::atoms::{CallType, LineType, StatusState};
use ui::blackbox::molecules::{
    LineHeader, LineMeta, ResultType, SessionMode, budget_meter, cost_accumulator,
    metrics_footer, mode_indicator, result_display,
};

LineMeta::new()
    .step(42)
    .elapsed(0, 15, 23)
    .call_id("call_47", CallType::Tool)
    .latency(340)
    .build();

LineHeader::new(StatusState::Success, LineType::Tool)
    .name("read")
    .meta(LineMeta::new().step(42))
    .build();

result_display(ResultType::Ok);
mode_indicator(SessionMode::Auto);
budget_meter(12.30, 50.0);
cost_accumulator(12.47, Some(0.02));
metrics_footer(2400, 62, Some(1800), 0.0018);"#))
    }
}
