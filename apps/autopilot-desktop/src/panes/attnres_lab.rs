use wgpui::components::hud::{DotShape, DotsGrid, Heatmap, RingGauge, Scanlines, SignalMeter};
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::app_state::{AttnResLabPaneState, AttnResLabSublayerSnapshot, AttnResLabViewMode};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_secondary_button,
    paint_source_badge, paint_state_summary, paint_tertiary_button,
};
use crate::pane_system::{
    attnres_lab_inference_button_bounds, attnres_lab_next_sublayer_button_bounds,
    attnres_lab_overview_button_bounds, attnres_lab_pipeline_button_bounds,
    attnres_lab_previous_sublayer_button_bounds, attnres_lab_refresh_button_bounds,
};

const PANEL_RADIUS: f32 = 10.0;
const ACCENT_CYAN: u32 = 0x67E8F9;
const ACCENT_MINT: u32 = 0x86EFAC;
const ACCENT_GOLD: u32 = 0xFDE68A;
const ACCENT_CORAL: u32 = 0xFDA4AF;

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

pub fn paint(content_bounds: Bounds, pane_state: &AttnResLabPaneState, paint: &mut PaintContext) {
    paint_source_badge(
        content_bounds,
        pane_state.snapshot.source_badge.as_str(),
        paint,
    );

    let overview_bounds = attnres_lab_overview_button_bounds(content_bounds);
    let pipeline_bounds = attnres_lab_pipeline_button_bounds(content_bounds);
    let inference_bounds = attnres_lab_inference_button_bounds(content_bounds);
    let refresh_bounds = attnres_lab_refresh_button_bounds(content_bounds);
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
    paint_action_button(refresh_bounds, "Refresh live", paint);
    paint_action_button(previous_bounds, "Prev sublayer", paint);
    paint_action_button(next_bounds, "Next sublayer", paint);

    let title_x = refresh_bounds.max_x() + 18.0;
    paint.scene.draw_text(paint.text.layout(
        "AttnRes Lab",
        Point::new(title_x, content_bounds.origin.y + 18.0),
        18.0,
        theme::text::PRIMARY,
    ));
    let subtitle = if pane_state.snapshot.source_badge.starts_with("psionic.") {
        "Psionic-backed routing diagnostics, live inference, and two-phase parity."
    } else {
        "Replay-first WGPUI port of the original Burn/TUI information architecture."
    };
    paint.scene.draw_text(paint.text.layout(
        subtitle,
        Point::new(title_x, content_bounds.origin.y + 36.0),
        11.0,
        theme::text::MUTED,
    ));

    let summary = format!(
        "{} // step {}/{} // {}x",
        pane_state.snapshot.run_status,
        pane_state.snapshot.step,
        pane_state.snapshot.max_steps,
        pane_state.snapshot.speed_multiplier
    );
    let summary_bottom = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        overview_bounds.max_y() + 12.0,
        pane_state.load_state,
        summary.as_str(),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    let dashboard_top = summary_bottom + 12.0;
    let left_width = (content_bounds.size.width * 0.56).max(420.0);
    let left_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        dashboard_top,
        (left_width - 18.0).min(content_bounds.size.width - 24.0),
        (content_bounds.size.height * 0.52).max(260.0),
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
        (content_bounds.max_y() - left_bounds.max_y() - 24.0).max(160.0),
    );

    match pane_state.selected_view {
        AttnResLabViewMode::Overview => {
            paint_overview(left_bounds, right_bounds, bottom_bounds, pane_state, paint);
        }
        AttnResLabViewMode::Pipeline => {
            paint_pipeline(left_bounds, right_bounds, bottom_bounds, pane_state, paint);
        }
        AttnResLabViewMode::Inference => {
            paint_inference(left_bounds, right_bounds, bottom_bounds, pane_state, paint);
        }
    }
}

