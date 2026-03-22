use chrono::{Local, TimeZone};
use openagents_kernel_core::data::{
    AccessGrant, DataAsset, DeliveryBundle, PermissionPolicy, RevocationReceipt,
};
use openagents_kernel_core::receipts::{Asset, Money, MoneyAmount};
use serde_json::Value;
use wgpui::{Bounds, PaintContext, Point, Quad, theme};

use crate::app_state::{
    DataMarketLifecycleEntry, DataMarketPaneState, PaneLoadState, RelayDatasetListingProjection,
    RelayDatasetOfferProjection,
};
use crate::pane_renderer::{paint_action_button, paint_label_line, paint_source_badge};
use crate::pane_system::data_market_refresh_button_bounds;

const PADDING: f32 = 12.0;
const METRIC_TOP: f32 = 92.0;
const METRIC_HEIGHT: f32 = 56.0;
const METRIC_GAP: f32 = 10.0;
const LIFECYCLE_TOP: f32 = 160.0;
const LIFECYCLE_HEIGHT: f32 = 92.0;
const PANELS_TOP: f32 = 264.0;
const PANEL_GAP: f32 = 12.0;
const PANEL_HEADER_HEIGHT: f32 = 24.0;
const ROW_HEIGHT: f32 = 38.0;
const MAX_ROWS_PER_PANEL: usize = 4;
const MAX_LIFECYCLE_ROWS: usize = 3;

pub fn paint(content_bounds: Bounds, pane_state: &DataMarketPaneState, paint: &mut PaintContext) {
    paint_source_badge(content_bounds, "kernel.data_market.v1", paint);
    paint_action_button(
        data_market_refresh_button_bounds(content_bounds),
        "Refresh",
        paint,
    );

    paint.scene.draw_text(paint.text.layout(
        "Combined Data Market view: Nexus settlement truth plus relay-discovered DS listings and offers.",
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
        "last refresh",
        &format_refresh_time(pane_state.last_refreshed_at_ms),
    );
    if let Some(action) = pane_state.last_action.as_deref() {
        let _ = paint_label_line(
            paint,
            content_bounds.origin.x + PADDING,
            status_y,
            "last action",
            action,
        );
    }
    if let Some(error) = pane_state.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(
                content_bounds.origin.x + PADDING,
                content_bounds.origin.y + 78.0,
            ),
            11.0,
            theme::status::ERROR,
        ));
    }

    paint_metric_cards(content_bounds, pane_state, paint);
    paint_lifecycle_panel(content_bounds, pane_state, paint);
    paint_panels(content_bounds, pane_state, paint);
}

fn paint_metric_cards(
    content_bounds: Bounds,
    pane_state: &DataMarketPaneState,
    paint: &mut PaintContext,
) {
    let card_width =
        ((content_bounds.size.width - PADDING * 2.0 - METRIC_GAP * 3.0) / 4.0).max(100.0);
    let metrics = if pane_state.relay_listings.is_empty() && pane_state.relay_offers.is_empty() {
        [
            ("Assets", pane_state.assets.len()),
            ("Grants", pane_state.grants.len()),
            ("Deliveries", pane_state.deliveries.len()),
            ("Revocations", pane_state.revocations.len()),
        ]
    } else {
        [
            ("Assets", pane_state.assets.len()),
            ("Grants", pane_state.grants.len()),
            ("Listings", pane_state.relay_listings.len()),
            ("Offers", pane_state.relay_offers.len()),
        ]
    };
    for (index, (label, count)) in metrics.iter().enumerate() {
        let bounds = Bounds::new(
            content_bounds.origin.x + PADDING + index as f32 * (card_width + METRIC_GAP),
            content_bounds.origin.y + METRIC_TOP,
            card_width,
            METRIC_HEIGHT,
        );
        paint.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::HOVER)
                .with_border(theme::border::DEFAULT, 1.0)
                .with_corner_radius(8.0),
        );
        paint.scene.draw_text(paint.text.layout(
            label,
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
            10.0,
            theme::text::MUTED,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            &count.to_string(),
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + 28.0),
            22.0,
            theme::text::PRIMARY,
        ));
    }
}

