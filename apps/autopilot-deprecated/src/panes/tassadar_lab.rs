use std::time::{Duration, SystemTime, UNIX_EPOCH};

use psionic_serve::{TassadarLabMetricChip, TassadarLabSourceKind, TassadarLabUpdate};
use wgpui::components::hud::{DotShape, DotsGrid, Scanlines, SignalMeter};
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::app_state::{TassadarLabPaneState, TassadarLabSourceMode, TassadarLabViewMode};
use crate::pane_renderer::{
    paint_action_button, paint_secondary_button, paint_source_badge, paint_state_summary,
    paint_wrapped_label_line, split_text_for_display,
};
use crate::pane_system::{
    tassadar_lab_article_mode_button_bounds, tassadar_lab_evidence_button_bounds,
    tassadar_lab_faster_button_bounds, tassadar_lab_help_button_bounds,
    tassadar_lab_hybrid_mode_button_bounds, tassadar_lab_next_family_button_bounds,
    tassadar_lab_next_replay_button_bounds, tassadar_lab_overview_button_bounds,
    tassadar_lab_play_button_bounds, tassadar_lab_previous_family_button_bounds,
    tassadar_lab_previous_replay_button_bounds, tassadar_lab_program_button_bounds,
    tassadar_lab_refresh_button_bounds, tassadar_lab_replay_mode_button_bounds,
    tassadar_lab_reset_button_bounds, tassadar_lab_slower_button_bounds,
    tassadar_lab_trace_button_bounds,
};

const PANEL_RADIUS: f32 = 10.0;
const ACCENT_CYAN: u32 = 0x67E8F9;
const ACCENT_MINT: u32 = 0x86EFAC;
const ACCENT_GOLD: u32 = 0xFDE68A;
const ACCENT_CORAL: u32 = 0xFDA4AF;
const PANEL_LINE_HEIGHT: f32 = 18.0;

