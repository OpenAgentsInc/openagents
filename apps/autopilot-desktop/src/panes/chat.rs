use wgpui::markdown::{MarkdownConfig, MarkdownParser, MarkdownRenderer};
use wgpui::{Bounds, Component, InputEvent, PaintContext, Point, Quad, SvgQuad, theme};

use crate::app_state::{
    AutopilotChatState, AutopilotMessage, AutopilotMessageStatus, AutopilotProgressBlock,
    AutopilotProgressRow, AutopilotRole, ChatPaneInputs, ChatTranscriptSelectionState, PaneKind,
    RenderState,
};
use crate::pane_system::{
    chat_composer_height_for_value, chat_composer_input_bounds_with_height,
    chat_send_button_bounds, chat_transcript_bounds, chat_transcript_bounds_with_height,
    pane_content_bounds,
};

const CHAT_TRANSCRIPT_LINE_HEIGHT: f32 = 14.0;
const CHAT_MARKDOWN_FONT_SIZE: f32 = 11.0;
const CHAT_MARKDOWN_MIN_WIDTH: f32 = 84.0;
const CHAT_PROGRESS_HEADER_LINE_HEIGHT: f32 = 12.0;
const CHAT_PROGRESS_ROW_LINE_HEIGHT: f32 = 12.0;
const CHAT_PROGRESS_BLOCK_GAP: f32 = 4.0;
const CHAT_ACTIVITY_HEADER_LINE_HEIGHT: f32 = 12.0;
const CHAT_ACTIVITY_ROW_LINE_HEIGHT: f32 = 12.0;
const CHAT_ACTIVITY_MAX_ROWS: usize = 14;
const CHAT_SEND_ICON_SVG_RAW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640"><path fill="#FFFFFF" d="M342.6 73.4C330.1 60.9 309.8 60.9 297.3 73.4L137.3 233.4C124.8 245.9 124.8 266.2 137.3 278.7C149.8 291.2 170.1 291.2 182.6 278.7L288 173.3L288 544C288 561.7 302.3 576 320 576C337.7 576 352 561.7 352 544L352 173.3L457.4 278.7C469.9 291.2 490.2 291.2 502.7 278.7C515.2 266.2 515.2 245.9 502.7 233.4L342.7 73.4z"/></svg>"##;

#[derive(Clone, Copy, Debug)]
struct WrappedTranscriptLine {
    start_byte_offset: usize,
    end_byte_offset: usize,
    char_count: usize,
}

fn transcript_scroll_clip_bounds(content_bounds: Bounds) -> Bounds {
    let transcript_bounds = chat_transcript_bounds(content_bounds);
    Bounds::new(
        transcript_bounds.origin.x + 8.0,
        transcript_bounds.origin.y + 8.0,
        (transcript_bounds.size.width - 16.0).max(0.0),
        (transcript_bounds.size.height - 24.0).max(0.0),
    )
}

fn paint_chat_send_button(bounds: Bounds, enabled: bool, paint: &mut PaintContext) {
    let diameter = bounds.size.height.min(bounds.size.width);
    let circle_bounds = Bounds::new(
        bounds.max_x() - diameter,
        bounds.origin.y + (bounds.size.height - diameter) * 0.5,
        diameter,
        diameter,
    );
    let (background, border, icon_tint) = if enabled {
        (
            theme::accent::PRIMARY,
            theme::accent::PRIMARY,
            theme::bg::APP,
        )
    } else {
        (
            theme::bg::ELEVATED,
            theme::border::DEFAULT,
            theme::text::DISABLED,
        )
    };
    paint.scene.draw_quad(
        Quad::new(circle_bounds)
            .with_background(background)
            .with_border(border, 1.0)
            .with_corner_radius(diameter * 0.5),
    );

    let icon_size = 16.0;
    let icon_bounds = Bounds::new(
        circle_bounds.origin.x + (circle_bounds.size.width - icon_size) * 0.5,
        circle_bounds.origin.y + (circle_bounds.size.height - icon_size) * 0.5,
        icon_size,
        icon_size,
    );
    paint.scene.draw_svg(
        SvgQuad::new(
            icon_bounds,
            std::sync::Arc::<[u8]>::from(CHAT_SEND_ICON_SVG_RAW.as_bytes()),
        )
        .with_tint(icon_tint),
    );
}

