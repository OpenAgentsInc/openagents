use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wgpui::components::hud::{DotShape, DotsGrid, Heatmap, RingGauge, Scanlines, SignalMeter};
use wgpui::viz::panel::paint_shell as paint_viz_panel_shell;
use wgpui::viz::theme as viz_theme;
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::app_state::LocalInferencePaneState;
use crate::local_inference_runtime::{
    LocalInferenceExecutionMetrics, LocalInferenceExecutionProvenance,
    LocalInferenceExecutionSnapshot,
};
use crate::pane_renderer::{
    paint_label_line, paint_multiline_phrase, paint_source_badge, paint_state_summary,
};

const LATTICE_ROWS: usize = 12;
const LATTICE_COLS: usize = 24;
const SYNTHETIC_LAYER_BANDS: usize = 24;
const RIBBON_SEGMENTS: usize = 32;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &LocalInferencePaneState,
    runtime: &LocalInferenceExecutionSnapshot,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime+viz", paint);

    let phase = animation_phase(runtime, pane_state);
    let active = runtime.busy || pane_state.pending_request_id.is_some();
    let accent = mesh_accent(active, runtime.artifact_present, runtime.is_ready());

    let field_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        content_bounds.origin.y + 34.0,
        content_bounds.size.width * 0.58,
        content_bounds.size.height * 0.48,
    );
    let layer_bounds = Bounds::new(
        field_bounds.origin.x,
        field_bounds.max_y() + 12.0,
        field_bounds.size.width,
        108.0,
    );
    let telemetry_bounds = Bounds::new(
        field_bounds.max_x() + 16.0,
        field_bounds.origin.y,
        content_bounds.max_x() - field_bounds.max_x() - 28.0,
        content_bounds.size.height * 0.68,
    );
    let ribbon_bounds = Bounds::new(
        field_bounds.origin.x,
        layer_bounds.max_y() + 12.0,
        content_bounds.size.width - 24.0,
        content_bounds.max_y() - layer_bounds.max_y() - 24.0,
    );

    paint_title_block(content_bounds, runtime, pane_state, active, accent, paint);
    paint_viz_panel_shell(field_bounds, accent, paint);
    paint_viz_panel_shell(layer_bounds, accent.with_alpha(0.88), paint);
    paint_viz_panel_shell(telemetry_bounds, accent.with_alpha(0.82), paint);
    paint_viz_panel_shell(ribbon_bounds, accent.with_alpha(0.78), paint);

    paint_decode_lattice(field_bounds, runtime, pane_state, phase, accent, paint);
    paint_layer_sweep(layer_bounds, runtime, pane_state, phase, accent, paint);
    paint_runtime_telemetry(telemetry_bounds, runtime, pane_state, active, accent, paint);
    paint_phase_ribbons(ribbon_bounds, runtime, pane_state, phase, accent, paint);
}

fn paint_title_block(
    content_bounds: Bounds,
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
    active: bool,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    let title = if active {
        "PSIONIC MESH  //  ACTIVE DECODE FIELD"
    } else if runtime.is_ready() {
        "PSIONIC MESH  //  READY STANDBY"
    } else if runtime.artifact_present {
        "PSIONIC MESH  //  UNLOADED"
    } else {
        "PSIONIC MESH  //  COLD SHELL"
    };
    let subtitle = if active {
        "Synthetic layer lattice driven by live Psionic / GPT-OSS runtime counters."
    } else {
        "Derived from runtime metrics and provenance. This is a visualization field, not raw tensor taps."
    };

    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 16.0,
        ),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        subtitle,
        Point::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 34.0,
        ),
        10.0,
        theme::text::MUTED,
    ));

    let summary = format!(
        "Field {}",
        if active {
            "tracking active decode"
        } else if runtime.is_ready() {
            "primed for next prompt"
        } else if runtime.artifact_present {
            "waiting for model warm"
        } else {
            "waiting for runtime artifact"
        }
    );
    let _ = paint_state_summary(
        paint,
        content_bounds.max_x() - 270.0,
        content_bounds.origin.y + 8.0,
        pane_state.load_state,
        summary.as_str(),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            content_bounds.max_x() - 156.0,
            content_bounds.origin.y + 18.0,
            136.0,
            2.0,
        ))
        .with_background(accent.with_alpha(0.65)),
    );
}

