use serde_json::Value;
use wgpui::{Bounds, Component, InputEvent, PaintContext, Point, Quad, theme};

use crate::app_state::{
    AutopilotChatState, AutopilotMessage, AutopilotRole, DataSellerPaneInputs, DataSellerPaneState,
    DataSellerShellSpeaker, PaneKind, RenderState,
};
use crate::pane_renderer::{paint_action_button, paint_label_line, paint_source_badge};
use crate::pane_system::{
    data_seller_composer_input_bounds, data_seller_confirm_button_bounds,
    data_seller_preview_button_bounds, data_seller_publish_button_bounds,
    data_seller_send_button_bounds, pane_content_bounds,
};

const PADDING: f32 = 12.0;
const HEADER_BOTTOM: f32 = 156.0;
const COLUMN_GAP: f32 = 12.0;
const CARD_RADIUS: f32 = 8.0;
const CARD_HEADER_HEIGHT: f32 = 36.0;
const TRANSCRIPT_ROW_HEIGHT: f32 = 72.0;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &DataSellerPaneState,
    autopilot_chat: &AutopilotChatState,
    inputs: &mut DataSellerPaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "codex.data_seller.v0", paint);
    paint_action_button(data_seller_send_button_bounds(content_bounds), "Send", paint);
    paint_action_button(
        data_seller_preview_button_bounds(content_bounds),
        "Preview Draft",
        paint,
    );
    paint_action_button(
        data_seller_confirm_button_bounds(content_bounds),
        "Confirm Preview",
        paint,
    );
    paint_action_button(
        data_seller_publish_button_bounds(content_bounds),
        "Publish",
        paint,
    );

    paint.scene.draw_text(paint.text.layout(
        "Conversational authoring surface for truthful data listings. Seller turns now run on the dedicated Codex thread and the typed Data Market tools update the structured draft.",
        Point::new(
            content_bounds.origin.x + PADDING,
            content_bounds.origin.y + 42.0,
        ),
        11.0,
        theme::text::SECONDARY,
    ));

    let mut status_y = content_bounds.origin.y + 60.0;
    status_y = paint_label_line(
        paint,
        content_bounds.origin.x + PADDING,
        status_y,
        "status",
        pane_state.load_state.label(),
    );
    status_y = paint_label_line(
        paint,
        content_bounds.origin.x + PADDING,
        status_y,
        "preview",
        gate_label(pane_state.preview_enabled),
    );
    status_y = paint_label_line(
        paint,
        content_bounds.origin.x + PADDING,
        status_y,
        "confirm",
        gate_label(pane_state.confirm_enabled),
    );
    status_y = paint_label_line(
        paint,
        content_bounds.origin.x + PADDING,
        status_y,
        "publish",
        gate_label(pane_state.publish_enabled),
    );
    status_y = paint_label_line(
        paint,
        content_bounds.origin.x + PADDING,
        status_y,
        "codex session",
        pane_state.codex_session_phase.label(),
    );
    let _ = paint_label_line(
        paint,
        content_bounds.origin.x + PADDING,
        status_y,
        "status line",
        &pane_state.status_line,
    );

    if let Some(action) = pane_state.last_action.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            action,
            Point::new(
                content_bounds.origin.x + PADDING,
                content_bounds.origin.y + 114.0,
            ),
            11.0,
            theme::text::SECONDARY,
        ));
    }
    if let Some(error) = pane_state.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(
                content_bounds.origin.x + PADDING,
                content_bounds.origin.y + 132.0,
            ),
            11.0,
            theme::status::ERROR,
        ));
    }

    let composer_bounds = data_seller_composer_input_bounds(content_bounds);
    inputs
        .composer
        .set_max_width(composer_bounds.size.width.max(140.0));
    inputs.composer.paint(composer_bounds, paint);
    paint.scene.draw_text(paint.text.layout(
        "Seller prompt",
        Point::new(composer_bounds.origin.x, composer_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));

    let available_height =
        (composer_bounds.origin.y - HEADER_BOTTOM - COLUMN_GAP - PADDING).max(220.0);
    let transcript_width =
        ((content_bounds.size.width - PADDING * 2.0 - COLUMN_GAP) * 0.57).max(360.0);
    let side_width =
        (content_bounds.size.width - PADDING * 2.0 - COLUMN_GAP - transcript_width).max(260.0);
    let transcript_bounds = Bounds::new(
        content_bounds.origin.x + PADDING,
        content_bounds.origin.y + HEADER_BOTTOM,
        transcript_width,
        available_height,
    );
    let draft_bounds = Bounds::new(
        transcript_bounds.max_x() + COLUMN_GAP,
        transcript_bounds.origin.y,
        side_width,
        available_height * 0.38,
    );
    let preview_bounds = Bounds::new(
        draft_bounds.origin.x,
        draft_bounds.max_y() + COLUMN_GAP,
        side_width,
        available_height * 0.30,
    );
    let status_bounds = Bounds::new(
        draft_bounds.origin.x,
        preview_bounds.max_y() + COLUMN_GAP,
        side_width,
        (transcript_bounds.max_y() - preview_bounds.max_y() - COLUMN_GAP).max(150.0),
    );

    paint_transcript_shell(transcript_bounds, pane_state, autopilot_chat, paint);
    paint_draft_shell_card(draft_bounds, pane_state, paint);
    paint_asset_preview_card(preview_bounds, pane_state, paint);
    paint_publication_status_card(status_bounds, pane_state, paint);
}

