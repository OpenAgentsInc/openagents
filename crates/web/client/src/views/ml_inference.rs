use wgpui::{
    Bounds, FontStyle, Hsla, Point, Quad, Scene, TextSystem, theme,
};
use wgpui::animation::AnimatorState;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Heatmap, RingGauge, Scanlines, SignalMeter};
use wgpui::components::Component;
use wgpui::PaintContext;

use crate::state::{AppState, GateStatus};

fn accent_cyan() -> Hsla {
    Hsla::from_hex(0x7fd3e5)
}

fn accent_orange() -> Hsla {
    Hsla::from_hex(0xff9900)
}

fn accent_green() -> Hsla {
    Hsla::from_hex(0x00ff88)
}

fn panel_bg() -> Hsla {
    Hsla::from_hex(0x05070b)
}

fn panel_border() -> Hsla {
    Hsla::from_hex(0x2a3640)
}

pub(crate) fn build_ml_inference_page(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    // Background
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    // Dots grid background
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 1.0, 0.18))
        .distance(34.0)
        .size(1.0)
        .origin(DotsOrigin::Center);
    state.dots_grid.update(AnimatorState::Entered);
    state.dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    let padding = 22.0;
    let content_width = (width - padding * 2.0).min(1100.0);
    let content_x = (width - content_width) / 2.0;
    let card_y = padding;
    let card_height = height - padding * 2.0;
    let card_bounds = Bounds::new(content_x, card_y, content_width, card_height);

    // Frame animation
    if !state.ml_viz.frame_started {
        state.ml_viz.frame_started = true;
    }
    let frame_progress = state
        .ml_viz
        .frame_animator
        .update(AnimatorState::Entering);

    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    wgpui::components::hud::Frame::corners()
        .line_color(Hsla::new(0.0, 0.0, 1.0, 0.75))
        .bg_color(Hsla::new(0.0, 0.0, 0.0, 0.4))
        .glow_color(Hsla::new(0.0, 0.0, 1.0, 0.16))
        .border_color(Hsla::new(0.0, 0.0, 1.0, 0.1))
        .stroke_width(1.0)
        .corner_length(26.0)
        .animation_progress(frame_progress)
        .paint(card_bounds, &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    let inner_padding = 26.0;
    let scrollbar_width = 10.0;
    let inner_x = content_x + inner_padding;
    let inner_width = content_width - inner_padding * 2.0 - scrollbar_width;
    let visible_height = card_height - inner_padding * 2.0;

    state.ml_viz.content_bounds = Bounds::new(
        inner_x,
        card_y + inner_padding,
        inner_width + scrollbar_width,
        visible_height,
    );

    scene.push_clip(Bounds::new(
        inner_x - 4.0,
        card_y + inner_padding,
        inner_width + scrollbar_width + 8.0,
        visible_height,
    ));

    let scroll_offset = state.ml_viz.scroll_offset;
    let mut y = card_y + inner_padding - scroll_offset;

    // Title
    let title = "ML INFERENCE VISUALIZATION";
    let title_size = 22.0;
    let title_width = measure_mono(text_system, title, title_size);
    let title_x = inner_x + (inner_width - title_width) * 0.5;
    draw_mono_text(
        scene,
        text_system,
        title,
        title_x,
        y,
        title_size,
        accent_cyan(),
    );
    y += title_size + 6.0;

    let subtitle = "CANDLE TELEMETRY HUD";
    let subtitle_size = 12.0;
    let subtitle_width = measure_mono(text_system, subtitle, subtitle_size);
    let subtitle_x = inner_x + (inner_width - subtitle_width) * 0.5;
    draw_mono_text(
        scene,
        text_system,
        subtitle,
        subtitle_x,
        y,
        subtitle_size,
        theme::text::MUTED,
    );
    y += subtitle_size + 18.0;

    // Gate status
    let gate_height = 120.0;
    let gate_bounds = Bounds::new(inner_x, y, inner_width, gate_height);
    draw_gate_panel(scene, text_system, state, gate_bounds);
    y += gate_height + 18.0;

    // Inference monitor
    let monitor_height = 250.0;
    let monitor_bounds = Bounds::new(inner_x, y, inner_width, monitor_height);
    draw_inference_monitor(scene, text_system, state, monitor_bounds, scale_factor);
    y += monitor_height + 18.0;

    // Attention visualizer
    let attention_height = 280.0;
    let attention_bounds = Bounds::new(inner_x, y, inner_width, attention_height);
    draw_attention_panel(scene, text_system, state, attention_bounds, scale_factor);
    y += attention_height + 18.0;

    // Layer activity + cache/power row
    let row_height = 220.0;
    let left_width = inner_width * 0.6;
    let right_width = inner_width - left_width - 18.0;
    let layer_bounds = Bounds::new(inner_x, y, left_width, row_height);
    let cache_bounds = Bounds::new(inner_x + left_width + 18.0, y, right_width, row_height);
    draw_layer_activity(scene, text_system, state, layer_bounds);
    draw_cache_panel(scene, text_system, state, cache_bounds, scale_factor);
    y += row_height + 18.0;

    // Probability history + attention flow
    let history_height = 240.0;
    let history_bounds = Bounds::new(inner_x, y, inner_width, history_height);
    draw_probability_history(scene, text_system, state, history_bounds);
    y += history_height + 18.0;

    let flow_height = 180.0;
    let flow_bounds = Bounds::new(inner_x, y, inner_width, flow_height);
    draw_attention_flow(scene, text_system, state, flow_bounds);
    y += flow_height + 20.0;

    let total_content_height = y - (card_y + inner_padding - scroll_offset);
    state.ml_viz.content_height = total_content_height;

    scene.pop_clip();

    let max_scroll = (total_content_height - visible_height).max(0.0);
    state.ml_viz.scroll_offset = state.ml_viz.scroll_offset.clamp(0.0, max_scroll);

    if max_scroll > 0.0 {
        draw_scrollbar(
            scene,
            content_x,
            content_width,
            card_y,
            card_height,
            scrollbar_width,
            visible_height,
            total_content_height,
            state.ml_viz.scroll_offset,
        );
    }

    // Clear unrelated bounds
    state.button_bounds = Bounds::ZERO;
    state.left_cta_bounds = Bounds::ZERO;
    state.right_cta_bounds = Bounds::ZERO;
    state.landing_issue_bounds = Bounds::ZERO;
    state.repo_bounds.clear();
}