fn paint_decode_lattice(
    bounds: Bounds,
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
    phase: f32,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        "LATTICE",
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
        10.0,
        accent.with_alpha(0.88),
    ));

    let matrix_bounds = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 28.0,
        bounds.size.width - 24.0,
        bounds.size.height - 40.0,
    );
    let mut dots = DotsGrid::new()
        .shape(DotShape::Cross)
        .distance(26.0)
        .size(1.2)
        .color(accent.with_alpha(0.24))
        .animation_progress(1.0);
    dots.paint(matrix_bounds, paint);

    let mut heatmap = Heatmap::new()
        .data(
            LATTICE_ROWS,
            LATTICE_COLS,
            build_lattice_matrix(runtime, pane_state, phase),
        )
        .range(0.0, 1.0)
        .gap(2.0)
        .low_color(viz_theme::surface::CHART_BG.with_alpha(0.94))
        .mid_color(Some(viz_theme::series::RUNTIME.with_alpha(0.76)))
        .high_color(theme::text::PRIMARY.with_alpha(0.96));
    heatmap.paint(matrix_bounds, paint);

    let mut scanlines = Scanlines::new()
        .spacing(14.0)
        .line_color(accent.with_alpha(0.08))
        .scan_color(accent.with_alpha(0.2))
        .scan_width(18.0)
        .scan_progress(phase)
        .opacity(0.9);
    scanlines.paint(matrix_bounds, paint);

    let metrics = active_metrics(runtime, pane_state);
    let provenance = active_provenance(pane_state);
    let load_share = phase_share(metrics.and_then(|value| value.load_duration_ns), metrics);
    let prefill_share = phase_share(
        metrics.and_then(|value| value.prompt_eval_duration_ns),
        metrics,
    );
    let decode_share = phase_share(metrics.and_then(|value| value.eval_duration_ns), metrics);
    let summary = format!(
        "prefill {:>2.0}%  decode {:>2.0}%  load {:>2.0}%  tokens {}→{}",
        prefill_share * 100.0,
        decode_share * 100.0,
        load_share * 100.0,
        provenance
            .and_then(|value| value.prompt_token_count)
            .unwrap_or_default(),
        provenance
            .and_then(|value| value.generated_token_count)
            .unwrap_or_default(),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        summary.as_str(),
        Point::new(bounds.origin.x + 12.0, bounds.max_y() - 10.0),
        9.0,
        theme::text::MUTED,
    ));
}

fn paint_layer_sweep(
    bounds: Bounds,
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
    phase: f32,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        "SYNTHETIC LAYER SWEEP",
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
        10.0,
        accent.with_alpha(0.86),
    ));

    let sweep_bounds = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 24.0,
        bounds.size.width - 24.0,
        bounds.size.height - 34.0,
    );
    let mut sweep = PsionicLayerSweep::new(
        build_layer_levels(runtime, pane_state, phase),
        phase,
        accent,
        runtime.busy || pane_state.pending_request_id.is_some(),
    );
    sweep.paint(sweep_bounds, paint);
}

