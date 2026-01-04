//! Chat panel - NIP-28 public channel messages

use wgpui::{Bounds, Hsla, Point, Quad, Scene, Size, TextSystem};

use crate::state::{FmVizState, InputFocus, NostrConnectionStatus};

use super::{accent_cyan, accent_green, panel_bg, text_dim};

/// Draw chat panel (right side)
pub fn draw_chat_panel(
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

    // Header with channel name and focus indicator
    let header_color = if state.input_focus == InputFocus::Chat {
        accent_cyan()
    } else {
        text_dim()
    };

    let channel_name = state.channel_id.as_deref().unwrap_or("#openagents-providers");
    let run = text.layout(channel_name, Point::new(x + 12.0, y + 8.0), 11.0, header_color);
    scene.draw_text(run);

    // Message count
    let count_text = format!("{}", state.chat_messages.len());
    let count_x = x + width - 30.0;
    let run = text.layout(&count_text, Point::new(count_x, y + 8.0), 11.0, text_dim());
    scene.draw_text(run);

    // Divider
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x + 12.0, y + 28.0),
            size: Size::new(width - 24.0, 1.0),
        })
        .with_background(text_dim().with_alpha(0.3)),
    );

    // Calculate areas
    let input_height = 40.0;
    let messages_y = y + 36.0;
    let messages_height = height - 44.0 - input_height;

    // Messages area
    let line_height = 20.0;
    let max_messages = (messages_height / line_height).floor() as usize;
    let font_size = 11.0;
    let char_width = text.measure("M", font_size);
    let max_content_width = width - 100.0; // Leave room for author

    // Display messages (newest at bottom, so reverse and take last N)
    let visible_messages: Vec<_> = state.chat_messages.iter().rev().take(max_messages).collect();
    let visible_messages: Vec<_> = visible_messages.into_iter().rev().collect();

    for (i, msg) in visible_messages.iter().enumerate() {
        let msg_y = messages_y + i as f32 * line_height;

        // Author with brackets
        let author_color = if msg.is_self {
            accent_green()
        } else {
            accent_cyan()
        };

        let author_short = if msg.author.len() > 8 {
            format!("[{}...]", &msg.author[..8])
        } else {
            format!("[{}]", msg.author)
        };

        let run = text.layout(&author_short, Point::new(x + 12.0, msg_y), font_size, author_color);
        scene.draw_text(run);

        // Content (truncate if too long)
        let author_width = text.measure(&author_short, font_size);
        let content_x = x + 16.0 + author_width;
        let max_chars = ((max_content_width - author_width) / char_width).floor() as usize;

        let content = if msg.content.len() > max_chars {
            format!("{}...", &msg.content[..max_chars.saturating_sub(3)])
        } else {
            msg.content.clone()
        };

        let run = text.layout(&content, Point::new(content_x, msg_y), font_size, Hsla::new(0.0, 0.0, 0.85, 1.0));
        scene.draw_text(run);
    }

    // Empty state
    if state.chat_messages.is_empty() {
        let empty_text = "No messages yet";
        let empty_x = x + (width - text.measure(empty_text, 11.0)) / 2.0;
        let empty_y = messages_y + messages_height / 2.0;
        let run = text.layout(empty_text, Point::new(empty_x, empty_y), 11.0, text_dim());
        scene.draw_text(run);
    }

    // Input area
    let input_y = y + height - input_height;

    // Divider above input
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x + 12.0, input_y),
            size: Size::new(width - 24.0, 1.0),
        })
        .with_background(text_dim().with_alpha(0.3)),
    );

    // Input background
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x + 12.0, input_y + 8.0),
            size: Size::new(width - 24.0, input_height - 16.0),
        })
        .with_background(Hsla::new(0.0, 0.0, 0.1, 1.0)),
    );

    // Prompt indicator
    let prompt_text = if state.nostr_status == NostrConnectionStatus::Authenticated {
        "> "
    } else {
        "# "
    };
    let prompt_color = if state.input_focus == InputFocus::Chat {
        accent_cyan()
    } else {
        text_dim()
    };
    let run = text.layout(prompt_text, Point::new(x + 18.0, input_y + 12.0), font_size, prompt_color);
    scene.draw_text(run);

    // Input text
    let prompt_width = text.measure(prompt_text, font_size);
    let input_text_x = x + 18.0 + prompt_width;

    let display_text = if state.chat_input.is_empty() && state.input_focus != InputFocus::Chat {
        "Type message, press Enter"
    } else {
        &state.chat_input
    };

    let text_color = if state.chat_input.is_empty() {
        text_dim()
    } else {
        Hsla::new(0.0, 0.0, 0.9, 1.0)
    };

    let run = text.layout(display_text, Point::new(input_text_x, input_y + 12.0), font_size, text_color);
    scene.draw_text(run);

    // Cursor (when focused)
    if state.input_focus == InputFocus::Chat {
        let cursor_pos = state.chat_cursor.min(state.chat_input.len());
        let text_before_cursor = &state.chat_input[..cursor_pos];
        let cursor_x = input_text_x + text.measure(text_before_cursor, font_size);
        scene.draw_quad(
            Quad::new(Bounds {
                origin: Point::new(cursor_x, input_y + 10.0),
                size: Size::new(2.0, 14.0),
            })
            .with_background(accent_cyan()),
        );
    }
}
