use std::borrow::Cow;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wgpui::components::hud::{DotShape, DotsGrid, Heatmap, RingGauge, Scanlines, SignalMeter};
use wgpui::viz::chart::{HistoryChartSeries, paint_history_chart_body};
use wgpui::viz::feed::{EventFeedRow, paint_event_feed_body};
use wgpui::viz::sampling::sample_history_series;
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::app_state::{
    AttnResLabMetricPoint, AttnResLabPaneState, AttnResLabPlaybackState,
    AttnResLabSublayerSnapshot, AttnResLabViewMode,
};
use crate::pane_renderer::{
    paint_action_button, paint_secondary_button, paint_source_badge, paint_state_summary,
    paint_tertiary_button, split_text_for_display,
};
use crate::pane_system::{
    attnres_lab_faster_button_bounds, attnres_lab_help_button_bounds,
    attnres_lab_inference_button_bounds, attnres_lab_loss_button_bounds,
    attnres_lab_next_sublayer_button_bounds, attnres_lab_overview_button_bounds,
    attnres_lab_pipeline_button_bounds, attnres_lab_previous_sublayer_button_bounds,
    attnres_lab_refresh_button_bounds, attnres_lab_reset_button_bounds,
    attnres_lab_slower_button_bounds, attnres_lab_toggle_playback_button_bounds,
};
const PANEL_RADIUS: f32 = 10.0;
const ACCENT_CYAN: u32 = 0x67E8F9;
const ACCENT_MINT: u32 = 0x86EFAC;
const ACCENT_GOLD: u32 = 0xFDE68A;
const ACCENT_CORAL: u32 = 0xFDA4AF;
const MESH_ROWS: usize = 8;
const MESH_COLS: usize = 20;
const RIBBON_SEGMENTS: usize = 32;
const PANEL_LINE_HEIGHT: f32 = 18.0;
const PANEL_TEXT_RIGHT_PAD: f32 = 12.0;
const PANEL_TITLE_BAR_HEIGHT: f32 = 20.0;
const PANEL_TITLE_FONT_SIZE: f32 = 10.0;

const ALGO_STEPS: [(&str, &str); 5] = [
    (
        "Stack block representations",
        "Collect completed blocks plus the active partial block into the depth stack.",
    ),
    (
        "Normalize the sources",
        "RMS-normalize each source before scoring so routing stays about content rather than scale.",
    ),
    (
        "Score depth with pseudo-query",
        "Project the normalized sources against the learned pseudo-query for the current sublayer.",
    ),
    (
        "Softmax over depth",
        "Convert the depth logits into routing mass across cache and partial lanes.",
    ),
    (
        "Route and accumulate",
        "Blend the chosen sources, run the sublayer, and fold the result into the active block.",
    ),
];

#[derive(Clone, Copy)]
enum MetricHistoryKind {
    Loss,
    Ema,
    Selectivity,
}

pub fn paint(content_bounds: Bounds, pane_state: &AttnResLabPaneState, paint: &mut PaintContext) {
    paint_source_badge(
        content_bounds,
        pane_state.snapshot.source_badge.as_str(),
        paint,
    );
    let phase = animation_phase(pane_state);
    let hero_accent = mesh_accent(pane_state);

    let overview_bounds = attnres_lab_overview_button_bounds(content_bounds);
    let pipeline_bounds = attnres_lab_pipeline_button_bounds(content_bounds);
    let inference_bounds = attnres_lab_inference_button_bounds(content_bounds);
    let loss_bounds = attnres_lab_loss_button_bounds(content_bounds);
    let playback_bounds = attnres_lab_toggle_playback_button_bounds(content_bounds);
    let reset_bounds = attnres_lab_reset_button_bounds(content_bounds);
    let refresh_bounds = attnres_lab_refresh_button_bounds(content_bounds);
    let slower_bounds = attnres_lab_slower_button_bounds(content_bounds);
    let faster_bounds = attnres_lab_faster_button_bounds(content_bounds);
    let help_bounds = attnres_lab_help_button_bounds(content_bounds);
    let previous_bounds = attnres_lab_previous_sublayer_button_bounds(content_bounds);
    let next_bounds = attnres_lab_next_sublayer_button_bounds(content_bounds);

    paint_filter_like_button(
        overview_bounds,
        "Overview",
        pane_state.selected_view == AttnResLabViewMode::Overview,
        paint,
    );
    paint_filter_like_button(
        pipeline_bounds,
        "Pipeline",
        pane_state.selected_view == AttnResLabViewMode::Pipeline,
        paint,
    );
    paint_filter_like_button(
        inference_bounds,
        "Inference",
        pane_state.selected_view == AttnResLabViewMode::Inference,
        paint,
    );
    paint_filter_like_button(
        loss_bounds,
        "Loss",
        pane_state.selected_view == AttnResLabViewMode::Loss,
        paint,
    );
    paint_action_button(
        playback_bounds,
        pane_state.playback_state.button_label(),
        paint,
    );
    paint_secondary_button(reset_bounds, "Reset", paint);
    paint_tertiary_button(refresh_bounds, "Refresh live", paint);
    paint_secondary_button(slower_bounds, "Slower", paint);
    paint_secondary_button(faster_bounds, "Faster", paint);
    paint_secondary_button(
        help_bounds,
        if pane_state.show_help {
            "Hide help"
        } else {
            "Help"
        },
        paint,
    );
    paint_action_button(previous_bounds, "Prev sublayer", paint);
    paint_action_button(next_bounds, "Next sublayer", paint);

    let controls_bottom = refresh_bounds.max_y().max(previous_bounds.max_y());
    let hero_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        controls_bottom + 12.0,
        content_bounds.size.width - 24.0,
        76.0,
    );
    let hero_bottom = paint_title_block(hero_bounds, pane_state, phase, hero_accent, paint);

    let dashboard_top = hero_bottom + 12.0;
    let available_height = (content_bounds.max_y() - dashboard_top - 12.0).max(320.0);
    let min_top_height = 240.0;
    let max_top_height = (available_height - 156.0).max(min_top_height);
    let top_height = (available_height * 0.56).clamp(min_top_height, max_top_height);
    let left_width = (content_bounds.size.width * 0.56).max(420.0);
    let left_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        dashboard_top,
        (left_width - 18.0).min(content_bounds.size.width - 24.0),
        top_height,
    );
    let right_bounds = Bounds::new(
        left_bounds.max_x() + 12.0,
        dashboard_top,
        (content_bounds.max_x() - left_bounds.max_x() - 24.0).max(280.0),
        left_bounds.size.height,
    );
    let bottom_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        left_bounds.max_y() + 12.0,
        content_bounds.size.width - 24.0,
        (available_height - top_height - 12.0).max(144.0),
    );

    match pane_state.selected_view {
        AttnResLabViewMode::Overview => {
            paint_overview(
                left_bounds,
                right_bounds,
                bottom_bounds,
                pane_state,
                phase,
                paint,
            );
        }
        AttnResLabViewMode::Pipeline => {
            paint_pipeline(
                left_bounds,
                right_bounds,
                bottom_bounds,
                pane_state,
                phase,
                paint,
            );
        }
        AttnResLabViewMode::Inference => {
            paint_inference(
                left_bounds,
                right_bounds,
                bottom_bounds,
                pane_state,
                phase,
                paint,
            );
        }
        AttnResLabViewMode::Loss => {
            paint_loss_focus(
                left_bounds,
                right_bounds,
                bottom_bounds,
                pane_state,
                phase,
                paint,
            );
        }
    }

    if pane_state.show_help {
        paint_help_overlay(content_bounds, pane_state.playback_state, paint);
    }
}

fn paint_title_block(
    bounds: Bounds,
    pane_state: &AttnResLabPaneState,
    phase: f32,
    accent: Hsla,
    paint: &mut PaintContext,
) -> f32 {
    paint_panel_shell(bounds, accent, paint);
    paint_panel_texture(bounds, accent, phase, paint);

    let title = match pane_state.playback_state {
        AttnResLabPlaybackState::Armed => "ATTNRES LAB  //  SEEDED CHECKPOINT",
        AttnResLabPlaybackState::Running => "ATTNRES LAB  //  LIVE ROUTING FIELD",
        AttnResLabPlaybackState::Paused => "ATTNRES LAB  //  PAUSED INSPECTION",
        AttnResLabPlaybackState::Completed => "ATTNRES LAB  //  COMPLETED RUN",
    };
    let subtitle = format!(
        "{}  //  {}",
        pane_state.snapshot.model_label, pane_state.snapshot.architecture_label
    );
    let selection_line = pane_state
        .current_sublayer()
        .map(|selected| {
            format!(
                "{} view  //  {}  //  {} {:.0}%  //  cache {:.0}% partial {:.0}%",
                pane_state.selected_view.label(),
                selected.label,
                selected.dominant_source_label,
                selected.dominant_weight * 100.0,
                selected.cache_mass * 100.0,
                selected.partial_mass * 100.0
            )
        })
        .unwrap_or_else(|| {
            format!(
                "{} view  //  no sublayer selected",
                pane_state.selected_view.label()
            )
        });
    let summary = format!(
        "{} // step {}/{} // {}x",
        pane_state.playback_state.status_label(),
        pane_state.snapshot.step,
        pane_state.snapshot.max_steps,
        pane_state.snapshot.speed_multiplier
    );
    let summary_x = bounds.max_x() - 266.0;
    let hero_text_right = summary_x - 18.0;

    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(bounds.origin.x + 14.0, bounds.origin.y + 18.0),
        12.0,
        theme::text::PRIMARY,
    ));
    let fitted_subtitle = truncate_text_to_width(
        paint,
        subtitle.as_str(),
        hero_text_right - (bounds.origin.x + 14.0),
        10.0,
    );
    paint.scene.draw_text(paint.text.layout(
        fitted_subtitle.as_str(),
        Point::new(bounds.origin.x + 14.0, bounds.origin.y + 36.0),
        10.0,
        theme::text::MUTED,
    ));
    let fitted_selection_line = truncate_mono_text_to_width(
        paint,
        selection_line.as_str(),
        hero_text_right - (bounds.origin.x + 14.0),
        10.0,
    );
    paint.scene.draw_text(paint.text.layout_mono(
        fitted_selection_line.as_str(),
        Point::new(bounds.origin.x + 14.0, bounds.origin.y + 54.0),
        10.0,
        accent.with_alpha(0.9),
    ));

    let summary_bottom = paint_state_summary(
        paint,
        summary_x,
        bounds.origin.y + 10.0,
        pane_state.load_state,
        summary.as_str(),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 14.0,
            bounds.origin.y + 23.0,
            138.0,
            2.0,
        ))
        .with_background(accent.with_alpha(0.68)),
    );

    let selected = pane_state.current_sublayer();
    let mut ribbon = AttnResRibbon::new(
        selected
            .map(|sublayer| build_selected_route_ribbon(sublayer, phase))
            .unwrap_or_else(|| build_ribbon_values(0.18, phase, 0.9)),
        accent,
        selected
            .map(|sublayer| sublayer.dominant_weight.clamp(0.0, 1.0))
            .unwrap_or(0.18),
        phase,
    );
    ribbon.paint(
        Bounds::new(bounds.max_x() - 228.0, bounds.max_y() - 20.0, 212.0, 12.0),
        paint,
    );

    summary_bottom.max(bounds.max_y())
}