fn draw_inference_monitor(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    bounds: Bounds,
    scale_factor: f32,
) {
    let inner = panel(scene, text_system, bounds, "INFERENCE MONITOR");

    let has_tokens = !state.ml_viz.token_stream.is_empty();
    let status = if has_tokens { "INFERENCE ACTIVE" } else { "INFERENCE IDLE" };
    draw_mono_text(
        scene,
        text_system,
        status,
        inner.x(),
        inner.y(),
        11.0,
        if has_tokens { accent_green() } else { theme::text::MUTED },
    );

    let speed = match state.ml_viz.tokens_per_sec {
        Some(value) => format!("{value:.1} tok/s"),
        None => "-- tok/s".to_string(),
    };
    let speed_w = measure_mono(text_system, &speed, 11.0);
    draw_mono_text(
        scene,
        text_system,
        &speed,
        inner.x() + inner.width() - speed_w,
        inner.y(),
        11.0,
        accent_cyan(),
    );

    let stream_y = inner.y() + 18.0;
    let stream_bounds = Bounds::new(inner.x(), stream_y, inner.width(), 54.0);
    scene.draw_quad(
        Quad::new(stream_bounds)
            .with_background(panel_bg().with_alpha(0.85))
            .with_border(panel_border().with_alpha(0.9), 1.0),
    );

    let stream_text = if has_tokens {
        state.ml_viz.token_stream.clone()
    } else {
        "Awaiting telemetry...".to_string()
    };
    let stream_run = text_system.layout_styled_mono(
        &stream_text,
        Point::new(stream_bounds.x() + 8.0, stream_bounds.y() + 16.0),
        12.0,
        theme::text::PRIMARY,
        FontStyle::normal(),
    );
    scene.draw_text(stream_run);

    let mut scanlines = Scanlines::new()
        .line_color(Hsla::new(0.0, 0.0, 1.0, 0.08))
        .scan_color(Hsla::new(0.0, 0.0, 1.0, 0.12))
        .spacing(10.0)
        .scan_width(16.0)
        .scan_progress(0.65)
        .opacity(0.7);
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    scanlines.paint(stream_bounds, &mut cx);

    let bars_y = stream_bounds.y() + stream_bounds.height() + 12.0;
    draw_top_k(scene, text_system, state, Bounds::new(inner.x(), bars_y, inner.width(), 88.0));

    let metrics_y = bars_y + 94.0;
    let metrics_bounds = Bounds::new(inner.x(), metrics_y, inner.width(), 46.0);
    draw_metric_row(scene, text_system, state, metrics_bounds, scale_factor);
}