fn paint_panels(
    content_bounds: Bounds,
    pane_state: &DataMarketPaneState,
    paint: &mut PaintContext,
) {
    let panel_width = ((content_bounds.size.width - PADDING * 2.0 - PANEL_GAP) / 2.0).max(180.0);
    let panel_height =
        ((content_bounds.size.height - PANELS_TOP - PADDING - PANEL_GAP) / 2.0).max(120.0);
    let left_x = content_bounds.origin.x + PADDING;
    let right_x = left_x + panel_width + PANEL_GAP;
    let top_y = content_bounds.origin.y + PANELS_TOP;
    let bottom_y = top_y + panel_height + PANEL_GAP;

    if pane_state.relay_listings.is_empty() && pane_state.relay_offers.is_empty() {
        paint_panel(
            Bounds::new(left_x, top_y, panel_width, panel_height),
            "Assets",
            pane_state.load_state,
            pane_state.assets.len(),
            pane_state
                .assets
                .iter()
                .take(MAX_ROWS_PER_PANEL)
                .map(asset_row_summary)
                .collect::<Vec<_>>(),
            paint,
        );
        paint_panel(
            Bounds::new(right_x, top_y, panel_width, panel_height),
            "Grants",
            pane_state.load_state,
            pane_state.grants.len(),
            pane_state
                .grants
                .iter()
                .take(MAX_ROWS_PER_PANEL)
                .map(grant_row_summary)
                .collect::<Vec<_>>(),
            paint,
        );
        paint_panel(
            Bounds::new(left_x, bottom_y, panel_width, panel_height),
            "Deliveries",
            pane_state.load_state,
            pane_state.deliveries.len(),
            pane_state
                .deliveries
                .iter()
                .take(MAX_ROWS_PER_PANEL)
                .map(delivery_row_summary)
                .collect::<Vec<_>>(),
            paint,
        );
        paint_panel(
            Bounds::new(right_x, bottom_y, panel_width, panel_height),
            "Revocations",
            pane_state.load_state,
            pane_state.revocations.len(),
            pane_state
                .revocations
                .iter()
                .take(MAX_ROWS_PER_PANEL)
                .map(revocation_row_summary)
                .collect::<Vec<_>>(),
            paint,
        );
        return;
    }

    paint_panel(
        Bounds::new(left_x, top_y, panel_width, panel_height),
        "Relay Listings",
        pane_state.load_state,
        pane_state.relay_listings.len(),
        pane_state
            .relay_listings
            .iter()
            .take(MAX_ROWS_PER_PANEL)
            .map(relay_listing_row_summary)
            .collect::<Vec<_>>(),
        paint,
    );
    paint_panel(
        Bounds::new(right_x, top_y, panel_width, panel_height),
        "Relay Offers",
        pane_state.load_state,
        pane_state.relay_offers.len(),
        pane_state
            .relay_offers
            .iter()
            .take(MAX_ROWS_PER_PANEL)
            .map(relay_offer_row_summary)
            .collect::<Vec<_>>(),
        paint,
    );
    paint_panel(
        Bounds::new(left_x, bottom_y, panel_width, panel_height),
        "Assets",
        pane_state.load_state,
        pane_state.assets.len(),
        pane_state
            .assets
            .iter()
            .take(MAX_ROWS_PER_PANEL)
            .map(asset_row_summary)
            .collect::<Vec<_>>(),
        paint,
    );
    paint_panel(
        Bounds::new(right_x, bottom_y, panel_width, panel_height),
        "Grants",
        pane_state.load_state,
        pane_state.grants.len(),
        pane_state
            .grants
            .iter()
            .take(MAX_ROWS_PER_PANEL)
            .map(grant_row_summary)
            .collect::<Vec<_>>(),
        paint,
    );
}

