use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use nostr::Event;
use nostr::nip_ds::{
    AddressableEventCoordinate, DatasetAccessContract, DatasetListing, DatasetOffer,
    DraftDatasetListing, KIND_DATASET_ACCESS_CONTRACT, KIND_DATASET_LISTING, KIND_DATASET_OFFER,
};
use nostr::nip15::{KIND_PRODUCT, KIND_STALL, MarketplaceProduct, MarketplaceStall};
use nostr::nip28::{KIND_CHANNEL_METADATA, parse_dataset_discussion_channel_link};
use nostr::nip90::{DataVendingRequest, DataVendingResult};
use nostr::nip99::{ClassifiedListing, KIND_CLASSIFIED_LISTING};
use nostr_client::{RelayConnection, RelayMessage};
use serde::{Deserialize, Serialize};

use crate::app_state::{
    RelayDatasetAccessContractProjection, RelayDatasetAccessRequestProjection,
    RelayDatasetAccessResultProjection, RelayDatasetListingProjection, RelayDatasetOfferProjection,
    RelayDatasetSettlementMatchProjection, RenderState,
};
use crate::spark_wallet::decode_lightning_invoice_payment_hash;

const DS_CATALOG_LIMIT: usize = 128;
const DS_CATALOG_TIMEOUT_MS: u64 = 1_500;
const DATA_MARKET_RELAY_REPLICA_SCHEMA_VERSION: u16 = 1;
const DATA_MARKET_RELAY_REPLICA_FILENAME: &str = "data-market-relay-replica.json";

fn current_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().min(i64::MAX as u128) as i64
        })
}

fn openagents_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
}

fn data_market_relay_replica_path() -> PathBuf {
    openagents_dir().join(DATA_MARKET_RELAY_REPLICA_FILENAME)
}

fn persist_relay_catalog(snapshot: &RelayCatalogSnapshot) -> Result<(), String> {
    persist_relay_catalog_to_path(snapshot, &data_market_relay_replica_path())
}

fn relay_catalog_snapshot_from_state(state: &RenderState) -> RelayCatalogSnapshot {
    RelayCatalogSnapshot {
        listings: state.data_market.relay_listings.clone(),
        offers: state.data_market.relay_offers.clone(),
        requests: state.data_market.relay_requests.clone(),
        access_contracts: state.data_market.relay_access_contracts.clone(),
        results: state.data_market.relay_results.clone(),
        settlement_matches: state.data_market.relay_settlement_matches.clone(),
    }
}

pub(crate) fn persist_data_market_relay_replica_from_state(
    state: &RenderState,
) -> Result<(), String> {
    persist_relay_catalog(&relay_catalog_snapshot_from_state(state))
}

fn persist_relay_catalog_to_path(
    snapshot: &RelayCatalogSnapshot,
    path: &Path,
) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "Cannot persist relay replica without parent directory: {}",
            path.display()
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Cannot create relay replica directory {}: {error}",
            parent.display()
        )
    })?;
    let persisted = PersistedRelayCatalogSnapshot {
        schema_version: DATA_MARKET_RELAY_REPLICA_SCHEMA_VERSION,
        updated_at_epoch_ms: current_epoch_ms().max(0) as u64,
        listings: snapshot.listings.clone(),
        offers: snapshot.offers.clone(),
        requests: snapshot.requests.clone(),
        access_contracts: snapshot.access_contracts.clone(),
        results: snapshot.results.clone(),
        settlement_matches: snapshot.settlement_matches.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&persisted)
        .map_err(|error| format!("Cannot encode relay replica: {error}"))?;
    fs::write(path, bytes)
        .map_err(|error| format!("Cannot write relay replica {}: {error}", path.display()))
}

fn load_persisted_relay_catalog() -> Result<RelayCatalogSnapshot, String> {
    load_persisted_relay_catalog_from_path(&data_market_relay_replica_path())
}

fn load_persisted_relay_catalog_from_path(path: &Path) -> Result<RelayCatalogSnapshot, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Cannot read relay replica {}: {error}", path.display()))?;
    let persisted: PersistedRelayCatalogSnapshot = serde_json::from_str(raw.as_str())
        .map_err(|error| format!("Cannot decode relay replica {}: {error}", path.display()))?;
    if persisted.schema_version != DATA_MARKET_RELAY_REPLICA_SCHEMA_VERSION {
        return Err(format!(
            "Relay replica schema mismatch at {}: expected {}, got {}",
            path.display(),
            DATA_MARKET_RELAY_REPLICA_SCHEMA_VERSION,
            persisted.schema_version
        ));
    }
    Ok(RelayCatalogSnapshot {
        listings: persisted.listings,
        offers: persisted.offers,
        requests: persisted.requests,
        access_contracts: persisted.access_contracts,
        results: persisted.results,
        settlement_matches: persisted.settlement_matches,
    })
}

pub(crate) fn hydrate_data_market_relay_replica(state: &mut RenderState) -> bool {
    if !state.data_market.relay_listings.is_empty()
        || !state.data_market.relay_offers.is_empty()
        || !state.data_market.relay_requests.is_empty()
        || !state.data_market.relay_access_contracts.is_empty()
        || !state.data_market.relay_results.is_empty()
        || !state.data_market.relay_settlement_matches.is_empty()
    {
        return false;
    }

    let Ok(snapshot) = load_persisted_relay_catalog() else {
        return false;
    };
    state.data_market.apply_relay_catalog(
        snapshot.listings,
        snapshot.offers,
        snapshot.requests,
        snapshot.access_contracts,
        snapshot.results,
        snapshot.settlement_matches,
        current_epoch_ms(),
    );
    state.data_buyer.sync_selection(&state.data_market);
    true
}

#[derive(Clone, Debug, Default)]
struct RelayCatalogSnapshot {
    listings: Vec<RelayDatasetListingProjection>,
    offers: Vec<RelayDatasetOfferProjection>,
    requests: Vec<RelayDatasetAccessRequestProjection>,
    access_contracts: Vec<RelayDatasetAccessContractProjection>,
    results: Vec<RelayDatasetAccessResultProjection>,
    settlement_matches: Vec<RelayDatasetSettlementMatchProjection>,
}

#[derive(Clone, Debug, Default)]
struct RelayCatalogLinkHints {
    listing_asset_ids: BTreeMap<String, String>,
    offer_grant_ids: BTreeMap<String, String>,
    offer_asset_ids: BTreeMap<String, String>,
    offer_listing_coordinates: BTreeMap<String, String>,
}

impl RelayCatalogLinkHints {
    fn record_listing_asset(&mut self, coordinate: &str, asset_id: &str) {
        if coordinate.trim().is_empty() || asset_id.trim().is_empty() {
            return;
        }
        self.listing_asset_ids
            .entry(coordinate.trim().to_ascii_lowercase())
            .or_insert_with(|| asset_id.trim().to_string());
    }

    fn record_offer_grant(&mut self, coordinate: &str, grant_id: &str) {
        if coordinate.trim().is_empty() || grant_id.trim().is_empty() {
            return;
        }
        self.offer_grant_ids
            .entry(coordinate.trim().to_ascii_lowercase())
            .or_insert_with(|| grant_id.trim().to_string());
    }

    fn record_offer_asset(&mut self, coordinate: &str, asset_id: &str) {
        if coordinate.trim().is_empty() || asset_id.trim().is_empty() {
            return;
        }
        self.offer_asset_ids
            .entry(coordinate.trim().to_ascii_lowercase())
            .or_insert_with(|| asset_id.trim().to_string());
    }

    fn record_offer_listing_coordinate(&mut self, coordinate: &str, listing_coordinate: &str) {
        if coordinate.trim().is_empty() || listing_coordinate.trim().is_empty() {
            return;
        }
        self.offer_listing_coordinates
            .entry(coordinate.trim().to_ascii_lowercase())
            .or_insert_with(|| listing_coordinate.trim().to_string());
    }

    fn linked_asset_id_for_listing_coordinate(&self, coordinate: &str) -> Option<String> {
        self.listing_asset_ids
            .get(&coordinate.trim().to_ascii_lowercase())
            .cloned()
    }

    fn linked_grant_id_for_offer_coordinate(&self, coordinate: &str) -> Option<String> {
        self.offer_grant_ids
            .get(&coordinate.trim().to_ascii_lowercase())
            .cloned()
    }

    fn linked_asset_id_for_offer_coordinate(&self, coordinate: &str) -> Option<String> {
        self.offer_asset_ids
            .get(&coordinate.trim().to_ascii_lowercase())
            .cloned()
    }

