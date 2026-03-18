use chrono::{Local, TimeZone};
use openagents_kernel_core::data::{AccessGrant, DataAsset, RevocationReceipt};
use serde_json::Value;
use wgpui::{Bounds, PaintContext, Point, Quad, theme};

use crate::app_state::{DataBuyerPaneState, DataMarketPaneState};
use crate::pane_renderer::{paint_action_button, paint_label_line, paint_source_badge};
use crate::pane_system::{
    data_buyer_next_asset_button_bounds, data_buyer_previous_asset_button_bounds,
    data_buyer_publish_button_bounds, data_buyer_refresh_button_bounds,
};
use crate::state::operations::{NetworkRequestsState, SubmittedNetworkRequest};

const PADDING: f32 = 12.0;
const HEADER_BOTTOM: f32 = 146.0;
const CARD_GAP: f32 = 12.0;
const CARD_HEADER_HEIGHT: f32 = 28.0;
const ROW_HEIGHT: f32 = 18.0;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &DataBuyerPaneState,
    market_state: &DataMarketPaneState,
    network_requests: &NetworkRequestsState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "buyer.data_access.v1", paint);
    paint_action_button(
        data_buyer_refresh_button_bounds(content_bounds),
        "Refresh",
        paint,
    );
    paint_action_button(
        data_buyer_previous_asset_button_bounds(content_bounds),
        "Prev Asset",
        paint,
    );
    paint_action_button(
        data_buyer_next_asset_button_bounds(content_bounds),
        "Next Asset",
        paint,
    );
    paint_action_button(
        data_buyer_publish_button_bounds(content_bounds),
        "Publish Request",
        paint,
    );

    paint.scene.draw_text(paint.text.layout(
        "Narrow buyer-side targeted request surface. This pane selects a visible asset and publishes a targeted NIP-90 data-access request without widening into public discovery.",
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
        "buyer",
        pane_state
            .local_buyer_id
            .as_deref()
            .unwrap_or("unconfigured"),
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

    let left_width = ((content_bounds.size.width - PADDING * 2.0 - CARD_GAP) * 0.54).max(360.0);
    let right_width =
        (content_bounds.size.width - PADDING * 2.0 - CARD_GAP - left_width).max(280.0);
    let top_height =
        ((content_bounds.size.height - HEADER_BOTTOM - PADDING * 2.0 - CARD_GAP) * 0.5).max(180.0);
    let bottom_height =
        (content_bounds.size.height - HEADER_BOTTOM - PADDING * 2.0 - CARD_GAP - top_height)
            .max(180.0);

    let asset_bounds = Bounds::new(
        content_bounds.origin.x + PADDING,
        content_bounds.origin.y + HEADER_BOTTOM,
        left_width,
        top_height,
    );
    let draft_bounds = Bounds::new(
        asset_bounds.max_x() + CARD_GAP,
        asset_bounds.origin.y,
        right_width,
        top_height,
    );
    let truth_bounds = Bounds::new(
        asset_bounds.origin.x,
        asset_bounds.max_y() + CARD_GAP,
        content_bounds.size.width - PADDING * 2.0,
        bottom_height,
    );

    let selected_asset = pane_state.selected_asset(market_state);
    let selected_grant = pane_state.selected_offer_grant(market_state);
    let selected_revocation = pane_state.selected_revocation(market_state);
    let draft = pane_state.derived_request_draft(market_state);
    let latest_request = pane_state
        .last_published_request_id
        .as_deref()
        .and_then(|request_id| {
            network_requests
                .submitted
                .iter()
                .find(|request| request.request_id == request_id)
        });

    paint_asset_card(
        asset_bounds,
        selected_asset,
        selected_grant,
        selected_revocation,
        paint,
    );
    paint_draft_card(draft_bounds, draft.as_ref(), paint);
    paint_truth_card(
        truth_bounds,
        pane_state,
        latest_request,
        selected_grant,
        selected_revocation,
        paint,
    );
}

fn paint_asset_card(
    bounds: Bounds,
    asset: Option<&DataAsset>,
    grant: Option<&AccessGrant>,
    revocation: Option<&RevocationReceipt>,
    paint: &mut PaintContext,
) {
    paint_card(bounds, "Selected Asset", "Visible market snapshot", paint);
    let Some(asset) = asset else {
        paint.scene.draw_text(paint.text.layout(
            "No active asset is selected yet. Refresh the market snapshot or publish a seller asset first.",
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + CARD_HEADER_HEIGHT + 12.0),
            11.0,
            theme::text::MUTED,
        ));
        return;
    };

    let mut row_y = bounds.origin.y + CARD_HEADER_HEIGHT + 12.0;
    for (label, value) in [
        ("title", compact_text(asset.title.as_str(), 44)),
        (
            "asset",
            compact_text(
                format!(
                    "{} // {} // {}",
                    short_id(asset.asset_id.as_str()),
                    asset.asset_kind,
                    asset.status.label()
                )
                .as_str(),
                44,
            ),
        ),
        ("provider", compact_text(asset.provider_id.as_str(), 44)),
        (
            "pricing",
            compact_text(
                format!(
                    "{} // {}",
                    asset
                        .price_hint
                        .as_ref()
                        .map(format_money)
                        .unwrap_or_else(|| "none".to_string()),
                    asset
                        .default_policy
                        .as_ref()
                        .map(|policy| policy.policy_id.clone())
                        .unwrap_or_else(|| "policy none".to_string()),
                )
                .as_str(),
                44,
            ),
        ),
        ("market", asset_market_posture(asset)),
        (
            "bundle",
            asset_bundle_summary(asset).unwrap_or_else(|| "generic package".to_string()),
        ),
    ] {
        row_y = paint_row(bounds, row_y, label, value.as_str(), paint);
    }
    if let Some(grant) = grant {
        row_y = paint_row(bounds, row_y, "offer_grant", grant.grant_id.as_str(), paint);
        row_y = paint_row(
            bounds,
            row_y,
            "offer",
            compact_text(
                format!(
                    "{} // {} // {}",
                    grant.status.label(),
                    grant_duration_label(grant),
                    grant
                        .offer_price
                        .as_ref()
                        .map(format_money)
                        .unwrap_or_else(|| "none".to_string())
                )
                .as_str(),
                44,
            )
            .as_str(),
            paint,
        );
    } else {
        row_y = paint_row(bounds, row_y, "offer", "none", paint);
    }
    let revocation_label = revocation
        .map(|revocation| {
            format!(
                "{} ({})",
                revocation.revocation_id,
                revocation.status.label()
            )
        })
        .unwrap_or_else(|| "none".to_string());
    let _ = paint_row(
        bounds,
        row_y,
        "revocation",
        revocation_label.as_str(),
        paint,
    );
}

