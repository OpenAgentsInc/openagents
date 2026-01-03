use wgpui::{Bounds, FontStyle, Hsla, Point, Quad, Scene, TextSystem, theme};
use js_sys;
use wgpui::animation::AnimatorState;
use wgpui::components::Component;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Frame};
use wgpui::PaintContext;

use crate::state::{AppState, GptOssStage, GptOssStageStatus, GptOssVizState};

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

pub(crate) fn build_gptoss_page(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 1.0, 0.2))
        .distance(34.0)
        .size(1.0)
        .origin(DotsOrigin::Center);
    state.dots_grid.update(AnimatorState::Entered);
    state.dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    let padding = 22.0;
    let content_width = (width - padding * 2.0).min(1200.0);
    let content_x = (width - content_width) * 0.5;
    let card_y = padding;
    let card_height = height - padding * 2.0;
    let card_bounds = Bounds::new(content_x, card_y, content_width, card_height);

    if !state.gptoss.frame_started {
        state.gptoss.frame_started = true;
    }
    let frame_progress = state
        .gptoss
        .frame_animator
        .update(AnimatorState::Entering);

    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    Frame::corners()
        .line_color(Hsla::new(0.0, 0.0, 1.0, 0.75))
        .bg_color(Hsla::new(0.0, 0.0, 0.0, 0.45))
        .glow_color(Hsla::new(0.0, 0.0, 1.0, 0.16))
        .border_color(Hsla::new(0.0, 0.0, 1.0, 0.12))
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

    state.gptoss.content_bounds = Bounds::new(
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

    let scroll_offset = state.gptoss.scroll_offset;
    let mut y = card_y + inner_padding - scroll_offset;

    let title = "GPT-OSS PIPELINE VIZ";
    let title_size = 22.0;
    let title_width = measure_mono(text_system, title, title_size);
    let title_x = inner_x + (inner_width - title_width) * 0.5;
    draw_mono_text(scene, text_system, title, title_x, y, title_size, accent_cyan());
    y += title_size + 6.0;

    let subtitle = "MODEL LOADING + INFERENCE TELEMETRY";
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
    y += subtitle_size + 14.0;

    let button_label = if state.gptoss.load_active {
        "LOADING..."
    } else {
        "START LOAD"
    };
    let button_font = 12.0;
    let button_pad_x = 18.0;
    let button_pad_y = 8.0;
    let button_width = measure_mono(text_system, button_label, button_font) + button_pad_x * 2.0;
    let button_height = button_font + button_pad_y * 2.0;
    let button_x = inner_x + (inner_width - button_width) * 0.5;
    let button_y = y;
    state.gptoss.start_button_bounds = Bounds::new(button_x, button_y, button_width, button_height);

    let button_bg = if state.gptoss.load_active {
        Hsla::new(0.0, 0.0, 0.12, 0.9)
    } else if state.gptoss.start_button_hovered {
        accent_cyan().with_alpha(0.18)
    } else {
        Hsla::new(0.0, 0.0, 0.08, 0.9)
    };
    let button_border = if state.gptoss.load_active {
        theme::text::MUTED
    } else if state.gptoss.start_button_hovered {
        accent_cyan()
    } else {
        panel_border()
    };

    scene.draw_quad(
        Quad::new(state.gptoss.start_button_bounds)
            .with_background(button_bg)
            .with_border(button_border, 1.0),
    );
    draw_mono_text(
        scene,
        text_system,
        button_label,
        button_x + button_pad_x,
        button_y + button_pad_y,
        button_font,
        theme::text::PRIMARY,
    );

    y += button_height + 10.0;
    if let Some(err) = &state.gptoss.load_error {
        draw_mono_text(
            scene,
            text_system,
            &truncate_text(err, 80),
            inner_x,
            y,
            10.0,
            theme::status::ERROR,
        );
        y += 14.0;
    }
    if let Some(url) = &state.gptoss.load_url {
        draw_mono_text(
            scene,
            text_system,
            &format!("GGUF: {}", truncate_text(url, 60)),
            inner_x,
            y,
            10.0,
            theme::text::MUTED,
        );
        y += 14.0;
    }
    if let Some(progress) = load_progress(&state.gptoss) {
        let bar_bounds = Bounds::new(inner_x, y + 2.0, inner_width, 6.0);
        scene.draw_quad(
            Quad::new(bar_bounds)
                .with_background(Hsla::new(0.0, 0.0, 0.1, 0.9))
                .with_border(panel_border(), 1.0),
        );
        let fill_width = (bar_bounds.width() * progress).max(2.0);
        scene.draw_quad(
            Quad::new(Bounds::new(
                bar_bounds.x(),
                bar_bounds.y(),
                fill_width,
                bar_bounds.height(),
            ))
            .with_background(accent_cyan().with_alpha(0.7)),
        );
        y += 12.0;
    }
    y += 6.0;

    let has_data = !state.gptoss.load_stages.is_empty()
        || !state.gptoss.inference_stages.is_empty()
        || !state.gptoss.events.is_empty()
        || !state.gptoss.token_stream.is_empty();

    if !has_data {
        let empty_bounds = Bounds::new(inner_x, y, inner_width, 120.0);
        panel(scene, text_system, empty_bounds, "AWAITING GPT-OSS TELEMETRY");
        draw_empty_state(
            scene,
            text_system,
            empty_bounds,
            "Run `cargo run -p ml --bin gguf_serve` then click START LOAD",
        );
        y += 140.0;
    } else {
        let col_gap = 18.0;
        let col_width = (inner_width - col_gap) * 0.5;
        let left_x = inner_x;
        let right_x = inner_x + col_width + col_gap;

        let stage_height = 220.0;
        let load_bounds = Bounds::new(left_x, y, col_width, stage_height);
        draw_stage_panel(scene, text_system, &state.gptoss, true, load_bounds);
        let infer_bounds = Bounds::new(right_x, y, col_width, stage_height);
        draw_stage_panel(scene, text_system, &state.gptoss, false, infer_bounds);
        y += stage_height + 16.0;

        let io_height = 160.0;
        let io_bounds = Bounds::new(left_x, y, col_width, io_height);
        draw_io_panel(scene, text_system, &state.gptoss, io_bounds);
        let stream_bounds = Bounds::new(right_x, y, col_width, io_height);
        draw_stream_panel(scene, text_system, &state.gptoss, stream_bounds);
        y += io_height + 16.0;

        let topk_height = 180.0;
        let topk_bounds = Bounds::new(left_x, y, col_width, topk_height);
        draw_topk_panel(scene, text_system, &state.gptoss, topk_bounds);
        let stats_bounds = Bounds::new(right_x, y, col_width, topk_height);
        draw_stats_panel(scene, text_system, &state.gptoss, stats_bounds);
        y += topk_height + 16.0;

        let log_height = 240.0;
        let log_bounds = Bounds::new(inner_x, y, inner_width, log_height);
        draw_log_panel(scene, text_system, &state.gptoss, log_bounds);
        y += log_height + 12.0;
    }

    let total_content_height = y - (card_y + inner_padding - scroll_offset);
    state.gptoss.content_height = total_content_height;

    scene.pop_clip();

    let max_scroll = (total_content_height - visible_height).max(0.0);
    state.gptoss.scroll_offset = state.gptoss.scroll_offset.clamp(0.0, max_scroll);

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
            state.gptoss.scroll_offset,
        );
    }

    state.button_bounds = Bounds::ZERO;
    state.left_cta_bounds = Bounds::ZERO;
    state.right_cta_bounds = Bounds::ZERO;
    state.landing_issue_bounds = Bounds::ZERO;
    state.repo_bounds.clear();
}