fn paint_transcript_shell(
    bounds: Bounds,
    pane_state: &DataSellerPaneState,
    autopilot_chat: &AutopilotChatState,
    paint: &mut PaintContext,
) {
    let thread_messages = pane_state
        .codex_thread_id
        .as_deref()
        .and_then(|thread_id| autopilot_chat.cached_thread_messages(thread_id));
    let has_thread_transcript = thread_messages.is_some_and(|messages| !messages.is_empty());
    paint_card(
        bounds,
        "Transcript",
        if has_thread_transcript {
            "Dedicated seller Codex lane"
        } else {
            "Dedicated seller lane"
        },
        paint,
    );

    let mut row_y = bounds.origin.y + CARD_HEADER_HEIGHT + 12.0;
    if let Some(messages) = thread_messages {
        for message in messages.iter().rev().take(4).rev() {
            paint_transcript_row(
                Bounds::new(
                    bounds.origin.x + 10.0,
                    row_y,
                    bounds.size.width - 20.0,
                    TRANSCRIPT_ROW_HEIGHT,
                ),
                transcript_role_label(message),
                transcript_role_color(message.role),
                &message.content,
                paint,
            );
            row_y += TRANSCRIPT_ROW_HEIGHT + 10.0;
        }
    } else {
        for message in pane_state.transcript_shell.iter().take(4) {
            paint_transcript_row(
                Bounds::new(
                    bounds.origin.x + 10.0,
                    row_y,
                    bounds.size.width - 20.0,
                    TRANSCRIPT_ROW_HEIGHT,
                ),
                message.speaker.label(),
                speaker_color(message.speaker),
                &message.content,
                paint,
            );
            row_y += TRANSCRIPT_ROW_HEIGHT + 10.0;
        }
    }

    paint.scene.draw_text(paint.text.layout(
        if has_thread_transcript {
            "Seller prompts now run on the dedicated Codex thread. Typed tools update the draft while the thread asks bounded follow-up questions."
        } else {
            "Send a seller prompt to start the dedicated Codex conversation and draft-normalization loop."
        },
        Point::new(bounds.origin.x + 10.0, bounds.max_y() - 18.0),
        10.0,
        theme::text::MUTED,
    ));
}

fn paint_transcript_row(
    row_bounds: Bounds,
    label: &str,
    label_color: wgpui::Hsla,
    content: &str,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(row_bounds)
            .with_background(theme::bg::HOVER)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(6.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 10.0),
        10.0,
        label_color,
    ));
    paint.scene.draw_text(paint.text.layout(
        content,
        Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 28.0),
        11.0,
        theme::text::PRIMARY,
    ));
}