fn paint_draft_card(
    bounds: Bounds,
    draft: Option<&crate::app_state::DataBuyerRequestDraft>,
    paint: &mut PaintContext,
) {
    paint_card(
        bounds,
        "Request Draft",
        "Derived from selected asset + offer terms",
        paint,
    );
    let Some(draft) = draft else {
        paint.scene.draw_text(paint.text.layout(
            "No request draft is available because there is no active asset selection.",
            Point::new(
                bounds.origin.x + 10.0,
                bounds.origin.y + CARD_HEADER_HEIGHT + 12.0,
            ),
            11.0,
            theme::text::MUTED,
        ));
        return;
    };

    let mut row_y = bounds.origin.y + CARD_HEADER_HEIGHT + 12.0;
    for (label, value) in [
        ("provider", draft.provider_id.as_str().to_string()),
        ("asset_id", draft.asset_id.as_str().to_string()),
        (
            "grant_id",
            draft
                .offer_grant_id
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        ),
        ("delivery_mode", draft.delivery_mode.as_str().to_string()),
        (
            "preview_posture",
            draft.preview_posture.as_str().to_string(),
        ),
        ("bid", format!("{} sats", draft.bid_sats)),
        ("timeout", format!("{}s", draft.timeout_seconds)),
        ("scopes", draft.permission_scopes.join(", ")),
    ] {
        row_y = paint_row(bounds, row_y, label, value.as_str(), paint);
    }
}

fn paint_truth_card(
    bounds: Bounds,
    pane_state: &DataBuyerPaneState,
    latest_request: Option<&SubmittedNetworkRequest>,
    grant: Option<&AccessGrant>,
    revocation: Option<&RevocationReceipt>,
    paint: &mut PaintContext,
) {
    paint_card(
        bounds,
        "Buyer Truth",
        "Published request plus current grant/revocation posture",
        paint,
    );
    let mut row_y = bounds.origin.y + CARD_HEADER_HEIGHT + 12.0;

    let request_id = pane_state
        .last_published_request_id
        .as_deref()
        .unwrap_or("none");
    row_y = paint_row(bounds, row_y, "request_id", request_id, paint);
    row_y = paint_row(
        bounds,
        row_y,
        "published_event",
        pane_state
            .last_published_request_event_id
            .as_deref()
            .unwrap_or("none"),
        paint,
    );
    row_y = paint_row(
        bounds,
        row_y,
        "request_status",
        latest_request
            .map(|request| request.status.label())
            .unwrap_or("unpublished"),
        paint,
    );
    row_y = paint_row(
        bounds,
        row_y,
        "seller_feedback",
        latest_request
            .and_then(|request| request.last_feedback_status.as_deref())
            .unwrap_or("none"),
        paint,
    );
    row_y = paint_row(
        bounds,
        row_y,
        "result_event",
        latest_request
            .and_then(|request| request.last_result_event_id.as_deref())
            .unwrap_or("none"),
        paint,
    );
    row_y = paint_row(
        bounds,
        row_y,
        "payment_pointer",
        latest_request
            .and_then(|request| request.last_payment_pointer.as_deref())
            .unwrap_or("none"),
        paint,
    );
    row_y = paint_row(
        bounds,
        row_y,
        "grant_duration",
        grant
            .map(grant_duration_label)
            .unwrap_or_else(|| "unknown".to_string())
            .as_str(),
        paint,
    );
    let revocation_label = revocation
        .map(|revocation| {
            format!(
                "{} // reason={} // recorded={}",
                revocation.status.label(),
                revocation.reason_code,
                format_epoch_ms(Some(revocation.created_at_ms))
            )
        })
        .unwrap_or_else(|| "none".to_string());
    let _ = paint_row(
        bounds,
        row_y,
        "revocation_posture",
        revocation_label.as_str(),
        paint,
    );
}

