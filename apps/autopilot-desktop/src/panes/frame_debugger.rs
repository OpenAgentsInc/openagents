use wgpui::components::hud::{RingGauge, Scanlines, SignalMeter};
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::app_state::{FrameDebuggerPaneState, FrameSample};
use crate::pane_renderer::{paint_label_line, paint_source_badge, paint_state_summary};

const PANEL_PAD: f32 = 12.0;
const GAUGE_CARD_HEIGHT: f32 = 156.0;
const TIMELINE_CARD_HEIGHT: f32 = 224.0;
const LEGEND_GAP: f32 = 14.0;
const FRAME_BUDGET_MS_60HZ: f32 = 16.67;
const FRAME_BUDGET_MS_30HZ: f32 = 33.34;

pub fn paint(
    content_bounds: Bounds,
    frame_debugger: &FrameDebuggerPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime+render", paint);
    let title_x = content_bounds.origin.x + PANEL_PAD;
    paint.scene.draw_text(paint.text.layout_mono(
        "FRAME DEBUGGER  //  LIVE REDRAW CADENCE + PERF HOTSPOTS",
        Point::new(title_x, content_bounds.origin.y + 16.0),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Rolling redraw cadence, pane paint hotspots, runtime pump timings, and current redraw drivers for the desktop loop.",
        Point::new(title_x, content_bounds.origin.y + 34.0),
        10.0,
        theme::text::MUTED,
    ));

    let summary = frame_debugger
        .rolling_fps
        .zip(frame_debugger.rolling_frame_interval_ms)
        .map(|(fps, interval_ms)| {
            format!("{fps:.1} redraw fps rolling // {interval_ms:.2} ms cadence")
        })
        .unwrap_or_else(|| "Waiting for first frame samples".to_string());
    let summary_bottom = paint_state_summary(
        paint,
        content_bounds.max_x() - 312.0,
        content_bounds.origin.y + 8.0,
        frame_debugger.load_state,
        summary.as_str(),
        frame_debugger.last_action.as_deref(),
        frame_debugger.last_error.as_deref(),
    );

    let top = (summary_bottom + 8.0).max(content_bounds.origin.y + 54.0);
    let left_width = (content_bounds.size.width * 0.56).max(520.0);
    let left_bounds = Bounds::new(
        content_bounds.origin.x + PANEL_PAD,
        top,
        left_width.min(content_bounds.size.width - PANEL_PAD * 2.0),
        GAUGE_CARD_HEIGHT,
    );
    let right_bounds = Bounds::new(
        left_bounds.max_x() + PANEL_PAD,
        top,
        content_bounds.max_x() - left_bounds.max_x() - PANEL_PAD * 2.0,
        GAUGE_CARD_HEIGHT,
    );
    let timeline_bounds = Bounds::new(
        content_bounds.origin.x + PANEL_PAD,
        left_bounds.max_y() + PANEL_PAD,
        content_bounds.size.width - PANEL_PAD * 2.0,
        TIMELINE_CARD_HEIGHT,
    );
    let footer_bounds = Bounds::new(
        content_bounds.origin.x + PANEL_PAD,
        timeline_bounds.max_y() + PANEL_PAD,
        content_bounds.size.width - PANEL_PAD * 2.0,
        content_bounds.max_y() - timeline_bounds.max_y() - PANEL_PAD * 2.0,
    );

    paint_panel_shell(left_bounds, theme::accent::PRIMARY.with_alpha(0.30), paint);
    paint_panel_shell(
        right_bounds,
        Hsla::from_hex(0x1D7A7A).with_alpha(0.30),
        paint,
    );
    paint_panel_shell(
        timeline_bounds,
        Hsla::from_hex(0xD97706).with_alpha(0.24),
        paint,
    );
    paint_panel_shell(
        footer_bounds,
        Hsla::from_hex(0x3B82F6).with_alpha(0.24),
        paint,
    );

    paint_gauges(left_bounds, frame_debugger, paint);
    paint_runtime_snapshot(right_bounds, frame_debugger, paint);
    paint_timeline(timeline_bounds, frame_debugger, paint);
    paint_footer(footer_bounds, frame_debugger, paint);
}

