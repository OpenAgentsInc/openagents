use std::collections::HashMap;

use serde_json::Value;
use wgpui::{Bounds, Component, InputEvent, PaintContext, Point, Quad, theme};

use crate::app_state::{
    AutopilotChatState, AutopilotMessage, AutopilotRole, DataSellerPaneInputs, DataSellerPaneState,
    DataSellerShellSpeaker, PaneKind, RenderState,
};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_source_badge, split_text_for_display,
};
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
const LABEL_ROW_HEIGHT: f32 = 15.0;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &DataSellerPaneState,
    autopilot_chat: &AutopilotChatState,
    inputs: &mut DataSellerPaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "codex.data_seller.v0", paint);
    paint_action_button(
        data_seller_send_button_bounds(content_bounds),
        "Send",
        paint,
    );
    paint_action_button(
        data_seller_preview_button_bounds(content_bounds),
        "Preview Asset",
        paint,
    );
    paint_action_button(
        data_seller_confirm_button_bounds(content_bounds),
        "Confirm Asset",
        paint,
    );
    paint_action_button(
        data_seller_publish_button_bounds(content_bounds),
        "Publish Asset",
        paint,
    );

    let intro_chunk_len = ((content_bounds.size.width - PADDING * 2.0) / 6.2).max(28.0) as usize;
    let mut intro_y = content_bounds.origin.y + 42.0;
    for line in split_text_for_display(
        "Conversational authoring surface for truthful data listings. Asset and grant publication remain separate economic actions even though the seller flow shares one pane.",
        intro_chunk_len,
    ) {
        paint.scene.draw_text(paint.text.layout(
            line.as_str(),
            Point::new(content_bounds.origin.x + PADDING, intro_y),
            11.0,
            theme::text::SECONDARY,
        ));
        intro_y += 14.0;
    }

    let mut status_y = intro_y + 4.0;
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
    let mut status_end_y = paint_label_line(
        paint,
        content_bounds.origin.x + PADDING,
        status_y,
        "status line",
        &pane_state.status_line,
    );

    let status_chunk_len = ((content_bounds.size.width - PADDING * 2.0) / 6.2).max(28.0) as usize;
    if let Some(action) = pane_state.last_action.as_deref() {
        for line in split_text_for_display(action, status_chunk_len).into_iter().take(2) {
            paint.scene.draw_text(paint.text.layout(
                line.as_str(),
                Point::new(content_bounds.origin.x + PADDING, status_end_y),
                11.0,
                theme::text::SECONDARY,
            ));
            status_end_y += 14.0;
        }
    }
    if let Some(error) = pane_state.last_error.as_deref() {
        for line in split_text_for_display(error, status_chunk_len).into_iter().take(2) {
            paint.scene.draw_text(paint.text.layout(
                line.as_str(),
                Point::new(content_bounds.origin.x + PADDING, status_end_y),
                11.0,
                theme::status::ERROR,
            ));
            status_end_y += 14.0;
        }
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

    let content_top = (status_end_y + 10.0).max(content_bounds.origin.y + HEADER_BOTTOM);
    let available_height = (composer_bounds.origin.y - content_top - COLUMN_GAP - PADDING).max(260.0);
    let draft_height = draft_card_height(pane_state).max(220.0);
    let preview_height = preview_card_height().max(170.0);
    let status_height = inventory_card_height(pane_state).max(260.0);
    let right_column_total_height = draft_height + preview_height + status_height + COLUMN_GAP * 2.0;
    let transcript_width =
        ((content_bounds.size.width - PADDING * 2.0 - COLUMN_GAP) * 0.52).max(340.0);
    let side_width =
        (content_bounds.size.width - PADDING * 2.0 - COLUMN_GAP - transcript_width).max(260.0);
    let viewport = scroll_viewport_bounds(content_bounds, content_top, composer_bounds.origin.y);
    let content_height = content_height(available_height, right_column_total_height);
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = pane_state.scroll_offset_px.min(max_scroll);

    let transcript_bounds = Bounds::new(
        content_bounds.origin.x + PADDING,
        content_top - scroll_offset,
        transcript_width,
        available_height,
    );
    let draft_bounds = Bounds::new(
        transcript_bounds.max_x() + COLUMN_GAP,
        transcript_bounds.origin.y,
        side_width,
        draft_height,
    );
    let preview_bounds = Bounds::new(
        draft_bounds.origin.x,
        draft_bounds.max_y() + COLUMN_GAP,
        side_width,
        preview_height,
    );
    let status_bounds = Bounds::new(
        draft_bounds.origin.x,
        preview_bounds.max_y() + COLUMN_GAP,
        side_width,
        status_height,
    );

    paint.scene.push_clip(viewport);
    paint_transcript_shell(transcript_bounds, pane_state, autopilot_chat, paint);
    paint_draft_shell_card(draft_bounds, pane_state, paint);
    paint_asset_preview_card(preview_bounds, pane_state, paint);
    paint_publication_status_card(status_bounds, pane_state, paint);
    paint.scene.pop_clip();
    paint_scrollbar(viewport, content_height, scroll_offset, paint);
}