fn draw_gate_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    bounds: Bounds,
) {
    let inner = panel(scene, text_system, bounds, "GATE C/D STATUS");

    let (status_text, status_color) = match state.ml_viz.gate_status {
        GateStatus::Idle => ("IDLE", theme::text::MUTED),
        GateStatus::Running => ("RUNNING", accent_orange()),
        GateStatus::Passed => ("PASS", accent_green()),
        GateStatus::Failed => ("FAIL", theme::status::ERROR),
    };

    let line_height = 14.0;
    let mut y = inner.y();

    draw_mono_text(
        scene,
        text_system,
        &format!("STATUS: {status_text}"),
        inner.x(),
        y,
        11.0,
        status_color,
    );
    y += line_height;

    let phase = state
        .ml_viz
        .gate_message
        .as_deref()
        .unwrap_or("idle");
    draw_mono_text(
        scene,
        text_system,
        &format!("PHASE: {}", truncate_text(phase, 48)),
        inner.x(),
        y,
        11.0,
        theme::text::PRIMARY,
    );
    y += line_height;

    let gguf = state
        .ml_viz
        .gate_source
        .as_deref()
        .unwrap_or("not set");
    draw_mono_text(
        scene,
        text_system,
        &format!("GGUF: {}", truncate_text(gguf, 52)),
        inner.x(),
        y,
        10.0,
        theme::text::MUTED,
    );
    y += line_height;

    let tensor = state
        .ml_viz
        .gate_tensor
        .as_deref()
        .unwrap_or("--");
    let k = state.ml_viz.gate_k.map_or("--".to_string(), |v| v.to_string());
    let n = state.ml_viz.gate_n.map_or("--".to_string(), |v| v.to_string());
    let bytes = state
        .ml_viz
        .gate_bytes
        .map(format_bytes)
        .unwrap_or_else(|| "--".to_string());
    draw_mono_text(
        scene,
        text_system,
        &format!(
            "TENSOR: {}  KxN: {}x{}  BYTES: {}",
            truncate_text(tensor, 24),
            k,
            n,
            bytes
        ),
        inner.x(),
        y,
        10.0,
        theme::text::PRIMARY,
    );
    y += line_height;

    if let Some(err) = &state.ml_viz.gate_error {
        draw_mono_text(
            scene,
            text_system,
            &format!("ERROR: {}", truncate_text(err, 56)),
            inner.x(),
            y,
            10.0,
            theme::status::ERROR,
        );
        return;
    }

    let max_abs = state
        .ml_viz
        .gate_max_abs
        .map(|v| format!("{v:.3e}"))
        .unwrap_or_else(|| "--".to_string());
    let mean_abs = state
        .ml_viz
        .gate_mean_abs
        .map(|v| format!("{v:.3e}"))
        .unwrap_or_else(|| "--".to_string());

    draw_mono_text(
        scene,
        text_system,
        &format!("MAX: {max_abs}  MEAN: {mean_abs}"),
        inner.x(),
        y,
        10.0,
        theme::text::MUTED,
    );
}