fn paint_gauges(bounds: Bounds, frame_debugger: &FrameDebuggerPaneState, paint: &mut PaintContext) {
    paint.scene.draw_text(paint.text.layout_mono(
        "REDRAW CADENCE",
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 12.0),
        10.0,
        theme::accent::PRIMARY,
    ));

    let fps_bounds = Bounds::new(bounds.origin.x + 16.0, bounds.origin.y + 30.0, 118.0, 118.0);
    let budget_bounds = Bounds::new(
        bounds.origin.x + 156.0,
        bounds.origin.y + 30.0,
        118.0,
        118.0,
    );
    let pressure_bounds = Bounds::new(bounds.origin.x + 294.0, bounds.origin.y + 42.0, 108.0, 96.0);

    let fps = frame_debugger.rolling_fps.unwrap_or_default();
    let mut fps_ring = RingGauge::new()
        .segments(48)
        .dot_size(4.0)
        .level((fps / 60.0).clamp(0.0, 1.0))
        .active_color(Hsla::from_hex(0x22C55E).with_alpha(0.88))
        .inactive_color(theme::bg::APP)
        .head_color(theme::text::PRIMARY);
    fps_ring.paint(fps_bounds, paint);

    let cpu_ms = frame_debugger
        .last_report
        .as_ref()
        .map(|sample| sample.total_cpu_ms)
        .unwrap_or_default();
    let budget_level = (cpu_ms / FRAME_BUDGET_MS_60HZ).clamp(0.0, 1.0);
    let mut budget_ring = RingGauge::new()
        .segments(36)
        .dot_size(4.0)
        .level(budget_level)
        .active_color(Hsla::from_hex(0xF97316).with_alpha(0.90))
        .inactive_color(theme::bg::APP)
        .head_color(theme::text::PRIMARY);
    budget_ring.paint(budget_bounds, paint);

    let slow_ratio = if frame_debugger.samples().is_empty() {
        0.0
    } else {
        frame_debugger
            .samples()
            .iter()
            .filter(|sample| sample.frame_interval_ms > FRAME_BUDGET_MS_60HZ)
            .count() as f32
            / frame_debugger.samples().len() as f32
    };
    let mut pressure_meter = SignalMeter::new()
        .bars(8)
        .gap(4.0)
        .min_bar_height(0.18)
        .level(slow_ratio)
        .active_color(Hsla::from_hex(0xEF4444).with_alpha(0.88))
        .inactive_color(theme::bg::APP.with_alpha(0.84));
    pressure_meter.paint(pressure_bounds, paint);

    paint.scene.draw_text(paint.text.layout_mono(
        &format!("{fps:>5.1} fps"),
        Point::new(fps_bounds.origin.x + 22.0, fps_bounds.max_y() - 12.0),
        10.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("{cpu_ms:>5.2} ms cpu"),
        Point::new(budget_bounds.origin.x + 10.0, budget_bounds.max_y() - 12.0),
        10.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("{:.0}% slow", slow_ratio * 100.0),
        Point::new(
            pressure_bounds.origin.x + 18.0,
            pressure_bounds.max_y() - 12.0,
        ),
        10.0,
        theme::text::PRIMARY,
    ));

    let mut y = bounds.origin.y + 30.0;
    let stats_x = bounds.origin.x + 424.0;
    y = paint_label_line(
        paint,
        stats_x,
        y,
        "Last interval",
        &format_ms(frame_debugger.last_frame_interval_ms),
    );
    y = paint_label_line(
        paint,
        stats_x,
        y,
        "Rolling interval",
        &format_ms(frame_debugger.rolling_frame_interval_ms),
    );
    y = paint_label_line(
        paint,
        stats_x,
        y,
        "Redraw requests",
        &frame_debugger.redraw_requests.to_string(),
    );
    y = paint_label_line(
        paint,
        stats_x,
        y,
        "Frames sampled",
        &frame_debugger.total_frames.to_string(),
    );
    y = paint_label_line(
        paint,
        stats_x,
        y,
        "Slow > 16.7ms",
        &frame_debugger.slow_frames_60hz.to_string(),
    );
    let _ = paint_label_line(
        paint,
        stats_x,
        y,
        "Slow > 33.3ms",
        &frame_debugger.slow_frames_30hz.to_string(),
    );
}