fn paint_overview(
    left_bounds: Bounds,
    right_bounds: Bounds,
    bottom_bounds: Bounds,
    pane_state: &AttnResLabPaneState,
    paint: &mut PaintContext,
) {
    let snapshot = &pane_state.snapshot;
    let selected = pane_state.current_sublayer();
    let accent = Hsla::from_hex(ACCENT_CYAN);

    paint_panel_shell(left_bounds, accent, paint);
    paint_panel_title(left_bounds, "Depth Routing Heatmap", accent, paint);
    paint_heatmap_panel(left_bounds, pane_state, accent, paint);

    let metrics_height = right_bounds.size.height * 0.48;
    let metrics_bounds = Bounds::new(
        right_bounds.origin.x,
        right_bounds.origin.y,
        right_bounds.size.width,
        metrics_height - 6.0,
    );
    let detail_bounds = Bounds::new(
        right_bounds.origin.x,
        metrics_bounds.max_y() + 12.0,
        right_bounds.size.width,
        (right_bounds.max_y() - metrics_bounds.max_y() - 12.0).max(120.0),
    );

    paint_panel_shell(metrics_bounds, Hsla::from_hex(ACCENT_MINT), paint);
    paint_panel_title(
        metrics_bounds,
        "Run Telemetry",
        Hsla::from_hex(ACCENT_MINT),
        paint,
    );
    paint_metrics_panel(metrics_bounds, snapshot, paint);

    paint_panel_shell(detail_bounds, Hsla::from_hex(ACCENT_GOLD), paint);
    paint_panel_title(
        detail_bounds,
        "Selected Sublayer",
        Hsla::from_hex(ACCENT_GOLD),
        paint,
    );
    paint_selected_sublayer(detail_bounds, selected, paint);

    paint_panel_shell(bottom_bounds, Hsla::from_hex(ACCENT_CORAL), paint);
    paint_panel_title(
        bottom_bounds,
        "Event Feed",
        Hsla::from_hex(ACCENT_CORAL),
        paint,
    );
    paint_event_feed(bottom_bounds, snapshot.events.as_slice(), paint);
}

fn paint_pipeline(
    left_bounds: Bounds,
    right_bounds: Bounds,
    bottom_bounds: Bounds,
    pane_state: &AttnResLabPaneState,
    paint: &mut PaintContext,
) {
    let selected = pane_state.current_sublayer();
    let accent = Hsla::from_hex(ACCENT_GOLD);

    let block_bounds = Bounds::new(
        left_bounds.origin.x,
        left_bounds.max_y() - 126.0,
        left_bounds.size.width,
        126.0,
    );
    let algo_bounds = Bounds::new(
        left_bounds.origin.x,
        left_bounds.origin.y,
        left_bounds.size.width,
        (block_bounds.origin.y - left_bounds.origin.y - 12.0).max(120.0),
    );
    let bars_height = right_bounds.size.height * 0.54;
    let bars_bounds = Bounds::new(
        right_bounds.origin.x,
        right_bounds.origin.y,
        right_bounds.size.width,
        bars_height - 6.0,
    );
    let story_bounds = Bounds::new(
        right_bounds.origin.x,
        bars_bounds.max_y() + 12.0,
        right_bounds.size.width,
        (right_bounds.max_y() - bars_bounds.max_y() - 12.0).max(120.0),
    );

    paint_panel_shell(algo_bounds, accent, paint);
    paint_panel_title(algo_bounds, "Algorithm Filmstrip", accent, paint);
    paint_algorithm_steps(algo_bounds, pane_state, paint);

    paint_panel_shell(block_bounds, Hsla::from_hex(ACCENT_MINT), paint);
    paint_panel_title(
        block_bounds,
        "Block Schedule",
        Hsla::from_hex(ACCENT_MINT),
        paint,
    );
    paint_block_schedule(block_bounds, pane_state, paint);

    paint_panel_shell(bars_bounds, Hsla::from_hex(ACCENT_CYAN), paint);
    paint_panel_title(
        bars_bounds,
        "Routing Logits And Mass",
        Hsla::from_hex(ACCENT_CYAN),
        paint,
    );
    paint_selected_route_bars(bars_bounds, selected, paint);

    paint_panel_shell(story_bounds, Hsla::from_hex(ACCENT_CORAL), paint);
    paint_panel_title(
        story_bounds,
        "Route Story",
        Hsla::from_hex(ACCENT_CORAL),
        paint,
    );
    paint_route_story(story_bounds, selected, paint);

    paint_panel_shell(bottom_bounds, Hsla::from_hex(ACCENT_CYAN), paint);
    paint_panel_title(
        bottom_bounds,
        "Event Feed",
        Hsla::from_hex(ACCENT_CYAN),
        paint,
    );
    paint_event_feed(bottom_bounds, pane_state.snapshot.events.as_slice(), paint);
}