fn paint_card(bounds: Bounds, title: &str, subtitle: &str, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_text(paint.text.layout(
        title,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 8.0),
        11.0,
        theme::text::SECONDARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        subtitle,
        Point::new(bounds.origin.x + 96.0, bounds.origin.y + 8.0),
        10.0,
        theme::text::MUTED,
    ));
}

fn paint_row(
    bounds: Bounds,
    row_y: f32,
    label: &str,
    value: &str,
    paint: &mut PaintContext,
) -> f32 {
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 10.0, row_y),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        value,
        Point::new(bounds.origin.x + 128.0, row_y),
        11.0,
        theme::text::PRIMARY,
    ));
    row_y + ROW_HEIGHT
}

fn format_money(money: &openagents_kernel_core::receipts::Money) -> String {
    match money.amount {
        openagents_kernel_core::receipts::MoneyAmount::AmountSats(amount) => {
            format!("{amount} sats")
        }
        openagents_kernel_core::receipts::MoneyAmount::AmountMsats(amount) => {
            format!("{amount} msats")
        }
    }
}

fn asset_bundle_summary(asset: &DataAsset) -> Option<String> {
    if asset_metadata_string(asset, "codex_conversation_export").as_deref() == Some("true") {
        let tier = asset_metadata_string(asset, "codex_redaction_tier")
            .unwrap_or_else(|| "unspecified".to_string());
        let sessions =
            asset_metadata_string(asset, "codex_session_count").unwrap_or_else(|| "?".to_string());
        return Some(compact_text(
            format!("codex export // {tier} // {sessions} sessions").as_str(),
            44,
        ));
    }
    asset_metadata_string(asset, "export_kind").map(|value| compact_text(value.as_str(), 44))
}

fn asset_market_posture(asset: &DataAsset) -> String {
    let visibility = asset_metadata_string(asset, "visibility_posture")
        .unwrap_or_else(|| "visibility n/a".to_string());
    let sensitivity = asset_metadata_string(asset, "sensitivity_posture")
        .unwrap_or_else(|| "sensitivity n/a".to_string());
    compact_text(format!("{visibility}/{sensitivity}").as_str(), 44)
}

fn asset_metadata_string(asset: &DataAsset, field: &str) -> Option<String> {
    asset_metadata_value(asset, field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn asset_metadata_value<'a>(asset: &'a DataAsset, field: &str) -> Option<&'a Value> {
    asset.metadata.get(field).or_else(|| {
        asset
            .metadata
            .get("draft_metadata")
            .and_then(|value| value.get(field))
    })
}

fn short_id(value: &str) -> String {
    if value.len() <= 18 {
        return value.to_string();
    }
    format!("{}..{}", &value[..8], &value[value.len() - 6..])
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

fn grant_duration_label(grant: &AccessGrant) -> String {
    let window_ms = grant
        .expires_at_ms
        .saturating_sub(grant.created_at_ms)
        .max(0);
    let window_hours = window_ms as f64 / 3_600_000.0;
    format!(
        "{window_hours:.1}h // expires {}",
        format_epoch_ms(Some(grant.expires_at_ms))
    )
}

fn format_epoch_ms(value: Option<i64>) -> String {
    let Some(value) = value else {
        return "never".to_string();
    };
    Local
        .timestamp_millis_opt(value)
        .single()
        .map(|timestamp| timestamp.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn asset_bundle_summary_prefers_codex_export_metadata() {
        let asset = DataAsset {
            metadata: json!({
                "draft_metadata": {
                    "codex_conversation_export": "true",
                    "codex_redaction_tier": "public",
                    "codex_session_count": "2"
                }
            }),
            ..Default::default()
        };

        let summary = asset_bundle_summary(&asset).expect("bundle summary");
        assert!(summary.contains("codex export"));
        assert!(summary.contains("public"));
        assert!(summary.contains("2"));
    }
}