fn paint_draft_shell_card(
    bounds: Bounds,
    pane_state: &DataSellerPaneState,
    paint: &mut PaintContext,
) {
    paint_card(bounds, "Draft card", "Structured local truth object", paint);

    let mut row_y = bounds.origin.y + CARD_HEADER_HEIGHT + 12.0;
    let title = option_label(pane_state.active_draft.title.as_deref());
    row_y = paint_label_line(paint, bounds.origin.x + 10.0, row_y, "title", &title);
    let asset_kind = option_label(pane_state.active_draft.asset_kind.as_deref());
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "asset kind",
        &asset_kind,
    );
    let price_hint = pane_state.active_draft.price_hint_label();
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "price hint",
        &price_hint,
    );
    let policy = option_label(pane_state.active_draft.default_policy.as_deref());
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "policy posture",
        &policy,
    );
    let provenance = option_label(pane_state.active_draft.provenance_ref.as_deref());
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "provenance",
        &provenance,
    );

    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "preview posture",
        pane_state.active_draft.preview_posture.label(),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "blockers",
        &pane_state.active_draft.blocker_summary(),
    );

    paint.scene.draw_text(paint.text.layout(
        &pane_state.status_line,
        Point::new(bounds.origin.x + 10.0, row_y + 10.0),
        11.0,
        theme::text::SECONDARY,
    ));
    if let Some(first_blocker) = pane_state.active_draft.readiness_blockers.first() {
        paint.scene.draw_text(paint.text.layout(
            &format!("next blocker: {}", first_blocker.message),
            Point::new(bounds.origin.x + 10.0, row_y + 28.0),
            10.0,
            theme::status::WARNING,
        ));
    }
    paint.scene.draw_text(paint.text.layout(
        "The draft is now expected to evolve from the seller conversation plus typed Data Market tool calls, not from transcript prose alone.",
        Point::new(bounds.origin.x + 10.0, bounds.max_y() - 18.0),
        10.0,
        theme::text::MUTED,
    ));
}

fn paint_publication_status_card(
    bounds: Bounds,
    pane_state: &DataSellerPaneState,
    paint: &mut PaintContext,
) {
    paint_card(
        bounds,
        "Publication status",
        "Truth boundary reminders",
        paint,
    );

    let mut row_y = bounds.origin.y + CARD_HEADER_HEIGHT + 12.0;
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "preview control",
        gate_label(pane_state.preview_enabled),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "confirm control",
        gate_label(pane_state.confirm_enabled),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "publish control",
        gate_label(pane_state.publish_enabled),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "preview posture",
        pane_state.active_draft.preview_posture.label(),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "confirmed",
        bool_label(pane_state.asset_preview_confirmed),
    );
    let thread_id = option_label(pane_state.codex_thread_id.as_deref());
    row_y = paint_label_line(paint, bounds.origin.x + 10.0, row_y, "thread", &thread_id);
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "personality",
        pane_state.codex_profile.personality.label(),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "collab",
        pane_state.codex_profile.collaboration_mode.label(),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "required skills",
        &pane_state.required_skill_summary(),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "authority truth",
        "kernel DataAsset / AccessGrant",
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "read-back surface",
        "Data Market pane",
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "published asset",
        &option_label(
            pane_state
                .last_published_asset
                .as_ref()
                .map(|asset| asset.asset_id.as_str()),
        ),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "publish receipt",
        &option_label(pane_state.last_publish_receipt_id.as_deref()),
    );

    paint.scene.draw_text(paint.text.layout(
        &format!(
            "Exact asset preview: {}",
            if pane_state.active_draft.last_previewed_asset_payload.is_some() {
                "ready"
            } else {
                "not ready"
            }
        ),
        Point::new(bounds.origin.x + 10.0, row_y + 8.0),
        11.0,
        theme::text::SECONDARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        &format!(
            "confirmed preview: {}",
            if pane_state.last_confirmed_asset_payload.is_some() {
                "ready"
            } else {
                "not confirmed"
            }
        ),
        Point::new(bounds.origin.x + 10.0, row_y + 26.0),
        11.0,
        theme::text::SECONDARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        &format!(
            "session origin: {}",
            pane_state.codex_profile.session_origin
        ),
        Point::new(bounds.origin.x + 10.0, row_y + 44.0),
        11.0,
        theme::text::SECONDARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "This pane is intentionally allowed to express intent before it is allowed to mutate authority state.",
        Point::new(bounds.origin.x + 10.0, row_y + 62.0),
        11.0,
        theme::text::SECONDARY,
    ));
}