fn paint_overview(
    left_bounds: Bounds,
    right_bounds: Bounds,
    bottom_bounds: Bounds,
    pane_state: &AttnResLabPaneState,
    phase: f32,
    paint: &mut PaintContext,
) {
    let snapshot = &pane_state.snapshot;
    let selected = pane_state.current_sublayer();
    let accent = Hsla::from_hex(ACCENT_CYAN);

    paint_panel_shell(left_bounds, accent, paint);
    paint_panel_title(left_bounds, "Depth Routing Heatmap", accent, paint);
    paint_heatmap_panel(left_bounds, pane_state, accent, paint);

    let metrics_height = (right_bounds.size.height * 0.34).clamp(108.0, 156.0);
    let metrics_bounds = Bounds::new(
        right_bounds.origin.x,
        right_bounds.origin.y,
        right_bounds.size.width,
        metrics_height - 6.0,
    );
    let topology_height =
        ((right_bounds.size.height - metrics_height - 18.0) * 0.46).clamp(84.0, 132.0);
    let topology_bounds = Bounds::new(
        right_bounds.origin.x,
        metrics_bounds.max_y() + 12.0,
        right_bounds.size.width,
        topology_height,
    );
    let runtime_bounds = Bounds::new(
        right_bounds.origin.x,
        topology_bounds.max_y() + 12.0,
        right_bounds.size.width,
        (right_bounds.max_y() - topology_bounds.max_y() - 12.0).max(88.0),
    );

    paint_panel_shell(metrics_bounds, Hsla::from_hex(ACCENT_MINT), paint);
    paint_panel_title(
        metrics_bounds,
        "Training Telemetry",
        Hsla::from_hex(ACCENT_MINT),
        paint,
    );
    paint_metrics_panel(metrics_bounds, snapshot, selected, phase, paint);

    paint_panel_shell(topology_bounds, Hsla::from_hex(ACCENT_GOLD), paint);
    paint_panel_title(
        topology_bounds,
        "Block Topology",
        Hsla::from_hex(ACCENT_GOLD),
        paint,
    );
    paint_topology_panel(topology_bounds, snapshot, phase, paint);

    paint_panel_shell(runtime_bounds, Hsla::from_hex(ACCENT_CORAL), paint);
    paint_panel_title(
        runtime_bounds,
        "Runtime",
        Hsla::from_hex(ACCENT_CORAL),
        paint,
    );
    paint_runtime_panel(runtime_bounds, snapshot, phase, paint);

    let selected_width = (bottom_bounds.size.width * 0.40).max(268.0);
    let loss_width = (bottom_bounds.size.width * 0.24).clamp(180.0, 260.0);
    let selected_bounds = Bounds::new(
        bottom_bounds.origin.x,
        bottom_bounds.origin.y,
        selected_width.min(bottom_bounds.size.width - 24.0),
        bottom_bounds.size.height,
    );
    let loss_bounds = Bounds::new(
        selected_bounds.max_x() + 12.0,
        bottom_bounds.origin.y,
        loss_width.min((bottom_bounds.max_x() - selected_bounds.max_x() - 24.0).max(160.0)),
        bottom_bounds.size.height,
    );
    let events_bounds = Bounds::new(
        loss_bounds.max_x() + 12.0,
        bottom_bounds.origin.y,
        (bottom_bounds.max_x() - loss_bounds.max_x() - 12.0).max(180.0),
        bottom_bounds.size.height,
    );

    paint_panel_shell(selected_bounds, Hsla::from_hex(ACCENT_GOLD), paint);
    paint_panel_title(
        selected_bounds,
        "Selected Sublayer",
        Hsla::from_hex(ACCENT_GOLD),
        paint,
    );
    paint_selected_sublayer(selected_bounds, selected, phase, paint);

    paint_panel_shell(loss_bounds, Hsla::from_hex(ACCENT_CYAN), paint);
    paint_panel_title(
        loss_bounds,
        "Loss Stream",
        Hsla::from_hex(ACCENT_CYAN),
        paint,
    );
    paint_loss_stream(loss_bounds, snapshot, phase, paint);

    paint_panel_shell(events_bounds, Hsla::from_hex(ACCENT_CORAL), paint);
    paint_panel_title(
        events_bounds,
        "Event Feed",
        Hsla::from_hex(ACCENT_CORAL),
        paint,
    );
    paint_event_feed(events_bounds, snapshot.events.as_slice(), phase, paint);
}

fn paint_pipeline(
    left_bounds: Bounds,
    right_bounds: Bounds,
    bottom_bounds: Bounds,
    pane_state: &AttnResLabPaneState,
    phase: f32,
    paint: &mut PaintContext,
) {
    let selected = pane_state.current_sublayer();
    let accent = Hsla::from_hex(ACCENT_GOLD);

    let block_bounds = Bounds::new(
        left_bounds.origin.x,
        left_bounds.max_y() - 136.0,
        left_bounds.size.width,
        136.0,
    );
    let algo_bounds = Bounds::new(
        left_bounds.origin.x,
        left_bounds.origin.y,
        left_bounds.size.width,
        (block_bounds.origin.y - left_bounds.origin.y - 12.0).max(128.0),
    );
    let inspector_height = (right_bounds.size.height * 0.22).clamp(52.0, 96.0);
    let inspector_bounds = Bounds::new(
        right_bounds.origin.x,
        right_bounds.origin.y,
        right_bounds.size.width,
        inspector_height,
    );
    let story_height = (right_bounds.size.height * 0.24).clamp(56.0, 110.0);
    let story_bounds = Bounds::new(
        right_bounds.origin.x,
        right_bounds.max_y() - story_height,
        right_bounds.size.width,
        story_height,
    );
    let bars_total_height = (story_bounds.origin.y - inspector_bounds.max_y() - 24.0).max(88.0);
    let score_height = ((bars_total_height - 12.0) * 0.5).max(34.0);
    let logits_bounds = Bounds::new(
        right_bounds.origin.x,
        inspector_bounds.max_y() + 12.0,
        right_bounds.size.width,
        score_height,
    );
    let weights_bounds = Bounds::new(
        right_bounds.origin.x,
        logits_bounds.max_y() + 12.0,
        right_bounds.size.width,
        (story_bounds.origin.y - logits_bounds.max_y() - 12.0).max(34.0),
    );

    paint_panel_shell(algo_bounds, accent, paint);
    paint_panel_title(algo_bounds, "Algorithm Filmstrip", accent, paint);
    paint_algorithm_steps(algo_bounds, pane_state, phase, paint);

    paint_panel_shell(block_bounds, Hsla::from_hex(ACCENT_MINT), paint);
    paint_panel_title(
        block_bounds,
        "Block Schedule",
        Hsla::from_hex(ACCENT_MINT),
        paint,
    );
    paint_block_schedule(block_bounds, pane_state, phase, paint);

    paint_panel_shell(inspector_bounds, Hsla::from_hex(ACCENT_CORAL), paint);
    paint_panel_title(
        inspector_bounds,
        "Inspector",
        Hsla::from_hex(ACCENT_CORAL),
        paint,
    );
    paint_pipeline_inspector(
        inspector_bounds,
        selected,
        snapshot_route_regime(&pane_state.snapshot),
        phase,
        paint,
    );

    paint_panel_shell(logits_bounds, Hsla::from_hex(ACCENT_GOLD), paint);
    paint_panel_title(
        logits_bounds,
        "Pre-Softmax Depth Scores",
        Hsla::from_hex(ACCENT_GOLD),
        paint,
    );
    paint_route_metric_panel(logits_bounds, selected, "logits", true, phase, paint);

    paint_panel_shell(weights_bounds, Hsla::from_hex(ACCENT_CYAN), paint);
    paint_panel_title(
        weights_bounds,
        "Softmax Routing Mass",
        Hsla::from_hex(ACCENT_CYAN),
        paint,
    );
    paint_route_metric_panel(
        weights_bounds,
        selected,
        "weights",
        false,
        phase + 0.18,
        paint,
    );

    paint_panel_shell(story_bounds, Hsla::from_hex(ACCENT_CORAL), paint);
    paint_panel_title(
        story_bounds,
        "What Happened Here",
        Hsla::from_hex(ACCENT_CORAL),
        paint,
    );
    paint_route_story(story_bounds, selected, phase, paint);

    paint_panel_shell(bottom_bounds, Hsla::from_hex(ACCENT_CYAN), paint);
    paint_panel_title(
        bottom_bounds,
        "Event Feed",
        Hsla::from_hex(ACCENT_CYAN),
        paint,
    );
    paint_event_feed(
        bottom_bounds,
        pane_state.snapshot.events.as_slice(),
        phase,
        paint,
    );
}

fn paint_inference(
    left_bounds: Bounds,
    right_bounds: Bounds,
    bottom_bounds: Bounds,
    pane_state: &AttnResLabPaneState,
    phase: f32,
    paint: &mut PaintContext,
) {
    let snapshot = &pane_state.snapshot;
    let selected = pane_state.current_sublayer();
    let parity_bounds = Bounds::new(
        left_bounds.origin.x,
        left_bounds.origin.y,
        left_bounds.size.width,
        (left_bounds.size.height * 0.36).max(96.0),
    );
    let schedule_bounds = Bounds::new(
        left_bounds.origin.x,
        parity_bounds.max_y() + 12.0,
        left_bounds.size.width,
        ((left_bounds.size.height - parity_bounds.size.height - 24.0) * 0.42).max(72.0),
    );
    let merge_bounds = Bounds::new(
        left_bounds.origin.x,
        schedule_bounds.max_y() + 12.0,
        left_bounds.size.width,
        (left_bounds.max_y() - schedule_bounds.max_y() - 12.0).max(68.0),
    );
    let cache_bounds = Bounds::new(
        right_bounds.origin.x,
        right_bounds.max_y() - (right_bounds.size.height * 0.26).clamp(76.0, 112.0),
        right_bounds.size.width,
        (right_bounds.size.height * 0.26).clamp(76.0, 112.0),
    );
    let detail_bounds = Bounds::new(
        right_bounds.origin.x,
        right_bounds.origin.y,
        right_bounds.size.width,
        (cache_bounds.origin.y - right_bounds.origin.y - 12.0).max(108.0),
    );

    paint_panel_shell(parity_bounds, Hsla::from_hex(ACCENT_CYAN), paint);
    paint_panel_title(
        parity_bounds,
        "Two-Phase Parity",
        Hsla::from_hex(ACCENT_CYAN),
        paint,
    );
    paint_inference_parity(parity_bounds, snapshot, phase, paint);

    paint_panel_shell(schedule_bounds, Hsla::from_hex(ACCENT_CORAL), paint);
    paint_panel_title(
        schedule_bounds,
        "Two-Phase Schedule",
        Hsla::from_hex(ACCENT_CORAL),
        paint,
    );
    paint_two_phase_schedule(schedule_bounds, snapshot, phase, paint);

    paint_panel_shell(merge_bounds, Hsla::from_hex(ACCENT_GOLD), paint);
    paint_panel_title(
        merge_bounds,
        "Online Merge",
        Hsla::from_hex(ACCENT_GOLD),
        paint,
    );
    paint_online_merge(merge_bounds, snapshot, selected, phase, paint);

    paint_panel_shell(detail_bounds, Hsla::from_hex(ACCENT_MINT), paint);
    paint_panel_title(
        detail_bounds,
        "Selected Detail",
        Hsla::from_hex(ACCENT_MINT),
        paint,
    );
    paint_selected_sublayer(detail_bounds, selected, phase, paint);

    paint_panel_shell(cache_bounds, Hsla::from_hex(ACCENT_MINT), paint);
    paint_panel_title(
        cache_bounds,
        "Block Cache Health",
        Hsla::from_hex(ACCENT_MINT),
        paint,
    );
    paint_cache_health(cache_bounds, snapshot, phase, paint);

    paint_panel_shell(bottom_bounds, Hsla::from_hex(ACCENT_CORAL), paint);
    paint_panel_title(
        bottom_bounds,
        "Event Feed",
        Hsla::from_hex(ACCENT_CORAL),
        paint,
    );
    paint_event_feed(bottom_bounds, snapshot.events.as_slice(), phase, paint);
}

fn paint_loss_focus(
    left_bounds: Bounds,
    right_bounds: Bounds,
    bottom_bounds: Bounds,
    pane_state: &AttnResLabPaneState,
    phase: f32,
    paint: &mut PaintContext,
) {
    let snapshot = &pane_state.snapshot;
    let chart_bounds = Bounds::new(
        left_bounds.origin.x,
        left_bounds.origin.y,
        right_bounds.max_x() - left_bounds.origin.x,
        left_bounds.size.height,
    );
    let summary_width = (bottom_bounds.size.width * 0.28).clamp(236.0, 320.0);
    let rails_width = (bottom_bounds.size.width * 0.26).clamp(220.0, 308.0);
    let summary_bounds = Bounds::new(
        bottom_bounds.origin.x,
        bottom_bounds.origin.y,
        summary_width.min(bottom_bounds.size.width - 24.0),
        bottom_bounds.size.height,
    );
    let rails_bounds = Bounds::new(
        summary_bounds.max_x() + 12.0,
        bottom_bounds.origin.y,
        rails_width.min((bottom_bounds.max_x() - summary_bounds.max_x() - 24.0).max(180.0)),
        bottom_bounds.size.height,
    );
    let events_bounds = Bounds::new(
        rails_bounds.max_x() + 12.0,
        bottom_bounds.origin.y,
        (bottom_bounds.max_x() - rails_bounds.max_x() - 12.0).max(180.0),
        bottom_bounds.size.height,
    );

    paint_panel_shell(chart_bounds, Hsla::from_hex(ACCENT_CYAN), paint);
    paint_panel_title(
        chart_bounds,
        "Training Loss Curve",
        Hsla::from_hex(ACCENT_CYAN),
        paint,
    );
    paint_loss_curve_panel(chart_bounds, snapshot, phase, paint);

    paint_panel_shell(summary_bounds, Hsla::from_hex(ACCENT_MINT), paint);
    paint_panel_title(
        summary_bounds,
        "Loss Summary",
        Hsla::from_hex(ACCENT_MINT),
        paint,
    );
    paint_loss_summary_panel(summary_bounds, snapshot, phase, paint);

    paint_panel_shell(rails_bounds, Hsla::from_hex(ACCENT_CYAN), paint);
    paint_panel_title(
        rails_bounds,
        "Loss Rails",
        Hsla::from_hex(ACCENT_CYAN),
        paint,
    );
    paint_loss_stream(rails_bounds, snapshot, phase, paint);

    paint_panel_shell(events_bounds, Hsla::from_hex(ACCENT_GOLD), paint);
    paint_panel_title(
        events_bounds,
        "Loss Events",
        Hsla::from_hex(ACCENT_GOLD),
        paint,
    );
    paint_event_feed(events_bounds, snapshot.events.as_slice(), phase, paint);
}