pub fn paint(content_bounds: Bounds, pane_state: &TassadarLabPaneState, paint: &mut PaintContext) {
    paint_source_badge(
        content_bounds,
        pane_state.snapshot().source_badge.as_str(),
        paint,
    );

    let accent = hero_accent(pane_state);
    let phase = animation_phase();

    paint_filter_button(
        tassadar_lab_overview_button_bounds(content_bounds),
        "Overview",
        pane_state.selected_view == TassadarLabViewMode::Overview,
        paint,
    );
    paint_filter_button(
        tassadar_lab_trace_button_bounds(content_bounds),
        "Trace",
        pane_state.selected_view == TassadarLabViewMode::Trace,
        paint,
    );
    paint_filter_button(
        tassadar_lab_program_button_bounds(content_bounds),
        "Program",
        pane_state.selected_view == TassadarLabViewMode::Program,
        paint,
    );
    paint_filter_button(
        tassadar_lab_evidence_button_bounds(content_bounds),
        "Evidence",
        pane_state.selected_view == TassadarLabViewMode::Evidence,
        paint,
    );
    paint_action_button(
        tassadar_lab_previous_replay_button_bounds(content_bounds),
        "Prev case",
        paint,
    );
    paint_action_button(
        tassadar_lab_next_replay_button_bounds(content_bounds),
        "Next case",
        paint,
    );
    paint_filter_button(
        tassadar_lab_replay_mode_button_bounds(content_bounds),
        TassadarLabSourceMode::Replay.short_label(),
        pane_state.selected_source_mode == TassadarLabSourceMode::Replay,
        paint,
    );
    paint_filter_button(
        tassadar_lab_article_mode_button_bounds(content_bounds),
        TassadarLabSourceMode::LiveArticleSession.short_label(),
        pane_state.selected_source_mode == TassadarLabSourceMode::LiveArticleSession,
        paint,
    );
    paint_filter_button(
        tassadar_lab_hybrid_mode_button_bounds(content_bounds),
        TassadarLabSourceMode::LiveArticleHybridWorkflow.short_label(),
        pane_state.selected_source_mode == TassadarLabSourceMode::LiveArticleHybridWorkflow,
        paint,
    );
    paint_secondary_button(
        tassadar_lab_refresh_button_bounds(content_bounds),
        "Refresh",
        paint,
    );
    paint_secondary_button(
        tassadar_lab_help_button_bounds(content_bounds),
        if pane_state.show_help {
            "Hide help"
        } else {
            "Help"
        },
        paint,
    );
    paint_action_button(
        tassadar_lab_play_button_bounds(content_bounds),
        pane_state.playback_button_label(),
        paint,
    );
    paint_secondary_button(
        tassadar_lab_reset_button_bounds(content_bounds),
        "Reset",
        paint,
    );
    paint_secondary_button(
        tassadar_lab_slower_button_bounds(content_bounds),
        "Slower",
        paint,
    );
    paint_secondary_button(
        tassadar_lab_faster_button_bounds(content_bounds),
        "Faster",
        paint,
    );
    paint_secondary_button(
        tassadar_lab_previous_family_button_bounds(content_bounds),
        "Prev family",
        paint,
    );
    paint_secondary_button(
        tassadar_lab_next_family_button_bounds(content_bounds),
        "Next family",
        paint,
    );

    let hero_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        content_bounds.origin.y + 96.0,
        content_bounds.size.width - 24.0,
        88.0,
    );
    let left_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        hero_bounds.max_y() + 12.0,
        (content_bounds.size.width * 0.54)
            .max(420.0)
            .min(content_bounds.size.width - 32.0),
        (content_bounds.size.height * 0.46).max(228.0),
    );
    let right_bounds = Bounds::new(
        left_bounds.max_x() + 12.0,
        left_bounds.origin.y,
        (content_bounds.max_x() - left_bounds.max_x() - 24.0).max(280.0),
        left_bounds.size.height,
    );
    let bottom_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        left_bounds.max_y() + 12.0,
        content_bounds.size.width - 24.0,
        (content_bounds.max_y() - left_bounds.max_y() - 24.0).max(180.0),
    );

    paint_hero(hero_bounds, pane_state, accent, phase, paint);
    match pane_state.selected_view {
        TassadarLabViewMode::Overview => {
            paint_overview(
                left_bounds,
                right_bounds,
                bottom_bounds,
                pane_state,
                accent,
                phase,
                paint,
            );
        }
        TassadarLabViewMode::Trace => {
            paint_trace(
                left_bounds,
                right_bounds,
                bottom_bounds,
                pane_state,
                accent,
                phase,
                paint,
            );
        }
        TassadarLabViewMode::Program => {
            paint_program(
                left_bounds,
                right_bounds,
                bottom_bounds,
                pane_state,
                accent,
                phase,
                paint,
            );
        }
        TassadarLabViewMode::Evidence => {
            paint_evidence(
                left_bounds,
                right_bounds,
                bottom_bounds,
                pane_state,
                accent,
                phase,
                paint,
            );
        }
    }

    if pane_state.show_help {
        paint_help_overlay(content_bounds, paint);
    }
}