pub fn scroll_viewport_bounds(content_bounds: Bounds, content_top: f32, composer_top_y: f32) -> Bounds {
    let top = content_top;
    Bounds::new(
        content_bounds.origin.x + 8.0,
        top,
        (content_bounds.size.width - 16.0).max(1.0),
        (composer_top_y - top - 8.0).max(1.0),
    )
}

fn content_height(available_height: f32, right_column_total_height: f32) -> f32 {
    right_column_total_height.max(available_height) + PADDING * 2.0
}

fn draft_card_height(pane_state: &DataSellerPaneState) -> f32 {
    let mut rows = 12.0;
    rows += 1.0; // status line summary
    rows += if pane_state.active_draft.readiness_blockers.is_empty() {
        1.0
    } else {
        2.0
    };
    rows += 3.0; // footer guidance lines
    CARD_HEADER_HEIGHT + 16.0 + rows * LABEL_ROW_HEIGHT + 16.0
}

fn preview_card_height() -> f32 {
    let rows = 9.0 + 3.0; // details + explanatory lines
    CARD_HEADER_HEIGHT + 16.0 + rows * LABEL_ROW_HEIGHT + 14.0
}

fn inventory_card_height(pane_state: &DataSellerPaneState) -> f32 {
    let mut rows = 7.0; // base rows before latest request
    rows += if pane_state.latest_incoming_request().is_some() {
        5.0
    } else {
        1.0
    };
    rows += 5.0; // warning + latest request summary lines
    CARD_HEADER_HEIGHT + 16.0 + rows * LABEL_ROW_HEIGHT + 16.0
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

    let content_clip = Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + CARD_HEADER_HEIGHT + 4.0,
        (bounds.size.width - 16.0).max(1.0),
        (bounds.size.height - CARD_HEADER_HEIGHT - 12.0).max(1.0),
    );
    paint.scene.push_clip(content_clip);
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

    let footer_chunk = ((bounds.size.width - 20.0) / 6.2).max(20.0) as usize;
    let footer = if has_thread_transcript {
        "Seller prompts now run on the dedicated Codex thread. Typed tools update the draft while the thread asks bounded follow-up questions."
    } else {
        "Send a seller prompt to start the dedicated Codex conversation and draft-normalization loop."
    };
    let mut footer_y = bounds.max_y() - 30.0;
    for line in split_text_for_display(footer, footer_chunk).into_iter().take(2) {
        paint.scene.draw_text(paint.text.layout(
            line.as_str(),
            Point::new(bounds.origin.x + 10.0, footer_y),
            10.0,
            theme::text::MUTED,
        ));
        footer_y += 11.0;
    }
    paint.scene.pop_clip();
}

fn paint_transcript_row(
    row_bounds: Bounds,
    label: &str,
    label_color: wgpui::Hsla,
    content: &str,
    paint: &mut PaintContext,
) {
    paint.scene.push_clip(row_bounds);
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
    let chunk_len = ((row_bounds.size.width - 28.0) / 7.1).max(14.0) as usize;
    let mut y = row_bounds.origin.y + 28.0;
    for line in split_text_for_display(content, chunk_len).into_iter().take(3) {
        paint.scene.draw_text(paint.text.layout(
            line.as_str(),
            Point::new(row_bounds.origin.x + 10.0, y),
            11.0,
            theme::text::PRIMARY,
        ));
        y += 12.0;
    }
    paint.scene.pop_clip();
}