fn paint_runtime_telemetry(
    bounds: Bounds,
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
    active: bool,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        "RUNTIME SIGNATURE",
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
        10.0,
        accent.with_alpha(0.86),
    ));

    let metrics = active_metrics(runtime, pane_state);
    let provenance = active_provenance(pane_state);
    let decode_tps = tokens_per_second(
        provenance
            .and_then(|value| value.generated_token_count)
            .or_else(|| metrics.and_then(|value| value.eval_count)),
        metrics.and_then(|value| value.eval_duration_ns),
    );
    let prefill_tps = tokens_per_second(
        provenance
            .and_then(|value| value.prompt_token_count)
            .or_else(|| metrics.and_then(|value| value.prompt_eval_count)),
        metrics.and_then(|value| value.prompt_eval_duration_ns),
    );
    let latency_ms = metrics
        .and_then(|value| value.total_duration_ns)
        .map(|value| value as f32 / 1_000_000.0)
        .unwrap_or_default();
    let load_ms = metrics
        .and_then(|value| value.load_duration_ns)
        .map(|value| value as f32 / 1_000_000.0)
        .unwrap_or_default();

    let ring_bounds = Bounds::new(bounds.origin.x + 16.0, bounds.origin.y + 30.0, 118.0, 118.0);
    let mut throughput_ring = RingGauge::new()
        .segments(48)
        .dot_size(4.0)
        .level(normalize_signal(decode_tps, 48.0))
        .active_color(accent.with_alpha(0.88))
        .inactive_color(theme::bg::APP)
        .head_color(theme::text::PRIMARY);
    throughput_ring.paint(ring_bounds, paint);

    let warm_bounds = Bounds::new(
        bounds.origin.x + 152.0,
        bounds.origin.y + 30.0,
        118.0,
        118.0,
    );
    let warm_level = match provenance.and_then(|value| value.warm_start) {
        Some(true) => 1.0,
        Some(false) => 0.28,
        None if active => 0.64,
        None => 0.12,
    };
    let mut warm_ring = RingGauge::new()
        .segments(36)
        .dot_size(4.0)
        .level(warm_level)
        .active_color(viz_theme::state::WARNING.with_alpha(0.9))
        .inactive_color(theme::bg::APP)
        .head_color(theme::text::PRIMARY);
    warm_ring.paint(warm_bounds, paint);

    paint.scene.draw_text(paint.text.layout_mono(
        &format!("{decode_tps:>5.1} tok/s"),
        Point::new(ring_bounds.origin.x + 18.0, ring_bounds.max_y() - 12.0),
        10.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        if provenance.and_then(|value| value.warm_start) == Some(true) {
            "WARM START"
        } else if provenance.and_then(|value| value.warm_start) == Some(false) {
            "COLD START"
        } else if active {
            "TRACKING"
        } else {
            "IDLE CACHE"
        },
        Point::new(warm_bounds.origin.x + 14.0, warm_bounds.max_y() - 12.0),
        10.0,
        theme::text::PRIMARY,
    ));

    let meter_top = ring_bounds.max_y() + 18.0;
    paint_signal_triplet(
        bounds.origin.x + 22.0,
        meter_top,
        prefill_tps,
        decode_tps,
        load_ms,
        accent,
        paint,
    );

    let mut y = meter_top + 92.0;
    y = paint_label_line(
        paint,
        bounds.origin.x + 16.0,
        y,
        "Backend",
        runtime.backend_label.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 16.0,
        y,
        "Ready model",
        runtime.ready_model.as_deref().unwrap_or("-"),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 16.0,
        y,
        "Request",
        pane_state
            .pending_request_id
            .as_deref()
            .or(pane_state.last_request_id.as_deref())
            .or(runtime.last_request_id.as_deref())
            .unwrap_or("-"),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 16.0,
        y,
        "Latency",
        &format!("{latency_ms:.1} ms"),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 16.0,
        y,
        "Load",
        &format!("{load_ms:.1} ms"),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 16.0,
        y,
        "Prefill",
        &format!("{prefill_tps:.1} tok/s"),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 16.0,
        y,
        "Decode",
        &format!("{decode_tps:.1} tok/s"),
    );

    if let Some(provenance) = provenance {
        let _ = paint_multiline_phrase(
            paint,
            bounds.origin.x + 16.0,
            y,
            "Prompt digest",
            provenance.normalized_prompt_digest.as_str(),
        );
    }
}