fn paint_asset_preview_card(bounds: Bounds, pane_state: &DataSellerPaneState, paint: &mut PaintContext) {
    paint_card(
        bounds,
        "Exact asset preview",
        "RegisterDataAssetRequest",
        paint,
    );

    let payload = pane_state.active_draft.last_previewed_asset_payload.as_ref();
    let mut row_y = bounds.origin.y + CARD_HEADER_HEIGHT + 12.0;
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "preview",
        if payload.is_some() { "ready" } else { "pending" },
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "confirmed",
        bool_label(pane_state.asset_preview_confirmed),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "asset id",
        &preview_field(payload, &["asset", "asset_id"]),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "provider",
        &preview_field(payload, &["asset", "provider_id"]),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "policy bundle",
        &preview_field(payload, &["policy", "policy_bundle_id"]),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "idempotency key",
        &preview_field(payload, &["idempotency_key"]),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "evidence refs",
        &preview_array_len(payload, &["evidence"]),
    );

    paint.scene.draw_text(paint.text.layout(
        if payload.is_some() {
            "Preview shows the exact authority request shape that the later publish path will submit."
        } else {
            "Run preview after resolving blockers to materialize the exact authority payload."
        },
        Point::new(bounds.origin.x + 10.0, row_y + 10.0),
        10.0,
        theme::text::MUTED,
    ));
}

fn paint_card(bounds: Bounds, title: &str, subtitle: &str, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(CARD_RADIUS),
    );
    paint.scene.draw_text(paint.text.layout(
        title,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 8.0),
        11.0,
        theme::text::SECONDARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        subtitle,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 22.0),
        10.0,
        theme::text::MUTED,
    ));
}

fn gate_label(enabled: bool) -> &'static str {
    if enabled { "armed" } else { "blocked" }
}

fn option_label(value: Option<&str>) -> String {
    value
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "pending".to_string())
}

fn bool_label(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn preview_field(payload: Option<&Value>, path: &[&str]) -> String {
    let mut current = payload;
    for segment in path {
        current = current.and_then(|value| value.get(*segment));
    }
    current
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "pending".to_string())
}

fn preview_array_len(payload: Option<&Value>, path: &[&str]) -> String {
    let mut current = payload;
    for segment in path {
        current = current.and_then(|value| value.get(*segment));
    }
    current
        .and_then(Value::as_array)
        .map(|entries| entries.len().to_string())
        .unwrap_or_else(|| "0".to_string())
}

fn speaker_color(speaker: DataSellerShellSpeaker) -> wgpui::Hsla {
    match speaker {
        DataSellerShellSpeaker::System => theme::text::MUTED,
        DataSellerShellSpeaker::SellerAgent => theme::status::INFO,
        DataSellerShellSpeaker::Seller => theme::status::SUCCESS,
    }
}

fn transcript_role_label(message: &AutopilotMessage) -> &'static str {
    match message.role {
        AutopilotRole::User => "seller",
        AutopilotRole::Codex => "seller agent",
    }
}

fn transcript_role_color(role: AutopilotRole) -> wgpui::Hsla {
    match role {
        AutopilotRole::User => theme::status::SUCCESS,
        AutopilotRole::Codex => theme::status::INFO,
    }
}

pub fn dispatch_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_pane = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::DataSeller)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_pane else {
        return false;
    };
    let input_bounds = data_seller_composer_input_bounds(pane_content_bounds(bounds));
    state
        .data_seller_inputs
        .composer
        .event(event, input_bounds, &mut state.event_context)
        .is_handled()
}
