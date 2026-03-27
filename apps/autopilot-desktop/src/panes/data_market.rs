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
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_source_badge, split_text_for_display,
};
use crate::pane_system::data_market_refresh_button_bounds;

const PADDING: f32 = 12.0;
const METRIC_TOP_BASE: f32 = 92.0;
const METRIC_HEIGHT: f32 = 56.0;
const METRIC_GAP: f32 = 10.0;
const LIFECYCLE_HEIGHT: f32 = 92.0;
const PANEL_GAP: f32 = 12.0;
const PANEL_HEADER_HEIGHT: f32 = 24.0;
const ROW_HEIGHT: f32 = 38.0;
const MAX_ROWS_PER_PANEL: usize = 4;
const MAX_LIFECYCLE_ROWS: usize = 3;

pub fn paint(content_bounds: Bounds, pane_state: &DataMarketPaneState, paint: &mut PaintContext) {
    paint_source_badge(content_bounds, "relay.ds_market.v1", paint);
    paint_action_button(
        data_market_refresh_button_bounds(content_bounds),
        "Refresh",
        paint,
    );

    let intro_chunk_len = ((content_bounds.size.width - PADDING * 2.0) / 6.2).max(28.0) as usize;
    let mut intro_y = content_bounds.origin.y + 42.0;
    for line in split_text_for_display(
        "Relay-native dataset market view: DS listings, offers, access contracts, request/result activity, and local wallet settlement matches.",
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
        "last refresh",
        &format_refresh_time(pane_state.last_refreshed_at_ms),
    );
    let mut status_end_y;
    let status_chunk_len = ((content_bounds.size.width - PADDING * 2.0) / 7.0).max(22.0) as usize;
    if let Some(action) = pane_state.last_action.as_deref() {
        status_end_y = paint_label_line(
            paint,
            content_bounds.origin.x + PADDING,
            status_y,
            "last action",
            "",
        );
        for line in split_text_for_display(action, status_chunk_len).into_iter().take(2) {
            paint.scene.draw_text(paint.text.layout(
                line.as_str(),
                Point::new(content_bounds.origin.x + PADDING + 72.0, status_end_y - 15.0),
                11.0,
                theme::text::SECONDARY,
            ));
            status_end_y += 14.0;
        }
    } else {
        status_end_y = status_y;
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
    let metric_top = compute_metric_top(content_bounds, pane_state);
    let lifecycle_top = metric_top + METRIC_HEIGHT + METRIC_GAP + 2.0;
    let panels_top = lifecycle_top + LIFECYCLE_HEIGHT + PANEL_GAP;

    let viewport = scroll_viewport_bounds(content_bounds, metric_top);
    let content_height = content_height(content_bounds, metric_top, panels_top);
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = pane_state.scroll_offset_px.min(max_scroll);
    paint.scene.push_clip(viewport);
    paint_metric_cards(content_bounds, pane_state, metric_top, scroll_offset, paint);
    paint_lifecycle_panel(content_bounds, pane_state, lifecycle_top, scroll_offset, paint);
    paint_panels(content_bounds, pane_state, panels_top, scroll_offset, paint);
    paint.scene.pop_clip();
    paint_scrollbar(viewport, content_height, scroll_offset, paint);
}

pub fn compute_metric_top(content_bounds: Bounds, pane_state: &DataMarketPaneState) -> f32 {
    let intro_chunk_len = ((content_bounds.size.width - PADDING * 2.0) / 6.2).max(28.0) as usize;
    let intro_lines = split_text_for_display(
        "Relay-native dataset market view: DS listings, offers, access contracts, request/result activity, and local wallet settlement matches.",
        intro_chunk_len,
    )
    .len() as f32;
    let mut status_end_y = content_bounds.origin.y + 42.0 + intro_lines * 14.0 + 4.0;
    status_end_y += 15.0; // status
    status_end_y += 15.0; // last refresh
    if pane_state.last_action.is_some() {
        status_end_y += 15.0; // last action label row
        status_end_y += 14.0 * pane_state.last_action.as_ref().map_or(0.0, |action| {
            split_text_for_display(
                action,
                ((content_bounds.size.width - PADDING * 2.0) / 7.0).max(22.0) as usize,
            )
            .len()
            .min(2) as f32
        });
    }
    if let Some(error) = pane_state.last_error.as_ref() {
        status_end_y += 14.0
            * split_text_for_display(
                error,
                ((content_bounds.size.width - PADDING * 2.0) / 7.0).max(22.0) as usize,
            )
            .len()
            .min(2) as f32;
    }
    (status_end_y + 10.0).max(content_bounds.origin.y + METRIC_TOP_BASE)
}

pub fn scroll_viewport_bounds(content_bounds: Bounds, metric_top: f32) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + 8.0,
        metric_top - 8.0,
        (content_bounds.size.width - 16.0).max(1.0),
        (content_bounds.max_y() - metric_top - 8.0).max(1.0),
    )
}