fn paint_phase_ribbons(
    bounds: Bounds,
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
    phase: f32,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        "PHASE RIBBONS",
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
        10.0,
        accent.with_alpha(0.86),
    ));
    paint.scene.draw_text(paint.text.layout(
        "Three derived rails for prefill, decode, and residency pressure.",
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 24.0),
        10.0,
        theme::text::MUTED,
    ));

    let metrics = active_metrics(runtime, pane_state);
    let load_share = phase_share(metrics.and_then(|value| value.load_duration_ns), metrics);
    let prefill_share = phase_share(
        metrics.and_then(|value| value.prompt_eval_duration_ns),
        metrics,
    );
    let decode_share = phase_share(metrics.and_then(|value| value.eval_duration_ns), metrics);

    let inner_width = bounds.size.width - 24.0;
    let ribbon_width = inner_width - 136.0;
    let start_x = bounds.origin.x + 132.0;
    let labels_x = bounds.origin.x + 14.0;
    let top = bounds.origin.y + 48.0;
    let gap = 14.0;
    let ribbon_height = ((bounds.size.height - 62.0 - gap * 2.0) / 3.0).max(14.0);

    let ribbons = [
        (
            "PREFILL",
            prefill_share,
            viz_theme::series::RUNTIME,
            build_ribbon_values(prefill_share, phase, 0.9),
        ),
        (
            "DECODE",
            decode_share,
            viz_theme::series::PROVENANCE,
            build_ribbon_values(decode_share, phase + 0.18, 1.25),
        ),
        (
            "LOAD",
            load_share.max(if runtime.artifact_present { 0.12 } else { 0.02 }),
            viz_theme::series::HARDWARE,
            build_ribbon_values(load_share.max(0.08), phase + 0.35, 0.72),
        ),
    ];

    for (index, (label, level, color, values)) in ribbons.into_iter().enumerate() {
        let y = top + index as f32 * (ribbon_height + gap);
        paint.scene.draw_text(paint.text.layout_mono(
            label,
            Point::new(labels_x, y + 10.0),
            10.0,
            color.with_alpha(0.9),
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("{:>3.0}%", level * 100.0),
            Point::new(labels_x, y + 26.0),
            10.0,
            theme::text::PRIMARY,
        ));

        let mut ribbon = PsionicRibbon::new(values, color, level, phase);
        ribbon.paint(
            Bounds::new(start_x, y, ribbon_width.max(60.0), ribbon_height),
            paint,
        );
    }
}

fn paint_signal_triplet(
    origin_x: f32,
    origin_y: f32,
    prefill_tps: f32,
    decode_tps: f32,
    load_ms: f32,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    let specs = [
        (
            "PF",
            normalize_signal(prefill_tps, 96.0),
            viz_theme::series::RUNTIME,
        ),
        (
            "DC",
            normalize_signal(decode_tps, 48.0),
            viz_theme::series::PROVENANCE,
        ),
        (
            "LD",
            if load_ms <= 0.0 {
                0.1
            } else {
                normalize_signal(250.0 / load_ms.max(1.0), 1.6)
            },
            viz_theme::series::HARDWARE,
        ),
    ];

    for (index, (label, level, color)) in specs.into_iter().enumerate() {
        let x = origin_x + index as f32 * 44.0;
        let mut meter = SignalMeter::new()
            .bars(6)
            .gap(2.0)
            .level(level)
            .min_bar_height(0.16)
            .active_color(color.with_alpha(0.94))
            .inactive_color(accent.with_alpha(0.08));
        meter.paint(Bounds::new(x, origin_y, 26.0, 72.0), paint);
        paint.scene.draw_text(paint.text.layout_mono(
            label,
            Point::new(x - 1.0, origin_y + 84.0),
            10.0,
            color.with_alpha(0.9),
        ));
    }
}