fn paint_inference(
    left_bounds: Bounds,
    right_bounds: Bounds,
    bottom_bounds: Bounds,
    pane_state: &AttnResLabPaneState,
    paint: &mut PaintContext,
) {
    let snapshot = &pane_state.snapshot;
    let selected = pane_state.current_sublayer();
    let parity_bounds = Bounds::new(
        left_bounds.origin.x,
        left_bounds.origin.y,
        left_bounds.size.width,
        (left_bounds.size.height * 0.46).max(150.0),
    );
    let merge_bounds = Bounds::new(
        left_bounds.origin.x,
        parity_bounds.max_y() + 12.0,
        left_bounds.size.width,
        (left_bounds.max_y() - parity_bounds.max_y() - 12.0).max(120.0),
    );
    let detail_bounds = Bounds::new(
        right_bounds.origin.x,
        right_bounds.origin.y,
        right_bounds.size.width,
        right_bounds.size.height,
    );

    paint_panel_shell(parity_bounds, Hsla::from_hex(ACCENT_CYAN), paint);
    paint_panel_title(
        parity_bounds,
        "Two-Phase Parity",
        Hsla::from_hex(ACCENT_CYAN),
        paint,
    );
    paint_inference_parity(parity_bounds, snapshot, paint);

    paint_panel_shell(merge_bounds, Hsla::from_hex(ACCENT_GOLD), paint);
    paint_panel_title(
        merge_bounds,
        "Merge And Cache",
        Hsla::from_hex(ACCENT_GOLD),
        paint,
    );
    paint_merge_and_cache(merge_bounds, snapshot, paint);

    paint_panel_shell(detail_bounds, Hsla::from_hex(ACCENT_MINT), paint);
    paint_panel_title(
        detail_bounds,
        "Selected Detail",
        Hsla::from_hex(ACCENT_MINT),
        paint,
    );
    paint_selected_sublayer(detail_bounds, selected, paint);

    paint_panel_shell(bottom_bounds, Hsla::from_hex(ACCENT_CORAL), paint);
    paint_panel_title(
        bottom_bounds,
        "Inference Notes",
        Hsla::from_hex(ACCENT_CORAL),
        paint,
    );
    let mut y = bottom_bounds.origin.y + 30.0;
    y = paint_multiline_phrase(
        paint,
        bottom_bounds.origin.x + 12.0,
        y,
        "schedule",
        snapshot.inference.schedule_note.as_str(),
    );
    y = paint_multiline_phrase(
        paint,
        bottom_bounds.origin.x + 12.0,
        y + 6.0,
        "merge",
        snapshot.inference.merge_note.as_str(),
    );
    let _ = paint_multiline_phrase(
        paint,
        bottom_bounds.origin.x + 12.0,
        y + 6.0,
        "cache",
        snapshot.inference.cache_note.as_str(),
    );
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

    let legend = format!(
        "{} sublayers  //  {} max sources  //  active block {}",
        snapshot.sublayers.len(),
        snapshot.max_sources(),
        snapshot.active_block
    );
    paint.scene.draw_text(paint.text.layout_mono(
        legend.as_str(),
        Point::new(bounds.origin.x + 12.0, bounds.max_y() - 10.0),
        10.0,
        theme::text::MUTED,
    ));
}

