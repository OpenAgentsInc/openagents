use wgpui::markdown::{MarkdownConfig, MarkdownParser, MarkdownRenderer};
use wgpui::{Bounds, Component, InputEvent, PaintContext, Point, Quad, theme};

use crate::app_state::{
    AutopilotChatState, AutopilotMessage, AutopilotMessageStatus, AutopilotProgressBlock,
    AutopilotProgressRow, AutopilotRole, ChatPaneInputs, PaneKind, RenderState,
};
use crate::pane_renderer::paint_action_button;
use crate::pane_system::{
    chat_composer_input_bounds, chat_send_button_bounds, chat_transcript_bounds,
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

fn transcript_scroll_clip_bounds(content_bounds: Bounds) -> Bounds {
    let transcript_bounds = chat_transcript_bounds(content_bounds);
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
    autopilot_chat: &AutopilotChatState,
    text_system: &mut wgpui::TextSystem,
) -> f32 {
    let mut height = 8.0;

    let transcript_scroll_clip = transcript_scroll_clip_bounds(content_bounds);
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

fn transcript_message_layouts(
    state: &mut RenderState,
    content_bounds: Bounds,
) -> Vec<(u64, Bounds)> {
    let autopilot_chat = &state.autopilot_chat;
    let transcript_scroll_clip = transcript_scroll_clip_bounds(content_bounds);
    let transcript_content_height =
        transcript_content_height(content_bounds, autopilot_chat, &mut state.text_system);
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

pub fn transcript_message_id_at_point(state: &mut RenderState, point: Point) -> Option<u64> {
    let content_bounds = top_chat_content_bounds(state)?;
    let clip = transcript_scroll_clip_bounds(content_bounds);
    if !clip.contains(point) {
        return None;
    }
    transcript_message_layouts(state, content_bounds)
        .into_iter()
        .find(|(_, bounds)| bounds.contains(point))
        .map(|(id, _)| id)
}

pub fn transcript_message_copy_text_by_id(state: &RenderState, message_id: u64) -> Option<String> {
    state
        .autopilot_chat
        .messages
        .iter()
        .find(|message| message.id == message_id)
        .map(|message| sanitize_chat_text(&message_display_content(message)))
}

pub fn paint(
    content_bounds: Bounds,
    autopilot_chat: &AutopilotChatState,
    chat_inputs: &mut ChatPaneInputs,
    paint: &mut PaintContext,
) {
    let transcript_bounds = chat_transcript_bounds(content_bounds);
    let composer_bounds = chat_composer_input_bounds(content_bounds);
    let send_bounds = chat_send_button_bounds(content_bounds);

    paint.scene.draw_quad(
        Quad::new(transcript_bounds)
            .with_background(theme::bg::APP.with_alpha(0.82))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(4.0),
    );

    let transcript_scroll_clip = transcript_scroll_clip_bounds(content_bounds);
    let transcript_content_height =
        transcript_content_height(content_bounds, autopilot_chat, paint.text);
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
        paint.scene.draw_text(paint.text.layout(
            "No messages yet. Send a message to start.",
            Point::new(transcript_scroll_clip.origin.x, y + 14.0),
            11.0,
            theme::text::MUTED,
        ));
    }

    for message in &autopilot_chat.messages {
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
    paint_action_button(send_bounds, "Send", paint);
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

    let composer_bounds = chat_composer_input_bounds(pane_content_bounds(bounds));
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
    let clip = transcript_scroll_clip_bounds(content_bounds);
    if !clip.contains(cursor_position) {
        return false;
    }

    let content_height = transcript_content_height(
        content_bounds,
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
        chat_tool_activity_lines, is_tool_activity_event, message_progress_height,
        progress_status_color, sanitize_chat_text,
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