fn paint_hero(
    bounds: Bounds,
    pane_state: &TassadarLabPaneState,
    accent: Hsla,
    phase: f32,
    paint: &mut PaintContext,
) {
    paint_panel_shell(bounds, accent, paint);
    paint_panel_texture(bounds, accent, phase, paint);

    let replay_label = pane_state.current_source_label();
    let title = format!(
        "TASSADAR LAB  //  {}",
        pane_state
            .selected_source_mode
            .hero_label()
            .to_ascii_uppercase()
    );
    let subtitle = if pane_state.selected_source_mode == TassadarLabSourceMode::Replay {
        format!(
            "{}  //  {}",
            pane_state.current_replay_family().label(),
            replay_label
        )
    } else {
        format!(
            "{}  //  {}",
            replay_label,
            pane_state.snapshot().family_label
        )
    };
    let status = if pane_state.selected_source_mode == TassadarLabSourceMode::Replay {
        format!(
            "{}  //  {}  //  family {}/{}  //  case {}/{}  //  {} updates",
            pane_state.playback_status_label(),
            pane_state.snapshot().status_label,
            pane_state.replay_family_position(),
            pane_state.replay_family_count(),
            pane_state.replay_family_case_position(),
            pane_state.replay_family_case_count(),
            pane_state.updates().len()
        )
    } else {
        format!(
            "{}  //  {}  //  {} cases  //  {} updates",
            pane_state.playback_status_label(),
            pane_state.snapshot().status_label,
            pane_state.source_case_count(),
            pane_state.updates().len()
        )
    };

    paint.scene.draw_text(paint.text.layout_mono(
        title.as_str(),
        Point::new(bounds.origin.x + 14.0, bounds.origin.y + 18.0),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        subtitle.as_str(),
        Point::new(bounds.origin.x + 14.0, bounds.origin.y + 36.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        status.as_str(),
        Point::new(bounds.origin.x + 14.0, bounds.origin.y + 54.0),
        10.0,
        accent.with_alpha(0.88),
    ));
    paint.scene.draw_text(paint.text.layout(
        pane_state.current_source_description(),
        Point::new(bounds.origin.x + 14.0, bounds.origin.y + 72.0),
        10.0,
        theme::text::MUTED,
    ));

    let summary = format!(
        "{}  //  {} view  //  speed {}  //  window {}",
        pane_state.load_state.label(),
        pane_state.selected_view.label(),
        pane_state.speed_multiplier,
        pane_state.trace_chunk_size
    );
    let _ = paint_state_summary(
        paint,
        bounds.max_x() - 288.0,
        bounds.origin.y + 12.0,
        pane_state.load_state,
        summary.as_str(),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    let meter_bounds = Bounds::new(bounds.max_x() - 170.0, bounds.origin.y + 18.0, 132.0, 18.0);
    let levels = metric_chip_levels(pane_state.snapshot().metric_chips.as_slice(), phase);
    let meter_level = if levels.is_empty() {
        0.0
    } else {
        levels.iter().copied().sum::<f32>() / levels.len() as f32
    };
    let mut meter = SignalMeter::new()
        .level(meter_level)
        .active_color(accent.with_alpha(0.9))
        .inactive_color(theme::bg::APP.with_alpha(0.48));
    meter.paint(meter_bounds, paint);
}

fn paint_overview(
    left_bounds: Bounds,
    right_bounds: Bounds,
    bottom_bounds: Bounds,
    pane_state: &TassadarLabPaneState,
    accent: Hsla,
    phase: f32,
    paint: &mut PaintContext,
) {
    paint_panel_shell(left_bounds, accent.with_alpha(0.86), paint);
    paint_panel_shell(right_bounds, accent.with_alpha(0.82), paint);
    paint_panel_shell(bottom_bounds, accent.with_alpha(0.78), paint);
    paint_panel_texture(left_bounds, accent, phase, paint);
    paint_panel_texture(right_bounds, accent.with_alpha(0.9), phase * 0.8, paint);

    paint_section_title(
        left_bounds,
        if pane_state.selected_source_mode == TassadarLabSourceMode::Replay {
            "ARTIFACT EXPLORER"
        } else {
            "RUN SURFACE"
        },
        accent,
        paint,
    );
    let mut y = left_bounds.origin.y + 30.0;
    if pane_state.selected_source_mode == TassadarLabSourceMode::Replay {
        y = paint_wrapped_label_line(
            paint,
            left_bounds.origin.x + 12.0,
            y,
            "Explorer family",
            pane_state.current_replay_family().label(),
            42,
        );
        y = paint_wrapped_label_line(
            paint,
            left_bounds.origin.x + 12.0,
            y,
            "Family case",
            format!(
                "{}/{}",
                pane_state.replay_family_case_position(),
                pane_state.replay_family_case_count().max(1)
            )
            .as_str(),
            42,
        );
        y = paint_wrapped_label_line(
            paint,
            left_bounds.origin.x + 12.0,
            y,
            "Explorer note",
            pane_state.current_replay_family().description(),
            42,
        );
    }
    y = paint_wrapped_label_line(
        paint,
        left_bounds.origin.x + 12.0,
        y,
        "Subject",
        pane_state.snapshot().subject_label.as_str(),
        42,
    );
    y = paint_wrapped_label_line(
        paint,
        left_bounds.origin.x + 12.0,
        y,
        "Status",
        pane_state.snapshot().status_label.as_str(),
        42,
    );
    y = paint_wrapped_label_line(
        paint,
        left_bounds.origin.x + 12.0,
        y,
        "Route",
        pane_state
            .snapshot()
            .route_state_label
            .as_deref()
            .unwrap_or("none"),
        42,
    );
    let _ = paint_wrapped_label_line(
        paint,
        left_bounds.origin.x + 12.0,
        y,
        "Detail",
        pane_state.snapshot().detail_label.as_str(),
        42,
    );

    paint_section_title(right_bounds, "STATUS CHIPS", accent, paint);
    paint_metric_chips(
        Bounds::new(
            right_bounds.origin.x + 10.0,
            right_bounds.origin.y + 28.0,
            right_bounds.size.width - 20.0,
            right_bounds.size.height - 38.0,
        ),
        pane_state.snapshot().metric_chips.as_slice(),
        paint,
    );

    paint_section_title(bottom_bounds, "EVENT FEED", accent, paint);
    paint_event_feed(
        Bounds::new(
            bottom_bounds.origin.x + 10.0,
            bottom_bounds.origin.y + 28.0,
            bottom_bounds.size.width - 20.0,
            bottom_bounds.size.height - 38.0,
        ),
        pane_state,
        paint,
    );
}

fn paint_trace(
    left_bounds: Bounds,
    right_bounds: Bounds,
    bottom_bounds: Bounds,
    pane_state: &TassadarLabPaneState,
    accent: Hsla,
    phase: f32,
    paint: &mut PaintContext,
) {
    paint_panel_shell(left_bounds, accent.with_alpha(0.86), paint);
    paint_panel_shell(right_bounds, accent.with_alpha(0.82), paint);
    paint_panel_shell(bottom_bounds, accent.with_alpha(0.78), paint);
    paint_panel_texture(left_bounds, accent, phase, paint);
    paint_panel_texture(right_bounds, accent.with_alpha(0.9), phase * 0.8, paint);

    paint_section_title(left_bounds, "READABLE LOG", accent, paint);
    let readable_log_lines = pane_state.current_readable_log_window(12);
    paint_text_block(
        Bounds::new(
            left_bounds.origin.x + 12.0,
            left_bounds.origin.y + 30.0,
            left_bounds.size.width - 24.0,
            left_bounds.size.height - 42.0,
        ),
        readable_log_lines.as_slice(),
        paint,
    );

    paint_section_title(right_bounds, "TOKEN TRACE + OUTPUTS", accent, paint);
    let mut lines = pane_state.current_token_trace_chunk().unwrap_or_default();
    if lines.is_empty() {
        lines.push(String::from("no token-trace chunk for this replay"));
    }
    lines.insert(
        0,
        format!(
            "window={}  chunk={}/{}",
            pane_state.trace_chunk_size,
            pane_state.selected_token_chunk + 1,
            pane_state.token_trace_chunk_count().max(1)
        ),
    );
    lines.push(format!("outputs={:?}", pane_state.snapshot().final_outputs));
    paint_text_block_owned(
        Bounds::new(
            right_bounds.origin.x + 12.0,
            right_bounds.origin.y + 30.0,
            right_bounds.size.width - 24.0,
            right_bounds.size.height - 42.0,
        ),
        lines.as_slice(),
        paint,
    );

    paint_section_title(bottom_bounds, "UPDATE FEED", accent, paint);
    paint_event_feed(
        Bounds::new(
            bottom_bounds.origin.x + 10.0,
            bottom_bounds.origin.y + 28.0,
            bottom_bounds.size.width - 20.0,
            bottom_bounds.size.height - 38.0,
        ),
        pane_state,
        paint,
    );
}

fn paint_program(
    left_bounds: Bounds,
    right_bounds: Bounds,
    bottom_bounds: Bounds,
    pane_state: &TassadarLabPaneState,
    accent: Hsla,
    phase: f32,
    paint: &mut PaintContext,
) {
    paint_panel_shell(left_bounds, accent.with_alpha(0.86), paint);
    paint_panel_shell(right_bounds, accent.with_alpha(0.82), paint);
    paint_panel_shell(bottom_bounds, accent.with_alpha(0.78), paint);
    paint_panel_texture(left_bounds, accent, phase, paint);
    paint_panel_texture(right_bounds, accent.with_alpha(0.9), phase * 0.8, paint);

    paint_section_title(left_bounds, "PROGRAM & RUNTIME", accent, paint);
    let mut y = left_bounds.origin.y + 30.0;
    y = paint_wrapped_label_line(
        paint,
        left_bounds.origin.x + 12.0,
        y,
        "Program id",
        pane_state.snapshot().program_id.as_deref().unwrap_or("n/a"),
        42,
    );
    y = paint_wrapped_label_line(
        paint,
        left_bounds.origin.x + 12.0,
        y,
        "Wasm profile",
        pane_state
            .snapshot()
            .wasm_profile_id
            .as_deref()
            .unwrap_or("n/a"),
        42,
    );
    y = paint_wrapped_label_line(
        paint,
        left_bounds.origin.x + 12.0,
        y,
        "Requested decode",
        decode_mode_label(pane_state.snapshot().requested_decode_mode),
        42,
    );
    y = paint_wrapped_label_line(
        paint,
        left_bounds.origin.x + 12.0,
        y,
        "Effective decode",
        decode_mode_label(pane_state.snapshot().effective_decode_mode),
        42,
    );
    let _ = paint_wrapped_label_line(
        paint,
        left_bounds.origin.x + 12.0,
        y,
        "Runtime backend",
        pane_state
            .snapshot()
            .runtime_capability
            .as_ref()
            .map_or("n/a", |capability| capability.runtime_backend.as_str()),
        42,
    );

    paint_section_title(right_bounds, "LINEAGE FACTS", accent, paint);
    paint_fact_lines(
        Bounds::new(
            right_bounds.origin.x + 12.0,
            right_bounds.origin.y + 30.0,
            right_bounds.size.width - 24.0,
            right_bounds.size.height - 42.0,
        ),
        pane_state,
        paint,
    );

    paint_section_title(bottom_bounds, "EVENT FEED", accent, paint);
    paint_event_feed(
        Bounds::new(
            bottom_bounds.origin.x + 10.0,
            bottom_bounds.origin.y + 28.0,
            bottom_bounds.size.width - 20.0,
            bottom_bounds.size.height - 38.0,
        ),
        pane_state,
        paint,
    );
}

fn paint_evidence(
    left_bounds: Bounds,
    right_bounds: Bounds,
    bottom_bounds: Bounds,
    pane_state: &TassadarLabPaneState,
    accent: Hsla,
    phase: f32,
    paint: &mut PaintContext,
) {
    paint_panel_shell(left_bounds, accent.with_alpha(0.86), paint);
    paint_panel_shell(right_bounds, accent.with_alpha(0.82), paint);
    paint_panel_shell(bottom_bounds, accent.with_alpha(0.78), paint);
    paint_panel_texture(left_bounds, accent, phase, paint);
    paint_panel_texture(right_bounds, accent.with_alpha(0.9), phase * 0.8, paint);

    paint_section_title(left_bounds, "PROOF & BENCHMARK", accent, paint);
    let mut y = left_bounds.origin.y + 30.0;
    if let Some(identity) = pane_state.snapshot().benchmark_identity.as_ref() {
        y = paint_wrapped_label_line(
            paint,
            left_bounds.origin.x + 12.0,
            y,
            "Case",
            format!("{} // {}", identity.workload_family, identity.case_id).as_str(),
            42,
        );
        y = paint_wrapped_label_line(
            paint,
            left_bounds.origin.x + 12.0,
            y,
            "Program digest",
            identity.validated_program_digest.as_str(),
            42,
        );
    }
    if let Some(proof) = pane_state.snapshot().proof_identity.as_ref() {
        y = paint_wrapped_label_line(
            paint,
            left_bounds.origin.x + 12.0,
            y,
            "Trace artifact",
            proof.trace_artifact_id.as_str(),
            42,
        );
        y = paint_wrapped_label_line(
            paint,
            left_bounds.origin.x + 12.0,
            y,
            "Trace digest",
            proof.trace_digest.as_str(),
            42,
        );
    }
    let _ = paint_wrapped_label_line(
        paint,
        left_bounds.origin.x + 12.0,
        y,
        "Artifact ref",
        pane_state
            .snapshot()
            .artifact_ref
            .as_deref()
            .unwrap_or("n/a"),
        42,
    );

    paint_section_title(right_bounds, "FOCUSED DETAIL", accent, paint);
    if let Some((label, value)) = pane_state.current_fact_line() {
        let mut fact_y = right_bounds.origin.y + 30.0;
        fact_y = paint_wrapped_label_line(
            paint,
            right_bounds.origin.x + 12.0,
            fact_y,
            label,
            value,
            36,
        );
        let current_update = pane_state
            .current_update()
            .map(update_summary)
            .unwrap_or_else(|| String::from("no update selected"));
        let _ = paint_wrapped_label_line(
            paint,
            right_bounds.origin.x + 12.0,
            fact_y,
            "Selected update",
            current_update.as_str(),
            36,
        );
    } else {
        paint.scene.draw_text(paint.text.layout(
            "No fact-line focus for this replay yet.",
            Point::new(right_bounds.origin.x + 12.0, right_bounds.origin.y + 34.0),
            10.0,
            theme::text::MUTED,
        ));
    }

    paint_section_title(bottom_bounds, "EVENT FEED", accent, paint);
    paint_event_feed(
        Bounds::new(
            bottom_bounds.origin.x + 10.0,
            bottom_bounds.origin.y + 28.0,
            bottom_bounds.size.width - 20.0,
            bottom_bounds.size.height - 38.0,
        ),
        pane_state,
        paint,
    );
}

fn paint_metric_chips(bounds: Bounds, chips: &[TassadarLabMetricChip], paint: &mut PaintContext) {
    let mut x = bounds.origin.x;
    let mut y = bounds.origin.y;
    let card_width = ((bounds.size.width - 12.0) / 2.0).max(128.0);
    for (index, chip) in chips.iter().take(6).enumerate() {
        let chip_bounds = Bounds::new(x, y, card_width, 54.0);
        paint.scene.draw_quad(
            Quad::new(chip_bounds)
                .with_background(chip_color(chip.tone.as_str()).with_alpha(0.14))
                .with_border(chip_color(chip.tone.as_str()).with_alpha(0.38), 1.0)
                .with_corner_radius(PANEL_RADIUS),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            chip.label.as_str(),
            Point::new(chip_bounds.origin.x + 10.0, chip_bounds.origin.y + 12.0),
            10.0,
            theme::text::MUTED,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            chip.value.as_str(),
            Point::new(chip_bounds.origin.x + 10.0, chip_bounds.origin.y + 30.0),
            12.0,
            chip_color(chip.tone.as_str()),
        ));
        if index % 2 == 0 {
            x += card_width + 12.0;
        } else {
            x = bounds.origin.x;
            y += 66.0;
        }
    }
}

fn paint_fact_lines(bounds: Bounds, pane_state: &TassadarLabPaneState, paint: &mut PaintContext) {
    let mut y = bounds.origin.y;
    let chunk_len = ((bounds.size.width - 146.0) / 6.2).floor() as usize;
    for fact in pane_state.snapshot().fact_lines.iter().take(6) {
        y = paint_wrapped_label_line(
            paint,
            bounds.origin.x,
            y,
            fact.label.as_str(),
            fact.value.as_str(),
            chunk_len.max(18),
        );
    }
}

fn paint_event_feed(bounds: Bounds, pane_state: &TassadarLabPaneState, paint: &mut PaintContext) {
    let mut lines = pane_state
        .local_events
        .iter()
        .rev()
        .map(|line| format!("desktop // {line}"))
        .collect::<Vec<_>>();
    lines.extend(
        pane_state
            .snapshot()
            .events
            .iter()
            .map(|line| format!("snapshot // {line}"))
            .collect::<Vec<_>>(),
    );
    lines.extend(
        pane_state
            .updates()
            .iter()
            .rev()
            .take(10)
            .map(update_summary),
    );
    paint_text_block_owned(bounds, lines.as_slice(), paint);
}

fn paint_text_block(bounds: Bounds, lines: &[String], paint: &mut PaintContext) {
    let borrowed = lines.iter().map(String::as_str).collect::<Vec<_>>();
    paint_text_block_owned(bounds, borrowed.as_slice(), paint);
}

fn paint_text_block_owned(bounds: Bounds, lines: &[impl AsRef<str>], paint: &mut PaintContext) {
    let mut y = bounds.origin.y;
    let line_limit = ((bounds.size.height - 8.0) / PANEL_LINE_HEIGHT).floor() as usize;
    for line in lines.iter().take(line_limit.max(1)) {
        for chunk in split_text_for_display(line.as_ref(), 72) {
            if y > bounds.max_y() - PANEL_LINE_HEIGHT {
                return;
            }
            paint.scene.draw_text(paint.text.layout_mono(
                chunk.as_str(),
                Point::new(bounds.origin.x, y),
                10.0,
                theme::text::PRIMARY,
            ));
            y += PANEL_LINE_HEIGHT;
        }
    }
}

fn paint_section_title(bounds: Bounds, title: &str, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
        10.0,
        accent.with_alpha(0.88),
    ));
}