fn paint_runtime_snapshot(
    bounds: Bounds,
    frame_debugger: &FrameDebuggerPaneState,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        "REDRAW DRIVERS",
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 12.0),
        10.0,
        Hsla::from_hex(0x67E8F9),
    ));

    let snapshot = &frame_debugger.redraw_pressure;
    let mut y = bounds.origin.y + 30.0;
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Should redraw",
        bool_label(snapshot.should_redraw),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Reasons",
        snapshot.reason_summary().as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Poll interval",
        &format!("{}ms", snapshot.poll_interval_ms),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Debugger probe",
        bool_label(snapshot.debug_probe_active),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Provider heartbeat",
        bool_label(snapshot.provider_online_heartbeat),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Provider HUD",
        snapshot.provider_control_hud.state_summary().as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Rive Preview",
        snapshot.rive_preview.state_summary().as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Presentation",
        snapshot.presentation.state_summary().as_str(),
    );

    let counters = &frame_debugger.redraw_reason_counters;
    let _ = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y + 10.0,
        "Driver counts",
        &format!(
            "bg {} hotbar {} provider {} provider_heartbeat {} chat {} debug {} text {} provider_hud {} rive {} present {}",
            counters.background_changed,
            counters.hotbar_flashing,
            counters.provider_animating,
            counters.provider_online_heartbeat,
            counters.chat_pending,
            counters.debug_probe_active,
            counters.text_input_focused,
            counters.provider_control_hud,
            counters.rive_preview,
            counters.presentation
        ),
    );

    let pane_timings = frame_debugger.top_pane_paint_summaries(1);
    let runtime_timings = frame_debugger.top_runtime_pump_summaries(1);
    let snapshot_timings = frame_debugger.top_snapshot_timing_summaries(1);
    let y = y + 26.0;
    let y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Top pane paint",
        pane_timings
            .first()
            .map(|entry| {
                format!(
                    "{} [{}] {:.2}ms total",
                    entry.pane_title.as_str(),
                    entry.render_mode.as_str(),
                    entry.total_ms
                )
            })
            .unwrap_or_else(|| "-".to_string())
            .as_str(),
    );
    let y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Top runtime pump",
        runtime_timings
            .first()
            .map(|entry| format!("{} {:.2}ms total", entry.operation.as_str(), entry.total_ms))
            .unwrap_or_else(|| "-".to_string())
            .as_str(),
    );
    let _ = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "Top snapshot",
        snapshot_timings
            .first()
            .map(|entry| {
                format!(
                    "{}:{} {:.2}ms",
                    entry.subsystem.as_str(),
                    entry.phase.as_str(),
                    entry.total_ms
                )
            })
            .unwrap_or_else(|| "-".to_string())
            .as_str(),
    );
}

fn paint_timeline(
    bounds: Bounds,
    frame_debugger: &FrameDebuggerPaneState,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        "FRAME TIMELINE  //  interval height with stacked cpu phases",
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 12.0),
        10.0,
        Hsla::from_hex(0xF59E0B),
    ));

    let chart_bounds = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 28.0,
        bounds.size.width - 24.0,
        bounds.size.height - 58.0,
    );
    let mut scanlines = Scanlines::new()
        .spacing(12.0)
        .line_color(Hsla::from_hex(0xF59E0B).with_alpha(0.05))
        .scan_color(Hsla::from_hex(0xF59E0B).with_alpha(0.12))
        .scan_width(14.0)
        .scan_progress(0.55)
        .opacity(0.85);
    scanlines.paint(chart_bounds, paint);

    let max_ms = frame_debugger
        .samples()
        .iter()
        .fold(FRAME_BUDGET_MS_30HZ, |max_ms, sample| {
            max_ms.max(sample.frame_interval_ms)
        })
        .max(FRAME_BUDGET_MS_30HZ);

    paint_threshold_line(
        chart_bounds,
        FRAME_BUDGET_MS_60HZ,
        max_ms,
        Hsla::from_hex(0x22C55E),
        paint,
    );
    paint_threshold_line(
        chart_bounds,
        FRAME_BUDGET_MS_30HZ,
        max_ms,
        Hsla::from_hex(0xEF4444),
        paint,
    );

    let samples = frame_debugger.samples();
    if samples.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No frame samples yet.",
            Point::new(chart_bounds.origin.x + 12.0, chart_bounds.origin.y + 24.0),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    let count = samples.len();
    let bar_gap = 2.0;
    let bar_width = ((chart_bounds.size.width - bar_gap * (count.saturating_sub(1) as f32))
        / count as f32)
        .max(1.0);
    for (index, sample) in samples.iter().enumerate() {
        let x = chart_bounds.origin.x + index as f32 * (bar_width + bar_gap);
        paint_frame_sample(
            Bounds::new(
                x,
                chart_bounds.origin.y,
                bar_width,
                chart_bounds.size.height,
            ),
            sample,
            max_ms,
            paint,
        );
    }

    paint_legend(bounds, paint);
}