fn transcript_scroll_clip_bounds_with_height(
    content_bounds: Bounds,
    composer_height: f32,
) -> Bounds {
    let transcript_bounds = chat_transcript_bounds_with_height(content_bounds, composer_height);
    Bounds::new(
        transcript_bounds.origin.x + 8.0,
        transcript_bounds.origin.y + 8.0,
        (transcript_bounds.size.width - 16.0).max(0.0),
        (transcript_bounds.size.height - 24.0).max(0.0),
    )
}

fn chat_markdown_config() -> MarkdownConfig {
    MarkdownConfig {
        base_font_size: CHAT_MARKDOWN_FONT_SIZE,
        header_sizes: [1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        text_color: theme::text::PRIMARY,
        ..MarkdownConfig::default()
    }
}

fn markdown_body_width(transcript_scroll_clip: Bounds) -> f32 {
    (transcript_scroll_clip.size.width - 6.0).max(CHAT_MARKDOWN_MIN_WIDTH)
}

fn message_markdown_source(message: &AutopilotMessage) -> String {
    sanitize_chat_text(&message_display_content(message))
}

fn message_progress_blocks(message: &AutopilotMessage) -> &[AutopilotProgressBlock] {
    message
        .structured
        .as_ref()
        .map(|structured| structured.progress_blocks.as_slice())
        .unwrap_or(&[])
}

fn progress_block_header(block: &AutopilotProgressBlock) -> String {
    format!("{} [{}]", block.title.trim(), block.status.trim())
}

fn progress_row_text(row: &AutopilotProgressRow) -> String {
    format!("{}: {}", row.label.trim(), row.value.trim())
}

fn progress_status_color(status: &str) -> wgpui::Hsla {
    match status.trim().to_ascii_lowercase().as_str() {
        "done" => theme::status::SUCCESS,
        "failed" => theme::status::ERROR,
        "rebuilding" | "applying" => theme::accent::PRIMARY,
        _ => theme::text::MUTED,
    }
}

fn progress_row_color(tone: &str) -> wgpui::Hsla {
    match tone.trim().to_ascii_lowercase().as_str() {
        "success" => theme::status::SUCCESS,
        "error" => theme::status::ERROR,
        "accent" => theme::accent::PRIMARY,
        "info" => theme::text::PRIMARY,
        _ => theme::text::MUTED,
    }
}

fn progress_block_height(block: &AutopilotProgressBlock) -> f32 {
    CHAT_PROGRESS_HEADER_LINE_HEIGHT
        + (block.rows.len() as f32 * CHAT_PROGRESS_ROW_LINE_HEIGHT)
        + CHAT_PROGRESS_BLOCK_GAP
}

fn message_progress_height(message: &AutopilotMessage) -> f32 {
    message_progress_blocks(message)
        .iter()
        .map(progress_block_height)
        .sum()
}

fn paint_message_progress_blocks(
    message: &AutopilotMessage,
    x: f32,
    mut y: f32,
    paint: &mut PaintContext,
) -> f32 {
    let start_y = y;
    for block in message_progress_blocks(message) {
        let header = sanitize_chat_text(&progress_block_header(block));
        paint.scene.draw_text(paint.text.layout_mono(
            &header,
            Point::new(x, y),
            10.0,
            progress_status_color(&block.status),
        ));
        y += CHAT_PROGRESS_HEADER_LINE_HEIGHT;

        for row in &block.rows {
            let text = sanitize_chat_text(&progress_row_text(row));
            paint.scene.draw_text(paint.text.layout(
                &text,
                Point::new(x + 6.0, y),
                10.0,
                progress_row_color(&row.tone),
            ));
            y += CHAT_PROGRESS_ROW_LINE_HEIGHT;
        }

        y += CHAT_PROGRESS_BLOCK_GAP;
    }
    y - start_y
}

fn is_tool_activity_event(event: &str) -> bool {
    let normalized = event.trim().to_ascii_lowercase();
    normalized.contains("tool call")
        || normalized.contains("tool user-input")
        || normalized.contains("command approval")
        || normalized.contains("file-change")
        || normalized.contains("auth token refresh")
        || normalized.contains("type=commandexecution")
}

fn chat_tool_activity_lines(autopilot_chat: &AutopilotChatState) -> Vec<String> {
    let mut lines = Vec::new();

    if !autopilot_chat.pending_tool_calls.is_empty() {
        lines.push(format!(
            "pending tool calls: {}",
            autopilot_chat.pending_tool_calls.len()
        ));
        for call in autopilot_chat.pending_tool_calls.iter().rev().take(3) {
            lines.push(format!(
                "tool call queued: {} ({})",
                call.tool, call.call_id
            ));
        }
    }
    if !autopilot_chat.pending_command_approvals.is_empty() {
        lines.push(format!(
            "pending command approvals: {}",
            autopilot_chat.pending_command_approvals.len()
        ));
    }
    if !autopilot_chat.pending_file_change_approvals.is_empty() {
        lines.push(format!(
            "pending file-change approvals: {}",
            autopilot_chat.pending_file_change_approvals.len()
        ));
    }
    if !autopilot_chat.pending_tool_user_input.is_empty() {
        lines.push(format!(
            "pending tool prompts: {}",
            autopilot_chat.pending_tool_user_input.len()
        ));
    }

    let mut timeline = autopilot_chat
        .turn_timeline
        .iter()
        .filter(|event| is_tool_activity_event(event))
        .rev()
        .take(CHAT_ACTIVITY_MAX_ROWS)
        .cloned()
        .collect::<Vec<_>>();
    timeline.reverse();
    lines.extend(timeline);

    if lines.len() > CHAT_ACTIVITY_MAX_ROWS {
        let overflow = lines.len().saturating_sub(CHAT_ACTIVITY_MAX_ROWS);
        lines.drain(0..overflow);
    }

    lines
}

fn transcript_content_height(
    content_bounds: Bounds,
    composer_height: f32,
    autopilot_chat: &AutopilotChatState,
    text_system: &mut wgpui::TextSystem,
) -> f32 {
    let mut height = 8.0;

    let transcript_scroll_clip =
        transcript_scroll_clip_bounds_with_height(content_bounds, composer_height);
    let markdown_width = markdown_body_width(transcript_scroll_clip);
    let markdown_parser = MarkdownParser::new();
    let markdown_renderer = MarkdownRenderer::with_config(chat_markdown_config());

    for message in &autopilot_chat.messages {
        height += CHAT_TRANSCRIPT_LINE_HEIGHT;
        let markdown_source = message_markdown_source(message);
        let markdown_document = markdown_parser.parse(&markdown_source);
        let markdown_size =
            markdown_renderer.measure(&markdown_document, markdown_width, text_system);
        height += markdown_size.height.max(CHAT_TRANSCRIPT_LINE_HEIGHT);
        height += message_progress_height(message);
        height += 8.0;
    }

    let activity_lines = chat_tool_activity_lines(autopilot_chat);
    if !activity_lines.is_empty() {
        height += CHAT_ACTIVITY_HEADER_LINE_HEIGHT;
        height += CHAT_ACTIVITY_ROW_LINE_HEIGHT * activity_lines.len() as f32;
        height += 8.0;
    }

    height + 8.0
}

fn message_display_content(message: &AutopilotMessage) -> String {
    if message.content.trim().is_empty() && matches!(message.status, AutopilotMessageStatus::Queued)
    {
        "Waiting for response...".to_string()
    } else {
        message.content.clone()
    }
}

fn transcript_mono_char_width(text_system: &mut wgpui::TextSystem) -> f32 {
    text_system
        .measure_styled_mono(
            "M",
            CHAT_MARKDOWN_FONT_SIZE,
            wgpui::text::FontStyle::normal(),
        )
        .max(1.0)
}

fn wrap_transcript_text_lines(text: &str, max_chars_per_line: usize) -> Vec<WrappedTranscriptLine> {
    if text.is_empty() {
        return vec![WrappedTranscriptLine {
            start_byte_offset: 0,
            end_byte_offset: 0,
            char_count: 0,
        }];
    }

    let max_chars_per_line = max_chars_per_line.max(1);
    let mut lines = Vec::new();
    let mut line_start = 0usize;
    let mut line_chars = 0usize;

    for (byte_offset, ch) in text.char_indices() {
        if ch == '\n' {
            lines.push(WrappedTranscriptLine {
                start_byte_offset: line_start,
                end_byte_offset: byte_offset,
                char_count: line_chars,
            });
            line_start = byte_offset + ch.len_utf8();
            line_chars = 0;
            continue;
        }

        if line_chars >= max_chars_per_line {
            lines.push(WrappedTranscriptLine {
                start_byte_offset: line_start,
                end_byte_offset: byte_offset,
                char_count: line_chars,
            });
            line_start = byte_offset;
            line_chars = 0;
        }

        line_chars = line_chars.saturating_add(1);
    }

    lines.push(WrappedTranscriptLine {
        start_byte_offset: line_start,
        end_byte_offset: text.len(),
        char_count: line_chars,
    });
    if text.ends_with('\n') {
        lines.push(WrappedTranscriptLine {
            start_byte_offset: text.len(),
            end_byte_offset: text.len(),
            char_count: 0,
        });
    }
    lines
}

fn byte_offset_for_char_index(text: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    text.char_indices()
        .nth(char_index)
        .map_or(text.len(), |(index, _)| index)
}

fn clamp_to_char_boundary(text: &str, mut byte_offset: usize) -> usize {
    byte_offset = byte_offset.min(text.len());
    while byte_offset > 0 && !text.is_char_boundary(byte_offset) {
        byte_offset -= 1;
    }
    byte_offset
}

fn transcript_message_layouts(
    state: &mut RenderState,
    content_bounds: Bounds,
    composer_height: f32,
) -> Vec<(u64, Bounds)> {
    let autopilot_chat = &state.autopilot_chat;
    let transcript_scroll_clip =
        transcript_scroll_clip_bounds_with_height(content_bounds, composer_height);
    let transcript_content_height = transcript_content_height(
        content_bounds,
        composer_height,
        autopilot_chat,
        &mut state.text_system,
    );
    let transcript_max_scroll =
        (transcript_content_height - transcript_scroll_clip.size.height).max(0.0);
    let transcript_scroll_offset =
        autopilot_chat.transcript_effective_scroll_offset(transcript_max_scroll);
    let markdown_width = markdown_body_width(transcript_scroll_clip);
    let markdown_parser = MarkdownParser::new();
    let markdown_renderer = MarkdownRenderer::with_config(chat_markdown_config());

    let mut y = transcript_scroll_clip.origin.y + 8.0 - transcript_scroll_offset;
    let mut layouts = Vec::with_capacity(autopilot_chat.messages.len());

    for message in &autopilot_chat.messages {
        let start_y = y;
        y += CHAT_TRANSCRIPT_LINE_HEIGHT;

        let markdown_source = message_markdown_source(message);
        let markdown_document = markdown_parser.parse(&markdown_source);
        let markdown_height = markdown_renderer
            .measure(&markdown_document, markdown_width, &mut state.text_system)
            .height
            .max(CHAT_TRANSCRIPT_LINE_HEIGHT);
        y += markdown_height;
        y += message_progress_height(message);
        y += 8.0;

        layouts.push((
            message.id,
            Bounds::new(
                transcript_scroll_clip.origin.x,
                start_y,
                transcript_scroll_clip.size.width,
                (y - start_y).max(0.0),
            ),
        ));
    }

    layouts
}

fn top_chat_content_bounds(state: &RenderState) -> Option<Bounds> {
    state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::AutopilotChat)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane_content_bounds(pane.bounds))
}