fn draw_attention_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    bounds: Bounds,
    scale_factor: f32,
) {
    let inner = panel(scene, text_system, bounds, "ATTENTION VISUALIZER");

    let controls_y = inner.y();
    let label_size = 10.0;
    let layer_label = format!("LAYER {}", state.ml_viz.selected_layer);
    draw_mono_text(
        scene,
        text_system,
        &layer_label,
        inner.x(),
        controls_y,
        label_size,
        theme::text::PRIMARY,
    );

    let head_label = format!("HEAD {}", state.ml_viz.selected_head);
    let head_label_w = measure_mono(text_system, &head_label, label_size);
    draw_mono_text(
        scene,
        text_system,
        &head_label,
        inner.x() + inner.width() - head_label_w,
        controls_y,
        label_size,
        theme::text::PRIMARY,
    );

    let slider_y = controls_y + 14.0;
    let slider_w = inner.width() * 0.45;
    let layer_slider_bounds = Bounds::new(inner.x(), slider_y, slider_w, 20.0);
    let head_slider_bounds = Bounds::new(
        inner.x() + inner.width() - slider_w,
        slider_y,
        slider_w,
        20.0,
    );
    state.ml_viz.layer_slider_bounds = layer_slider_bounds;
    state.ml_viz.head_slider_bounds = head_slider_bounds;

    draw_slider(
        scene,
        layer_slider_bounds,
        state.ml_viz.selected_layer as u32,
        0,
        state.ml_viz.max_layers.saturating_sub(1) as u32,
        accent_cyan(),
    );
    draw_slider(
        scene,
        head_slider_bounds,
        state.ml_viz.selected_head as u32,
        0,
        state.ml_viz.max_heads.saturating_sub(1) as u32,
        accent_orange(),
    );

    let heatmap_bounds = Bounds::new(
        inner.x(),
        slider_y + 26.0,
        inner.width(),
        inner.height() - 40.0,
    );
    scene.draw_quad(
        Quad::new(heatmap_bounds)
            .with_background(panel_bg().with_alpha(0.8))
            .with_border(panel_border().with_alpha(0.6), 1.0),
    );

    if let Some(weights) = &state.ml_viz.attention_weights {
        if state.ml_viz.attention_layer == state.ml_viz.selected_layer
            && state.ml_viz.attention_head == state.ml_viz.selected_head
        {
            let rows = weights.len();
            let cols = weights.first().map(|row| row.len()).unwrap_or(0);
            let mut data = Vec::with_capacity(rows * cols);
            for row in weights {
                data.extend(row.iter().copied());
            }
            let mut heatmap = Heatmap::new()
                .data(rows, cols, data)
                .gap(1.0)
                .low_color(Hsla::from_hex(0x04101a))
                .mid_color(Some(Hsla::from_hex(0x2ec4d6)))
                .high_color(Hsla::from_hex(0xf8fbff));
            let mut cx = PaintContext::new(scene, text_system, scale_factor);
            heatmap.paint(heatmap_bounds, &mut cx);
        } else {
            draw_empty_state(scene, text_system, heatmap_bounds, "No attention telemetry");
        }
    } else {
        draw_empty_state(scene, text_system, heatmap_bounds, "No attention telemetry");
    }
}