fn build_lattice_matrix(
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
    phase: f32,
) -> Vec<f32> {
    let metrics = active_metrics(runtime, pane_state);
    let provenance = active_provenance(pane_state);
    let prompt_share = phase_share(
        metrics.and_then(|value| value.prompt_eval_duration_ns),
        metrics,
    );
    let decode_share = phase_share(metrics.and_then(|value| value.eval_duration_ns), metrics);
    let load_share = phase_share(metrics.and_then(|value| value.load_duration_ns), metrics);
    let prompt_tokens = provenance
        .and_then(|value| value.prompt_token_count)
        .or_else(|| metrics.and_then(|value| value.prompt_eval_count))
        .unwrap_or_default() as f32;
    let decode_tokens = provenance
        .and_then(|value| value.generated_token_count)
        .or_else(|| metrics.and_then(|value| value.eval_count))
        .unwrap_or_default() as f32;
    let token_pressure = ((prompt_tokens + decode_tokens).max(1.0).ln() / 7.0).clamp(0.0, 1.0);
    let busy_boost = if runtime.busy || pane_state.pending_request_id.is_some() {
        0.16
    } else if runtime.is_ready() {
        0.08
    } else {
        0.03
    };

    let mut values = Vec::with_capacity(LATTICE_ROWS * LATTICE_COLS);
    for row in 0..LATTICE_ROWS {
        let row_pos = row as f32 / (LATTICE_ROWS.saturating_sub(1)) as f32;
        for col in 0..LATTICE_COLS {
            let col_pos = col as f32 / (LATTICE_COLS.saturating_sub(1)) as f32;
            let sweep = (((col_pos * 7.2 + row_pos * 2.1 + phase * 4.0).sin()) + 1.0) * 0.5;
            let diagonal = (((col_pos + row_pos * 0.7 + phase * 1.7) * 9.0).cos() + 1.0) * 0.5;
            let prefill_band =
                prompt_share * (1.0 - ((row_pos - 0.24).abs() * 3.4).clamp(0.0, 1.0));
            let decode_band = decode_share * (1.0 - ((row_pos - 0.68).abs() * 3.0).clamp(0.0, 1.0));
            let load_front =
                load_share * (1.0 - ((col_pos - (0.1 + phase * 0.2)).abs() * 4.0).clamp(0.0, 1.0));
            let ridge = ((col_pos - row_pos).abs() * -3.0 + 1.0).clamp(0.0, 1.0);
            let value = 0.16 * sweep
                + 0.12 * diagonal
                + 0.2 * prefill_band
                + 0.24 * decode_band
                + 0.14 * load_front
                + 0.08 * ridge
                + 0.12 * token_pressure
                + busy_boost;
            values.push(value.clamp(0.0, 1.0));
        }
    }
    values
}

fn build_layer_levels(
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
    phase: f32,
) -> Vec<f32> {
    let metrics = active_metrics(runtime, pane_state);
    let provenance = active_provenance(pane_state);
    let prompt_share = phase_share(
        metrics.and_then(|value| value.prompt_eval_duration_ns),
        metrics,
    );
    let decode_share = phase_share(metrics.and_then(|value| value.eval_duration_ns), metrics);
    let load_share = phase_share(metrics.and_then(|value| value.load_duration_ns), metrics);
    let warm_bias = match provenance.and_then(|value| value.warm_start) {
        Some(true) => 0.14,
        Some(false) => 0.06,
        None => 0.08,
    };

    (0..SYNTHETIC_LAYER_BANDS)
        .map(|index| {
            let pos = index as f32 / (SYNTHETIC_LAYER_BANDS.saturating_sub(1)) as f32;
            let decode_wave = (((pos * 11.0) + phase * 6.0).sin() + 1.0) * 0.5;
            let shoulder = (((1.0 - pos) * 7.0 + phase * 2.5).cos() + 1.0) * 0.5;
            (0.14
                + prompt_share * (1.0 - ((pos - 0.2).abs() * 3.2).clamp(0.0, 1.0))
                + decode_share * (1.0 - ((pos - 0.68).abs() * 2.6).clamp(0.0, 1.0))
                + load_share * (1.0 - ((pos - 0.08).abs() * 5.0).clamp(0.0, 1.0))
                + decode_wave * 0.2
                + shoulder * 0.1
                + warm_bias)
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

fn active_metrics<'a>(
    runtime: &'a LocalInferenceExecutionSnapshot,
    pane_state: &'a LocalInferencePaneState,
) -> Option<&'a LocalInferenceExecutionMetrics> {
    pane_state
        .last_metrics
        .as_ref()
        .or(runtime.last_metrics.as_ref())
}

fn active_provenance(
    pane_state: &LocalInferencePaneState,
) -> Option<&LocalInferenceExecutionProvenance> {
    pane_state.last_provenance.as_ref()
}

fn phase_share(
    phase_duration_ns: Option<u64>,
    metrics: Option<&LocalInferenceExecutionMetrics>,
) -> f32 {
    let total = metrics
        .and_then(|value| value.total_duration_ns)
        .unwrap_or_default();
    if total == 0 {
        return 0.0;
    }
    phase_duration_ns.unwrap_or_default() as f32 / total as f32
}

fn tokens_per_second(tokens: Option<u64>, duration_ns: Option<u64>) -> f32 {
    let Some(tokens) = tokens else {
        return 0.0;
    };
    let Some(duration_ns) = duration_ns else {
        return 0.0;
    };
    if duration_ns == 0 {
        return 0.0;
    }
    tokens as f32 / (duration_ns as f32 / 1_000_000_000.0)
}

fn normalize_signal(value: f32, reference: f32) -> f32 {
    if value <= 0.0 || reference <= 0.0 {
        return 0.0;
    }
    (value / reference).sqrt().clamp(0.0, 1.0)
}

fn animation_phase(
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
) -> f32 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as f32;
    let seed = pane_state
        .pending_request_id
        .as_deref()
        .or(pane_state.last_request_id.as_deref())
        .or(runtime.last_request_id.as_deref())
        .map(seed_from_text)
        .unwrap_or(17.0);
    ((millis / 2600.0) + seed / 97.0).fract()
}