fn update_summary(update: &TassadarLabUpdate) -> String {
    match update {
        TassadarLabUpdate::StatusLine { line, .. } => line.clone(),
        TassadarLabUpdate::BenchmarkIdentity { benchmark_identity } => format!(
            "benchmark // {} // {}",
            benchmark_identity.benchmark_identity.workload_family,
            benchmark_identity.benchmark_identity.case_id
        ),
        TassadarLabUpdate::Capability { runtime_capability } => format!(
            "capability // {} // default {:?}",
            runtime_capability.runtime_backend, runtime_capability.default_decode_mode
        ),
        TassadarLabUpdate::Selection { selection } => {
            format!(
                "selection // {:?} // {}",
                selection.selection_state, selection.detail
            )
        }
        TassadarLabUpdate::RoutingStatus {
            route_state,
            detail,
            ..
        } => format!("routing // {route_state} // {detail}"),
        TassadarLabUpdate::ProofIdentity { proof_identity } => format!(
            "proof // {}",
            proof_identity.proof_identity.trace_artifact_id
        ),
        TassadarLabUpdate::ReadableLogLine { readable_log_line } => {
            format!(
                "log[{}] // {}",
                readable_log_line.line_index, readable_log_line.line
            )
        }
        TassadarLabUpdate::TokenTraceChunk { token_trace_chunk } => format!(
            "trace[{}] // {} tokens",
            token_trace_chunk.chunk_index,
            token_trace_chunk.tokens.len()
        ),
        TassadarLabUpdate::Output { output } => {
            format!("output[{}] // {}", output.ordinal, output.value)
        }
        TassadarLabUpdate::Terminal {
            status_label,
            detail,
            ..
        } => format!("terminal // {status_label} // {detail}"),
    }
}