fn paint_heatmap_panel(
    bounds: Bounds,
    pane_state: &AttnResLabPaneState,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    let snapshot = &pane_state.snapshot;
    let matrix_bounds = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 30.0,
        bounds.size.width - 24.0,
        (bounds.size.height - 64.0).max(120.0),
    );

    let mut dots = DotsGrid::new()
        .shape(DotShape::Cross)
        .distance(26.0)
        .size(1.1)
        .color(accent.with_alpha(0.18))
        .animation_progress(1.0);
    dots.paint(matrix_bounds, paint);

    let mut heatmap = Heatmap::new()
        .data(
            snapshot.sublayers.len().max(1),
            snapshot.max_sources(),
            build_heatmap_data(snapshot),
        )
        .range(0.0, 1.0)
        .gap(2.0)
        .low_color(Hsla::from_hex(0x061018).with_alpha(0.94))
        .mid_color(Some(Hsla::from_hex(ACCENT_CYAN).with_alpha(0.72)))
        .high_color(Hsla::from_hex(0xF8FAFC).with_alpha(0.98));
    heatmap.paint(matrix_bounds, paint);

    let scan_progress =
        (pane_state.selected_sublayer as f32 + 1.0) / snapshot.sublayers.len().max(1) as f32;
    let mut scanlines = Scanlines::new()
        .spacing(14.0)
        .line_color(accent.with_alpha(0.08))
        .scan_color(accent.with_alpha(0.16))
        .scan_width(18.0)
        .scan_progress(scan_progress)
        .opacity(0.92);
    scanlines.paint(matrix_bounds, paint);

    let rows = snapshot.sublayers.len().max(1);
    let row_height = ((matrix_bounds.size.height - (rows.saturating_sub(1) as f32 * 2.0))
        / rows as f32)
        .max(8.0);
    let highlight_y =
        matrix_bounds.origin.y + pane_state.selected_sublayer as f32 * (row_height + 2.0);
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            matrix_bounds.origin.x,
            highlight_y,
            matrix_bounds.size.width,
            row_height,
        ))
        .with_background(accent.with_alpha(0.08))
        .with_border(accent.with_alpha(0.32), 1.0)
        .with_corner_radius(4.0),
    );

    let legend = format!(
        "{} sublayers  //  {} max sources  //  active block {}  //  selected {}",
        snapshot.sublayers.len(),
        snapshot.max_sources(),
        snapshot.active_block,
        pane_state
            .current_sublayer()
            .map(|selected| selected.label.as_str())
            .unwrap_or("-")
    );
    let fitted_legend =
        truncate_mono_text_to_width(paint, legend.as_str(), bounds.size.width - 24.0, 10.0);
    paint.scene.draw_text(paint.text.layout_mono(
        fitted_legend.as_str(),
        Point::new(bounds.origin.x + 12.0, bounds.max_y() - 10.0),
        10.0,
        theme::text::MUTED,
    ));
}

fn paint_metrics_panel(
    bounds: Bounds,
    snapshot: &crate::app_state::AttnResLabSnapshot,
    selected: Option<&AttnResLabSublayerSnapshot>,
    phase: f32,
    paint: &mut PaintContext,
) {
    paint_panel_texture(bounds, Hsla::from_hex(ACCENT_MINT), phase, paint);

    let progress = if snapshot.max_steps == 0 {
        0.0
    } else {
        snapshot.step as f32 / snapshot.max_steps as f32
    };
    let progress_bounds = Bounds::new(bounds.origin.x + 14.0, bounds.origin.y + 38.0, 76.0, 76.0);
    let mut progress_ring = RingGauge::new()
        .level(progress.clamp(0.0, 1.0))
        .segments(42)
        .dot_size(4.0)
        .active_color(Hsla::from_hex(ACCENT_CYAN).with_alpha(0.88))
        .inactive_color(theme::bg::APP)
        .head_color(theme::text::PRIMARY);
    progress_ring.paint(progress_bounds, paint);

    let selectivity_bounds =
        Bounds::new(bounds.origin.x + 102.0, bounds.origin.y + 38.0, 76.0, 76.0);
    let mut selectivity_ring = RingGauge::new()
        .level(snapshot.avg_selectivity.clamp(0.0, 1.0))
        .segments(42)
        .dot_size(4.0)
        .active_color(Hsla::from_hex(ACCENT_MINT).with_alpha(0.92))
        .inactive_color(theme::bg::APP)
        .head_color(theme::text::PRIMARY);
    selectivity_ring.paint(selectivity_bounds, paint);

    paint.scene.draw_text(paint.text.layout_mono(
        &format!("{:>3.0}%", progress * 100.0),
        Point::new(
            progress_bounds.origin.x + 18.0,
            progress_bounds.max_y() - 10.0,
        ),
        10.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("{:>3.0}%", snapshot.avg_selectivity * 100.0),
        Point::new(
            selectivity_bounds.origin.x + 18.0,
            selectivity_bounds.max_y() - 10.0,
        ),
        10.0,
        theme::text::PRIMARY,
    ));

    let meta_x = (bounds.origin.x + 196.0).min(bounds.max_x() - 146.0);
    let mut y = bounds.origin.y + 40.0;
    y = paint_panel_label_line(paint, bounds, meta_x, y, "Run", snapshot.run_label.as_str());
    y = paint_panel_label_line(
        paint,
        bounds,
        meta_x,
        y,
        "Loss",
        format!("{:.3}", snapshot.training_loss).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        meta_x,
        y,
        "EMA",
        format!("{:.3}", snapshot.ema_loss).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        meta_x,
        y,
        "Block",
        format!(
            "{} active // {} complete // fill {}",
            snapshot.active_block, snapshot.completed_blocks, snapshot.current_block_fill
        )
        .as_str(),
    );
    let query_norm = selected
        .map(|sublayer| sublayer.query_norm)
        .unwrap_or_else(|| mean_query_norm(snapshot));
    let _ = paint_panel_label_line(
        paint,
        bounds,
        meta_x,
        y,
        "Selected q",
        format!("{query_norm:.2}").as_str(),
    );

    paint_signal_triplet(
        bounds.origin.x + 20.0,
        bounds.origin.y + 126.0,
        [
            (
                "LS",
                descent_level(
                    snapshot.metrics.first().map(|point| point.training_loss),
                    snapshot.training_loss,
                ),
                Hsla::from_hex(ACCENT_CORAL),
            ),
            (
                "EM",
                descent_level(
                    snapshot.metrics.first().map(|point| point.ema_loss),
                    snapshot.ema_loss,
                ),
                Hsla::from_hex(ACCENT_GOLD),
            ),
            (
                "QN",
                normalize_signal(query_norm, 1.15),
                Hsla::from_hex(ACCENT_MINT),
            ),
        ],
        Hsla::from_hex(ACCENT_MINT),
        paint,
    );

    let ribbon_top = (bounds.origin.y + 122.0).max(y + 12.0);
    let ribbon_gap = 8.0;
    let ribbon_height = ((bounds.max_y() - ribbon_top - 12.0 - ribbon_gap * 2.0) / 3.0).max(10.0);
    let labels_x = bounds.origin.x + 16.0;
    let ribbon_x = bounds.origin.x + 88.0;
    let ribbon_width = (bounds.size.width - 102.0).max(64.0);
    let ribbons = [
        (
            "LOSS",
            Hsla::from_hex(ACCENT_CORAL),
            build_metric_history_ribbon(
                snapshot.metrics.as_slice(),
                MetricHistoryKind::Loss,
                phase,
            ),
        ),
        (
            "EMA",
            Hsla::from_hex(ACCENT_GOLD),
            build_metric_history_ribbon(
                snapshot.metrics.as_slice(),
                MetricHistoryKind::Ema,
                phase + 0.13,
            ),
        ),
        (
            "SEL",
            Hsla::from_hex(ACCENT_MINT),
            build_metric_history_ribbon(
                snapshot.metrics.as_slice(),
                MetricHistoryKind::Selectivity,
                phase + 0.27,
            ),
        ),
    ];

    for (index, (label, color, (values, level))) in ribbons.into_iter().enumerate() {
        let y = ribbon_top + index as f32 * (ribbon_height + ribbon_gap);
        paint.scene.draw_text(paint.text.layout_mono(
            label,
            Point::new(labels_x, y + 9.0),
            10.0,
            color.with_alpha(0.92),
        ));
        let mut ribbon = AttnResRibbon::new(values, color, level, phase + index as f32 * 0.11);
        ribbon.paint(Bounds::new(ribbon_x, y, ribbon_width, ribbon_height), paint);
    }
}

fn paint_topology_panel(
    bounds: Bounds,
    snapshot: &crate::app_state::AttnResLabSnapshot,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = Hsla::from_hex(ACCENT_GOLD);
    paint_panel_texture(bounds, accent, phase, paint);

    let mut y = bounds.origin.y + 34.0;
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Layers",
        format!(
            "{} transformer // {} sublayers",
            snapshot.num_transformer_layers,
            snapshot.sublayers.len()
        )
        .as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Blocks",
        format!(
            "{} total // block size {}",
            snapshot.num_residual_blocks, snapshot.block_size
        )
        .as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Heads",
        format!("{}", snapshot.num_heads).as_str(),
    );
    let _ = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Boundaries",
        join_labels(snapshot.inference.boundary_layers.as_slice()).as_str(),
    );

    let rail_top = (bounds.max_y() - 34.0).max(bounds.origin.y + 88.0);
    let gap = 8.0;
    let block_count = snapshot.block_summaries.len().max(1);
    let chip_width = ((bounds.size.width - 24.0 - gap * (block_count as f32 - 1.0))
        / block_count as f32)
        .max(36.0);
    for (index, block) in snapshot.block_summaries.iter().enumerate() {
        let x = bounds.origin.x + 12.0 + index as f32 * (chip_width + gap);
        let chip_bounds = Bounds::new(x, rail_top, chip_width, 22.0);
        let is_active = block.block_index + 1 == snapshot.active_block;
        paint.scene.draw_quad(
            Quad::new(chip_bounds)
                .with_background(if is_active {
                    accent.with_alpha(0.24)
                } else {
                    theme::bg::APP.with_alpha(0.78)
                })
                .with_border(
                    if is_active {
                        accent.with_alpha(0.42)
                    } else {
                        accent.with_alpha(0.12)
                    },
                    1.0,
                )
                .with_corner_radius(5.0),
        );
        paint.scene.draw_text(
            paint.text.layout_mono(
                format!(
                    "B{}  {:>2.0}%",
                    block.block_index,
                    block.avg_selectivity * 100.0
                )
                .as_str(),
                Point::new(chip_bounds.origin.x + 6.0, chip_bounds.origin.y + 14.0),
                9.0,
                if is_active {
                    theme::text::PRIMARY
                } else {
                    accent.with_alpha(0.88)
                },
            ),
        );
    }
}

fn paint_runtime_panel(
    bounds: Bounds,
    snapshot: &crate::app_state::AttnResLabSnapshot,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = Hsla::from_hex(ACCENT_CYAN);
    paint_panel_texture(bounds, accent, phase, paint);

    let mut y = bounds.origin.y + 34.0;
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Tensor",
        format!(
            "{} batch // {} seq // {} hidden",
            snapshot.batch_size, snapshot.sequence_length, snapshot.hidden_size
        )
        .as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Query",
        format!(
            "{:.2} avg // {:.2} max",
            snapshot.mean_query_norm, snapshot.max_query_norm
        )
        .as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Throughput",
        format!("{:.1} steps/s", snapshot.steps_per_second).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Loop",
        format!(
            "{:.1} ms train // {:.1} ms diag // {:.1} ms avg",
            snapshot.last_train_ms, snapshot.last_diag_ms, snapshot.avg_loop_ms
        )
        .as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "ETA",
        format!("{:.1}s remaining", snapshot.eta_seconds).as_str(),
    );
    let _ = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Playback",
        format!(
            "{} // {}x // {}",
            snapshot.run_status,
            snapshot.speed_multiplier,
            snapshot_route_regime(snapshot)
        )
        .as_str(),
    );

    let signals_y = (bounds.max_y() - 74.0).max(bounds.origin.y + 54.0);
    paint_signal_triplet(
        bounds.max_x() - 148.0,
        signals_y,
        [
            (
                "QN",
                normalize_signal(snapshot.mean_query_norm, 1.0),
                Hsla::from_hex(ACCENT_GOLD),
            ),
            (
                "CF",
                snapshot.inference.block_cache_fill_share.clamp(0.0, 1.0),
                Hsla::from_hex(ACCENT_MINT),
            ),
            (
                "PR",
                if snapshot.final_partial_block_present {
                    1.0
                } else {
                    0.0
                },
                accent,
            ),
        ],
        accent,
        paint,
    );
}

