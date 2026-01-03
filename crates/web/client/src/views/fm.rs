//! FM Bridge (Apple Foundation Models) visualization page

use wgpui::{
    Bounds, Hsla, Point, Quad, Scene, TextSystem, theme,
};
use wgpui::animation::AnimatorState;
use wgpui::components::hud::{DotsGrid, DotsOrigin};
use wgpui::components::Component;
use wgpui::PaintContext;

use crate::state::{AppState, FmConnectionStatus, FmStreamStatus};

fn accent_cyan() -> Hsla {
    Hsla::from_hex(0x7fd3e5)
}

fn accent_green() -> Hsla {
    Hsla::from_hex(0x00ff88)
}

fn accent_orange() -> Hsla {
    Hsla::from_hex(0xff9900)
}

fn accent_red() -> Hsla {
    Hsla::from_hex(0xff4444)
}

fn panel_bg() -> Hsla {
    Hsla::from_hex(0x05070b)
}

fn panel_border() -> Hsla {
    Hsla::from_hex(0x2a3640)
}

pub(crate) fn build_fm_page(
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
    if !state.fm_viz.frame_started {
        state.fm_viz.frame_started = true;
    }
    let frame_progress = state
        .fm_viz
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
    let inner_x = content_x + inner_padding;
    let inner_width = content_width - inner_padding * 2.0;
    let mut y = card_y + inner_padding;

    // ========================================================================
    // Status Bar
    // ========================================================================
    let status_height = 28.0;
    draw_status_bar(scene, text_system, state, inner_x, y, inner_width, status_height);
    y += status_height + 16.0;

    // ========================================================================
    // Title
    // ========================================================================
    let title = "FM BRIDGE";
    let title_run = text_system.layout(
        title,
        Point::new(inner_x, y),
        18.0,
        accent_cyan(),
    );
    scene.draw_text(title_run);
    y += 28.0;

    let subtitle = "Apple Foundation Models Inference";
    let subtitle_run = text_system.layout(
        subtitle,
        Point::new(inner_x, y),
        12.0,
        theme::text::SECONDARY,
    );
    scene.draw_text(subtitle_run);
    y += 32.0;

    // ========================================================================
    // Token Stream Panel
    // ========================================================================
    let stream_height = 200.0;
    draw_token_stream_panel(scene, text_system, state, inner_x, y, inner_width, stream_height);
    y += stream_height + 16.0;

    // ========================================================================
    // Session Info Panel
    // ========================================================================
    if state.fm_viz.session_id.is_some() || !state.fm_viz.transcript.is_empty() {
        let session_height = 150.0;
        draw_session_panel(scene, text_system, state, inner_x, y, inner_width, session_height);
        y += session_height + 16.0;
    }

    // ========================================================================
    // Stats
    // ========================================================================
    draw_stats(scene, text_system, state, inner_x, y, inner_width);
}

fn draw_status_bar(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &AppState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Background
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, width, height))
            .with_background(panel_bg())
            .with_border(panel_border(), 1.0),
    );

    let padding = 10.0;
    let mut text_x = x + padding;

    // Connection status indicator
    let (status_color, status_text) = match state.fm_viz.connection_status {
        FmConnectionStatus::Connected => (accent_green(), "CONNECTED"),
        FmConnectionStatus::Connecting => (accent_orange(), "CONNECTING"),
        FmConnectionStatus::Disconnected => (theme::text::SECONDARY, "DISCONNECTED"),
        FmConnectionStatus::Error => (accent_red(), "ERROR"),
    };

    // Status dot
    let dot_size = 8.0;
    let dot_y = y + (height - dot_size) / 2.0;
    scene.draw_quad(
        Quad::new(Bounds::new(text_x, dot_y, dot_size, dot_size))
            .with_background(status_color)
            .with_corner_radius(dot_size / 2.0),
    );
    text_x += dot_size + 8.0;

    // Status text
    let status_run = text_system.layout(
        status_text,
        Point::new(text_x, y + 7.0),
        10.0,
        status_color,
    );
    scene.draw_text(status_run);
    text_x += text_system.measure(status_text, 10.0) + 20.0;

    // Separator
    scene.draw_quad(
        Quad::new(Bounds::new(text_x, y + 6.0, 1.0, height - 12.0))
            .with_background(panel_border()),
    );
    text_x += 20.0;

    // Bridge URL
    let url_run = text_system.layout(
        &state.fm_viz.bridge_url,
        Point::new(text_x, y + 7.0),
        10.0,
        theme::text::SECONDARY,
    );
    scene.draw_text(url_run);
    text_x += text_system.measure(&state.fm_viz.bridge_url, 10.0) + 20.0;

    // Model availability
    if state.fm_viz.model_available {
        let model_run = text_system.layout(
            "MODEL: AVAILABLE",
            Point::new(text_x, y + 7.0),
            10.0,
            accent_green(),
        );
        scene.draw_text(model_run);
    }

    // Latency (right side)
    if let Some(latency) = state.fm_viz.ping_latency_ms {
        let latency_text = format!("{}ms", latency);
        let latency_width = text_system.measure(&latency_text, 10.0);
        let latency_run = text_system.layout(
            &latency_text,
            Point::new(x + width - padding - latency_width, y + 7.0),
            10.0,
            theme::text::SECONDARY,
        );
        scene.draw_text(latency_run);
    }
}