fn draw_stage_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    gptoss: &GptOssVizState,
    is_load: bool,
    bounds: Bounds,
) {
    let title = if is_load { "LOAD PIPELINE" } else { "INFERENCE PIPELINE" };
    let inner = panel(scene, text_system, bounds, title);
    let stages = if is_load {
        &gptoss.load_stages
    } else {
        &gptoss.inference_stages
    };

    if stages.is_empty() {
        draw_empty_state(scene, text_system, inner, "No stages yet");
        return;
    }

    let mut y = inner.y();
    let line_h = 14.0;
    for stage in stages {
        let status_text = match stage.status {
            GptOssStageStatus::Idle => "IDLE",
            GptOssStageStatus::Running => "RUN",
            GptOssStageStatus::Completed => "OK",
            GptOssStageStatus::Failed => "FAIL",
        };
        let status_color = status_color(stage.status);
        draw_mono_text(scene, text_system, status_text, inner.x(), y, 10.0, status_color);

        let name = truncate_text(&stage.name, 20);
        draw_mono_text(
            scene,
            text_system,
            &name,
            inner.x() + 46.0,
            y,
            10.0,
            theme::text::PRIMARY,
        );

        let mut right_text = String::new();
        if let Some(bytes) = stage.bytes {
            let bytes = format_bytes(bytes as usize);
            if let Some(total) = stage.total_bytes {
                right_text = format!("{}/{}", bytes, format_bytes(total as usize));
            } else {
                right_text = bytes;
            }
        } else if let Some(step) = stage.step {
            if let Some(total) = stage.total_steps {
                right_text = format!("{step}/{total}");
            } else {
                right_text = format!("{step}");
            }
        }

        if !right_text.is_empty() {
            let right_w = measure_mono(text_system, &right_text, 10.0);
            draw_mono_text(
                scene,
                text_system,
                &right_text,
                inner.x() + inner.width() - right_w,
                y,
                10.0,
                theme::text::MUTED,
            );
        }

        y += line_h;
        if y > inner.y() + inner.height() - line_h {
            break;
        }
    }
}