fn paint_loss_stream(
    bounds: Bounds,
    snapshot: &crate::app_state::AttnResLabSnapshot,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = Hsla::from_hex(ACCENT_CYAN);
    paint_panel_texture(bounds, accent, phase, paint);

    let mut y = bounds.origin.y + 34.0;
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Window",
        format!("{} points", snapshot.metrics.len()).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Loss",
        format!("{:.3}", snapshot.training_loss).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "EMA",
        format!("{:.3}", snapshot.ema_loss).as_str(),
    );
    let _ = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Selectivity",
        format!("{:.0}%", snapshot.avg_selectivity * 100.0).as_str(),
    );

    let ribbon_top = (bounds.origin.y + 106.0).max(bounds.origin.y + 30.0);
    let ribbon_gap = 8.0;
    let ribbon_height = ((bounds.max_y() - ribbon_top - 12.0 - ribbon_gap * 2.0) / 3.0).max(10.0);
    let labels_x = bounds.origin.x + 14.0;
    let ribbon_x = bounds.origin.x + 72.0;
    let ribbon_width = (bounds.size.width - 86.0).max(72.0);
    let ribbons = [
        (
            "LOSS",
            Hsla::from_hex(ACCENT_CORAL),
            build_metric_history_ribbon(
                snapshot.metrics.as_slice(),
                MetricHistoryKind::Loss,
                phase,
            ),
        ),
        (
            "EMA",
            Hsla::from_hex(ACCENT_GOLD),
            build_metric_history_ribbon(
                snapshot.metrics.as_slice(),
                MetricHistoryKind::Ema,
                phase + 0.13,
            ),
        ),
        (
            "SEL",
            Hsla::from_hex(ACCENT_MINT),
            build_metric_history_ribbon(
                snapshot.metrics.as_slice(),
                MetricHistoryKind::Selectivity,
                phase + 0.27,
            ),
        ),
    ];

    for (index, (label, color, (values, level))) in ribbons.into_iter().enumerate() {
        let row_y = ribbon_top + index as f32 * (ribbon_height + ribbon_gap);
        paint.scene.draw_text(paint.text.layout_mono(
            label,
            Point::new(labels_x, row_y + 9.0),
            10.0,
            color.with_alpha(0.92),
        ));
        let mut ribbon = AttnResRibbon::new(values, color, level, phase + index as f32 * 0.09);
        ribbon.paint(
            Bounds::new(ribbon_x, row_y, ribbon_width, ribbon_height),
            paint,
        );
    }
}

fn paint_loss_curve_panel(
    bounds: Bounds,
    snapshot: &crate::app_state::AttnResLabSnapshot,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = Hsla::from_hex(ACCENT_CORAL);
    let loss_raw = metric_history_raw_values(snapshot.metrics.as_slice(), MetricHistoryKind::Loss);
    let ema_raw = metric_history_raw_values(snapshot.metrics.as_slice(), MetricHistoryKind::Ema);
    let start_loss = loss_raw.first().copied().unwrap_or(snapshot.training_loss);
    let best_loss = loss_raw
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min)
        .min(snapshot.training_loss);
    let live_step = snapshot
        .metrics
        .last()
        .map(|point| point.global_step)
        .unwrap_or(0);
    let header = format!(
        "start {:.3}  //  best {:.3}  //  live {:.3}  //  ema {:.3}",
        start_loss, best_loss, snapshot.training_loss, snapshot.ema_loss
    );
    let fitted_header =
        truncate_mono_text_to_width(paint, header.as_str(), bounds.size.width - 32.0, 10.0);
    paint.scene.draw_text(paint.text.layout_mono(
        fitted_header.as_str(),
        Point::new(bounds.origin.x + 16.0, bounds.origin.y + 34.0),
        10.0,
        theme::text::PRIMARY,
    ));

    let footer = format!(
        "step {} / {}  //  samples {}  //  improvement {:.1}%",
        live_step,
        snapshot.max_steps,
        snapshot.metrics.len(),
        ((start_loss - snapshot.training_loss) / start_loss.max(0.0001) * 100.0).max(0.0)
    );
    paint_history_chart_body(
        bounds,
        accent,
        phase,
        Some(header.as_str()),
        Some(footer.as_str()),
        "No AttnRes metrics recorded yet.",
        &[
            HistoryChartSeries {
                label: "loss",
                values: loss_raw.as_slice(),
                color: accent,
                fill_alpha: 0.11,
                line_alpha: 0.82,
            },
            HistoryChartSeries {
                label: "ema",
                values: ema_raw.as_slice(),
                color: Hsla::from_hex(ACCENT_GOLD),
                fill_alpha: 0.0,
                line_alpha: 0.88,
            },
        ],
        paint,
    );
}

fn paint_loss_summary_panel(
    bounds: Bounds,
    snapshot: &crate::app_state::AttnResLabSnapshot,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = Hsla::from_hex(ACCENT_MINT);
    paint_panel_texture(bounds, accent, phase, paint);

    let loss_raw = metric_history_raw_values(snapshot.metrics.as_slice(), MetricHistoryKind::Loss);
    let start_loss = loss_raw.first().copied().unwrap_or(snapshot.training_loss);
    let best_loss = loss_raw
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min)
        .min(snapshot.training_loss);
    let loss_gap = (snapshot.ema_loss - snapshot.training_loss).abs();
    let stability = (1.0 - normalize_signal(loss_gap, start_loss.max(0.1) * 0.08)).clamp(0.0, 1.0);
    let cadence = normalize_signal(snapshot.steps_per_second as f32, 50.0);
    let descent = descent_level(Some(start_loss), snapshot.training_loss);

    paint_signal_triplet(
        bounds.origin.x + 16.0,
        bounds.origin.y + 38.0,
        [
            ("DS", descent, Hsla::from_hex(ACCENT_CORAL)),
            ("ST", stability, Hsla::from_hex(ACCENT_GOLD)),
            ("SP", cadence, Hsla::from_hex(ACCENT_CYAN)),
        ],
        accent,
        paint,
    );

    let mut y = bounds.origin.y + 118.0;
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Step",
        format!("{}/{}", snapshot.step, snapshot.max_steps).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Loss",
        format!("{:.4}", snapshot.training_loss).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "EMA",
        format!("{:.4}", snapshot.ema_loss).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Start",
        format!("{:.4}", start_loss).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Best",
        format!("{:.4}", best_loss).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Gap",
        format!("{:.4}", loss_gap).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Gain",
        format!(
            "{:.1}%",
            ((start_loss - snapshot.training_loss) / start_loss.max(0.0001) * 100.0).max(0.0)
        )
        .as_str(),
    );
    let _ = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Selectivity",
        format!("{:.0}%", snapshot.avg_selectivity * 100.0).as_str(),
    );
}

fn paint_pipeline_inspector(
    bounds: Bounds,
    selected: Option<&AttnResLabSublayerSnapshot>,
    regime: &str,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = Hsla::from_hex(ACCENT_CORAL);
    paint_panel_texture(bounds, accent, phase, paint);

    let Some(selected) = selected else {
        paint.scene.draw_text(paint.text.layout(
            "No sublayer selected.",
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 34.0),
            11.0,
            theme::text::MUTED,
        ));
        return;
    };

    let mut y = bounds.origin.y + 34.0;
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Label",
        selected.label.as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Layer / slot",
        format!(
            "T{} // slot {}",
            selected.transformer_layer_index,
            selected.slot_in_block + 1
        )
        .as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Mode",
        selected.route_mode_label(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Entropy",
        format!("{:.2}", selected.entropy).as_str(),
    );
    let _ = paint_panel_label_line(paint, bounds, bounds.origin.x + 12.0, y, "Regime", regime);
}

fn paint_selected_sublayer(
    bounds: Bounds,
    selected: Option<&AttnResLabSublayerSnapshot>,
    phase: f32,
    paint: &mut PaintContext,
) {
    let Some(selected) = selected else {
        paint.scene.draw_text(paint.text.layout(
            "No sublayer selected.",
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 36.0),
            12.0,
            theme::text::MUTED,
        ));
        return;
    };

    let accent = if selected.kind_label.eq_ignore_ascii_case("attention") {
        Hsla::from_hex(ACCENT_CYAN)
    } else {
        Hsla::from_hex(ACCENT_GOLD)
    };
    paint_panel_texture(bounds, accent, phase, paint);

    let cache_bounds = Bounds::new(bounds.max_x() - 90.0, bounds.origin.y + 38.0, 22.0, 64.0);
    let partial_bounds = Bounds::new(bounds.max_x() - 56.0, bounds.origin.y + 38.0, 22.0, 64.0);
    let mut cache_meter = SignalMeter::new()
        .bars(6)
        .gap(2.0)
        .level(selected.cache_mass.clamp(0.0, 1.0))
        .min_bar_height(0.16)
        .active_color(Hsla::from_hex(ACCENT_CYAN).with_alpha(0.92))
        .inactive_color(accent.with_alpha(0.08));
    cache_meter.paint(cache_bounds, paint);
    let mut partial_meter = SignalMeter::new()
        .bars(6)
        .gap(2.0)
        .level(selected.partial_mass.clamp(0.0, 1.0))
        .min_bar_height(0.16)
        .active_color(Hsla::from_hex(ACCENT_GOLD).with_alpha(0.92))
        .inactive_color(accent.with_alpha(0.08));
    partial_meter.paint(partial_bounds, paint);
    paint.scene.draw_text(paint.text.layout_mono(
        "CA",
        Point::new(cache_bounds.origin.x - 2.0, cache_bounds.max_y() + 12.0),
        9.0,
        Hsla::from_hex(ACCENT_CYAN).with_alpha(0.86),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        "PT",
        Point::new(partial_bounds.origin.x - 2.0, partial_bounds.max_y() + 12.0),
        9.0,
        Hsla::from_hex(ACCENT_GOLD).with_alpha(0.86),
    ));

    let compact = bounds.size.height < 190.0 || bounds.size.width < 360.0;
    let mut y = bounds.origin.y + 36.0;
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Label",
        selected.label.as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Layer / slot",
        format!(
            "T{} // slot {}",
            selected.transformer_layer_index,
            selected.slot_in_block + 1
        )
        .as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Mode",
        selected.route_mode_label(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Dominant route",
        format!(
            "{} ({:.0}%)",
            selected.dominant_source_label,
            selected.dominant_weight * 100.0
        )
        .as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Kind / block",
        format!("{} // B{}", selected.kind_label, selected.target_block).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Query / entropy",
        format!("{:.2} // {:.2}", selected.query_norm, selected.entropy).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Cache / partial",
        format!(
            "{:.0}% / {:.0}%",
            selected.cache_mass * 100.0,
            selected.partial_mass * 100.0
        )
        .as_str(),
    );
    if !compact {
        y = paint_panel_label_line(
            paint,
            bounds,
            bounds.origin.x + 12.0,
            y,
            "Sources",
            format!("{}", selected.source_count()).as_str(),
        );
        y = paint_panel_label_line(
            paint,
            bounds,
            bounds.origin.x + 12.0,
            y,
            "Boundary",
            if selected.starts_new_block_before {
                "opened before sublayer"
            } else {
                "stayed inside block"
            },
        );
    }
    y = paint_panel_multiline_phrase(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y + 8.0,
        "route",
        selected.route_note.as_str(),
        if compact { 2 } else { 3 },
    );

    let ribbon_top = (y + 8.0).min(bounds.max_y() - 28.0);
    if !compact && ribbon_top + 12.0 <= bounds.max_y() - 10.0 {
        paint.scene.draw_text(paint.text.layout_mono(
            "ROUTE FIELD",
            Point::new(bounds.origin.x + 12.0, ribbon_top + 9.0),
            10.0,
            accent.with_alpha(0.88),
        ));
        let mut ribbon = AttnResRibbon::new(
            build_selected_route_ribbon(selected, phase),
            accent,
            selected.dominant_weight.clamp(0.0, 1.0),
            phase,
        );
        ribbon.paint(
            Bounds::new(
                bounds.origin.x + 102.0,
                ribbon_top,
                bounds.size.width - 116.0,
                12.0,
            ),
            paint,
        );
    }
}