fn draw_token_stream_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &AppState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Panel background
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, width, height))
            .with_background(panel_bg())
            .with_border(panel_border(), 1.0),
    );

    let padding = 12.0;
    let header_height = 24.0;

    // Header
    let header_text = "TOKEN STREAM";
    let header_run = text_system.layout(
        header_text,
        Point::new(x + padding, y + 6.0),
        11.0,
        accent_cyan(),
    );
    scene.draw_text(header_run);

    // Throughput (right side of header)
    let throughput_text = format!("{:.1} t/s", state.fm_viz.tokens_per_sec);
    let throughput_width = text_system.measure(&throughput_text, 11.0);
    let throughput_color = if state.fm_viz.tokens_per_sec > 10.0 {
        accent_green()
    } else if state.fm_viz.tokens_per_sec > 5.0 {
        accent_orange()
    } else {
        theme::text::SECONDARY
    };
    let throughput_run = text_system.layout(
        &throughput_text,
        Point::new(x + width - padding - throughput_width, y + 6.0),
        11.0,
        throughput_color,
    );
    scene.draw_text(throughput_run);

    // Divider
    scene.draw_quad(
        Quad::new(Bounds::new(x + padding, y + header_height, width - padding * 2.0, 1.0))
            .with_background(panel_border()),
    );

    // Token stream content
    let content_y = y + header_height + 8.0;
    let content_height = height - header_height - padding - 8.0;

    // Display token stream or placeholder
    let display_text = if state.fm_viz.token_stream.is_empty() {
        match state.fm_viz.stream_status {
            FmStreamStatus::Idle => "Waiting for input...",
            FmStreamStatus::Streaming => "",
            FmStreamStatus::Complete => "Generation complete.",
            FmStreamStatus::Error => "Error during generation.",
        }
    } else {
        &state.fm_viz.token_stream
    };

    let text_color = if state.fm_viz.token_stream.is_empty() {
        theme::text::SECONDARY
    } else {
        theme::text::PRIMARY
    };

    // Simple text wrapping
    let max_chars_per_line = ((width - padding * 2.0) / 8.0) as usize;
    let lines: Vec<&str> = display_text
        .chars()
        .collect::<Vec<_>>()
        .chunks(max_chars_per_line)
        .map(|chunk| {
            let s: String = chunk.iter().collect();
            Box::leak(s.into_boxed_str()) as &str
        })
        .collect();

    let line_height = 16.0;
    let max_lines = (content_height / line_height) as usize;
    let visible_lines = lines.iter().rev().take(max_lines).rev();

    for (i, line) in visible_lines.enumerate() {
        let line_y = content_y + (i as f32) * line_height;
        let line_run = text_system.layout(
            line,
            Point::new(x + padding, line_y),
            12.0,
            text_color,
        );
        scene.draw_text(line_run);
    }

    // Cursor when streaming
    if matches!(state.fm_viz.stream_status, FmStreamStatus::Streaming) {
        let cursor_x = x + padding + (state.fm_viz.token_stream.len() % max_chars_per_line) as f32 * 8.0;
        let cursor_y = content_y + ((lines.len().saturating_sub(1)) as f32) * line_height;
        scene.draw_quad(
            Quad::new(Bounds::new(cursor_x, cursor_y, 8.0, 14.0))
                .with_background(accent_cyan().with_alpha(0.8)),
        );
    }
}

fn draw_session_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &AppState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Panel background
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, width, height))
            .with_background(panel_bg())
            .with_border(panel_border(), 1.0),
    );

    let padding = 12.0;

    // Header
    let session_text = if let Some(ref sid) = state.fm_viz.session_id {
        format!("SESSION: {} | TURNS: {}", &sid[..8.min(sid.len())], state.fm_viz.turn_count)
    } else {
        "SESSION".to_string()
    };
    let header_run = text_system.layout(
        &session_text,
        Point::new(x + padding, y + 6.0),
        11.0,
        accent_cyan(),
    );
    scene.draw_text(header_run);

    // Transcript messages
    let content_y = y + 28.0;
    let line_height = 18.0;
    let max_messages = ((height - 40.0) / line_height) as usize;

    for (i, msg) in state.fm_viz.transcript.iter().rev().take(max_messages).rev().enumerate() {
        let msg_y = content_y + (i as f32) * line_height;
        let role_color = if msg.role == "user" {
            accent_orange()
        } else {
            accent_green()
        };

        let role_text = if msg.role == "user" { "USER:" } else { "ASST:" };
        let role_run = text_system.layout(
            role_text,
            Point::new(x + padding, msg_y),
            10.0,
            role_color,
        );
        scene.draw_text(role_run);

        let content_preview: String = msg.content.chars().take(80).collect();
        let content_run = text_system.layout(
            &content_preview,
            Point::new(x + padding + 45.0, msg_y),
            10.0,
            theme::text::SECONDARY,
        );
        scene.draw_text(content_run);
    }
}

fn draw_stats(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &AppState,
    x: f32,
    y: f32,
    _width: f32,
) {
    let stats = [
        ("TOKENS", state.fm_viz.token_count.to_string()),
        ("TTFT", state.fm_viz.ttft_ms.map(|t| format!("{}ms", t)).unwrap_or("-".to_string())),
        ("TOOLS", state.fm_viz.registered_tools.len().to_string()),
    ];

    let mut stat_x = x;
    for (label, value) in stats {
        let label_run = text_system.layout(
            label,
            Point::new(stat_x, y),
            10.0,
            theme::text::SECONDARY,
        );
        scene.draw_text(label_run);

        let value_run = text_system.layout(
            &value,
            Point::new(stat_x, y + 14.0),
            12.0,
            accent_cyan(),
        );
        scene.draw_text(value_run);

        stat_x += 100.0;
    }
}