fn metric_chip_levels(chips: &[TassadarLabMetricChip], phase: f32) -> Vec<f32> {
    if chips.is_empty() {
        return vec![0.18, 0.32, 0.22, 0.28, 0.16];
    }
    chips
        .iter()
        .take(8)
        .enumerate()
        .map(|(index, chip)| {
            let base = match chip.tone.as_str() {
                "green" => 0.86,
                "amber" => 0.62,
                "red" => 0.34,
                "blue" => 0.54,
                _ => 0.42,
            };
            (base + ((phase + index as f32 * 0.13).sin() * 0.08)).clamp(0.08, 1.0)
        })
        .collect()
}

fn decode_mode_label(mode: Option<psionic_runtime::TassadarExecutorDecodeMode>) -> &'static str {
    match mode {
        Some(psionic_runtime::TassadarExecutorDecodeMode::ReferenceLinear) => "reference_linear",
        Some(psionic_runtime::TassadarExecutorDecodeMode::HullCache) => "hull_cache",
        Some(psionic_runtime::TassadarExecutorDecodeMode::SparseTopK) => "sparse_top_k",
        None => "n/a",
    }
}

fn chip_color(tone: &str) -> Hsla {
    match tone {
        "green" => Hsla::from_hex(ACCENT_MINT),
        "amber" => Hsla::from_hex(ACCENT_GOLD),
        "red" => Hsla::from_hex(ACCENT_CORAL),
        _ => Hsla::from_hex(ACCENT_CYAN),
    }
}