fn paint_event_feed(bounds: Bounds, events: &[String], phase: f32, paint: &mut PaintContext) {
    let rows = events
        .iter()
        .enumerate()
        .map(|(index, event)| EventFeedRow {
            label: Cow::Owned(format!("E{:02}", index + 1)),
            detail: Cow::Borrowed(event.as_str()),
            color: Hsla::from_hex(ACCENT_CORAL),
        })
        .collect::<Vec<_>>();
    paint_event_feed_body(
        bounds,
        Hsla::from_hex(ACCENT_CORAL),
        phase,
        "No AttnRes events recorded yet.",
        rows.as_slice(),
        paint,
    );
}

fn paint_algorithm_steps(
    bounds: Bounds,
    pane_state: &AttnResLabPaneState,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = Hsla::from_hex(ACCENT_GOLD);
    paint_panel_texture(bounds, accent, phase, paint);

    let active = pane_state.selected_sublayer % ALGO_STEPS.len();
    let sweep_height = 28.0;
    let sweep_bounds = Bounds::new(
        bounds.origin.x + 14.0,
        bounds.max_y() - sweep_height - 12.0,
        bounds.size.width - 28.0,
        sweep_height,
    );
    let top = bounds.origin.y + 34.0;
    let gap = 8.0;
    let row_height =
        ((sweep_bounds.origin.y - top - gap * 4.0) / ALGO_STEPS.len() as f32).max(32.0);

    for (index, (title, detail)) in ALGO_STEPS.iter().enumerate() {
        let y = top + index as f32 * (row_height + gap);
        let row_bounds = Bounds::new(
            bounds.origin.x + 12.0,
            y,
            bounds.size.width - 24.0,
            row_height,
        );
        let row_accent = if index == active {
            accent.with_alpha(0.24)
        } else {
            accent.with_alpha(0.06)
        };
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(row_accent)
                .with_border(
                    if index == active {
                        accent.with_alpha(0.42)
                    } else {
                        accent.with_alpha(0.12)
                    },
                    1.0,
                )
                .with_corner_radius(7.0),
        );
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                row_bounds.origin.x + 8.0,
                row_bounds.origin.y + 7.0,
                20.0,
                18.0,
            ))
            .with_background(if index == active {
                accent.with_alpha(0.32)
            } else {
                theme::bg::APP.with_alpha(0.72)
            })
            .with_corner_radius(4.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("{:02}", index + 1),
            Point::new(row_bounds.origin.x + 12.0, row_bounds.origin.y + 19.0),
            10.0,
            if index == active {
                theme::text::PRIMARY
            } else {
                accent.with_alpha(0.82)
            },
        ));
        let fitted_title = truncate_text_to_width(paint, title, row_bounds.size.width - 56.0, 11.0);
        paint.scene.draw_text(paint.text.layout(
            fitted_title.as_str(),
            Point::new(row_bounds.origin.x + 40.0, row_bounds.origin.y + 12.0),
            11.0,
            theme::text::PRIMARY,
        ));
        let fitted_detail =
            truncate_text_to_width(paint, detail, row_bounds.size.width - 56.0, 10.0);
        paint.scene.draw_text(paint.text.layout(
            fitted_detail.as_str(),
            Point::new(row_bounds.origin.x + 40.0, row_bounds.origin.y + 26.0),
            10.0,
            theme::text::MUTED,
        ));
    }

    paint.scene.draw_text(paint.text.layout_mono(
        "SUBLAYER SWEEP",
        Point::new(sweep_bounds.origin.x, sweep_bounds.origin.y - 4.0),
        10.0,
        accent.with_alpha(0.88),
    ));
    let mut sweep = AttnResLayerSweep::new(
        build_sublayer_levels(&pane_state.snapshot, phase),
        phase,
        accent,
        pane_state.playback_state.is_running(),
    );
    sweep.paint(sweep_bounds, paint);
}

fn paint_block_schedule(
    bounds: Bounds,
    pane_state: &AttnResLabPaneState,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = Hsla::from_hex(ACCENT_MINT);
    paint_panel_texture(bounds, accent, phase, paint);

    if pane_state.snapshot.block_summaries.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No block summaries loaded.",
            Point::new(bounds.origin.x + 14.0, bounds.origin.y + 36.0),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    let top = bounds.origin.y + 36.0;
    let gap = 8.0;
    let row_height = ((bounds.size.height - 52.0 - gap * 2.0)
        / pane_state.snapshot.block_summaries.len().max(1) as f32)
        .max(28.0);
    let max_block = pane_state.snapshot.block_summaries.len().saturating_sub(1);
    let levels = build_block_levels(&pane_state.snapshot, phase);

    for (index, block) in pane_state.snapshot.block_summaries.iter().enumerate() {
        let y = top + index as f32 * (row_height + gap);
        let row_bounds = Bounds::new(
            bounds.origin.x + 12.0,
            y,
            bounds.size.width - 24.0,
            row_height,
        );
        let is_active = block.block_index == pane_state.snapshot.active_block.min(max_block);
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(if is_active {
                    accent.with_alpha(0.18)
                } else {
                    Hsla::from_hex(0x071019).with_alpha(0.88)
                })
                .with_border(
                    if is_active {
                        accent.with_alpha(0.38)
                    } else {
                        accent.with_alpha(0.10)
                    },
                    1.0,
                )
                .with_corner_radius(6.0),
        );

        paint.scene.draw_text(paint.text.layout_mono(
            &format!("B{}", block.block_index),
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 18.0),
            10.0,
            if is_active {
                theme::text::PRIMARY
            } else {
                accent.with_alpha(0.88)
            },
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("{} layers", block.sublayers),
            Point::new(row_bounds.origin.x + 42.0, row_bounds.origin.y + 18.0),
            10.0,
            theme::text::MUTED,
        ));

        let select_track = Bounds::new(
            row_bounds.origin.x + 112.0,
            row_bounds.origin.y + 8.0,
            (row_bounds.size.width - 210.0).max(56.0),
            8.0,
        );
        let query_track = Bounds::new(
            select_track.origin.x,
            row_bounds.origin.y + row_height - 14.0,
            select_track.size.width,
            6.0,
        );
        paint.scene.draw_quad(
            Quad::new(select_track)
                .with_background(theme::bg::APP.with_alpha(0.88))
                .with_corner_radius(4.0),
        );
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                select_track.origin.x,
                select_track.origin.y,
                select_track.size.width * block.avg_selectivity.clamp(0.0, 1.0),
                select_track.size.height,
            ))
            .with_background(accent.with_alpha(0.82))
            .with_corner_radius(4.0),
        );
        paint.scene.draw_quad(
            Quad::new(query_track)
                .with_background(theme::bg::APP.with_alpha(0.78))
                .with_corner_radius(3.0),
        );
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                query_track.origin.x,
                query_track.origin.y,
                query_track.size.width * normalize_signal(block.avg_query_norm, 1.1),
                query_track.size.height,
            ))
            .with_background(Hsla::from_hex(ACCENT_GOLD).with_alpha(0.78))
            .with_corner_radius(3.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("{:>3.0}%", block.avg_selectivity * 100.0),
            Point::new(row_bounds.max_x() - 86.0, row_bounds.origin.y + 12.0),
            10.0,
            theme::text::PRIMARY,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("q {:.2}", block.avg_query_norm),
            Point::new(
                row_bounds.max_x() - 82.0,
                row_bounds.origin.y + row_height - 6.0,
            ),
            9.0,
            theme::text::MUTED,
        ));

        if let Some(level) = levels.get(index).copied() {
            paint.scene.draw_quad(
                Quad::new(Bounds::new(
                    row_bounds.origin.x + 92.0,
                    row_bounds.origin.y + 7.0,
                    4.0,
                    row_bounds.size.height - 14.0,
                ))
                .with_background(accent.with_alpha(0.16 + level * 0.4))
                .with_corner_radius(2.0),
            );
        }
    }
}

fn paint_route_metric_panel(
    bounds: Bounds,
    selected: Option<&AttnResLabSublayerSnapshot>,
    title: &str,
    use_logits: bool,
    phase: f32,
    paint: &mut PaintContext,
) {
    let Some(selected) = selected else {
        paint_panel_texture(bounds, Hsla::from_hex(ACCENT_CYAN), phase, paint);
        paint.scene.draw_text(paint.text.layout(
            "No sublayer selected.",
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 34.0),
            11.0,
            theme::text::MUTED,
        ));
        return;
    };

    let accent = if use_logits {
        Hsla::from_hex(ACCENT_GOLD)
    } else {
        Hsla::from_hex(ACCENT_CYAN)
    };
    paint_panel_texture(bounds, accent, phase, paint);
    let route_metric_summary = format!(
        "{} {} at {:.0}%.",
        if use_logits {
            "Score field favors"
        } else {
            "Routing mass favors"
        },
        selected.dominant_source_label,
        selected.dominant_weight * 100.0
    );
    let fitted_route_metric_summary = truncate_text_to_width(
        paint,
        route_metric_summary.as_str(),
        bounds.size.width - 24.0,
        10.0,
    );
    paint.scene.draw_text(paint.text.layout(
        fitted_route_metric_summary.as_str(),
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 30.0),
        10.0,
        theme::text::MUTED,
    ));
    paint_bar_series(
        Bounds::new(
            bounds.origin.x + 12.0,
            bounds.origin.y + 44.0,
            bounds.size.width - 24.0,
            bounds.size.height - 54.0,
        ),
        title,
        selected.source_labels.as_slice(),
        if use_logits {
            selected.source_logits.as_slice()
        } else {
            selected.routing_weights.as_slice()
        },
        accent,
        phase + 0.18,
        paint,
    );
}

fn paint_route_story(
    bounds: Bounds,
    selected: Option<&AttnResLabSublayerSnapshot>,
    phase: f32,
    paint: &mut PaintContext,
) {
    let Some(selected) = selected else {
        return;
    };

    let accent = Hsla::from_hex(ACCENT_CORAL);
    paint_panel_texture(bounds, accent, phase, paint);

    let compact = bounds.size.height < 128.0;
    let mut y = bounds.origin.y + 34.0;
    y = paint_panel_multiline_phrase(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "selected",
        selected.label.as_str(),
        1,
    );
    y = paint_panel_multiline_phrase(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y + 6.0,
        "story",
        selected.route_note.as_str(),
        if compact { 1 } else { 2 },
    );

    let timeline_top = (y + 8.0).min(bounds.max_y() - 48.0);
    paint_route_boundary_row(
        Bounds::new(
            bounds.origin.x + 12.0,
            timeline_top,
            bounds.size.width - 24.0,
            20.0,
        ),
        "before",
        selected.completed_blocks_before,
        selected.partial_block_present_before,
        selected.target_block,
        Hsla::from_hex(ACCENT_CYAN),
        paint,
    );
    paint_route_boundary_row(
        Bounds::new(
            bounds.origin.x + 12.0,
            timeline_top + 26.0,
            bounds.size.width - 24.0,
            20.0,
        ),
        "after",
        selected.completed_blocks_after,
        selected.partial_block_present_after,
        selected.target_block,
        Hsla::from_hex(ACCENT_MINT),
        paint,
    );
    let boundary_note = if selected.starts_new_block_before {
        "Boundary opened before this sublayer"
    } else {
        "Boundary remained inside the active block"
    };
    let fitted_boundary_note =
        truncate_mono_text_to_width(paint, boundary_note, bounds.size.width - 24.0, 10.0);

    paint.scene.draw_text(paint.text.layout_mono(
        fitted_boundary_note.as_str(),
        Point::new(
            bounds.origin.x + 12.0,
            (timeline_top + 58.0).min(bounds.max_y() - 10.0),
        ),
        10.0,
        theme::text::MUTED,
    ));
}