fn paint_lifecycle_panel(
    content_bounds: Bounds,
    pane_state: &DataMarketPaneState,
    paint: &mut PaintContext,
) {
    let bounds = Bounds::new(
        content_bounds.origin.x + PADDING,
        content_bounds.origin.y + LIFECYCLE_TOP,
        content_bounds.size.width - PADDING * 2.0,
        LIFECYCLE_HEIGHT,
    );
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_text(paint.text.layout(
        "Lifecycle",
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 8.0),
        11.0,
        theme::text::SECONDARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Recent seller-side market activity with policy and receipt context.",
        Point::new(bounds.origin.x + 70.0, bounds.origin.y + 8.0),
        10.0,
        theme::text::MUTED,
    ));

    if pane_state.lifecycle_entries.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No lifecycle entries recorded yet. Publish, settle, deliver, or revoke from the seller lane to populate this activity view.",
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + 32.0),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    for (index, (primary, secondary)) in pane_state
        .lifecycle_entries
        .iter()
        .take(MAX_LIFECYCLE_ROWS)
        .map(lifecycle_row_summary)
        .enumerate()
    {
        let row_y = bounds.origin.y + 30.0 + index as f32 * 20.0;
        paint.scene.draw_text(paint.text.layout(
            primary.as_str(),
            Point::new(bounds.origin.x + 10.0, row_y),
            11.0,
            theme::text::PRIMARY,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            secondary.as_str(),
            Point::new(bounds.origin.x + 10.0, row_y + 12.0),
            10.0,
            theme::text::MUTED,
        ));
    }
}

fn paint_panel(
    bounds: Bounds,
    title: &str,
    load_state: PaneLoadState,
    total_count: usize,
    rows: Vec<(String, String)>,
    paint: &mut PaintContext,
) {
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
    paint.scene.draw_text(paint.text.layout_mono(
        &total_count.to_string(),
        Point::new(bounds.max_x() - 32.0, bounds.origin.y + 8.0),
        11.0,
        theme::text::MUTED,
    ));

    if rows.is_empty() {
        let empty_label = match load_state {
            PaneLoadState::Loading => "Refreshing from Nexus...",
            PaneLoadState::Error => "Refresh failed or no kernel data is available.",
            PaneLoadState::Ready => "No rows loaded yet.",
        };
        paint.scene.draw_text(paint.text.layout(
            empty_label,
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + 34.0),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    let row_origin_y = bounds.origin.y + PANEL_HEADER_HEIGHT + 12.0;
    for (index, (primary, secondary)) in rows.iter().enumerate() {
        let row_y = row_origin_y + index as f32 * ROW_HEIGHT;
        paint.scene.draw_text(paint.text.layout(
            primary,
            Point::new(bounds.origin.x + 10.0, row_y),
            11.0,
            theme::text::PRIMARY,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            secondary,
            Point::new(bounds.origin.x + 10.0, row_y + 16.0),
            10.0,
            theme::text::MUTED,
        ));
    }

    if total_count > rows.len() {
        paint.scene.draw_text(paint.text.layout(
            &format!("+ {} more", total_count - rows.len()),
            Point::new(bounds.origin.x + 10.0, bounds.max_y() - 18.0),
            10.0,
            theme::text::MUTED,
        ));
    }
}

fn asset_row_summary(asset: &DataAsset) -> (String, String) {
    let packaging = asset_packaging_summary(asset);
    let market_posture = asset_market_posture(asset);
    let listing_coordinate = asset
        .nostr_publications
        .ds_listing
        .as_ref()
        .and_then(|reference| reference.coordinate.as_deref())
        .map(short_id)
        .unwrap_or_else(|| "no-ds".to_string());
    let primary = compact_text(
        if packaging.is_some() {
            format!(
                "{} // codex export // {}",
                asset.title,
                asset.status.label()
            )
        } else {
            format!(
                "{} // {} // {}",
                asset.title,
                asset.asset_kind,
                asset.status.label()
            )
        }
        .as_str(),
        58,
    );
    let secondary = compact_text(
        format!(
            "{} // listing {} // {}{} // {} // provider {}",
            short_id(asset.asset_id.as_str()),
            listing_coordinate,
            market_posture,
            packaging
                .map(|summary| format!(" // {summary}"))
                .unwrap_or_else(|| format!(" // {}", format_policy(asset.default_policy.as_ref()))),
            format_money(asset.price_hint.as_ref()),
            short_id(asset.provider_id.as_str()),
        )
        .as_str(),
        92,
    );
    (primary, secondary)
}

fn grant_row_summary(grant: &AccessGrant) -> (String, String) {
    let offer_coordinate = grant
        .nostr_publications
        .ds_offer
        .as_ref()
        .and_then(|reference| reference.coordinate.as_deref())
        .map(short_id)
        .unwrap_or_else(|| "no-ds".to_string());
    let primary = compact_text(
        format!(
            "{} // {} // {}",
            short_id(grant.grant_id.as_str()),
            short_id(grant.asset_id.as_str()),
            grant.status.label()
        )
        .as_str(),
        58,
    );
    let secondary = compact_text(
        format!(
            "offer {} // consumer {} // policy {} // expires {}",
            offer_coordinate,
            grant
                .consumer_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "open".to_string()),
            compact_text(grant.permission_policy.policy_id.as_str(), 18),
            format_timestamp_ms(Some(grant.expires_at_ms))
        )
        .as_str(),
        76,
    );
    (primary, secondary)
}

fn relay_listing_row_summary(listing: &RelayDatasetListingProjection) -> (String, String) {
    let primary = compact_text(
        format!(
            "{} // {} // {}",
            listing.title,
            listing.dataset_kind.as_deref().unwrap_or("dataset"),
            if listing.draft { "draft" } else { "active" }
        )
        .as_str(),
        58,
    );
    let secondary = compact_text(
        format!(
            "{} // access {} // asset {}",
            short_id(listing.coordinate.as_str()),
            listing.access.as_deref().unwrap_or("unspecified"),
            listing
                .linked_asset_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "unlinked".to_string()),
        )
        .as_str(),
        92,
    );
    (primary, secondary)
}

