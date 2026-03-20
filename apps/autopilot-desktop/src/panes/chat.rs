use reqwest::Url;
use wgpui::markdown::{MarkdownConfig, MarkdownParser, MarkdownRenderer};
use wgpui::{Bounds, Component, InputEvent, PaintContext, Point, Quad, SvgQuad, theme};

use crate::app_state::{
    AutopilotChatState, AutopilotCompactionArtifact, AutopilotDiffArtifact, AutopilotMessage,
    AutopilotMessageStatus, AutopilotPlanArtifact, AutopilotProgressBlock, AutopilotProgressRow,
    AutopilotReviewArtifact, AutopilotRole, AutopilotTerminalSession, ChatBrowseMode,
    ChatPaneInputs, ChatTranscriptSelectionState, DirectMessageMessageProjection,
    DirectMessageRoomProjection, ManagedChatChannelProjection, ManagedChatDeliveryState,
    ManagedChatGroupProjection, ManagedChatMessageProjection, PaneKind, RenderState,
};
use crate::labor_orchestrator::CodexLaborBinding;
use crate::pane_renderer::split_text_for_display;
use crate::pane_system::{
    chat_compact_button_bounds, chat_composer_height_for_value,
    chat_composer_input_bounds_with_height, chat_cycle_approval_mode_button_bounds,
    chat_cycle_collaboration_mode_button_bounds, chat_cycle_model_button_bounds,
    chat_cycle_personality_button_bounds, chat_cycle_reasoning_effort_button_bounds,
    chat_cycle_sandbox_mode_button_bounds, chat_cycle_service_tier_button_bounds,
    chat_help_toggle_button_bounds, chat_interrupt_button_bounds, chat_new_thread_button_bounds,
    chat_refresh_threads_button_bounds, chat_review_button_bounds, chat_send_button_bounds,
    chat_thread_action_archive_button_bounds, chat_thread_action_copy_button_bounds,
    chat_thread_action_fork_button_bounds, chat_thread_action_open_editor_button_bounds,
    chat_thread_action_reload_button_bounds, chat_thread_action_rename_button_bounds,
    chat_thread_action_rollback_button_bounds, chat_thread_action_unarchive_button_bounds,
    chat_thread_action_unsubscribe_button_bounds, chat_thread_filter_archived_button_bounds,
    chat_thread_filter_provider_button_bounds, chat_thread_filter_source_button_bounds,
    chat_thread_rail_bounds, chat_thread_rail_toggle_button_bounds, chat_thread_row_bounds,
    chat_thread_search_input_bounds, chat_transcript_body_bounds_with_height,
    chat_transcript_bounds, chat_visible_thread_row_count, chat_workspace_rail_bounds,
    chat_workspace_rail_toggle_button_bounds, pane_content_bounds, set_chat_shell_layout_state,
};
use wgpui::components::sections::TerminalStream;

const CHAT_TRANSCRIPT_LINE_HEIGHT: f32 = 14.0;
const CHAT_MARKDOWN_FONT_SIZE: f32 = 11.0;
const CHAT_MARKDOWN_MIN_WIDTH: f32 = 84.0;
const CHAT_PROGRESS_HEADER_LINE_HEIGHT: f32 = 12.0;
const CHAT_PROGRESS_ROW_LINE_HEIGHT: f32 = 12.0;
const CHAT_PROGRESS_BLOCK_GAP: f32 = 4.0;
const CHAT_ACTIVITY_HEADER_LINE_HEIGHT: f32 = 12.0;
const CHAT_ACTIVITY_ROW_LINE_HEIGHT: f32 = 12.0;
const CHAT_ACTIVITY_MAX_ROWS: usize = 14;
const CHAT_TERMINAL_LINE_HEIGHT: f32 = 12.0;
const CHAT_TERMINAL_MAX_VISIBLE_LINES: usize = 10;
const CHAT_WORKSPACE_AVATAR_SIZE: f32 = 36.0;
const CHAT_ATTACHMENT_CARD_GAP: f32 = 6.0;
const CHAT_ATTACHMENT_LABEL_LINE_HEIGHT: f32 = 10.0;
const CHAT_ATTACHMENT_SUMMARY_LINE_HEIGHT: f32 = 12.0;
const CHAT_ATTACHMENT_DETAIL_LINE_HEIGHT: f32 = 10.0;
const CHAT_SEND_ICON_SVG_RAW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640"><path fill="#FFFFFF" d="M342.6 73.4C330.1 60.9 309.8 60.9 297.3 73.4L137.3 233.4C124.8 245.9 124.8 266.2 137.3 278.7C149.8 291.2 170.1 291.2 182.6 278.7L288 173.3L288 544C288 561.7 302.3 576 320 576C337.7 576 352 561.7 352 544L352 173.3L457.4 278.7C469.9 291.2 490.2 291.2 502.7 278.7C515.2 266.2 515.2 245.9 502.7 233.4L342.7 73.4z"/></svg>"##;
const CHAT_MISSION_HEADER_HEIGHT: f32 = 26.0;

fn chat_mission_background_color() -> wgpui::Hsla {
    wgpui::Hsla::from_hex(0x070C14)
}

fn chat_mission_panel_color() -> wgpui::Hsla {
    wgpui::Hsla::from_hex(0x0D121A)
}

fn chat_mission_panel_header_color() -> wgpui::Hsla {
    wgpui::Hsla::from_hex(0x121924)
}

fn chat_mission_panel_border_color() -> wgpui::Hsla {
    wgpui::Hsla::from_hex(0x263245)
}

fn chat_mission_text_color() -> wgpui::Hsla {
    wgpui::Hsla::from_hex(0xD8DFF0)
}

fn chat_mission_muted_color() -> wgpui::Hsla {
    wgpui::Hsla::from_hex(0x8A909E)
}

fn chat_mission_green_color() -> wgpui::Hsla {
    wgpui::Hsla::from_hex(0x52E06D)
}

fn chat_mission_cyan_color() -> wgpui::Hsla {
    wgpui::Hsla::from_hex(0x2FB7F2)
}

fn chat_mission_orange_color() -> wgpui::Hsla {
    wgpui::Hsla::from_hex(0xFFA122)
}

fn paint_chat_mission_panel(
    bounds: Bounds,
    title: &str,
    accent: wgpui::Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(chat_mission_panel_color())
            .with_border(chat_mission_panel_border_color(), 1.0)
            .with_corner_radius(3.0),
    );
    let accent_rail_width = 6.0;
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 1.0,
            bounds.origin.y + 1.0,
            accent_rail_width,
            (bounds.size.height - 2.0).max(0.0),
        ))
        .with_background(accent.with_alpha(0.95))
        .with_corner_radius(2.0),
    );
    let header_bounds = Bounds::new(
        bounds.origin.x + accent_rail_width + 1.0,
        bounds.origin.y + 1.0,
        (bounds.size.width - accent_rail_width - 2.0).max(0.0),
        CHAT_MISSION_HEADER_HEIGHT,
    );
    paint.scene.draw_quad(
        Quad::new(header_bounds)
            .with_background(chat_mission_panel_header_color().with_alpha(0.95)),
    );
    // Mission Control-style accent framing: strong top accent plus subtle
    // right/bottom fades to make the shell feel energized without being noisy.
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + accent_rail_width + 1.0,
            bounds.origin.y + 1.0,
            (bounds.size.width - accent_rail_width - 2.0).max(0.0),
            1.0,
        ))
        .with_background(accent.with_alpha(0.35)),
    );
    let right_x = bounds.max_x() - 1.0;
    let right_h = (bounds.size.height - 2.0).max(0.0);
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            right_x,
            bounds.origin.y + 1.0,
            1.0,
            right_h * 0.48,
        ))
        .with_background(accent.with_alpha(0.12)),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            right_x,
            bounds.origin.y + 1.0 + right_h * 0.48,
            1.0,
            right_h * 0.52,
        ))
        .with_background(accent.with_alpha(0.26)),
    );
    let bottom_y = bounds.max_y() - 1.0;
    let bottom_w = (bounds.size.width - accent_rail_width - 2.0).max(0.0);
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + accent_rail_width + 1.0,
            bottom_y,
            bottom_w * 0.58,
            1.0,
        ))
        .with_background(accent.with_alpha(0.10)),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + accent_rail_width + 1.0 + bottom_w * 0.58,
            bottom_y,
            bottom_w * 0.42,
            1.0,
        ))
        .with_background(accent.with_alpha(0.22)),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(header_bounds.origin.x + 14.0, header_bounds.origin.y + 8.0),
        10.0,
        accent,
    ));
}

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
    badge_count: usize,
    badge_urgent: bool,
}