fn hero_accent(pane_state: &TassadarLabPaneState) -> Hsla {
    match pane_state.snapshot().source_kind {
        TassadarLabSourceKind::ReplayArtifact => Hsla::from_hex(ACCENT_GOLD),
        _ => Hsla::from_hex(ACCENT_CYAN),
    }
}

fn animation_phase() -> f32 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs_f32();
    (elapsed.fract() + (elapsed / 8.0).sin() * 0.5).fract()
}

fn paint_filter_button(bounds: Bounds, label: &str, active: bool, paint: &mut PaintContext) {
    if active {
        paint_action_button(bounds, label, paint);
    } else {
        paint_secondary_button(bounds, label, paint);
    }
}

fn paint_panel_shell(bounds: Bounds, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::APP.with_alpha(0.92))
            .with_border(accent.with_alpha(0.28), 1.0)
            .with_corner_radius(PANEL_RADIUS),
    );
}

fn paint_panel_texture(bounds: Bounds, accent: Hsla, phase: f32, paint: &mut PaintContext) {
    let inner = Bounds::new(
        bounds.origin.x + 6.0,
        bounds.origin.y + 6.0,
        bounds.size.width - 12.0,
        bounds.size.height - 12.0,
    );
    let mut dots = DotsGrid::new()
        .shape(DotShape::Cross)
        .distance(20.0)
        .size(1.0)
        .color(accent.with_alpha(0.16))
        .animation_progress(1.0);
    dots.paint(inner, paint);

    let mut scanlines = Scanlines::new()
        .spacing(14.0)
        .line_color(accent.with_alpha(0.06))
        .scan_color(accent.with_alpha(0.18))
        .scan_width(20.0)
        .scan_progress(phase)
        .opacity(0.84);
    scanlines.paint(inner, paint);
}