fn draw_io_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    gptoss: &GptOssVizState,
    bounds: Bounds,
) {
    let inner = panel(scene, text_system, bounds, "I/O + MEMORY");

    let weights = find_stage(gptoss, "weights_fetch")
        .or_else(|| find_stage(gptoss, "weights_map"));
    let weights_text = if let Some(stage) = weights {
        if let Some(bytes) = stage.bytes {
            if let Some(total) = stage.total_bytes {
                format!("WEIGHTS: {}/{}", format_bytes(bytes as usize), format_bytes(total as usize))
            } else {
                format!("WEIGHTS: {}", format_bytes(bytes as usize))
            }
        } else {
            "WEIGHTS: --".to_string()
        }
    } else {
        "WEIGHTS: --".to_string()
    };

    draw_mono_text(
        scene,
        text_system,
        &weights_text,
        inner.x(),
        inner.y(),
        11.0,
        theme::text::PRIMARY,
    );

    let tokenizer = find_stage(gptoss, "tokenizer_load");
    let tokenizer_text = if let Some(stage) = tokenizer {
        if let Some(detail) = &stage.detail {
            format!("TOKENIZER: {}", truncate_text(detail, 40))
        } else {
            "TOKENIZER: ok".to_string()
        }
    } else {
        "TOKENIZER: --".to_string()
    };
    draw_mono_text(
        scene,
        text_system,
        &tokenizer_text,
        inner.x(),
        inner.y() + 16.0,
        10.0,
        theme::text::MUTED,
    );

    let mem = gptoss.memory_usage.as_ref();
    let mem_text = if let Some(mem) = mem {
        format!(
            "GPU: {}  CACHE: {}  ACT: {}",
            format_bytes(mem.gpu_allocated),
            format_bytes(mem.cache_total),
            format_bytes(mem.activations),
        )
    } else {
        "GPU: --  CACHE: --  ACT: --".to_string()
    };
    draw_mono_text(
        scene,
        text_system,
        &mem_text,
        inner.x(),
        inner.y() + 34.0,
        10.0,
        theme::text::MUTED,
    );

    let mut ry = inner.y() + 52.0;
    draw_mono_text(
        scene,
        text_system,
        "RESIDENT:",
        inner.x(),
        ry,
        9.0,
        theme::text::MUTED,
    );
    ry += 12.0;
    for tensor in gptoss.resident_tensors.iter().rev().take(3) {
        let label = format!(
            "{} {} {}",
            tensor.kind,
            truncate_text(&tensor.name, 22),
            format_bytes(tensor.bytes),
        );
        draw_mono_text(scene, text_system, &label, inner.x(), ry, 9.0, theme::text::MUTED);
        ry += 12.0;
        if ry > inner.y() + inner.height() - 10.0 {
            break;
        }
    }
}