fn draw_layer_activity(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    bounds: Bounds,
) {
    let inner = panel(scene, text_system, bounds, "NETWORK PULSE");

    if state.ml_viz.layer_activations.is_empty() {
        draw_empty_state(scene, text_system, inner, "No activation telemetry");
        return;
    }

    let row_height = 18.0;
    let max_rows = (inner.height() / row_height).floor() as usize;
    let rows = state.ml_viz.layer_activations.len().min(max_rows).max(1);

    let max_value = state
        .ml_viz
        .layer_activations
        .iter()
        .take(rows)
        .flat_map(|layer| [layer.attention_norm, layer.mlp_norm, layer.output_norm])
        .fold(0.0f32, |acc, v| acc.max(v));
    let max_value = max_value.max(1e-3);

    for (idx, layer) in state.ml_viz.layer_activations.iter().take(rows).enumerate() {
        let y = inner.y() + idx as f32 * row_height;
        let label = format!("{:02}", layer.layer);
        draw_mono_text(scene, text_system, &label, inner.x(), y + 2.0, 9.0, theme::text::MUTED);

        let bar_x = inner.x() + 24.0;
        let bar_w = inner.width() - 24.0;
        let bar_h = 6.0;
        let attn_w = bar_w * (layer.attention_norm / max_value).clamp(0.0, 1.0);
        let mlp_w = bar_w * (layer.mlp_norm / max_value).clamp(0.0, 1.0);
        let out_w = bar_w * (layer.output_norm / max_value).clamp(0.0, 1.0);

        scene.draw_quad(
            Quad::new(Bounds::new(bar_x, y + 2.0, attn_w, bar_h))
                .with_background(accent_cyan().with_alpha(0.9)),
        );
        scene.draw_quad(
            Quad::new(Bounds::new(bar_x, y + 9.0, mlp_w, bar_h))
                .with_background(accent_orange().with_alpha(0.8)),
        );
        scene.draw_quad(
            Quad::new(Bounds::new(bar_x, y + 16.0, out_w, bar_h))
                .with_background(accent_green().with_alpha(0.8)),
        );
    }
}

fn draw_cache_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    bounds: Bounds,
    scale_factor: f32,
) {
    let inner = panel(scene, text_system, bounds, "KV CACHE");

    let layer = state.ml_viz.selected_layer;
    let cache_info = state.ml_viz.cache_status.get(layer);

    let ring_bounds = Bounds::new(inner.x() + 20.0, inner.y() + 12.0, 120.0, 120.0);
    scene.draw_quad(
        Quad::new(ring_bounds)
            .with_background(panel_bg().with_alpha(0.85))
            .with_border(panel_border().with_alpha(0.7), 1.0),
    );

    if let Some(info) = cache_info {
        let level = if info.max_len > 0 {
            info.seq_len as f32 / info.max_len as f32
        } else {
            0.0
        };
        let head = if info.max_len > 0 {
            Some(info.seq_len.saturating_sub(1) % 64)
        } else {
            None
        };
        let mut ring = RingGauge::new()
            .segments(64)
            .level(level)
            .dot_size(5.0)
            .head(head)
            .active_color(accent_cyan())
            .inactive_color(Hsla::from_hex(0x0c151d))
            .head_color(accent_green());
        let mut cx = PaintContext::new(scene, text_system, scale_factor);
        ring.paint(ring_bounds, &mut cx);
    } else {
        draw_empty_state(scene, text_system, ring_bounds, "No cache telemetry");
    }

    let stats_x = ring_bounds.x() + ring_bounds.width() + 12.0;
    let mut stats_y = inner.y() + 10.0;
    let stats = cache_info.map(|info| {
        vec![
            format!("SEQ {}/{}", info.seq_len, info.max_len),
            format!("OFFSET {}", info.offset),
            format!("BYTES {}", format_bytes(info.memory_bytes)),
        ]
    });

    if let Some(lines) = stats {
        for line in lines {
            draw_mono_text(
                scene,
                text_system,
                &line,
                stats_x,
                stats_y,
                10.0,
                theme::text::PRIMARY,
            );
            stats_y += 14.0;
        }
    } else {
        draw_mono_text(
            scene,
            text_system,
            "No cache telemetry",
            stats_x,
            stats_y,
            10.0,
            theme::text::MUTED,
        );
    }

    let mem_y = ring_bounds.y() + ring_bounds.height() + 10.0;
    let mem_bounds = Bounds::new(inner.x(), mem_y, inner.width(), 20.0);
    draw_memory_row(scene, text_system, state, mem_bounds);
}