struct ChatShellChannelEntry {
    title: String,
    subtitle: String,
    active: bool,
    is_category: bool,
    collapsed: bool,
    badge_count: usize,
    badge_urgent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RichMessageAttachmentKind {
    Image,
    Video,
    Link,
    Lightning,
    Bitcoin,
    NostrReference,
    PaymentObject,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RichMessageAttachment {
    kind: RichMessageAttachmentKind,
    label: String,
    summary: String,
    detail: Option<String>,
}

fn notification_badge(unread_count: usize, mention_count: usize) -> Option<(usize, bool)> {
    if mention_count > 0 {
        Some((mention_count, true))
    } else if unread_count > 0 {
        Some((unread_count, false))
    } else {
        None
    }
}

fn notification_badge_label(count: usize) -> String {
    if count > 99 {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

fn paint_notification_badge(bounds: Bounds, count: usize, urgent: bool, paint: &mut PaintContext) {
    let label = notification_badge_label(count);
    let width = if label.len() >= 3 { 26.0 } else { 18.0 };
    let badge_bounds = Bounds::new(bounds.max_x() - width, bounds.origin.y, width, 16.0);
    let background = if urgent {
        theme::status::ERROR
    } else {
        theme::accent::PRIMARY
    };
    paint.scene.draw_quad(
        Quad::new(badge_bounds)
            .with_background(background)
            .with_border(background.with_alpha(0.9), 1.0)
            .with_corner_radius(8.0),
    );
    let text_x = if label.len() >= 3 {
        badge_bounds.origin.x + 4.0
    } else {
        badge_bounds.origin.x + 6.0
    };
    paint.scene.draw_text(paint.text.layout_mono(
        &label,
        Point::new(text_x, badge_bounds.origin.y + 4.0),
        9.0,
        theme::bg::APP,
    ));
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
            chat_mission_green_color().with_alpha(0.22),
            chat_mission_green_color().with_alpha(0.68),
            chat_mission_green_color(),
        )
    } else {
        (
            chat_mission_panel_color().with_alpha(0.9),
            chat_mission_panel_border_color().with_alpha(0.7),
            chat_mission_muted_color(),
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

fn truncate_for_width(text: &str, width: f32) -> String {
    let max_chars = ((width / 6.2).floor() as usize).max(6);
    let chunks = split_text_for_display(text, max_chars);
    let mut first = chunks.first().cloned().unwrap_or_default();
    if chunks.len() > 1 && !first.ends_with('…') {
        first.push('…');
    }
    first
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

fn rich_message_attachment_key(attachment: &RichMessageAttachment) -> String {
    format!(
        "{}|{}|{}|{}",
        rich_attachment_kind_label(&attachment.kind),
        attachment.label,
        attachment.summary,
        attachment.detail.as_deref().unwrap_or("")
    )
}

fn rich_attachment_kind_label(kind: &RichMessageAttachmentKind) -> &'static str {
    match kind {
        RichMessageAttachmentKind::Image => "image",
        RichMessageAttachmentKind::Video => "video",
        RichMessageAttachmentKind::Link => "link",
        RichMessageAttachmentKind::Lightning => "lightning",
        RichMessageAttachmentKind::Bitcoin => "bitcoin",
        RichMessageAttachmentKind::NostrReference => "nostr",
        RichMessageAttachmentKind::PaymentObject => "payment",
    }
}

fn rich_attachment_colors(
    kind: &RichMessageAttachmentKind,
) -> (wgpui::Hsla, wgpui::Hsla, wgpui::Hsla) {
    match kind {
        RichMessageAttachmentKind::Image => (
            theme::status::SUCCESS.with_alpha(0.12),
            theme::status::SUCCESS.with_alpha(0.32),
            theme::status::SUCCESS,
        ),
        RichMessageAttachmentKind::Video => (
            theme::status::INFO.with_alpha(0.12),
            theme::status::INFO.with_alpha(0.32),
            theme::status::INFO,
        ),
        RichMessageAttachmentKind::Link => (
            theme::accent::PRIMARY.with_alpha(0.12),
            theme::accent::PRIMARY.with_alpha(0.32),
            theme::accent::PRIMARY,
        ),
        RichMessageAttachmentKind::Lightning => (
            theme::status::WARNING.with_alpha(0.12),
            theme::status::WARNING.with_alpha(0.32),
            theme::status::WARNING,
        ),
        RichMessageAttachmentKind::Bitcoin => (
            theme::status::WARNING.with_alpha(0.12),
            theme::status::WARNING.with_alpha(0.32),
            theme::status::WARNING,
        ),
        RichMessageAttachmentKind::NostrReference => (
            theme::bg::SURFACE.with_alpha(0.42),
            theme::border::DEFAULT.with_alpha(0.42),
            theme::text::SECONDARY,
        ),
        RichMessageAttachmentKind::PaymentObject => (
            theme::accent::PRIMARY.with_alpha(0.12),
            theme::accent::PRIMARY.with_alpha(0.32),
            theme::accent::PRIMARY,
        ),
    }
}

fn compact_display_token(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        format!(
            "{}...",
            trimmed
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect::<String>()
        )
    }
}

fn trim_rich_token(token: &str) -> &str {
    token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
        )
    })
}

fn is_image_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".avif"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

fn is_video_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    [".mp4", ".mov", ".webm", ".m4v", ".ogg", ".3gp"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
        || lower.contains("youtube.com/")
        || lower.contains("youtu.be/")
        || lower.contains("vimeo.com/")
}

fn describe_url_attachment(token: &str) -> Option<RichMessageAttachment> {
    let parsed = Url::parse(token).ok()?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str().unwrap_or("link");
    let summary = if parsed.path().trim_matches('/').is_empty() {
        host.to_string()
    } else {
        compact_display_token(format!("{host}{}", parsed.path()).as_str(), 56)
    };
    let detail = Some(compact_display_token(token, 78));
    let kind = if is_image_url(token) {
        RichMessageAttachmentKind::Image
    } else if is_video_url(token) {
        RichMessageAttachmentKind::Video
    } else {
        RichMessageAttachmentKind::Link
    };
    let label = rich_attachment_kind_label(&kind).to_string();
    Some(RichMessageAttachment {
        kind,
        label,
        summary,
        detail,
    })
}

fn describe_lightning_attachment(token: &str) -> Option<RichMessageAttachment> {
    let trimmed = trim_rich_token(token);
    let normalized = trimmed
        .strip_prefix("lightning:")
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    if !normalized.starts_with("lnbc")
        && !normalized.starts_with("lntb")
        && !normalized.starts_with("lnbcrt")
        && !normalized.starts_with("lnurl1")
    {
        return None;
    }
    let summary = if normalized.starts_with("lnurl1") {
        "lnurl payment request".to_string()
    } else {
        "bolt11 invoice".to_string()
    };
    Some(RichMessageAttachment {
        kind: RichMessageAttachmentKind::Lightning,
        label: "lightning".to_string(),
        summary,
        detail: Some(compact_display_token(trimmed, 78)),
    })
}

fn describe_bitcoin_attachment(token: &str) -> Option<RichMessageAttachment> {
    let trimmed = trim_rich_token(token);
    let normalized = trimmed.to_ascii_lowercase();
    if !normalized.starts_with("bitcoin:") {
        return None;
    }
    let payload = trimmed.trim_start_matches("bitcoin:");
    let address = payload.split('?').next().unwrap_or(payload);
    let amount = payload
        .split('?')
        .nth(1)
        .and_then(|query| {
            query
                .split('&')
                .find_map(|part| part.strip_prefix("amount=").map(str::to_string))
        })
        .map(|amount| format!("amount={amount}"));
    Some(RichMessageAttachment {
        kind: RichMessageAttachmentKind::Bitcoin,
        label: "bitcoin".to_string(),
        summary: compact_display_token(address, 42),
        detail: amount.or_else(|| Some(compact_display_token(trimmed, 78))),
    })
}

fn describe_nostr_reference_attachment(token: &str) -> Option<RichMessageAttachment> {
    let trimmed = trim_rich_token(token);
    let normalized = trimmed.to_ascii_lowercase();
    let raw = normalized
        .strip_prefix("nostr:")
        .unwrap_or(normalized.as_str())
        .to_string();
    let label = if raw.starts_with("note1") || raw.starts_with("nevent1") {
        Some("note reference")
    } else if raw.starts_with("naddr1") {
        Some("address reference")
    } else if raw.starts_with("npub1") || raw.starts_with("nprofile1") {
        Some("profile reference")
    } else {
        None
    }?;
    Some(RichMessageAttachment {
        kind: RichMessageAttachmentKind::NostrReference,
        label: "nostr".to_string(),
        summary: label.to_string(),
        detail: Some(compact_display_token(trimmed, 78)),
    })
}

fn describe_payment_object_attachment(content: &str) -> Option<RichMessageAttachment> {
    let value = serde_json::from_str::<serde_json::Value>(content.trim()).ok()?;
    let object = value.as_object()?;
    let payment_request = ["payment_request", "invoice", "bolt11"]
        .into_iter()
        .find_map(|key| object.get(key).and_then(serde_json::Value::as_str))
        .map(str::to_string);
    let payment_id = ["payment_id", "id", "request_id"]
        .into_iter()
        .find_map(|key| object.get(key).and_then(serde_json::Value::as_str))
        .map(str::to_string);
    let status = ["status", "payment_status"]
        .into_iter()
        .find_map(|key| object.get(key).and_then(serde_json::Value::as_str))
        .map(str::to_string);
    let amount = ["amount_sats", "amount_sat", "amount"]
        .into_iter()
        .find_map(|key| object.get(key))
        .and_then(|value| {
            value
                .as_u64()
                .map(|value| format!("{value} sats"))
                .or_else(|| value.as_str().map(str::to_string))
        });
    if payment_request.is_none() && payment_id.is_none() && status.is_none() && amount.is_none() {
        return None;
    }

    let summary = if let Some(payment_request) = payment_request.as_deref() {
        if describe_lightning_attachment(payment_request).is_some() {
            "lightning payment object".to_string()
        } else if describe_bitcoin_attachment(payment_request).is_some() {
            "bitcoin payment object".to_string()
        } else {
            "wallet payment object".to_string()
        }
    } else {
        "wallet payment object".to_string()
    };
    let mut detail_parts = Vec::new();
    if let Some(amount) = amount {
        detail_parts.push(amount);
    }
    if let Some(status) = status {
        detail_parts.push(format!("chat-reported {status}"));
    }
    if let Some(payment_id) = payment_id {
        detail_parts.push(compact_display_token(payment_id.as_str(), 32));
    }
    if let Some(payment_request) = payment_request {
        detail_parts.push(compact_display_token(payment_request.as_str(), 48));
    }

    Some(RichMessageAttachment {
        kind: RichMessageAttachmentKind::PaymentObject,
        label: "payment".to_string(),
        summary,
        detail: (!detail_parts.is_empty()).then(|| detail_parts.join("  •  ")),
    })
}

fn rich_message_attachments(content: &str) -> Vec<RichMessageAttachment> {
    let mut attachments = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    if let Some(attachment) = describe_payment_object_attachment(content) {
        seen.insert(rich_message_attachment_key(&attachment));
        attachments.push(attachment);
    }

    for token in content.split_whitespace() {
        let normalized = trim_rich_token(token);
        if normalized.is_empty() {
            continue;
        }
        let attachment = describe_url_attachment(normalized)
            .or_else(|| describe_lightning_attachment(normalized))
            .or_else(|| describe_bitcoin_attachment(normalized))
            .or_else(|| describe_nostr_reference_attachment(normalized));
        let Some(attachment) = attachment else {
            continue;
        };
        let key = rich_message_attachment_key(&attachment);
        if seen.insert(key) {
            attachments.push(attachment);
        }
    }

    attachments
}

fn rich_message_attachments_height(content: &str) -> f32 {
    rich_message_attachments(content)
        .into_iter()
        .map(|attachment| {
            CHAT_ATTACHMENT_LABEL_LINE_HEIGHT
                + CHAT_ATTACHMENT_SUMMARY_LINE_HEIGHT
                + if attachment.detail.is_some() {
                    CHAT_ATTACHMENT_DETAIL_LINE_HEIGHT
                } else {
                    0.0
                }
                + 12.0
                + CHAT_ATTACHMENT_CARD_GAP
        })
        .sum()
}

fn paint_rich_message_attachments(
    content: &str,
    x: f32,
    mut y: f32,
    width: f32,
    paint: &mut PaintContext,
) -> f32 {
    let start_y = y;
    for attachment in rich_message_attachments(content) {
        let height = CHAT_ATTACHMENT_LABEL_LINE_HEIGHT
            + CHAT_ATTACHMENT_SUMMARY_LINE_HEIGHT
            + if attachment.detail.is_some() {
                CHAT_ATTACHMENT_DETAIL_LINE_HEIGHT
            } else {
                0.0
            }
            + 12.0;
        let bounds = Bounds::new(x, y, width.max(96.0), height);
        let (background, border, accent) = rich_attachment_colors(&attachment.kind);
        paint.scene.draw_quad(
            Quad::new(bounds)
                .with_background(background)
                .with_border(border, 1.0)
                .with_corner_radius(8.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &attachment.label,
            Point::new(bounds.origin.x + 8.0, bounds.origin.y + 6.0),
            9.0,
            accent,
        ));
        paint.scene.draw_text(paint.text.layout(
            &attachment.summary,
            Point::new(bounds.origin.x + 8.0, bounds.origin.y + 18.0),
            10.0,
            theme::text::PRIMARY,
        ));
        if let Some(detail) = attachment.detail.as_deref() {
            paint.scene.draw_text(paint.text.layout_mono(
                detail,
                Point::new(bounds.origin.x + 8.0, bounds.origin.y + 30.0),
                9.0,
                theme::text::MUTED,
            ));
        }
        y += height + CHAT_ATTACHMENT_CARD_GAP;
    }
    y - start_y
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
    if !autopilot_chat.pending_auth_refresh.is_empty() {
        pending_lines.push(format!(
            "pending auth refresh: {}",
            autopilot_chat.pending_auth_refresh.len()
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
            let overview_lines = managed_group_overview_lines(autopilot_chat);
            if !overview_lines.is_empty() {
                height += CHAT_ACTIVITY_HEADER_LINE_HEIGHT;
                height += CHAT_ACTIVITY_ROW_LINE_HEIGHT * overview_lines.len() as f32;
                height += 8.0;
            }
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
                height += rich_message_attachments_height(&markdown_source);
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
                height += rich_message_attachments_height(&markdown_source);
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
        height += rich_message_attachments_height(&markdown_source);
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

fn truncate_line(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let truncated = trimmed
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    format!("{truncated}…")
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
    if channel.mention_count > 0 {
        format!(
            "{} mention{}  •  {} unread",
            channel.mention_count,
            if channel.mention_count == 1 { "" } else { "s" },
            channel.unread_count
        )
    } else if channel.unread_count > 0 {
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
    let mut parts = Vec::new();
    if room.mention_count > 0 {
        parts.push(format!(
            "{} direct ping{}",
            room.mention_count,
            if room.mention_count == 1 { "" } else { "s" }
        ));
    } else if room.unread_count > 0 {
        parts.push(format!("{} unread", room.unread_count));
    }
    parts.push(format!("{} participant(s)", room.participant_pubkeys.len()));
    parts.push(format!("{} message(s)", room.message_ids.len()));
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

fn managed_group_membership_label(autopilot_chat: &AutopilotChatState) -> String {
    match autopilot_chat.active_managed_chat_local_member() {
        Some(member) if member.is_admin => "you are admin".to_string(),
        Some(member) if !member.labels.is_empty() => {
            format!("you are member ({})", member.labels.join(", "))
        }
        Some(_) => "you are member".to_string(),
        None => "you are outside the roster".to_string(),
    }
}

fn managed_group_policy_summary(group: &ManagedChatGroupProjection) -> String {
    let mut parts = vec![
        if group.metadata.private {
            "read: members-only".to_string()
        } else {
            "read: public".to_string()
        },
        if group.metadata.restricted {
            "write: restricted".to_string()
        } else {
            "write: open".to_string()
        },
        if group.metadata.hidden {
            "metadata: hidden".to_string()
        } else {
            "metadata: visible".to_string()
        },
        if group.metadata.closed {
            "join: closed".to_string()
        } else {
            "join: open".to_string()
        },
    ];
    if group.mention_count > 0 {
        parts.push(format!("{} mention(s)", group.mention_count));
    } else if group.unread_count > 0 {
        parts.push(format!("{} unread", group.unread_count));
    }
    parts.join("  •  ")
}

fn managed_group_role_summary(group: &ManagedChatGroupProjection) -> String {
    if group.roles.is_empty() {
        return "relay did not advertise group roles".to_string();
    }
    group
        .roles
        .iter()
        .take(4)
        .map(|role| match role.description.as_deref() {
            Some(description) if !description.trim().is_empty() => {
                format!("{} ({})", role.name, compact_shell_label(description))
            }
            _ => role.name.clone(),
        })
        .collect::<Vec<_>>()
        .join("  •  ")
}

fn managed_group_member_preview(autopilot_chat: &AutopilotChatState) -> String {
    let Some(group) = autopilot_chat.active_managed_chat_group() else {
        return String::new();
    };
    if group.members.is_empty() {
        return "relay has not exposed a member roster yet".to_string();
    }
    let mut preview = group
        .members
        .iter()
        .take(4)
        .map(|member| {
            let mut labels = Vec::new();
            if member.is_admin {
                labels.push("admin".to_string());
            }
            labels.extend(member.labels.iter().cloned());
            if autopilot_chat.managed_chat_member_is_locally_muted(&member.pubkey) {
                labels.push("muted-local".to_string());
            }
            if labels.is_empty() {
                compact_hex_label(&member.pubkey, 8)
            } else {
                format!(
                    "{} [{}]",
                    compact_hex_label(&member.pubkey, 8),
                    labels.join(", ")
                )
            }
        })
        .collect::<Vec<_>>();
    if group.members.len() > preview.len() {
        preview.push(format!("+{}", group.members.len() - preview.len()));
    }
    preview.join("  •  ")
}

fn managed_group_controls_summary(autopilot_chat: &AutopilotChatState) -> String {
    let membership = if autopilot_chat.active_managed_chat_local_is_admin() {
        "delete <#|id>, remove <pubkey>, invite <code>, meta key=value"
    } else if autopilot_chat.active_managed_chat_local_member().is_some() {
        "leave [reason]"
    } else {
        "join [invite] | [reason]"
    };
    format!("{membership}  •  mute/unmute <pubkey> stays local-only")
}

fn managed_group_overview_lines(autopilot_chat: &AutopilotChatState) -> Vec<String> {
    let Some(group) = autopilot_chat.active_managed_chat_group() else {
        return Vec::new();
    };
    vec![
        format!(
            "[server] {}  •  {}",
            managed_group_membership_label(autopilot_chat),
            managed_group_policy_summary(group)
        ),
        format!(
            "[roles] {} role(s)  •  {} admin(s)  •  {}",
            group.roles.len(),
            group
                .members
                .iter()
                .filter(|member| member.is_admin)
                .count(),
            managed_group_role_summary(group)
        ),
        format!(
            "[members] {} known  •  {}",
            group.members.len(),
            managed_group_member_preview(autopilot_chat)
        ),
        format!(
            "[controls] {}",
            managed_group_controls_summary(autopilot_chat)
        ),
    ]
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
    let wallet_hint = "Wallet: `wallet pay <#|id>`, `wallet request <#|id>`, `wallet copy-address <#|id>`, `wallet status <#|id>`.";
    let search_hint = "Search: `/search <text>` uses the optional Spacetime-derived index and falls back to canonical Nostr history.";
    let command_hint = if autopilot_chat.active_managed_chat_local_is_admin() {
        "Admin controls: `delete <#|id>`, `remove <pubkey>`, `invite <code>`, `meta key=value | ...`."
    } else if autopilot_chat.active_managed_chat_local_member().is_some() {
        "Member controls: `leave [reason]`. `mute/unmute <pubkey>` stays local-only."
    } else {
        "Access controls: `join [invite] | [reason]`. `mute/unmute <pubkey>` stays local-only."
    };
    if channel.relay_url.is_none() {
        return format!(
            "Channel relay target is unknown; publish waits for metadata or synced history. {wallet_hint} {search_hint} {command_hint}"
        );
    }
    if composer_value.trim().is_empty()
        && autopilot_chat
            .active_managed_chat_retryable_message()
            .is_some()
    {
        return format!(
            "Use `reply <#|id> <text>` or `react <#|id> <emoji>`. Empty composer retries the latest failed publish. {wallet_hint} {search_hint} {command_hint}"
        );
    }
    format!(
        "Use `reply <#|id> <text>` or `react <#|id> <emoji>`. `@hexprefix` adds mention tags. Shift+Enter inserts a newline. {wallet_hint} {search_hint} {command_hint}"
    )
}

fn direct_message_composer_hint(
    autopilot_chat: &AutopilotChatState,
    composer_value: &str,
) -> String {
    let wallet_hint = "Wallet: `wallet pay <#|id>`, `wallet request <#|id>`, `wallet copy-address <#|id>`, `wallet status <#|id>`.";
    let search_hint = "Search: `/search <text>` uses the optional Spacetime-derived index and falls back to canonical Nostr history.";
    if composer_value.trim().is_empty()
        && autopilot_chat
            .active_direct_message_retryable_message()
            .is_some()
    {
        return format!(
            "Use `reply <#|id> <text>`, `dm <pubkey> <text>`, or `room <pubkey[,pubkey...]> | <subject> | <text>`. Empty composer retries the latest failed DM publish. {wallet_hint} {search_hint}"
        );
    }
    format!(
        "Use plain text in the selected room, `reply <#|id> <text>`, `dm <pubkey> <text>`, or `room <pubkey[,pubkey...]> | <subject> | <text>`. {wallet_hint} {search_hint}"
    )
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

fn active_thread_subtitle(
    autopilot_chat: &AutopilotChatState,
    spacetime_presence: &crate::spacetime_presence::SpacetimePresenceSnapshot,
) -> String {
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
                    managed_group_membership_label(autopilot_chat),
                ];
                if let Some(local_delivery) = managed_local_delivery_summary(autopilot_chat) {
                    parts.push(local_delivery);
                }
                if let Some(presence) = crate::chat_spacetime::active_chat_presence_summary(
                    autopilot_chat,
                    spacetime_presence,
                ) {
                    parts.push(presence);
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
                if let Some(presence) = crate::chat_spacetime::active_chat_presence_summary(
                    autopilot_chat,
                    spacetime_presence,
                ) {
                    parts.push(presence);
                }
                return parts.join("  •  ");
            }
        }
        ChatBrowseMode::Autopilot => {}
    }

    let thread_id = autopilot_chat.active_thread_id.as_deref();
    let metadata = thread_id.and_then(|value| autopilot_chat.thread_metadata.get(value));
    let status = metadata
        .and_then(|metadata| metadata.status.as_ref())
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_else(|| autopilot_chat.connection_status.trim().to_ascii_lowercase());
    let loaded = metadata
        .map(|metadata| {
            if metadata.loaded {
                "loaded"
            } else {
                "detached"
            }
        })
        .unwrap_or("new");
    let updated = metadata
        .and_then(|metadata| metadata.updated_at)
        .and_then(format_thread_timestamp);
    let thread_label = thread_id
        .map(|value| compact_display_token(value, 18))
        .unwrap_or_else(|| "new-thread".to_string());
    let mut parts = vec![
        format!("thread:{}", thread_label),
        if status.is_empty() {
            "ready".to_string()
        } else {
            status
        },
        loaded.to_string(),
    ];
    if let Some(project_name) = metadata.and_then(|metadata| metadata.project_name.as_deref()) {
        parts.push(format!(
            "project:{}",
            compact_display_token(project_name, 18)
        ));
    }
    if let Some(workspace_root) = metadata.and_then(|metadata| metadata.workspace_root.as_deref()) {
        parts.push(format!("ws:{}", compact_display_token(workspace_root, 24)));
    }
    let git_state = git_state_summary(
        metadata.and_then(|metadata| metadata.git_branch.as_deref()),
        metadata.and_then(|metadata| metadata.git_dirty),
    );
    if git_state != "git:n/a" {
        parts.push(git_state);
    }
    if let Some(updated) = updated {
        parts.push(format!("updated:{updated}"));
    }
    parts.join("  •  ")
}

fn active_thread_preview_line(autopilot_chat: &AutopilotChatState) -> Option<String> {
    let preview = autopilot_chat
        .active_thread_preview()
        .map(str::trim)
        .filter(|preview| !preview.is_empty())?;
    Some(compact_display_token(preview, 84))
}

fn plan_step_status_label(status: &str) -> &'static str {
    match status {
        "completed" => "done",
        "inProgress" => "doing",
        _ => "next",
    }
}

fn active_plan_meta_line(artifact: &AutopilotPlanArtifact) -> String {
    let mut parts = vec![format!(
        "turn:{}",
        compact_display_token(artifact.source_turn_id.as_str(), 18)
    )];
    if let Some(project_name) = artifact.project_name.as_deref() {
        parts.push(format!(
            "project:{}",
            compact_display_token(project_name, 18)
        ));
    }
    if let Some(workspace_root) = artifact.workspace_root.as_deref() {
        parts.push(format!("ws:{}", compact_display_token(workspace_root, 28)));
    } else if let Some(workspace) = artifact.workspace_cwd.as_deref() {
        parts.push(format!(
            "workspace:{}",
            compact_display_token(workspace, 28)
        ));
    } else if let Some(path) = artifact.workspace_path.as_deref() {
        parts.push(format!("path:{}", compact_display_token(path, 28)));
    }
    let git_state = git_state_summary(artifact.git_branch.as_deref(), artifact.git_dirty);
    if git_state != "git:n/a" {
        parts.push(git_state);
    }
    if let Some(updated) = format_thread_timestamp(artifact.updated_at_epoch_ms as i64) {
        parts.push(format!("updated:{updated}"));
    }
    if artifact.restored_from_thread_read {
        parts.push("restored".to_string());
    }
    parts.join("  •  ")
}

fn active_plan_markdown_source(artifact: &AutopilotPlanArtifact) -> String {
    let mut lines = Vec::new();
    if let Some(explanation) = artifact.explanation.as_deref() {
        if !explanation.trim().is_empty() {
            lines.push(explanation.trim().to_string());
            lines.push(String::new());
        }
    }
    for step in artifact.steps.iter().take(6) {
        lines.push(format!(
            "- **{}** {}",
            plan_step_status_label(step.status.as_str()),
            step.step.trim()
        ));
    }
    if artifact.steps.len() > 6 {
        lines.push(format!("_{} more steps saved_", artifact.steps.len() - 6));
    }
    lines.join("\n")
}

fn active_diff_meta_line(artifact: &AutopilotDiffArtifact) -> String {
    let mut parts = vec![format!(
        "turn:{}",
        compact_display_token(artifact.source_turn_id.as_str(), 18)
    )];
    if let Some(project_name) = artifact.project_name.as_deref() {
        parts.push(format!(
            "project:{}",
            compact_display_token(project_name, 18)
        ));
    }
    if let Some(workspace_root) = artifact.workspace_root.as_deref() {
        parts.push(format!("ws:{}", compact_display_token(workspace_root, 24)));
    }
    parts.push(format!(
        "files:{} +{} -{}",
        artifact.files.len(),
        artifact.added_line_count,
        artifact.removed_line_count
    ));
    let git_state = git_state_summary(artifact.git_branch.as_deref(), artifact.git_dirty);
    if git_state != "git:n/a" {
        parts.push(git_state);
    }
    if let Some(updated) = format_thread_timestamp(artifact.updated_at_epoch_ms as i64) {
        parts.push(format!("updated:{updated}"));
    }
    parts.join("  •  ")
}

fn active_diff_markdown_source(artifact: &AutopilotDiffArtifact) -> String {
    let mut lines = Vec::new();
    for file in artifact.files.iter().take(6) {
        lines.push(format!(
            "- `{}` (+{} / -{})",
            file.path, file.added_line_count, file.removed_line_count
        ));
    }
    if artifact.files.len() > 6 {
        lines.push(format!("_{} more files changed_", artifact.files.len() - 6));
    }
    let diff_lines = artifact.raw_diff.lines().collect::<Vec<_>>();
    if !diff_lines.is_empty() {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push("```diff".to_string());
        for line in diff_lines.iter().take(24) {
            lines.push((*line).to_string());
        }
        if diff_lines.len() > 24 {
            lines.push("...".to_string());
        }
        lines.push("```".to_string());
    }
    lines.join("\n")
}

fn active_review_meta_line(artifact: &AutopilotReviewArtifact) -> String {
    let mut parts = vec![format!(
        "turn:{}",
        compact_display_token(artifact.source_turn_id.as_str(), 18)
    )];
    parts.push(format!("delivery:{}", artifact.delivery));
    parts.push(format!(
        "review:{}",
        compact_display_token(artifact.review_thread_id.as_str(), 18)
    ));
    parts.push(format!("status:{}", artifact.status));
    if let Some(updated) = format_thread_timestamp(artifact.updated_at_epoch_ms as i64) {
        parts.push(format!("updated:{updated}"));
    }
    if artifact.restored_from_thread_read {
        parts.push("restored".to_string());
    }
    parts.join("  •  ")
}

fn active_review_markdown_source(artifact: &AutopilotReviewArtifact) -> String {
    let mut lines = vec![format!("Target: {}", artifact.target)];
    if let Some(summary) = artifact.summary.as_deref() {
        let summary_lines = summary.lines().collect::<Vec<_>>();
        if !summary_lines.is_empty() {
            lines.push(String::new());
            for line in summary_lines.iter().take(16) {
                lines.push((*line).to_string());
            }
            if summary_lines.len() > 16 {
                lines.push(String::new());
                lines.push("_review output truncated_".to_string());
            }
        }
    } else {
        lines.push(String::new());
        lines.push("_review in progress_".to_string());
    }
    lines.join("\n")
}

fn active_compaction_meta_line(artifact: &AutopilotCompactionArtifact) -> String {
    let mut parts = vec![format!(
        "turn:{}",
        compact_display_token(artifact.source_turn_id.as_str(), 18)
    )];
    if let Some(updated) = format_thread_timestamp(artifact.updated_at_epoch_ms as i64) {
        parts.push(format!("updated:{updated}"));
    }
    if artifact.restored_from_thread_read {
        parts.push("restored".to_string());
    }
    parts.join("  •  ")
}

fn active_terminal_meta_line(session: &AutopilotTerminalSession) -> String {
    let mut parts = vec![format!("status:{}", session.status.label())];
    if let Some(pid) = session.pid {
        parts.push(format!("pid:{pid}"));
    }
    if !session.shell.trim().is_empty() {
        parts.push(format!(
            "shell:{}",
            compact_display_token(session.shell.as_str(), 18)
        ));
    }
    parts.push(format!("size:{}x{}", session.cols, session.rows));
    if !session.workspace_root.trim().is_empty() {
        parts.push(format!(
            "ws:{}",
            compact_display_token(session.workspace_root.as_str(), 24)
        ));
    }
    if let Some(updated) = format_thread_timestamp(session.updated_at_epoch_ms as i64) {
        parts.push(format!("updated:{updated}"));
    }
    if let Some(exit_code) = session.exit_code {
        parts.push(format!("exit:{exit_code}"));
    }
    parts.join("  •  ")
}

fn terminal_stream_color(stream: &TerminalStream) -> wgpui::Hsla {
    match stream {
        TerminalStream::Stdout => theme::text::PRIMARY,
        TerminalStream::Stderr => theme::status::ERROR,
    }
}

fn paint_active_terminal_session(
    session: &AutopilotTerminalSession,
    x: f32,
    mut y: f32,
    width: f32,
    paint: &mut PaintContext,
) -> f32 {
    paint.scene.draw_text(paint.text.layout_mono(
        "[thread terminal]",
        Point::new(x, y),
        10.0,
        theme::accent::PRIMARY,
    ));
    y += CHAT_PROGRESS_HEADER_LINE_HEIGHT;

    paint.scene.draw_text(paint.text.layout_mono(
        &active_terminal_meta_line(session),
        Point::new(x + 6.0, y),
        9.0,
        theme::text::MUTED,
    ));
    y += CHAT_ACTIVITY_ROW_LINE_HEIGHT;

    let line_count = session.lines.len().min(CHAT_TERMINAL_MAX_VISIBLE_LINES);
    let body_height = (line_count.max(1) as f32) * CHAT_TERMINAL_LINE_HEIGHT + 10.0;
    let body_bounds = Bounds::new(x + 6.0, y, (width - 12.0).max(60.0), body_height);
    paint.scene.draw_quad(
        Quad::new(body_bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(8.0),
    );

    let visible_lines = session
        .lines
        .iter()
        .rev()
        .take(CHAT_TERMINAL_MAX_VISIBLE_LINES)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    let mut line_y = body_bounds.origin.y + 6.0;
    if visible_lines.is_empty() {
        let placeholder = match session.status {
            crate::app_state::AutopilotTerminalSessionStatus::Pending => {
                "Terminal session is starting..."
            }
            crate::app_state::AutopilotTerminalSessionStatus::Running => "No terminal output yet.",
            _ => "Terminal session has no buffered output.",
        };
        paint.scene.draw_text(paint.text.layout_mono(
            placeholder,
            Point::new(body_bounds.origin.x + 8.0, line_y),
            9.0,
            theme::text::MUTED,
        ));
    } else {
        for line in &visible_lines {
            paint.scene.draw_text(paint.text.layout_mono(
                &truncate_line(line.text.as_str(), 120),
                Point::new(body_bounds.origin.x + 8.0, line_y),
                9.0,
                terminal_stream_color(&line.stream),
            ));
            line_y += CHAT_TERMINAL_LINE_HEIGHT;
        }
    }

    if session.lines.len() > CHAT_TERMINAL_MAX_VISIBLE_LINES {
        paint.scene.draw_text(paint.text.layout_mono(
            &format!(
                "... {} older terminal lines hidden",
                session.lines.len() - CHAT_TERMINAL_MAX_VISIBLE_LINES
            ),
            Point::new(body_bounds.origin.x + 8.0, body_bounds.max_y() - 12.0),
            9.0,
            theme::text::MUTED,
        ));
    }

    y = body_bounds.max_y() + 10.0;
    y
}

fn format_thread_timestamp(raw: i64) -> Option<String> {
    if raw <= 0 {
        return None;
    }
    let timestamp = if raw > 1_000_000_000_000 {
        chrono::DateTime::<chrono::Utc>::from_timestamp_millis(raw)?
    } else {
        chrono::DateTime::<chrono::Utc>::from_timestamp(raw, 0)?
    };
    Some(timestamp.format("%Y-%m-%d %H:%MZ").to_string())
}

fn auth_summary_label(account_summary: &str) -> String {
    let trimmed = account_summary.trim();
    if trimmed.is_empty() || matches!(trimmed, "unknown" | "none") {
        return "auth:unknown".to_string();
    }
    if trimmed == "apiKey" {
        return "auth:api-key".to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("chatgpt:") {
        let mut parts = rest.split(':');
        let email = parts.next().unwrap_or("chatgpt");
        let plan = parts.next().unwrap_or("chatgpt");
        return format!(
            "auth:{}:{}",
            compact_display_token(email, 16),
            plan.to_ascii_lowercase()
        );
    }
    format!("auth:{}", compact_display_token(trimmed, 22))
}

fn approval_mode_label(policy: codex_client::AskForApproval) -> &'static str {
    match policy {
        codex_client::AskForApproval::Never => "never",
        codex_client::AskForApproval::OnFailure => "on-failure",
        codex_client::AskForApproval::OnRequest => "on-request",
        codex_client::AskForApproval::UnlessTrusted => "untrusted",
        codex_client::AskForApproval::Reject { .. } => "reject",
    }
}

fn sandbox_mode_label(mode: codex_client::SandboxMode) -> &'static str {
    match mode {
        codex_client::SandboxMode::DangerFullAccess => "danger-full",
        codex_client::SandboxMode::WorkspaceWrite => "workspace-write",
        codex_client::SandboxMode::ReadOnly => "read-only",
    }
}

fn git_state_summary(branch: Option<&str>, dirty: Option<bool>) -> String {
    match (branch, dirty) {
        (Some(branch), Some(true)) => format!("git:{}/dirty", compact_display_token(branch, 14)),
        (Some(branch), Some(false)) => format!("git:{}/clean", compact_display_token(branch, 14)),
        (Some(branch), None) => format!("git:{}", compact_display_token(branch, 14)),
        (None, Some(true)) => "git:dirty".to_string(),
        (None, Some(false)) => "git:clean".to_string(),
        (None, None) => "git:n/a".to_string(),
    }
}

fn autopilot_status_lines(
    autopilot_chat: &AutopilotChatState,
    account_summary: &str,
) -> [String; 2] {
    let model = autopilot_chat
        .selected_model_override()
        .or_else(|| {
            autopilot_chat
                .active_thread_id
                .as_ref()
                .and_then(|thread_id| autopilot_chat.thread_metadata.get(thread_id))
                .and_then(|metadata| metadata.model.clone())
        })
        .unwrap_or_else(|| autopilot_chat.current_model().to_string());
    let effort = autopilot_chat.reasoning_effort.as_deref().unwrap_or("auto");
    let cwd = autopilot_chat
        .active_thread_cwd()
        .map(|value| compact_display_token(value, 28))
        .unwrap_or_else(|| "n/a".to_string());
    let workspace = autopilot_chat
        .active_thread_workspace_root()
        .map(|value| compact_display_token(value, 22))
        .unwrap_or_else(|| "n/a".to_string());
    let project = autopilot_chat
        .active_thread_project_name()
        .map(|value| compact_display_token(value, 14))
        .unwrap_or_else(|| "workspace".to_string());
    let git = git_state_summary(
        autopilot_chat.active_thread_git_branch(),
        autopilot_chat.active_thread_git_dirty(),
    );
    let permissions = format!(
        "{}/{}",
        approval_mode_label(autopilot_chat.approval_mode),
        sandbox_mode_label(autopilot_chat.sandbox_mode)
    );
    let token_usage = autopilot_chat
        .token_usage
        .as_ref()
        .map(|usage| {
            format!(
                "tok:{}+{}/{}",
                usage.input_tokens, usage.cached_input_tokens, usage.output_tokens
            )
        })
        .unwrap_or_else(|| "tok:n/a".to_string());
    [
        format!(
            "model:{}  effort:{}  tier:{}  mode:{}",
            compact_display_token(model.as_str(), 18),
            effort,
            autopilot_chat.service_tier.label(),
            autopilot_chat.collaboration_mode.label()
        ),
        format!(
            "proj:{}  root:{}  cwd:{}  {}  {}  perms:{}  {}",
            project,
            workspace,
            cwd,
            git,
            auth_summary_label(account_summary),
            permissions,
            token_usage
        ),
    ]
}

fn thread_filter_archived_label(autopilot_chat: &AutopilotChatState) -> &'static str {
    match autopilot_chat.thread_filter_archived {
        Some(false) => "Show active",
        Some(true) => "Show archived",
        None => "Show all",
    }
}

fn thread_filter_sort_label(autopilot_chat: &AutopilotChatState) -> &'static str {
    match autopilot_chat.thread_filter_sort_key {
        codex_client::ThreadSortKey::UpdatedAt => "Sort recent",
        codex_client::ThreadSortKey::CreatedAt => "Sort created",
    }
}

fn thread_rows_clip_bounds(
    content_bounds: Bounds,
    thread_tools_expanded: bool,
    channel_bounds: Bounds,
) -> Bounds {
    let first_row_y = chat_thread_row_bounds(content_bounds, 0, thread_tools_expanded)
        .origin
        .y;
    Bounds::new(
        channel_bounds.origin.x + 6.0,
        first_row_y,
        (channel_bounds.size.width - 12.0).max(0.0),
        (channel_bounds.max_y() - first_row_y - 6.0).max(0.0),
    )
}

fn autopilot_has_copyable_output(autopilot_chat: &AutopilotChatState) -> bool {
    autopilot_chat
        .messages
        .iter()
        .any(|message| message.role == AutopilotRole::Codex && !message.content.trim().is_empty())
}

fn paint_thread_rail_button(
    bounds: Bounds,
    label: &str,
    accent: wgpui::Hsla,
    active: bool,
    enabled: bool,
    paint: &mut PaintContext,
) {
    let background = if enabled {
        if active {
            accent.with_alpha(0.2)
        } else {
            chat_mission_panel_header_color().with_alpha(0.55)
        }
    } else {
        chat_mission_panel_color().with_alpha(0.85)
    };
    let border = if enabled {
        if active {
            accent.with_alpha(0.56)
        } else {
            chat_mission_panel_border_color().with_alpha(0.62)
        }
    } else {
        chat_mission_panel_border_color().with_alpha(0.35)
    };
    let text = if enabled {
        if active {
            chat_mission_text_color()
        } else {
            chat_mission_muted_color()
        }
    } else {
        chat_mission_muted_color().with_alpha(0.7)
    };
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(background)
            .with_border(border, 1.0)
            .with_corner_radius(3.0),
    );
    let clipped_label = truncate_for_width(label, bounds.size.width - 12.0);
    paint.scene.draw_text(paint.text.layout_mono(
        &clipped_label,
        Point::new(bounds.origin.x + 6.0, bounds.origin.y + 7.0),
        9.0,
        text,
    ));
}

fn paint_header_chip(bounds: Bounds, label: &str, accent: wgpui::Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(chat_mission_panel_header_color().with_alpha(0.58))
            .with_border(accent.with_alpha(0.6), 1.0)
            .with_corner_radius(3.0),
    );
    let clipped_label = truncate_for_width(label, bounds.size.width - 14.0);
    paint.scene.draw_text(paint.text.layout_mono(
        &clipped_label,
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 7.0),
        9.0,
        chat_mission_text_color(),
    ));
}