fn paint_footer(bounds: Bounds, frame_debugger: &FrameDebuggerPaneState, paint: &mut PaintContext) {
    paint.scene.draw_text(paint.text.layout_mono(
        "LAST FRAME + HOTSPOTS",
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 12.0),
        10.0,
        Hsla::from_hex(0x60A5FA),
    ));

    let Some(sample) = frame_debugger.last_report.as_ref() else {
        paint.scene.draw_text(paint.text.layout(
            "Waiting for render metrics.",
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 30.0),
            10.0,
            theme::text::MUTED,
        ));
        return;
    };

    let left_x = bounds.origin.x + 12.0;
    let metrics_x = bounds.origin.x + bounds.size.width * 0.26;
    let pane_x = bounds.origin.x + bounds.size.width * 0.52;
    let runtime_x = bounds.origin.x + bounds.size.width * 0.76;
    let mut left_y = bounds.origin.y + 30.0;
    paint.scene.draw_text(paint.text.layout_mono(
        "FRAME PHASES",
        Point::new(left_x, left_y - 4.0),
        9.0,
        theme::accent::PRIMARY,
    ));
    left_y += 12.0;
    left_y = paint_label_line(
        paint,
        left_x,
        left_y,
        "Scene build",
        &format!("{:.2}ms", sample.scene_build_ms),
    );
    left_y = paint_label_line(
        paint,
        left_x,
        left_y,
        "Surface acquire",
        &format!("{:.2}ms", sample.surface_acquire_ms),
    );
    left_y = paint_label_line(
        paint,
        left_x,
        left_y,
        "Renderer prepare",
        &format!("{:.2}ms", sample.prepare_cpu_ms),
    );
    let _ = paint_label_line(
        paint,
        left_x,
        left_y,
        "Renderer render",
        &format!("{:.2}ms", sample.render_cpu_ms),
    );

    let mut metrics_y = bounds.origin.y + 42.0;
    paint.scene.draw_text(paint.text.layout_mono(
        "RENDER METRICS",
        Point::new(metrics_x, bounds.origin.y + 26.0),
        9.0,
        Hsla::from_hex(0x93C5FD),
    ));
    metrics_y = paint_label_line(
        paint,
        metrics_x,
        metrics_y,
        "Submit + present",
        &format!("{:.2}ms", sample.submit_present_ms),
    );
    metrics_y = paint_label_line(
        paint,
        metrics_x,
        metrics_y,
        "Draw calls",
        &sample.draw_calls.to_string(),
    );
    metrics_y = paint_label_line(
        paint,
        metrics_x,
        metrics_y,
        "Layers / vectors",
        &format!("{} / {}", sample.layer_count, sample.vector_batches),
    );
    metrics_y = paint_label_line(
        paint,
        metrics_x,
        metrics_y,
        "Images / SVGs",
        &format!("{} / {}", sample.image_instances, sample.svg_instances),
    );
    let _ = paint_label_line(
        paint,
        metrics_x,
        metrics_y,
        "SVG cache",
        &sample.svg_cache_size.to_string(),
    );

    paint_timing_summary_block(
        Bounds::new(
            pane_x,
            bounds.origin.y + 26.0,
            (runtime_x - pane_x - 12.0).max(120.0),
            bounds.size.height - 38.0,
        ),
        "TOP PANE PAINT",
        frame_debugger
            .top_pane_paint_summaries(3)
            .iter()
            .map(|entry| {
                format!(
                    "{} [{}] tot {:.2}ms max {:.2}ms",
                    entry.pane_title.as_str(),
                    entry.render_mode.as_str(),
                    entry.total_ms,
                    entry.max_ms
                )
            })
            .collect::<Vec<_>>(),
        paint,
    );

    let runtime_lines = frame_debugger
        .top_runtime_pump_summaries(3)
        .iter()
        .map(|entry| {
            format!(
                "{} {:.2}ms tot // max {:.2}ms",
                entry.operation.as_str(),
                entry.total_ms,
                entry.max_ms
            )
        })
        .collect::<Vec<_>>();
    let snapshot_lines = frame_debugger
        .top_snapshot_timing_summaries(3)
        .iter()
        .map(|entry| {
            format!(
                "{}:{} {:.2}ms tot",
                entry.subsystem.as_str(),
                entry.phase.as_str(),
                entry.total_ms
            )
        })
        .collect::<Vec<_>>();
    paint_timing_summary_block(
        Bounds::new(
            runtime_x,
            bounds.origin.y + 26.0,
            bounds.max_x() - runtime_x - 12.0,
            ((bounds.size.height - 44.0) * 0.5).max(56.0),
        ),
        "TOP RUNTIME PUMPS",
        runtime_lines,
        paint,
    );
    paint_timing_summary_block(
        Bounds::new(
            runtime_x,
            bounds.origin.y + bounds.size.height * 0.54,
            bounds.max_x() - runtime_x - 12.0,
            bounds.max_y() - (bounds.origin.y + bounds.size.height * 0.54) - 12.0,
        ),
        "SNAPSHOT TIMINGS",
        snapshot_lines,
        paint,
    );
}

