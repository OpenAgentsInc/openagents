use chrono::{Local, TimeZone};
use openagents_kernel_core::data::{AccessGrant, DataAsset, RevocationReceipt};
use serde_json::Value;
use wgpui::{Bounds, PaintContext, Point, Quad, theme};

use crate::app_state::{
    DataBuyerPaneState, DataMarketPaneState, RelayDatasetAccessContractProjection,
    RelayDatasetAccessRequestProjection, RelayDatasetAccessResultProjection,
    RelayDatasetListingProjection, RelayDatasetOfferProjection,
    RelayDatasetSettlementMatchProjection,
};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, split_text_for_display,
};
use crate::pane_system::{
    data_buyer_next_asset_button_bounds, data_buyer_previous_asset_button_bounds,
    data_buyer_publish_button_bounds, data_buyer_refresh_button_bounds,
};
use crate::state::operations::{NetworkRequestsState, SubmittedNetworkRequest};

const PADDING: f32 = 12.0;
const HEADER_BOTTOM: f32 = 146.0;
const CARD_GAP: f32 = 12.0;
const CARD_HEADER_HEIGHT: f32 = 38.0;
const ROW_HEIGHT: f32 = 18.0;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &DataBuyerPaneState,
    market_state: &DataMarketPaneState,
    network_requests: &NetworkRequestsState,
    paint: &mut PaintContext,
) {
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

    let intro_chunk_len = ((content_bounds.size.width - PADDING * 2.0) / 7.0).max(24.0) as usize;
    let mut intro_y = content_bounds.origin.y + 42.0;
    for line in split_text_for_display(
        "Buyer-side DS discovery surface. This pane selects a relay-visible dataset listing, publishes a targeted DS-DVM request, and tracks relay contracts, results, and wallet settlement.",
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

    let mut status_y = intro_y + 6.0;
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
    let mut status_end_y = paint_label_line(
        paint,
        content_bounds.origin.x + PADDING,
        status_y,
        "status line",
        &pane_state.status_line,
    );
    let status_chunk_len = ((content_bounds.size.width - PADDING * 2.0) / 7.0).max(24.0) as usize;
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

    let content_top = (status_end_y + 10.0).max(content_bounds.origin.y + HEADER_BOTTOM);
    let available_height = (content_bounds.max_y() - content_top - PADDING).max(320.0);
    let left_width = ((content_bounds.size.width - PADDING * 2.0 - CARD_GAP) * 0.54).max(360.0);
    let right_width =
        (content_bounds.size.width - PADDING * 2.0 - CARD_GAP - left_width).max(280.0);
    let top_height = (available_height * 0.40).max(220.0);
    let bottom_height = buyer_truth_min_height()
        .max(available_height - top_height - CARD_GAP)
        .max(280.0);

    let viewport = scroll_viewport_bounds_with_top(content_bounds, content_top);
    let content_height = content_height(top_height, bottom_height);
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = pane_state.scroll_offset_px.min(max_scroll);

    let asset_bounds = Bounds::new(
        content_bounds.origin.x + PADDING,
        content_top - scroll_offset,
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

    let selected_listing = pane_state.selected_listing(market_state);
    let selected_catalog_offer = pane_state.selected_catalog_offer(market_state);
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
    let relay_request = pane_state.latest_relay_request(market_state);
    let relay_contract = pane_state.latest_relay_access_contract(market_state);
    let relay_result = pane_state.latest_relay_result(market_state);
    let relay_settlement = pane_state.latest_wallet_settlement(market_state);

    paint.scene.push_clip(viewport);
    paint_asset_card(
        asset_bounds,
        selected_listing,
        selected_catalog_offer,
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
        relay_request,
        relay_contract,
        relay_result,
        relay_settlement,
        paint,
    );
    paint.scene.pop_clip();
    paint_scrollbar(viewport, content_height, scroll_offset, paint);
}

pub fn scroll_viewport_bounds(content_bounds: Bounds) -> Bounds {
    scroll_viewport_bounds_with_top(content_bounds, content_bounds.origin.y + HEADER_BOTTOM)
}

fn scroll_viewport_bounds_with_top(content_bounds: Bounds, content_top: f32) -> Bounds {
    let top = content_top;
    Bounds::new(
        content_bounds.origin.x + 8.0,
        top,
        (content_bounds.size.width - 16.0).max(1.0),
        (content_bounds.max_y() - top - 8.0).max(1.0),
    )
}

fn content_height(top_height: f32, bottom_height: f32) -> f32 {
    top_height + bottom_height + CARD_GAP + PADDING * 2.0
}

fn buyer_truth_min_height() -> f32 {
    let rows = 11.0;
    CARD_HEADER_HEIGHT + 16.0 + (rows * ROW_HEIGHT) + 18.0
}

fn paint_asset_card(
    bounds: Bounds,
    listing: Option<&RelayDatasetListingProjection>,
    catalog_offer: Option<&RelayDatasetOfferProjection>,
    asset: Option<&DataAsset>,
    grant: Option<&AccessGrant>,
    revocation: Option<&RevocationReceipt>,
    paint: &mut PaintContext,
) {
    paint_card(
        bounds,
        "Selected Dataset",
        "Relay listing with optional compatibility bridge metadata",
        paint,
    );
    if listing.is_none() && asset.is_none() {
        paint.scene.draw_text(paint.text.layout(
            "No relay listing is selected yet. Refresh the relay catalog or publish a seller listing first.",
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + CARD_HEADER_HEIGHT + 12.0),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    let clip = Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + CARD_HEADER_HEIGHT + 4.0,
        (bounds.size.width - 16.0).max(1.0),
        (bounds.size.height - CARD_HEADER_HEIGHT - 10.0).max(1.0),
    );
    paint.scene.push_clip(clip);
    let mut row_y = bounds.origin.y + CARD_HEADER_HEIGHT + 12.0;
    let title = listing
        .map(|listing| compact_text(listing.title.as_str(), 44))
        .or_else(|| asset.map(|asset| compact_text(asset.title.as_str(), 44)))
        .unwrap_or_else(|| "unavailable".to_string());
    let listing_ref = listing
        .map(|listing| compact_text(listing.coordinate.as_str(), 44))
        .unwrap_or_else(|| "none".to_string());
    let provider = listing
        .map(|listing| compact_text(listing.publisher_pubkey.as_str(), 44))
        .or_else(|| asset.map(|asset| compact_text(asset.provider_id.as_str(), 44)))
        .unwrap_or_else(|| "unknown".to_string());
    let linked_asset = asset
        .map(|asset| {
            compact_text(
                format!(
                    "{} // {} // {}",
                    short_id(asset.asset_id.as_str()),
                    asset.asset_kind,
                    asset.status.label()
                )
                .as_str(),
                44,
            )
        })
        .unwrap_or_else(|| "none".to_string());
    let pricing = grant
        .and_then(|grant| grant.offer_price.as_ref().map(format_money))
        .or_else(|| {
            catalog_offer.map(|offer| {
                format!(
                    "{} {}",
                    offer.price_amount.as_deref().unwrap_or("-"),
                    offer.price_currency.as_deref().unwrap_or("-"),
                )
            })
        })
        .or_else(|| {
            listing.and_then(|listing| {
                listing
                    .classified_price_amount
                    .as_deref()
                    .zip(listing.classified_price_currency.as_deref())
                    .map(|(amount, currency)| format!("{amount} {currency}"))
            })
        })
        .or_else(|| {
            listing.and_then(|listing| {
                listing
                    .storefront_product_price_amount
                    .as_deref()
                    .zip(listing.storefront_product_price_currency.as_deref())
                    .map(|(amount, currency)| format!("{amount} {currency}"))
            })
        })
        .or_else(|| asset.and_then(|asset| asset.price_hint.as_ref().map(format_money)))
        .unwrap_or_else(|| "none".to_string());
    for (label, value) in [
        ("title", title),
        ("listing", listing_ref),
        ("provider", provider),
        ("bridge_asset", linked_asset),
        ("pricing", compact_text(pricing.as_str(), 44)),
        (
            "market",
            asset
                .map(asset_market_posture)
                .or_else(|| listing.and_then(|listing| listing.access.clone()))
                .unwrap_or_else(|| "relay_only".to_string()),
        ),
    ] {
        row_y = paint_row(bounds, row_y, label, value.as_str(), paint);
    }
    if let Some(offer) = catalog_offer {
        row_y = paint_row(
            bounds,
            row_y,
            "relay_offer",
            compact_text(offer.coordinate.as_str(), 44).as_str(),
            paint,
        );
    }
    if let Some(listing) = listing {
        row_y = paint_row(
            bounds,
            row_y,
            "catalog",
            compact_text(
                listing.classified_coordinate.as_deref().unwrap_or("none"),
                44,
            )
            .as_str(),
            paint,
        );
        row_y = paint_row(
            bounds,
            row_y,
            "discussion",
            compact_text(
                listing
                    .discussion_channel_id
                    .as_deref()
                    .or(listing.discussion_channel_name.as_deref())
                    .unwrap_or("none"),
                44,
            )
            .as_str(),
            paint,
        );
        row_y = paint_row(
            bounds,
            row_y,
            "storefront",
            compact_text(
                listing
                    .storefront_product_coordinate
                    .as_deref()
                    .or(listing.storefront_stall_name.as_deref())
                    .unwrap_or("none"),
                44,
            )
            .as_str(),
            paint,
        );
    }
    if let Some(grant) = grant {
        row_y = paint_row(bounds, row_y, "bridge_grant", grant.grant_id.as_str(), paint);
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
    if let Some(offer) = catalog_offer {
        row_y = paint_row(
            bounds,
            row_y,
            "offer_chat",
            compact_text(
                offer
                    .discussion_channel_id
                    .as_deref()
                    .or(offer.discussion_channel_name.as_deref())
                    .unwrap_or("none"),
                44,
            )
            .as_str(),
            paint,
        );
        row_y = paint_row(
            bounds,
            row_y,
            "offer_storefront",
            compact_text(
                offer
                    .storefront_product_coordinate
                    .as_deref()
                    .or(offer.storefront_stall_name.as_deref())
                    .unwrap_or("none"),
                44,
            )
            .as_str(),
            paint,
        );
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
        "bridge_revoke",
        revocation_label.as_str(),
        paint,
    );
    paint.scene.pop_clip();
}

fn paint_draft_card(
    bounds: Bounds,
    draft: Option<&crate::app_state::DataBuyerRequestDraft>,
    paint: &mut PaintContext,
) {
    paint_card(
        bounds,
        "Request Draft",
        "Derived from the selected relay listing and offer terms",
        paint,
    );
    let Some(draft) = draft else {
        paint.scene.draw_text(paint.text.layout(
            "No request draft is available because there is no relay listing selection.",
            Point::new(
                bounds.origin.x + 10.0,
                bounds.origin.y + CARD_HEADER_HEIGHT + 12.0,
            ),
            11.0,
            theme::text::MUTED,
        ));
        return;
    };

    let clip = Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + CARD_HEADER_HEIGHT + 4.0,
        (bounds.size.width - 16.0).max(1.0),
        (bounds.size.height - CARD_HEADER_HEIGHT - 10.0).max(1.0),
    );
    paint.scene.push_clip(clip);
    let mut row_y = bounds.origin.y + CARD_HEADER_HEIGHT + 12.0;
    for (label, value) in [
        ("provider", compact_text(draft.provider_id.as_str(), 56)),
        ("asset_ref", compact_text(draft.asset_ref.as_str(), 56)),
        ("bridge_asset", compact_text(draft.asset_id.as_str(), 56)),
        (
            "listing",
            draft
                .listing_coordinate
                .clone()
                .map(|value| compact_text(value.as_str(), 56))
                .unwrap_or_else(|| "none".to_string()),
        ),
        (
            "bridge_grant",
            draft
                .offer_grant_id
                .clone()
                .map(|value| compact_text(value.as_str(), 56))
                .unwrap_or_else(|| "none".to_string()),
        ),
        (
            "offer",
            draft
                .offer_coordinate
                .clone()
                .map(|value| compact_text(value.as_str(), 56))
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
    paint.scene.pop_clip();
}

fn paint_truth_card(
    bounds: Bounds,
    pane_state: &DataBuyerPaneState,
    latest_request: Option<&SubmittedNetworkRequest>,
    relay_request: Option<&RelayDatasetAccessRequestProjection>,
    relay_contract: Option<&RelayDatasetAccessContractProjection>,
    relay_result: Option<&RelayDatasetAccessResultProjection>,
    relay_settlement: Option<&RelayDatasetSettlementMatchProjection>,
    paint: &mut PaintContext,
) {
    paint_card(
        bounds,
        "Buyer Truth",
        "Published request plus relay contract, result, and wallet posture",
        paint,
    );
    let clip = Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + CARD_HEADER_HEIGHT + 4.0,
        (bounds.size.width - 16.0).max(1.0),
        (bounds.size.height - CARD_HEADER_HEIGHT - 10.0).max(1.0),
    );
    paint.scene.push_clip(clip);
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
        "local_status",
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
        "relay_request",
        relay_request
            .map(|request| request.event_id.as_str())
            .unwrap_or("none"),
        paint,
    );
    row_y = paint_row(
        bounds,
        row_y,
        "contract",
        relay_contract
            .map(|contract| contract.coordinate.as_str())
            .unwrap_or("none"),
        paint,
    );
    row_y = paint_row(
        bounds,
        row_y,
        "contract_status",
        relay_contract
            .map(|contract| contract.status.as_str())
            .unwrap_or("none"),
        paint,
    );
    row_y = paint_row(
        bounds,
        row_y,
        "result_event",
        relay_result
            .map(|result| result.event_id.as_str())
            .or_else(|| latest_request.and_then(|request| request.last_result_event_id.as_deref()))
            .unwrap_or("none"),
        paint,
    );
    row_y = paint_row(
        bounds,
        row_y,
        "payment_hash",
        relay_contract
            .and_then(|contract| contract.payment_hash.as_deref())
            .or_else(|| relay_result.and_then(|result| result.payment_hash.as_deref()))
            .unwrap_or("none"),
        paint,
    );
    row_y = paint_row(
        bounds,
        row_y,
        "wallet_settle",
        relay_settlement
            .map(|settlement| settlement.status.as_str())
            .unwrap_or("none"),
        paint,
    );
    let _ = paint_row(
        bounds,
        row_y,
        "delivery_digest",
        relay_result
            .and_then(|result| result.delivery_digest.as_deref())
            .or_else(|| relay_contract.and_then(|contract| contract.delivery_digest.as_deref()))
            .unwrap_or("none"),
        paint,
    );
    paint.scene.pop_clip();
}

fn paint_card(bounds: Bounds, title: &str, subtitle: &str, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 8.0),
        11.0,
        theme::text::SECONDARY,
    ));
    let chunk_len = ((bounds.size.width - 20.0) / 7.0).max(16.0) as usize;
    let mut subtitle_y = bounds.origin.y + 21.0;
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

fn paint_row(
    bounds: Bounds,
    row_y: f32,
    label: &str,
    value: &str,
    paint: &mut PaintContext,
) -> f32 {
    let value_chunk_len = ((bounds.max_x() - (bounds.origin.x + 128.0) - 10.0) / 7.0).max(14.0)
        as usize;
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 10.0, row_y),
        10.0,
        theme::text::MUTED,
    ));
    let mut y = row_y;
    for line in split_text_for_display(value, value_chunk_len).into_iter().take(2) {
        paint.scene.draw_text(paint.text.layout(
            line.as_str(),
            Point::new(bounds.origin.x + 128.0, y),
            11.0,
            theme::text::PRIMARY,
        ));
        y += 12.0;
    }
    y + 6.0
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