fn relay_offer_row_summary(offer: &RelayDatasetOfferProjection) -> (String, String) {
    let primary = compact_text(
        format!(
            "{} // {} // {}",
            short_id(offer.coordinate.as_str()),
            short_id(offer.listing_coordinate.as_str()),
            offer.status
        )
        .as_str(),
        58,
    );
    let secondary = compact_text(
        format!(
            "price {} {} // grant {} // asset {}",
            offer.price_amount.as_deref().unwrap_or("-"),
            offer.price_currency.as_deref().unwrap_or("-"),
            offer
                .linked_grant_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "unlinked".to_string()),
            offer
                .linked_asset_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "unlinked".to_string()),
        )
        .as_str(),
        92,
    );
    (primary, secondary)
}

fn delivery_row_summary(delivery: &DeliveryBundle) -> (String, String) {
    let primary = compact_text(
        format!(
            "{} // {} // {}",
            short_id(delivery.delivery_bundle_id.as_str()),
            short_id(delivery.grant_id.as_str()),
            delivery.status.label()
        )
        .as_str(),
        58,
    );
    let secondary = compact_text(
        format!(
            "{} // {} // expires {}",
            short_id(delivery.delivery_ref.as_str()),
            delivery
                .bundle_size_bytes
                .map(|value| format!("{value} bytes"))
                .unwrap_or_else(|| "size n/a".to_string()),
            format_timestamp_ms(delivery.expires_at_ms)
        )
        .as_str(),
        76,
    );
    (primary, secondary)
}

fn revocation_row_summary(revocation: &RevocationReceipt) -> (String, String) {
    let primary = compact_text(
        format!(
            "{} // {} // {}",
            short_id(revocation.revocation_id.as_str()),
            short_id(revocation.grant_id.as_str()),
            revocation.status.label()
        )
        .as_str(),
        58,
    );
    let secondary = compact_text(
        format!(
            "{} // refund {} // {} bundle(s)",
            compact_text(revocation.reason_code.as_str(), 18),
            format_money(revocation.refund_amount.as_ref()),
            revocation.revoked_delivery_bundle_ids.len()
        )
        .as_str(),
        76,
    );
    (primary, secondary)
}

fn lifecycle_row_summary(entry: &DataMarketLifecycleEntry) -> (String, String) {
    let primary = compact_text(
        format!(
            "{} // {} // {}",
            compact_text(entry.stage.as_str(), 22),
            short_id(entry.subject_id.as_str()),
            compact_text(entry.status.as_str(), 16)
        )
        .as_str(),
        76,
    );
    let secondary = compact_text(
        format!(
            "{} // policy {} // receipt {} // {}",
            entry
                .counterparty
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "n/a".to_string()),
            entry
                .policy_id
                .as_deref()
                .map(|value| compact_text(value, 16))
                .unwrap_or_else(|| "n/a".to_string()),
            entry
                .receipt_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "n/a".to_string()),
            compact_text(entry.summary.as_str(), 28),
        )
        .as_str(),
        112,
    );
    (primary, secondary)
}