fn paint_inference_parity(
    bounds: Bounds,
    snapshot: &crate::app_state::AttnResLabSnapshot,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = Hsla::from_hex(ACCENT_CYAN);
    paint_panel_texture(bounds, accent, phase, paint);

    let hidden_level = parity_level(snapshot.inference.hidden_max_abs_diff, 1.0e-5, 6.0e-5);
    let logit_level = parity_level(snapshot.inference.logit_max_abs_diff, 1.0e-5, 6.0e-5);
    let mut hidden_ring = RingGauge::new()
        .level(hidden_level)
        .segments(42)
        .dot_size(4.0)
        .active_color(accent.with_alpha(0.88))
        .inactive_color(theme::bg::APP)
        .head_color(theme::text::PRIMARY);
    hidden_ring.paint(
        Bounds::new(bounds.origin.x + 14.0, bounds.origin.y + 42.0, 74.0, 74.0),
        paint,
    );

    let mut logit_ring = RingGauge::new()
        .level(logit_level)
        .segments(42)
        .dot_size(4.0)
        .active_color(Hsla::from_hex(ACCENT_MINT).with_alpha(0.88))
        .inactive_color(theme::bg::APP)
        .head_color(theme::text::PRIMARY);
    logit_ring.paint(
        Bounds::new(bounds.origin.x + 102.0, bounds.origin.y + 42.0, 74.0, 74.0),
        paint,
    );

    paint.scene.draw_text(paint.text.layout_mono(
        "HIDDEN",
        Point::new(bounds.origin.x + 18.0, bounds.origin.y + 126.0),
        10.0,
        accent.with_alpha(0.9),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        "LOGITS",
        Point::new(bounds.origin.x + 108.0, bounds.origin.y + 126.0),
        10.0,
        Hsla::from_hex(ACCENT_MINT).with_alpha(0.9),
    ));

    paint_signal_triplet(
        bounds.origin.x + 20.0,
        bounds.origin.y + 140.0,
        [
            ("HD", hidden_level, accent),
            ("LG", logit_level, Hsla::from_hex(ACCENT_MINT)),
            (
                "CF",
                snapshot.inference.block_cache_fill_share.clamp(0.0, 1.0),
                Hsla::from_hex(ACCENT_GOLD),
            ),
        ],
        accent,
        paint,
    );

    let mut y = bounds.origin.y + 42.0;
    let x = bounds.origin.x + 194.0;
    y = paint_panel_label_line(
        paint,
        bounds,
        x,
        y,
        "Hidden",
        snapshot.inference.hidden_parity_label.as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        x,
        y,
        "Hidden max abs",
        format!("{:.2e}", snapshot.inference.hidden_max_abs_diff).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        x,
        y,
        "Logits",
        snapshot.inference.logit_parity_label.as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        x,
        y,
        "Logit max abs",
        format!("{:.2e}", snapshot.inference.logit_max_abs_diff).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        x,
        y,
        "Merge Split",
        format!(
            "{:.0}% partial / {:.0}% cache",
            snapshot.inference.partial_merge_share * 100.0,
            snapshot.inference.cache_merge_share * 100.0
        )
        .as_str(),
    );
    let _ = paint_panel_label_line(
        paint,
        bounds,
        x,
        y,
        "Block Cache",
        format!(
            "{} cached // {:.0}% full",
            snapshot.inference.cached_blocks,
            snapshot.inference.block_cache_fill_share * 100.0
        )
        .as_str(),
    );
}

fn paint_two_phase_schedule(
    bounds: Bounds,
    snapshot: &crate::app_state::AttnResLabSnapshot,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = Hsla::from_hex(ACCENT_CORAL);
    paint_panel_texture(bounds, accent, phase, paint);

    let mut y = bounds.origin.y + 34.0;
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Prompt",
        format!("{} tokens", snapshot.inference.prompt_token_count).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Generated",
        format!("{} tokens", snapshot.inference.generated_token_count).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Sequence",
        format!("{} total", snapshot.inference.decoded_token_count).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Boundaries",
        join_labels(snapshot.inference.boundary_layers.as_slice()).as_str(),
    );
    let _ = paint_panel_multiline_phrase(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y + 8.0,
        "schedule",
        snapshot.inference.schedule_note.as_str(),
        2,
    );
}

fn paint_online_merge(
    bounds: Bounds,
    snapshot: &crate::app_state::AttnResLabSnapshot,
    selected: Option<&AttnResLabSublayerSnapshot>,
    phase: f32,
    paint: &mut PaintContext,
) {
    paint_panel_texture(bounds, Hsla::from_hex(ACCENT_GOLD), phase, paint);

    let rows = [
        (
            "partial",
            snapshot.inference.partial_merge_share,
            Hsla::from_hex(ACCENT_GOLD),
            phase,
        ),
        (
            "cache",
            snapshot.inference.cache_merge_share,
            Hsla::from_hex(ACCENT_CYAN),
            phase + 0.17,
        ),
    ];
    let gap = 8.0;
    let row_height = ((bounds.size.height - 86.0 - gap) / 2.0).max(24.0);
    for (index, (label, level, color, row_phase)) in rows.into_iter().enumerate() {
        let y = bounds.origin.y + 36.0 + index as f32 * (row_height + gap);
        paint_merge_meter(
            Bounds::new(
                bounds.origin.x + 12.0,
                y,
                bounds.size.width - 24.0,
                row_height,
            ),
            label,
            level,
            color,
            row_phase,
            paint,
        );
    }

    let merge_summary = selected
        .map(|sublayer| {
            format!(
                "{} stayed {} while cache carried {:.0}% of merge mass.",
                sublayer.label,
                sublayer.route_mode_label(),
                snapshot.inference.cache_merge_share * 100.0
            )
        })
        .unwrap_or_else(|| snapshot.inference.merge_note.clone());
    let _ = paint_panel_multiline_phrase(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        bounds.max_y() - 34.0,
        "merge",
        merge_summary.as_str(),
        1,
    );
}

fn paint_cache_health(
    bounds: Bounds,
    snapshot: &crate::app_state::AttnResLabSnapshot,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = Hsla::from_hex(ACCENT_MINT);
    paint_panel_texture(bounds, accent, phase, paint);

    let mut y = bounds.origin.y + 34.0;
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Cached blocks",
        format!("{}", snapshot.inference.cached_blocks).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Active block",
        format!("{}", snapshot.active_block).as_str(),
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Partial block",
        if snapshot.inference.partial_block_present {
            "present"
        } else {
            "closed"
        },
    );
    y = paint_panel_label_line(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y,
        "Fill",
        format!("{:.0}%", snapshot.inference.block_cache_fill_share * 100.0).as_str(),
    );
    let _ = paint_panel_multiline_phrase(
        paint,
        bounds,
        bounds.origin.x + 12.0,
        y + 8.0,
        "cache",
        snapshot.inference.cache_note.as_str(),
        1,
    );

    let track_bounds = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.max_y() - 26.0,
        bounds.size.width - 24.0,
        12.0,
    );
    paint.scene.draw_quad(
        Quad::new(track_bounds)
            .with_background(theme::bg::APP.with_alpha(0.88))
            .with_corner_radius(5.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            track_bounds.origin.x,
            track_bounds.origin.y,
            track_bounds.size.width * snapshot.inference.block_cache_fill_share.clamp(0.0, 1.0),
            track_bounds.size.height,
        ))
        .with_background(accent.with_alpha(0.84))
        .with_corner_radius(5.0),
    );
}

fn paint_merge_meter(
    bounds: Bounds,
    label: &str,
    level: f32,
    accent: Hsla,
    phase: f32,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(Hsla::from_hex(0x071019).with_alpha(0.82))
            .with_border(accent.with_alpha(0.12), 1.0)
            .with_corner_radius(6.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(
            bounds.origin.x + 10.0,
            bounds.origin.y + bounds.size.height * 0.54,
        ),
        10.0,
        accent.with_alpha(0.92),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("{:>3.0}%", level * 100.0),
        Point::new(
            bounds.origin.x + 62.0,
            bounds.origin.y + bounds.size.height * 0.54,
        ),
        10.0,
        theme::text::PRIMARY,
    ));

    let meter_height = (bounds.size.height - 10.0).max(12.0);
    let mut meter = SignalMeter::new()
        .bars(6)
        .gap(2.0)
        .level(level.clamp(0.0, 1.0))
        .min_bar_height(0.16)
        .active_color(accent.with_alpha(0.94))
        .inactive_color(theme::bg::APP.with_alpha(0.8));
    meter.paint(
        Bounds::new(
            bounds.origin.x + 108.0,
            bounds.origin.y + 5.0,
            20.0,
            meter_height,
        ),
        paint,
    );

    let mut ribbon = AttnResRibbon::new(
        build_ribbon_values(level.max(0.08), phase, 0.82),
        accent,
        level.clamp(0.0, 1.0),
        phase,
    );
    ribbon.paint(
        Bounds::new(
            bounds.origin.x + 140.0,
            bounds.origin.y + 7.0,
            (bounds.size.width - 152.0).max(44.0),
            (bounds.size.height - 14.0).max(10.0),
        ),
        paint,
    );
}

fn paint_bar_series(
    bounds: Bounds,
    title: &str,
    labels: &[String],
    values: &[f32],
    accent: Hsla,
    phase: f32,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(bounds.origin.x, bounds.origin.y - 4.0),
        10.0,
        accent.with_alpha(0.88),
    ));
    let top = bounds.origin.y + 12.0;
    let bar_height = 10.0;
    let gap = 14.0;
    let has_negative = values.iter().any(|value| *value < 0.0);
    let track_bounds = Bounds::new(
        bounds.origin.x + 66.0,
        top,
        bounds.size.width - 88.0,
        bounds.size.height - 20.0,
    );
    let dominant_index = values
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| {
            left.abs()
                .partial_cmp(&right.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(index, _)| index)
        .unwrap_or_default();
    let max_value = values
        .iter()
        .copied()
        .map(f32::abs)
        .fold(0.0_f32, f32::max)
        .max(1.0);
    let drawable_width = if has_negative {
        (track_bounds.size.width * 0.5 - 4.0).max(10.0)
    } else {
        track_bounds.size.width.max(10.0)
    };
    let axis_x = if has_negative {
        track_bounds.origin.x + track_bounds.size.width * 0.5
    } else {
        track_bounds.origin.x
    };
    if has_negative {
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                axis_x,
                top - 4.0,
                1.0,
                track_bounds.size.height,
            ))
            .with_background(accent.with_alpha(0.22)),
        );
    }

    for (index, value) in values.iter().enumerate() {
        let y = top + index as f32 * (bar_height + gap);
        paint.scene.draw_text(paint.text.layout_mono(
            labels.get(index).map(String::as_str).unwrap_or("source"),
            Point::new(bounds.origin.x, y + 2.0),
            9.0,
            theme::text::MUTED,
        ));
        let row_track = Bounds::new(
            track_bounds.origin.x,
            y,
            track_bounds.size.width,
            bar_height,
        );
        paint.scene.draw_quad(
            Quad::new(row_track)
                .with_background(theme::bg::APP.with_alpha(0.78))
                .with_corner_radius(3.0),
        );

        let normalized = (value.abs() / max_value).clamp(0.0, 1.0);
        let width = (drawable_width * normalized).max(2.0);
        let (bar_x, bar_width, bar_color) = if has_negative && *value < 0.0 {
            (
                (axis_x - width).max(row_track.origin.x),
                width.min(drawable_width),
                Hsla::from_hex(ACCENT_CORAL).with_alpha(0.72),
            )
        } else {
            (
                if has_negative {
                    axis_x
                } else {
                    row_track.origin.x
                },
                width.min(drawable_width),
                accent.with_alpha(0.72),
            )
        };
        let emphasis = if index == dominant_index {
            0.18 + phase * 0.10
        } else {
            0.0
        };
        paint.scene.draw_quad(
            Quad::new(Bounds::new(bar_x, y, bar_width, bar_height))
                .with_background(bar_color.with_alpha((0.72 + emphasis).clamp(0.0, 1.0)))
                .with_corner_radius(3.0),
        );
        if index == dominant_index {
            paint.scene.draw_quad(
                Quad::new(Bounds::new(bar_x, y, bar_width, 1.0))
                    .with_background(theme::text::PRIMARY.with_alpha(0.24 + emphasis)),
            );
        }
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("{value:.2}"),
            Point::new(bounds.max_x() - 38.0, y + 2.0),
            9.0,
            theme::text::PRIMARY,
        ));
    }
}

fn build_heatmap_data(snapshot: &crate::app_state::AttnResLabSnapshot) -> Vec<f32> {
    let rows = snapshot.sublayers.len().max(1);
    let cols = snapshot.max_sources();
    let mut data = vec![0.0; rows * cols];
    for (row, sublayer) in snapshot.sublayers.iter().enumerate() {
        for (col, value) in sublayer.routing_weights.iter().enumerate() {
            data[row * cols + col] = *value;
        }
    }
    data
}

fn paint_filter_like_button(bounds: Bounds, label: &str, active: bool, paint: &mut PaintContext) {
    if active {
        paint_secondary_button(bounds, label, paint);
    } else {
        paint_tertiary_button(bounds, label, paint);
    }
}

fn paint_panel_shell(bounds: Bounds, accent: Hsla, paint: &mut PaintContext) {
    wgpui::viz::panel::paint_shell(bounds, accent, paint);
}