fn draw_probability_history(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    bounds: Bounds,
) {
    let inner = panel(scene, text_system, bounds, "PROBABILITY WATERFALL");

    if state.ml_viz.probability_history.is_empty() {
        draw_empty_state(scene, text_system, inner, "No probability telemetry");
        return;
    }

    let columns = state.ml_viz.probability_history.len().min(10);
    let col_w = inner.width() / columns as f32;
    let _col_h = inner.height() - 26.0;
    let bar_h = 8.0;
    let bar_gap = 4.0;
    let base_y = inner.y() + 8.0;

    for (col_idx, entry) in state
        .ml_viz
        .probability_history
        .iter()
        .skip(state.ml_viz.probability_history.len().saturating_sub(columns))
        .enumerate()
    {
        let x = inner.x() + col_idx as f32 * col_w + 4.0;
        let max_prob = entry
            .iter()
            .map(|c| c.probability)
            .fold(0.0f32, |acc, v| acc.max(v))
            .max(1e-4);

        for (row_idx, candidate) in entry.iter().take(5).enumerate() {
            let width = (candidate.probability / max_prob).clamp(0.0, 1.0) * (col_w - 8.0);
            let y = base_y + row_idx as f32 * (bar_h + bar_gap);
            scene.draw_quad(
                Quad::new(Bounds::new(x, y, width, bar_h))
                    .with_background(accent_cyan().with_alpha(0.7)),
            );
        }
    }

    let entropy_y = inner.y() + inner.height() - 14.0;
    draw_entropy_line(scene, text_system, state, Bounds::new(inner.x(), entropy_y, inner.width(), 12.0));
}

fn draw_attention_flow(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    _state: &mut AppState,
    bounds: Bounds,
) {
    let inner = panel(scene, text_system, bounds, "ATTENTION FLOW");
    draw_empty_state(scene, text_system, inner, "No attention flow telemetry");
}

fn draw_top_k(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    bounds: Bounds,
) {
    let label = "NEXT TOKEN";
    draw_mono_text(scene, text_system, label, bounds.x(), bounds.y(), 10.0, theme::text::MUTED);

    let list_y = bounds.y() + 14.0;
    let row_h = 14.0;
    let max_rows = ((bounds.height() - 14.0) / row_h).floor() as usize;
    let entries = state.ml_viz.top_k.iter().take(max_rows).collect::<Vec<_>>();

    if entries.is_empty() {
        draw_mono_text(
            scene,
            text_system,
            "No probability telemetry",
            bounds.x(),
            list_y,
            10.0,
            theme::text::MUTED,
        );
        return;
    }

    let max_prob = entries
        .iter()
        .map(|c| c.probability)
        .fold(0.0f32, |acc, v| acc.max(v))
        .max(1e-6);

    for (idx, candidate) in entries.into_iter().enumerate() {
        let y = list_y + idx as f32 * row_h;
        let label = truncate_text(&candidate.token_text, 10);
        draw_mono_text(scene, text_system, &label, bounds.x(), y, 10.0, theme::text::PRIMARY);

        let bar_x = bounds.x() + 90.0;
        let bar_w = bounds.width() - 140.0;
        let width = (candidate.probability / max_prob).clamp(0.0, 1.0) * bar_w;
        scene.draw_quad(
            Quad::new(Bounds::new(bar_x, y + 2.0, width, 6.0))
                .with_background(accent_cyan().with_alpha(0.8)),
        );

        let prob_text = format!("{:.3}", candidate.probability);
        draw_mono_text(
            scene,
            text_system,
            &prob_text,
            bar_x + bar_w + 6.0,
            y,
            10.0,
            theme::text::MUTED,
        );
    }
}