fn paint_panel_shell(bounds: Bounds, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(accent, 1.0)
            .with_corner_radius(10.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 1.0,
            bounds.origin.y + 1.0,
            (bounds.size.width - 2.0).max(0.0),
            26.0,
        ))
        .with_background(accent.with_alpha(0.08))
        .with_corner_radius(9.0),
    );
}

fn paint_threshold_line(
    chart_bounds: Bounds,
    threshold_ms: f32,
    max_ms: f32,
    color: Hsla,
    paint: &mut PaintContext,
) {
    let y =
        chart_bounds.max_y() - (threshold_ms / max_ms).clamp(0.0, 1.0) * chart_bounds.size.height;
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            chart_bounds.origin.x,
            y,
            chart_bounds.size.width,
            1.0,
        ))
        .with_background(color.with_alpha(0.35)),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("{threshold_ms:.1}ms"),
        Point::new(chart_bounds.origin.x + 4.0, y - 10.0),
        8.0,
        color.with_alpha(0.88),
    ));
}

fn paint_frame_sample(bounds: Bounds, sample: &FrameSample, max_ms: f32, paint: &mut PaintContext) {
    let interval_ratio = (sample.frame_interval_ms / max_ms).clamp(0.0, 1.0);
    let total_height = bounds.size.height * interval_ratio;
    let bar_bounds = Bounds::new(
        bounds.origin.x,
        bounds.max_y() - total_height,
        bounds.size.width,
        total_height.max(1.0),
    );
    let interval_color = if sample.frame_interval_ms > FRAME_BUDGET_MS_30HZ {
        Hsla::from_hex(0xEF4444).with_alpha(0.36)
    } else if sample.frame_interval_ms > FRAME_BUDGET_MS_60HZ {
        Hsla::from_hex(0xF59E0B).with_alpha(0.34)
    } else {
        Hsla::from_hex(0x22C55E).with_alpha(0.24)
    };
    paint
        .scene
        .draw_quad(Quad::new(bar_bounds).with_background(interval_color));

    let phase_total = sample.scene_build_ms
        + sample.surface_acquire_ms
        + sample.prepare_cpu_ms
        + sample.render_cpu_ms
        + sample.submit_present_ms;
    if phase_total <= f32::EPSILON {
        return;
    }

    let phase_scale = if phase_total > sample.frame_interval_ms && phase_total > f32::EPSILON {
        sample.frame_interval_ms / phase_total
    } else {
        1.0
    };
    let mut cursor_y = bar_bounds.max_y();
    for (value, color) in [
        (
            sample.scene_build_ms,
            Hsla::from_hex(0x06B6D4).with_alpha(0.82),
        ),
        (
            sample.surface_acquire_ms,
            Hsla::from_hex(0x8B5CF6).with_alpha(0.82),
        ),
        (
            sample.prepare_cpu_ms,
            Hsla::from_hex(0x14B8A6).with_alpha(0.88),
        ),
        (
            sample.render_cpu_ms,
            Hsla::from_hex(0xF8FAFC).with_alpha(0.92),
        ),
        (
            sample.submit_present_ms,
            Hsla::from_hex(0xFB923C).with_alpha(0.88),
        ),
    ] {
        let height = (bounds.size.height * ((value * phase_scale) / max_ms)).max(1.0);
        cursor_y -= height;
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                cursor_y.max(bar_bounds.origin.y),
                bounds.size.width,
                height.min(cursor_y + height - bar_bounds.origin.y).max(1.0),
            ))
            .with_background(color),
        );
    }
}