fn content_height(content_bounds: Bounds, metric_top: f32, panels_top: f32) -> f32 {
    let panel_height = ((content_bounds.max_y() - panels_top - PADDING - PANEL_GAP) / 2.0).max(120.0);
    let bottom_y = panels_top + panel_height * 2.0 + PANEL_GAP;
    (bottom_y - (metric_top - 8.0) + PADDING).max(0.0)
}

fn paint_metric_cards(
    content_bounds: Bounds,
    pane_state: &DataMarketPaneState,
    metric_top: f32,
    scroll_offset: f32,
    paint: &mut PaintContext,
) {
    let card_width =
        ((content_bounds.size.width - PADDING * 2.0 - METRIC_GAP * 3.0) / 4.0).max(100.0);
    let metrics = [
        ("Listings", pane_state.relay_listings.len()),
        ("Offers", pane_state.relay_offers.len()),
        ("Contracts", pane_state.relay_access_contracts.len()),
        ("Wallet", pane_state.relay_settlement_matches.len()),
    ];
    for (index, (label, count)) in metrics.iter().enumerate() {
        let bounds = Bounds::new(
            content_bounds.origin.x + PADDING + index as f32 * (card_width + METRIC_GAP),
            metric_top - scroll_offset,
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
    panels_top: f32,
    scroll_offset: f32,
    paint: &mut PaintContext,
) {
    let panel_width = ((content_bounds.size.width - PADDING * 2.0 - PANEL_GAP) / 2.0).max(180.0);
    let panel_height = ((content_bounds.max_y() - panels_top - PADDING - PANEL_GAP) / 2.0).max(120.0);
    let left_x = content_bounds.origin.x + PADDING;
    let right_x = left_x + panel_width + PANEL_GAP;
    let top_y = panels_top - scroll_offset;
    let bottom_y = top_y + panel_height + PANEL_GAP;

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
        "Access Contracts",
        pane_state.load_state,
        pane_state.relay_access_contracts.len(),
        pane_state
            .relay_access_contracts
            .iter()
            .take(MAX_ROWS_PER_PANEL)
            .map(relay_contract_row_summary)
            .collect::<Vec<_>>(),
        paint,
    );
    paint_panel(
        Bounds::new(right_x, bottom_y, panel_width, panel_height),
        "Wallet Matches",
        pane_state.load_state,
        pane_state.relay_settlement_matches.len(),
        pane_state
            .relay_settlement_matches
            .iter()
            .take(MAX_ROWS_PER_PANEL)
            .map(relay_settlement_row_summary)
            .collect::<Vec<_>>(),
        paint,
    );
}

fn paint_lifecycle_panel(
    content_bounds: Bounds,
    pane_state: &DataMarketPaneState,
    lifecycle_top: f32,
    scroll_offset: f32,
    paint: &mut PaintContext,
) {
    let bounds = Bounds::new(
        content_bounds.origin.x + PADDING,
        lifecycle_top - scroll_offset,
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
        "Recent relay-native request, contract, result, and settlement activity.",
        Point::new(bounds.origin.x + 70.0, bounds.origin.y + 8.0),
        10.0,
        theme::text::MUTED,
    ));

    let rows = relay_activity_row_summaries(pane_state);
    if rows.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No relay activity recorded yet. Listings, requests, contracts, results, and wallet matches will appear here.",
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + 32.0),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    let lifecycle_clip = Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + PANEL_HEADER_HEIGHT + 6.0,
        (bounds.size.width - 16.0).max(1.0),
        (bounds.size.height - PANEL_HEADER_HEIGHT - 12.0).max(1.0),
    );
    paint.scene.push_clip(lifecycle_clip);
    for (index, (primary, secondary)) in rows.into_iter().enumerate() {
        let row_y = bounds.origin.y + 30.0 + index as f32 * 22.0;
        let primary_chunk = ((bounds.size.width - 20.0) / 7.0).max(16.0) as usize;
        for (line_idx, line) in split_text_for_display(primary.as_str(), primary_chunk)
            .into_iter()
            .take(1)
            .enumerate()
        {
            paint.scene.draw_text(paint.text.layout(
                line.as_str(),
                Point::new(bounds.origin.x + 10.0, row_y + line_idx as f32 * 11.0),
                11.0,
                theme::text::PRIMARY,
            ));
        }
        let secondary_chunk = ((bounds.size.width - 20.0) / 7.0).max(16.0) as usize;
        for (line_idx, line) in split_text_for_display(secondary.as_str(), secondary_chunk)
            .into_iter()
            .take(1)
            .enumerate()
        {
            paint.scene.draw_text(paint.text.layout_mono(
                line.as_str(),
                Point::new(bounds.origin.x + 10.0, row_y + 12.0 + line_idx as f32 * 10.0),
                10.0,
                theme::text::MUTED,
            ));
        }
    }
    paint.scene.pop_clip();
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
            PaneLoadState::Loading => "Refreshing DS relay catalog...",
            PaneLoadState::Error => "Refresh failed or no relay data is available.",
            PaneLoadState::Ready => "No relay rows loaded yet.",
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
    let panel_clip = Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + PANEL_HEADER_HEIGHT + 4.0,
        (bounds.size.width - 16.0).max(1.0),
        (bounds.size.height - PANEL_HEADER_HEIGHT - 10.0).max(1.0),
    );
    paint.scene.push_clip(panel_clip);
    for (index, (primary, secondary)) in rows.iter().enumerate() {
        let row_y = row_origin_y + index as f32 * ROW_HEIGHT;
        let primary_chunk = ((bounds.size.width - 20.0) / 7.0).max(16.0) as usize;
        for (line_idx, line) in split_text_for_display(primary.as_str(), primary_chunk)
            .into_iter()
            .take(1)
            .enumerate()
        {
            paint.scene.draw_text(paint.text.layout(
                line.as_str(),
                Point::new(bounds.origin.x + 10.0, row_y + line_idx as f32 * 12.0),
                11.0,
                theme::text::PRIMARY,
            ));
        }
        let secondary_chunk = ((bounds.size.width - 20.0) / 7.0).max(16.0) as usize;
        for (line_idx, line) in split_text_for_display(secondary.as_str(), secondary_chunk)
            .into_iter()
            .take(1)
            .enumerate()
        {
            paint.scene.draw_text(paint.text.layout_mono(
                line.as_str(),
                Point::new(bounds.origin.x + 10.0, row_y + 16.0 + line_idx as f32 * 10.0),
                10.0,
                theme::text::MUTED,
            ));
        }
    }

    if total_count > rows.len() {
        paint.scene.draw_text(paint.text.layout(
            &format!("+ {} more", total_count - rows.len()),
            Point::new(bounds.origin.x + 10.0, bounds.max_y() - 18.0),
            10.0,
            theme::text::MUTED,
        ));
    }
    paint.scene.pop_clip();
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
            "{} // access {} // asset {} // catalog {}{} // storefront {} // chat {}",
            short_id(listing.coordinate.as_str()),
            listing.access.as_deref().unwrap_or("unspecified"),
            listing
                .linked_asset_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "unlinked".to_string()),
            listing
                .classified_coordinate
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "none".to_string()),
            listing
                .classified_price_amount
                .as_deref()
                .zip(listing.classified_price_currency.as_deref())
                .map(|(amount, currency)| format!(" // {amount} {currency}"))
                .unwrap_or_default(),
            listing
                .storefront_product_coordinate
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "none".to_string()),
            listing
                .discussion_channel_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "none".to_string()),
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
            "price {} {} // grant {} // asset {} // catalog {} // storefront {} // chat {}",
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
            offer
                .classified_coordinate
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "none".to_string()),
            offer
                .storefront_product_coordinate
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "none".to_string()),
            offer
                .discussion_channel_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "none".to_string()),
        )
        .as_str(),
        92,
    );
    (primary, secondary)
}