fn draw_metric_row(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    bounds: Bounds,
    scale_factor: f32,
) {
    let box_w = (bounds.width() - 16.0) / 3.0;
    let labels = ["CACHE", "MEMORY", "ENTROPY"];

    for i in 0..3 {
        let x = bounds.x() + i as f32 * (box_w + 8.0);
        let box_bounds = Bounds::new(x, bounds.y(), box_w, bounds.height());
        scene.draw_quad(
            Quad::new(box_bounds)
                .with_background(panel_bg().with_alpha(0.85))
                .with_border(panel_border().with_alpha(0.8), 1.0),
        );

        draw_mono_text(
            scene,
            text_system,
            labels[i],
            box_bounds.x() + 6.0,
            box_bounds.y() + 4.0,
            9.0,
            theme::text::MUTED,
        );

        let meter_bounds = Bounds::new(
            box_bounds.x() + 6.0,
            box_bounds.y() + 18.0,
            box_bounds.width() - 12.0,
            box_bounds.height() - 22.0,
        );
        let mut meter = SignalMeter::new()
            .bars(10)
            .gap(2.0)
            .min_bar_height(0.1)
            .active_color(accent_cyan().with_alpha(0.9))
            .inactive_color(Hsla::from_hex(0x1a2730).with_alpha(0.7));

        match i {
            0 => {
                if let Some(cache) = state.ml_viz.cache_status.get(state.ml_viz.selected_layer) {
                    let level = if cache.max_len > 0 {
                        cache.seq_len as f32 / cache.max_len as f32
                    } else {
                        0.0
                    };
                    meter.set_level(level);
                } else {
                    meter.set_level(0.0);
                }
            }
            1 => {
                let level = state
                    .ml_viz
                    .memory_usage
                    .as_ref()
                    .map(|mem| (mem.cache_total as f32 / (mem.gpu_allocated.max(1) as f32)).clamp(0.0, 1.0))
                    .unwrap_or(0.0);
                meter.set_level(level);
            }
            _ => {
                let entropy = state.ml_viz.entropy.unwrap_or(0.0);
                meter.set_level((entropy / 6.0).clamp(0.0, 1.0));
            }
        }

        let mut cx = PaintContext::new(scene, text_system, scale_factor);
        meter.paint(meter_bounds, &mut cx);
    }
}

fn draw_memory_row(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    bounds: Bounds,
) {
    scene.draw_quad(
        Quad::new(bounds)
            .with_background(panel_bg().with_alpha(0.7))
            .with_border(panel_border().with_alpha(0.6), 1.0),
    );
    let text = if let Some(mem) = &state.ml_viz.memory_usage {
        format!(
            "GPU {}  CACHE {}  ACT {}",
            format_bytes(mem.gpu_allocated),
            format_bytes(mem.cache_total),
            format_bytes(mem.activations)
        )
    } else {
        "MEMORY TELEMETRY OFFLINE".to_string()
    };
    draw_mono_text(scene, text_system, &text, bounds.x() + 6.0, bounds.y() + 4.0, 9.0, theme::text::PRIMARY);
}

fn draw_entropy_line(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    bounds: Bounds,
) {
    let points = state.ml_viz.entropy_history.len();
    if points < 2 {
        draw_mono_text(
            scene,
            text_system,
            "ENTROPY OFFLINE",
            bounds.x(),
            bounds.y(),
            9.0,
            theme::text::MUTED,
        );
        return;
    }

    let max_entropy = state
        .ml_viz
        .entropy_history
        .iter()
        .copied()
        .fold(0.0f32, |acc, v| acc.max(v))
        .max(1e-3);

    let step = bounds.width() / points as f32;
    for (idx, value) in state.ml_viz.entropy_history.iter().enumerate() {
        let h = (value / max_entropy).clamp(0.0, 1.0) * bounds.height();
        let x = bounds.x() + idx as f32 * step;
        scene.draw_quad(
            Quad::new(Bounds::new(x, bounds.y() + bounds.height() - h, 2.0, h))
                .with_background(accent_orange().with_alpha(0.8)),
        );
    }
}

fn panel(scene: &mut Scene, text_system: &mut TextSystem, bounds: Bounds, title: &str) -> Bounds {
    scene.draw_quad(
        Quad::new(bounds)
            .with_background(panel_bg().with_alpha(0.9))
            .with_border(panel_border().with_alpha(0.9), 1.0),
    );

    draw_mono_text(
        scene,
        text_system,
        title,
        bounds.x() + 8.0,
        bounds.y() + 6.0,
        11.0,
        theme::text::PRIMARY,
    );

    Bounds::new(
        bounds.x() + 8.0,
        bounds.y() + 22.0,
        bounds.width() - 16.0,
        bounds.height() - 30.0,
    )
}

