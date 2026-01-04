//! FM Bridge panel components

use wgpui::{Bounds, Hsla, Point, Quad, Scene, Size, TextSystem};

use crate::state::{FmConnectionStatus, FmStreamStatus, FmVizState};

use super::{accent_cyan, accent_green, panel_bg, text_dim};

pub fn draw_status_bar(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &FmVizState,
    x: f32,
    y: f32,
    width: f32,
) {
    // Background
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y),
            size: Size::new(width, 28.0),
        })
        .with_background(panel_bg()),
    );

    // Connection status dot
    let dot_color = match state.connection_status {
        FmConnectionStatus::Connected => accent_green(),
        FmConnectionStatus::Connecting => Hsla::new(45.0 / 360.0, 1.0, 0.5, 1.0),
        FmConnectionStatus::Disconnected => text_dim(),
        FmConnectionStatus::Error => Hsla::new(0.0, 0.9, 0.5, 1.0),
    };

    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x + 8.0, y + 10.0),
            size: Size::new(8.0, 8.0),
        })
        .with_background(dot_color)
        .with_corner_radius(4.0),
    );

    // Connection text
    let status_text = match state.connection_status {
        FmConnectionStatus::Connected => "CONNECTED",
        FmConnectionStatus::Connecting => "CONNECTING",
        FmConnectionStatus::Disconnected => "DISCONNECTED",
        FmConnectionStatus::Error => "ERROR",
    };
    let run = text.layout(status_text, Point::new(x + 22.0, y + 7.0), 11.0, dot_color);
    scene.draw_text(run);

    // Bridge status or URL
    if let Some(ref bridge_msg) = state.bridge_status_message {
        // Show bridge status message
        let msg_color = if bridge_msg.contains("failed") || bridge_msg.contains("not found") {
            Hsla::new(0.0, 0.9, 0.5, 1.0) // red for errors
        } else if bridge_msg.contains("running") {
            accent_green() // green when running
        } else {
            Hsla::new(45.0 / 360.0, 1.0, 0.5, 1.0) // yellow for starting
        };
        let run = text.layout(bridge_msg, Point::new(x + 120.0, y + 7.0), 11.0, msg_color);
        scene.draw_text(run);
    } else {
        // Show URL
        let run = text.layout(&state.bridge_url, Point::new(x + 120.0, y + 7.0), 11.0, text_dim());
        scene.draw_text(run);
    }

    // Model status
    let model_text = if state.model_available {
        "MODEL: AVAILABLE"
    } else {
        "MODEL: UNAVAILABLE"
    };
    let model_color = if state.model_available {
        accent_green()
    } else {
        Hsla::new(0.0, 0.9, 0.5, 1.0)
    };
    let run = text.layout(model_text, Point::new(x + width - 150.0, y + 7.0), 11.0, model_color);
    scene.draw_text(run);
}

pub fn draw_token_stream(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &FmVizState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Panel background
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        })
        .with_background(panel_bg()),
    );

    // Header
    let run = text.layout("TOKEN STREAM", Point::new(x + 12.0, y + 8.0), 11.0, text_dim());
    scene.draw_text(run);

    // Tokens per second
    let tps_text = format!("{:.1} t/s", state.tokens_per_sec);
    let run = text.layout(&tps_text, Point::new(x + width - 70.0, y + 8.0), 11.0, accent_cyan());
    scene.draw_text(run);

    // Divider
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x + 12.0, y + 28.0),
            size: Size::new(width - 24.0, 1.0),
        })
        .with_background(text_dim().with_alpha(0.3)),
    );

    // Token stream text (with word wrap simulation - just truncate for now)
    let stream_y = y + 40.0;
    let font_size = 13.0;

    // Measure char width using actual font metrics
    let char_width = text.measure("M", font_size);
    let line_width = width - 24.0;
    let chars_per_line = (line_width / char_width).floor() as usize;
    let max_chars = chars_per_line * 6; // 6 lines worth

    let display_text = if state.token_stream.len() > max_chars {
        &state.token_stream[state.token_stream.len() - max_chars..]
    } else {
        &state.token_stream
    };

    // Split into lines
    let lines: Vec<&str> = display_text
        .as_bytes()
        .chunks(chars_per_line.max(1))
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
        .collect();

    for (i, line) in lines.iter().take(8).enumerate() {
        let run = text.layout(
            line,
            Point::new(x + 12.0, stream_y + i as f32 * 18.0),
            font_size,
            Hsla::new(0.0, 0.0, 0.9, 1.0),
        );
        scene.draw_text(run);
    }

    // Blinking cursor - use actual measured width
    let cursor_visible = (state.token_count / 2) % 2 == 0;
    if state.stream_status == FmStreamStatus::Streaming && cursor_visible {
        let last_line = lines.last().copied().unwrap_or("");
        let cursor_x = x + 12.0 + text.measure(last_line, font_size);
        let cursor_y = stream_y + (lines.len().saturating_sub(1)) as f32 * 18.0;
        scene.draw_quad(
            Quad::new(Bounds {
                origin: Point::new(cursor_x, cursor_y),
                size: Size::new(2.0, 14.0),
            })
            .with_background(accent_cyan()),
        );
    }
}

