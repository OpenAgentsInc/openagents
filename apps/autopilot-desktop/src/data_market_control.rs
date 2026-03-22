use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use nostr::Event;
use nostr::nip_ds::{
    AddressableEventCoordinate, DatasetListing, DatasetOffer, DraftDatasetListing,
    KIND_DATASET_LISTING, KIND_DATASET_OFFER,
};
use nostr::nip15::{KIND_PRODUCT, KIND_STALL, MarketplaceProduct, MarketplaceStall};
use nostr::nip28::{KIND_CHANNEL_METADATA, parse_dataset_discussion_channel_link};
use nostr::nip99::{ClassifiedListing, KIND_CLASSIFIED_LISTING};
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
            vec![
                serde_json::json!({
                    "kinds": [30017, 30018, 30402, 30404, 30405, 30406],
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
    assets: &[openagents_kernel_core::data::DataAsset],
    grants: &[openagents_kernel_core::data::AccessGrant],
) -> RelayCatalogSnapshot {
    let mut listings = BTreeMap::<String, RelayDatasetListingProjection>::new();
    let mut offers = BTreeMap::<String, RelayDatasetOfferProjection>::new();
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
            linked_asset_id_for_listing_coordinate(assets, listing_coordinate.as_str());
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
            linked_grant_id_for_offer_coordinate(grants, offer_coordinate.as_str());
        let listing_coordinate = wrapper
            .listing_coordinate
            .clone()
            .or_else(|| {
                linked_grant_id.as_deref().and_then(|grant_id| {
                    grants
                        .iter()
                        .find(|grant| grant.grant_id == grant_id)
                        .and_then(|grant| {
                            assets.iter().find_map(|asset| {
                                (asset.asset_id == grant.asset_id)
                                    .then(|| {
                                        asset
                                            .nostr_publications
                                            .ds_listing
                                            .as_ref()
                                            .and_then(|reference| reference.coordinate.clone())
                                    })
                                    .flatten()
                            })
                        })
                })
            })
            .unwrap_or_else(|| "unknown".to_string());
        let linked_asset_id =
            linked_asset_id_for_offer_coordinate(grants, offer_coordinate.as_str()).or_else(|| {
                linked_asset_id_for_listing_coordinate(assets, listing_coordinate.as_str())
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
            linked_asset_id_for_listing_coordinate(assets, listing_coordinate.as_str());
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
            linked_grant_id_for_offer_coordinate(grants, offer_coordinate.as_str());
        let listing_coordinate = linked_grant_id
            .as_deref()
            .and_then(|grant_id| {
                grants
                    .iter()
                    .find(|grant| grant.grant_id == grant_id)
                    .and_then(|grant| {
                        assets.iter().find_map(|asset| {
                            (asset.asset_id == grant.asset_id)
                                .then(|| {
                                    asset
                                        .nostr_publications
                                        .ds_listing
                                        .as_ref()
                                        .and_then(|reference| reference.coordinate.clone())
                                })
                                .flatten()
                        })
                    })
            })
            .unwrap_or_else(|| "unknown".to_string());
        let linked_asset_id =
            linked_asset_id_for_offer_coordinate(grants, offer_coordinate.as_str()).or_else(|| {
                linked_asset_id_for_listing_coordinate(assets, listing_coordinate.as_str())
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

    RelayCatalogSnapshot {
        listings: listings.into_values().collect(),
        offers: offers.into_values().collect(),
    }
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

#[cfg(test)]
mod tests {
    use super::project_relay_catalog;
    use nostr::nip_ds::{
        AddressableEventCoordinate, AddressableEventReference, DatasetListing, DatasetOffer,
        PaymentMethod,
    };
    use nostr::nip15::{MarketplaceProduct, MarketplaceStall};
    use nostr::nip99::{ClassifiedListing, ListingStatus, Price};
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
            &[asset],
            &[grant],
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
            &[asset],
            &[grant],
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
            &[asset],
            &[grant],
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
}