pub fn transcript_message_byte_offset_at_point(
    state: &mut RenderState,
    point: Point,
) -> Option<(u64, usize)> {
    let content_bounds = top_chat_content_bounds(state)?;
    let composer_value = state.chat_inputs.composer.get_value().to_string();
    let composer_height = chat_composer_height_for_value(content_bounds, &composer_value);
    let clip = transcript_scroll_clip_bounds_with_height(content_bounds, composer_height);
    if !clip.contains(point) {
        return None;
    }
    let (message_id, message_bounds) =
        transcript_message_layouts(state, content_bounds, composer_height)
            .into_iter()
            .find(|(_, bounds)| bounds.contains(point))?;
    let message_text = transcript_message_copy_text_by_id(state, message_id)?;
    if message_text.is_empty() {
        return Some((message_id, 0));
    }

    let markdown_width = markdown_body_width(clip);
    let char_width = transcript_mono_char_width(&mut state.text_system);
    let max_chars_per_line = (markdown_width / char_width).floor().max(1.0) as usize;
    let wrapped_lines = wrap_transcript_text_lines(&message_text, max_chars_per_line);
    if wrapped_lines.is_empty() {
        return Some((message_id, 0));
    }

    let message_text_origin_y = message_bounds.origin.y + CHAT_TRANSCRIPT_LINE_HEIGHT;
    let relative_y = (point.y - message_text_origin_y).max(0.0);
    let line_index = (relative_y / CHAT_TRANSCRIPT_LINE_HEIGHT).floor() as usize;
    let line_index = line_index.min(wrapped_lines.len().saturating_sub(1));
    let line = wrapped_lines[line_index];

    let relative_x = (point.x - clip.origin.x).max(0.0);
    let char_index = (relative_x / char_width).floor() as usize;
    let char_index = char_index.min(line.char_count);

    let line_text = &message_text[line.start_byte_offset..line.end_byte_offset];
    let local_byte_offset = byte_offset_for_char_index(line_text, char_index);
    let byte_offset = (line.start_byte_offset + local_byte_offset).min(message_text.len());
    Some((message_id, byte_offset))
}

