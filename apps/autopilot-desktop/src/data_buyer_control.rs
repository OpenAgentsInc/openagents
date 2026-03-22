use std::str::FromStr;

use nostr::nip90::{
    DataVendingDeliveryMode, DataVendingPreviewPosture, DataVendingRequest,
    create_data_vending_request_event,
};
use nostr::{Event, EventTemplate, NostrIdentity};
use serde_json::json;

use crate::app_state::{
    DATA_MARKET_BUYER_REQUEST_TYPE, DataBuyerRequestDraft,
    OPENAGENTS_DATA_VENDING_LOCAL_REQUEST_KIND, RenderState,
};
use crate::nip90_compute_semantics::normalize_pubkey;

fn parse_private_key_hex(private_key_hex: &str) -> Result<[u8; 32], String> {
    let key_bytes = hex::decode(private_key_hex.trim())
        .map_err(|error| format!("invalid identity private_key_hex: {error}"))?;
    if key_bytes.len() != 32 {
        return Err(format!(
            "invalid identity private_key_hex length {}, expected 32 bytes",
            key_bytes.len()
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(key_bytes.as_slice());
    Ok(key)
}

fn sign_event_template(
    identity: &NostrIdentity,
    template: &EventTemplate,
) -> Result<Event, String> {
    let private_key = parse_private_key_hex(identity.private_key_hex.as_str())?;
    nostr::finalize_event(template, &private_key)
        .map_err(|error| format!("Cannot sign NIP-90 event template: {error}"))
}

fn build_data_buyer_request_payload(
    draft: &DataBuyerRequestDraft,
    buyer_id: Option<&str>,
) -> String {
    json!({
        "request_type": DATA_MARKET_BUYER_REQUEST_TYPE,
        "asset_ref": draft.asset_ref,
        "asset_id": draft.asset_id,
        "ds_listing_coordinate": draft.listing_coordinate,
        "target_provider_pubkey": draft.provider_id,
        "grant_id": draft.offer_grant_id,
        "ds_offer_coordinate": draft.offer_coordinate,
        "permission_scopes": draft.permission_scopes,
        "delivery_mode": draft.delivery_mode,
        "preview_posture": draft.preview_posture,
        "bid_sats": draft.bid_sats,
        "timeout_seconds": draft.timeout_seconds,
        "buyer_id": buyer_id,
        "targeting_posture": "targeted_only",
    })
    .to_string()
}

fn build_data_buyer_request_event(
    identity: &NostrIdentity,
    relay_urls: &[String],
    draft: &DataBuyerRequestDraft,
    buyer_id: Option<&str>,
) -> Result<Event, String> {
    let first_scope = draft
        .permission_scopes
        .first()
        .cloned()
        .unwrap_or_else(|| "targeted_request".to_string());
    let mut request = DataVendingRequest::new(
        OPENAGENTS_DATA_VENDING_LOCAL_REQUEST_KIND,
        draft.asset_ref.as_str(),
        first_scope,
    )
    .map_err(|error| format!("Cannot build data-vending request: {error}"))?
    .with_delivery_mode(
        DataVendingDeliveryMode::from_str(draft.delivery_mode.as_str()).unwrap_or_default(),
    )
    .with_preview_posture(
        DataVendingPreviewPosture::from_str(draft.preview_posture.as_str())
            .unwrap_or(DataVendingPreviewPosture::MetadataOnly),
    )
    .with_bid(draft.bid_sats.saturating_mul(1000))
    .with_content(build_data_buyer_request_payload(draft, buyer_id));
    for scope in draft.permission_scopes.iter().skip(1) {
        request = request.add_scope(scope.clone());
    }

    let normalized_relays = relay_urls
        .iter()
        .map(|relay| relay.trim().to_string())
        .filter(|relay| !relay.is_empty())
        .collect::<Vec<_>>();
    if normalized_relays.is_empty() {
        return Err(
            "Cannot publish data-access request: no relay URLs configured for request publication"
                .to_string(),
        );
    }
    for relay in normalized_relays {
        request = request.add_relay(relay);
    }
    request = request.add_service_provider(normalize_pubkey(draft.provider_id.as_str()));

    let template = create_data_vending_request_event(&request)
        .map_err(|error| format!("Cannot build data-vending request event: {error}"))?;
    sign_event_template(identity, &template)
}

pub(crate) fn open_data_buyer_pane(state: &mut RenderState) -> bool {
    let buyer_id = crate::kernel_control::provider_id_for_state(state);
    state.data_buyer.configure_local_buyer_id(buyer_id);
    state.data_buyer.mark_opened();
    refresh_data_buyer_market(state)
}

pub(crate) fn refresh_data_buyer_market(state: &mut RenderState) -> bool {
    let buyer_id = crate::kernel_control::provider_id_for_state(state);
    state.data_buyer.configure_local_buyer_id(buyer_id);
    crate::data_market_control::refresh_data_market_snapshot(state);
    state.data_buyer.sync_selection(&state.data_market);
    true
}

pub(crate) fn select_previous_data_buyer_asset(state: &mut RenderState) -> bool {
    let buyer_id = crate::kernel_control::provider_id_for_state(state);
    state.data_buyer.configure_local_buyer_id(buyer_id);
    state.data_buyer.sync_selection(&state.data_market);
    state.data_buyer.select_previous_asset(&state.data_market)
}

pub(crate) fn select_next_data_buyer_asset(state: &mut RenderState) -> bool {
    let buyer_id = crate::kernel_control::provider_id_for_state(state);
    state.data_buyer.configure_local_buyer_id(buyer_id);
    state.data_buyer.sync_selection(&state.data_market);
    state.data_buyer.select_next_asset(&state.data_market)
}

pub(crate) fn select_data_buyer_asset(
    state: &mut RenderState,
    asset_id: &str,
) -> Result<bool, String> {
    let buyer_id = crate::kernel_control::provider_id_for_state(state);
    state.data_buyer.configure_local_buyer_id(buyer_id);
    state.data_buyer.sync_selection(&state.data_market);
    state
        .data_buyer
        .select_asset_by_id(&state.data_market, asset_id)?;
    Ok(true)
}

pub(crate) fn publish_data_buyer_request(state: &mut RenderState) -> bool {
    let buyer_id = crate::kernel_control::provider_id_for_state(state);
    state.data_buyer.configure_local_buyer_id(buyer_id.clone());
    state.data_buyer.sync_selection(&state.data_market);

    let Some(draft) = state.data_buyer.derived_request_draft(&state.data_market) else {
        state.data_buyer.record_publish_error(
            "No active data asset is selected for buyer request publication.",
        );
        return true;
    };
    let Some(identity) = state.nostr_identity.as_ref() else {
        state.data_buyer.record_publish_error(
            "Cannot publish data-access request: Nostr identity unavailable.",
        );
        return true;
    };

    let event = match build_data_buyer_request_event(
        identity,
        state.configured_provider_relay_urls().as_slice(),
        &draft,
        Some(buyer_id.as_str()),
    ) {
        Ok(event) => event,
        Err(error) => {
            state.data_buyer.record_publish_error(error);
            return true;
        }
    };
    let published_event_id = event.id.clone();
    let payload = build_data_buyer_request_payload(&draft, Some(buyer_id.as_str()));
    match crate::input::actions::submit_signed_network_request_with_event(
        state,
        DATA_MARKET_BUYER_REQUEST_TYPE.to_string(),
        payload,
        None,
        None,
        draft.bid_sats,
        draft.timeout_seconds,
        vec![draft.provider_id.clone()],
        event,
    ) {
        Ok(request_id) => {
            state.data_buyer.note_request_published(
                request_id,
                published_event_id,
                draft.provider_id,
                draft.asset_id,
            );
            true
        }
        Err(error) => {
            state
                .data_buyer
                .record_publish_error(format!("Data access request publish failed: {error}"));
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{build_data_buyer_request_event, build_data_buyer_request_payload};
    use crate::app_state::{
        DATA_MARKET_BUYER_REQUEST_TYPE, DataBuyerRequestDraft,
        OPENAGENTS_DATA_VENDING_LOCAL_REQUEST_KIND,
    };
    use nostr::nip90::DataVendingRequest;

    fn fixture_draft() -> DataBuyerRequestDraft {
        DataBuyerRequestDraft {
            asset_ref:
                "30404:1111111111111111111111111111111111111111111111111111111111111111:data_asset.npub1seller.document.context.sha256_abc"
                    .to_string(),
            asset_id: "data_asset.npub1seller.document.context.sha256_abc".to_string(),
            provider_id: "npub1seller".to_string(),
            offer_grant_id: Some("grant.data.offer.001".to_string()),
            listing_coordinate: Some(
                "30404:1111111111111111111111111111111111111111111111111111111111111111:data_asset.npub1seller.document.context.sha256_abc"
                    .to_string(),
            ),
            offer_coordinate: Some(
                "30406:1111111111111111111111111111111111111111111111111111111111111111:grant.data.offer.001"
                    .to_string(),
            ),
            permission_scopes: vec![
                "encrypted_pointer".to_string(),
                "targeted_request".to_string(),
            ],
            delivery_mode: "encrypted_pointer".to_string(),
            preview_posture: "metadata_only".to_string(),
            bid_sats: 42,
            timeout_seconds: 120,
        }
    }

    #[test]
    fn buyer_request_payload_is_explicit_about_targeting_and_budget() {
        let payload = build_data_buyer_request_payload(&fixture_draft(), Some("npub1buyer"));
        let payload: serde_json::Value =
            serde_json::from_str(payload.as_str()).expect("payload should parse");
        assert_eq!(
            payload["request_type"].as_str(),
            Some(DATA_MARKET_BUYER_REQUEST_TYPE)
        );
        assert_eq!(
            payload["target_provider_pubkey"].as_str(),
            Some("npub1seller")
        );
        assert_eq!(
            payload["asset_ref"].as_str(),
            Some(
                "30404:1111111111111111111111111111111111111111111111111111111111111111:data_asset.npub1seller.document.context.sha256_abc"
            )
        );
        assert_eq!(payload["bid_sats"].as_u64(), Some(42));
        assert_eq!(payload["buyer_id"].as_str(), Some("npub1buyer"));
    }

    #[test]
    fn buyer_request_event_uses_targeted_data_vending_shape() {
        let identity = nostr::regenerate_identity().expect("identity");
        let event = build_data_buyer_request_event(
            &identity,
            &["wss://relay.one".to_string(), "wss://relay.two".to_string()],
            &fixture_draft(),
            Some("npub1buyer"),
        )
        .expect("request event");
        assert_eq!(event.kind, OPENAGENTS_DATA_VENDING_LOCAL_REQUEST_KIND);
        let request = DataVendingRequest::from_event(&event).expect("data-vending request");
        assert_eq!(
            request.asset_ref,
            "30404:1111111111111111111111111111111111111111111111111111111111111111:data_asset.npub1seller.document.context.sha256_abc"
        );
        assert_eq!(request.bid, Some(42_000));
        assert_eq!(request.service_providers, vec!["npub1seller".to_string()]);
        assert_eq!(request.permission_scopes.len(), 2);
        assert!(
            request
                .permission_scopes
                .iter()
                .any(|scope| scope == "targeted_request")
        );
    }

    #[test]
    fn buyer_request_event_normalizes_real_npub_targets_to_hex_p_tags() {
        let identity = nostr::regenerate_identity().expect("identity");
        let provider_identity = nostr::regenerate_identity().expect("provider identity");
        let mut draft = fixture_draft();
        draft.provider_id = provider_identity.npub.clone();
        let event = build_data_buyer_request_event(
            &identity,
            &["wss://relay.one".to_string()],
            &draft,
            Some("npub1buyer"),
        )
        .expect("request event");
        let request = DataVendingRequest::from_event(&event).expect("data-vending request");
        assert_eq!(
            request.service_providers,
            vec![provider_identity.public_key_hex.to_ascii_lowercase()]
        );
    }
}
