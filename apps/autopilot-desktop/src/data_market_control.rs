use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use nostr::Event;
use nostr::nip_ds::{
    DatasetListing, DatasetOffer, DraftDatasetListing, KIND_DATASET_LISTING, KIND_DATASET_OFFER,
};
use nostr_client::{RelayConnection, RelayMessage};
use openagents_kernel_core::authority::KernelAuthority;

use crate::app_state::{RelayDatasetListingProjection, RelayDatasetOfferProjection, RenderState};

const DS_CATALOG_LIMIT: usize = 128;
const DS_CATALOG_TIMEOUT_MS: u64 = 1_500;

fn current_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().min(i64::MAX as u128) as i64
        })
}

#[derive(Default)]
struct RelayCatalogSnapshot {
    listings: Vec<RelayDatasetListingProjection>,
    offers: Vec<RelayDatasetOfferProjection>,
}

pub(crate) fn refresh_data_market_snapshot(state: &mut RenderState) -> bool {
    state.data_market.begin_refresh();

    let mut authority_error = None::<String>;
    let mut assets = Vec::new();
    let mut grants = Vec::new();
    let mut deliveries = Vec::new();
    let mut revocations = Vec::new();
    let mut refreshed_at_ms = current_epoch_ms();

    match crate::kernel_control::remote_authority_client_for_state(state) {
        Ok(client) => {
            match crate::kernel_control::run_kernel_call(client.get_data_market_snapshot()) {
                Ok(snapshot) => {
                    assets = snapshot.assets;
                    grants = snapshot.grants;
                    deliveries = snapshot.deliveries;
                    revocations = snapshot.revocations;
                    refreshed_at_ms = snapshot.refreshed_at_ms;
                }
                Err(error) => authority_error = Some(error),
            }
        }
        Err(error) => authority_error = Some(error),
    }

    let relay_catalog = match fetch_relay_catalog(
        state.configured_provider_relay_urls().as_slice(),
        assets.as_slice(),
        grants.as_slice(),
    ) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if authority_error.is_none() {
                authority_error = Some(error);
            }
            RelayCatalogSnapshot::default()
        }
    };

    state
        .data_market
        .apply_snapshot(assets, grants, deliveries, revocations, refreshed_at_ms);
    state.data_market.apply_relay_catalog(
        relay_catalog.listings,
        relay_catalog.offers,
        refreshed_at_ms,
    );
    if let Some(error) = authority_error {
        state.data_market.last_error = Some(error);
    }
    state.data_buyer.sync_selection(&state.data_market);
    true
}