fn paint_panel_title(bounds: Bounds, title: &str, accent: Hsla, paint: &mut PaintContext) {
    wgpui::viz::panel::paint_title(bounds, title, accent, paint);
}

fn paint_panel_texture(bounds: Bounds, accent: Hsla, phase: f32, paint: &mut PaintContext) {
    wgpui::viz::panel::paint_texture(bounds, accent, phase, paint);
}

fn paint_signal_triplet(
    origin_x: f32,
    origin_y: f32,
    specs: [(&str, f32, Hsla); 3],
    accent: Hsla,
    paint: &mut PaintContext,
) {
    for (index, (label, level, color)) in specs.into_iter().enumerate() {
        let x = origin_x + index as f32 * 42.0;
        let mut meter = SignalMeter::new()
            .bars(6)
            .gap(2.0)
            .level(level.clamp(0.0, 1.0))
            .min_bar_height(0.16)
            .active_color(color.with_alpha(0.94))
            .inactive_color(accent.with_alpha(0.08));
        meter.paint(Bounds::new(x, origin_y, 24.0, 54.0), paint);
        paint.scene.draw_text(paint.text.layout_mono(
            label,
            Point::new(x - 1.0, origin_y + 66.0),
            10.0,
            color.with_alpha(0.9),
        ));
    }
}

fn paint_route_boundary_row(
    bounds: Bounds,
    label: &str,
    completed_blocks: usize,
    partial_present: bool,
    target_block: usize,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x, bounds.origin.y + 14.0),
        10.0,
        accent.with_alpha(0.86),
    ));

    let slot_count = (target_block + 2)
        .max(completed_blocks + usize::from(partial_present))
        .max(3)
        .min(6);
    let lane_x = bounds.origin.x + 66.0;
    let gap = 6.0;
    let block_width = ((bounds.size.width - 98.0 - gap * slot_count as f32)
        / (slot_count as f32 + 1.0))
        .max(16.0);

    for index in 0..slot_count {
        let x = lane_x + index as f32 * (block_width + gap);
        let is_complete = index < completed_blocks;
        let is_target = index == target_block;
        let color = if is_complete {
            accent.with_alpha(0.66)
        } else {
            theme::bg::APP.with_alpha(0.88)
        };
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                x,
                bounds.origin.y,
                block_width,
                bounds.size.height,
            ))
            .with_background(color)
            .with_border(
                if is_target {
                    Hsla::from_hex(ACCENT_GOLD).with_alpha(0.42)
                } else {
                    accent.with_alpha(0.12)
                },
                1.0,
            )
            .with_corner_radius(4.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("B{index}"),
            Point::new(x + 4.0, bounds.origin.y + 13.0),
            9.0,
            if is_complete {
                theme::text::PRIMARY
            } else {
                theme::text::MUTED
            },
        ));
    }

    let partial_x = lane_x + slot_count as f32 * (block_width + gap);
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            partial_x,
            bounds.origin.y,
            block_width,
            bounds.size.height,
        ))
        .with_background(if partial_present {
            Hsla::from_hex(ACCENT_CORAL).with_alpha(0.62)
        } else {
            theme::bg::APP.with_alpha(0.78)
        })
        .with_border(Hsla::from_hex(ACCENT_CORAL).with_alpha(0.18), 1.0)
        .with_corner_radius(4.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        "P",
        Point::new(partial_x + 6.0, bounds.origin.y + 13.0),
        9.0,
        if partial_present {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        },
    ));
}

fn build_metric_history_ribbon(
    metrics: &[AttnResLabMetricPoint],
    kind: MetricHistoryKind,
    phase: f32,
) -> (Vec<f32>, f32) {
    if metrics.is_empty() {
        return (build_ribbon_values(0.18, phase, 0.9), 0.18);
    }

    let raw = metric_history_raw_values(metrics, kind);
    let invert = matches!(kind, MetricHistoryKind::Loss | MetricHistoryKind::Ema);
    let min = raw.iter().copied().fold(f32::INFINITY, f32::min);
    let max = raw.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let span = (max - min).max(0.0001);
    let mut values = Vec::with_capacity(RIBBON_SEGMENTS);
    let steps = raw.len().saturating_sub(1);

    for index in 0..RIBBON_SEGMENTS {
        let pos = index as f32 / (RIBBON_SEGMENTS.saturating_sub(1)) as f32;
        let sample_pos = pos * steps as f32;
        let low = sample_pos.floor() as usize;
        let high = sample_pos.ceil() as usize;
        let blend = sample_pos - low as f32;
        let sample = if steps == 0 {
            raw[0]
        } else {
            raw[low] + (raw[high] - raw[low]) * blend
        };
        let mut normalized = ((sample - min) / span).clamp(0.0, 1.0);
        if invert {
            normalized = 1.0 - normalized;
        }
        let envelope = 1.0 - ((pos - phase).abs() * 1.8).clamp(0.0, 1.0);
        values.push((0.08 + normalized * 0.68 + envelope * 0.12).clamp(0.0, 1.0));
    }

    let current = *raw.last().unwrap_or(&0.0);
    let mut current_level = ((current - min) / span).clamp(0.0, 1.0);
    if invert {
        current_level = 1.0 - current_level;
    }
    (values, current_level.clamp(0.0, 1.0))
}

fn metric_history_raw_values(
    metrics: &[AttnResLabMetricPoint],
    kind: MetricHistoryKind,
) -> Vec<f32> {
    match kind {
        MetricHistoryKind::Loss => metrics.iter().map(|point| point.training_loss).collect(),
        MetricHistoryKind::Ema => metrics.iter().map(|point| point.ema_loss).collect(),
        MetricHistoryKind::Selectivity => metrics.iter().map(|point| point.selectivity).collect(),
    }
}

fn sample_metric_series(raw: &[f32], sample_count: usize) -> Vec<f32> {
    sample_history_series(raw, sample_count)
}

fn build_selected_route_ribbon(selected: &AttnResLabSublayerSnapshot, phase: f32) -> Vec<f32> {
    if selected.routing_weights.is_empty() {
        return build_ribbon_values(0.18, phase, 0.94);
    }

    let steps = selected.routing_weights.len().saturating_sub(1);
    let mut values = Vec::with_capacity(RIBBON_SEGMENTS);
    for index in 0..RIBBON_SEGMENTS {
        let pos = index as f32 / (RIBBON_SEGMENTS.saturating_sub(1)) as f32;
        let sample_pos = pos * steps as f32;
        let low = sample_pos.floor() as usize;
        let high = sample_pos.ceil() as usize;
        let blend = sample_pos - low as f32;
        let base = if steps == 0 {
            selected.routing_weights[0]
        } else {
            selected.routing_weights[low]
                + (selected.routing_weights[high] - selected.routing_weights[low]) * blend
        };
        let wave = (((pos * 7.0) + phase * 5.0).sin() + 1.0) * 0.08;
        values.push((0.12 + base * 0.74 + wave).clamp(0.0, 1.0));
    }
    values
}

fn build_sublayer_levels(snapshot: &crate::app_state::AttnResLabSnapshot, phase: f32) -> Vec<f32> {
    snapshot
        .sublayers
        .iter()
        .enumerate()
        .map(|(index, sublayer)| {
            let pos = index as f32 / snapshot.sublayers.len().max(1) as f32;
            let wave = (((pos * 9.0) + phase * 6.0).sin() + 1.0) * 0.04;
            (0.14
                + sublayer.selectivity * 0.58
                + normalize_signal(sublayer.query_norm, 1.1) * 0.18
                + sublayer.dominant_weight * 0.10
                + wave)
                .clamp(0.0, 1.0)
        })
        .collect()
}

fn build_block_levels(snapshot: &crate::app_state::AttnResLabSnapshot, phase: f32) -> Vec<f32> {
    snapshot
        .block_summaries
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let pos = index as f32 / snapshot.block_summaries.len().max(1) as f32;
            let wave = (((pos * 6.0) + phase * 4.0).sin() + 1.0) * 0.04;
            (0.18
                + block.avg_selectivity * 0.56
                + normalize_signal(block.avg_query_norm, 1.1) * 0.18
                + wave)
                .clamp(0.0, 1.0)
        })
        .collect()
}

fn build_ribbon_values(level: f32, phase: f32, frequency: f32) -> Vec<f32> {
    (0..RIBBON_SEGMENTS)
        .map(|index| {
            let pos = index as f32 / (RIBBON_SEGMENTS.saturating_sub(1)) as f32;
            let wave = (((pos * frequency * 10.0) + phase * 6.0).sin() + 1.0) * 0.5;
            let envelope = 1.0 - ((pos - phase).abs() * 1.8).clamp(0.0, 1.0);
            (0.08 + level * 0.62 + wave * 0.2 + envelope * 0.16).clamp(0.0, 1.0)
        })
        .collect()
}

fn paint_panel_label_line(
    paint: &mut PaintContext,
    bounds: Bounds,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
) -> f32 {
    if remaining_panel_line_capacity(bounds, y) == 0 {
        return y;
    }

    let value_x = x + panel_value_x_offset(bounds, x, label);
    let max_width = (bounds.max_x() - value_x - PANEL_TEXT_RIGHT_PAD).max(24.0);
    paint.scene.draw_text(paint.text.layout(
        &format!("{label}:"),
        Point::new(x, y),
        theme::font_size::SM,
        theme::text::MUTED,
    ));
    let fitted_value = truncate_mono_text_to_width(paint, value, max_width, theme::font_size::SM);
    paint.scene.draw_text(paint.text.layout_mono(
        fitted_value.as_str(),
        Point::new(value_x, y),
        theme::font_size::SM,
        theme::text::PRIMARY,
    ));
    y + PANEL_LINE_HEIGHT
}