pub fn draw_session_panel(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &FmVizState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Panel background
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        })
        .with_background(panel_bg()),
    );

    // Session info
    let session_text = format!(
        "SESSION: {} | TURNS: {}",
        state.session_id.as_deref().unwrap_or("none"),
        state.turn_count
    );
    let run = text.layout(&session_text, Point::new(x + 12.0, y + 8.0), 11.0, text_dim());
    scene.draw_text(run);

    // Transcript
    let transcript_y = y + 32.0;
    let font_size = 11.0;
    let char_width = text.measure("M", font_size);

    for (i, msg) in state.transcript.iter().take(4).enumerate() {
        let role_color = if msg.role == "USER" {
            accent_cyan()
        } else {
            accent_green()
        };

        let role_text = format!("{}: ", msg.role);
        let run = text.layout(
            &role_text,
            Point::new(x + 12.0, transcript_y + i as f32 * 20.0),
            font_size,
            role_color,
        );
        scene.draw_text(run);

        // Truncate content based on actual font metrics
        let role_width = text.measure(&role_text, font_size);
        let max_content = ((width - 24.0 - role_width) / char_width).floor() as usize;
        let content = if msg.content.len() > max_content {
            format!("{}...", &msg.content[..max_content.saturating_sub(3)])
        } else {
            msg.content.clone()
        };

        let run = text.layout(
            &content,
            Point::new(x + 12.0 + role_width, transcript_y + i as f32 * 20.0),
            font_size,
            Hsla::new(0.0, 0.0, 0.8, 1.0),
        );
        scene.draw_text(run);
    }
}

pub fn draw_prompt_input(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &FmVizState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Panel background
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        })
        .with_background(panel_bg()),
    );

    // Label
    let label = if state.is_streaming() {
        "STREAMING..."
    } else if state.connection_status != FmConnectionStatus::Connected {
        "WAITING FOR CONNECTION..."
    } else {
        "TYPE PROMPT, PRESS ENTER"
    };
    let run = text.layout(label, Point::new(x + 12.0, y + 8.0), 10.0, text_dim());
    scene.draw_text(run);

    // Input field background
    let input_y = y + 24.0;
    let input_height = height - 32.0;
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x + 12.0, input_y),
            size: Size::new(width - 24.0, input_height),
        })
        .with_background(Hsla::new(0.0, 0.0, 0.1, 1.0)),
    );

    // Input text
    let font_size = 13.0;
    let display_text = if state.prompt_input.is_empty() && !state.is_streaming() {
        "Enter your prompt here..."
    } else {
        &state.prompt_input
    };

    let text_color = if state.prompt_input.is_empty() {
        text_dim()
    } else {
        Hsla::new(0.0, 0.0, 0.9, 1.0)
    };

    let run = text.layout(
        display_text,
        Point::new(x + 18.0, input_y + 6.0),
        font_size,
        text_color,
    );
    scene.draw_text(run);

    // Cursor (blinking) - use actual measured width
    if !state.is_streaming() && state.connection_status == FmConnectionStatus::Connected {
        let cursor_x = x + 18.0 + text.measure(&state.prompt_input, font_size);
        scene.draw_quad(
            Quad::new(Bounds {
                origin: Point::new(cursor_x, input_y + 4.0),
                size: Size::new(2.0, 16.0),
            })
            .with_background(accent_cyan()),
        );
    }
}