fn paint_metrics_panel(
    bounds: Bounds,
    snapshot: &crate::app_state::AttnResLabSnapshot,
    paint: &mut PaintContext,
) {
    let left_x = bounds.origin.x + 12.0;
    let gauge_y = bounds.origin.y + 40.0;
    let progress = if snapshot.max_steps == 0 {
        0.0
    } else {
        snapshot.step as f32 / snapshot.max_steps as f32
    };
    let mut progress_ring = RingGauge::new()
        .level(progress.clamp(0.0, 1.0))
        .segments(42)
        .dot_size(5.0)
        .active_color(Hsla::from_hex(ACCENT_CYAN).with_alpha(0.88))
        .inactive_color(theme::bg::APP);
    progress_ring.paint(Bounds::new(left_x, gauge_y, 84.0, 84.0), paint);

    let mut selectivity_meter = SignalMeter::new()
        .bars(7)
        .gap(3.0)
        .level(snapshot.avg_selectivity.clamp(0.0, 1.0))
        .min_bar_height(0.16)
        .active_color(Hsla::from_hex(ACCENT_MINT).with_alpha(0.94))
        .inactive_color(theme::bg::APP.with_alpha(0.84));
    selectivity_meter.paint(
        Bounds::new(left_x + 102.0, gauge_y + 6.0, 34.0, 72.0),
        paint,
    );

    let mut y = bounds.origin.y + 42.0;
    let label_x = bounds.origin.x + 164.0;
    y = paint_label_line(paint, label_x, y, "Model", snapshot.model_label.as_str());
    y = paint_label_line(
        paint,
        label_x,
        y,
        "Topology",
        snapshot.architecture_label.as_str(),
    );
    y = paint_label_line(paint, label_x, y, "Run", snapshot.run_label.as_str());
    y = paint_label_line(
        paint,
        label_x,
        y,
        "Loss",
        format!("{:.3}", snapshot.training_loss).as_str(),
    );
    y = paint_label_line(
        paint,
        label_x,
        y,
        "EMA",
        format!("{:.3}", snapshot.ema_loss).as_str(),
    );
    y = paint_label_line(
        paint,
        label_x,
        y,
        "Selectivity",
        format!("{:.0}%", snapshot.avg_selectivity * 100.0).as_str(),
    );
    y = paint_label_line(
        paint,
        label_x,
        y,
        "Blocks",
        format!(
            "{} complete // fill {}",
            snapshot.completed_blocks, snapshot.current_block_fill
        )
        .as_str(),
    );

    let latest = snapshot
        .metrics
        .last()
        .map(|point| {
            format!(
                "last tracked point: step {} // loss {:.3} // selectivity {:.0}%",
                point.global_step,
                point.training_loss,
                point.selectivity * 100.0
            )
        })
        .unwrap_or_else(|| "no tracked points loaded".to_string());
    let _ = paint_multiline_phrase(paint, left_x, y + 10.0, "history", latest.as_str());
}

fn paint_selected_sublayer(
    bounds: Bounds,
    selected: Option<&AttnResLabSublayerSnapshot>,
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

    let mut y = bounds.origin.y + 36.0;
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Label",
        selected.label.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Kind",
        selected.kind_label.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Target block",
        format!("{}", selected.target_block).as_str(),
    );
    y = paint_label_line(
        paint,
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
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Query norm",
        format!("{:.2}", selected.query_norm).as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Selectivity",
        format!("{:.0}%", selected.selectivity * 100.0).as_str(),
    );
    y = paint_label_line(
        paint,
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
    let _ = paint_multiline_phrase(
        paint,
        bounds.origin.x + 12.0,
        y + 8.0,
        "route",
        selected.route_note.as_str(),
    );
}

fn paint_event_feed(bounds: Bounds, events: &[String], paint: &mut PaintContext) {
    let mut y = bounds.origin.y + 36.0;
    for event in events.iter().take(6) {
        paint.scene.draw_quad(
            Quad::new(Bounds::new(bounds.origin.x + 12.0, y - 2.0, 6.0, 6.0))
                .with_background(Hsla::from_hex(ACCENT_CORAL).with_alpha(0.82))
                .with_corner_radius(3.0),
        );
        paint.scene.draw_text(paint.text.layout(
            event.as_str(),
            Point::new(bounds.origin.x + 24.0, y),
            11.0,
            theme::text::PRIMARY,
        ));
        y += 20.0;
    }
}

fn paint_algorithm_steps(
    bounds: Bounds,
    pane_state: &AttnResLabPaneState,
    paint: &mut PaintContext,
) {
    let active = pane_state.selected_sublayer % ALGO_STEPS.len();
    let mut y = bounds.origin.y + 36.0;
    for (index, (title, detail)) in ALGO_STEPS.iter().enumerate() {
        let accent = if index == active {
            Hsla::from_hex(ACCENT_GOLD)
        } else {
            theme::text::MUTED
        };
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("{:02}", index + 1),
            Point::new(bounds.origin.x + 12.0, y),
            10.0,
            accent,
        ));
        paint.scene.draw_text(paint.text.layout(
            title,
            Point::new(bounds.origin.x + 42.0, y),
            11.0,
            theme::text::PRIMARY,
        ));
        paint.scene.draw_text(paint.text.layout(
            detail,
            Point::new(bounds.origin.x + 42.0, y + 14.0),
            10.0,
            theme::text::MUTED,
        ));
        y += 40.0;
    }
}