pub fn transcript_message_copy_text_by_id(state: &RenderState, message_id: u64) -> Option<String> {
    state
        .autopilot_chat
        .messages
        .iter()
        .find(|message| message.id == message_id)
        .map(|message| sanitize_chat_text(&message_display_content(message)))
}

pub fn transcript_selection_text(
    state: &RenderState,
    selection: ChatTranscriptSelectionState,
) -> Option<String> {
    let message_text = transcript_message_copy_text_by_id(state, selection.message_id)?;
    let start = clamp_to_char_boundary(&message_text, selection.start_byte_offset);
    let end = clamp_to_char_boundary(&message_text, selection.end_byte_offset);
    if end <= start {
        return None;
    }
    Some(message_text[start..end].to_string())
}

fn paint_message_selection_highlight(
    message_text: &str,
    selection: ChatTranscriptSelectionState,
    text_origin: Point,
    markdown_width: f32,
    paint: &mut PaintContext,
) {
    let start = clamp_to_char_boundary(message_text, selection.start_byte_offset);
    let end = clamp_to_char_boundary(message_text, selection.end_byte_offset);
    if end <= start {
        return;
    }

    let char_width = transcript_mono_char_width(paint.text);
    let max_chars_per_line = (markdown_width / char_width).floor().max(1.0) as usize;
    let wrapped_lines = wrap_transcript_text_lines(message_text, max_chars_per_line);
    let highlight_color = theme::accent::PRIMARY.with_alpha(0.24);

    for (line_index, line) in wrapped_lines.into_iter().enumerate() {
        if line.end_byte_offset <= start || line.start_byte_offset >= end {
            continue;
        }

        let selection_start = start.max(line.start_byte_offset);
        let selection_end = end.min(line.end_byte_offset);
        if selection_end <= selection_start {
            continue;
        }

        let prefix = &message_text[line.start_byte_offset..selection_start];
        let selected = &message_text[selection_start..selection_end];
        let start_chars = prefix.chars().count() as f32;
        let selected_chars = selected.chars().count() as f32;
        if selected_chars <= 0.0 {
            continue;
        }

        let highlight_x = text_origin.x + start_chars * char_width;
        let highlight_y = text_origin.y + line_index as f32 * CHAT_TRANSCRIPT_LINE_HEIGHT + 1.0;
        let highlight_width = selected_chars * char_width;
        let highlight_height = (CHAT_TRANSCRIPT_LINE_HEIGHT - 2.0).max(1.0);
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                highlight_x,
                highlight_y,
                highlight_width,
                highlight_height,
            ))
            .with_background(highlight_color),
        );
    }
}