fn relay_contract_row_summary(
    contract: &crate::app_state::RelayDatasetAccessContractProjection,
) -> (String, String) {
    let primary = compact_text(
        format!(
            "{} // {} // {}",
            short_id(contract.coordinate.as_str()),
            short_id(contract.listing_coordinate.as_str()),
            compact_text(contract.status.as_str(), 18)
        )
        .as_str(),
        58,
    );
    let secondary = compact_text(
        format!(
            "request {} // result {} // amount {} // hash {}",
            short_id(contract.request_event_id.as_str()),
            contract
                .result_event_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "pending".to_string()),
            contract
                .amount_msats
                .map(|amount| format!("{amount} msats"))
                .unwrap_or_else(|| "n/a".to_string()),
            contract
                .payment_hash
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "n/a".to_string()),
        )
        .as_str(),
        92,
    );
    (primary, secondary)
}

fn relay_settlement_row_summary(
    settlement: &crate::app_state::RelayDatasetSettlementMatchProjection,
) -> (String, String) {
    let primary = compact_text(
        format!(
            "{} // {} sats // {}",
            compact_text(settlement.status.as_str(), 18),
            settlement.amount_sats,
            compact_text(settlement.direction.as_str(), 12)
        )
        .as_str(),
        58,
    );
    let secondary = compact_text(
        format!(
            "pointer {} // hash {} // contract {} // result {}",
            short_id(settlement.payment_pointer.as_str()),
            short_id(settlement.payment_hash.as_str()),
            settlement
                .contract_coordinate
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "n/a".to_string()),
            settlement
                .result_event_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "n/a".to_string()),
        )
        .as_str(),
        92,
    );
    (primary, secondary)
}