fn paint_block_schedule(
    bounds: Bounds,
    pane_state: &AttnResLabPaneState,
    paint: &mut PaintContext,
) {
    let mut y = bounds.origin.y + 36.0;
    for block in &pane_state.snapshot.block_summaries {
        y = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            format!("Block {}", block.block_index).as_str(),
            format!(
                "{:.0}% selective // q {:.2} // {} sublayers",
                block.avg_selectivity * 100.0,
                block.avg_query_norm,
                block.sublayers
            )
            .as_str(),
        );
    }
}

fn paint_selected_route_bars(
    bounds: Bounds,
    selected: Option<&AttnResLabSublayerSnapshot>,
    paint: &mut PaintContext,
) {
    let Some(selected) = selected else {
        return;
    };

    let left = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 36.0,
        (bounds.size.width * 0.48).max(120.0),
        bounds.size.height - 48.0,
    );
    let right = Bounds::new(
        left.max_x() + 12.0,
        left.origin.y,
        (bounds.max_x() - left.max_x() - 24.0).max(120.0),
        left.size.height,
    );
    paint_bar_series(
        left,
        "logits",
        selected.source_labels.as_slice(),
        selected.source_logits.as_slice(),
        Hsla::from_hex(ACCENT_GOLD),
        paint,
    );
    paint_bar_series(
        right,
        "weights",
        selected.source_labels.as_slice(),
        selected.routing_weights.as_slice(),
        Hsla::from_hex(ACCENT_CYAN),
        paint,
    );
}

fn paint_route_story(
    bounds: Bounds,
    selected: Option<&AttnResLabSublayerSnapshot>,
    paint: &mut PaintContext,
) {
    let Some(selected) = selected else {
        return;
    };

    let mut y = bounds.origin.y + 36.0;
    y = paint_multiline_phrase(
        paint,
        bounds.origin.x + 12.0,
        y,
        "selected",
        selected.label.as_str(),
    );
    y = paint_multiline_phrase(
        paint,
        bounds.origin.x + 12.0,
        y + 6.0,
        "story",
        selected.route_note.as_str(),
    );
    let _ = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y + 6.0,
        "boundaries",
        format!(
            "start={} complete {}->{} partial {}->{}",
            selected.starts_new_block_before,
            selected.completed_blocks_before,
            selected.completed_blocks_after,
            selected.partial_block_present_before,
            selected.partial_block_present_after
        )
        .as_str(),
    );
}

fn paint_inference_parity(
    bounds: Bounds,
    snapshot: &crate::app_state::AttnResLabSnapshot,
    paint: &mut PaintContext,
) {
    let mut hidden_ring = RingGauge::new()
        .level(0.98)
        .segments(42)
        .dot_size(5.0)
        .active_color(Hsla::from_hex(ACCENT_CYAN).with_alpha(0.88))
        .inactive_color(theme::bg::APP);
    hidden_ring.paint(
        Bounds::new(bounds.origin.x + 12.0, bounds.origin.y + 42.0, 78.0, 78.0),
        paint,
    );

    let mut logit_ring = RingGauge::new()
        .level(0.97)
        .segments(42)
        .dot_size(5.0)
        .active_color(Hsla::from_hex(ACCENT_MINT).with_alpha(0.88))
        .inactive_color(theme::bg::APP);
    logit_ring.paint(
        Bounds::new(bounds.origin.x + 104.0, bounds.origin.y + 42.0, 78.0, 78.0),
        paint,
    );

    let mut y = bounds.origin.y + 42.0;
    let x = bounds.origin.x + 200.0;
    y = paint_label_line(
        paint,
        x,
        y,
        "Hidden",
        snapshot.inference.hidden_parity_label.as_str(),
    );
    y = paint_label_line(
        paint,
        x,
        y,
        "Hidden max abs",
        format!("{:.2e}", snapshot.inference.hidden_max_abs_diff).as_str(),
    );
    y = paint_label_line(
        paint,
        x,
        y,
        "Logits",
        snapshot.inference.logit_parity_label.as_str(),
    );
    let _ = paint_label_line(
        paint,
        x,
        y,
        "Logit max abs",
        format!("{:.2e}", snapshot.inference.logit_max_abs_diff).as_str(),
    );
}

