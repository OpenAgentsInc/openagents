use wgpui::{Bounds, PaintContext, Point, Quad, theme};

use crate::app_state::{DataSellerPaneState, DataSellerShellSpeaker};
use crate::pane_renderer::{paint_action_button, paint_label_line, paint_source_badge};
use crate::pane_system::{data_seller_preview_button_bounds, data_seller_publish_button_bounds};

const PADDING: f32 = 12.0;
const HEADER_BOTTOM: f32 = 156.0;
const COLUMN_GAP: f32 = 12.0;
const CARD_RADIUS: f32 = 8.0;
const CARD_HEADER_HEIGHT: f32 = 36.0;
const TRANSCRIPT_ROW_HEIGHT: f32 = 72.0;

pub fn paint(content_bounds: Bounds, pane_state: &DataSellerPaneState, paint: &mut PaintContext) {
    paint_source_badge(content_bounds, "codex.data_seller.v0", paint);
    paint_action_button(
        data_seller_preview_button_bounds(content_bounds),
        "Preview Draft",
        paint,
    );
    paint_action_button(
        data_seller_publish_button_bounds(content_bounds),
        "Publish",
        paint,
    );

    paint.scene.draw_text(paint.text.layout(
        "Conversational authoring surface for truthful data listings. The pane now keeps a structured draft locally while the seller-specific Codex session and typed publish tools land next.",
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

    let available_height = (content_bounds.size.height - HEADER_BOTTOM - PADDING).max(220.0);
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
        available_height * 0.52,
    );
    let status_bounds = Bounds::new(
        draft_bounds.origin.x,
        draft_bounds.max_y() + COLUMN_GAP,
        side_width,
        (transcript_bounds.max_y() - draft_bounds.max_y() - COLUMN_GAP).max(150.0),
    );

    paint_transcript_shell(transcript_bounds, pane_state, paint);
    paint_draft_shell_card(draft_bounds, pane_state, paint);
    paint_publication_status_card(status_bounds, pane_state, paint);
}

fn paint_transcript_shell(
    bounds: Bounds,
    pane_state: &DataSellerPaneState,
    paint: &mut PaintContext,
) {
    paint_card(bounds, "Transcript shell", "Dedicated seller lane", paint);

    let mut row_y = bounds.origin.y + CARD_HEADER_HEIGHT + 12.0;
    for message in pane_state.transcript_shell.iter().take(4) {
        let row_bounds = Bounds::new(
            bounds.origin.x + 10.0,
            row_y,
            bounds.size.width - 20.0,
            TRANSCRIPT_ROW_HEIGHT,
        );
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(theme::bg::HOVER)
                .with_border(theme::border::DEFAULT, 1.0)
                .with_corner_radius(6.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            message.speaker.label(),
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 10.0),
            10.0,
            speaker_color(message.speaker),
        ));
        paint.scene.draw_text(paint.text.layout(
            &message.content,
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 28.0),
            11.0,
            theme::text::PRIMARY,
        ));
        row_y += TRANSCRIPT_ROW_HEIGHT + 10.0;
    }

    paint.scene.draw_text(paint.text.layout(
        "Later issues replace this shell transcript with a seller-specific Codex session, typed tool calls, and exact preview/publish history.",
        Point::new(bounds.origin.x + 10.0, bounds.max_y() - 18.0),
        10.0,
        theme::text::MUTED,
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
        "Later issues replace the local preview contour with the exact authority payload and explicit confirmation.",
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

    paint.scene.draw_text(paint.text.layout(
        &format!(
            "Local asset preview: {}",
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
            "session origin: {}",
            pane_state.codex_profile.session_origin
        ),
        Point::new(bounds.origin.x + 10.0, row_y + 26.0),
        11.0,
        theme::text::SECONDARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "This pane is intentionally allowed to express intent before it is allowed to mutate authority state.",
        Point::new(bounds.origin.x + 10.0, row_y + 44.0),
        11.0,
        theme::text::SECONDARY,
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

fn speaker_color(speaker: DataSellerShellSpeaker) -> wgpui::Hsla {
    match speaker {
        DataSellerShellSpeaker::System => theme::text::MUTED,
        DataSellerShellSpeaker::SellerAgent => theme::status::INFO,
        DataSellerShellSpeaker::Seller => theme::status::SUCCESS,
    }
}