fn draw_stream_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    gptoss: &GptOssVizState,
    bounds: Bounds,
) {
    let inner = panel(scene, text_system, bounds, "TOKEN STREAM");
    let stream = if gptoss.token_stream.is_empty() {
        "No tokens yet".to_string()
    } else {
        gptoss.token_stream.clone()
    };

    let now_ms = js_sys::Date::now().max(0.0) as u64;
    let pulse = gptoss
        .last_token_ts_ms
        .and_then(|ts| now_ms.checked_sub(ts))
        .map(|delta| {
            if delta >= 420 {
                0.0
            } else {
                1.0 - (delta as f32 / 420.0)
            }
        })
        .unwrap_or(0.0);
    let stream_color = if pulse > 0.0 {
        accent_cyan().with_alpha(0.35 + 0.45 * pulse)
    } else {
        theme::text::PRIMARY
    };

    let speed = gptoss
        .tokens_per_sec
        .map(|v| format!("{v:.1} tok/s"))
        .unwrap_or_else(|| "-- tok/s".to_string());
    let speed_w = measure_mono(text_system, &speed, 10.0);
    draw_mono_text(
        scene,
        text_system,
        &speed,
        inner.x() + inner.width() - speed_w,
        inner.y(),
        10.0,
        accent_cyan(),
    );

    draw_mono_text(
        scene,
        text_system,
        &truncate_text(&stream, 180),
        inner.x(),
        inner.y() + 16.0,
        10.0,
        stream_color,
    );
}

fn draw_topk_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    gptoss: &GptOssVizState,
    bounds: Bounds,
) {
    let inner = panel(scene, text_system, bounds, "TOP-K");

    if gptoss.top_k.is_empty() {
        draw_empty_state(scene, text_system, inner, "No top-k data");
        return;
    }

    let mut y = inner.y();
    let bar_h = 10.0;
    let max_prob = gptoss
        .top_k
        .iter()
        .map(|c| c.probability)
        .fold(0.0f32, |acc, v| acc.max(v))
        .max(1e-4);
    let now_ms = js_sys::Date::now().max(0.0) as u64;
    let pulse = gptoss
        .last_token_ts_ms
        .and_then(|ts| now_ms.checked_sub(ts))
        .map(|delta| {
            if delta >= 420 {
                0.0
            } else {
                1.0 - (delta as f32 / 420.0)
            }
        })
        .unwrap_or(0.0);
    let bar_color = accent_cyan().with_alpha(0.35 + 0.45 * pulse);

    for candidate in &gptoss.top_k {
        let label = truncate_text(&candidate.token_text, 12);
        draw_mono_text(scene, text_system, &label, inner.x(), y, 10.0, theme::text::PRIMARY);

        let bar_w = (candidate.probability / max_prob).clamp(0.0, 1.0) * (inner.width() - 120.0);
        scene.draw_quad(
            Quad::new(Bounds::new(inner.x() + 80.0, y + 2.0, bar_w, bar_h))
                .with_background(bar_color),
        );

        let prob_text = format!("{:.3}", candidate.probability);
        draw_mono_text(
            scene,
            text_system,
            &prob_text,
            inner.x() + inner.width() - 48.0,
            y,
            10.0,
            theme::text::MUTED,
        );
        y += 14.0;
        if y > inner.y() + inner.height() - 12.0 {
            break;
        }
    }
}