fn shell_workspaces(_autopilot_chat: &AutopilotChatState) -> Vec<ChatShellWorkspace> {
    vec![ChatShellWorkspace {
        label: "Private".to_string(),
        initials: "AG".to_string(),
        accent: theme::accent::PRIMARY,
        active: true,
        badge_count: 0,
        badge_urgent: false,
    }]
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
                        unread_count,
                        mention_count,
                        ..
                    } => Some(ChatShellChannelEntry {
                        title: format!("{} {}", if collapsed { "▸" } else { "▾" }, label),
                        subtitle: if unread_count > 0 {
                            format!("{channel_count} channel(s)  •  {unread_count} unread")
                        } else {
                            format!("{channel_count} channel(s)")
                        },
                        active: false,
                        is_category: true,
                        collapsed,
                        badge_count: notification_badge(unread_count, mention_count)
                            .map(|(count, _)| count)
                            .unwrap_or(0),
                        badge_urgent: notification_badge(unread_count, mention_count)
                            .map(|(_, urgent)| urgent)
                            .unwrap_or(false),
                    }),
                    crate::app_state::ManagedChatChannelRailRow::Channel { channel_id } => {
                        let channel = autopilot_chat
                            .managed_chat_projection
                            .snapshot
                            .channels
                            .iter()
                            .find(|channel| channel.channel_id == channel_id)?;
                        let (badge_count, badge_urgent) =
                            notification_badge(channel.unread_count, channel.mention_count)
                                .unwrap_or((0, false));
                        Some(ChatShellChannelEntry {
                            title: format!("# {}", managed_channel_label(channel)),
                            subtitle: managed_channel_subtitle(channel),
                            active: active_channel_id == Some(channel.channel_id.as_str()),
                            is_category: false,
                            collapsed: false,
                            badge_count,
                            badge_urgent,
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
                .map(|room| {
                    let (badge_count, badge_urgent) =
                        notification_badge(room.unread_count, room.mention_count)
                            .unwrap_or((0, false));
                    ChatShellChannelEntry {
                        title: direct_room_label(
                            room,
                            autopilot_chat.direct_message_projection.local_pubkey(),
                        ),
                        subtitle: direct_room_subtitle(room),
                        active: active_room_id == Some(room.room_id.as_str()),
                        is_category: false,
                        collapsed: false,
                        badge_count,
                        badge_urgent,
                    }
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
        badge_count: 0,
        badge_urgent: false,
    }];

    entries.extend(autopilot_chat.threads.iter().map(|thread_id| {
        let metadata = autopilot_chat.thread_metadata.get(thread_id);
        let title = metadata
            .and_then(|value| value.thread_name.as_ref())
            .map(|name| compact_shell_label(name))
            .unwrap_or_else(|| compact_shell_label(thread_id));
        let summary = metadata
            .and_then(|value| value.preview.as_deref())
            .map(str::trim)
            .filter(|preview| !preview.is_empty())
            .map(|preview| compact_display_token(preview, 28))
            .or_else(|| {
                metadata
                    .and_then(|value| value.status.as_ref())
                    .map(|status| status.trim().to_ascii_lowercase())
                    .filter(|status| !status.is_empty())
            })
            .unwrap_or_else(|| "thread".to_string());
        let subtitle = metadata
            .and_then(|value| value.project_name.as_deref())
            .map(|project| format!("{}  •  {}", compact_display_token(project, 12), summary))
            .unwrap_or(summary);
        ChatShellChannelEntry {
            title: format!("# {title}"),
            subtitle,
            active: autopilot_chat.active_thread_id.as_deref() == Some(thread_id.as_str()),
            is_category: false,
            collapsed: false,
            badge_count: 0,
            badge_urgent: false,
        }
    }));

    entries.push(ChatShellChannelEntry {
        title: "@ approvals".to_string(),
        subtitle: format!(
            "{} pending",
            autopilot_chat.pending_command_approvals.len()
                + autopilot_chat.pending_file_change_approvals.len()
                + autopilot_chat.pending_tool_calls.len()
                + autopilot_chat.pending_tool_user_input.len()
                + autopilot_chat.pending_auth_refresh.len()
        ),
        active: false,
        is_category: false,
        collapsed: false,
        badge_count: 0,
        badge_urgent: false,
    });
    entries
}

fn paint_chat_shell(
    content_bounds: Bounds,
    autopilot_chat: &AutopilotChatState,
    _codex_account_summary: &str,
    spacetime_presence: &crate::spacetime_presence::SpacetimePresenceSnapshot,
    paint: &mut PaintContext,
) {
    set_chat_shell_layout_state(
        autopilot_chat.workspace_rail_collapsed,
        autopilot_chat.thread_rail_collapsed,
    );
    let workspace_bounds = chat_workspace_rail_bounds(content_bounds);
    let channel_bounds = chat_thread_rail_bounds(content_bounds);
    let transcript_bounds = chat_transcript_bounds(content_bounds);
    let header_bounds = Bounds::new(
        transcript_bounds.origin.x + 8.0,
        transcript_bounds.origin.y + 8.0,
        (transcript_bounds.size.width - 16.0).max(0.0),
        106.0,
    );

    paint
        .scene
        .draw_quad(Quad::new(content_bounds).with_background(chat_mission_background_color()));
    paint_chat_mission_panel(
        workspace_bounds,
        if autopilot_chat.workspace_rail_collapsed {
            ""
        } else {
            "SPACES"
        },
        chat_mission_cyan_color(),
        paint,
    );
    if !autopilot_chat.workspace_rail_collapsed {
        paint.scene.draw_text(paint.text.layout_mono(
            "SELECT LANE",
            Point::new(
                workspace_bounds.origin.x + 12.0,
                workspace_bounds.origin.y + 46.0,
            ),
            9.0,
            chat_mission_muted_color(),
        ));
        for (index, workspace) in shell_workspaces(autopilot_chat).iter().enumerate() {
            let avatar_bounds = Bounds::new(
                workspace_bounds.origin.x
                    + (workspace_bounds.size.width - CHAT_WORKSPACE_AVATAR_SIZE) * 0.5,
                workspace_bounds.origin.y + 64.0 + index as f32 * 52.0,
                CHAT_WORKSPACE_AVATAR_SIZE,
                CHAT_WORKSPACE_AVATAR_SIZE,
            );
            let background = if workspace.active {
                workspace.accent
            } else {
                chat_mission_panel_header_color().with_alpha(0.75)
            };
            let text_color = if workspace.active {
                chat_mission_background_color()
            } else {
                chat_mission_text_color()
            };
            paint.scene.draw_quad(
                Quad::new(avatar_bounds)
                    .with_background(background)
                    .with_border(
                        if workspace.active {
                            workspace.accent
                        } else {
                            chat_mission_panel_border_color()
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
            if workspace.badge_count > 0 {
                paint_notification_badge(
                    Bounds::new(
                        avatar_bounds.max_x() - 10.0,
                        avatar_bounds.origin.y - 4.0,
                        26.0,
                        16.0,
                    ),
                    workspace.badge_count,
                    workspace.badge_urgent,
                    paint,
                );
            }
            paint.scene.draw_text(paint.text.layout(
                &workspace.label,
                Point::new(
                    workspace_bounds.origin.x + 12.0,
                    avatar_bounds.max_y() + 4.0,
                ),
                9.0,
                if workspace.active {
                    chat_mission_text_color()
                } else {
                    chat_mission_muted_color()
                },
            ));
        }
    }
    let workspace_toggle = chat_workspace_rail_toggle_button_bounds(content_bounds);
    paint.scene.draw_quad(
        Quad::new(workspace_toggle)
            .with_background(chat_mission_panel_header_color().with_alpha(0.75))
            .with_border(chat_mission_panel_border_color().with_alpha(0.8), 1.0)
            .with_corner_radius(2.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        if autopilot_chat.workspace_rail_collapsed {
            ">"
        } else {
            "<"
        },
        Point::new(
            workspace_toggle.origin.x + 3.0,
            workspace_toggle.origin.y + 2.0,
        ),
        9.0,
        chat_mission_cyan_color(),
    ));

    let (shell_mode_label, rail_title) = match autopilot_chat.chat_browse_mode() {
        ChatBrowseMode::Managed => ("OPENAGENTS / GROUP CHAT", "Channels"),
        ChatBrowseMode::DirectMessages => ("OPENAGENTS / DIRECT MESSAGES", "Rooms"),
        ChatBrowseMode::Autopilot => ("OPENAGENTS / AUTOPILOT", "THREADS"),
    };
    paint_chat_mission_panel(
        channel_bounds,
        if autopilot_chat.thread_rail_collapsed {
            ""
        } else {
            rail_title
        },
        chat_mission_green_color(),
        paint,
    );
    if !autopilot_chat.thread_rail_collapsed {
        paint.scene.draw_text(paint.text.layout_mono(
            shell_mode_label,
            Point::new(
                channel_bounds.origin.x + 16.0,
                channel_bounds.origin.y + 34.0,
            ),
            9.0,
            chat_mission_muted_color(),
        ));
    }
    if matches!(autopilot_chat.chat_browse_mode(), ChatBrowseMode::Autopilot)
        && !autopilot_chat.thread_rail_collapsed
    {
        let refresh_bounds = chat_refresh_threads_button_bounds(content_bounds);
        paint.scene.draw_quad(
            Quad::new(refresh_bounds)
                .with_background(chat_mission_panel_header_color().with_alpha(0.55))
                .with_border(chat_mission_panel_border_color().with_alpha(0.6), 1.0)
                .with_corner_radius(3.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            "↻",
            Point::new(refresh_bounds.origin.x + 6.0, refresh_bounds.origin.y + 3.0),
            11.0,
            chat_mission_cyan_color(),
        ));
        let new_thread_bounds = chat_new_thread_button_bounds(content_bounds);
        paint.scene.draw_quad(
            Quad::new(new_thread_bounds)
                .with_background(chat_mission_panel_header_color().with_alpha(0.55))
                .with_border(chat_mission_panel_border_color().with_alpha(0.6), 1.0)
                .with_corner_radius(3.0),
        );
        let plus_origin = Point::new(
            new_thread_bounds.origin.x
                + (new_thread_bounds.size.width - paint.text.measure("+", 14.0)) * 0.5,
            new_thread_bounds.origin.y + (new_thread_bounds.size.height - 14.0) * 0.5 - 2.0,
        );
        paint.scene.draw_text(
            paint
                .text
                .layout("+", plus_origin, 14.0, chat_mission_text_color()),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            "HISTORY",
            Point::new(
                chat_thread_search_input_bounds(content_bounds).origin.x + 2.0,
                chat_thread_search_input_bounds(content_bounds).origin.y - 10.0,
            ),
            9.0,
            chat_mission_muted_color(),
        ));
        paint_thread_rail_button(
            chat_thread_filter_archived_button_bounds(content_bounds),
            thread_filter_archived_label(autopilot_chat),
            theme::accent::PRIMARY,
            autopilot_chat.thread_filter_archived.is_some(),
            true,
            paint,
        );
        paint_thread_rail_button(
            chat_thread_filter_provider_button_bounds(content_bounds),
            if autopilot_chat.thread_tools_expanded {
                "Tools -"
            } else {
                "Tools +"
            },
            theme::status::SUCCESS,
            autopilot_chat.thread_tools_expanded,
            true,
            paint,
        );
        if autopilot_chat.thread_tools_expanded {
            paint_thread_rail_button(
                chat_thread_filter_source_button_bounds(content_bounds),
                thread_filter_sort_label(autopilot_chat),
                theme::status::INFO,
                autopilot_chat.thread_filter_sort_key == codex_client::ThreadSortKey::CreatedAt,
                true,
                paint,
            );
            let active_thread_status = autopilot_chat.active_thread_status().unwrap_or_default();
            let active_archived = active_thread_status.eq_ignore_ascii_case("archived");
            let has_active_thread = autopilot_chat.active_thread_id.is_some();
            paint_thread_rail_button(
                chat_thread_action_fork_button_bounds(content_bounds),
                "Fork",
                theme::accent::PRIMARY,
                false,
                has_active_thread,
                paint,
            );
            paint_thread_rail_button(
                chat_thread_action_archive_button_bounds(content_bounds),
                "Archive",
                theme::status::WARNING,
                false,
                has_active_thread && !active_archived,
                paint,
            );
            paint_thread_rail_button(
                chat_thread_action_unarchive_button_bounds(content_bounds),
                "Restore",
                theme::status::SUCCESS,
                active_archived,
                has_active_thread,
                paint,
            );
            paint_thread_rail_button(
                chat_thread_action_rename_button_bounds(content_bounds),
                "Rename",
                theme::accent::PRIMARY,
                false,
                has_active_thread,
                paint,
            );
            paint_thread_rail_button(
                chat_thread_action_reload_button_bounds(content_bounds),
                "Reload",
                theme::status::INFO,
                false,
                has_active_thread,
                paint,
            );
            paint_thread_rail_button(
                chat_thread_action_open_editor_button_bounds(content_bounds),
                "Open ws",
                theme::accent::PRIMARY,
                false,
                autopilot_chat.active_thread_workspace_root().is_some()
                    || autopilot_chat.active_thread_cwd().is_some(),
                paint,
            );
            paint_thread_rail_button(
                chat_thread_action_copy_button_bounds(content_bounds),
                "Copy",
                theme::status::SUCCESS,
                false,
                autopilot_has_copyable_output(autopilot_chat),
                paint,
            );
            paint_thread_rail_button(
                chat_thread_action_rollback_button_bounds(content_bounds),
                "Rollback",
                theme::status::ERROR,
                false,
                has_active_thread,
                paint,
            );
            paint_thread_rail_button(
                chat_thread_action_unsubscribe_button_bounds(content_bounds),
                "Unload",
                theme::text::MUTED,
                autopilot_chat.active_thread_loaded().unwrap_or(false),
                has_active_thread,
                paint,
            );
        }
    }
    if !autopilot_chat.thread_rail_collapsed {
        let channel_entries = shell_channel_entries(autopilot_chat);
        let total_rows = channel_entries.len();
        let visible_rows = chat_visible_thread_row_count(
            content_bounds,
            total_rows,
            autopilot_chat.thread_tools_expanded,
        );
        let start_index = autopilot_chat.thread_rail_scroll_start_index(total_rows, visible_rows);
        let max_start = total_rows.saturating_sub(visible_rows);
        let rows_clip = thread_rows_clip_bounds(
            content_bounds,
            autopilot_chat.thread_tools_expanded,
            channel_bounds,
        );
        paint.scene.push_clip(rows_clip);
        for (index, entry) in channel_entries
            .into_iter()
            .skip(start_index)
            .take(visible_rows)
            .enumerate()
        {
            let row_bounds =
                chat_thread_row_bounds(content_bounds, index, autopilot_chat.thread_tools_expanded);
            let background = if entry.is_category {
                chat_mission_panel_header_color().with_alpha(0.5)
            } else if entry.active {
                chat_mission_green_color().with_alpha(0.18)
            } else {
                chat_mission_panel_color().with_alpha(0.9)
            };
            let border = if entry.is_category {
                chat_mission_panel_border_color().with_alpha(0.45)
            } else if entry.active {
                chat_mission_green_color().with_alpha(0.6)
            } else {
                chat_mission_panel_border_color().with_alpha(0.6)
            };
            paint.scene.draw_quad(
                Quad::new(row_bounds)
                    .with_background(background)
                    .with_border(border, 1.0)
                    .with_corner_radius(3.0),
            );
            let title_color = if entry.is_category {
                chat_mission_muted_color()
            } else if entry.active {
                chat_mission_text_color()
            } else {
                chat_mission_muted_color()
            };
            paint.scene.draw_text(paint.text.layout(
                &entry.title,
                Point::new(row_bounds.origin.x + 12.0, row_bounds.origin.y + 6.0),
                11.0,
                title_color,
            ));
            paint.scene.draw_text(paint.text.layout_mono(
                &truncate_for_width(&entry.subtitle, row_bounds.size.width - 16.0),
                Point::new(row_bounds.origin.x + 12.0, row_bounds.origin.y + 18.0),
                9.0,
                if entry.is_category {
                    chat_mission_muted_color().with_alpha(0.85)
                } else {
                    chat_mission_muted_color()
                },
            ));
            if entry.badge_count > 0 {
                paint_notification_badge(
                    Bounds::new(
                        row_bounds.max_x() - 30.0,
                        row_bounds.origin.y + 7.0,
                        26.0,
                        16.0,
                    ),
                    entry.badge_count,
                    entry.badge_urgent,
                    paint,
                );
            }
        }
        paint.scene.pop_clip();
        if max_start > 0 && rows_clip.size.height > 0.0 {
            let track_bounds = Bounds::new(
                rows_clip.max_x() - 4.0,
                rows_clip.origin.y,
                3.0,
                rows_clip.size.height,
            );
            paint.scene.draw_quad(
                Quad::new(track_bounds)
                    .with_background(chat_mission_panel_border_color().with_alpha(0.35)),
            );
            let thumb_height = (rows_clip.size.height
                * (visible_rows as f32 / total_rows.max(1) as f32))
                .clamp(18.0, rows_clip.size.height);
            let progress = start_index as f32 / max_start.max(1) as f32;
            let thumb_y =
                rows_clip.origin.y + progress * (rows_clip.size.height - thumb_height).max(0.0);
            paint.scene.draw_quad(
                Quad::new(Bounds::new(
                    track_bounds.origin.x,
                    thumb_y,
                    track_bounds.size.width,
                    thumb_height,
                ))
                .with_background(chat_mission_panel_border_color().with_alpha(0.8)),
            );
        }
    }
    let thread_toggle = chat_thread_rail_toggle_button_bounds(content_bounds);
    paint.scene.draw_quad(
        Quad::new(thread_toggle)
            .with_background(chat_mission_panel_header_color().with_alpha(0.75))
            .with_border(chat_mission_panel_border_color().with_alpha(0.8), 1.0)
            .with_corner_radius(2.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        if autopilot_chat.thread_rail_collapsed {
            ">"
        } else {
            "<"
        },
        Point::new(thread_toggle.origin.x + 3.0, thread_toggle.origin.y + 2.0),
        9.0,
        chat_mission_green_color(),
    ));

    paint_chat_mission_panel(
        transcript_bounds,
        "CHAT",
        chat_mission_orange_color(),
        paint,
    );
    paint.scene.draw_quad(
        Quad::new(header_bounds)
            .with_background(chat_mission_panel_color().with_alpha(0.92))
            .with_border(chat_mission_panel_border_color().with_alpha(0.7), 1.0)
            .with_corner_radius(3.0),
    );
    paint.scene.draw_text(paint.text.layout(
        &active_thread_title(autopilot_chat),
        Point::new(header_bounds.origin.x + 16.0, header_bounds.origin.y + 10.0),
        16.0,
        chat_mission_text_color(),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        &truncate_for_width(
            &active_thread_subtitle(autopilot_chat, spacetime_presence),
            (header_bounds.size.width * 0.5).max(120.0),
        ),
        Point::new(header_bounds.origin.x + 16.0, header_bounds.origin.y + 28.0),
        10.0,
        chat_mission_muted_color(),
    ));
    if let Some(preview) = active_thread_preview_line(autopilot_chat) {
        paint.scene.draw_text(paint.text.layout(
            &preview,
            Point::new(header_bounds.origin.x + 16.0, header_bounds.origin.y + 44.0),
            10.0,
            chat_mission_muted_color(),
        ));
    }
    match autopilot_chat.chat_browse_mode() {
        ChatBrowseMode::Autopilot => {
            paint_header_chip(
                chat_cycle_model_button_bounds(content_bounds),
                format!(
                    "Model {}",
                    compact_display_token(autopilot_chat.current_model(), 16)
                )
                .as_str(),
                theme::accent::PRIMARY,
                paint,
            );
            paint_header_chip(
                chat_interrupt_button_bounds(content_bounds),
                if autopilot_chat.active_turn_id.is_some() {
                    "Interrupt turn"
                } else {
                    "Interrupt"
                },
                chat_mission_cyan_color(),
                paint,
            );
            paint_header_chip(
                chat_compact_button_bounds(content_bounds),
                if autopilot_chat.header_controls_expanded {
                    "Less"
                } else {
                    "More"
                },
                chat_mission_orange_color(),
                paint,
            );
            if autopilot_chat.header_controls_expanded {
                paint_header_chip(
                    chat_cycle_reasoning_effort_button_bounds(content_bounds),
                    format!(
                        "Effort {}",
                        autopilot_chat.reasoning_effort.as_deref().unwrap_or("auto")
                    )
                    .as_str(),
                    theme::status::INFO,
                    paint,
                );
                paint_header_chip(
                    chat_cycle_service_tier_button_bounds(content_bounds),
                    format!("Tier {}", autopilot_chat.service_tier.label()).as_str(),
                    chat_mission_green_color(),
                    paint,
                );
                paint_header_chip(
                    chat_cycle_personality_button_bounds(content_bounds),
                    format!("Tone {}", autopilot_chat.personality.label()).as_str(),
                    theme::status::WARNING,
                    paint,
                );
                paint_header_chip(
                    chat_cycle_collaboration_mode_button_bounds(content_bounds),
                    format!("Mode {}", autopilot_chat.collaboration_mode.label()).as_str(),
                    theme::accent::PRIMARY,
                    paint,
                );
                paint_header_chip(
                    chat_cycle_approval_mode_button_bounds(content_bounds),
                    format!(
                        "Approve {}",
                        approval_mode_label(autopilot_chat.approval_mode)
                    )
                    .as_str(),
                    theme::status::ERROR,
                    paint,
                );
                paint_header_chip(
                    chat_cycle_sandbox_mode_button_bounds(content_bounds),
                    format!(
                        "Sandbox {}",
                        sandbox_mode_label(autopilot_chat.sandbox_mode)
                    )
                    .as_str(),
                    theme::text::MUTED,
                    paint,
                );
                paint_header_chip(
                    chat_review_button_bounds(content_bounds),
                    if autopilot_chat.active_plan_artifact().is_some() {
                        if autopilot_chat.active_turn_id.is_some() {
                            "Steer plan"
                        } else {
                            "Implement plan"
                        }
                    } else {
                        "Review changes"
                    },
                    if autopilot_chat.active_plan_artifact().is_some() {
                        chat_mission_green_color()
                    } else {
                        theme::status::WARNING
                    },
                    paint,
                );
            }
        }
        ChatBrowseMode::Managed => {
            let status_text = managed_status_text(autopilot_chat);
            let status_width = (header_bounds.size.width * 0.45).max(150.0);
            let status_x = header_bounds.max_x() - status_width - 10.0;
            let mut status_y = header_bounds.origin.y + 12.0;
            let max_chars = ((status_width / 6.2).floor() as usize).max(12);
            for chunk in split_text_for_display(&status_text, max_chars)
                .into_iter()
                .take(3)
            {
                paint.scene.draw_text(paint.text.layout_mono(
                    &chunk,
                    Point::new(status_x, status_y),
                    9.0,
                    chat_mission_cyan_color(),
                ));
                status_y += 12.0;
            }
        }
        ChatBrowseMode::DirectMessages => {
            let status_text = direct_status_text(autopilot_chat);
            let status_width = (header_bounds.size.width * 0.45).max(150.0);
            let status_x = header_bounds.max_x() - status_width - 10.0;
            let mut status_y = header_bounds.origin.y + 12.0;
            let max_chars = ((status_width / 6.2).floor() as usize).max(12);
            for chunk in split_text_for_display(&status_text, max_chars)
                .into_iter()
                .take(3)
            {
                paint.scene.draw_text(paint.text.layout_mono(
                    &chunk,
                    Point::new(status_x, status_y),
                    9.0,
                    chat_mission_cyan_color(),
                ));
                status_y += 12.0;
            }
        }
    }
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
    codex_account_summary: &str,
    spacetime_presence: &crate::spacetime_presence::SpacetimePresenceSnapshot,
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
    paint_chat_shell(
        content_bounds,
        autopilot_chat,
        codex_account_summary,
        spacetime_presence,
        paint,
    );
    if browse_mode == ChatBrowseMode::Autopilot && !autopilot_chat.thread_rail_collapsed {
        let search_bounds = chat_thread_search_input_bounds(content_bounds);
        chat_inputs
            .thread_search
            .set_max_width(search_bounds.size.width);
        chat_inputs.thread_search.paint(search_bounds, paint);
    }

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
            let overview_lines = managed_group_overview_lines(autopilot_chat);
            if !overview_lines.is_empty() {
                paint.scene.draw_text(paint.text.layout_mono(
                    "[server state]",
                    Point::new(transcript_scroll_clip.origin.x, y),
                    10.0,
                    theme::accent::PRIMARY,
                ));
                y += CHAT_ACTIVITY_HEADER_LINE_HEIGHT;
                for line in overview_lines {
                    paint.scene.draw_text(paint.text.layout(
                        &line,
                        Point::new(transcript_scroll_clip.origin.x + 6.0, y),
                        10.0,
                        theme::text::MUTED,
                    ));
                    y += CHAT_ACTIVITY_ROW_LINE_HEIGHT;
                }
                y += 8.0;
            }
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
                y += paint_rich_message_attachments(
                    &markdown_source,
                    transcript_scroll_clip.origin.x,
                    y,
                    markdown_width,
                    paint,
                );

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
                y += paint_rich_message_attachments(
                    &markdown_source,
                    transcript_scroll_clip.origin.x,
                    y,
                    markdown_width,
                    paint,
                );

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
            if let Some(review_artifact) = autopilot_chat.active_review_artifact() {
                paint.scene.draw_text(paint.text.layout_mono(
                    "[latest review]",
                    Point::new(transcript_scroll_clip.origin.x, y),
                    10.0,
                    theme::status::WARNING,
                ));
                y += CHAT_PROGRESS_HEADER_LINE_HEIGHT;

                paint.scene.draw_text(paint.text.layout_mono(
                    &active_review_meta_line(review_artifact),
                    Point::new(transcript_scroll_clip.origin.x + 6.0, y),
                    9.0,
                    theme::text::MUTED,
                ));
                y += CHAT_ACTIVITY_ROW_LINE_HEIGHT;

                let review_markdown = active_review_markdown_source(review_artifact);
                if !review_markdown.trim().is_empty() {
                    let markdown_document = markdown_parser.parse(&review_markdown);
                    let markdown_height = markdown_renderer
                        .render(
                            &markdown_document,
                            Point::new(transcript_scroll_clip.origin.x + 6.0, y),
                            markdown_width,
                            paint.text,
                            paint.scene,
                        )
                        .height
                        .max(CHAT_TRANSCRIPT_LINE_HEIGHT);
                    y += markdown_height;
                }
                y += 10.0;
            }
            if let Some(diff_artifact) = autopilot_chat.active_diff_artifact() {
                paint.scene.draw_text(paint.text.layout_mono(
                    "[latest diff]",
                    Point::new(transcript_scroll_clip.origin.x, y),
                    10.0,
                    theme::status::INFO,
                ));
                y += CHAT_PROGRESS_HEADER_LINE_HEIGHT;

                paint.scene.draw_text(paint.text.layout_mono(
                    &active_diff_meta_line(diff_artifact),
                    Point::new(transcript_scroll_clip.origin.x + 6.0, y),
                    9.0,
                    theme::text::MUTED,
                ));
                y += CHAT_ACTIVITY_ROW_LINE_HEIGHT;

                let diff_markdown = active_diff_markdown_source(diff_artifact);
                if !diff_markdown.trim().is_empty() {
                    let markdown_document = markdown_parser.parse(&diff_markdown);
                    let markdown_height = markdown_renderer
                        .render(
                            &markdown_document,
                            Point::new(transcript_scroll_clip.origin.x + 6.0, y),
                            markdown_width,
                            paint.text,
                            paint.scene,
                        )
                        .height
                        .max(CHAT_TRANSCRIPT_LINE_HEIGHT);
                    y += markdown_height;
                }
                y += 10.0;
            }
            if let Some(plan_artifact) = autopilot_chat.active_plan_artifact() {
                paint.scene.draw_text(paint.text.layout_mono(
                    "[latest plan]",
                    Point::new(transcript_scroll_clip.origin.x, y),
                    10.0,
                    theme::status::SUCCESS,
                ));
                y += CHAT_PROGRESS_HEADER_LINE_HEIGHT;

                paint.scene.draw_text(paint.text.layout_mono(
                    &active_plan_meta_line(plan_artifact),
                    Point::new(transcript_scroll_clip.origin.x + 6.0, y),
                    9.0,
                    theme::text::MUTED,
                ));
                y += CHAT_ACTIVITY_ROW_LINE_HEIGHT;

                let plan_markdown = active_plan_markdown_source(plan_artifact);
                if !plan_markdown.trim().is_empty() {
                    let markdown_document = markdown_parser.parse(&plan_markdown);
                    let markdown_height = markdown_renderer
                        .render(
                            &markdown_document,
                            Point::new(transcript_scroll_clip.origin.x + 6.0, y),
                            markdown_width,
                            paint.text,
                            paint.scene,
                        )
                        .height
                        .max(CHAT_TRANSCRIPT_LINE_HEIGHT);
                    y += markdown_height;
                }
                y += 10.0;
            }
            if let Some(compaction_artifact) = autopilot_chat.active_compaction_artifact() {
                paint.scene.draw_text(paint.text.layout_mono(
                    "[latest compact]",
                    Point::new(transcript_scroll_clip.origin.x, y),
                    10.0,
                    theme::accent::PRIMARY,
                ));
                y += CHAT_PROGRESS_HEADER_LINE_HEIGHT;

                paint.scene.draw_text(paint.text.layout_mono(
                    &active_compaction_meta_line(compaction_artifact),
                    Point::new(transcript_scroll_clip.origin.x + 6.0, y),
                    9.0,
                    theme::text::MUTED,
                ));
                y += CHAT_ACTIVITY_ROW_LINE_HEIGHT + 10.0;
            }
            if let Some(terminal_session) = autopilot_chat.active_terminal_session() {
                y = paint_active_terminal_session(
                    terminal_session,
                    transcript_scroll_clip.origin.x,
                    y,
                    transcript_scroll_clip.size.width,
                    paint,
                );
            }
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
                y += paint_rich_message_attachments(
                    &markdown_source,
                    transcript_scroll_clip.origin.x,
                    y,
                    markdown_width,
                    paint,
                );
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
        if let Some(typing) = crate::chat_spacetime::active_chat_typing_summary(
            autopilot_chat,
            &composer_value,
            spacetime_presence,
        ) {
            paint.scene.draw_text(paint.text.layout_mono(
                &typing,
                Point::new(transcript_body_bounds.origin.x, footer_y),
                9.0,
                theme::accent::PRIMARY,
            ));
            footer_y -= CHAT_TRANSCRIPT_LINE_HEIGHT;
        }
        let hint_chunk_len = ((transcript_body_bounds.size.width / 6.2).floor() as usize).max(24);
        let hint_lines = split_text_for_display(&hint, hint_chunk_len);
        for line in hint_lines.iter().rev().take(2) {
            paint.scene.draw_text(paint.text.layout_mono(
                line,
                Point::new(transcript_body_bounds.origin.x, footer_y),
                9.0,
                theme::text::MUTED,
            ));
            footer_y -= CHAT_TRANSCRIPT_LINE_HEIGHT;
        }
    }
    if browse_mode == ChatBrowseMode::Autopilot {
        let help_bounds = chat_help_toggle_button_bounds(content_bounds);
        paint.scene.draw_quad(
            Quad::new(help_bounds)
                .with_background(chat_mission_panel_header_color().with_alpha(0.45))
                .with_border(
                    if autopilot_chat.show_autopilot_help_hint {
                        chat_mission_cyan_color().with_alpha(0.9)
                    } else {
                        chat_mission_panel_border_color().with_alpha(0.7)
                    },
                    1.0,
                )
                .with_corner_radius(8.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            "?",
            Point::new(help_bounds.origin.x + 5.0, help_bounds.origin.y + 3.0),
            10.0,
            if autopilot_chat.show_autopilot_help_hint {
                chat_mission_cyan_color()
            } else {
                chat_mission_muted_color()
            },
        ));

        if autopilot_chat.show_autopilot_help_hint {
            let hint = if autopilot_chat.active_turn_id.is_some() {
                "Use `/git ...`, `/pr prep`, `/term ...`, `/skills ...`, `/mcp ...`, `/apps ...`, `/requests`, `/approvals ...`, `/remote ...`, `/ps`, `/clean`, `/mention PATH`, or `/image PATH|URL`. Sending normal text while a turn runs steers the live task."
            } else {
                "Use `/git ...`, `/pr prep`, `/term ...`, `/skills ...`, `/mcp ...`, `/apps ...`, `/requests`, `/approvals ...`, `/remote ...`, `/ps`, `/clean`, `/mention PATH`, or `/image PATH|URL` for local coding workflow control."
            };
            let hint_chunk_len =
                ((transcript_body_bounds.size.width / 6.2).floor() as usize).max(24);
            let hint_lines = split_text_for_display(hint, hint_chunk_len);
            for line in hint_lines.iter().rev().take(3) {
                paint.scene.draw_text(paint.text.layout_mono(
                    line,
                    Point::new(transcript_body_bounds.origin.x, footer_y),
                    9.0,
                    theme::text::MUTED,
                ));
                footer_y -= CHAT_TRANSCRIPT_LINE_HEIGHT;
            }
        }
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
    paint.scene.draw_quad(
        Quad::new(composer_bounds)
            .with_background(chat_mission_panel_header_color().with_alpha(0.18))
            .with_border(chat_mission_cyan_color().with_alpha(0.85), 1.0)
            .with_corner_radius(3.0),
    );
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
    set_chat_shell_layout_state(
        state.autopilot_chat.workspace_rail_collapsed,
        state.autopilot_chat.thread_rail_collapsed,
    );
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
    let composer_before = state.chat_inputs.composer.get_value().to_string();
    let composer_value = state.chat_inputs.composer.get_value().to_string();
    let composer_height = chat_composer_height_for_value(content_bounds, &composer_value);
    let composer_bounds = chat_composer_input_bounds_with_height(content_bounds, composer_height);
    let mut handled = state
        .chat_inputs
        .composer
        .event(event, composer_bounds, &mut state.event_context)
        .is_handled();
    if state.autopilot_chat.chat_browse_mode() == ChatBrowseMode::Autopilot
        && !state.autopilot_chat.thread_rail_collapsed
    {
        handled |= state
            .chat_inputs
            .thread_search
            .event(
                event,
                chat_thread_search_input_bounds(content_bounds),
                &mut state.event_context,
            )
            .is_handled();
    }
    if handled
        && state.autopilot_chat.chat_browse_mode() == ChatBrowseMode::Autopilot
        && composer_before != state.chat_inputs.composer.get_value()
    {
        state
            .autopilot_chat
            .record_composer_draft(state.chat_inputs.composer.get_value().to_string());
    }
    handled
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
    if !state.autopilot_chat.thread_rail_collapsed {
        let channel_bounds = chat_thread_rail_bounds(content_bounds);
        let channel_rows = shell_channel_entries(&state.autopilot_chat);
        let visible_rows = chat_visible_thread_row_count(
            content_bounds,
            channel_rows.len(),
            state.autopilot_chat.thread_tools_expanded,
        );
        let rows_clip = thread_rows_clip_bounds(
            content_bounds,
            state.autopilot_chat.thread_tools_expanded,
            channel_bounds,
        );
        if rows_clip.contains(cursor_position)
            && state.autopilot_chat.scroll_thread_rail_by(
                scroll_dy,
                channel_rows.len(),
                visible_rows,
            )
        {
            return true;
        }
    }
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
        is_tool_activity_event, message_progress_height, progress_status_color,
        rich_message_attachments, sanitize_chat_text, wrap_transcript_text_lines,
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
    fn rich_message_parser_detects_links_media_payment_objects_and_refs() {
        let payment_attachments = rich_message_attachments(
            r#"{"payment_request":"lnbc1invoiceexample","amount_sats":1500,"status":"pending"}"#,
        );
        assert!(
            payment_attachments
                .iter()
                .any(|attachment| attachment.label == "payment")
        );
        assert!(payment_attachments.iter().any(|attachment| {
            attachment
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("chat-reported pending"))
        }));

        let attachments = rich_message_attachments(
            "https://cdn.example.com/cat.jpg https://youtu.be/demo note1deadbeef lnbc1invoiceexample bitcoin:bc1qexample?amount=0.001",
        );
        let labels = attachments
            .iter()
            .map(|attachment| attachment.label.as_str())
            .collect::<Vec<_>>();
        assert!(labels.contains(&"image"));
        assert!(labels.contains(&"video"));
        assert!(labels.contains(&"nostr"));
        assert!(labels.contains(&"lightning"));
        assert!(labels.contains(&"bitcoin"));
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