fn relay_activity_row_summaries(pane_state: &DataMarketPaneState) -> Vec<(String, String)> {
    let mut rows = Vec::<(u64, String, String)>::new();

    for request in pane_state.relay_requests.iter().take(MAX_LIFECYCLE_ROWS) {
        rows.push((
            request.created_at_seconds,
            compact_text(
                format!(
                    "request // {} // {}",
                    short_id(request.event_id.as_str()),
                    short_id(request.listing_coordinate.as_str())
                )
                .as_str(),
                76,
            ),
            compact_text(
                format!(
                    "{} // provider {} // bid {}",
                    compact_text(request.delivery_mode.as_str(), 18),
                    request
                        .targeted_provider_pubkeys
                        .first()
                        .map(|value| short_id(value.as_str()))
                        .unwrap_or_else(|| "open".to_string()),
                    request
                        .bid_msats
                        .map(|value| format!("{value} msats"))
                        .unwrap_or_else(|| "n/a".to_string()),
                )
                .as_str(),
                112,
            ),
        ));
    }
    for contract in pane_state.relay_access_contracts.iter().take(MAX_LIFECYCLE_ROWS) {
        rows.push((
            contract.created_at_seconds,
            compact_text(
                format!(
                    "contract // {} // {}",
                    short_id(contract.coordinate.as_str()),
                    compact_text(contract.status.as_str(), 18)
                )
                .as_str(),
                76,
            ),
            compact_text(
                format!(
                    "buyer {} // request {} // hash {}",
                    short_id(contract.buyer_pubkey.as_str()),
                    short_id(contract.request_event_id.as_str()),
                    contract
                        .payment_hash
                        .as_deref()
                        .map(short_id)
                        .unwrap_or_else(|| "n/a".to_string()),
                )
                .as_str(),
                112,
            ),
        ));
    }
    for result in pane_state.relay_results.iter().take(MAX_LIFECYCLE_ROWS) {
        rows.push((
            result.created_at_seconds,
            compact_text(
                format!(
                    "result // {} // {}",
                    short_id(result.event_id.as_str()),
                    short_id(result.request_event_id.as_str())
                )
                .as_str(),
                76,
            ),
            compact_text(
                format!(
                    "{} // digest {} // hash {}",
                    compact_text(result.delivery_mode.as_str(), 18),
                    result
                        .delivery_digest
                        .as_deref()
                        .map(short_id)
                        .unwrap_or_else(|| "n/a".to_string()),
                    result
                        .payment_hash
                        .as_deref()
                        .map(short_id)
                        .unwrap_or_else(|| "n/a".to_string()),
                )
                .as_str(),
                112,
            ),
        ));
    }
    for settlement in pane_state
        .relay_settlement_matches
        .iter()
        .take(MAX_LIFECYCLE_ROWS)
    {
        rows.push((
            settlement.observed_at_seconds,
            compact_text(
                format!(
                    "wallet // {} // {} sats",
                    compact_text(settlement.status.as_str(), 18),
                    settlement.amount_sats
                )
                .as_str(),
                76,
            ),
            compact_text(
                format!(
                    "{} // hash {} // request {}",
                    compact_text(settlement.direction.as_str(), 18),
                    short_id(settlement.payment_hash.as_str()),
                    settlement
                        .request_event_id
                        .as_deref()
                        .map(short_id)
                        .unwrap_or_else(|| "n/a".to_string()),
                )
                .as_str(),
                112,
            ),
        ));
    }

    rows.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    rows.into_iter()
        .take(MAX_LIFECYCLE_ROWS)
        .map(|(_, primary, secondary)| (primary, secondary))
        .collect()
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