fn draw_empty_state(scene: &mut Scene, text_system: &mut TextSystem, bounds: Bounds, text: &str) {
    let width = measure_mono(text_system, text, 11.0);
    let x = bounds.x() + (bounds.width() - width) * 0.5;
    let y = bounds.y() + (bounds.height() - 11.0) * 0.5;
    draw_mono_text(scene, text_system, text, x, y, 11.0, theme::text::MUTED);
}

fn draw_scrollbar(
    scene: &mut Scene,
    content_x: f32,
    content_width: f32,
    card_y: f32,
    card_height: f32,
    scrollbar_width: f32,
    visible_height: f32,
    total_content_height: f32,
    scroll_offset: f32,
) {
    let scrollbar_x = content_x + content_width - scrollbar_width - 4.0;
    let scrollbar_y = card_y + 8.0;
    let scrollbar_height = card_height - 16.0;

    scene.draw_quad(
        Quad::new(Bounds::new(scrollbar_x, scrollbar_y, scrollbar_width, scrollbar_height))
            .with_background(Hsla::new(0.0, 0.0, 0.02, 0.9))
            .with_border(Hsla::new(0.0, 0.0, 1.0, 0.35), 1.0),
    );

    let thumb_height = (visible_height / total_content_height * scrollbar_height).max(38.0);
    let thumb_progress = scroll_offset / (total_content_height - visible_height).max(1.0);
    let thumb_y = scrollbar_y + 2.0 + thumb_progress * (scrollbar_height - thumb_height - 4.0);

    scene.draw_quad(
        Quad::new(Bounds::new(scrollbar_x + 2.0, thumb_y, scrollbar_width - 4.0, thumb_height))
            .with_background(Hsla::new(0.0, 0.0, 0.1, 0.95))
            .with_border(Hsla::new(0.0, 0.0, 1.0, 0.7), 1.0),
    );
}

fn draw_slider(scene: &mut Scene, bounds: Bounds, value: u32, min: u32, max: u32, color: Hsla) {
    let track_h = 4.0;
    let track_y = bounds.y() + (bounds.height() - track_h) * 0.5;

    scene.draw_quad(
        Quad::new(Bounds::new(bounds.x(), track_y, bounds.width(), track_h))
            .with_background(Hsla::from_hex(0x111820))
            .with_border(panel_border().with_alpha(0.6), 1.0),
    );

    let span = (max - min).max(1) as f32;
    let fill_pct = (value.saturating_sub(min) as f32 / span).clamp(0.0, 1.0);
    let fill_w = fill_pct * bounds.width();
    scene.draw_quad(
        Quad::new(Bounds::new(bounds.x(), track_y, fill_w, track_h))
            .with_background(color.with_alpha(0.8)),
    );

    let thumb_size = 12.0;
    let thumb_x = bounds.x() + fill_pct * (bounds.width() - thumb_size);
    let thumb_y = bounds.y() + (bounds.height() - thumb_size) * 0.5;
    scene.draw_quad(
        Quad::new(Bounds::new(thumb_x, thumb_y, thumb_size, thumb_size))
            .with_background(color)
            .with_border(Hsla::from_hex(0xf5faff).with_alpha(0.8), 1.0),
    );
}

fn draw_mono_text(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    color: Hsla,
) {
    let run = text_system.layout_styled_mono(text, Point::new(x, y), size, color, FontStyle::normal());
    scene.draw_text(run);
}

fn measure_mono(text_system: &mut TextSystem, text: &str, size: f32) -> f32 {
    text_system.measure_styled_mono(text, size, FontStyle::normal())
}

fn format_bytes(bytes: usize) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1}GB", bytes as f32 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.1}MB", bytes as f32 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1}KB", bytes as f32 / 1_000.0)
    } else {
        format!("{bytes}B")
    }
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        return text.to_string();
    }
    let mut out = text.chars().take(max_len).collect::<String>();
    out.push_str("...");
    out
}