fn paint_merge_and_cache(
    bounds: Bounds,
    snapshot: &crate::app_state::AttnResLabSnapshot,
    paint: &mut PaintContext,
) {
    paint_merge_meter(
        Bounds::new(bounds.origin.x + 12.0, bounds.origin.y + 42.0, 110.0, 88.0),
        "partial",
        snapshot.inference.partial_merge_share,
        Hsla::from_hex(ACCENT_GOLD),
        paint,
    );
    paint_merge_meter(
        Bounds::new(bounds.origin.x + 132.0, bounds.origin.y + 42.0, 110.0, 88.0),
        "cache",
        snapshot.inference.cache_merge_share,
        Hsla::from_hex(ACCENT_CYAN),
        paint,
    );
    paint_merge_meter(
        Bounds::new(bounds.origin.x + 252.0, bounds.origin.y + 42.0, 110.0, 88.0),
        "fill",
        snapshot.inference.block_cache_fill_share,
        Hsla::from_hex(ACCENT_MINT),
        paint,
    );
}

fn paint_merge_meter(
    bounds: Bounds,
    label: &str,
    level: f32,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    let mut meter = SignalMeter::new()
        .bars(6)
        .gap(3.0)
        .level(level.clamp(0.0, 1.0))
        .min_bar_height(0.12)
        .active_color(accent.with_alpha(0.94))
        .inactive_color(theme::bg::APP.with_alpha(0.82));
    meter.paint(
        Bounds::new(bounds.origin.x + 16.0, bounds.origin.y, 32.0, 64.0),
        paint,
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 2.0, bounds.max_y() - 10.0),
        10.0,
        accent.with_alpha(0.9),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("{:>3.0}%", level * 100.0),
        Point::new(bounds.origin.x + 52.0, bounds.origin.y + 20.0),
        11.0,
        theme::text::PRIMARY,
    ));
}

fn paint_bar_series(
    bounds: Bounds,
    title: &str,
    labels: &[String],
    values: &[f32],
    accent: Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(bounds.origin.x, bounds.origin.y - 4.0),
        10.0,
        accent.with_alpha(0.88),
    ));
    let top = bounds.origin.y + 12.0;
    let bar_height = 12.0;
    let gap = 14.0;
    let max_value = values
        .iter()
        .copied()
        .map(f32::abs)
        .fold(0.0_f32, f32::max)
        .max(1.0);

    for (index, value) in values.iter().enumerate() {
        let y = top + index as f32 * (bar_height + gap);
        let normalized = value.abs() / max_value;
        let width = (bounds.size.width - 78.0) * normalized.clamp(0.0, 1.0);
        paint.scene.draw_text(paint.text.layout_mono(
            labels.get(index).map(String::as_str).unwrap_or("source"),
            Point::new(bounds.origin.x, y + 2.0),
            9.0,
            theme::text::MUTED,
        ));
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x + 64.0,
                y,
                width.max(2.0),
                bar_height,
            ))
            .with_background(accent.with_alpha(0.72))
            .with_corner_radius(2.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("{:.2}", value),
            Point::new(bounds.origin.x + 64.0 + width + 6.0, y + 2.0),
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
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(Hsla::from_hex(0x071019).with_alpha(0.96))
            .with_border(accent.with_alpha(0.26), 1.0)
            .with_corner_radius(PANEL_RADIUS),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 1.0,
            bounds.origin.y + 1.0,
            bounds.size.width - 2.0,
            20.0,
        ))
        .with_background(accent.with_alpha(0.06))
        .with_corner_radius(PANEL_RADIUS - 1.0),
    );
}

fn paint_panel_title(bounds: Bounds, title: &str, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 11.0),
        10.0,
        accent.with_alpha(0.9),
    ));
}
