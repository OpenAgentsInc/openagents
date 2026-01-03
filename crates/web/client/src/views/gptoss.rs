use wgpui::{Bounds, FontStyle, Hsla, Point, Quad, Scene, TextSystem, theme};
use js_sys;
use wgpui::animation::AnimatorState;
use wgpui::components::Component;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Frame, Heatmap};
use wgpui::PaintContext;

use crate::gptoss_runtime::{
    default_max_kv_tokens, default_max_new_tokens, default_sample_temp, default_sample_top_k,
    default_sample_top_p, default_user_prompt, local_gguf_path, local_gguf_dev_url, local_gguf_url,
    local_gguf_serve_cmd, read_query_param,
};
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

    if state.gptoss.drop_active && !state.gptoss.load_active {
        scene.draw_quad(
            Quad::new(card_bounds)
                .with_background(Hsla::new(0.0, 0.0, 0.0, 0.35))
                .with_border(accent_cyan().with_alpha(0.8), 2.0),
        );
        let drop_label = "DROP GGUF TO LOAD";
        let drop_size = 14.0;
        let drop_width = measure_mono(text_system, drop_label, drop_size);
        let drop_x = card_bounds.x() + (card_bounds.width() - drop_width) * 0.5;
        let drop_y = card_bounds.y() + 24.0;
        draw_mono_text(
            scene,
            text_system,
            drop_label,
            drop_x,
            drop_y,
            drop_size,
            accent_cyan(),
        );
    }

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

    ensure_gptoss_inputs(&mut state.gptoss);

    draw_mono_text(
        scene,
        text_system,
        "GGUF SOURCE",
        inner_x,
        y,
        9.0,
        theme::text::MUTED,
    );
    y += 12.0;
    let gguf_input_bounds = Bounds::new(inner_x, y, inner_width, 28.0);
    state.gptoss.gguf_input_bounds = gguf_input_bounds;
    {
        let mut input_cx = PaintContext::new(scene, text_system, scale_factor);
        state.gptoss.gguf_input.paint(gguf_input_bounds, &mut input_cx);
    }
    y += 32.0;

    let file_label = state
        .gptoss
        .gguf_file_label
        .as_deref()
        .unwrap_or("FILE: none");
    let file_button_label = if state.gptoss.gguf_file_label.is_some() {
        "CHANGE FILE"
    } else {
        "PICK FILE"
    };
    let file_button_font = 10.0;
    let file_button_pad_x = 12.0;
    let file_button_pad_y = 6.0;
    let file_button_width =
        measure_mono(text_system, file_button_label, file_button_font) + file_button_pad_x * 2.0;
    let file_button_height = file_button_font + file_button_pad_y * 2.0;
    let file_button_x = inner_x;
    let file_button_y = y;
    state.gptoss.file_button_bounds =
        Bounds::new(file_button_x, file_button_y, file_button_width, file_button_height);
    let file_button_bg = if state.gptoss.file_button_hovered {
        accent_cyan().with_alpha(0.16)
    } else {
        Hsla::new(0.0, 0.0, 0.08, 0.9)
    };
    let file_button_border = if state.gptoss.file_button_hovered {
        accent_cyan()
    } else {
        panel_border()
    };
    scene.draw_quad(
        Quad::new(state.gptoss.file_button_bounds)
            .with_background(file_button_bg)
            .with_border(file_button_border, 1.0),
    );
    draw_mono_text(
        scene,
        text_system,
        file_button_label,
        file_button_x + file_button_pad_x,
        file_button_y + file_button_pad_y,
        file_button_font,
        theme::text::PRIMARY,
    );
    y += file_button_height + 4.0;
    draw_mono_text(
        scene,
        text_system,
        &truncate_text(file_label, 96),
        inner_x,
        y,
        9.0,
        theme::text::MUTED,
    );
    y += 12.0;

    let local_hint = format!("LOCAL: {}  (run: {})", local_gguf_path(), local_gguf_serve_cmd());
    draw_mono_text(
        scene,
        text_system,
        &truncate_text(&local_hint, 96),
        inner_x,
        y,
        9.0,
        theme::text::MUTED,
    );
    y += 12.0;
    draw_mono_text(
        scene,
        text_system,
        "TIP: click LOAD MODEL to pick a local GGUF file",
        inner_x,
        y,
        9.0,
        theme::text::MUTED,
    );
    y += 12.0;
    let local_url = format!("URL: {}", local_gguf_url());
    draw_mono_text(
        scene,
        text_system,
        &truncate_text(&local_url, 96),
        inner_x,
        y,
        9.0,
        theme::text::MUTED,
    );
    y += 12.0;
    let dev_url = format!("DEV URL: {}", local_gguf_dev_url());
    draw_mono_text(
        scene,
        text_system,
        &truncate_text(&dev_url, 96),
        inner_x,
        y,
        9.0,
        theme::text::MUTED,
    );
    y += 20.0;

    draw_mono_text(
        scene,
        text_system,
        "PROMPT",
        inner_x,
        y,
        9.0,
        theme::text::MUTED,
    );
    y += 12.0;
    let prompt_input_bounds = Bounds::new(inner_x, y, inner_width, 28.0);
    state.gptoss.prompt_input_bounds = prompt_input_bounds;
    {
        let mut input_cx = PaintContext::new(scene, text_system, scale_factor);
        state.gptoss.prompt_input.paint(prompt_input_bounds, &mut input_cx);
    }
    y += 40.0;

    let controls_gap = 12.0;
    let control_label_size = 9.0;
    let control_input_height = 26.0;
    let compact_controls = inner_width < 360.0;

    if compact_controls {
        let control_width = inner_width;

        draw_mono_text(
            scene,
            text_system,
            "LAYERS",
            inner_x,
            y,
            control_label_size,
            theme::text::MUTED,
        );
        y += 12.0;
        let layers_bounds =
            Bounds::new(inner_x, y, control_width, control_input_height);
        state.gptoss.layers_input_bounds = layers_bounds;
        {
            let mut input_cx = PaintContext::new(scene, text_system, scale_factor);
            state.gptoss.layers_input.paint(layers_bounds, &mut input_cx);
        }
        y += control_input_height + controls_gap;

        draw_mono_text(
            scene,
            text_system,
            "MAX KV",
            inner_x,
            y,
            control_label_size,
            theme::text::MUTED,
        );
        y += 12.0;
        let max_kv_bounds =
            Bounds::new(inner_x, y, control_width, control_input_height);
        state.gptoss.max_kv_input_bounds = max_kv_bounds;
        {
            let mut input_cx = PaintContext::new(scene, text_system, scale_factor);
            state.gptoss.max_kv_input.paint(max_kv_bounds, &mut input_cx);
        }
        y += control_input_height + controls_gap;

        draw_mono_text(
            scene,
            text_system,
            "MAX NEW",
            inner_x,
            y,
            control_label_size,
            theme::text::MUTED,
        );
        y += 12.0;
        let max_new_bounds =
            Bounds::new(inner_x, y, control_width, control_input_height);
        state.gptoss.max_new_input_bounds = max_new_bounds;
        {
            let mut input_cx = PaintContext::new(scene, text_system, scale_factor);
            state.gptoss.max_new_input.paint(max_new_bounds, &mut input_cx);
        }
        y += control_input_height + 16.0;
    } else {
        let controls_width = (inner_width - controls_gap * 2.0) / 3.0;
        let control_input_y = y + 12.0;
        let control_x0 = inner_x;
        let control_x1 = inner_x + controls_width + controls_gap;
        let control_x2 = inner_x + (controls_width + controls_gap) * 2.0;

        draw_mono_text(
            scene,
            text_system,
            "LAYERS",
            control_x0,
            y,
            control_label_size,
            theme::text::MUTED,
        );
        draw_mono_text(
            scene,
            text_system,
            "MAX KV",
            control_x1,
            y,
            control_label_size,
            theme::text::MUTED,
        );
        draw_mono_text(
            scene,
            text_system,
            "MAX NEW",
            control_x2,
            y,
            control_label_size,
            theme::text::MUTED,
        );

        let layers_bounds = Bounds::new(
            control_x0,
            control_input_y,
            controls_width,
            control_input_height,
        );
        let max_kv_bounds = Bounds::new(
            control_x1,
            control_input_y,
            controls_width,
            control_input_height,
        );
        let max_new_bounds = Bounds::new(
            control_x2,
            control_input_y,
            controls_width,
            control_input_height,
        );
        state.gptoss.layers_input_bounds = layers_bounds;
        state.gptoss.max_kv_input_bounds = max_kv_bounds;
        state.gptoss.max_new_input_bounds = max_new_bounds;
        {
            let mut input_cx = PaintContext::new(scene, text_system, scale_factor);
            state.gptoss.layers_input.paint(layers_bounds, &mut input_cx);
            state.gptoss.max_kv_input.paint(max_kv_bounds, &mut input_cx);
            state.gptoss.max_new_input.paint(max_new_bounds, &mut input_cx);
        }
        y = control_input_y + control_input_height + 16.0;
    }

    let sampling_gap = 12.0;
    let sampling_label_size = 9.0;
    let sampling_input_height = 26.0;
    if compact_controls {
        let control_width = inner_width;
        draw_mono_text(
            scene,
            text_system,
            "SAMPLE",
            inner_x,
            y,
            sampling_label_size,
            theme::text::MUTED,
        );
        y += 12.0;
        let sample_bounds =
            Bounds::new(inner_x, y, control_width, sampling_input_height);
        state.gptoss.sample_input_bounds = sample_bounds;
        {
            let mut input_cx = PaintContext::new(scene, text_system, scale_factor);
            state.gptoss.sample_input.paint(sample_bounds, &mut input_cx);
        }
        y += sampling_input_height + sampling_gap;

        draw_mono_text(
            scene,
            text_system,
            "TEMP",
            inner_x,
            y,
            sampling_label_size,
            theme::text::MUTED,
        );
        y += 12.0;
        let temp_bounds =
            Bounds::new(inner_x, y, control_width, sampling_input_height);
        state.gptoss.temp_input_bounds = temp_bounds;
        {
            let mut input_cx = PaintContext::new(scene, text_system, scale_factor);
            state.gptoss.temp_input.paint(temp_bounds, &mut input_cx);
        }
        y += sampling_input_height + sampling_gap;

        draw_mono_text(
            scene,
            text_system,
            "TOP-K",
            inner_x,
            y,
            sampling_label_size,
            theme::text::MUTED,
        );
        y += 12.0;
        let top_k_bounds =
            Bounds::new(inner_x, y, control_width, sampling_input_height);
        state.gptoss.top_k_input_bounds = top_k_bounds;
        {
            let mut input_cx = PaintContext::new(scene, text_system, scale_factor);
            state.gptoss.top_k_input.paint(top_k_bounds, &mut input_cx);
        }
        y += sampling_input_height + sampling_gap;

        draw_mono_text(
            scene,
            text_system,
            "TOP-P",
            inner_x,
            y,
            sampling_label_size,
            theme::text::MUTED,
        );
        y += 12.0;
        let top_p_bounds =
            Bounds::new(inner_x, y, control_width, sampling_input_height);
        state.gptoss.top_p_input_bounds = top_p_bounds;
        {
            let mut input_cx = PaintContext::new(scene, text_system, scale_factor);
            state.gptoss.top_p_input.paint(top_p_bounds, &mut input_cx);
        }
        y += sampling_input_height + 16.0;
    } else {
        let sampling_width = (inner_width - sampling_gap) / 2.0;
        let sample_x0 = inner_x;
        let sample_x1 = inner_x + sampling_width + sampling_gap;

        draw_mono_text(
            scene,
            text_system,
            "SAMPLE",
            sample_x0,
            y,
            sampling_label_size,
            theme::text::MUTED,
        );
        draw_mono_text(
            scene,
            text_system,
            "TEMP",
            sample_x1,
            y,
            sampling_label_size,
            theme::text::MUTED,
        );
        let row1_input_y = y + 12.0;
        let sample_bounds =
            Bounds::new(sample_x0, row1_input_y, sampling_width, sampling_input_height);
        let temp_bounds =
            Bounds::new(sample_x1, row1_input_y, sampling_width, sampling_input_height);
        state.gptoss.sample_input_bounds = sample_bounds;
        state.gptoss.temp_input_bounds = temp_bounds;
        {
            let mut input_cx = PaintContext::new(scene, text_system, scale_factor);
            state.gptoss.sample_input.paint(sample_bounds, &mut input_cx);
            state.gptoss.temp_input.paint(temp_bounds, &mut input_cx);
        }
        y = row1_input_y + sampling_input_height + sampling_gap;

        draw_mono_text(
            scene,
            text_system,
            "TOP-K",
            sample_x0,
            y,
            sampling_label_size,
            theme::text::MUTED,
        );
        draw_mono_text(
            scene,
            text_system,
            "TOP-P",
            sample_x1,
            y,
            sampling_label_size,
            theme::text::MUTED,
        );
        let row2_input_y = y + 12.0;
        let top_k_bounds =
            Bounds::new(sample_x0, row2_input_y, sampling_width, sampling_input_height);
        let top_p_bounds =
            Bounds::new(sample_x1, row2_input_y, sampling_width, sampling_input_height);
        state.gptoss.top_k_input_bounds = top_k_bounds;
        state.gptoss.top_p_input_bounds = top_p_bounds;
        {
            let mut input_cx = PaintContext::new(scene, text_system, scale_factor);
            state.gptoss.top_k_input.paint(top_k_bounds, &mut input_cx);
            state.gptoss.top_p_input.paint(top_p_bounds, &mut input_cx);
        }
        y = row2_input_y + sampling_input_height + 16.0;
    }

    let has_source = state.gptoss.gguf_file_label.is_some()
        || !state.gptoss.gguf_input.get_value().trim().is_empty();
    let button_label = if state.gptoss.load_active {
        "LOADING..."
    } else if has_source {
        "LOAD MODEL"
    } else {
        "PICK FILE"
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

    y += button_height + 8.0;
    let layers_hint = "LAYERS: default ALL (input or ?layers=N)";
    draw_mono_text(
        scene,
        text_system,
        &truncate_text(layers_hint, 72),
        inner_x,
        y,
        9.0,
        theme::text::MUTED,
    );
    y += 18.0;
    if let Some(err) = &state.gptoss.load_error {
        for line in err.lines().take(3) {
            draw_mono_text(
                scene,
                text_system,
                &truncate_text(line, 96),
                inner_x,
                y,
                10.0,
                theme::status::ERROR,
            );
            y += 14.0;
        }
    }
    if let Some(err) = &state.gptoss.inference_error {
        let mut first = true;
        for line in err.lines().take(2) {
            let label = if first {
                format!("INFER: {}", line)
            } else {
                line.to_string()
            };
            first = false;
            draw_mono_text(
                scene,
                text_system,
                &truncate_text(&label, 96),
                inner_x,
                y,
                10.0,
                theme::status::ERROR,
            );
            y += 14.0;
        }
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
        let pct = format!("{:.1}%", progress * 100.0);
        let pct_w = measure_mono(text_system, &pct, 9.0);
        draw_mono_text(
            scene,
            text_system,
            &pct,
            bar_bounds.x() + bar_bounds.width() - pct_w,
            bar_bounds.y() - 10.0,
            9.0,
            theme::text::MUTED,
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
            "Run `cargo run -p ml --bin gguf_serve` or click LOAD MODEL to pick a file",
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

        let io_height = 240.0;
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

        let history_height = 200.0;
        let history_bounds = Bounds::new(inner_x, y, inner_width, history_height);
        draw_probability_panel(scene, text_system, &state.gptoss, history_bounds);
        y += history_height + 16.0;

        let layer_height = 180.0;
        let layer_bounds = Bounds::new(inner_x, y, inner_width, layer_height);
        draw_layer_panel(scene, text_system, &state.gptoss, layer_bounds);
        y += layer_height + 16.0;

        let attention_height = 220.0;
        let attention_bounds = Bounds::new(inner_x, y, inner_width, attention_height);
        draw_attention_panel(scene, text_system, &mut state.gptoss, attention_bounds, scale_factor);
        y += attention_height + 16.0;

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

        let (name, name_color) = if let Some(detail) = stage.detail.as_ref() {
            let detail_lower = detail.to_ascii_lowercase();
            let warn_cpu = detail_lower.contains("cpu") && !detail_lower.contains("cpu_fallback=off");
            let color = if warn_cpu { accent_orange() } else { theme::text::PRIMARY };
            (truncate_text(&format!("{}: {}", stage.name, detail), 32), color)
        } else {
            (truncate_text(&stage.name, 32), theme::text::PRIMARY)
        };
        draw_mono_text(
            scene,
            text_system,
            &name,
            inner.x() + 46.0,
            y,
            10.0,
            name_color,
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

        if matches!(stage.status, GptOssStageStatus::Running) {
            let ratio = if let (Some(bytes), Some(total)) = (stage.bytes, stage.total_bytes) {
                if total > 0 {
                    Some((bytes as f32 / total as f32).clamp(0.0, 1.0))
                } else {
                    None
                }
            } else if let (Some(step), Some(total)) = (stage.step, stage.total_steps) {
                if total > 0 {
                    Some((step as f32 / total as f32).clamp(0.0, 1.0))
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(ratio) = ratio {
                let bar_y = y + line_h - 3.0;
                let bar_x = inner.x() + 46.0;
                let bar_w = inner.width() - 46.0;
                scene.draw_quad(
                    Quad::new(Bounds::new(bar_x, bar_y, bar_w, 2.0))
                        .with_background(panel_border().with_alpha(0.4)),
                );
                scene.draw_quad(
                    Quad::new(Bounds::new(bar_x, bar_y, bar_w * ratio, 2.0))
                        .with_background(accent_cyan().with_alpha(0.7)),
                );
            }
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

    let weights_detail = weights
        .and_then(|stage| stage.detail.as_ref())
        .map(|detail| format!("LOAD: {}", truncate_text(detail, 56)))
        .unwrap_or_else(|| "LOAD: --".to_string());
    draw_mono_text(
        scene,
        text_system,
        &weights_detail,
        inner.x(),
        inner.y() + 12.0,
        9.0,
        theme::text::MUTED,
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
        inner.y() + 28.0,
        10.0,
        theme::text::MUTED,
    );

    let prompt_stage = find_stage(gptoss, "prompt_encode");
    let prompt_text = if let Some(stage) = prompt_stage {
        if let Some(detail) = &stage.detail {
            format!("PROMPT: {}", truncate_text(detail, 40))
        } else {
            "PROMPT: ok".to_string()
        }
    } else {
        "PROMPT: --".to_string()
    };
    draw_mono_text(
        scene,
        text_system,
        &prompt_text,
        inner.x(),
        inner.y() + 42.0,
        9.0,
        theme::text::MUTED,
    );

    let token_limits = gptoss
        .token_limits
        .as_ref()
        .map(|limits| format!("TOKENS: {limits}"))
        .unwrap_or_else(|| "TOKENS: --".to_string());
    draw_mono_text(
        scene,
        text_system,
        &token_limits,
        inner.x(),
        inner.y() + 56.0,
        10.0,
        theme::text::MUTED,
    );

    let gpu_buffers = find_stage(gptoss, "gpu_alloc")
        .and_then(|stage| stage.detail.as_ref())
        .and_then(|detail| parse_stage_value(detail, "buffers"));
    let mem = gptoss.memory_usage.as_ref();
    let mem_text = if let Some(mem) = mem {
        if let Some(buffers) = gpu_buffers {
            format!(
                "GPU: {} (buf {buffers})  CACHE: {}  ACT: {}",
                format_bytes(mem.gpu_allocated),
                format_bytes(mem.cache_total),
                format_bytes(mem.activations),
            )
        } else {
            format!(
                "GPU: {}  CACHE: {}  ACT: {}",
                format_bytes(mem.gpu_allocated),
                format_bytes(mem.cache_total),
                format_bytes(mem.activations),
            )
        }
    } else {
        "GPU: --  CACHE: --  ACT: --".to_string()
    };
    draw_mono_text(
        scene,
        text_system,
        &mem_text,
        inner.x(),
        inner.y() + 72.0,
        10.0,
        theme::text::MUTED,
    );

    let mut gauge_offset = 0.0;
    if let (Some(mem), Some(limits)) = (mem, gptoss.gpu_limits.as_ref()) {
        let max_buffer = parse_limit_bytes(limits, "max_buffer")
            .or_else(|| parse_limit_bytes(limits, "max_storage"));
        if let Some(max_buffer) = max_buffer {
            if max_buffer > 0 {
                let ratio = (mem.gpu_allocated as f32 / max_buffer as f32)
                    .clamp(0.0, 1.0);
                let bar_bounds = Bounds::new(inner.x(), inner.y() + 84.0, inner.width(), 4.0);
                scene.draw_quad(
                    Quad::new(bar_bounds)
                        .with_background(panel_border().with_alpha(0.35)),
                );
                scene.draw_quad(
                    Quad::new(Bounds::new(
                        bar_bounds.x(),
                        bar_bounds.y(),
                        bar_bounds.width() * ratio,
                        bar_bounds.height(),
                    ))
                    .with_background(accent_cyan().with_alpha(0.7)),
                );
                gauge_offset = 8.0;
            }
        }
    }

    let cache_base_y = inner.y() + 88.0 + gauge_offset;
    let cache_line_gap = 12.0;

    let token_cache = find_stage(gptoss, "token_cache")
        .and_then(|stage| stage.detail.as_ref())
        .map(|detail| format!("TOKEN CACHE: {}", truncate_text(detail, 46)))
        .unwrap_or_else(|| "TOKEN CACHE: --".to_string());
    draw_mono_text(
        scene,
        text_system,
        &token_cache,
        inner.x(),
        cache_base_y,
        9.0,
        theme::text::MUTED,
    );

    let tensor_cache = find_stage(gptoss, "tensor_cache")
        .and_then(|stage| stage.detail.as_ref())
        .map(|detail| format!("TENSOR CACHE: {}", truncate_text(detail, 46)))
        .unwrap_or_else(|| "TENSOR CACHE: --".to_string());
    draw_mono_text(
        scene,
        text_system,
        &tensor_cache,
        inner.x(),
        cache_base_y + cache_line_gap,
        9.0,
        theme::text::MUTED,
    );

    let q8_cache = find_stage(gptoss, "q8_0_cache")
        .and_then(|stage| stage.detail.as_ref())
        .map(|detail| format!("Q8_0 CACHE: {}", truncate_text(detail, 46)))
        .unwrap_or_else(|| "Q8_0 CACHE: --".to_string());
    draw_mono_text(
        scene,
        text_system,
        &q8_cache,
        inner.x(),
        cache_base_y + cache_line_gap * 2.0,
        9.0,
        theme::text::MUTED,
    );

    let expert_cache = find_stage(gptoss, "expert_cache")
        .and_then(|stage| stage.detail.as_ref())
        .map(|detail| format!("EXPERT CACHE: {}", truncate_text(detail, 46)))
        .unwrap_or_else(|| "EXPERT CACHE: --".to_string());
    draw_mono_text(
        scene,
        text_system,
        &expert_cache,
        inner.x(),
        cache_base_y + cache_line_gap * 3.0,
        9.0,
        theme::text::MUTED,
    );

    let limits_lines = gptoss
        .gpu_limits
        .as_ref()
        .map(|limits| format!("LIMITS: {limits}"))
        .unwrap_or_else(|| "LIMITS: --".to_string());
    let mut limits_lines = wrap_tokens(&limits_lines, 62, 2);
    if limits_lines.is_empty() {
        limits_lines.push("LIMITS: --".to_string());
    }
    for (idx, line) in limits_lines.iter().enumerate() {
        draw_mono_text(
            scene,
            text_system,
            line,
            inner.x(),
            inner.y() + 140.0 + gauge_offset + (idx as f32 * 12.0),
            9.0,
            theme::text::MUTED,
        );
    }

    let mut ry = inner.y()
        + 156.0
        + gauge_offset
        + ((limits_lines.len().saturating_sub(1) as f32) * 12.0);
    let show_scan = gptoss.load_active
        || (gptoss.resident_tensors.is_empty() && !gptoss.recent_tensors.is_empty());
    let label = if show_scan { "LOAD SCAN:" } else { "RESIDENT:" };
    draw_mono_text(
        scene,
        text_system,
        label,
        inner.x(),
        ry,
        9.0,
        theme::text::MUTED,
    );
    ry += 12.0;
    if show_scan {
        for name in gptoss.recent_tensors.iter().rev().take(3) {
            let line = truncate_text(name, 40);
            draw_mono_text(scene, text_system, &line, inner.x(), ry, 9.0, theme::text::MUTED);
            ry += 12.0;
            if ry > inner.y() + inner.height() - 10.0 {
                break;
            }
        }
    } else {
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
    let token_id_text = gptoss
        .last_token_id
        .map(|id| format!("id={id}"))
        .unwrap_or_else(|| "id=--".to_string());
    let token_id_w = measure_mono(text_system, &token_id_text, 10.0);
    draw_mono_text(
        scene,
        text_system,
        &token_id_text,
        inner.x() + inner.width() - speed_w - token_id_w - 10.0,
        inner.y(),
        10.0,
        theme::text::MUTED,
    );
    draw_mono_text(
        scene,
        text_system,
        &speed,
        inner.x() + inner.width() - speed_w,
        inner.y(),
        10.0,
        accent_cyan(),
    );

    let show_cursor = gptoss.load_active || !gptoss.token_stream.is_empty();
    let char_w = measure_mono(text_system, "W", 10.0).max(1.0);
    let max_chars = (inner.width() / char_w).floor().max(24.0) as usize;
    let tail = tail_chars(&stream, max_chars.saturating_mul(3));
    let mut lines = wrap_tokens(&tail, max_chars.saturating_sub(2), 3);
    if lines.is_empty() {
        lines.push(String::new());
    }
    if show_cursor {
        if let Some(last) = lines.last_mut() {
            last.push_str(" |");
        }
    }
    let mut line_y = inner.y() + 16.0;
    for line in lines {
        draw_mono_text(scene, text_system, &line, inner.x(), line_y, 10.0, stream_color);
        line_y += 12.0;
    }
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
        let label = token_label(&candidate.token_text, candidate.token_id, 12);
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

fn draw_attention_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    gptoss: &mut GptOssVizState,
    bounds: Bounds,
    scale_factor: f32,
) {
    let inner = panel(scene, text_system, bounds, "ATTENTION");

    let label_size = 9.0;
    let layer_label = format!("LAYER {}", gptoss.attention_selected_layer);
    let head_label = format!("HEAD {}", gptoss.attention_selected_head);
    let head_label_w = measure_mono(text_system, &head_label, label_size);
    draw_mono_text(
        scene,
        text_system,
        &layer_label,
        inner.x(),
        inner.y(),
        label_size,
        theme::text::MUTED,
    );
    draw_mono_text(
        scene,
        text_system,
        &head_label,
        inner.x() + inner.width() - head_label_w,
        inner.y(),
        label_size,
        theme::text::MUTED,
    );

    let slider_y = inner.y() + 12.0;
    let slider_w = inner.width() * 0.45;
    gptoss.layer_slider_bounds = Bounds::new(inner.x(), slider_y, slider_w, 18.0);
    gptoss.head_slider_bounds = Bounds::new(
        inner.x() + inner.width() - slider_w,
        slider_y,
        slider_w,
        18.0,
    );
    draw_slider(
        scene,
        gptoss.layer_slider_bounds,
        gptoss.attention_selected_layer as u32,
        0,
        gptoss.max_layers.saturating_sub(1) as u32,
        accent_cyan(),
    );
    draw_slider(
        scene,
        gptoss.head_slider_bounds,
        gptoss.attention_selected_head as u32,
        0,
        gptoss.max_heads.saturating_sub(1) as u32,
        accent_orange(),
    );

    let heatmap_bounds = Bounds::new(
        inner.x(),
        slider_y + 24.0,
        inner.width(),
        inner.height() - 32.0,
    );
    scene.draw_quad(
        Quad::new(heatmap_bounds)
            .with_background(panel_bg().with_alpha(0.8))
            .with_border(panel_border().with_alpha(0.6), 1.0),
    );

    if let Some(weights) = &gptoss.attention_weights {
        if gptoss.attention_layer != gptoss.attention_selected_layer
            || gptoss.attention_head != gptoss.attention_selected_head
        {
            draw_empty_state(scene, text_system, heatmap_bounds, "No attention telemetry");
            return;
        }
        if weights.is_empty() || weights.first().map(|row| row.is_empty()).unwrap_or(true) {
            draw_empty_state(scene, text_system, heatmap_bounds, "No attention telemetry");
            return;
        }
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
            .mid_color(Some(accent_cyan()))
            .high_color(Hsla::from_hex(0xf8fbff));
        let mut cx = PaintContext::new(scene, text_system, scale_factor);
        heatmap.paint(heatmap_bounds, &mut cx);
    } else {
        draw_empty_state(scene, text_system, heatmap_bounds, "No attention telemetry");
    }
}

fn draw_probability_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    gptoss: &GptOssVizState,
    bounds: Bounds,
) {
    let inner = panel(scene, text_system, bounds, "PROBABILITY HISTORY");
    if gptoss.probability_history.is_empty() {
        draw_empty_state(scene, text_system, inner, "No token history");
        return;
    }

    let history_len = gptoss.probability_history.len();
    let max_cols = (inner.width() / 120.0).floor().max(1.0) as usize;
    let cols = history_len.min(max_cols).max(1);
    let col_width = inner.width() / cols as f32;
    let bar_height = 8.0;
    let row_gap = 4.0;
    let max_rows = 5usize;
    let label_height = 12.0;

    for col in 0..cols {
        let idx = history_len - cols + col;
        let candidates = &gptoss.probability_history[idx];
        if candidates.is_empty() {
            continue;
        }
        let x = inner.x() + col as f32 * col_width;
        let y = inner.y();
        let top_label = token_label(&candidates[0].token_text, candidates[0].token_id, 10);
        draw_mono_text(
            scene,
            text_system,
            &top_label,
            x,
            y,
            9.0,
            theme::text::PRIMARY,
        );

        let max_prob = candidates
            .iter()
            .take(max_rows)
            .map(|c| c.probability)
            .fold(0.0f32, |acc, v| acc.max(v))
            .max(1e-4);
        for (row_idx, candidate) in candidates.iter().take(max_rows).enumerate() {
            let norm = (candidate.probability / max_prob).clamp(0.0, 1.0);
            let bar_w = (col_width - 8.0) * norm;
            let y_offset = y + label_height + row_idx as f32 * (bar_height + row_gap);
            let color = if row_idx == 0 {
                accent_cyan().with_alpha(0.8)
            } else {
                theme::text::MUTED.with_alpha(0.5)
            };
            scene.draw_quad(
                Quad::new(Bounds::new(x, y_offset, bar_w, bar_height)).with_background(color),
            );
        }
    }

    if gptoss.entropy_history.len() >= 2 {
        let spark_height = 18.0;
        let spark_y = inner.y() + inner.height() - spark_height;
        let points = gptoss.entropy_history.len();
        let step = inner.width() / points as f32;
        let max_entropy = gptoss
            .entropy_history
            .iter()
            .fold(0.0f32, |acc, v| acc.max(*v))
            .max(1e-4);
        for (idx, entropy) in gptoss.entropy_history.iter().enumerate() {
            let norm = (*entropy / max_entropy).clamp(0.0, 1.0);
            let bar_h = norm * spark_height;
            let x = inner.x() + idx as f32 * step;
            scene.draw_quad(
                Quad::new(Bounds::new(x, spark_y + spark_height - bar_h, 2.0, bar_h))
                    .with_background(accent_orange().with_alpha(0.35)),
            );
        }
    }
}

fn draw_layer_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    gptoss: &GptOssVizState,
    bounds: Bounds,
) {
    let inner = panel(scene, text_system, bounds, "LAYER ACTIVITY");
    if gptoss.layer_activations.is_empty() {
        draw_empty_state(scene, text_system, inner, "No layer telemetry");
        return;
    }

    let mut max_attn = 0.0f32;
    let mut max_mlp = 0.0f32;
    let mut max_out = 0.0f32;
    for act in &gptoss.layer_activations {
        max_attn = max_attn.max(act.attention_norm);
        max_mlp = max_mlp.max(act.mlp_norm);
        max_out = max_out.max(act.output_norm);
    }
    max_attn = max_attn.max(1e-4);
    max_mlp = max_mlp.max(1e-4);
    max_out = max_out.max(1e-4);

    let row_height = 12.0;
    let rows_per_col = ((inner.height() - row_height).max(row_height) / row_height) as usize;
    let total = gptoss.layer_activations.len();
    let cols = if total > rows_per_col { 2 } else { 1 };
    let col_width = inner.width() / cols as f32;
    let label_w = 26.0;
    let bar_gap = 4.0;
    let bars_total = (col_width - label_w - 6.0).max(0.0);
    let bar_w = (bars_total - bar_gap * 2.0).max(0.0) / 3.0;

    let legend_y = inner.y();
    draw_mono_text(
        scene,
        text_system,
        "A",
        inner.x() + label_w,
        legend_y,
        9.0,
        accent_cyan(),
    );
    draw_mono_text(
        scene,
        text_system,
        "M",
        inner.x() + label_w + bar_w + bar_gap,
        legend_y,
        9.0,
        accent_orange(),
    );
    draw_mono_text(
        scene,
        text_system,
        "O",
        inner.x() + label_w + (bar_w + bar_gap) * 2.0,
        legend_y,
        9.0,
        accent_green(),
    );

    for (idx, act) in gptoss
        .layer_activations
        .iter()
        .take(rows_per_col * cols)
        .enumerate()
    {
        let col = idx / rows_per_col;
        let row = idx % rows_per_col;
        let x = inner.x() + col as f32 * col_width;
        let y = inner.y() + row_height + row as f32 * row_height;
        let label = format!("L{:02}", act.layer);
        draw_mono_text(scene, text_system, &label, x, y, 9.0, theme::text::MUTED);

        let bar_x = x + label_w;
        let attn_w = (act.attention_norm / max_attn).clamp(0.0, 1.0) * bar_w;
        let mlp_w = (act.mlp_norm / max_mlp).clamp(0.0, 1.0) * bar_w;
        let out_w = (act.output_norm / max_out).clamp(0.0, 1.0) * bar_w;
        scene.draw_quad(
            Quad::new(Bounds::new(bar_x, y + 2.0, attn_w, 6.0))
                .with_background(accent_cyan().with_alpha(0.7)),
        );
        scene.draw_quad(
            Quad::new(Bounds::new(
                bar_x + bar_w + bar_gap,
                y + 2.0,
                mlp_w,
                6.0,
            ))
            .with_background(accent_orange().with_alpha(0.7)),
        );
        scene.draw_quad(
            Quad::new(Bounds::new(
                bar_x + (bar_w + bar_gap) * 2.0,
                y + 2.0,
                out_w,
                6.0,
            ))
            .with_background(accent_green().with_alpha(0.7)),
        );
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

    let mut y = inner.y();
    draw_mono_text(
        scene,
        text_system,
        &format!("ENTROPY: {entropy}"),
        inner.x(),
        y,
        11.0,
        accent_orange(),
    );

    let stage_text = gptoss
        .current_stage
        .as_ref()
        .map(|stage| format!("STAGE: {}", truncate_text(stage, 32)))
        .unwrap_or_else(|| "STAGE: --".to_string());
    y += 14.0;
    draw_mono_text(
        scene,
        text_system,
        &stage_text,
        inner.x(),
        y,
        10.0,
        theme::text::MUTED,
    );

    let cache = gptoss.cache_status.last();
    let cache_text = if let Some(cache) = cache {
        if cache.memory_bytes > 0 {
            format!(
                "KV CACHE: {}/{} {}",
                cache.seq_len,
                cache.max_len,
                format_bytes(cache.memory_bytes),
            )
        } else {
            format!("KV CACHE: {}/{}", cache.seq_len, cache.max_len)
        }
    } else {
        "KV CACHE: --".to_string()
    };
    y += 14.0;
    draw_mono_text(
        scene,
        text_system,
        &cache_text,
        inner.x(),
        y,
        10.0,
        theme::text::MUTED,
    );
    if let Some(cache) = cache {
        if cache.max_len > 0 {
            let ratio = (cache.seq_len as f32 / cache.max_len as f32).clamp(0.0, 1.0);
            let bar_y = y + 12.0;
            let bar_h = 4.0;
            scene.draw_quad(
                Quad::new(Bounds::new(inner.x(), bar_y, inner.width(), bar_h))
                    .with_background(panel_border().with_alpha(0.4)),
            );
            scene.draw_quad(
                Quad::new(Bounds::new(inner.x(), bar_y, inner.width() * ratio, bar_h))
                    .with_background(accent_cyan().with_alpha(0.7)),
            );
            y += 8.0;
        }
    }

    let layers_text = gptoss
        .active_layers
        .map(|layers| format!("LAYERS: {layers}"))
        .unwrap_or_else(|| "LAYERS: --".to_string());
    y += 14.0;
    draw_mono_text(
        scene,
        text_system,
        &layers_text,
        inner.x(),
        y,
        10.0,
        theme::text::MUTED,
    );

    let attn_text = gptoss
        .attention_mode
        .as_ref()
        .map(|mode| format!("ATTN: {}", truncate_text(mode, 26)))
        .unwrap_or_else(|| "ATTN: --".to_string());
    y += 14.0;
    draw_mono_text(
        scene,
        text_system,
        &attn_text,
        inner.x(),
        y,
        10.0,
        theme::text::MUTED,
    );

    let rms_text = stage_kernel_mode(gptoss, "attn_norm")
        .map(|mode| format!("RMSNORM: {mode}"))
        .unwrap_or_else(|| "RMSNORM: --".to_string());
    y += 14.0;
    draw_mono_text(
        scene,
        text_system,
        &rms_text,
        inner.x(),
        y,
        10.0,
        theme::text::MUTED,
    );

    let rope_text = stage_rope_mode(gptoss)
        .map(|mode| format!("ROPE: {mode}"))
        .unwrap_or_else(|| "ROPE: --".to_string());
    y += 14.0;
    draw_mono_text(
        scene,
        text_system,
        &rope_text,
        inner.x(),
        y,
        10.0,
        theme::text::MUTED,
    );

    let moe_text = gptoss
        .moe_mode
        .as_ref()
        .map(|mode| format!("MOE: {}", truncate_text(mode, 26)))
        .unwrap_or_else(|| "MOE: --".to_string());
    y += 14.0;
    draw_mono_text(
        scene,
        text_system,
        &moe_text,
        inner.x(),
        y,
        10.0,
        theme::text::MUTED,
    );

    let coherence_text = find_inference_stage(gptoss, "coherence_check")
        .and_then(|stage| stage.detail.as_ref())
        .map(|detail| format!("COHERENCE: {}", truncate_text(detail, 24)))
        .unwrap_or_else(|| "COHERENCE: --".to_string());
    y += 14.0;
    draw_mono_text(
        scene,
        text_system,
        &coherence_text,
        inner.x(),
        y,
        10.0,
        theme::text::MUTED,
    );

    let budget_text = find_inference_stage(gptoss, "decode_budget")
        .and_then(|stage| stage.detail.as_ref())
        .map(|detail| format!("DECODE BUDGET: {}", truncate_text(detail, 24)))
        .unwrap_or_else(|| "DECODE BUDGET: --".to_string());
    y += 14.0;
    draw_mono_text(
        scene,
        text_system,
        &budget_text,
        inner.x(),
        y,
        10.0,
        theme::text::MUTED,
    );

    let stop_reason = gptoss
        .inference_stages
        .iter()
        .find(|stage| stage.name == "generation")
        .and_then(|stage| stage.detail.as_ref())
        .map(|detail| format!("STOP: {}", truncate_text(detail, 26)))
        .unwrap_or_else(|| "STOP: --".to_string());
    y += 14.0;
    draw_mono_text(
        scene,
        text_system,
        &stop_reason,
        inner.x(),
        y,
        10.0,
        theme::text::MUTED,
    );

    let sample_text = gptoss
        .sampling_mode
        .as_ref()
        .map(|mode| format!("SAMPLE: {}", truncate_text(mode, 26)))
        .unwrap_or_else(|| "SAMPLE: --".to_string());
    y += 14.0;
    draw_mono_text(
        scene,
        text_system,
        &sample_text,
        inner.x(),
        y,
        10.0,
        theme::text::MUTED,
    );

    let fallback_text = gptoss
        .cpu_fallback
        .as_ref()
        .map(|mode| format!("CPU FALLBACK: {}", truncate_text(mode, 10)))
        .unwrap_or_else(|| "CPU FALLBACK: --".to_string());
    y += 14.0;
    draw_mono_text(
        scene,
        text_system,
        &fallback_text,
        inner.x(),
        y,
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

fn find_inference_stage<'a>(
    gptoss: &'a GptOssVizState,
    name: &str,
) -> Option<&'a GptOssStage> {
    gptoss
        .inference_stages
        .iter()
        .rev()
        .find(|stage| stage.name == name)
}

fn stage_kernel_mode(gptoss: &GptOssVizState, name: &str) -> Option<&'static str> {
    let detail = find_inference_stage(gptoss, name)?.detail.as_ref()?;
    kernel_mode_from_detail(detail)
}

fn stage_rope_mode(gptoss: &GptOssVizState) -> Option<&'static str> {
    let detail = find_inference_stage(gptoss, "rope")?.detail.as_ref()?;
    rope_mode_from_detail(detail)
}

fn kernel_mode_from_detail(detail: &str) -> Option<&'static str> {
    let lower = detail.to_ascii_lowercase();
    let has_gpu = lower.contains("gpu");
    let has_cpu = lower.contains("cpu");
    match (has_gpu, has_cpu) {
        (true, false) => Some("GPU"),
        (false, true) => Some("CPU"),
        (true, true) => Some("MIXED"),
        _ => None,
    }
}

fn rope_mode_from_detail(detail: &str) -> Option<&'static str> {
    let lower = detail.to_ascii_lowercase();
    let q_gpu = lower.contains("q=gpu");
    let q_cpu = lower.contains("q=cpu");
    let k_gpu = lower.contains("k=gpu");
    let k_cpu = lower.contains("k=cpu");
    if (q_gpu || q_cpu) && (k_gpu || k_cpu) {
        if q_gpu && k_gpu {
            return Some("GPU");
        }
        if q_cpu && k_cpu {
            return Some("CPU");
        }
        return Some("MIXED");
    }
    kernel_mode_from_detail(detail)
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

fn load_progress(gptoss: &GptOssVizState) -> Option<f32> {
    if let Some(progress) = gptoss.load_progress {
        return Some(progress.clamp(0.0, 1.0));
    }
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

fn token_label(text: &str, token_id: u32, max_len: usize) -> String {
    if text.trim().is_empty() {
        format!("#{token_id}")
    } else {
        truncate_text(text, max_len)
    }
}

fn tail_chars(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        return text.to_string();
    }
    let mut tail = text.chars().rev().take(max_len).collect::<Vec<_>>();
    tail.reverse();
    tail.into_iter().collect()
}

fn wrap_tokens(text: &str, max_len: usize, max_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for token in text.split_whitespace() {
        let pending_len = if current.is_empty() {
            token.len()
        } else {
            current.len() + 1 + token.len()
        };
        if pending_len > max_len && !current.is_empty() {
            lines.push(current);
            if lines.len() >= max_lines {
                return lines;
            }
            current = token.to_string();
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(token);
        }
    }
    if !current.is_empty() && lines.len() < max_lines {
        lines.push(current);
    }
    lines
}

fn parse_stage_value(detail: &str, key: &str) -> Option<usize> {
    for part in detail.split_whitespace() {
        if let Some(value) = part.strip_prefix(&format!("{key}=")) {
            if let Ok(parsed) = value.parse::<usize>() {
                return Some(parsed);
            }
        }
    }
    None
}

fn parse_limit_bytes(detail: &str, key: &str) -> Option<u64> {
    for part in detail.split_whitespace() {
        if let Some(value) = part.strip_prefix(&format!("{key}=")) {
            return parse_human_bytes(value);
        }
    }
    None
}

fn parse_human_bytes(value: &str) -> Option<u64> {
    if let Some(num) = value.strip_suffix("GB") {
        let parsed = num.parse::<f64>().ok()?;
        return Some((parsed * 1_000_000_000.0) as u64);
    }
    if let Some(num) = value.strip_suffix("MB") {
        let parsed = num.parse::<f64>().ok()?;
        return Some((parsed * 1_000_000.0) as u64);
    }
    if let Some(num) = value.strip_suffix("KB") {
        let parsed = num.parse::<f64>().ok()?;
        return Some((parsed * 1_000.0) as u64);
    }
    if let Some(num) = value.strip_suffix("B") {
        return num.parse::<u64>().ok();
    }
    None
}

fn ensure_gptoss_inputs(gptoss: &mut GptOssVizState) {
    if gptoss.inputs_initialized {
        return;
    }
    let gguf_value = read_query_param("gguf").filter(|value| !value.is_empty());
    if let Some(value) = gguf_value {
        gptoss.gguf_input.set_value(value);
    }

    let prompt_value = read_query_param("prompt")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(default_user_prompt);
    if !prompt_value.is_empty() {
        gptoss.prompt_input.set_value(prompt_value);
    }

    if let Some(value) = read_query_param("layers").filter(|value| !value.is_empty()) {
        gptoss.layers_input.set_value(value);
    } else if gptoss.layers_input.get_value().trim().is_empty() {
        gptoss.layers_input.set_value("all");
    }
    if let Some(value) = read_query_param("max_kv").filter(|value| !value.is_empty()) {
        gptoss.max_kv_input.set_value(value);
    } else if gptoss.max_kv_input.get_value().trim().is_empty() {
        gptoss
            .max_kv_input
            .set_value(default_max_kv_tokens().to_string());
    }
    if let Some(value) = read_query_param("max_new").filter(|value| !value.is_empty()) {
        gptoss.max_new_input.set_value(value);
    } else if gptoss.max_new_input.get_value().trim().is_empty() {
        gptoss
            .max_new_input
            .set_value(default_max_new_tokens().to_string());
    }
    if let Some(value) = read_query_param("sample").filter(|value| !value.is_empty()) {
        gptoss.sample_input.set_value(value);
    } else if gptoss.sample_input.get_value().trim().is_empty() {
        gptoss.sample_input.set_value("on");
    }
    if let Some(value) = read_query_param("temp").filter(|value| !value.is_empty()) {
        gptoss.temp_input.set_value(value);
    } else if gptoss.temp_input.get_value().trim().is_empty() {
        gptoss
            .temp_input
            .set_value(format!("{:.1}", default_sample_temp()));
    }
    if let Some(value) = read_query_param("top_k").filter(|value| !value.is_empty()) {
        gptoss.top_k_input.set_value(value);
    } else if gptoss.top_k_input.get_value().trim().is_empty() {
        gptoss
            .top_k_input
            .set_value(default_sample_top_k().to_string());
    }
    if let Some(value) = read_query_param("top_p").filter(|value| !value.is_empty()) {
        gptoss.top_p_input.set_value(value);
    } else if gptoss.top_p_input.get_value().trim().is_empty() {
        gptoss
            .top_p_input
            .set_value(format!("{:.1}", default_sample_top_p()));
    }

    gptoss.inputs_initialized = true;
}