    fn listing_coordinate_for_offer_coordinate(&self, coordinate: &str) -> Option<String> {
        self.offer_listing_coordinates
            .get(&coordinate.trim().to_ascii_lowercase())
            .cloned()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedRelayCatalogSnapshot {
    schema_version: u16,
    updated_at_epoch_ms: u64,
    #[serde(default)]
    listings: Vec<RelayDatasetListingProjection>,
    #[serde(default)]
    offers: Vec<RelayDatasetOfferProjection>,
    #[serde(default)]
    requests: Vec<RelayDatasetAccessRequestProjection>,
    #[serde(default)]
    access_contracts: Vec<RelayDatasetAccessContractProjection>,
    #[serde(default)]
    results: Vec<RelayDatasetAccessResultProjection>,
    #[serde(default)]
    settlement_matches: Vec<RelayDatasetSettlementMatchProjection>,
}

#[derive(Clone, Debug)]
struct ClassifiedListingWrapperProjection {
    coordinate: String,
    event_id: String,
    relay_url: String,
    title: String,
    summary: Option<String>,
    price_amount: Option<String>,
    price_currency: Option<String>,
    created_at_seconds: u64,
}

#[derive(Clone, Debug)]
struct ClassifiedOfferWrapperProjection {
    coordinate: String,
    event_id: String,
    relay_url: String,
    listing_coordinate: Option<String>,
    price_amount: Option<String>,
    price_currency: Option<String>,
    created_at_seconds: u64,
}

#[derive(Clone, Debug)]
struct StorefrontStallProjection {
    coordinate: String,
    relay_url: String,
    name: String,
    created_at_seconds: u64,
}

#[derive(Clone, Debug)]
struct StorefrontProductWrapperProjection {
    coordinate: String,
    event_id: String,
    relay_url: String,
    stall_coordinate: String,
    stall_name: Option<String>,
    title: String,
    price_amount: String,
    price_currency: String,
    created_at_seconds: u64,
}

#[derive(Clone, Debug)]
struct DatasetDiscussionChannelProjection {
    channel_id: String,
    relay_url: Option<String>,
    name: String,
    created_at_seconds: u64,
}

pub(crate) fn refresh_data_market_snapshot(state: &mut RenderState) -> bool {
    state.data_market.begin_refresh();

    let mut relay_error = None::<String>;
    let refreshed_at_ms = current_epoch_ms();

    let relay_catalog = match fetch_relay_catalog(
        state.configured_provider_relay_urls().as_slice(),
        state.data_market.assets.as_slice(),
        state.data_market.grants.as_slice(),
        state.data_market.relay_listings.as_slice(),
        state.data_market.relay_offers.as_slice(),
        state.data_market.relay_requests.as_slice(),
        state.data_market.relay_access_contracts.as_slice(),
        state.data_market.relay_results.as_slice(),
        state.spark_wallet.recent_payments.as_slice(),
    ) {
        Ok(snapshot) => {
            if let Err(error) = persist_relay_catalog(&snapshot) {
                relay_error = Some(error);
            }
            snapshot
        }
        Err(error) => {
            relay_error = Some(error.clone());
            match load_persisted_relay_catalog() {
                Ok(snapshot) => snapshot,
                Err(load_error) => {
                    relay_error = Some(format!("{error} | {load_error}"));
                    RelayCatalogSnapshot::default()
                }
            }
        }
    };

    state.data_market.apply_relay_catalog(
        relay_catalog.listings,
        relay_catalog.offers,
        relay_catalog.requests,
        relay_catalog.access_contracts,
        relay_catalog.results,
        relay_catalog.settlement_matches,
        refreshed_at_ms,
    );
    if let Some(error) = relay_error {
        if state.data_market.has_relay_snapshot() {
            if let Some(last_action) = state.data_market.last_action.clone() {
                state.data_market.last_action = Some(format!(
                    "{last_action} // refresh used the persisted relay replica because live relay refresh failed: {error}"
                ));
            }
        } else {
            state.data_market.last_error = Some(error);
        }
    }
    state.data_buyer.sync_selection(&state.data_market);
    true
}

fn fetch_relay_catalog(
    relay_urls: &[String],
    existing_assets: &[openagents_kernel_core::data::DataAsset],
    existing_grants: &[openagents_kernel_core::data::AccessGrant],
    existing_listings: &[RelayDatasetListingProjection],
    existing_offers: &[RelayDatasetOfferProjection],
    existing_requests: &[RelayDatasetAccessRequestProjection],
    existing_access_contracts: &[RelayDatasetAccessContractProjection],
    existing_results: &[RelayDatasetAccessResultProjection],
    payments: &[openagents_spark::PaymentSummary],
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
        Ok(project_relay_catalog(
            events,
            build_relay_catalog_link_hints(
                existing_assets,
                existing_grants,
                existing_listings,
                existing_offers,
                existing_requests,
                existing_access_contracts,
                existing_results,
            ),
            payments,
        ))
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
            vec![
                serde_json::json!({
                    "kinds": [30017, 30018, 30402, 30404, 30405, 30406, 30407, 5960, 6960],
                    "limit": DS_CATALOG_LIMIT,
                }),
                serde_json::json!({
                    "kinds": [41],
                    "#t": ["dataset", "nip-ds"],
                    "limit": DS_CATALOG_LIMIT,
                }),
            ],
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
                    KIND_CHANNEL_METADATA
                        | KIND_PRODUCT
                        | KIND_STALL
                        | KIND_CLASSIFIED_LISTING
                        | KIND_DATASET_LISTING
                        | KIND_DATASET_OFFER
                        | KIND_DATASET_ACCESS_CONTRACT
                        | nostr::KIND_DATASET_ACCESS_REQUEST
                        | nostr::KIND_DATASET_ACCESS_RESULT
                        | 30405
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
    link_hints: RelayCatalogLinkHints,
    payments: &[openagents_spark::PaymentSummary],
) -> RelayCatalogSnapshot {
    let mut listings = BTreeMap::<String, RelayDatasetListingProjection>::new();
    let mut offers = BTreeMap::<String, RelayDatasetOfferProjection>::new();
    let mut requests = BTreeMap::<String, RelayDatasetAccessRequestProjection>::new();
    let mut access_contracts = BTreeMap::<String, RelayDatasetAccessContractProjection>::new();
    let mut results = BTreeMap::<String, RelayDatasetAccessResultProjection>::new();
    let mut settlement_matches = BTreeMap::<String, RelayDatasetSettlementMatchProjection>::new();
    let mut listing_wrappers = BTreeMap::<String, ClassifiedListingWrapperProjection>::new();
    let mut offer_wrappers = BTreeMap::<String, ClassifiedOfferWrapperProjection>::new();
    let mut stalls = BTreeMap::<String, StorefrontStallProjection>::new();
    let mut listing_storefront_products =
        BTreeMap::<String, StorefrontProductWrapperProjection>::new();
    let mut offer_storefront_products =
        BTreeMap::<String, StorefrontProductWrapperProjection>::new();
    let mut listing_discussions = BTreeMap::<String, DatasetDiscussionChannelProjection>::new();
    let mut offer_discussions = BTreeMap::<String, DatasetDiscussionChannelProjection>::new();

    for (relay_url, event) in events {
        match event.kind {
            KIND_DATASET_LISTING => {
                if let Ok(listing) = DatasetListing::from_event(&event)
                    && let Ok(coordinate) = listing.coordinate(event.pubkey.clone())
                {
                    let coordinate = coordinate.to_string();
                    let linked_asset_id =
                        link_hints.linked_asset_id_for_listing_coordinate(coordinate.as_str());
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
                        classified_coordinate: None,
                        classified_event_id: None,
                        classified_price_amount: None,
                        classified_price_currency: None,
                        storefront_stall_coordinate: None,
                        storefront_stall_name: None,
                        storefront_product_coordinate: None,
                        storefront_product_event_id: None,
                        storefront_product_title: None,
                        storefront_product_price_amount: None,
                        storefront_product_price_currency: None,
                        discussion_channel_id: None,
                        discussion_channel_name: None,
                        discussion_channel_relay_url: None,
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
                    let linked_asset_id =
                        link_hints.linked_asset_id_for_listing_coordinate(coordinate.as_str());
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
                        classified_coordinate: None,
                        classified_event_id: None,
                        classified_price_amount: None,
                        classified_price_currency: None,
                        storefront_stall_coordinate: None,
                        storefront_stall_name: None,
                        storefront_product_coordinate: None,
                        storefront_product_event_id: None,
                        storefront_product_title: None,
                        storefront_product_price_amount: None,
                        storefront_product_price_currency: None,
                        discussion_channel_id: None,
                        discussion_channel_name: None,
                        discussion_channel_relay_url: None,
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
                    let linked_grant_id =
                        link_hints.linked_grant_id_for_offer_coordinate(coordinate.as_str());
                    let linked_asset_id = link_hints
                        .linked_asset_id_for_offer_coordinate(coordinate.as_str())
                        .or_else(|| {
                            link_hints
                                .linked_asset_id_for_listing_coordinate(listing_coordinate.as_str())
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
                        classified_coordinate: None,
                        classified_event_id: None,
                        storefront_stall_coordinate: None,
                        storefront_stall_name: None,
                        storefront_product_coordinate: None,
                        storefront_product_event_id: None,
                        storefront_product_title: None,
                        storefront_product_price_amount: None,
                        storefront_product_price_currency: None,
                        discussion_channel_id: None,
                        discussion_channel_name: None,
                        discussion_channel_relay_url: None,
                    };
                    if offers
                        .get(coordinate.as_str())
                        .is_none_or(|existing| existing.created_at_seconds <= event.created_at)
                    {
                        offers.insert(coordinate, projection);
                    }
                }
            }
            nostr::KIND_DATASET_ACCESS_REQUEST => {
                if let Ok(request) = DataVendingRequest::from_event(&event) {
                    let listing_coordinate = request
                        .listing_ref
                        .as_ref()
                        .map(|reference| reference.coordinate.to_string())
                        .or_else(|| listing_coordinate_for_asset_ref(request.asset_ref.as_str()))
                        .unwrap_or_else(|| request.asset_ref.clone());
                    let offer_coordinate = request
                        .offer_ref
                        .as_ref()
                        .map(|reference| reference.coordinate.to_string());
                    let linked_grant_id = request.grant_id.clone().or_else(|| {
                        offer_coordinate.as_deref().and_then(|coordinate| {
                            link_hints.linked_grant_id_for_offer_coordinate(coordinate)
                        })
                    });
                    let linked_asset_id = request
                        .asset_id
                        .clone()
                        .or_else(|| {
                            link_hints
                                .linked_asset_id_for_listing_coordinate(listing_coordinate.as_str())
                        })
                        .or_else(|| {
                            offer_coordinate.as_deref().and_then(|coordinate| {
                                link_hints.linked_asset_id_for_offer_coordinate(coordinate)
                            })
                        });
                    requests.insert(
                        event.id.clone(),
                        RelayDatasetAccessRequestProjection {
                            event_id: event.id.clone(),
                            request_kind: request.request_kind,
                            requester_pubkey: event.pubkey.clone(),
                            relay_url: Some(relay_url),
                            listing_coordinate,
                            offer_coordinate,
                            asset_ref: request.asset_ref,
                            asset_id: request.asset_id,
                            grant_id: linked_grant_id.clone(),
                            targeted_provider_pubkeys: request.service_providers,
                            permission_scopes: request.permission_scopes,
                            delivery_mode: request.delivery_mode.as_str().to_string(),
                            preview_posture: request.preview_posture.as_str().to_string(),
                            bid_msats: request.bid,
                            encrypted: request.encrypted,
                            created_at_seconds: event.created_at,
                            expires_at_seconds: data_request_timeout_seconds(
                                request.content.as_str(),
                                event.created_at,
                            ),
                            linked_asset_id,
                            linked_grant_id,
                        },
                    );
                }
            }
            KIND_DATASET_ACCESS_CONTRACT => {
                if let Ok(contract) = DatasetAccessContract::from_event(&event)
                    && let Ok(coordinate) = contract.coordinate(event.pubkey.clone())
                {
                    let coordinate = coordinate.to_string();
                    let listing_coordinate = contract.listing_ref.coordinate.to_string();
                    let offer_coordinate = contract
                        .offer_ref
                        .as_ref()
                        .map(|reference| reference.coordinate.to_string());
                    let linked_grant_id = offer_coordinate
                        .as_deref()
                        .and_then(|value| {
                            link_hints.linked_grant_id_for_offer_coordinate(value)
                        });
                    let linked_asset_id = link_hints
                        .linked_asset_id_for_listing_coordinate(listing_coordinate.as_str())
                        .or_else(|| {
                            offer_coordinate.as_deref().and_then(|coordinate| {
                                link_hints.linked_asset_id_for_offer_coordinate(coordinate)
                            })
                        });
                    let projection = RelayDatasetAccessContractProjection {
                        coordinate: coordinate.clone(),
                        seller_pubkey: event.pubkey.clone(),
                        buyer_pubkey: contract.buyer.pubkey.clone(),
                        relay_url: Some(relay_url),
                        listing_coordinate,
                        offer_coordinate,
                        request_event_id: contract.request_ref.event_id.clone(),
                        result_event_id: contract
                            .result_ref
                            .as_ref()
                            .map(|reference| reference.event_id.clone()),
                        status: contract.status.as_str().to_string(),
                        payment_method: contract.payment_method.as_ref().map(payment_method_rail),
                        amount_msats: contract.amount_msats,
                        bolt11: contract.bolt11.clone(),
                        payment_hash: contract
                            .bolt11
                            .as_deref()
                            .and_then(decode_lightning_invoice_payment_hash),
                        payment_evidence_event_ids: contract
                            .payment_evidence_refs
                            .iter()
                            .map(|reference| reference.event_id.clone())
                            .collect(),
                        delivery_mode: contract.delivery_mode.clone(),
                        delivery_ref: contract.delivery_ref.clone(),
                        delivery_mime_type: contract.delivery_mime_type.clone(),
                        delivery_digest: contract.delivery_digest.clone(),
                        created_at_seconds: event.created_at,
                        expires_at_seconds: contract.expires_at,
                        reason_code: contract.reason_code.clone(),
                        linked_asset_id,
                        linked_grant_id,
                    };
                    if access_contracts
                        .get(coordinate.as_str())
                        .is_none_or(|existing| existing.created_at_seconds <= event.created_at)
                    {
                        access_contracts.insert(coordinate, projection);
                    }
                }
            }
            nostr::KIND_DATASET_ACCESS_RESULT => {
                if let Ok(result) = DataVendingResult::from_event(&event) {
                    let listing_coordinate = result
                        .listing_ref
                        .as_ref()
                        .map(|reference| reference.coordinate.to_string())
                        .or_else(|| listing_coordinate_for_asset_ref(result.asset_ref.as_str()))
                        .unwrap_or_else(|| result.asset_ref.clone());
                    let offer_coordinate = result
                        .offer_ref
                        .as_ref()
                        .map(|reference| reference.coordinate.to_string());
                    let linked_grant_id = result.grant_id.clone().or_else(|| {
                        offer_coordinate.as_deref().and_then(|coordinate| {
                            link_hints.linked_grant_id_for_offer_coordinate(coordinate)
                        })
                    });
                    let linked_asset_id = result
                        .asset_id
                        .clone()
                        .or_else(|| {
                            link_hints
                                .linked_asset_id_for_listing_coordinate(listing_coordinate.as_str())
                        })
                        .or_else(|| {
                            offer_coordinate.as_deref().and_then(|coordinate| {
                                link_hints.linked_asset_id_for_offer_coordinate(coordinate)
                            })
                        });
                    results.insert(
                        event.id.clone(),
                        RelayDatasetAccessResultProjection {
                            event_id: event.id.clone(),
                            seller_pubkey: event.pubkey.clone(),
                            buyer_pubkey: result.customer_pubkey,
                            relay_url: Some(relay_url),
                            request_event_id: result.request_id,
                            listing_coordinate,
                            offer_coordinate,
                            asset_ref: result.asset_ref,
                            asset_id: result.asset_id,
                            grant_id: linked_grant_id.clone(),
                            delivery_bundle_id: result.delivery_bundle_id,
                            delivery_mode: result.delivery_mode.as_str().to_string(),
                            preview_posture: result.preview_posture.as_str().to_string(),
                            delivery_ref: result.delivery_ref,
                            delivery_digest: result.delivery_digest,
                            amount_msats: result.amount,
                            bolt11: result.bolt11.clone(),
                            payment_hash: result
                                .bolt11
                                .as_deref()
                                .and_then(decode_lightning_invoice_payment_hash),
                            created_at_seconds: event.created_at,
                            linked_asset_id,
                            linked_grant_id,
                        },
                    );
                }
            }
            KIND_STALL => {
                if let Ok(stall) = MarketplaceStall::from_event(&event)
                    && let Ok(coordinate) = stall.coordinate(event.pubkey.clone())
                {
                    let stall_projection = StorefrontStallProjection {
                        coordinate: coordinate.clone(),
                        relay_url: relay_url.clone(),
                        name: stall.name.clone(),
                        created_at_seconds: event.created_at,
                    };
                    if stalls
                        .get(coordinate.as_str())
                        .is_none_or(|existing| existing.created_at_seconds <= event.created_at)
                    {
                        stalls.insert(coordinate, stall_projection);
                    }
                }
            }
            KIND_PRODUCT => {
                if let Ok(product) = MarketplaceProduct::from_event(&event)
                    && let Ok(product_coordinate) = product.coordinate(event.pubkey.clone())
                    && let Ok(stall_coordinate) = product.stall_coordinate(event.pubkey.clone())
                {
                    let wrapper = StorefrontProductWrapperProjection {
                        coordinate: product_coordinate,
                        event_id: event.id.clone(),
                        relay_url: relay_url.clone(),
                        stall_coordinate: stall_coordinate.clone(),
                        stall_name: stalls
                            .get(stall_coordinate.as_str())
                            .map(|stall| stall.name.clone()),
                        title: product.name.clone(),
                        price_amount: marketplace_price_string(product.price),
                        price_currency: product.currency.clone(),
                        created_at_seconds: event.created_at,
                    };
                    for listing_coordinate in product
                        .address_refs
                        .iter()
                        .filter_map(|value| AddressableEventCoordinate::parse(value).ok())
                        .filter(|coordinate| coordinate.kind == KIND_DATASET_LISTING)
                        .map(|coordinate| coordinate.to_string())
                    {
                        if listing_storefront_products
                            .get(listing_coordinate.as_str())
                            .is_none_or(|existing| existing.created_at_seconds <= event.created_at)
                        {
                            listing_storefront_products.insert(listing_coordinate, wrapper.clone());
                        }
                    }
                    for offer_coordinate in product
                        .address_refs
                        .iter()
                        .filter_map(|value| AddressableEventCoordinate::parse(value).ok())
                        .filter(|coordinate| coordinate.kind == KIND_DATASET_OFFER)
                        .map(|coordinate| coordinate.to_string())
                    {
                        if offer_storefront_products
                            .get(offer_coordinate.as_str())
                            .is_none_or(|existing| existing.created_at_seconds <= event.created_at)
                        {
                            offer_storefront_products.insert(offer_coordinate, wrapper.clone());
                        }
                    }
                }
            }
            KIND_CHANNEL_METADATA => {
                if let Ok(link) = parse_dataset_discussion_channel_link(&event) {
                    let discussion = DatasetDiscussionChannelProjection {
                        channel_id: link.channel_create_event_id.clone(),
                        relay_url: link.relay_url.clone(),
                        name: if link.metadata.name.trim().is_empty() {
                            "Dataset discussion".to_string()
                        } else {
                            link.metadata.name
                        },
                        created_at_seconds: event.created_at,
                    };
                    for listing_ref in link.listing_refs {
                        let listing_coordinate = listing_ref.coordinate.to_string();
                        if listing_discussions
                            .get(listing_coordinate.as_str())
                            .is_none_or(|existing| existing.created_at_seconds <= event.created_at)
                        {
                            listing_discussions.insert(listing_coordinate, discussion.clone());
                        }
                    }
                    for offer_ref in link.offer_refs {
                        let offer_coordinate = offer_ref.coordinate.to_string();
                        if offer_discussions
                            .get(offer_coordinate.as_str())
                            .is_none_or(|existing| existing.created_at_seconds <= event.created_at)
                        {
                            offer_discussions.insert(offer_coordinate, discussion.clone());
                        }
                    }
                }
            }
            KIND_CLASSIFIED_LISTING => {
                if let Ok(classified) = ClassifiedListing::from_event(&event)
                    && let Ok(classified_coordinate) = classified.coordinate(event.pubkey.clone())
                {
                    let listing_coordinates = classified
                        .address_refs
                        .iter()
                        .filter_map(|value| AddressableEventCoordinate::parse(value).ok())
                        .filter(|coordinate| coordinate.kind == KIND_DATASET_LISTING)
                        .map(|coordinate| coordinate.to_string())
                        .collect::<Vec<_>>();
                    let offer_coordinates = classified
                        .address_refs
                        .iter()
                        .filter_map(|value| AddressableEventCoordinate::parse(value).ok())
                        .filter(|coordinate| coordinate.kind == KIND_DATASET_OFFER)
                        .map(|coordinate| coordinate.to_string())
                        .collect::<Vec<_>>();
                    let listing_wrapper = ClassifiedListingWrapperProjection {
                        coordinate: classified_coordinate.clone(),
                        event_id: event.id.clone(),
                        relay_url: relay_url.clone(),
                        title: classified.title.clone(),
                        summary: classified.summary.clone(),
                        price_amount: classified.price.as_ref().map(|price| price.amount.clone()),
                        price_currency: classified
                            .price
                            .as_ref()
                            .map(|price| price.currency.clone()),
                        created_at_seconds: event.created_at,
                    };
                    for listing_coordinate in &listing_coordinates {
                        if listing_wrappers
                            .get(listing_coordinate.as_str())
                            .is_none_or(|existing| existing.created_at_seconds <= event.created_at)
                        {
                            listing_wrappers
                                .insert(listing_coordinate.clone(), listing_wrapper.clone());
                        }
                    }
                    for offer_coordinate in offer_coordinates {
                        let offer_wrapper = ClassifiedOfferWrapperProjection {
                            coordinate: classified_coordinate.clone(),
                            event_id: event.id.clone(),
                            relay_url: relay_url.clone(),
                            listing_coordinate: listing_coordinates.first().cloned(),
                            price_amount: classified
                                .price
                                .as_ref()
                                .map(|price| price.amount.clone()),
                            price_currency: classified
                                .price
                                .as_ref()
                                .map(|price| price.currency.clone()),
                            created_at_seconds: event.created_at,
                        };
                        if offer_wrappers
                            .get(offer_coordinate.as_str())
                            .is_none_or(|existing| existing.created_at_seconds <= event.created_at)
                        {
                            offer_wrappers.insert(offer_coordinate, offer_wrapper);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    for (listing_coordinate, wrapper) in listing_wrappers {
        let linked_asset_id =
            link_hints.linked_asset_id_for_listing_coordinate(listing_coordinate.as_str());
        match listings.entry(listing_coordinate.clone()) {
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let projection = entry.get_mut();
                projection.classified_coordinate = Some(wrapper.coordinate.clone());
                projection.classified_event_id = Some(wrapper.event_id.clone());
                projection.classified_price_amount = wrapper.price_amount.clone();
                projection.classified_price_currency = wrapper.price_currency.clone();
                if projection.linked_asset_id.is_none() {
                    projection.linked_asset_id = linked_asset_id;
                }
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                let publisher_pubkey =
                    AddressableEventCoordinate::parse(listing_coordinate.as_str())
                        .map(|coordinate| coordinate.pubkey)
                        .unwrap_or_default();
                entry.insert(RelayDatasetListingProjection {
                    coordinate: listing_coordinate,
                    publisher_pubkey,
                    relay_url: Some(wrapper.relay_url),
                    title: wrapper.title,
                    summary: wrapper.summary,
                    dataset_kind: None,
                    access: Some("public_catalog".to_string()),
                    delivery_modes: Vec::new(),
                    created_at_seconds: wrapper.created_at_seconds,
                    draft: false,
                    linked_asset_id,
                    classified_coordinate: Some(wrapper.coordinate),
                    classified_event_id: Some(wrapper.event_id),
                    classified_price_amount: wrapper.price_amount,
                    classified_price_currency: wrapper.price_currency,
                    storefront_stall_coordinate: None,
                    storefront_stall_name: None,
                    storefront_product_coordinate: None,
                    storefront_product_event_id: None,
                    storefront_product_title: None,
                    storefront_product_price_amount: None,
                    storefront_product_price_currency: None,
                    discussion_channel_id: None,
                    discussion_channel_name: None,
                    discussion_channel_relay_url: None,
                });
            }
        }
    }
    for (offer_coordinate, wrapper) in offer_wrappers {
        let linked_grant_id =
            link_hints.linked_grant_id_for_offer_coordinate(offer_coordinate.as_str());
        let listing_coordinate = wrapper
            .listing_coordinate
            .clone()
            .or_else(|| {
                link_hints.listing_coordinate_for_offer_coordinate(offer_coordinate.as_str())
            })
            .unwrap_or_else(|| "unknown".to_string());
        let linked_asset_id = link_hints
            .linked_asset_id_for_offer_coordinate(offer_coordinate.as_str())
            .or_else(|| {
                link_hints.linked_asset_id_for_listing_coordinate(listing_coordinate.as_str())
            });
        match offers.entry(offer_coordinate.clone()) {
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let projection = entry.get_mut();
                projection.classified_coordinate = Some(wrapper.coordinate.clone());
                projection.classified_event_id = Some(wrapper.event_id.clone());
                if projection.price_amount.is_none() {
                    projection.price_amount = wrapper.price_amount.clone();
                }
                if projection.price_currency.is_none() {
                    projection.price_currency = wrapper.price_currency.clone();
                }
                if projection.linked_grant_id.is_none() {
                    projection.linked_grant_id = linked_grant_id.clone();
                }
                if projection.linked_asset_id.is_none() {
                    projection.linked_asset_id = linked_asset_id.clone();
                }
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                let publisher_pubkey = AddressableEventCoordinate::parse(offer_coordinate.as_str())
                    .map(|coordinate| coordinate.pubkey)
                    .unwrap_or_default();
                entry.insert(RelayDatasetOfferProjection {
                    coordinate: offer_coordinate,
                    listing_coordinate,
                    publisher_pubkey,
                    relay_url: Some(wrapper.relay_url),
                    status: "active".to_string(),
                    policy: None,
                    delivery_modes: Vec::new(),
                    targeted_buyer_pubkeys: Vec::new(),
                    price_amount: wrapper.price_amount,
                    price_currency: wrapper.price_currency,
                    created_at_seconds: wrapper.created_at_seconds,
                    linked_asset_id,
                    linked_grant_id,
                    classified_coordinate: Some(wrapper.coordinate),
                    classified_event_id: Some(wrapper.event_id),
                    storefront_stall_coordinate: None,
                    storefront_stall_name: None,
                    storefront_product_coordinate: None,
                    storefront_product_event_id: None,
                    storefront_product_title: None,
                    storefront_product_price_amount: None,
                    storefront_product_price_currency: None,
                    discussion_channel_id: None,
                    discussion_channel_name: None,
                    discussion_channel_relay_url: None,
                });
            }
        }
    }
    for (listing_coordinate, wrapper) in listing_storefront_products {
        let linked_asset_id =
            link_hints.linked_asset_id_for_listing_coordinate(listing_coordinate.as_str());
        let stall_name = stalls
            .get(wrapper.stall_coordinate.as_str())
            .map(|stall| stall.name.clone())
            .or(wrapper.stall_name.clone());
        match listings.entry(listing_coordinate.clone()) {
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let projection = entry.get_mut();
                projection.storefront_stall_coordinate = Some(wrapper.stall_coordinate.clone());
                projection.storefront_stall_name = stall_name.clone();
                projection.storefront_product_coordinate = Some(wrapper.coordinate.clone());
                projection.storefront_product_event_id = Some(wrapper.event_id.clone());
                projection.storefront_product_title = Some(wrapper.title.clone());
                projection.storefront_product_price_amount = Some(wrapper.price_amount.clone());
                projection.storefront_product_price_currency = Some(wrapper.price_currency.clone());
                if projection.linked_asset_id.is_none() {
                    projection.linked_asset_id = linked_asset_id;
                }
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                let publisher_pubkey =
                    AddressableEventCoordinate::parse(listing_coordinate.as_str())
                        .map(|coordinate| coordinate.pubkey)
                        .unwrap_or_default();
                entry.insert(RelayDatasetListingProjection {
                    coordinate: listing_coordinate,
                    publisher_pubkey,
                    relay_url: Some(wrapper.relay_url),
                    title: wrapper.title.clone(),
                    summary: Some("Storefront product wrapper".to_string()),
                    dataset_kind: None,
                    access: Some("storefront".to_string()),
                    delivery_modes: Vec::new(),
                    created_at_seconds: wrapper.created_at_seconds,
                    draft: false,
                    linked_asset_id,
                    classified_coordinate: None,
                    classified_event_id: None,
                    classified_price_amount: None,
                    classified_price_currency: None,
                    storefront_stall_coordinate: Some(wrapper.stall_coordinate),
                    storefront_stall_name: stall_name,
                    storefront_product_coordinate: Some(wrapper.coordinate),
                    storefront_product_event_id: Some(wrapper.event_id),
                    storefront_product_title: Some(wrapper.title),
                    storefront_product_price_amount: Some(wrapper.price_amount),
                    storefront_product_price_currency: Some(wrapper.price_currency),
                    discussion_channel_id: None,
                    discussion_channel_name: None,
                    discussion_channel_relay_url: None,
                });
            }
        }
    }
    for (offer_coordinate, wrapper) in offer_storefront_products {
        let linked_grant_id =
            link_hints.linked_grant_id_for_offer_coordinate(offer_coordinate.as_str());
        let listing_coordinate = link_hints
            .listing_coordinate_for_offer_coordinate(offer_coordinate.as_str())
            .unwrap_or_else(|| "unknown".to_string());
        let linked_asset_id = link_hints
            .linked_asset_id_for_offer_coordinate(offer_coordinate.as_str())
            .or_else(|| {
                link_hints.linked_asset_id_for_listing_coordinate(listing_coordinate.as_str())
            });
        let stall_name = stalls
            .get(wrapper.stall_coordinate.as_str())
            .map(|stall| stall.name.clone())
            .or(wrapper.stall_name.clone());
        match offers.entry(offer_coordinate.clone()) {
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let projection = entry.get_mut();
                projection.storefront_stall_coordinate = Some(wrapper.stall_coordinate.clone());
                projection.storefront_stall_name = stall_name.clone();
                projection.storefront_product_coordinate = Some(wrapper.coordinate.clone());
                projection.storefront_product_event_id = Some(wrapper.event_id.clone());
                projection.storefront_product_title = Some(wrapper.title.clone());
                projection.storefront_product_price_amount = Some(wrapper.price_amount.clone());
                projection.storefront_product_price_currency = Some(wrapper.price_currency.clone());
                if projection.linked_grant_id.is_none() {
                    projection.linked_grant_id = linked_grant_id.clone();
                }
                if projection.linked_asset_id.is_none() {
                    projection.linked_asset_id = linked_asset_id.clone();
                }
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                let publisher_pubkey = AddressableEventCoordinate::parse(offer_coordinate.as_str())
                    .map(|coordinate| coordinate.pubkey)
                    .unwrap_or_default();
                entry.insert(RelayDatasetOfferProjection {
                    coordinate: offer_coordinate,
                    listing_coordinate,
                    publisher_pubkey,
                    relay_url: Some(wrapper.relay_url),
                    status: "active".to_string(),
                    policy: Some("storefront".to_string()),
                    delivery_modes: Vec::new(),
                    targeted_buyer_pubkeys: Vec::new(),
                    price_amount: Some(wrapper.price_amount.clone()),
                    price_currency: Some(wrapper.price_currency.clone()),
                    created_at_seconds: wrapper.created_at_seconds,
                    linked_asset_id,
                    linked_grant_id,
                    classified_coordinate: None,
                    classified_event_id: None,
                    storefront_stall_coordinate: Some(wrapper.stall_coordinate),
                    storefront_stall_name: stall_name,
                    storefront_product_coordinate: Some(wrapper.coordinate),
                    storefront_product_event_id: Some(wrapper.event_id),
                    storefront_product_title: Some(wrapper.title),
                    storefront_product_price_amount: Some(wrapper.price_amount),
                    storefront_product_price_currency: Some(wrapper.price_currency),
                    discussion_channel_id: None,
                    discussion_channel_name: None,
                    discussion_channel_relay_url: None,
                });
            }
        }
    }
    for (listing_coordinate, discussion) in listing_discussions {
        if let Some(projection) = listings.get_mut(listing_coordinate.as_str()) {
            projection.discussion_channel_id = Some(discussion.channel_id.clone());
            projection.discussion_channel_name = Some(discussion.name.clone());
            projection.discussion_channel_relay_url = discussion.relay_url.clone();
        }
    }
    for (offer_coordinate, discussion) in offer_discussions {
        if let Some(projection) = offers.get_mut(offer_coordinate.as_str()) {
            projection.discussion_channel_id = Some(discussion.channel_id.clone());
            projection.discussion_channel_name = Some(discussion.name.clone());
            projection.discussion_channel_relay_url = discussion.relay_url.clone();
        }
    }

    for contract in access_contracts.values() {
        let Some(payment_hash) = contract.payment_hash.as_deref() else {
            continue;
        };
        for payment in payments.iter().filter(|payment| {
            payment
                .payment_hash
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(payment_hash))
        }) {
            let key = format!("{}:{}", payment.id, contract.request_event_id);
            settlement_matches
                .entry(key)
                .and_modify(|entry| {
                    entry.contract_coordinate = Some(contract.coordinate.clone());
                    entry.result_event_id = entry
                        .result_event_id
                        .clone()
                        .or_else(|| contract.result_event_id.clone());
                })
                .or_insert_with(|| RelayDatasetSettlementMatchProjection {
                    payment_pointer: payment.id.clone(),
                    payment_hash: payment_hash.to_string(),
                    direction: payment.direction.clone(),
                    status: payment.status.clone(),
                    amount_sats: payment.amount_sats,
                    observed_at_seconds: payment.timestamp,
                    contract_coordinate: Some(contract.coordinate.clone()),
                    request_event_id: Some(contract.request_event_id.clone()),
                    result_event_id: contract.result_event_id.clone(),
                });
        }
    }

    for result in results.values() {
        let Some(payment_hash) = result.payment_hash.as_deref() else {
            continue;
        };
        for payment in payments.iter().filter(|payment| {
            payment
                .payment_hash
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(payment_hash))
        }) {
            let key = format!("{}:{}", payment.id, result.request_event_id);
            settlement_matches
                .entry(key)
                .and_modify(|entry| {
                    entry.result_event_id = Some(result.event_id.clone());
                    if entry.contract_coordinate.is_none() {
                        entry.contract_coordinate = access_contracts
                            .values()
                            .find(|contract| {
                                contract
                                    .request_event_id
                                    .eq_ignore_ascii_case(result.request_event_id.as_str())
                            })
                            .map(|contract| contract.coordinate.clone());
                    }
                })
                .or_insert_with(|| RelayDatasetSettlementMatchProjection {
                    payment_pointer: payment.id.clone(),
                    payment_hash: payment_hash.to_string(),
                    direction: payment.direction.clone(),
                    status: payment.status.clone(),
                    amount_sats: payment.amount_sats,
                    observed_at_seconds: payment.timestamp,
                    contract_coordinate: access_contracts
                        .values()
                        .find(|contract| {
                            contract
                                .request_event_id
                                .eq_ignore_ascii_case(result.request_event_id.as_str())
                        })
                        .map(|contract| contract.coordinate.clone()),
                    request_event_id: Some(result.request_event_id.clone()),
                    result_event_id: Some(result.event_id.clone()),
                });
        }
    }

    RelayCatalogSnapshot {
        listings: listings.into_values().collect(),
        offers: offers.into_values().collect(),
        requests: requests.into_values().collect(),
        access_contracts: access_contracts.into_values().collect(),
        results: results.into_values().collect(),
        settlement_matches: settlement_matches.into_values().collect(),
    }
}

fn listing_coordinate_for_asset_ref(asset_ref: &str) -> Option<String> {
    let coordinate = AddressableEventCoordinate::parse(asset_ref).ok()?;
    (coordinate.kind == KIND_DATASET_LISTING).then(|| coordinate.to_string())
}

fn data_request_timeout_seconds(content: &str, created_at_seconds: u64) -> Option<u64> {
    let value = serde_json::from_str::<serde_json::Value>(content)
        .ok()?
        .get("timeout_seconds")?
        .clone();
    let timeout_seconds = match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(string) => string.parse::<u64>().ok(),
        _ => None,
    }?;
    Some(created_at_seconds.saturating_add(timeout_seconds))
}

fn payment_method_rail(method: &nostr::nip_ds::PaymentMethod) -> String {
    method.rail.clone()
}

fn marketplace_price_string(value: f64) -> String {
    let mut rendered = format!("{value:.8}");
    while rendered.contains('.') && rendered.ends_with('0') {
        rendered.pop();
    }
    if rendered.ends_with('.') {
        rendered.pop();
    }
    if rendered.is_empty() {
        "0".to_string()
    } else {
        rendered
    }
}

fn linked_asset_id_for_listing_coordinate(
    assets: &[openagents_kernel_core::data::DataAsset],
    coordinate: &str,
) -> Option<String> {
    assets.iter().find_map(|asset| {
        asset
            .nostr_publications
            .ds_listing
            .as_ref()
            .and_then(|reference| reference.coordinate.as_deref())
            .filter(|value| value.eq_ignore_ascii_case(coordinate))
            .map(|_| asset.asset_id.clone())
    })
}

fn linked_grant_id_for_offer_coordinate(
    grants: &[openagents_kernel_core::data::AccessGrant],
    coordinate: &str,
) -> Option<String> {
    grants.iter().find_map(|grant| {
        grant
            .nostr_publications
            .ds_offer
            .as_ref()
            .and_then(|reference| reference.coordinate.as_deref())
            .filter(|value| value.eq_ignore_ascii_case(coordinate))
            .map(|_| grant.grant_id.clone())
    })
}

fn linked_asset_id_for_offer_coordinate(
    grants: &[openagents_kernel_core::data::AccessGrant],
    coordinate: &str,
) -> Option<String> {
    linked_grant_id_for_offer_coordinate(grants, coordinate)
        .as_deref()
        .and_then(|grant_id| {
            grants
                .iter()
                .find(|grant| grant.grant_id == grant_id)
                .map(|grant| grant.asset_id.clone())
        })
}

fn build_relay_catalog_link_hints(
    existing_assets: &[openagents_kernel_core::data::DataAsset],
    existing_grants: &[openagents_kernel_core::data::AccessGrant],
    existing_listings: &[RelayDatasetListingProjection],
    existing_offers: &[RelayDatasetOfferProjection],
    existing_requests: &[RelayDatasetAccessRequestProjection],
    existing_access_contracts: &[RelayDatasetAccessContractProjection],
    existing_results: &[RelayDatasetAccessResultProjection],
) -> RelayCatalogLinkHints {
    let mut hints = RelayCatalogLinkHints::default();
    for asset in existing_assets {
        if let Some(coordinate) = asset
            .nostr_publications
            .ds_listing
            .as_ref()
            .and_then(|reference| reference.coordinate.as_deref())
        {
            hints.record_listing_asset(coordinate, asset.asset_id.as_str());
        }
    }
    for grant in existing_grants {
        if let Some(coordinate) = grant
            .nostr_publications
            .ds_offer
            .as_ref()
            .and_then(|reference| reference.coordinate.as_deref())
        {
            hints.record_offer_grant(coordinate, grant.grant_id.as_str());
            hints.record_offer_asset(coordinate, grant.asset_id.as_str());
        }
    }
    for listing in existing_listings {
        if let Some(asset_id) = listing.linked_asset_id.as_deref() {
            hints.record_listing_asset(listing.coordinate.as_str(), asset_id);
        }
    }
    for offer in existing_offers {
        hints.record_offer_listing_coordinate(
            offer.coordinate.as_str(),
            offer.listing_coordinate.as_str(),
        );
        if let Some(grant_id) = offer.linked_grant_id.as_deref() {
            hints.record_offer_grant(offer.coordinate.as_str(), grant_id);
        }
        if let Some(asset_id) = offer.linked_asset_id.as_deref() {
            hints.record_offer_asset(offer.coordinate.as_str(), asset_id);
            hints.record_listing_asset(offer.listing_coordinate.as_str(), asset_id);
        }
    }
    for request in existing_requests {
        if let Some(asset_id) = request.linked_asset_id.as_deref() {
            hints.record_listing_asset(request.listing_coordinate.as_str(), asset_id);
        }
        if let Some(offer_coordinate) = request.offer_coordinate.as_deref() {
            hints.record_offer_listing_coordinate(
                offer_coordinate,
                request.listing_coordinate.as_str(),
            );
            if let Some(grant_id) = request.linked_grant_id.as_deref() {
                hints.record_offer_grant(offer_coordinate, grant_id);
            }
            if let Some(asset_id) = request.linked_asset_id.as_deref() {
                hints.record_offer_asset(offer_coordinate, asset_id);
            }
        }
    }
    for contract in existing_access_contracts {
        if let Some(asset_id) = contract.linked_asset_id.as_deref() {
            hints.record_listing_asset(contract.listing_coordinate.as_str(), asset_id);
        }
        if let Some(offer_coordinate) = contract.offer_coordinate.as_deref() {
            hints.record_offer_listing_coordinate(
                offer_coordinate,
                contract.listing_coordinate.as_str(),
            );
            if let Some(grant_id) = contract.linked_grant_id.as_deref() {
                hints.record_offer_grant(offer_coordinate, grant_id);
            }
            if let Some(asset_id) = contract.linked_asset_id.as_deref() {
                hints.record_offer_asset(offer_coordinate, asset_id);
            }
        }
    }
    for result in existing_results {
        if let Some(asset_id) = result.linked_asset_id.as_deref() {
            hints.record_listing_asset(result.listing_coordinate.as_str(), asset_id);
        }
        if let Some(offer_coordinate) = result.offer_coordinate.as_deref() {
            hints.record_offer_listing_coordinate(
                offer_coordinate,
                result.listing_coordinate.as_str(),
            );
            if let Some(grant_id) = result.linked_grant_id.as_deref() {
                hints.record_offer_grant(offer_coordinate, grant_id);
            }
            if let Some(asset_id) = result.linked_asset_id.as_deref() {
                hints.record_offer_asset(offer_coordinate, asset_id);
            }
        }
    }
    hints
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        PersistedRelayCatalogSnapshot, RelayCatalogSnapshot, build_relay_catalog_link_hints,
        load_persisted_relay_catalog_from_path, persist_relay_catalog_to_path,
        project_relay_catalog,
    };
    use nostr::nip_ds::{
        AddressableEventCoordinate, AddressableEventReference, DatasetAccessContract,
        DatasetAccessContractStatus, DatasetListing, DatasetOffer, EventReference, PaymentMethod,
        PublicKeyReference,
    };
    use nostr::nip15::{MarketplaceProduct, MarketplaceStall};
    use nostr::nip90::{
        DataVendingDeliveryMode, DataVendingPreviewPosture, DataVendingRequest, DataVendingResult,
        create_data_vending_request_event, create_data_vending_result_event,
    };
    use nostr::nip99::{ClassifiedListing, ListingStatus, Price};
    use openagents_kernel_core::data::{
        AccessGrant, AccessGrantNostrPublications, AccessGrantStatus, DataAsset,
        DataAssetNostrPublications, NostrPublicationRef, PermissionPolicy,
    };
    use openagents_kernel_core::receipts::{Asset, Money, MoneyAmount};
    use openagents_spark::PaymentSummary;

    const VALID_TEST_INVOICE: &str = "lnbc10u1p3unwfusp5t9r3yymhpfqculx78u027lxspgxcr2n2987mx2j55nnfs95nxnzqpp5jmrh92pfld78spqs78v9euf2385t83uvpwk9ldrlvf6ch7tpascqhp5zvkrmemgth3tufcvflmzjzfvjt023nazlhljz2n9hattj4f8jq8qxqyjw5qcqpjrzjqtc4fc44feggv7065fqe5m4ytjarg3repr5j9el35xhmtfexc42yczarjuqqfzqqqqqqqqlgqqqqqqgq9q9qxpqysgq079nkq507a5tw7xgttmj4u990j7wfggtrasah5gd4ywfr2pjcn29383tphp4t48gquelz9z78p4cq7ml3nrrphw5w6eckhjwmhezhnqpy6gyf0";

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
            build_relay_catalog_link_hints(&[asset], &[grant], &[], &[], &[], &[], &[]),
            &[],
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

    #[test]
    fn relay_catalog_projection_preserves_existing_link_hints_without_kernel_records() {
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
                AddressableEventCoordinate::parse(listing_coordinate.as_str())
                    .expect("listing coordinate parse"),
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

        let snapshot = project_relay_catalog(
            vec![
                ("wss://relay.example".to_string(), listing_event),
                ("wss://relay.example".to_string(), offer_event),
            ],
            build_relay_catalog_link_hints(
                &[],
                &[],
                &[super::RelayDatasetListingProjection {
                    coordinate: listing_coordinate.clone(),
                    publisher_pubkey: identity.public_key_hex.clone(),
                    relay_url: Some("wss://relay.example".to_string()),
                    title: "Seller bundle".to_string(),
                    summary: None,
                    dataset_kind: Some("conversation_bundle".to_string()),
                    access: Some("paid".to_string()),
                    delivery_modes: vec!["encrypted_pointer".to_string()],
                    created_at_seconds: 1_762_700_000,
                    draft: false,
                    linked_asset_id: Some("data_asset.provider.alpha.bundle".to_string()),
                    classified_coordinate: None,
                    classified_event_id: None,
                    classified_price_amount: None,
                    classified_price_currency: None,
                    storefront_stall_coordinate: None,
                    storefront_stall_name: None,
                    storefront_product_coordinate: None,
                    storefront_product_event_id: None,
                    storefront_product_title: None,
                    storefront_product_price_amount: None,
                    storefront_product_price_currency: None,
                    discussion_channel_id: None,
                    discussion_channel_name: None,
                    discussion_channel_relay_url: None,
                }],
                &[super::RelayDatasetOfferProjection {
                    coordinate: offer_coordinate.clone(),
                    listing_coordinate: listing_coordinate.clone(),
                    publisher_pubkey: identity.public_key_hex.clone(),
                    relay_url: Some("wss://relay.example".to_string()),
                    status: "active".to_string(),
                    policy: Some("targeted_request".to_string()),
                    delivery_modes: vec!["encrypted_pointer".to_string()],
                    targeted_buyer_pubkeys: vec!["npub1buyer".to_string()],
                    price_amount: Some("42".to_string()),
                    price_currency: Some("SAT".to_string()),
                    created_at_seconds: 1_762_700_010,
                    linked_asset_id: Some("data_asset.provider.alpha.bundle".to_string()),
                    linked_grant_id: Some("grant.data.offer.001".to_string()),
                    classified_coordinate: None,
                    classified_event_id: None,
                    storefront_stall_coordinate: None,
                    storefront_stall_name: None,
                    storefront_product_coordinate: None,
                    storefront_product_event_id: None,
                    storefront_product_title: None,
                    storefront_product_price_amount: None,
                    storefront_product_price_currency: None,
                    discussion_channel_id: None,
                    discussion_channel_name: None,
                    discussion_channel_relay_url: None,
                }],
                &[],
                &[],
                &[],
            ),
            &[],
        );

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

    #[test]
    fn relay_catalog_projection_surfaces_nip99_wrappers_on_ds_coordinates() {
        let identity = nostr::regenerate_identity().expect("identity");
        let listing_coordinate = format!(
            "30404:{}:data_asset.provider.alpha.bundle",
            identity.public_key_hex
        );
        let offer_coordinate = format!("30406:{}:grant.data.offer.001", identity.public_key_hex);
        let asset = DataAsset {
            asset_id: "data_asset.provider.alpha.bundle".to_string(),
            provider_id: identity.public_key_hex.clone(),
            asset_kind: "conversation_bundle".to_string(),
            title: "Seller bundle".to_string(),
            description: Some("Public catalog wrapper only.".to_string()),
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
            consumer_id: None,
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

        let mut classified = ClassifiedListing::new(
            "catalog.data_asset.provider.alpha.bundle",
            "Public catalog wrapper",
            "Seller bundle",
        )
        .with_summary("NIP-99 wrapper for DS")
        .with_published_at(1_762_700_020)
        .with_price(Price::one_time("42", "SAT"))
        .with_status(ListingStatus::Active);
        classified.add_tag("dataset");
        classified.add_address_ref(listing_coordinate.clone());
        classified.add_address_ref(offer_coordinate.clone());
        let wrapper_coordinate = classified
            .coordinate(identity.public_key_hex.clone())
            .expect("wrapper coordinate");
        let wrapper_event = sign_template(
            &identity,
            &classified
                .to_event_template(1_762_700_020)
                .expect("classified template"),
        );

        let snapshot = project_relay_catalog(
            vec![("wss://relay.example".to_string(), wrapper_event)],
            build_relay_catalog_link_hints(&[asset], &[grant], &[], &[], &[], &[], &[]),
            &[],
        );

        assert_eq!(snapshot.listings.len(), 1);
        assert_eq!(snapshot.offers.len(), 1);
        assert_eq!(snapshot.listings[0].coordinate, listing_coordinate);
        assert_eq!(
            snapshot.listings[0].classified_coordinate.as_deref(),
            Some(wrapper_coordinate.as_str())
        );
        assert_eq!(
            snapshot.listings[0].classified_price_amount.as_deref(),
            Some("42")
        );
        assert_eq!(snapshot.offers[0].coordinate, offer_coordinate);
        assert_eq!(
            snapshot.offers[0].classified_coordinate.as_deref(),
            Some(wrapper_coordinate.as_str())
        );
        assert_eq!(
            snapshot.offers[0].linked_grant_id.as_deref(),
            Some("grant.data.offer.001")
        );
        assert_eq!(snapshot.offers[0].listing_coordinate, listing_coordinate);
    }

    #[test]
    fn relay_catalog_projection_surfaces_nip15_storefront_wrappers_on_ds_coordinates() {
        let identity = nostr::regenerate_identity().expect("identity");
        let listing_coordinate = format!(
            "30404:{}:data_asset.provider.alpha.bundle",
            identity.public_key_hex
        );
        let offer_coordinate = format!("30406:{}:grant.data.offer.001", identity.public_key_hex);
        let asset = DataAsset {
            asset_id: "data_asset.provider.alpha.bundle".to_string(),
            provider_id: identity.public_key_hex.clone(),
            asset_kind: "conversation_bundle".to_string(),
            title: "Seller bundle".to_string(),
            description: Some("Storefront wrapper only.".to_string()),
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
            consumer_id: None,
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

        let stall = MarketplaceStall::new("datasets.sat", "OpenAgents datasets (SAT)", "SAT")
            .with_description("Storefront for DS datasets.");
        let stall_coordinate = stall
            .coordinate(identity.public_key_hex.clone())
            .expect("stall coordinate");
        let stall_event = sign_template(
            &identity,
            &stall
                .to_event_template(1_762_700_015)
                .expect("stall template"),
        );

        let mut product = MarketplaceProduct::new(
            "storefront.data_asset.provider.alpha.bundle",
            "datasets.sat",
            "Seller bundle",
            "SAT",
            42.0,
        )
        .expect("product");
        product.add_tag("dataset");
        product.add_tag("nip-ds");
        product.add_address_ref(listing_coordinate.clone());
        product.add_address_ref(offer_coordinate.clone());
        let product_coordinate = product
            .coordinate(identity.public_key_hex.clone())
            .expect("product coordinate");
        let product_event = sign_template(
            &identity,
            &product
                .to_event_template(1_762_700_020)
                .expect("product template"),
        );

        let snapshot = project_relay_catalog(
            vec![
                ("wss://relay.example".to_string(), stall_event),
                ("wss://relay.example".to_string(), product_event),
            ],
            build_relay_catalog_link_hints(&[asset], &[grant], &[], &[], &[], &[], &[]),
            &[],
        );

        assert_eq!(snapshot.listings.len(), 1);
        assert_eq!(snapshot.offers.len(), 1);
        assert_eq!(
            snapshot.listings[0].storefront_stall_coordinate.as_deref(),
            Some(stall_coordinate.as_str())
        );
        assert_eq!(
            snapshot.listings[0].storefront_stall_name.as_deref(),
            Some("OpenAgents datasets (SAT)")
        );
        assert_eq!(
            snapshot.listings[0]
                .storefront_product_coordinate
                .as_deref(),
            Some(product_coordinate.as_str())
        );
        assert_eq!(
            snapshot.listings[0]
                .storefront_product_price_amount
                .as_deref(),
            Some("42")
        );
        assert_eq!(
            snapshot.offers[0].storefront_product_coordinate.as_deref(),
            Some(product_coordinate.as_str())
        );
        assert_eq!(
            snapshot.offers[0].storefront_product_title.as_deref(),
            Some("Seller bundle")
        );
        assert_eq!(
            snapshot.offers[0]
                .storefront_product_price_currency
                .as_deref(),
            Some("SAT")
        );
    }

    #[test]
    fn relay_catalog_projection_links_dataset_discussion_channels_to_ds_entries() {
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
        let channel_create_id = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
        let channel_metadata =
            nostr::ChannelMetadata::new("Corpus Q&A", "Public dataset discussion", "");
        let discussion_template = nostr::EventTemplate {
            created_at: 1_762_700_020,
            kind: nostr::KIND_CHANNEL_METADATA,
            tags: nostr::ChannelMetadataEvent::new(
                channel_create_id,
                channel_metadata.clone(),
                1_762_700_020,
            )
            .with_relay_url("wss://relay.example")
            .with_dataset_discussion_refs(
                AddressableEventReference::new(
                    AddressableEventCoordinate::parse(listing_coordinate.as_str())
                        .expect("listing coordinate parse"),
                ),
                Some(AddressableEventReference::new(
                    AddressableEventCoordinate::parse(offer_coordinate.as_str())
                        .expect("offer coordinate parse"),
                )),
            )
            .to_tags(),
            content: channel_metadata.to_json().expect("channel metadata json"),
        };
        let discussion_event = sign_template(&identity, &discussion_template);

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
            consumer_id: None,
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
                ("wss://relay.example".to_string(), discussion_event),
            ],
            build_relay_catalog_link_hints(&[asset], &[grant], &[], &[], &[], &[], &[]),
            &[],
        );

        assert_eq!(
            snapshot.listings[0].discussion_channel_id.as_deref(),
            Some(channel_create_id)
        );
        assert_eq!(
            snapshot.listings[0].discussion_channel_name.as_deref(),
            Some("Corpus Q&A")
        );
        assert_eq!(
            snapshot.offers[0].discussion_channel_id.as_deref(),
            Some(channel_create_id)
        );
        assert_eq!(
            snapshot.offers[0].discussion_channel_name.as_deref(),
            Some("Corpus Q&A")
        );
    }

    #[test]
    fn relay_catalog_projection_surfaces_ds_access_lifecycle_and_wallet_match() {
        let seller = nostr::regenerate_identity().expect("seller identity");
        let buyer = nostr::regenerate_identity().expect("buyer identity");

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
            .coordinate(seller.public_key_hex.clone())
            .expect("listing coordinate");
        let listing_coordinate_string = listing_coordinate.to_string();
        let listing_event = sign_template(
            &seller,
            &listing
                .to_event_template(1_762_700_000)
                .expect("listing template"),
        );

        let offer = DatasetOffer::new(
            "grant.data.offer.001",
            "Targeted access",
            AddressableEventReference::new(listing_coordinate.clone()),
        )
        .with_policy("targeted_request")
        .with_price(Price::one_time("42", "SAT"))
        .add_payment_method(PaymentMethod::new("ln"))
        .add_delivery_mode("encrypted_pointer");
        let offer_coordinate = offer
            .coordinate(seller.public_key_hex.clone())
            .expect("offer coordinate");
        let offer_coordinate_string = offer_coordinate.to_string();
        let offer_event = sign_template(
            &seller,
            &offer
                .to_event_template(1_762_700_010)
                .expect("offer template"),
        );

        let request = DataVendingRequest::new(
            nostr::KIND_DATASET_ACCESS_REQUEST,
            "data_asset.provider.alpha.bundle",
            "download",
        )
        .expect("request")
        .with_listing_ref(AddressableEventReference::new(listing_coordinate.clone()))
        .with_offer_ref(AddressableEventReference::new(offer_coordinate.clone()))
        .with_asset_id("data_asset.provider.alpha.bundle")
        .with_grant_id("grant.data.offer.001")
        .with_delivery_mode(DataVendingDeliveryMode::EncryptedPointer)
        .with_preview_posture(DataVendingPreviewPosture::MetadataOnly)
        .with_bid(42_000)
        .add_service_provider(seller.public_key_hex.clone())
        .add_relay("wss://relay.example")
        .with_content(r#"{"timeout_seconds":120}"#);
        let request_event = sign_template(
            &buyer,
            &create_data_vending_request_event(&request).expect("request template"),
        );

        let contract = DatasetAccessContract::new(
            "contract.alpha.bundle",
            "Payment required",
            AddressableEventReference::new(listing_coordinate.clone()),
            EventReference::new(request_event.id.clone()).expect("request ref"),
            PublicKeyReference::new(buyer.public_key_hex.clone()).expect("buyer ref"),
        )
        .with_offer_ref(AddressableEventReference::new(offer_coordinate.clone()))
        .with_status(DatasetAccessContractStatus::Paid)
        .with_payment_method(PaymentMethod::new("ln"))
        .with_amount_msats(42_000, Some(VALID_TEST_INVOICE.to_string()))
        .with_delivery_mode("encrypted_pointer")
        .with_delivery_ref("https://delivery.example/contracts/alpha")
        .with_delivery_mime_type("application/json")
        .with_delivery_digest("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        .with_expires_at(1_762_700_240);
        let contract_event = sign_template(
            &seller,
            &contract
                .to_event_template(1_762_700_040)
                .expect("contract template"),
        );
        let contract_coordinate = contract
            .coordinate(seller.public_key_hex.clone())
            .expect("contract coordinate")
            .to_string();

        let result = DataVendingResult::new(
            nostr::KIND_DATASET_ACCESS_REQUEST,
            request_event.id.clone(),
            buyer.public_key_hex.clone(),
            "data_asset.provider.alpha.bundle",
            "delivery.alpha.bundle",
            r#"{"status":"delivered"}"#,
        )
        .expect("result")
        .with_listing_ref(AddressableEventReference::new(listing_coordinate.clone()))
        .with_offer_ref(AddressableEventReference::new(offer_coordinate.clone()))
        .with_asset_id("data_asset.provider.alpha.bundle")
        .with_grant_id("grant.data.offer.001")
        .with_delivery_mode(DataVendingDeliveryMode::EncryptedPointer)
        .with_preview_posture(DataVendingPreviewPosture::MetadataOnly)
        .with_amount(42_000, Some(VALID_TEST_INVOICE.to_string()));
        let result_event = sign_template(
            &seller,
            &create_data_vending_result_event(&result).expect("result template"),
        );

        let payment_hash =
            crate::spark_wallet::decode_lightning_invoice_payment_hash(VALID_TEST_INVOICE)
                .expect("payment hash");
        let payment = PaymentSummary {
            id: "payment-001".to_string(),
            direction: "outbound".to_string(),
            status: "completed".to_string(),
            amount_sats: 42,
            timestamp: 1_762_700_050,
            invoice: Some(VALID_TEST_INVOICE.to_string()),
            payment_hash: Some(payment_hash.clone()),
            ..Default::default()
        };

        let asset = DataAsset {
            asset_id: "data_asset.provider.alpha.bundle".to_string(),
            provider_id: seller.public_key_hex.clone(),
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
                    coordinate: Some(listing_coordinate_string.clone()),
                    event_id: Some(listing_event.id.clone()),
                    relay_url: Some("wss://relay.example".to_string()),
                }),
                ds_draft_listing: None,
            },
            metadata: serde_json::json!({}),
        };
        let grant = AccessGrant {
            grant_id: "grant.data.offer.001".to_string(),
            asset_id: asset.asset_id.clone(),
            provider_id: seller.public_key_hex.clone(),
            consumer_id: Some(buyer.public_key_hex.clone()),
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
                    coordinate: Some(offer_coordinate_string.clone()),
                    event_id: Some(offer_event.id.clone()),
                    relay_url: Some("wss://relay.example".to_string()),
                }),
                ds_access_request: Some(NostrPublicationRef {
                    coordinate: None,
                    event_id: Some(request_event.id.clone()),
                    relay_url: Some("wss://relay.example".to_string()),
                }),
                ds_access_result: Some(NostrPublicationRef {
                    coordinate: None,
                    event_id: Some(result_event.id.clone()),
                    relay_url: Some("wss://relay.example".to_string()),
                }),
            },
            metadata: serde_json::json!({}),
        };

        let snapshot = project_relay_catalog(
            vec![
                ("wss://relay.example".to_string(), listing_event),
                ("wss://relay.example".to_string(), offer_event),
                ("wss://relay.example".to_string(), request_event.clone()),
                ("wss://relay.example".to_string(), contract_event),
                ("wss://relay.example".to_string(), result_event.clone()),
            ],
            build_relay_catalog_link_hints(&[asset], &[grant], &[], &[], &[], &[], &[]),
            &[payment],
        );

        assert_eq!(snapshot.requests.len(), 1);
        assert_eq!(
            snapshot.requests[0].listing_coordinate,
            listing_coordinate_string
        );
        assert_eq!(
            snapshot.requests[0].offer_coordinate.as_deref(),
            Some(offer_coordinate_string.as_str())
        );
        assert_eq!(
            snapshot.requests[0].expires_at_seconds,
            Some(request_event.created_at.saturating_add(120))
        );
        assert_eq!(
            snapshot.requests[0].linked_grant_id.as_deref(),
            Some("grant.data.offer.001")
        );

        assert_eq!(snapshot.access_contracts.len(), 1);
        assert_eq!(snapshot.access_contracts[0].coordinate, contract_coordinate);
        assert_eq!(
            snapshot.access_contracts[0].payment_hash.as_deref(),
            Some(payment_hash.as_str())
        );
        assert_eq!(
            snapshot.access_contracts[0].linked_asset_id.as_deref(),
            Some("data_asset.provider.alpha.bundle")
        );

        assert_eq!(snapshot.results.len(), 1);
        assert_eq!(snapshot.results[0].request_event_id, request_event.id);
        assert_eq!(
            snapshot.results[0].payment_hash.as_deref(),
            Some(payment_hash.as_str())
        );

        assert_eq!(snapshot.settlement_matches.len(), 1);
        assert_eq!(snapshot.settlement_matches[0].payment_hash, payment_hash);
        assert_eq!(
            snapshot.settlement_matches[0].result_event_id.as_deref(),
            Some(result_event.id.as_str())
        );
    }

    #[test]
    fn relay_catalog_snapshot_persists_and_loads_extended_replica() {
        let snapshot = RelayCatalogSnapshot {
            listings: vec![super::RelayDatasetListingProjection {
                coordinate: "30404:pubkey:asset.alpha".to_string(),
                publisher_pubkey: "pubkey".to_string(),
                relay_url: Some("wss://relay.example".to_string()),
                title: "Dataset alpha".to_string(),
                summary: Some("alpha summary".to_string()),
                dataset_kind: Some("bundle".to_string()),
                access: Some("paid".to_string()),
                delivery_modes: vec!["encrypted_pointer".to_string()],
                created_at_seconds: 10,
                draft: false,
                linked_asset_id: Some("asset.alpha".to_string()),
                classified_coordinate: None,
                classified_event_id: None,
                classified_price_amount: None,
                classified_price_currency: None,
                storefront_stall_coordinate: None,
                storefront_stall_name: None,
                storefront_product_coordinate: None,
                storefront_product_event_id: None,
                storefront_product_title: None,
                storefront_product_price_amount: None,
                storefront_product_price_currency: None,
                discussion_channel_id: None,
                discussion_channel_name: None,
                discussion_channel_relay_url: None,
            }],
            offers: vec![super::RelayDatasetOfferProjection {
                coordinate: "30406:pubkey:grant.alpha".to_string(),
                listing_coordinate: "30404:pubkey:asset.alpha".to_string(),
                publisher_pubkey: "pubkey".to_string(),
                relay_url: Some("wss://relay.example".to_string()),
                status: "active".to_string(),
                policy: Some("targeted_request".to_string()),
                delivery_modes: vec!["encrypted_pointer".to_string()],
                targeted_buyer_pubkeys: vec!["buyer".to_string()],
                price_amount: Some("5".to_string()),
                price_currency: Some("SAT".to_string()),
                created_at_seconds: 20,
                linked_asset_id: Some("asset.alpha".to_string()),
                linked_grant_id: Some("grant.alpha".to_string()),
                classified_coordinate: None,
                classified_event_id: None,
                storefront_stall_coordinate: None,
                storefront_stall_name: None,
                storefront_product_coordinate: None,
                storefront_product_event_id: None,
                storefront_product_title: None,
                storefront_product_price_amount: None,
                storefront_product_price_currency: None,
                discussion_channel_id: None,
                discussion_channel_name: None,
                discussion_channel_relay_url: None,
            }],
            requests: vec![super::RelayDatasetAccessRequestProjection {
                event_id: "request-001".to_string(),
                request_kind: nostr::KIND_DATASET_ACCESS_REQUEST,
                requester_pubkey: "buyer".to_string(),
                relay_url: Some("wss://relay.example".to_string()),
                listing_coordinate: "30404:pubkey:asset.alpha".to_string(),
                offer_coordinate: Some("30406:pubkey:grant.alpha".to_string()),
                asset_ref: "asset.alpha".to_string(),
                asset_id: Some("asset.alpha".to_string()),
                grant_id: Some("grant.alpha".to_string()),
                targeted_provider_pubkeys: vec!["pubkey".to_string()],
                permission_scopes: vec!["download".to_string()],
                delivery_mode: "encrypted_pointer".to_string(),
                preview_posture: "metadata_only".to_string(),
                bid_msats: Some(5_000),
                encrypted: false,
                created_at_seconds: 30,
                expires_at_seconds: Some(150),
                linked_asset_id: Some("asset.alpha".to_string()),
                linked_grant_id: Some("grant.alpha".to_string()),
            }],
            access_contracts: vec![super::RelayDatasetAccessContractProjection {
                coordinate: "30407:pubkey:contract.alpha".to_string(),
                seller_pubkey: "pubkey".to_string(),
                buyer_pubkey: "buyer".to_string(),
                relay_url: Some("wss://relay.example".to_string()),
                listing_coordinate: "30404:pubkey:asset.alpha".to_string(),
                offer_coordinate: Some("30406:pubkey:grant.alpha".to_string()),
                request_event_id: "request-001".to_string(),
                result_event_id: Some("result-001".to_string()),
                status: "paid".to_string(),
                payment_method: Some("ln".to_string()),
                amount_msats: Some(5_000),
                bolt11: Some("lnbc1example".to_string()),
                payment_hash: Some("abc123".to_string()),
                payment_evidence_event_ids: vec!["payment-evidence-001".to_string()],
                delivery_mode: Some("encrypted_pointer".to_string()),
                delivery_ref: Some("https://delivery.example/contracts/alpha".to_string()),
                delivery_mime_type: Some("application/json".to_string()),
                delivery_digest: Some(
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
                ),
                created_at_seconds: 40,
                expires_at_seconds: Some(160),
                reason_code: None,
                linked_asset_id: Some("asset.alpha".to_string()),
                linked_grant_id: Some("grant.alpha".to_string()),
            }],
            results: vec![super::RelayDatasetAccessResultProjection {
                event_id: "result-001".to_string(),
                seller_pubkey: "pubkey".to_string(),
                buyer_pubkey: "buyer".to_string(),
                relay_url: Some("wss://relay.example".to_string()),
                request_event_id: "request-001".to_string(),
                listing_coordinate: "30404:pubkey:asset.alpha".to_string(),
                offer_coordinate: Some("30406:pubkey:grant.alpha".to_string()),
                asset_ref: "asset.alpha".to_string(),
                asset_id: Some("asset.alpha".to_string()),
                grant_id: Some("grant.alpha".to_string()),
                delivery_bundle_id: "bundle-001".to_string(),
                delivery_mode: "encrypted_pointer".to_string(),
                preview_posture: "metadata_only".to_string(),
                delivery_ref: Some("https://delivery.example/results/alpha".to_string()),
                delivery_digest: Some(
                    "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210".to_string(),
                ),
                amount_msats: Some(5_000),
                bolt11: Some("lnbc1example".to_string()),
                payment_hash: Some("abc123".to_string()),
                created_at_seconds: 50,
                linked_asset_id: Some("asset.alpha".to_string()),
                linked_grant_id: Some("grant.alpha".to_string()),
            }],
            settlement_matches: vec![super::RelayDatasetSettlementMatchProjection {
                payment_pointer: "payment-001".to_string(),
                payment_hash: "abc123".to_string(),
                direction: "outbound".to_string(),
                status: "completed".to_string(),
                amount_sats: 5,
                observed_at_seconds: 55,
                contract_coordinate: Some("30407:pubkey:contract.alpha".to_string()),
                request_event_id: Some("request-001".to_string()),
                result_event_id: Some("result-001".to_string()),
            }],
        };

        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("replica.json");
        persist_relay_catalog_to_path(&snapshot, &path).expect("persist relay catalog");

        let stored: PersistedRelayCatalogSnapshot =
            serde_json::from_str(fs::read_to_string(&path).expect("read replica").as_str())
                .expect("decode persisted schema");
        assert_eq!(stored.schema_version, 1);

        let loaded = load_persisted_relay_catalog_from_path(&path).expect("load relay catalog");
        assert_eq!(loaded.listings, snapshot.listings);
        assert_eq!(loaded.offers, snapshot.offers);
        assert_eq!(loaded.requests, snapshot.requests);
        assert_eq!(loaded.access_contracts, snapshot.access_contracts);
        assert_eq!(loaded.results, snapshot.results);
        assert_eq!(loaded.settlement_matches, snapshot.settlement_matches);
    }
}