fn draw_stats_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    gptoss: &GptOssVizState,
    bounds: Bounds,
) {
    let inner = panel(scene, text_system, bounds, "INFERENCE STATS");
    let entropy = gptoss
        .entropy
        .map(|v| format!("{v:.3}"))
        .unwrap_or_else(|| "--".to_string());

    draw_mono_text(
        scene,
        text_system,
        &format!("ENTROPY: {entropy}"),
        inner.x(),
        inner.y(),
        11.0,
        accent_orange(),
    );

    let cache = gptoss.cache_status.last();
    let cache_text = if let Some(cache) = cache {
        format!(
            "KV CACHE: {}/{}",
            cache.seq_len,
            cache.max_len
        )
    } else {
        "KV CACHE: --".to_string()
    };
    draw_mono_text(
        scene,
        text_system,
        &cache_text,
        inner.x(),
        inner.y() + 16.0,
        10.0,
        theme::text::MUTED,
    );
}

fn draw_log_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    gptoss: &GptOssVizState,
    bounds: Bounds,
) {
    let inner = panel(scene, text_system, bounds, "EVENT LOG");
    if gptoss.events.is_empty() {
        draw_empty_state(scene, text_system, inner, "No events yet");
        return;
    }

    let base_ts = gptoss.start_ts_ms.unwrap_or(0);
    let mut y = inner.y();
    let line_h = 12.0;
    let max_lines = (inner.height() / line_h).floor() as usize;
    let start_idx = gptoss.events.len().saturating_sub(max_lines);

    for entry in gptoss.events.iter().skip(start_idx) {
        let ts = entry
            .ts_ms
            .and_then(|ts| ts.checked_sub(base_ts))
            .map(|ms| format!("{:.2}s", ms as f32 / 1000.0))
            .unwrap_or_else(|| "--".to_string());

        let status_color = status_color(entry.status);
        draw_mono_text(scene, text_system, &ts, inner.x(), y, 9.0, theme::text::MUTED);
        draw_mono_text(
            scene,
            text_system,
            &truncate_text(&entry.message, 90),
            inner.x() + 48.0,
            y,
            9.0,
            status_color,
        );
        y += line_h;
        if y > inner.y() + inner.height() - line_h {
            break;
        }
    }
}

fn find_stage<'a>(gptoss: &'a GptOssVizState, name: &str) -> Option<&'a GptOssStage> {
    gptoss
        .load_stages
        .iter()
        .find(|stage| stage.name == name)
        .or_else(|| gptoss.inference_stages.iter().find(|stage| stage.name == name))
}

fn status_color(status: GptOssStageStatus) -> Hsla {
    match status {
        GptOssStageStatus::Idle => theme::text::MUTED,
        GptOssStageStatus::Running => accent_orange(),
        GptOssStageStatus::Completed => accent_green(),
        GptOssStageStatus::Failed => theme::status::ERROR,
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

    if total_content_height <= visible_height {
        return;
    }

    let thumb_height = (visible_height / total_content_height) * scrollbar_height;
    let max_scroll = total_content_height - visible_height;
    let thumb_progress = scroll_offset / max_scroll;
    let thumb_y = scrollbar_y + (scrollbar_height - thumb_height) * thumb_progress;

    scene.draw_quad(
        Quad::new(Bounds::new(scrollbar_x + 1.0, thumb_y + 1.0, scrollbar_width - 2.0, thumb_height - 2.0))
            .with_background(accent_cyan().with_alpha(0.6)),
    );
}

fn load_progress(gptoss: &GptOssVizState) -> Option<f32> {
    let stage = gptoss
        .load_stages
        .iter()
        .find(|stage| stage.name == "weights_fetch")?;
    let bytes = stage.bytes?;
    let total = stage.total_bytes?;
    if total == 0 {
        return None;
    }
    Some((bytes as f32 / total as f32).clamp(0.0, 1.0))
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