fn seed_from_text(value: &str) -> f32 {
    let mut sum = 0u64;
    for (index, byte) in value.bytes().enumerate() {
        sum = sum.saturating_add((index as u64 + 1) * u64::from(byte));
    }
    (sum % 4096) as f32
}

fn mesh_accent(active: bool, artifact_present: bool, ready: bool) -> Hsla {
    if active {
        viz_theme::state::ACTIVE
    } else if ready {
        viz_theme::state::LIVE
    } else if artifact_present {
        viz_theme::state::WARNING
    } else {
        viz_theme::state::ERROR
    }
}

struct PsionicLayerSweep {
    levels: Vec<f32>,
    phase: f32,
    accent: Hsla,
    active: bool,
}

impl PsionicLayerSweep {
    fn new(levels: Vec<f32>, phase: f32, accent: Hsla, active: bool) -> Self {
        Self {
            levels,
            phase,
            accent,
            active,
        }
    }
}

impl Component for PsionicLayerSweep {
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

struct PsionicRibbon {
    values: Vec<f32>,
    color: Hsla,
    level: f32,
    phase: f32,
}

impl PsionicRibbon {
    fn new(values: Vec<f32>, color: Hsla, level: f32, phase: f32) -> Self {
        Self {
            values,
            color,
            level,
            phase,
        }
    }
}

impl Component for PsionicRibbon {
    fn paint(&mut self, bounds: Bounds, paint: &mut PaintContext) {
        if self.values.is_empty() {
            return;
        }

        paint.scene.draw_quad(
            Quad::new(bounds)
                .with_background(viz_theme::surface::CHART_BG.with_alpha(0.9))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lattice_matrix_matches_expected_size() {
        let matrix = build_lattice_matrix(
            &LocalInferenceExecutionSnapshot::default(),
            &LocalInferencePaneState::default(),
            0.42,
        );
        assert_eq!(matrix.len(), LATTICE_ROWS * LATTICE_COLS);
        assert!(matrix.iter().all(|value| (0.0..=1.0).contains(value)));
    }

    #[test]
    fn layer_levels_are_clamped() {
        let levels = build_layer_levels(
            &LocalInferenceExecutionSnapshot::default(),
            &LocalInferencePaneState::default(),
            0.67,
        );
        assert_eq!(levels.len(), SYNTHETIC_LAYER_BANDS);
        assert!(levels.iter().all(|value| (0.0..=1.0).contains(value)));
    }

    #[test]
    fn normalize_signal_is_bounded() {
        assert_eq!(normalize_signal(0.0, 48.0), 0.0);
        assert!((0.0..=1.0).contains(&normalize_signal(24.0, 48.0)));
        assert_eq!(normalize_signal(9_999.0, 48.0), 1.0);
    }
}