pub fn paint(
    content_bounds: Bounds,
    autopilot_chat: &AutopilotChatState,
    chat_inputs: &mut ChatPaneInputs,
    paint: &mut PaintContext,
) {
    let composer_value = chat_inputs.composer.get_value().to_string();
    let composer_height = chat_composer_height_for_value(content_bounds, &composer_value);
    let transcript_bounds = chat_transcript_bounds_with_height(content_bounds, composer_height);
    let composer_bounds = chat_composer_input_bounds_with_height(content_bounds, composer_height);
    let send_bounds = chat_send_button_bounds(content_bounds);

    paint.scene.draw_quad(
        Quad::new(transcript_bounds)
            .with_background(theme::bg::APP.with_alpha(0.82))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(6.0),
    );

    let transcript_scroll_clip =
        transcript_scroll_clip_bounds_with_height(content_bounds, composer_height);
    let transcript_content_height =
        transcript_content_height(content_bounds, composer_height, autopilot_chat, paint.text);
    let transcript_max_scroll =
        (transcript_content_height - transcript_scroll_clip.size.height).max(0.0);
    let transcript_scroll_offset =
        autopilot_chat.transcript_effective_scroll_offset(transcript_max_scroll);
    let markdown_parser = MarkdownParser::new();
    let markdown_renderer = MarkdownRenderer::with_config(chat_markdown_config());
    let markdown_width = markdown_body_width(transcript_scroll_clip);

    paint.scene.push_clip(transcript_scroll_clip);
    let mut y = transcript_scroll_clip.origin.y + 8.0 - transcript_scroll_offset;

    if autopilot_chat.messages.is_empty() {
        let empty_state = "Ask me to do anything...";
        let empty_state_font_size = 18.0;
        let empty_state_width = paint.text.measure(empty_state, empty_state_font_size);
        let empty_state_x = transcript_scroll_clip.origin.x
            + (transcript_scroll_clip.size.width - empty_state_width) * 0.5;
        let empty_state_y = transcript_scroll_clip.origin.y
            + transcript_scroll_clip.size.height * 0.5
            - empty_state_font_size * 0.5;
        paint.scene.draw_text(paint.text.layout(
            empty_state,
            Point::new(
                empty_state_x.max(transcript_scroll_clip.origin.x),
                empty_state_y,
            ),
            empty_state_font_size,
            theme::text::MUTED,
        ));
    }

    for message in &autopilot_chat.messages {
        let message_text_origin_y = y + CHAT_TRANSCRIPT_LINE_HEIGHT;
        let status = match message.status {
            AutopilotMessageStatus::Queued => "thinking",
            AutopilotMessageStatus::Running => "running",
            AutopilotMessageStatus::Done => "done",
            AutopilotMessageStatus::Error => "error",
        };
        let role = match message.role {
            AutopilotRole::User => "you",
            AutopilotRole::Codex => "autopilot",
        };
        let status_color = match message.status {
            AutopilotMessageStatus::Queued => theme::text::MUTED,
            AutopilotMessageStatus::Running => theme::accent::PRIMARY,
            AutopilotMessageStatus::Done => theme::status::SUCCESS,
            AutopilotMessageStatus::Error => theme::status::ERROR,
        };
        let role_label = if status == "done" {
            format!("[{role}]")
        } else {
            format!("[{role}] [{status}]")
        };

        paint.scene.draw_text(paint.text.layout_mono(
            &role_label,
            Point::new(transcript_scroll_clip.origin.x, y),
            10.0,
            status_color,
        ));
        y += CHAT_TRANSCRIPT_LINE_HEIGHT;

        let markdown_source = message_markdown_source(message);
        if let Some(selection) = autopilot_chat.transcript_selection
            && selection.message_id == message.id
        {
            paint_message_selection_highlight(
                &markdown_source,
                selection,
                Point::new(transcript_scroll_clip.origin.x, message_text_origin_y),
                markdown_width,
                paint,
            );
        }
        let markdown_document = markdown_parser.parse(&markdown_source);
        let markdown_height = markdown_renderer
            .render(
                &markdown_document,
                Point::new(transcript_scroll_clip.origin.x, y),
                markdown_width,
                paint.text,
                paint.scene,
            )
            .height
            .max(CHAT_TRANSCRIPT_LINE_HEIGHT);
        y += markdown_height;
        y += paint_message_progress_blocks(message, transcript_scroll_clip.origin.x, y, paint);
        y += 8.0;
    }

    let activity_lines = chat_tool_activity_lines(autopilot_chat);
    if !activity_lines.is_empty() {
        paint.scene.draw_text(paint.text.layout_mono(
            "[activity]",
            Point::new(transcript_scroll_clip.origin.x, y),
            10.0,
            theme::accent::PRIMARY,
        ));
        y += CHAT_ACTIVITY_HEADER_LINE_HEIGHT;

        for line in activity_lines {
            let line = sanitize_chat_text(&line);
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(transcript_scroll_clip.origin.x + 6.0, y),
                10.0,
                theme::text::MUTED,
            ));
            y += CHAT_ACTIVITY_ROW_LINE_HEIGHT;
        }
    }
    paint.scene.pop_clip();

    let mut footer_y = transcript_bounds.max_y() - 14.0;
    if let Some(error) = autopilot_chat.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(transcript_bounds.origin.x + 10.0, footer_y),
            11.0,
            theme::status::ERROR,
        ));
        footer_y -= CHAT_TRANSCRIPT_LINE_HEIGHT;
    }
    if let Some(copy_notice) = autopilot_chat.copy_notice.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            copy_notice,
            Point::new(transcript_bounds.origin.x + 10.0, footer_y),
            11.0,
            theme::status::SUCCESS,
        ));
    }

    chat_inputs
        .composer
        .set_max_width(composer_bounds.size.width);
    chat_inputs.composer.paint(composer_bounds, paint);
    let can_send = !chat_inputs.composer.get_value().trim().is_empty();
    paint_chat_send_button(send_bounds, can_send, paint);
}

