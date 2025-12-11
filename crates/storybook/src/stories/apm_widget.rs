//! APM Widget Story
//!
//! Showcases the Actions Per Minute widget with different states and levels.

use crate::story::Story;
use gpui_oa::*;
use gpui_oa::prelude::FluentBuilder;
use ::hud::{ApmComparison, ApmLevel, ApmSnapshot, ApmState};
use theme_oa::hud;

pub struct ApmWidgetStory;

impl Render for ApmWidgetStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Story::container()
            .child(Story::title("APM Widget"))
            .child(Story::description(
                "Actions Per Minute display for agent velocity tracking. Inspired by StarCraft APM meters.",
            ))
            // Section 1: APM Levels
            .child(render_apm_levels_section())
            // Section 2: APM States
            .child(render_apm_states_section())
            // Section 3: Full Widget States
            .child(render_widget_states_section())
    }
}

// ============================================================================
// Section 1: APM Levels
// ============================================================================

fn render_apm_levels_section() -> impl IntoElement {
    Story::section()
        .child(Story::section_title("APM Levels"))
        .child(Story::description("Color-coded velocity indicators"))
        .child(
            Story::row()
                .child(render_level_badge(ApmLevel::Baseline, 2.5))
                .child(render_level_badge(ApmLevel::Active, 10.0))
                .child(render_level_badge(ApmLevel::High, 22.0))
                .child(render_level_badge(ApmLevel::Elite, 45.0)),
        )
        .child(
            Story::column()
                .mt(px(16.0))
                .gap(px(4.0))
                .child(div().text_xs().text_color(theme_oa::text::MUTED).child("Baseline: 0-5 APM (Idle)"))
                .child(div().text_xs().text_color(theme_oa::text::MUTED).child("Active: 5-15 APM (Working)"))
                .child(div().text_xs().text_color(theme_oa::text::MUTED).child("High: 15-30 APM (Fast)"))
                .child(div().text_xs().text_color(theme_oa::text::MUTED).child("Elite: 30+ APM (Pro-level)")),
        )
}

fn render_level_badge(level: ApmLevel, apm: f64) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(4.0))
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(theme_oa::bg::SURFACE)
        .border_1()
        .border_color(theme_oa::border::DEFAULT)
        .child(
            div()
                .text_size(px(20.0))
                .font_weight(FontWeight::BOLD)
                .text_color(level.color())
                .child(format!("{:.1}", apm)),
        )
        .child(
            div()
                .text_xs()
                .text_color(level.color())
                .child(level.label()),
        )
}

// ============================================================================
// Section 2: APM States
// ============================================================================

fn render_apm_states_section() -> impl IntoElement {
    Story::section()
        .child(Story::section_title("APM State Data"))
        .child(Story::description("Different data configurations"))
        .child(
            Story::column()
                .gap(px(12.0))
                .child(Story::item("Minimal State").child(render_state_card(&minimal_state())))
                .child(Story::item("With Snapshot").child(render_state_card(&state_with_snapshot())))
                .child(Story::item("With Comparison").child(render_state_card(&state_with_comparison())))
                .child(Story::item("Full State").child(render_state_card(&full_state()))),
        )
}

fn minimal_state() -> ApmState {
    let mut state = ApmState::new();
    state.update_from_message(8.5, 12.0, 45, 5.3);
    state
}

fn state_with_snapshot() -> ApmState {
    let mut state = minimal_state();
    state.update_snapshot(ApmSnapshot {
        apm_1h: 15.2,
        apm_6h: 12.8,
        apm_24h: 10.5,
        total_sessions: 42,
        total_actions: 12500,
    });
    state
}

fn state_with_comparison() -> ApmState {
    let mut state = minimal_state();
    state.update_comparison(ApmComparison {
        claude_code_apm: 8.2,
        mecha_coder_apm: 15.5,
        efficiency_ratio: 1.89,
    });
    state
}