fn paint_panel_multiline_phrase(
    paint: &mut PaintContext,
    bounds: Bounds,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
    max_lines: usize,
) -> f32 {
    let capacity = remaining_panel_line_capacity(bounds, y).min(max_lines.max(1));
    if capacity == 0 {
        return y;
    }

    let value_x = x + panel_value_x_offset(bounds, x, label);
    let max_width = (bounds.max_x() - value_x - PANEL_TEXT_RIGHT_PAD).max(24.0);
    let chunk_len = mono_chunk_len_for_width(max_width, theme::font_size::SM);
    let lines = truncated_display_lines(value, chunk_len, capacity);

    paint.scene.draw_text(paint.text.layout(
        &format!("{label}:"),
        Point::new(x, y),
        theme::font_size::SM,
        theme::text::MUTED,
    ));

    let mut line_y = y;
    for line in lines {
        paint.scene.draw_text(paint.text.layout_mono(
            line.as_str(),
            Point::new(value_x, line_y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        ));
        line_y += PANEL_LINE_HEIGHT;
    }
    line_y
}

fn remaining_panel_line_capacity(bounds: Bounds, y: f32) -> usize {
    let available_height = bounds.max_y() - y - PANEL_TEXT_RIGHT_PAD;
    if available_height < 12.0 {
        0
    } else {
        (available_height / PANEL_LINE_HEIGHT).floor().max(1.0) as usize
    }
}

fn panel_value_x_offset(bounds: Bounds, x: f32, label: &str) -> f32 {
    let available = (bounds.max_x() - x - PANEL_TEXT_RIGHT_PAD).max(64.0);
    let preferred = (label.chars().count() as f32 * 6.2 + 18.0).clamp(72.0, 122.0);
    preferred.min((available * 0.42).max(56.0))
}

fn mono_chunk_len_for_width(width: f32, font_size: f32) -> usize {
    ((width / (font_size * 0.62)).floor() as usize).max(8)
}

fn truncated_display_lines(value: &str, chunk_len: usize, max_lines: usize) -> Vec<String> {
    let mut lines = split_text_for_display(value, chunk_len.max(1));
    if lines.len() <= max_lines {
        return lines;
    }

    lines.truncate(max_lines);
    if let Some(last) = lines.last_mut() {
        *last = ellipsize_to_chars(last, chunk_len.max(4));
    }
    lines
}

fn ellipsize_to_chars(value: &str, max_chars: usize) -> String {
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let prefix = value.chars().take(max_chars - 3).collect::<String>();
    format!("{prefix}...")
}

fn truncate_mono_text_to_width(
    paint: &mut PaintContext,
    value: &str,
    max_width: f32,
    font_size: f32,
) -> String {
    truncate_text_to_width_with_renderer(paint, value, max_width, font_size, true)
}

fn truncate_text_to_width(
    paint: &mut PaintContext,
    value: &str,
    max_width: f32,
    font_size: f32,
) -> String {
    truncate_text_to_width_with_renderer(paint, value, max_width, font_size, false)
}

fn truncate_text_to_width_with_renderer(
    paint: &mut PaintContext,
    value: &str,
    max_width: f32,
    font_size: f32,
    mono: bool,
) -> String {
    if value.is_empty() || max_width <= 0.0 {
        return String::new();
    }

    let measure = |candidate: &str, paint: &mut PaintContext| -> f32 {
        if mono {
            paint
                .text
                .layout_mono(candidate, Point::ZERO, font_size, theme::text::PRIMARY)
                .bounds()
                .size
                .width
        } else {
            paint
                .text
                .layout(candidate, Point::ZERO, font_size, theme::text::PRIMARY)
                .bounds()
                .size
                .width
        }
    };

    if measure(value, paint) <= max_width {
        return value.to_string();
    }
    if measure("...", paint) > max_width {
        return String::new();
    }

    let chars: Vec<char> = value.chars().collect();
    let mut low = 0usize;
    let mut high = chars.len();
    while low < high {
        let mid = (low + high + 1) / 2;
        let candidate = format!("{}...", chars[..mid].iter().collect::<String>());
        if measure(candidate.as_str(), paint) <= max_width {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    if low == 0 {
        String::from("...")
    } else {
        format!("{}...", chars[..low].iter().collect::<String>())
    }
}

fn join_labels(values: &[String]) -> String {
    if values.is_empty() {
        String::from("-")
    } else {
        values.join(", ")
    }
}

fn snapshot_route_regime(snapshot: &crate::app_state::AttnResLabSnapshot) -> &'static str {
    match snapshot.avg_selectivity {
        value if value < 0.18 => "uniform averaging",
        value if value < 0.36 => "forming preferences",
        _ => "selective routing",
    }
}

fn descent_level(start: Option<f32>, current: f32) -> f32 {
    let Some(start) = start else {
        return 0.0;
    };
    if start <= current {
        return 0.0;
    }
    normalize_signal(start - current, (start * 0.35).max(0.1))
}

fn mean_query_norm(snapshot: &crate::app_state::AttnResLabSnapshot) -> f32 {
    if snapshot.mean_query_norm > 0.0 {
        snapshot.mean_query_norm
    } else if snapshot.sublayers.is_empty() {
        0.0
    } else {
        snapshot
            .sublayers
            .iter()
            .map(|sublayer| sublayer.query_norm)
            .sum::<f32>()
            / snapshot.sublayers.len() as f32
    }
}

fn parity_level(value: f32, good_budget: f32, hard_budget: f32) -> f32 {
    if value <= good_budget {
        return 1.0;
    }
    if value >= hard_budget {
        return 0.0;
    }
    let normalized = ((value - good_budget) / (hard_budget - good_budget)).clamp(0.0, 1.0);
    (1.0 - normalized.sqrt()).clamp(0.0, 1.0)
}

fn normalize_signal(value: f32, reference: f32) -> f32 {
    if value <= 0.0 || reference <= 0.0 {
        return 0.0;
    }
    (value / reference).sqrt().clamp(0.0, 1.0)
}

fn animation_phase(pane_state: &AttnResLabPaneState) -> f32 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as f32;
    let seed = seed_from_text(
        format!(
            "{}:{}:{}:{}",
            pane_state.snapshot.run_label,
            pane_state.snapshot.step,
            pane_state.selected_sublayer,
            pane_state.selected_view.label()
        )
        .as_str(),
    );
    let speed_bias = pane_state.snapshot.speed_multiplier as f32 * 140.0;
    ((millis / (2800.0 - speed_bias.min(960.0))) + seed / 97.0).fract()
}

fn seed_from_text(value: &str) -> f32 {
    let mut sum = 0u64;
    for (index, byte) in value.bytes().enumerate() {
        sum = sum.saturating_add((index as u64 + 1) * u64::from(byte));
    }
    (sum % 4096) as f32
}

fn mesh_accent(pane_state: &AttnResLabPaneState) -> Hsla {
    if pane_state.last_error.is_some() {
        Hsla::from_hex(ACCENT_CORAL)
    } else {
        match pane_state.playback_state {
            AttnResLabPlaybackState::Armed => Hsla::from_hex(ACCENT_GOLD),
            AttnResLabPlaybackState::Running => Hsla::from_hex(ACCENT_CYAN),
            AttnResLabPlaybackState::Paused => Hsla::from_hex(ACCENT_GOLD),
            AttnResLabPlaybackState::Completed => Hsla::from_hex(ACCENT_MINT),
        }
    }
}

struct AttnResLayerSweep {
    levels: Vec<f32>,
    phase: f32,
    accent: Hsla,
    active: bool,
}

impl AttnResLayerSweep {
    fn new(levels: Vec<f32>, phase: f32, accent: Hsla, active: bool) -> Self {
        Self {
            levels,
            phase,
            accent,
            active,
        }
    }
}

impl Component for AttnResLayerSweep {
    fn paint(&mut self, bounds: Bounds, paint: &mut PaintContext) {
        if self.levels.is_empty() {
            return;
        }

        let count = self.levels.len();
        let gap = 3.0;
        let bar_width =
            ((bounds.size.width - gap * (count.saturating_sub(1) as f32)) / count as f32).max(1.0);
        let head_index = ((self.phase * count as f32).floor() as usize).min(count - 1);

        for (index, level) in self.levels.iter().copied().enumerate() {
            let height =
                (bounds.size.height * (0.16 + level * 0.84)).clamp(8.0, bounds.size.height);
            let x = bounds.origin.x + index as f32 * (bar_width + gap);
            let y = bounds.max_y() - height;
            let emphasis = if index == head_index {
                0.34
            } else if self.active && (index as isize - head_index as isize).abs() <= 1 {
                0.18
            } else {
                0.0
            };
            let color = self
                .accent
                .with_alpha((0.28 + level * 0.42 + emphasis).clamp(0.12, 0.96));

            paint.scene.draw_quad(
                Quad::new(Bounds::new(x, y, bar_width, height))
                    .with_background(color)
                    .with_corner_radius(2.0),
            );
            paint.scene.draw_quad(
                Quad::new(Bounds::new(x, y, bar_width, 2.0))
                    .with_background(theme::text::PRIMARY.with_alpha(0.22 + emphasis)),
            );
        }
    }
}

struct AttnResRibbon {
    values: Vec<f32>,
    color: Hsla,
    level: f32,
    phase: f32,
}

impl AttnResRibbon {
    fn new(values: Vec<f32>, color: Hsla, level: f32, phase: f32) -> Self {
        Self {
            values,
            color,
            level,
            phase,
        }
    }
}

impl Component for AttnResRibbon {
    fn paint(&mut self, bounds: Bounds, paint: &mut PaintContext) {
        if self.values.is_empty() {
            return;
        }

        paint.scene.draw_quad(
            Quad::new(bounds)
                .with_background(Hsla::from_hex(0x041018).with_alpha(0.9))
                .with_corner_radius(7.0),
        );

        let count = self.values.len();
        let gap = 2.0;
        let cell_width =
            ((bounds.size.width - gap * (count.saturating_sub(1) as f32)) / count as f32).max(1.0);
        let head_index = ((self.phase * count as f32).floor() as usize).min(count - 1);

        for (index, value) in self.values.iter().copied().enumerate() {
            let active_height =
                (bounds.size.height * (0.18 + value * 0.76)).clamp(3.0, bounds.size.height);
            let x = bounds.origin.x + index as f32 * (cell_width + gap);
            let y = bounds.max_y() - active_height;
            let emphasis = if index == head_index { 0.26 } else { 0.0 };
            paint.scene.draw_quad(
                Quad::new(Bounds::new(x, y, cell_width, active_height))
                    .with_background(self.color.with_alpha(
                        (0.16 + value * 0.56 + emphasis + self.level * 0.1).clamp(0.08, 0.96),
                    ))
                    .with_corner_radius(2.0),
            );
        }

        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                bounds.origin.y + bounds.size.height * (1.0 - self.level.clamp(0.0, 1.0)),
                bounds.size.width,
                1.0,
            ))
            .with_background(theme::text::PRIMARY.with_alpha(0.18)),
        );
    }
}

fn paint_help_overlay(
    content_bounds: Bounds,
    playback_state: AttnResLabPlaybackState,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(content_bounds)
            .with_background(Hsla::from_hex(0x02060B).with_alpha(0.72))
            .with_corner_radius(PANEL_RADIUS),
    );
    let overlay = Bounds::new(
        content_bounds.origin.x + content_bounds.size.width * 0.18,
        content_bounds.origin.y + content_bounds.size.height * 0.12,
        content_bounds.size.width * 0.64,
        content_bounds.size.height * 0.64,
    );
    let accent = Hsla::from_hex(ACCENT_GOLD);
    paint_panel_shell(overlay, accent, paint);
    paint_panel_title(overlay, "Controls", accent, paint);
    paint_panel_texture(overlay, accent, 0.32, paint);

    let mut y = overlay.origin.y + 38.0;
    y = paint_panel_multiline_phrase(
        paint,
        overlay,
        overlay.origin.x + 16.0,
        y,
        "Space",
        match playback_state {
            AttnResLabPlaybackState::Armed => "Start the current Psionic run",
            AttnResLabPlaybackState::Running => "Pause the current run",
            AttnResLabPlaybackState::Paused => "Resume the current run",
            AttnResLabPlaybackState::Completed => "Restart from the seeded model",
        },
        2,
    );
    y = paint_panel_multiline_phrase(
        paint,
        overlay,
        overlay.origin.x + 16.0,
        y + 10.0,
        "Up / Down",
        "Increase or decrease training speed (1x to 5x)",
        2,
    );
    y = paint_panel_multiline_phrase(
        paint,
        overlay,
        overlay.origin.x + 16.0,
        y + 10.0,
        "Left / Right",
        "Inspect the previous or next AttnRes sublayer",
        2,
    );
    y = paint_panel_multiline_phrase(
        paint,
        overlay,
        overlay.origin.x + 16.0,
        y + 10.0,
        "Tab or 1/2/3/4",
        "Cycle views or jump directly to Overview, Pipeline, Inference, or Loss",
        2,
    );
    y = paint_panel_multiline_phrase(
        paint,
        overlay,
        overlay.origin.x + 16.0,
        y + 10.0,
        "r",
        "Reset the pane to the seeded Psionic checkpoint",
        2,
    );
    y = paint_panel_multiline_phrase(
        paint,
        overlay,
        overlay.origin.x + 16.0,
        y + 10.0,
        "? / Esc",
        "Show or dismiss this overlay without closing the pane",
        2,
    );
    let _ = paint_panel_multiline_phrase(
        paint,
        overlay,
        overlay.origin.x + 16.0,
        y + 10.0,
        "Mouse",
        "Use the top-row controls for playback, speed, help, refresh, and sublayer inspection",
        2,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heatmap_data_matches_snapshot_shape() {
        let state = crate::app_state::AttnResLabPaneState::default();
        let data = build_heatmap_data(&state.snapshot);
        assert_eq!(
            data.len(),
            state.snapshot.sublayers.len() * state.snapshot.max_sources()
        );
        assert!(data.iter().all(|value| (0.0..=1.0).contains(value)));
    }

    #[test]
    fn metric_history_ribbon_is_bounded() {
        let state = crate::app_state::AttnResLabPaneState::default();
        let (values, level) = build_metric_history_ribbon(
            state.snapshot.metrics.as_slice(),
            MetricHistoryKind::Selectivity,
            0.42,
        );
        assert_eq!(values.len(), RIBBON_SEGMENTS);
        assert!(values.iter().all(|value| (0.0..=1.0).contains(value)));
        assert!((0.0..=1.0).contains(&level));
    }

    #[test]
    fn selected_route_ribbon_is_bounded() {
        let state = crate::app_state::AttnResLabPaneState::default();
        let values = build_selected_route_ribbon(
            state
                .current_sublayer()
                .expect("default pane should select a sublayer"),
            0.58,
        );
        assert_eq!(values.len(), RIBBON_SEGMENTS);
        assert!(values.iter().all(|value| (0.0..=1.0).contains(value)));
    }

    #[test]
    fn parity_level_rewards_lower_error() {
        assert!(parity_level(1.0e-6, 1.0e-5, 6.0e-5) > parity_level(2.0e-5, 1.0e-5, 6.0e-5));
        assert_eq!(parity_level(8.0e-5, 1.0e-5, 6.0e-5), 0.0);
    }

    #[test]
    fn normalize_signal_is_bounded() {
        assert_eq!(normalize_signal(0.0, 48.0), 0.0);
        assert!((0.0..=1.0).contains(&normalize_signal(24.0, 48.0)));
        assert_eq!(normalize_signal(9_999.0, 48.0), 1.0);
    }
}