pub fn dispatch_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_chat = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::AutopilotChat)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_chat else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let composer_value = state.chat_inputs.composer.get_value().to_string();
    let composer_height = chat_composer_height_for_value(content_bounds, &composer_value);
    let composer_bounds = chat_composer_input_bounds_with_height(content_bounds, composer_height);
    state
        .chat_inputs
        .composer
        .event(event, composer_bounds, &mut state.event_context)
        .is_handled()
}

pub fn dispatch_transcript_scroll_event(
    state: &mut RenderState,
    cursor_position: Point,
    scroll_dy: f32,
) -> bool {
    if scroll_dy.abs() <= f32::EPSILON {
        return false;
    }

    let top_chat = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::AutopilotChat)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_chat else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let composer_value = state.chat_inputs.composer.get_value().to_string();
    let composer_height = chat_composer_height_for_value(content_bounds, &composer_value);
    let clip = transcript_scroll_clip_bounds_with_height(content_bounds, composer_height);
    if !clip.contains(cursor_position) {
        return false;
    }

    let content_height = transcript_content_height(
        content_bounds,
        composer_height,
        &state.autopilot_chat,
        &mut state.text_system,
    );
    let max_scroll = (content_height - clip.size.height).max(0.0);
    if max_scroll <= 0.0 {
        return false;
    }

    state
        .autopilot_chat
        .scroll_transcript_by(scroll_dy, max_scroll);
    true
}