fn full_state() -> ApmState {
    let mut state = ApmState::new();
    state.update_from_message(28.5, 32.0, 250, 8.8);
    state.update_snapshot(ApmSnapshot {
        apm_1h: 25.2,
        apm_6h: 18.8,
        apm_24h: 15.5,
        total_sessions: 156,
        total_actions: 45000,
    });
    state.update_comparison(ApmComparison {
        claude_code_apm: 12.0,
        mecha_coder_apm: 28.5,
        efficiency_ratio: 2.38,
    });
    state
}

fn render_state_card(state: &ApmState) -> impl IntoElement {
    let level = state.level();

    div()
        .w(px(280.0))
        .bg(hud::APM_WIDGET_BG)
        .border_1()
        .border_color(hud::APM_WIDGET_BORDER)
        .rounded_md()
        .p(px(12.0))
        .child(
            // Main APM display
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_color(level.color())
                        .text_size(px(24.0))
                        .font_weight(FontWeight::BOLD)
                        .child(format!("APM: {:.1}", state.session_apm)),
                )
                .child(
                    div()
                        .text_color(level.color())
                        .text_size(px(12.0))
                        .child(level.label()),
                ),
        )
        .child(
            // Session stats
            div()
                .flex()
                .gap(px(12.0))
                .mt(px(8.0))
                .text_color(theme_oa::text::SECONDARY)
                .text_size(px(12.0))
                .child(div().child(format!("{} actions", state.total_actions)))
                .child(div().child(format!("{:.1}m", state.duration_minutes))),
        )
        .when_some(state.snapshot.as_ref(), |el, snapshot| {
            el.child(
                div()
                    .mt(px(8.0))
                    .pt(px(8.0))
                    .border_t_1()
                    .border_color(theme_oa::border::SUBTLE)
                    .text_color(theme_oa::text::MUTED)
                    .text_size(px(11.0))
                    .child(format!(
                        "1h: {:.1} | 6h: {:.1} | 24h: {:.1}",
                        snapshot.apm_1h, snapshot.apm_6h, snapshot.apm_24h
                    )),
            )
        })
        .when_some(state.comparison.as_ref(), |el, comparison| {
            let efficiency_text = if comparison.efficiency_ratio >= 1.0 {
                format!("+{:.0}%", (comparison.efficiency_ratio - 1.0) * 100.0)
            } else {
                format!("-{:.0}%", (1.0 - comparison.efficiency_ratio) * 100.0)
            };
            el.child(
                div()
                    .mt(px(4.0))
                    .text_color(theme_oa::status::SUCCESS)
                    .text_size(px(11.0))
                    .child(format!("vs Claude Code: {}", efficiency_text)),
            )
        })
}

// ============================================================================
// Section 3: Full Widget States
// ============================================================================

fn render_widget_states_section() -> impl IntoElement {
    Story::section()
        .child(Story::section_title("Widget Permutations"))
        .child(Story::description("Different APM levels with full widget styling"))
        .child(
            Story::row()
                .gap(px(16.0))
                .child(render_widget_at_level(ApmLevel::Baseline, 3.2, 18, 5.6))
                .child(render_widget_at_level(ApmLevel::Active, 12.5, 75, 6.0))
                .child(render_widget_at_level(ApmLevel::High, 24.0, 144, 6.0))
                .child(render_widget_at_level(ApmLevel::Elite, 42.0, 252, 6.0)),
        )
}

fn render_widget_at_level(level: ApmLevel, apm: f64, actions: usize, minutes: f64) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .text_xs()
                .text_color(theme_oa::text::MUTED)
                .child(level.label()),
        )
        .child(
            div()
                .w(px(200.0))
                .bg(hud::APM_WIDGET_BG)
                .border_1()
                .border_color(hud::APM_WIDGET_BORDER)
                .rounded_md()
                .p(px(10.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_color(level.color())
                                .text_size(px(18.0))
                                .font_weight(FontWeight::BOLD)
                                .child(format!("{:.1}", apm)),
                        )
                        .child(
                            div()
                                .text_color(level.color())
                                .text_size(px(10.0))
                                .child("APM"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .mt(px(4.0))
                        .text_color(theme_oa::text::MUTED)
                        .text_size(px(10.0))
                        .child(div().child(format!("{} acts", actions)))
                        .child(div().child(format!("{:.0}m", minutes))),
                ),
        )
}