fn paint_draft_shell_card(
    bounds: Bounds,
    pane_state: &DataSellerPaneState,
    paint: &mut PaintContext,
) {
    paint_card(bounds, "Draft card", "Structured local truth object", paint);

    let clip = Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + CARD_HEADER_HEIGHT + 4.0,
        (bounds.size.width - 16.0).max(1.0),
        (bounds.size.height - CARD_HEADER_HEIGHT - 10.0).max(1.0),
    );
    paint.scene.push_clip(clip);
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
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "package",
        &draft_package_summary(&pane_state.active_draft.metadata),
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
    let grant_template = option_label(pane_state.active_draft.grant_policy_template.as_deref());
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "grant template",
        &grant_template,
    );
    let grant_consumer = option_label(pane_state.active_draft.grant_consumer_id.as_deref());
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "grant consumer",
        &grant_consumer,
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "grant ttl",
        &hours_label(pane_state.active_draft.grant_expires_in_hours),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "grant warranty",
        &hours_label(pane_state.active_draft.grant_warranty_window_hours),
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
    } else if let Some(package_note) = draft_package_note(&pane_state.active_draft.metadata) {
        paint.scene.draw_text(paint.text.layout(
            package_note.as_str(),
            Point::new(bounds.origin.x + 10.0, row_y + 28.0),
            10.0,
            theme::text::MUTED,
        ));
    }
    paint.scene.draw_text(paint.text.layout(
        "The draft evolves through the seller conversation plus typed Data Market tools. Use autopilotctl or headless flows to steer the same underlying state machine.",
        Point::new(bounds.origin.x + 10.0, bounds.max_y() - 18.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.pop_clip();
}

fn paint_publication_status_card(
    bounds: Bounds,
    pane_state: &DataSellerPaneState,
    paint: &mut PaintContext,
) {
    paint_card(
        bounds,
        "Seller inventory",
        "Published authority + latest request flow",
        paint,
    );

    let clip = Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + CARD_HEADER_HEIGHT + 4.0,
        (bounds.size.width - 16.0).max(1.0),
        (bounds.size.height - CARD_HEADER_HEIGHT - 10.0).max(1.0),
    );
    paint.scene.push_clip(clip);
    let published_asset = pane_state.last_published_asset.as_ref();
    let published_grant = pane_state.last_published_grant.as_ref();
    let mut row_y = bounds.origin.y + CARD_HEADER_HEIGHT + 12.0;
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "inventory",
        format!(
            "{} assets // {} grants",
            pane_state.published_asset_count(),
            pane_state.published_grant_count()
        )
        .as_str(),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "latest asset",
        published_asset
            .map(asset_inventory_summary)
            .unwrap_or_else(|| "pending".to_string())
            .as_str(),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "latest grant",
        published_grant
            .map(grant_inventory_summary)
            .unwrap_or_else(|| "pending".to_string())
            .as_str(),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "receipts",
        format!(
            "asset {} // grant {}",
            option_short_id(pane_state.last_publish_receipt_id.as_deref()),
            option_short_id(pane_state.last_grant_publish_receipt_id.as_deref())
        )
        .as_str(),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "fulfillment",
        format!(
            "delivery {} // revocation {}",
            option_short_id(
                pane_state
                    .last_published_delivery
                    .as_ref()
                    .map(|delivery| delivery.delivery_bundle_id.as_str())
            ),
            option_short_id(
                pane_state
                    .last_published_revocation
                    .as_ref()
                    .map(|revocation| revocation.revocation_id.as_str())
            )
        )
        .as_str(),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "previews",
        format!(
            "asset {} ({}) // grant {} ({})",
            if pane_state
                .active_draft
                .last_previewed_asset_payload
                .is_some()
            {
                "ready"
            } else {
                "pending"
            },
            bool_label(pane_state.asset_preview_confirmed),
            if pane_state
                .active_draft
                .last_previewed_grant_payload
                .is_some()
            {
                "ready"
            } else {
                "pending"
            },
            bool_label(pane_state.grant_preview_confirmed),
        )
        .as_str(),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "warnings",
        &inventory_warning_summary(pane_state),
    );

    if let Some(latest_request) = pane_state.latest_incoming_request() {
        row_y = paint_label_line(
            paint,
            bounds.origin.x + 10.0,
            row_y,
            "latest request",
            request_inventory_summary(latest_request).as_str(),
        );
        row_y = paint_label_line(
            paint,
            bounds.origin.x + 10.0,
            row_y,
            "route",
            request_route_summary(latest_request).as_str(),
        );
        row_y = paint_label_line(
            paint,
            bounds.origin.x + 10.0,
            row_y,
            "payment",
            request_payment_summary(latest_request).as_str(),
        );
        row_y = paint_label_line(
            paint,
            bounds.origin.x + 10.0,
            row_y,
            "delivery",
            request_delivery_summary(latest_request).as_str(),
        );
        row_y = paint_label_line(
            paint,
            bounds.origin.x + 10.0,
            row_y,
            "revocation",
            request_revocation_summary(latest_request).as_str(),
        );
    } else {
        row_y = paint_label_line(
            paint,
            bounds.origin.x + 10.0,
            row_y,
            "latest request",
            "none",
        );
    }

    let warnings = pane_state.inventory_warnings();
    let warning_color = if warnings.is_empty() {
        theme::status::SUCCESS
    } else {
        theme::status::WARNING
    };
    let warning_text = if warnings.is_empty() {
        "Seller inventory and draft posture are currently aligned.".to_string()
    } else {
        warnings
            .iter()
            .take(2)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    };
    paint.scene.draw_text(paint.text.layout(
        warning_text.as_str(),
        Point::new(bounds.origin.x + 10.0, row_y + 10.0),
        10.0,
        warning_color,
    ));
    if let Some(latest_request) = pane_state.latest_incoming_request() {
        let mut payment_summary = format!("payment={}", latest_request.payment_state.label());
        if let Some(amount_sats) = latest_request.payment_amount_sats {
            payment_summary.push_str(format!(" | settled={} sats", amount_sats).as_str());
        } else if let Some(invoice) = latest_request.pending_bolt11.as_ref() {
            payment_summary.push_str(format!(" | invoice={} chars", invoice.len()).as_str());
        }
        if let Some(payment_pointer) = latest_request.payment_pointer.as_ref() {
            payment_summary.push_str(format!(" | receipt={payment_pointer}").as_str());
        }
        let delivery_summary = format!(
            "delivery={}{}",
            latest_request.delivery_state.label(),
            latest_request
                .delivery_bundle_id
                .as_deref()
                .map(|bundle_id| format!(" | bundle={bundle_id}"))
                .unwrap_or_default()
        );
        let revocation_summary = format!(
            "revocation={}{}",
            latest_request.revocation_state.label(),
            latest_request
                .revocation_id
                .as_deref()
                .map(|revocation_id| format!(" | receipt={revocation_id}"))
                .unwrap_or_default()
        );
        paint.scene.draw_text(paint.text.layout(
            &format!(
                "{} | buyer={} | bid={} sats{} | {} | {} | {}",
                latest_request.evaluation_summary,
                latest_request.requester,
                latest_request.price_sats,
                latest_request
                    .required_price_sats
                    .map(|price| format!(" | ask={} sats", price))
                    .unwrap_or_default(),
                payment_summary,
                delivery_summary,
                revocation_summary,
            ),
            Point::new(bounds.origin.x + 10.0, row_y + 28.0),
            10.0,
            theme::text::MUTED,
        ));
    }
    paint.scene.pop_clip();
}