fn sanitize_chat_text(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            match chars.next() {
                Some('[') => {
                    // CSI: consume until final byte.
                    for next in chars.by_ref() {
                        if ('@'..='~').contains(&next) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC: consume until BEL or ST (ESC \\\).
                    while let Some(next) = chars.next() {
                        if next == '\u{7}' {
                            break;
                        }
                        if next == '\u{1b}' && matches!(chars.peek(), Some('\\')) {
                            chars.next();
                            break;
                        }
                    }
                }
                Some(_) | None => {}
            }
            continue;
        }

        if ch == '\r' {
            continue;
        }

        if ch == '\n' || ch == '\t' || !ch.is_control() {
            output.push(ch);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        byte_offset_for_char_index, chat_tool_activity_lines, clamp_to_char_boundary,
        is_tool_activity_event, message_progress_height, progress_status_color, sanitize_chat_text,
        wrap_transcript_text_lines,
    };
    use crate::app_state::{
        AutopilotChatState, AutopilotMessage, AutopilotMessageStatus, AutopilotProgressBlock,
        AutopilotProgressRow, AutopilotRole, AutopilotStructuredMessage, AutopilotToolCallRequest,
    };
    use codex_client::AppServerRequestId;
    use wgpui::theme;

    fn fixture_progress_message(status: &str) -> AutopilotMessage {
        AutopilotMessage {
            id: 1,
            role: AutopilotRole::Codex,
            status: AutopilotMessageStatus::Running,
            content: "building".to_string(),
            structured: Some(AutopilotStructuredMessage {
                reasoning: String::new(),
                answer: "building".to_string(),
                events: Vec::new(),
                status: Some("answer".to_string()),
                progress_blocks: vec![AutopilotProgressBlock {
                    kind: "cad-build".to_string(),
                    title: "CAD Build".to_string(),
                    status: status.to_string(),
                    rows: vec![AutopilotProgressRow {
                        label: "phase".to_string(),
                        value: status.to_string(),
                        tone: "info".to_string(),
                    }],
                }],
            }),
        }
    }

    #[test]
    fn sanitize_chat_text_strips_ansi_and_control_chars() {
        let raw = "ok\u{1b}[31m red\u{1b}[0m\tline\r\nnext\u{7}";
        let sanitized = sanitize_chat_text(raw);
        assert_eq!(sanitized, "ok red\tline\nnext");
    }

    #[test]
    fn wrap_transcript_text_lines_preserves_offsets_for_newlines_and_wraps() {
        let text = "abcd\nefghi";
        let lines = wrap_transcript_text_lines(text, 3);
        let ranges = lines
            .into_iter()
            .map(|line| (line.start_byte_offset, line.end_byte_offset))
            .collect::<Vec<_>>();
        assert_eq!(ranges, vec![(0, 3), (3, 4), (5, 8), (8, 10)]);
        assert_eq!(&text[0..3], "abc");
        assert_eq!(&text[3..4], "d");
        assert_eq!(&text[5..8], "efg");
        assert_eq!(&text[8..10], "hi");
    }

    #[test]
    fn byte_offset_helpers_respect_utf8_boundaries() {
        let text = "AéB";
        assert_eq!(byte_offset_for_char_index(text, 0), 0);
        assert_eq!(byte_offset_for_char_index(text, 1), 1);
        assert_eq!(byte_offset_for_char_index(text, 2), 3);
        assert_eq!(byte_offset_for_char_index(text, 3), 4);
        assert_eq!(clamp_to_char_boundary(text, 2), 1);
        assert_eq!(clamp_to_char_boundary(text, 3), 3);
    }

    #[test]
    fn progress_blocks_contribute_to_message_height() {
        let baseline = AutopilotMessage {
            id: 2,
            role: AutopilotRole::Codex,
            status: AutopilotMessageStatus::Running,
            content: "plain".to_string(),
            structured: None,
        };
        let with_progress = fixture_progress_message("rebuilding");
        assert_eq!(message_progress_height(&baseline), 0.0);
        assert!(message_progress_height(&with_progress) > 0.0);
    }

    #[test]
    fn progress_status_colors_map_terminal_states() {
        assert_eq!(progress_status_color("done"), theme::status::SUCCESS);
        assert_eq!(progress_status_color("failed"), theme::status::ERROR);
        assert_eq!(progress_status_color("rebuilding"), theme::accent::PRIMARY);
    }

    #[test]
    fn tool_activity_event_filter_is_targeted() {
        assert!(is_tool_activity_event(
            "item completed: turn=abc id=xyz type=commandExecution"
        ));
        assert!(is_tool_activity_event("tool call requested"));
        assert!(!is_tool_activity_event("reasoning delta: chars=12"));
    }

    #[test]
    fn tool_activity_lines_include_pending_tool_calls_and_timeline() {
        let mut chat = AutopilotChatState::default();
        chat.pending_tool_calls.push(AutopilotToolCallRequest {
            request_id: AppServerRequestId::String("r1".to_string()),
            thread_id: "t1".to_string(),
            turn_id: "u1".to_string(),
            call_id: "call_1".to_string(),
            tool: "openagents.cad.intent".to_string(),
            arguments: "{}".to_string(),
        });
        chat.record_turn_timeline_event(
            "item completed: turn=u1 id=call_1 type=commandExecution".to_string(),
        );
        chat.record_turn_timeline_event("tool call requested".to_string());
        chat.record_turn_timeline_event("reasoning delta: chars=4".to_string());

        let lines = chat_tool_activity_lines(&chat);
        assert!(lines.iter().any(|line| line.contains("pending tool calls")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("openagents.cad.intent"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("type=commandExecution"))
        );
        assert!(!lines.iter().any(|line| line.contains("reasoning delta")));
    }
}