fn paint_help_overlay(content_bounds: Bounds, paint: &mut PaintContext) {
    let overlay = Bounds::new(
        content_bounds.origin.x + 72.0,
        content_bounds.origin.y + 92.0,
        content_bounds.size.width - 144.0,
        content_bounds.size.height - 184.0,
    );
    paint.scene.draw_quad(
        Quad::new(overlay)
            .with_background(theme::bg::APP.with_alpha(0.96))
            .with_border(theme::accent::PRIMARY.with_alpha(0.35), 1.0)
            .with_corner_radius(PANEL_RADIUS),
    );
    let lines = [
        "TASSADAR LAB",
        "",
        "Artifact explorer plus live shell over Psionic Tassadar lab surfaces.",
        "Source modes:",
        "- Explorer: canonical committed artifact families",
        "- Session: live article executor sessions",
        "- Hybrid: live planner-owned hybrid workflows",
        "",
        "Views:",
        "- Overview: workload, posture, outputs, event feed",
        "- Trace: readable log, token chunks, outputs",
        "- Program: runtime, decode, fact lines",
        "- Evidence: proof identity, lineage, selected update",
        "",
        "Controls:",
        "- Explorer / Session / Hybrid buttons switch source mode",
        "- Prev family / Next family step replay families inside explorer mode",
        "- Prev case / Next case step within the active source mode",
        "- Refresh reloads the current live or replay source from Psionic",
        "",
        "Playback, persistence, and CLI control are desktop-owned.",
        "Explorer mode keeps compiled, learned, acceptance, and comparison roots separate.",
    ];
    let mut y = overlay.origin.y + 18.0;
    for line in lines {
        let color = if line.is_empty() {
            theme::text::MUTED
        } else if line == "TASSADAR LAB" {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        };
        paint.scene.draw_text(paint.text.layout_mono(
            line,
            Point::new(overlay.origin.x + 18.0, y),
            if line == "TASSADAR LAB" { 12.0 } else { 10.0 },
            color,
        ));
        y += PANEL_LINE_HEIGHT;
    }
}

#[cfg(test)]
mod tests {
    use crate::app_state::TassadarLabPaneState;

    #[test]
    fn tassadar_lab_renderer_default_state_exposes_replay_and_updates() {
        let state = TassadarLabPaneState::default();
        assert!(state.current_replay().is_some());
        assert!(!state.updates().is_empty());
    }

    #[test]
    fn tassadar_lab_renderer_can_summarize_updates() {
        let state = TassadarLabPaneState::default();
        let summary = state
            .current_update()
            .map(super::update_summary)
            .unwrap_or_else(|| String::from("none"));
        assert!(!summary.is_empty());
    }
}