fn fetch_relay_catalog(
    relay_urls: &[String],
    assets: &[openagents_kernel_core::data::DataAsset],
    grants: &[openagents_kernel_core::data::AccessGrant],
) -> Result<RelayCatalogSnapshot, String> {
    let relay_urls = relay_urls
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if relay_urls.is_empty() {
        return Ok(RelayCatalogSnapshot::default());
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Cannot start DS catalog runtime: {error}"))?;
    runtime.block_on(async move {
        let mut events = Vec::new();
        let mut errors = Vec::new();
        for relay_url in relay_urls {
            match query_ds_events_from_relay(relay_url.as_str()).await {
                Ok(mut relay_events) => events.append(&mut relay_events),
                Err(error) => errors.push(format!("{relay_url}: {error}")),
            }
        }
        if events.is_empty() && !errors.is_empty() {
            return Err(format!(
                "Cannot refresh DS relay catalog: {}",
                errors.join(" | ")
            ));
        }
        Ok(project_relay_catalog(events, assets, grants))
    })
}

async fn query_ds_events_from_relay(relay_url: &str) -> Result<Vec<(String, Event)>, String> {
    let connection = RelayConnection::new(relay_url)
        .map_err(|error| format!("create relay connection failed: {error}"))?;
    connection
        .connect()
        .await
        .map_err(|error| format!("connect failed: {error}"))?;

    let subscription_id = format!(
        "data-market-catalog-{}",
        relay_url
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .take(16)
            .collect::<String>()
    );
    connection
        .subscribe_filters(
            subscription_id.as_str(),
            vec![serde_json::json!({
                "kinds": [30404, 30405, 30406],
                "limit": DS_CATALOG_LIMIT,
            })],
        )
        .await
        .map_err(|error| format!("subscribe failed: {error}"))?;

    let started = Instant::now();
    let timeout = Duration::from_millis(DS_CATALOG_TIMEOUT_MS);
    let mut events = Vec::new();
    while started.elapsed() < timeout {
        let remaining = timeout
            .checked_sub(started.elapsed())
            .unwrap_or_else(|| Duration::from_millis(1));
        match tokio::time::timeout(remaining.min(Duration::from_secs(1)), connection.recv()).await {
            Ok(Ok(Some(RelayMessage::Event(_, event)))) => {
                if matches!(
                    event.kind,
                    KIND_DATASET_LISTING | KIND_DATASET_OFFER | 30405
                ) {
                    events.push((relay_url.to_string(), event));
                }
            }
            Ok(Ok(Some(RelayMessage::Eose(_)))) => break,
            Ok(Ok(Some(_))) => {}
            Ok(Ok(None)) => break,
            Ok(Err(error)) => {
                let _ = connection.unsubscribe(subscription_id.as_str()).await;
                let _ = connection.disconnect().await;
                return Err(format!("receive failed: {error}"));
            }
            Err(_) => break,
        }
    }

    let _ = connection.unsubscribe(subscription_id.as_str()).await;
    let _ = connection.disconnect().await;
    Ok(events)
}

fn project_relay_catalog(
    events: Vec<(String, Event)>,
    assets: &[openagents_kernel_core::data::DataAsset],
    grants: &[openagents_kernel_core::data::AccessGrant],
) -> RelayCatalogSnapshot {
    let mut listings = BTreeMap::<String, RelayDatasetListingProjection>::new();
    let mut offers = BTreeMap::<String, RelayDatasetOfferProjection>::new();

    for (relay_url, event) in events {
        match event.kind {
            KIND_DATASET_LISTING => {
                if let Ok(listing) = DatasetListing::from_event(&event)
                    && let Ok(coordinate) = listing.coordinate(event.pubkey.clone())
                {
                    let coordinate = coordinate.to_string();
                    let linked_asset_id = assets.iter().find_map(|asset| {
                        asset
                            .nostr_publications
                            .ds_listing
                            .as_ref()
                            .and_then(|reference| reference.coordinate.as_deref())
                            .filter(|value| value.eq_ignore_ascii_case(coordinate.as_str()))
                            .map(|_| asset.asset_id.clone())
                    });
                    let projection = RelayDatasetListingProjection {
                        coordinate: coordinate.clone(),
                        publisher_pubkey: event.pubkey.clone(),
                        relay_url: Some(relay_url),
                        title: listing.title.clone(),
                        summary: listing.summary.clone(),
                        dataset_kind: listing.dataset_kind.clone(),
                        access: listing.access.clone(),
                        delivery_modes: listing.delivery_modes.clone(),
                        created_at_seconds: event.created_at,
                        draft: false,
                        linked_asset_id,
                    };
                    if listings
                        .get(coordinate.as_str())
                        .is_none_or(|existing| existing.created_at_seconds <= event.created_at)
                    {
                        listings.insert(coordinate, projection);
                    }
                }
            }
            30405 => {
                if let Ok(draft) = DraftDatasetListing::from_event(&event)
                    && let Ok(coordinate) = draft.coordinate(event.pubkey.clone())
                {
                    let coordinate = coordinate.to_string();
                    let linked_asset_id = assets.iter().find_map(|asset| {
                        asset
                            .nostr_publications
                            .ds_draft_listing
                            .as_ref()
                            .and_then(|reference| reference.coordinate.as_deref())
                            .filter(|value| value.eq_ignore_ascii_case(coordinate.as_str()))
                            .map(|_| asset.asset_id.clone())
                    });
                    let projection = RelayDatasetListingProjection {
                        coordinate: coordinate.clone(),
                        publisher_pubkey: event.pubkey.clone(),
                        relay_url: Some(relay_url),
                        title: draft.listing.title.clone(),
                        summary: draft.listing.summary.clone(),
                        dataset_kind: draft.listing.dataset_kind.clone(),
                        access: draft.listing.access.clone(),
                        delivery_modes: draft.listing.delivery_modes.clone(),
                        created_at_seconds: event.created_at,
                        draft: true,
                        linked_asset_id,
                    };
                    if listings
                        .get(coordinate.as_str())
                        .is_none_or(|existing| existing.created_at_seconds <= event.created_at)
                    {
                        listings.insert(coordinate, projection);
                    }
                }
            }
            KIND_DATASET_OFFER => {
                if let Ok(offer) = DatasetOffer::from_event(&event)
                    && let Ok(coordinate) = offer.coordinate(event.pubkey.clone())
                {
                    let coordinate = coordinate.to_string();
                    let listing_coordinate = offer.listing_ref.coordinate.to_string();
                    let linked_grant_id = grants.iter().find_map(|grant| {
                        grant
                            .nostr_publications
                            .ds_offer
                            .as_ref()
                            .and_then(|reference| reference.coordinate.as_deref())
                            .filter(|value| value.eq_ignore_ascii_case(coordinate.as_str()))
                            .map(|_| grant.grant_id.clone())
                    });
                    let linked_asset_id = linked_grant_id
                        .as_deref()
                        .and_then(|grant_id| {
                            grants
                                .iter()
                                .find(|grant| grant.grant_id == grant_id)
                                .map(|grant| grant.asset_id.clone())
                        })
                        .or_else(|| {
                            assets.iter().find_map(|asset| {
                                asset
                                    .nostr_publications
                                    .ds_listing
                                    .as_ref()
                                    .and_then(|reference| reference.coordinate.as_deref())
                                    .filter(|value| {
                                        value.eq_ignore_ascii_case(listing_coordinate.as_str())
                                    })
                                    .map(|_| asset.asset_id.clone())
                            })
                        });
                    let projection = RelayDatasetOfferProjection {
                        coordinate: coordinate.clone(),
                        listing_coordinate,
                        publisher_pubkey: event.pubkey.clone(),
                        relay_url: Some(relay_url),
                        status: offer.status.as_str().to_string(),
                        policy: offer.policy.clone(),
                        delivery_modes: offer.delivery_modes.clone(),
                        targeted_buyer_pubkeys: offer
                            .targeted_buyers
                            .iter()
                            .map(|buyer| buyer.pubkey.clone())
                            .collect(),
                        price_amount: offer.price.as_ref().map(|price| price.amount.clone()),
                        price_currency: offer.price.as_ref().map(|price| price.currency.clone()),
                        created_at_seconds: event.created_at,
                        linked_asset_id,
                        linked_grant_id,
                    };
                    if offers
                        .get(coordinate.as_str())
                        .is_none_or(|existing| existing.created_at_seconds <= event.created_at)
                    {
                        offers.insert(coordinate, projection);
                    }
                }
            }
            _ => {}
        }
    }

    RelayCatalogSnapshot {
        listings: listings.into_values().collect(),
        offers: offers.into_values().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::project_relay_catalog;
    use nostr::nip_ds::{AddressableEventReference, DatasetListing, DatasetOffer, PaymentMethod};
    use nostr::nip99::Price;
    use openagents_kernel_core::data::{
        AccessGrant, AccessGrantNostrPublications, AccessGrantStatus, DataAsset,
        DataAssetNostrPublications, NostrPublicationRef, PermissionPolicy,
    };
    use openagents_kernel_core::receipts::{Asset, Money, MoneyAmount};

    fn sign_template(
        identity: &nostr::NostrIdentity,
        template: &nostr::EventTemplate,
    ) -> nostr::Event {
        let key_bytes = hex::decode(identity.private_key_hex.as_str()).expect("private key hex");
        let mut private_key = [0_u8; 32];
        private_key.copy_from_slice(key_bytes.as_slice());
        nostr::finalize_event(template, &private_key).expect("sign event")
    }

    #[test]
    fn relay_catalog_projection_links_ds_listing_and_offer_to_kernel_records() {
        let identity = nostr::regenerate_identity().expect("identity");
        let listing = DatasetListing::new(
            "data_asset.provider.alpha.bundle",
            "Example dataset listing.",
            "Seller bundle",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .with_published_at(1_762_700_000)
        .with_dataset_kind("conversation_bundle")
        .with_access("paid")
        .add_delivery_mode("encrypted_pointer");
        let listing_coordinate = listing
            .coordinate(identity.public_key_hex.clone())
            .expect("listing coordinate")
            .to_string();
        let listing_event = sign_template(
            &identity,
            &listing
                .to_event_template(1_762_700_000)
                .expect("listing template"),
        );

        let offer = DatasetOffer::new(
            "grant.data.offer.001",
            "Targeted access",
            AddressableEventReference::new(
                listing
                    .coordinate(identity.public_key_hex.clone())
                    .expect("listing coordinate"),
            ),
        )
        .with_policy("targeted_request")
        .with_price(Price::one_time("42", "SAT"))
        .add_payment_method(PaymentMethod::new("ln"))
        .add_delivery_mode("encrypted_pointer");
        let offer_coordinate = offer
            .coordinate(identity.public_key_hex.clone())
            .expect("offer coordinate")
            .to_string();
        let offer_event = sign_template(
            &identity,
            &offer
                .to_event_template(1_762_700_010)
                .expect("offer template"),
        );

        let asset = DataAsset {
            asset_id: "data_asset.provider.alpha.bundle".to_string(),
            provider_id: identity.public_key_hex.clone(),
            asset_kind: "conversation_bundle".to_string(),
            title: "Seller bundle".to_string(),
            description: None,
            content_digest: None,
            provenance_ref: None,
            default_policy: Some(PermissionPolicy::default()),
            price_hint: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(42),
            }),
            created_at_ms: 1_762_700_000_000,
            status: Default::default(),
            nostr_publications: DataAssetNostrPublications {
                ds_listing: Some(NostrPublicationRef {
                    coordinate: Some(listing_coordinate.clone()),
                    event_id: None,
                    relay_url: None,
                }),
                ds_draft_listing: None,
            },
            metadata: serde_json::json!({}),
        };
        let grant = AccessGrant {
            grant_id: "grant.data.offer.001".to_string(),
            asset_id: asset.asset_id.clone(),
            provider_id: identity.public_key_hex.clone(),
            consumer_id: Some("npub1buyer".to_string()),
            permission_policy: PermissionPolicy::default(),
            offer_price: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(42),
            }),
            warranty_window_ms: None,
            created_at_ms: 1_762_700_010_000,
            expires_at_ms: 1_762_786_410_000,
            accepted_at_ms: None,
            status: AccessGrantStatus::Offered,
            nostr_publications: AccessGrantNostrPublications {
                ds_offer: Some(NostrPublicationRef {
                    coordinate: Some(offer_coordinate.clone()),
                    event_id: None,
                    relay_url: None,
                }),
                ds_access_request: None,
                ds_access_result: None,
            },
            metadata: serde_json::json!({}),
        };

        let snapshot = project_relay_catalog(
            vec![
                ("wss://relay.example".to_string(), listing_event),
                ("wss://relay.example".to_string(), offer_event),
            ],
            &[asset],
            &[grant],
        );

        assert_eq!(snapshot.listings.len(), 1);
        assert_eq!(snapshot.offers.len(), 1);
        assert_eq!(
            snapshot.listings[0].linked_asset_id.as_deref(),
            Some("data_asset.provider.alpha.bundle")
        );
        assert_eq!(
            snapshot.offers[0].linked_grant_id.as_deref(),
            Some("grant.data.offer.001")
        );
        assert_eq!(
            snapshot.offers[0].linked_asset_id.as_deref(),
            Some("data_asset.provider.alpha.bundle")
        );
    }
}
