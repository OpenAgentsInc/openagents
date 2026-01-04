//! FM Bridge panel components

use wgpui::{Bounds, Hsla, Point, Quad, Scene, Size, TextSystem};

use crate::state::{FmConnectionStatus, FmStreamStatus, FmVizState, InputFocus};

use super::{accent_cyan, panel_bg, text_dim};

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

    // Label with focus indicator
    let is_focused = state.input_focus == InputFocus::Prompt;
    let label = if state.is_streaming() {
        "STREAMING..."
    } else if state.connection_status != FmConnectionStatus::Connected {
        "WAITING FOR CONNECTION..."
    } else if is_focused {
        "PROMPT (focused) - ENTER to send"
    } else {
        "PROMPT - TAB to focus"
    };
    let label_color = if is_focused { accent_cyan() } else { text_dim() };
    let run = text.layout(label, Point::new(x + 12.0, y + 8.0), 10.0, label_color);
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
    let text_x = x + 18.0;
    let text_y = input_y + 6.0;

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

    // Draw selection highlight if active
    if let Some((start, end)) = state.selection {
        let start = start.min(state.prompt_input.len());
        let end = end.min(state.prompt_input.len());
        if start < end && !state.prompt_input.is_empty() {
            let before_sel = &state.prompt_input[..start];
            let sel_text = &state.prompt_input[start..end];
            let sel_start_x = text_x + text.measure(before_sel, font_size);
            let sel_width = text.measure(sel_text, font_size);

            scene.draw_quad(
                Quad::new(Bounds {
                    origin: Point::new(sel_start_x, input_y + 4.0),
                    size: Size::new(sel_width, 16.0),
                })
                .with_background(accent_cyan().with_alpha(0.3)),
            );
        }
    }

    let run = text.layout(
        display_text,
        Point::new(text_x, text_y),
        font_size,
        text_color,
    );
    scene.draw_text(run);

    // Cursor at cursor_pos - only show when focused
    if is_focused && !state.is_streaming() && state.connection_status == FmConnectionStatus::Connected {
        let cursor_pos = state.cursor_pos.min(state.prompt_input.len());
        let text_before_cursor = &state.prompt_input[..cursor_pos];
        let cursor_x = text_x + text.measure(text_before_cursor, font_size);
        scene.draw_quad(
            Quad::new(Bounds {
                origin: Point::new(cursor_x, input_y + 4.0),
                size: Size::new(2.0, 16.0),
            })
            .with_background(accent_cyan()),
        );
    }
}