fn format_refresh_time(timestamp_ms: Option<i64>) -> String {
    timestamp_ms
        .and_then(|value| Local.timestamp_millis_opt(value).single())
        .map(|value| value.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "never".to_string())
}

fn format_timestamp_ms(timestamp_ms: Option<i64>) -> String {
    timestamp_ms
        .and_then(|value| Local.timestamp_millis_opt(value).single())
        .map(|value| value.format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "n/a".to_string())
}

fn format_policy(policy: Option<&PermissionPolicy>) -> String {
    match policy {
        Some(policy) => {
            let scope_count = policy.allowed_scopes.len();
            format!(
                "{} // {scope_count} scope(s)",
                compact_text(policy.policy_id.as_str(), 16)
            )
        }
        None => "policy n/a".to_string(),
    }
}

fn asset_packaging_summary(asset: &DataAsset) -> Option<String> {
    if asset_metadata_string(asset, "codex_conversation_export").as_deref() == Some("true") {
        let tier = asset_metadata_string(asset, "codex_redaction_tier")
            .unwrap_or_else(|| "unspecified".to_string());
        let sessions =
            asset_metadata_string(asset, "codex_session_count").unwrap_or_else(|| "?".to_string());
        return Some(compact_text(
            format!("{tier} redaction // {sessions} sessions").as_str(),
            36,
        ));
    }
    asset_metadata_string(asset, "export_kind").map(|value| compact_text(value.as_str(), 36))
}

fn asset_market_posture(asset: &DataAsset) -> String {
    let visibility = asset_metadata_string(asset, "visibility_posture")
        .unwrap_or_else(|| "visibility n/a".to_string());
    let sensitivity = asset_metadata_string(asset, "sensitivity_posture")
        .unwrap_or_else(|| "sensitivity n/a".to_string());
    compact_text(format!("{visibility}/{sensitivity}").as_str(), 24)
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

fn format_money(money: Option<&Money>) -> String {
    match money {
        Some(Money {
            asset: Asset::Btc,
            amount: MoneyAmount::AmountSats(value),
        }) => format!("{value} sats"),
        Some(Money {
            asset: Asset::Btc,
            amount: MoneyAmount::AmountMsats(value),
        }) => format!("{value} msats"),
        Some(Money {
            asset: Asset::UsdCents,
            amount: MoneyAmount::AmountSats(value),
        }) => format!("{value} usd_cents"),
        Some(Money {
            asset: Asset::UsdCents,
            amount: MoneyAmount::AmountMsats(value),
        }) => format!("{value} usd_msats"),
        Some(Money {
            asset: Asset::AssetUnspecified,
            amount: MoneyAmount::AmountSats(value),
        }) => format!("{value} units"),
        Some(Money {
            asset: Asset::AssetUnspecified,
            amount: MoneyAmount::AmountMsats(value),
        }) => format!("{value} milli-units"),
        None => "price n/a".to_string(),
    }
}

fn short_id(value: &str) -> String {
    if value.len() <= 16 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn asset_row_summary_surfaces_codex_export_context() {
        let asset = DataAsset {
            asset_id: "data_asset.provider.codex.demo".to_string(),
            provider_id: "provider.demo".to_string(),
            asset_kind: "conversation_bundle".to_string(),
            title: "Redacted Codex Conversations".to_string(),
            status: openagents_kernel_core::data::DataAssetStatus::Active,
            nostr_publications: Default::default(),
            metadata: json!({
                "visibility_posture": "targeted_only",
                "sensitivity_posture": "private",
                "draft_metadata": {
                    "codex_conversation_export": "true",
                    "codex_redaction_tier": "public",
                    "codex_session_count": "3"
                }
            }),
            ..Default::default()
        };

        let (primary, secondary) = asset_row_summary(&asset);
        assert!(primary.contains("codex export"));
        assert!(secondary.contains("targeted_only/private"));
        assert!(secondary.contains("3 sessions"));
    }
}