fn paint_legend(bounds: Bounds, paint: &mut PaintContext) {
    let legend_y = bounds.max_y() - 16.0;
    let mut x = bounds.origin.x + 14.0;
    for (label, color) in [
        ("scene", Hsla::from_hex(0x06B6D4).with_alpha(0.82)),
        ("acquire", Hsla::from_hex(0x8B5CF6).with_alpha(0.82)),
        ("prepare", Hsla::from_hex(0x14B8A6).with_alpha(0.88)),
        ("render", Hsla::from_hex(0xF8FAFC).with_alpha(0.92)),
        ("present", Hsla::from_hex(0xFB923C).with_alpha(0.88)),
    ] {
        paint.scene.draw_quad(
            Quad::new(Bounds::new(x, legend_y, 10.0, 10.0))
                .with_background(color)
                .with_corner_radius(2.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            label,
            Point::new(x + 14.0, legend_y + 1.0),
            8.0,
            theme::text::MUTED,
        ));
        x += 14.0 + label.len() as f32 * 6.2 + LEGEND_GAP;
    }
}

fn paint_timing_summary_block(
    bounds: Bounds,
    title: &str,
    lines: Vec<String>,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(bounds.origin.x, bounds.origin.y),
        9.0,
        Hsla::from_hex(0xBFDBFE),
    ));
    if lines.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No timing samples yet.",
            Point::new(bounds.origin.x, bounds.origin.y + 16.0),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }
    let mut y = bounds.origin.y + 16.0;
    for line in lines {
        if y > bounds.max_y() - 12.0 {
            break;
        }
        paint.scene.draw_text(paint.text.layout_mono(
            &line,
            Point::new(bounds.origin.x, y),
            9.0,
            theme::text::SECONDARY,
        ));
        y += 14.0;
    }
}

fn format_ms(value: Option<f32>) -> String {
    value
        .map(|value| format!("{value:.2}ms"))
        .unwrap_or_else(|| "-".to_string())
}

fn bool_label(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use super::paint;
    use crate::app_state::{
        FrameDebuggerPaneState, FrameRenderReport, PanePaintTimingSample, RuntimePumpTimingSample,
        SnapshotTimingSample,
    };
    use wgpui::{Bounds, PaintContext, Scene, TextSystem};

    #[test]
    fn frame_debugger_paint_accepts_recorded_samples() {
        let mut state = FrameDebuggerPaneState::default();
        state.record_frame(FrameRenderReport {
            scene_build_ms: 4.0,
            prepare_cpu_ms: 2.0,
            render_cpu_ms: 1.0,
            total_cpu_ms: 8.0,
            draw_calls: 12,
            layer_count: 3,
            pane_paint_samples: vec![PanePaintTimingSample {
                pane_kind: "ProviderControl".to_string(),
                pane_title: "Provider Control".to_string(),
                render_mode: "full".to_string(),
                active: true,
                elapsed_ms: 1.8,
            }],
            ..FrameRenderReport::default()
        });
        state.record_runtime_pump_sample(RuntimePumpTimingSample {
            cadence: "every_loop".to_string(),
            operation: "desktop_control::drain_runtime_updates".to_string(),
            changed: true,
            elapsed_ms: 0.8,
        });
        state.record_snapshot_timing_sample(SnapshotTimingSample {
            subsystem: "desktop_control".to_string(),
            phase: "sync_snapshot".to_string(),
            synced: true,
            success: true,
            elapsed_ms: 0.6,
        });
        let mut scene = Scene::new();
        let mut text_system = TextSystem::new(1.0);
        let mut paint_context = PaintContext::new(&mut scene, &mut text_system, 1.0);

        paint(
            Bounds::new(0.0, 0.0, 1120.0, 620.0),
            &state,
            &mut paint_context,
        );
    }
}
