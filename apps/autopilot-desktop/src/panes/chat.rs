use wgpui::markdown::{MarkdownConfig, MarkdownParser, MarkdownRenderer};
use wgpui::{Bounds, Component, InputEvent, PaintContext, Point, Quad, SvgQuad, theme};

use crate::app_state::{
    AutopilotChatState, AutopilotMessage, AutopilotMessageStatus, AutopilotProgressBlock,
    AutopilotProgressRow, AutopilotRole, ChatBrowseMode, ChatPaneInputs,
    ChatTranscriptSelectionState, DirectMessageMessageProjection, DirectMessageRoomProjection,
    ManagedChatChannelProjection, ManagedChatDeliveryState, ManagedChatGroupProjection,
    ManagedChatMessageProjection, PaneKind, RenderState,
};
use crate::labor_orchestrator::CodexLaborBinding;
use crate::pane_system::{
    chat_composer_height_for_value, chat_composer_input_bounds_with_height,
    chat_send_button_bounds, chat_thread_rail_bounds, chat_transcript_body_bounds_with_height,
    chat_transcript_bounds, chat_workspace_rail_bounds, pane_content_bounds,
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
const CHAT_SHELL_ROW_HEIGHT: f32 = 30.0;
const CHAT_WORKSPACE_AVATAR_SIZE: f32 = 36.0;
const CHAT_SEND_ICON_SVG_RAW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640"><path fill="#FFFFFF" d="M342.6 73.4C330.1 60.9 309.8 60.9 297.3 73.4L137.3 233.4C124.8 245.9 124.8 266.2 137.3 278.7C149.8 291.2 170.1 291.2 182.6 278.7L288 173.3L288 544C288 561.7 302.3 576 320 576C337.7 576 352 561.7 352 544L352 173.3L457.4 278.7C469.9 291.2 490.2 291.2 502.7 278.7C515.2 266.2 515.2 245.9 502.7 233.4L342.7 73.4z"/></svg>"##;

#[derive(Clone, Copy, Debug)]
struct WrappedTranscriptLine {
    start_byte_offset: usize,
    end_byte_offset: usize,
    char_count: usize,
}

struct ChatShellWorkspace {
    label: String,
    initials: String,
    accent: wgpui::Hsla,
    active: bool,
}

struct ChatShellChannelEntry {
    title: String,
    subtitle: String,
    active: bool,
    is_category: bool,
    collapsed: bool,
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
    let transcript_bounds =
        chat_transcript_body_bounds_with_height(content_bounds, composer_height);
    Bounds::new(
        transcript_bounds.origin.x,
        transcript_bounds.origin.y + 8.0,
        transcript_bounds.size.width.max(0.0),
        (transcript_bounds.size.height - 16.0).max(0.0),
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

fn local_turn_status_summary(status: Option<&str>) -> Option<&'static str> {
    match status {
        Some("completed") => Some("completed locally; not a labor verdict or settlement"),
        Some("failed") => Some("local execution failed"),
        Some("inProgress") => Some("local execution in progress"),
        _ => None,
    }
}

fn labor_binding_status_lines(binding: &CodexLaborBinding) -> Vec<String> {
    let mut lines = vec![
        format!("work unit: {}", binding.work_unit_id),
        format!("contract: {}", binding.contract_id),
        format!("submission: {}", binding.submission_runtime_state_label()),
        format!("verdict: {}", binding.verdict_runtime_state_label()),
        format!("settlement: {}", binding.ui_settlement_state_label()),
    ];
    if let Some(claim_state) = binding.claim_runtime_state_label() {
        lines.push(format!("claim: {claim_state}"));
    }
    lines
}

fn chat_turn_status_lines(autopilot_chat: &AutopilotChatState) -> Vec<String> {
    let Some(metadata) = autopilot_chat.active_turn_metadata() else {
        return Vec::new();
    };

    let mut lines = vec![
        format!("mode: {}", metadata.run_classification.ui_mode_label()),
        format!(
            "execution lane: {}",
            metadata.run_classification.ui_execution_lane_label()
        ),
        format!(
            "authority: {}",
            metadata.run_classification.ui_authority_label()
        ),
    ];

    if let Some(binding) = metadata.labor_binding.as_ref() {
        lines.extend(labor_binding_status_lines(binding));
    } else if let Some(summary) =
        local_turn_status_summary(autopilot_chat.last_turn_status.as_deref())
    {
        lines.push(format!("turn status: {summary}"));
    }

    lines
}

fn chat_tool_activity_lines(autopilot_chat: &AutopilotChatState) -> Vec<String> {
    let status_lines = chat_turn_status_lines(autopilot_chat);
    let mut pending_lines = Vec::new();

    if !autopilot_chat.pending_tool_calls.is_empty() {
        pending_lines.push(format!(
            "pending tool calls: {}",
            autopilot_chat.pending_tool_calls.len()
        ));
        for call in autopilot_chat.pending_tool_calls.iter().rev().take(3) {
            pending_lines.push(format!(
                "tool call queued: {} ({})",
                call.tool, call.call_id
            ));
        }
    }
    if !autopilot_chat.pending_command_approvals.is_empty() {
        pending_lines.push(format!(
            "pending command approvals: {}",
            autopilot_chat.pending_command_approvals.len()
        ));
    }
    if !autopilot_chat.pending_file_change_approvals.is_empty() {
        pending_lines.push(format!(
            "pending file-change approvals: {}",
            autopilot_chat.pending_file_change_approvals.len()
        ));
    }
    if !autopilot_chat.pending_tool_user_input.is_empty() {
        pending_lines.push(format!(
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

    let reserved = status_lines.len().saturating_add(pending_lines.len());
    let timeline_budget = CHAT_ACTIVITY_MAX_ROWS.saturating_sub(reserved);
    if timeline.len() > timeline_budget {
        let overflow = timeline.len().saturating_sub(timeline_budget);
        timeline.drain(0..overflow);
    }

    let mut lines = status_lines;
    lines.extend(pending_lines);
    lines.extend(timeline);
    lines.truncate(CHAT_ACTIVITY_MAX_ROWS);

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

    match autopilot_chat.chat_browse_mode() {
        ChatBrowseMode::Managed => {
            for message in autopilot_chat.active_managed_chat_messages() {
                height += CHAT_TRANSCRIPT_LINE_HEIGHT;
                if managed_message_reply_label(message).is_some() {
                    height += CHAT_ACTIVITY_ROW_LINE_HEIGHT;
                }
                let markdown_source = managed_message_markdown_source(message);
                let markdown_document = markdown_parser.parse(&markdown_source);
                let markdown_size =
                    markdown_renderer.measure(&markdown_document, markdown_width, text_system);
                height += markdown_size.height.max(CHAT_TRANSCRIPT_LINE_HEIGHT);
                if managed_message_reaction_summary(message).is_some() {
                    height += CHAT_ACTIVITY_ROW_LINE_HEIGHT;
                }
                if managed_message_delivery_note(message).is_some() {
                    height += CHAT_ACTIVITY_ROW_LINE_HEIGHT;
                }
                height += 8.0;
            }
            return height + 8.0;
        }
        ChatBrowseMode::DirectMessages => {
            for message in autopilot_chat.active_direct_message_messages() {
                height += CHAT_TRANSCRIPT_LINE_HEIGHT;
                if direct_message_reply_label(message).is_some() {
                    height += CHAT_ACTIVITY_ROW_LINE_HEIGHT;
                }
                let markdown_source = direct_message_markdown_source(message);
                let markdown_document = markdown_parser.parse(&markdown_source);
                let markdown_size =
                    markdown_renderer.measure(&markdown_document, markdown_width, text_system);
                height += markdown_size.height.max(CHAT_TRANSCRIPT_LINE_HEIGHT);
                if direct_message_delivery_note(message).is_some() {
                    height += CHAT_ACTIVITY_ROW_LINE_HEIGHT;
                }
                height += 8.0;
            }
            return height + 8.0;
        }
        ChatBrowseMode::Autopilot => {}
    }

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
    if autopilot_chat.chat_browse_mode() != ChatBrowseMode::Autopilot {
        return Vec::new();
    }
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

fn compact_shell_label(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "untitled".to_string();
    }
    if trimmed.chars().count() <= 18 {
        return trimmed.to_string();
    }

    let prefix = trimmed.chars().take(12).collect::<String>();
    let suffix = trimmed
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{prefix}…{suffix}")
}

fn compact_hex_label(value: &str, prefix_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    let prefix = trimmed
        .chars()
        .take(prefix_chars.max(1))
        .collect::<String>();
    if trimmed.chars().count() <= prefix_chars.max(1) {
        prefix
    } else {
        format!("{prefix}…")
    }
}

fn shell_initials(value: &str) -> String {
    let mut initials = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(2)
        .collect::<String>()
        .to_ascii_uppercase();
    if initials.is_empty() {
        initials.push_str("OA");
    }
    initials
}

fn managed_group_label(group: &ManagedChatGroupProjection) -> String {
    group
        .metadata
        .name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(compact_shell_label)
        .unwrap_or_else(|| compact_shell_label(&group.group_id))
}

fn managed_channel_label(channel: &ManagedChatChannelProjection) -> String {
    if !channel.metadata.name.trim().is_empty() {
        compact_shell_label(&channel.metadata.name)
    } else if let Some(slug) = channel.hints.slug.as_deref() {
        compact_shell_label(slug)
    } else {
        compact_shell_label(&channel.channel_id)
    }
}

fn managed_channel_subtitle(channel: &ManagedChatChannelProjection) -> String {
    if channel.unread_count > 0 {
        format!("{} unread", channel.unread_count)
    } else if !channel.metadata.about.trim().is_empty() {
        compact_shell_label(&channel.metadata.about)
    } else {
        channel.room_mode.to_string()
    }
}

fn managed_status_text(autopilot_chat: &AutopilotChatState) -> String {
    let server_count = autopilot_chat.managed_chat_projection.snapshot.groups.len();
    let channel_count = autopilot_chat
        .managed_chat_projection
        .snapshot
        .channels
        .len();
    let cached_events = autopilot_chat.managed_chat_projection.relay_events.len();
    let mut parts = vec![
        format!("{server_count} server(s)"),
        format!("{channel_count} channel(s)"),
        format!("{cached_events} cached"),
    ];
    let mut publishing = 0usize;
    let mut acked = 0usize;
    let mut failed = 0usize;
    for message in autopilot_chat
        .managed_chat_projection
        .snapshot
        .messages
        .values()
    {
        match message.delivery_state {
            ManagedChatDeliveryState::Publishing => publishing += 1,
            ManagedChatDeliveryState::Acked => acked += 1,
            ManagedChatDeliveryState::Failed => failed += 1,
            ManagedChatDeliveryState::Confirmed => {}
        }
    }
    if publishing > 0 {
        parts.push(format!("{publishing} sending"));
    }
    if acked > 0 {
        parts.push(format!("{acked} acked"));
    }
    if failed > 0 {
        parts.push(format!("{failed} failed"));
    }
    parts.join("  •  ")
}

fn managed_message_role_label(index: usize, message: &ManagedChatMessageProjection) -> String {
    let base = format!(
        "[#{}] [{}]",
        index + 1,
        compact_hex_label(&message.author_pubkey, 8)
    );
    match message.delivery_state {
        ManagedChatDeliveryState::Confirmed => base,
        ManagedChatDeliveryState::Publishing => {
            format!("{base} [sending x{}]", message.attempt_count.max(1))
        }
        ManagedChatDeliveryState::Acked => format!("{base} [acked]"),
        ManagedChatDeliveryState::Failed => {
            format!("{base} [failed x{}]", message.attempt_count.max(1))
        }
    }
}

fn managed_message_role_color(message: &ManagedChatMessageProjection) -> wgpui::Hsla {
    match message.delivery_state {
        ManagedChatDeliveryState::Confirmed => theme::accent::PRIMARY,
        ManagedChatDeliveryState::Publishing => theme::status::INFO,
        ManagedChatDeliveryState::Acked => theme::status::SUCCESS,
        ManagedChatDeliveryState::Failed => theme::status::ERROR,
    }
}

fn managed_message_reply_label(message: &ManagedChatMessageProjection) -> Option<String> {
    message
        .reply_to_event_id
        .as_deref()
        .map(|reply_id| format!("reply {}", compact_hex_label(reply_id, 8)))
}

fn managed_message_reaction_summary(message: &ManagedChatMessageProjection) -> Option<String> {
    if message.reaction_summaries.is_empty() {
        return None;
    }
    Some(
        message
            .reaction_summaries
            .iter()
            .map(|reaction| format!("{} x{}", reaction.content, reaction.count))
            .collect::<Vec<_>>()
            .join("  "),
    )
}

fn managed_message_delivery_note(message: &ManagedChatMessageProjection) -> Option<String> {
    match message.delivery_state {
        ManagedChatDeliveryState::Confirmed => None,
        ManagedChatDeliveryState::Publishing => Some(format!(
            "publishing local echo attempt {}",
            message.attempt_count.max(1)
        )),
        ManagedChatDeliveryState::Acked => {
            Some("relay acknowledged local echo; waiting for sync".to_string())
        }
        ManagedChatDeliveryState::Failed => Some(match message.delivery_error.as_deref() {
            Some(error) => format!("publish failed: {error}"),
            None => format!("publish failed on attempt {}", message.attempt_count.max(1)),
        }),
    }
}

fn managed_message_markdown_source(message: &ManagedChatMessageProjection) -> String {
    sanitize_chat_text(&message.content)
}

fn direct_room_label(room: &DirectMessageRoomProjection, local_pubkey: Option<&str>) -> String {
    if let Some(subject) = room.subject.as_deref() {
        return compact_shell_label(subject);
    }
    let mut others = room
        .participant_pubkeys
        .iter()
        .filter(|pubkey| Some(pubkey.as_str()) != local_pubkey)
        .map(|pubkey| compact_hex_label(pubkey, 8))
        .collect::<Vec<_>>();
    if others.is_empty() {
        return "direct".to_string();
    }
    let primary = others.remove(0);
    if others.is_empty() {
        format!("@ {primary}")
    } else {
        format!("@ {primary} +{}", others.len())
    }
}

fn direct_room_subtitle(room: &DirectMessageRoomProjection) -> String {
    let relay_hint_count = room
        .relay_hints
        .values()
        .map(|relays| relays.len())
        .sum::<usize>();
    let mut parts = vec![
        format!("{} participant(s)", room.participant_pubkeys.len()),
        format!("{} message(s)", room.message_ids.len()),
    ];
    if relay_hint_count > 0 {
        parts.push(format!("{relay_hint_count} relay hint(s)"));
    }
    parts.join("  •  ")
}

fn direct_status_text(autopilot_chat: &AutopilotChatState) -> String {
    let room_count = autopilot_chat
        .direct_message_projection
        .snapshot
        .rooms
        .len();
    let cached_events = autopilot_chat.direct_message_projection.relay_events.len();
    let relay_lists = autopilot_chat
        .direct_message_projection
        .snapshot
        .relay_lists
        .len();
    let mut parts = vec![
        format!("{room_count} room(s)"),
        format!("{cached_events} cached"),
        format!("{relay_lists} relay list(s)"),
    ];
    let mut publishing = 0usize;
    let mut acked = 0usize;
    let mut failed = 0usize;
    for message in autopilot_chat
        .direct_message_projection
        .snapshot
        .messages
        .values()
    {
        match message.delivery_state {
            ManagedChatDeliveryState::Publishing => publishing += 1,
            ManagedChatDeliveryState::Acked => acked += 1,
            ManagedChatDeliveryState::Failed => failed += 1,
            ManagedChatDeliveryState::Confirmed => {}
        }
    }
    if publishing > 0 {
        parts.push(format!("{publishing} sending"));
    }
    if acked > 0 {
        parts.push(format!("{acked} acked"));
    }
    if failed > 0 {
        parts.push(format!("{failed} failed"));
    }
    parts.join("  •  ")
}

fn direct_message_role_label(
    index: usize,
    message: &DirectMessageMessageProjection,
    local_pubkey: Option<&str>,
) -> String {
    let author_label = if Some(message.author_pubkey.as_str()) == local_pubkey {
        "you".to_string()
    } else {
        compact_hex_label(&message.author_pubkey, 8)
    };
    let base = format!("[#{}] [{}]", index + 1, author_label);
    match message.delivery_state {
        ManagedChatDeliveryState::Confirmed => base,
        ManagedChatDeliveryState::Publishing => {
            format!("{base} [sending x{}]", message.attempt_count.max(1))
        }
        ManagedChatDeliveryState::Acked => format!("{base} [acked]"),
        ManagedChatDeliveryState::Failed => {
            format!("{base} [failed x{}]", message.attempt_count.max(1))
        }
    }
}

fn direct_message_role_color(message: &DirectMessageMessageProjection) -> wgpui::Hsla {
    match message.delivery_state {
        ManagedChatDeliveryState::Confirmed => theme::status::SUCCESS,
        ManagedChatDeliveryState::Publishing => theme::status::INFO,
        ManagedChatDeliveryState::Acked => theme::status::SUCCESS,
        ManagedChatDeliveryState::Failed => theme::status::ERROR,
    }
}

fn direct_message_reply_label(message: &DirectMessageMessageProjection) -> Option<String> {
    message
        .reply_to_event_id
        .as_deref()
        .map(|reply_id| format!("reply {}", compact_hex_label(reply_id, 8)))
}

fn direct_message_delivery_note(message: &DirectMessageMessageProjection) -> Option<String> {
    match message.delivery_state {
        ManagedChatDeliveryState::Confirmed => None,
        ManagedChatDeliveryState::Publishing => Some(format!(
            "publishing local echo attempt {}",
            message.attempt_count.max(1)
        )),
        ManagedChatDeliveryState::Acked => {
            Some("relay acknowledged local echo; waiting for sync".to_string())
        }
        ManagedChatDeliveryState::Failed => Some(match message.delivery_error.as_deref() {
            Some(error) => format!("publish failed: {error}"),
            None => format!("publish failed on attempt {}", message.attempt_count.max(1)),
        }),
    }
}

fn direct_message_markdown_source(message: &DirectMessageMessageProjection) -> String {
    sanitize_chat_text(&message.content)
}

fn managed_local_delivery_summary(autopilot_chat: &AutopilotChatState) -> Option<String> {
    let mut publishing = 0usize;
    let mut acked = 0usize;
    let mut failed = 0usize;
    for message in autopilot_chat.active_managed_chat_messages() {
        match message.delivery_state {
            ManagedChatDeliveryState::Publishing => publishing += 1,
            ManagedChatDeliveryState::Acked => acked += 1,
            ManagedChatDeliveryState::Failed => failed += 1,
            ManagedChatDeliveryState::Confirmed => {}
        }
    }
    let mut parts = Vec::new();
    if publishing > 0 {
        parts.push(format!("{publishing} sending"));
    }
    if acked > 0 {
        parts.push(format!("{acked} acked"));
    }
    if failed > 0 {
        parts.push(format!("{failed} failed local"));
    }
    (!parts.is_empty()).then(|| parts.join("  •  "))
}

fn direct_local_delivery_summary(autopilot_chat: &AutopilotChatState) -> Option<String> {
    let mut publishing = 0usize;
    let mut acked = 0usize;
    let mut failed = 0usize;
    for message in autopilot_chat.active_direct_message_messages() {
        match message.delivery_state {
            ManagedChatDeliveryState::Publishing => publishing += 1,
            ManagedChatDeliveryState::Acked => acked += 1,
            ManagedChatDeliveryState::Failed => failed += 1,
            ManagedChatDeliveryState::Confirmed => {}
        }
    }
    let mut parts = Vec::new();
    if publishing > 0 {
        parts.push(format!("{publishing} sending"));
    }
    if acked > 0 {
        parts.push(format!("{acked} acked"));
    }
    if failed > 0 {
        parts.push(format!("{failed} failed local"));
    }
    (!parts.is_empty()).then(|| parts.join("  •  "))
}

fn managed_chat_composer_hint(autopilot_chat: &AutopilotChatState, composer_value: &str) -> String {
    let Some(channel) = autopilot_chat.active_managed_chat_channel() else {
        return "No managed channel selected.".to_string();
    };
    if channel.relay_url.is_none() {
        return "Channel relay target is unknown; publish waits for metadata or synced history."
            .to_string();
    }
    if composer_value.trim().is_empty()
        && autopilot_chat
            .active_managed_chat_retryable_message()
            .is_some()
    {
        return "Use `reply <#|id> <text>` or `react <#|id> <emoji>`. Empty composer retries the latest failed publish."
            .to_string();
    }
    "Use `reply <#|id> <text>` or `react <#|id> <emoji>`. `@hexprefix` adds mention tags. Shift+Enter inserts a newline."
        .to_string()
}

fn direct_message_composer_hint(
    autopilot_chat: &AutopilotChatState,
    composer_value: &str,
) -> String {
    if composer_value.trim().is_empty()
        && autopilot_chat
            .active_direct_message_retryable_message()
            .is_some()
    {
        return "Use `reply <#|id> <text>`, `dm <pubkey> <text>`, or `room <pubkey[,pubkey...]> | <subject> | <text>`. Empty composer retries the latest failed DM publish."
            .to_string();
    }
    "Use plain text in the selected room, `reply <#|id> <text>`, `dm <pubkey> <text>`, or `room <pubkey[,pubkey...]> | <subject> | <text>`."
        .to_string()
}

fn active_thread_title(autopilot_chat: &AutopilotChatState) -> String {
    match autopilot_chat.chat_browse_mode() {
        ChatBrowseMode::Managed => {
            if let Some(channel) = autopilot_chat.active_managed_chat_channel() {
                return managed_channel_label(channel);
            }
        }
        ChatBrowseMode::DirectMessages => {
            if let Some(room) = autopilot_chat.active_direct_message_room() {
                let local_pubkey = autopilot_chat.direct_message_projection.local_pubkey();
                return direct_room_label(room, local_pubkey);
            }
        }
        ChatBrowseMode::Autopilot => {}
    }

    autopilot_chat
        .active_thread_id
        .as_ref()
        .and_then(|thread_id| autopilot_chat.thread_metadata.get(thread_id))
        .and_then(|metadata| metadata.thread_name.as_ref())
        .map(|name| compact_shell_label(name))
        .or_else(|| {
            autopilot_chat
                .active_thread_id
                .as_ref()
                .map(|thread_id| compact_shell_label(thread_id))
        })
        .unwrap_or_else(|| "Mission control".to_string())
}

fn active_thread_subtitle(autopilot_chat: &AutopilotChatState) -> String {
    match autopilot_chat.chat_browse_mode() {
        ChatBrowseMode::Managed => {
            if let (Some(group), Some(channel)) = (
                autopilot_chat.active_managed_chat_group(),
                autopilot_chat.active_managed_chat_channel(),
            ) {
                let mut parts = vec![
                    managed_group_label(group),
                    format!("{} message(s)", channel.message_ids.len()),
                    channel.room_mode.to_string(),
                ];
                if let Some(local_delivery) = managed_local_delivery_summary(autopilot_chat) {
                    parts.push(local_delivery);
                }
                return parts.join("  •  ");
            }
        }
        ChatBrowseMode::DirectMessages => {
            if let Some(room) = autopilot_chat.active_direct_message_room() {
                let mut parts = vec![
                    format!("{} participant(s)", room.participant_pubkeys.len()),
                    format!("{} message(s)", room.message_ids.len()),
                ];
                if let Some(local_delivery) = direct_local_delivery_summary(autopilot_chat) {
                    parts.push(local_delivery);
                }
                return parts.join("  •  ");
            }
        }
        ChatBrowseMode::Autopilot => {}
    }

    let status = autopilot_chat
        .active_thread_id
        .as_ref()
        .and_then(|thread_id| autopilot_chat.thread_metadata.get(thread_id))
        .and_then(|metadata| metadata.status.as_ref())
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_else(|| autopilot_chat.connection_status.trim().to_ascii_lowercase());
    format!(
        "autopilot thread  •  {}",
        if status.is_empty() { "ready" } else { &status }
    )
}

fn shell_workspaces(autopilot_chat: &AutopilotChatState) -> Vec<ChatShellWorkspace> {
    if autopilot_chat.chat_has_browseable_content() {
        let accents = [
            theme::accent::PRIMARY,
            theme::status::SUCCESS,
            theme::status::WARNING,
            theme::status::INFO,
        ];
        return autopilot_chat
            .chat_workspace_entries()
            .into_iter()
            .enumerate()
            .filter_map(|(index, workspace)| match workspace {
                crate::app_state::ChatWorkspaceSelection::ManagedGroup(group_id) => {
                    let group = autopilot_chat
                        .managed_chat_projection
                        .snapshot
                        .groups
                        .iter()
                        .find(|group| group.group_id == group_id)?;
                    let label = managed_group_label(group);
                    Some(ChatShellWorkspace {
                        initials: shell_initials(&label),
                        label,
                        accent: accents[index % accents.len()],
                        active: autopilot_chat.chat_browse_mode() == ChatBrowseMode::Managed
                            && autopilot_chat
                                .active_managed_chat_group()
                                .is_some_and(|active| active.group_id == group.group_id),
                    })
                }
                crate::app_state::ChatWorkspaceSelection::DirectMessages => {
                    Some(ChatShellWorkspace {
                        label: "Direct".to_string(),
                        initials: "DM".to_string(),
                        accent: theme::status::SUCCESS,
                        active: autopilot_chat.chat_browse_mode() == ChatBrowseMode::DirectMessages,
                    })
                }
                crate::app_state::ChatWorkspaceSelection::Autopilot => None,
            })
            .collect();
    }

    vec![
        ChatShellWorkspace {
            label: "OpenAgents".to_string(),
            initials: "OA".to_string(),
            accent: theme::accent::PRIMARY,
            active: true,
        },
        ChatShellWorkspace {
            label: "Direct".to_string(),
            initials: "DM".to_string(),
            accent: theme::status::SUCCESS,
            active: false,
        },
        ChatShellWorkspace {
            label: "Ops".to_string(),
            initials: "OP".to_string(),
            accent: theme::status::WARNING,
            active: false,
        },
    ]
}

fn shell_channel_entries(autopilot_chat: &AutopilotChatState) -> Vec<ChatShellChannelEntry> {
    match autopilot_chat.chat_browse_mode() {
        ChatBrowseMode::Managed => {
            let active_channel_id = autopilot_chat
                .active_managed_chat_channel()
                .map(|channel| channel.channel_id.as_str());
            return autopilot_chat
                .active_managed_chat_channel_rail_rows()
                .into_iter()
                .filter_map(|row| match row {
                    crate::app_state::ManagedChatChannelRailRow::Category {
                        label,
                        collapsed,
                        channel_count,
                        ..
                    } => Some(ChatShellChannelEntry {
                        title: format!("{} {}", if collapsed { "▸" } else { "▾" }, label),
                        subtitle: format!("{channel_count} channel(s)"),
                        active: false,
                        is_category: true,
                        collapsed,
                    }),
                    crate::app_state::ManagedChatChannelRailRow::Channel { channel_id } => {
                        let channel = autopilot_chat
                            .managed_chat_projection
                            .snapshot
                            .channels
                            .iter()
                            .find(|channel| channel.channel_id == channel_id)?;
                        Some(ChatShellChannelEntry {
                            title: format!("# {}", managed_channel_label(channel)),
                            subtitle: managed_channel_subtitle(channel),
                            active: active_channel_id == Some(channel.channel_id.as_str()),
                            is_category: false,
                            collapsed: false,
                        })
                    }
                })
                .collect();
        }
        ChatBrowseMode::DirectMessages => {
            let active_room_id = autopilot_chat
                .active_direct_message_room()
                .map(|room| room.room_id.as_str());
            return autopilot_chat
                .active_direct_message_rooms()
                .into_iter()
                .map(|room| ChatShellChannelEntry {
                    title: direct_room_label(
                        room,
                        autopilot_chat.direct_message_projection.local_pubkey(),
                    ),
                    subtitle: direct_room_subtitle(room),
                    active: active_room_id == Some(room.room_id.as_str()),
                    is_category: false,
                    collapsed: false,
                })
                .collect();
        }
        ChatBrowseMode::Autopilot => {}
    }

    let mut entries = vec![ChatShellChannelEntry {
        title: "# mission-control".to_string(),
        subtitle: "provider coordination".to_string(),
        active: autopilot_chat.active_thread_id.is_none(),
        is_category: false,
        collapsed: false,
    }];

    entries.extend(autopilot_chat.threads.iter().take(6).map(|thread_id| {
        let metadata = autopilot_chat.thread_metadata.get(thread_id);
        let title = metadata
            .and_then(|value| value.thread_name.as_ref())
            .map(|name| compact_shell_label(name))
            .unwrap_or_else(|| compact_shell_label(thread_id));
        let subtitle = metadata
            .and_then(|value| value.status.as_ref())
            .map(|status| status.trim().to_ascii_lowercase())
            .filter(|status| !status.is_empty())
            .unwrap_or_else(|| "thread".to_string());
        ChatShellChannelEntry {
            title: format!("# {title}"),
            subtitle,
            active: autopilot_chat.active_thread_id.as_deref() == Some(thread_id.as_str()),
            is_category: false,
            collapsed: false,
        }
    }));

    entries.push(ChatShellChannelEntry {
        title: "@ approvals".to_string(),
        subtitle: format!(
            "{} pending",
            autopilot_chat.pending_command_approvals.len()
                + autopilot_chat.pending_file_change_approvals.len()
                + autopilot_chat.pending_tool_calls.len()
        ),
        active: false,
        is_category: false,
        collapsed: false,
    });
    entries
}

fn paint_chat_shell(
    content_bounds: Bounds,
    autopilot_chat: &AutopilotChatState,
    paint: &mut PaintContext,
) {
    let workspace_bounds = chat_workspace_rail_bounds(content_bounds);
    let channel_bounds = chat_thread_rail_bounds(content_bounds);
    let transcript_bounds = chat_transcript_bounds(content_bounds);
    let header_bounds = Bounds::new(
        transcript_bounds.origin.x + 8.0,
        transcript_bounds.origin.y + 8.0,
        (transcript_bounds.size.width - 16.0).max(0.0),
        52.0,
    );

    paint.scene.draw_quad(
        Quad::new(workspace_bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(10.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        "WORKSPACES",
        Point::new(
            workspace_bounds.origin.x + 8.0,
            workspace_bounds.origin.y + 10.0,
        ),
        9.0,
        theme::text::MUTED,
    ));
    for (index, workspace) in shell_workspaces(autopilot_chat).iter().enumerate() {
        let avatar_bounds = Bounds::new(
            workspace_bounds.origin.x
                + (workspace_bounds.size.width - CHAT_WORKSPACE_AVATAR_SIZE) * 0.5,
            workspace_bounds.origin.y + 28.0 + index as f32 * 52.0,
            CHAT_WORKSPACE_AVATAR_SIZE,
            CHAT_WORKSPACE_AVATAR_SIZE,
        );
        let background = if workspace.active {
            workspace.accent
        } else {
            theme::bg::ELEVATED
        };
        let text_color = if workspace.active {
            theme::bg::APP
        } else {
            theme::text::SECONDARY
        };
        paint.scene.draw_quad(
            Quad::new(avatar_bounds)
                .with_background(background)
                .with_border(
                    if workspace.active {
                        workspace.accent
                    } else {
                        theme::border::DEFAULT
                    },
                    1.0,
                )
                .with_corner_radius(CHAT_WORKSPACE_AVATAR_SIZE * 0.5),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &workspace.initials,
            Point::new(avatar_bounds.origin.x + 8.0, avatar_bounds.origin.y + 11.0),
            11.0,
            text_color,
        ));
        paint.scene.draw_text(paint.text.layout(
            &workspace.label,
            Point::new(
                workspace_bounds.origin.x + 10.0,
                avatar_bounds.max_y() + 4.0,
            ),
            9.0,
            if workspace.active {
                theme::text::PRIMARY
            } else {
                theme::text::MUTED
            },
        ));
    }

    paint.scene.draw_quad(
        Quad::new(channel_bounds)
            .with_background(theme::bg::ELEVATED.with_alpha(0.92))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(10.0),
    );
    let (shell_mode_label, rail_title) = match autopilot_chat.chat_browse_mode() {
        ChatBrowseMode::Managed => ("OPENAGENTS / GROUP CHAT", "Channels"),
        ChatBrowseMode::DirectMessages => ("OPENAGENTS / DIRECT MESSAGES", "Rooms"),
        ChatBrowseMode::Autopilot => ("OPENAGENTS / AUTOPILOT", "Threads"),
    };
    paint.scene.draw_text(paint.text.layout_mono(
        shell_mode_label,
        Point::new(
            channel_bounds.origin.x + 12.0,
            channel_bounds.origin.y + 12.0,
        ),
        9.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        rail_title,
        Point::new(
            channel_bounds.origin.x + 12.0,
            channel_bounds.origin.y + 28.0,
        ),
        15.0,
        theme::text::PRIMARY,
    ));
    let mut row_y = channel_bounds.origin.y + 54.0;
    for entry in shell_channel_entries(autopilot_chat) {
        let row_bounds = Bounds::new(
            channel_bounds.origin.x + 8.0,
            row_y,
            (channel_bounds.size.width - 16.0).max(0.0),
            CHAT_SHELL_ROW_HEIGHT,
        );
        let background = if entry.is_category {
            theme::bg::APP.with_alpha(0.08)
        } else if entry.active {
            theme::accent::PRIMARY.with_alpha(0.16)
        } else {
            theme::bg::APP.with_alpha(0.26)
        };
        let border = if entry.is_category {
            theme::border::DEFAULT.with_alpha(0.18)
        } else if entry.active {
            theme::accent::PRIMARY.with_alpha(0.45)
        } else {
            theme::border::DEFAULT.with_alpha(0.35)
        };
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(background)
                .with_border(border, 1.0)
                .with_corner_radius(8.0),
        );
        let title_color = if entry.is_category {
            theme::text::MUTED
        } else if entry.active {
            theme::text::PRIMARY
        } else {
            theme::text::SECONDARY
        };
        paint.scene.draw_text(paint.text.layout(
            &entry.title,
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 6.0),
            11.0,
            title_color,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            &entry.subtitle,
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 18.0),
            9.0,
            if entry.is_category {
                theme::text::MUTED.with_alpha(0.85)
            } else {
                theme::text::MUTED
            },
        ));
        row_y += CHAT_SHELL_ROW_HEIGHT + 6.0;
    }

    paint.scene.draw_quad(
        Quad::new(transcript_bounds)
            .with_background(theme::bg::APP.with_alpha(0.82))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(10.0),
    );
    paint.scene.draw_quad(
        Quad::new(header_bounds)
            .with_background(theme::bg::SURFACE.with_alpha(0.82))
            .with_border(theme::border::DEFAULT.with_alpha(0.45), 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_text(paint.text.layout(
        &active_thread_title(autopilot_chat),
        Point::new(header_bounds.origin.x + 12.0, header_bounds.origin.y + 10.0),
        16.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        &active_thread_subtitle(autopilot_chat),
        Point::new(header_bounds.origin.x + 12.0, header_bounds.origin.y + 28.0),
        10.0,
        theme::text::MUTED,
    ));
    let status_text = match autopilot_chat.chat_browse_mode() {
        ChatBrowseMode::Managed => managed_status_text(autopilot_chat),
        ChatBrowseMode::DirectMessages => direct_status_text(autopilot_chat),
        ChatBrowseMode::Autopilot => format!(
            "{}  •  {} model{}",
            autopilot_chat.connection_status.trim(),
            autopilot_chat.models.len(),
            if autopilot_chat.models.len() == 1 {
                ""
            } else {
                "s"
            }
        ),
    };
    paint.scene.draw_text(paint.text.layout_mono(
        &status_text,
        Point::new(header_bounds.max_x() - 150.0, header_bounds.origin.y + 18.0),
        10.0,
        theme::accent::PRIMARY,
    ));
}

pub fn transcript_message_byte_offset_at_point(
    state: &mut RenderState,
    point: Point,
) -> Option<(u64, usize)> {
    if state.autopilot_chat.chat_browse_mode() != ChatBrowseMode::Autopilot {
        return None;
    }
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
    if state.autopilot_chat.chat_browse_mode() != ChatBrowseMode::Autopilot {
        return None;
    }
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
    if state.autopilot_chat.chat_browse_mode() != ChatBrowseMode::Autopilot {
        return None;
    }
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
    let browse_mode = autopilot_chat.chat_browse_mode();
    let composer_value = chat_inputs.composer.get_value().to_string();
    let composer_height = chat_composer_height_for_value(content_bounds, &composer_value);
    let transcript_body_bounds =
        chat_transcript_body_bounds_with_height(content_bounds, composer_height);
    let composer_bounds = chat_composer_input_bounds_with_height(content_bounds, composer_height);
    let send_bounds = chat_send_button_bounds(content_bounds);
    paint_chat_shell(content_bounds, autopilot_chat, paint);

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

    match browse_mode {
        ChatBrowseMode::Managed => {
            let managed_messages = autopilot_chat.active_managed_chat_messages();
            if managed_messages.is_empty() {
                let empty_state = "No managed channel history backfilled yet.";
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

            for (index, message) in managed_messages.into_iter().enumerate() {
                paint.scene.draw_text(paint.text.layout_mono(
                    &managed_message_role_label(index, message),
                    Point::new(transcript_scroll_clip.origin.x, y),
                    10.0,
                    managed_message_role_color(message),
                ));
                y += CHAT_TRANSCRIPT_LINE_HEIGHT;

                if let Some(reply_label) = managed_message_reply_label(message) {
                    paint.scene.draw_text(paint.text.layout_mono(
                        &reply_label,
                        Point::new(transcript_scroll_clip.origin.x + 6.0, y),
                        9.0,
                        theme::text::MUTED,
                    ));
                    y += CHAT_ACTIVITY_ROW_LINE_HEIGHT;
                }

                let markdown_source = managed_message_markdown_source(message);
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

                if let Some(reaction_summary) = managed_message_reaction_summary(message) {
                    paint.scene.draw_text(paint.text.layout_mono(
                        &reaction_summary,
                        Point::new(transcript_scroll_clip.origin.x + 6.0, y),
                        9.0,
                        theme::text::MUTED,
                    ));
                    y += CHAT_ACTIVITY_ROW_LINE_HEIGHT;
                }
                if let Some(delivery_note) = managed_message_delivery_note(message) {
                    paint.scene.draw_text(paint.text.layout_mono(
                        &delivery_note,
                        Point::new(transcript_scroll_clip.origin.x + 6.0, y),
                        9.0,
                        managed_message_role_color(message),
                    ));
                    y += CHAT_ACTIVITY_ROW_LINE_HEIGHT;
                }
                y += 8.0;
            }
        }
        ChatBrowseMode::DirectMessages => {
            let direct_messages = autopilot_chat.active_direct_message_messages();
            if direct_messages.is_empty() {
                let empty_state = "No direct message history backfilled yet.";
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

            for (index, message) in direct_messages.into_iter().enumerate() {
                paint.scene.draw_text(paint.text.layout_mono(
                    &direct_message_role_label(
                        index,
                        message,
                        autopilot_chat.direct_message_projection.local_pubkey(),
                    ),
                    Point::new(transcript_scroll_clip.origin.x, y),
                    10.0,
                    direct_message_role_color(message),
                ));
                y += CHAT_TRANSCRIPT_LINE_HEIGHT;

                if let Some(reply_label) = direct_message_reply_label(message) {
                    paint.scene.draw_text(paint.text.layout_mono(
                        &reply_label,
                        Point::new(transcript_scroll_clip.origin.x + 6.0, y),
                        9.0,
                        theme::text::MUTED,
                    ));
                    y += CHAT_ACTIVITY_ROW_LINE_HEIGHT;
                }

                let markdown_source = direct_message_markdown_source(message);
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

                if let Some(delivery_note) = direct_message_delivery_note(message) {
                    paint.scene.draw_text(paint.text.layout_mono(
                        &delivery_note,
                        Point::new(transcript_scroll_clip.origin.x + 6.0, y),
                        9.0,
                        direct_message_role_color(message),
                    ));
                    y += CHAT_ACTIVITY_ROW_LINE_HEIGHT;
                }
                y += 8.0;
            }
        }
        ChatBrowseMode::Autopilot => {
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
                y += paint_message_progress_blocks(
                    message,
                    transcript_scroll_clip.origin.x,
                    y,
                    paint,
                );
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
        }
    }
    paint.scene.pop_clip();

    let mut footer_y = transcript_body_bounds.max_y() - 12.0;
    if browse_mode == ChatBrowseMode::Managed || browse_mode == ChatBrowseMode::DirectMessages {
        let hint = if browse_mode == ChatBrowseMode::Managed {
            managed_chat_composer_hint(autopilot_chat, &composer_value)
        } else {
            direct_message_composer_hint(autopilot_chat, &composer_value)
        };
        paint.scene.draw_text(paint.text.layout_mono(
            &hint,
            Point::new(transcript_body_bounds.origin.x, footer_y),
            9.0,
            theme::text::MUTED,
        ));
        footer_y -= CHAT_TRANSCRIPT_LINE_HEIGHT;
    }
    if let Some(error) = autopilot_chat.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(transcript_body_bounds.origin.x, footer_y),
            11.0,
            theme::status::ERROR,
        ));
        footer_y -= CHAT_TRANSCRIPT_LINE_HEIGHT;
    }
    if let Some(copy_notice) = autopilot_chat.copy_notice.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            copy_notice,
            Point::new(transcript_body_bounds.origin.x, footer_y),
            11.0,
            theme::status::SUCCESS,
        ));
    }

    chat_inputs
        .composer
        .set_max_width(composer_bounds.size.width);
    chat_inputs.composer.paint(composer_bounds, paint);
    let can_send = match browse_mode {
        ChatBrowseMode::Managed => {
            autopilot_chat.managed_chat_can_send(chat_inputs.composer.get_value())
        }
        ChatBrowseMode::DirectMessages => {
            autopilot_chat.direct_message_can_send(chat_inputs.composer.get_value())
        }
        ChatBrowseMode::Autopilot => !chat_inputs.composer.get_value().trim().is_empty(),
    };
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
        AutopilotTurnMetadata,
    };
    use crate::labor_orchestrator::{
        CodexLaborBinding, CodexLaborClaimState, CodexLaborProvenanceBundle,
        CodexLaborSubmissionState, CodexLaborVerdictState, CodexLaborVerifierPath,
        CodexRunClassification,
    };
    use codex_client::AppServerRequestId;
    use openagents_kernel_core::labor::{
        ClaimHook, ClaimHookStatus, SettlementStatus, Submission, SubmissionStatus, Verdict,
        VerdictOutcome,
    };
    use openagents_kernel_core::receipts::TraceContext;
    use serde_json::json;
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

    fn fixture_turn_metadata(
        run_classification: CodexRunClassification,
        labor_binding: Option<CodexLaborBinding>,
    ) -> AutopilotTurnMetadata {
        AutopilotTurnMetadata {
            submission_seq: 1,
            thread_id: "thread-1".to_string(),
            run_classification,
            labor_binding,
            is_cad_turn: false,
            classifier_reason: "test fixture".to_string(),
            submitted_at_epoch_ms: 1_730_000_000_000,
            selected_skill_names: vec!["skill.alpha".to_string()],
        }
    }

    fn fixture_labor_binding() -> CodexLaborBinding {
        CodexLaborBinding {
            work_unit_id: "work-unit-1".to_string(),
            contract_id: "contract-1".to_string(),
            idempotency_key: "idem-1".to_string(),
            trace: TraceContext::default(),
            provenance: CodexLaborProvenanceBundle {
                bundle_id: "bundle-1".to_string(),
                thread_id: "thread-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                prompt_digest: "sha256:prompt".to_string(),
                selected_model_id: Some("gpt-test".to_string()),
                selected_skill_names: vec!["skill.alpha".to_string()],
                cwd: Some("/tmp/openagents".to_string()),
                sandbox_policy: Some("workspace-write".to_string()),
                approval_policy: Some("on-failure".to_string()),
                approval_events: Vec::new(),
                tool_invocations: Vec::new(),
                produced_artifacts: Vec::new(),
                final_output_digest: Some("sha256:output".to_string()),
                transcript_digest: Some("sha256:transcript".to_string()),
            },
            required_artifact_kinds: vec!["final_output".to_string(), "transcript".to_string()],
            attached_evidence_refs: Vec::new(),
            incident_evidence_refs: Vec::new(),
            submission: Some(CodexLaborSubmissionState {
                submission: Submission {
                    submission_id: "submission-1".to_string(),
                    contract_id: "contract-1".to_string(),
                    work_unit_id: "work-unit-1".to_string(),
                    created_at_ms: 1_730_000_000_100,
                    status: SubmissionStatus::Accepted,
                    output_ref: Some("oa://autopilot/codex/work-unit-1/output".to_string()),
                    provenance_digest: Some("sha256:bundle".to_string()),
                    metadata: json!({}),
                },
                evidence_refs: Vec::new(),
                verifier_path: CodexLaborVerifierPath::DeterministicOutputGate,
                verifier_id: "verifier-1".to_string(),
                settlement_ready: true,
            }),
            verdict: Some(CodexLaborVerdictState {
                verdict: Verdict {
                    verdict_id: "verdict-1".to_string(),
                    contract_id: "contract-1".to_string(),
                    work_unit_id: "work-unit-1".to_string(),
                    created_at_ms: 1_730_000_000_200,
                    outcome: VerdictOutcome::Fail,
                    verification_tier: None,
                    settlement_status: SettlementStatus::Disputed,
                    reason_code: Some("deterministic_output_mismatch".to_string()),
                    metadata: json!({}),
                },
                evidence_refs: Vec::new(),
                verifier_path: CodexLaborVerifierPath::DeterministicOutputGate,
                verifier_id: "verifier-1".to_string(),
                independence_note: Some("heterogeneous checker pending".to_string()),
                correlation_note: None,
                settlement_ready: false,
                settlement_withheld_reason: Some("claim pending".to_string()),
            }),
            claim: Some(CodexLaborClaimState {
                claim: ClaimHook {
                    claim_id: "claim-1".to_string(),
                    contract_id: "contract-1".to_string(),
                    work_unit_id: "work-unit-1".to_string(),
                    created_at_ms: 1_730_000_000_300,
                    status: ClaimHookStatus::UnderReview,
                    reason_code: Some("deterministic_output_mismatch".to_string()),
                    metadata: json!({}),
                },
                evidence_refs: Vec::new(),
                status_note: Some("review in progress".to_string()),
                reviewed_at_epoch_ms: Some(1_730_000_000_350),
                resolved_at_epoch_ms: None,
                remedy: None,
            }),
            verifier_failure: None,
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

    #[test]
    fn tool_activity_lines_make_personal_agent_turns_explicitly_local() {
        let mut chat = AutopilotChatState::default();
        chat.last_submitted_turn_metadata = Some(fixture_turn_metadata(
            CodexRunClassification::PersonalAgent,
            None,
        ));
        chat.last_turn_status = Some("completed".to_string());

        let lines = chat_tool_activity_lines(&chat);

        assert!(lines.iter().any(|line| line == "mode: personal agent"));
        assert!(
            lines
                .iter()
                .any(|line| line == "execution lane: personal agent / Codex")
        );
        assert!(lines.iter().any(|line| line == "authority: local only"));
        assert!(lines.iter().any(|line| {
            line == "turn status: completed locally; not a labor verdict or settlement"
        }));
    }

    #[test]
    fn tool_activity_lines_show_labor_contract_state_and_claims() {
        let mut chat = AutopilotChatState::default();
        chat.last_submitted_turn_metadata = Some(fixture_turn_metadata(
            CodexRunClassification::LaborMarket {
                work_unit_id: "work-unit-1".to_string(),
                contract_id: Some("contract-1".to_string()),
            },
            Some(fixture_labor_binding()),
        ));

        let lines = chat_tool_activity_lines(&chat);

        assert!(lines.iter().any(|line| line == "mode: labor / contract"));
        assert!(
            lines
                .iter()
                .any(|line| line == "authority: projected / non-authoritative")
        );
        assert!(lines.iter().any(|line| line == "work unit: work-unit-1"));
        assert!(lines.iter().any(|line| line == "contract: contract-1"));
        assert!(lines.iter().any(|line| line == "submission: accepted"));
        assert!(lines.iter().any(|line| line == "verdict: fail"));
        assert!(
            lines
                .iter()
                .any(|line| line == "settlement: claim / dispute path")
        );
        assert!(lines.iter().any(|line| line == "claim: under_review"));
    }
}