fn paint_asset_preview_card(
    bounds: Bounds,
    pane_state: &DataSellerPaneState,
    paint: &mut PaintContext,
) {
    paint_card(
        bounds,
        "Exact previews",
        "Asset + grant authority payloads",
        paint,
    );

    let clip = Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + CARD_HEADER_HEIGHT + 4.0,
        (bounds.size.width - 16.0).max(1.0),
        (bounds.size.height - CARD_HEADER_HEIGHT - 10.0).max(1.0),
    );
    paint.scene.push_clip(clip);
    let asset_payload = pane_state
        .active_draft
        .last_previewed_asset_payload
        .as_ref();
    let grant_payload = pane_state
        .active_draft
        .last_previewed_grant_payload
        .as_ref();
    let mut row_y = bounds.origin.y + CARD_HEADER_HEIGHT + 12.0;
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "asset preview",
        if asset_payload.is_some() {
            "ready"
        } else {
            "pending"
        },
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "asset confirm",
        bool_label(pane_state.asset_preview_confirmed),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "asset id",
        &preview_value(asset_payload, &["asset", "asset_id"]),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "asset policy",
        &preview_value(asset_payload, &["policy", "policy_bundle_id"]),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "grant preview",
        if grant_payload.is_some() {
            "ready"
        } else {
            "pending"
        },
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "grant confirm",
        bool_label(pane_state.grant_preview_confirmed),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "grant id",
        &preview_value(grant_payload, &["grant", "grant_id"]),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "grant consumer",
        &preview_value(grant_payload, &["grant", "consumer_id"]),
    );
    row_y = paint_label_line(
        paint,
        bounds.origin.x + 10.0,
        row_y,
        "grant policy",
        &preview_value(grant_payload, &["grant", "permission_policy", "policy_id"]),
    );

    paint.scene.draw_text(paint.text.layout(
        if asset_payload.is_some() || grant_payload.is_some() {
            "These previews are the exact authority payloads that autopilotctl, headless runs, and the pane publish paths submit."
        } else {
            "Run preview after resolving blockers to materialize the exact authority payload."
        },
        Point::new(bounds.origin.x + 10.0, row_y + 10.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.pop_clip();
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
    let chunk_len = ((bounds.size.width - 20.0) / 6.2).max(18.0) as usize;
    let mut subtitle_y = bounds.origin.y + 22.0;
    for line in split_text_for_display(subtitle, chunk_len).into_iter().take(2) {
        paint.scene.draw_text(paint.text.layout(
            line.as_str(),
            Point::new(bounds.origin.x + 10.0, subtitle_y),
            10.0,
            theme::text::MUTED,
        ));
        subtitle_y += 11.0;
    }
}

fn paint_scrollbar(viewport: Bounds, content_height: f32, scroll_offset: f32, paint: &mut PaintContext) {
    if viewport.size.height <= 0.0 || content_height <= viewport.size.height + 0.5 {
        return;
    }
    let max_offset = (content_height - viewport.size.height).max(0.0);
    let track_bounds = Bounds::new(viewport.max_x() - 2.0, viewport.origin.y, 2.0, viewport.size.height);
    let thumb_height = ((viewport.size.height / content_height) * viewport.size.height)
        .clamp(16.0, viewport.size.height.max(0.0));
    let thumb_y =
        viewport.origin.y + ((scroll_offset / max_offset.max(1.0)) * (viewport.size.height - thumb_height));
    paint.scene.draw_quad(
        Quad::new(track_bounds)
            .with_background(theme::border::DEFAULT.with_alpha(0.45))
            .with_corner_radius(1.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(track_bounds.origin.x, thumb_y, track_bounds.size.width, thumb_height))
            .with_background(theme::text::MUTED.with_alpha(0.75))
            .with_corner_radius(1.0),
    );
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

fn inventory_warning_summary(pane_state: &DataSellerPaneState) -> String {
    let warnings = pane_state.inventory_warnings();
    if warnings.is_empty() {
        "none".to_string()
    } else {
        warnings.into_iter().take(2).collect::<Vec<_>>().join(" | ")
    }
}

fn hours_label(value: Option<u64>) -> String {
    value
        .map(|hours| format!("{hours}h"))
        .unwrap_or_else(|| "pending".to_string())
}

fn format_money(value: &openagents_kernel_core::receipts::Money) -> String {
    match value.amount {
        openagents_kernel_core::receipts::MoneyAmount::AmountSats(amount) => {
            format!("{amount} sats")
        }
        openagents_kernel_core::receipts::MoneyAmount::AmountMsats(amount) => {
            format!("{amount} msats")
        }
    }
}

fn preview_value(payload: Option<&Value>, path: &[&str]) -> String {
    let mut current = payload;
    for segment in path {
        current = current.and_then(|value| value.get(*segment));
    }
    current
        .map(value_label)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "pending".to_string())
}

fn value_label(value: &Value) -> String {
    match value {
        Value::String(value) => compact_text(value, 48),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(entries) => format!("{} item(s)", entries.len()),
        Value::Object(_) => "object".to_string(),
        Value::Null => "pending".to_string(),
    }
}

fn draft_package_summary(metadata: &HashMap<String, String>) -> String {
    if metadata
        .get("codex_conversation_export")
        .is_some_and(|value| value == "true")
    {
        let tier = metadata
            .get("codex_redaction_tier")
            .map(String::as_str)
            .unwrap_or("unspecified");
        let sessions = metadata
            .get("codex_session_count")
            .map(String::as_str)
            .unwrap_or("?");
        let selection = metadata
            .get("codex_selection_mode")
            .map(String::as_str)
            .unwrap_or("unknown");
        return compact_text(
            format!("codex export // {tier} // {sessions} sessions // {selection}").as_str(),
            48,
        );
    }
    metadata
        .get("export_kind")
        .map(|value| compact_text(value, 48))
        .unwrap_or_else(|| "local package pending".to_string())
}

fn draft_package_note(metadata: &HashMap<String, String>) -> Option<String> {
    let index = metadata
        .get("codex_export_index_path")
        .or_else(|| metadata.get("packaging_summary_path"))?;
    Some(format!("bundle index: {}", compact_text(index, 48)))
}

fn asset_inventory_summary(asset: &openagents_kernel_core::data::DataAsset) -> String {
    compact_text(
        format!(
            "{} // {} // {}",
            short_id(asset.asset_id.as_str()),
            asset.status.label(),
            asset.asset_kind
        )
        .as_str(),
        48,
    )
}

fn grant_inventory_summary(grant: &openagents_kernel_core::data::AccessGrant) -> String {
    compact_text(
        format!(
            "{} // {} // {} // {}",
            short_id(grant.grant_id.as_str()),
            grant.status.label(),
            grant
                .consumer_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "open".to_string()),
            grant
                .offer_price
                .as_ref()
                .map(format_money)
                .unwrap_or_else(|| "price pending".to_string())
        )
        .as_str(),
        48,
    )
}

fn request_inventory_summary(request: &crate::app_state::DataSellerIncomingRequest) -> String {
    compact_text(
        format!(
            "{} // buyer {} // {}",
            short_id(request.request_id.as_str()),
            short_id(request.requester.as_str()),
            request.evaluation_disposition.label()
        )
        .as_str(),
        48,
    )
}

fn request_route_summary(request: &crate::app_state::DataSellerIncomingRequest) -> String {
    compact_text(
        format!(
            "{} // kind {}",
            relay_label(request.source_relay_url.as_deref()),
            request.request_kind
        )
        .as_str(),
        48,
    )
}

fn request_payment_summary(request: &crate::app_state::DataSellerIncomingRequest) -> String {
    let price = request
        .required_price_sats
        .map(|value| format!("ask {value} sats"))
        .unwrap_or_else(|| format!("bid {} sats", request.price_sats));
    compact_text(
        format!(
            "{} // {} // {}",
            request.payment_state.label(),
            price,
            option_short_id(request.payment_pointer.as_deref())
        )
        .as_str(),
        48,
    )
}

fn request_delivery_summary(request: &crate::app_state::DataSellerIncomingRequest) -> String {
    compact_text(
        format!(
            "{} // bundle {} // result {}",
            request.delivery_state.label(),
            option_short_id(request.delivery_bundle_id.as_deref()),
            option_short_id(request.delivery_result_event_id.as_deref())
        )
        .as_str(),
        48,
    )
}

fn request_revocation_summary(request: &crate::app_state::DataSellerIncomingRequest) -> String {
    compact_text(
        format!(
            "{} // {}",
            request.revocation_state.label(),
            option_short_id(request.revocation_receipt_id.as_deref())
        )
        .as_str(),
        48,
    )
}

fn option_short_id(value: Option<&str>) -> String {
    value.map(short_id).unwrap_or_else(|| "none".to_string())
}

fn short_id(value: &str) -> String {
    if value.len() <= 18 {
        return value.to_string();
    }
    format!("{}..{}", &value[..8], &value[value.len() - 6..])
}

fn relay_label(value: Option<&str>) -> String {
    value
        .map(|relay| {
            compact_text(
                relay
                    .trim()
                    .trim_start_matches("wss://")
                    .trim_start_matches("ws://"),
                22,
            )
        })
        .unwrap_or_else(|| "direct/local".to_string())
}

fn compact_text(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let compact = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{compact}...")
    } else {
        compact
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_package_summary_prefers_codex_export_metadata() {
        let metadata = HashMap::from([
            ("codex_conversation_export".to_string(), "true".to_string()),
            ("codex_redaction_tier".to_string(), "public".to_string()),
            ("codex_session_count".to_string(), "3".to_string()),
            (
                "codex_selection_mode".to_string(),
                "latest_from_codex_home".to_string(),
            ),
        ]);
        let summary = draft_package_summary(&metadata);
        assert!(summary.contains("codex export"));
        assert!(summary.contains("public"));
        assert!(summary.contains("3"));
    }
}
