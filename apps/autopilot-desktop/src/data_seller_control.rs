use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use codex_client::{ThreadResumeParams, ThreadStartParams, TurnStartParams, UserInput};
use nostr::nip_ds::{
    AddressableEventCoordinate, AddressableEventReference, DatasetAccessContract,
    DatasetAccessContractStatus, DatasetListing, DatasetOffer, DatasetOfferStatus,
    EventReference, PaymentMethod, PublicKeyReference,
};
use nostr::nip15::{MarketplaceProduct, MarketplaceStall};
use nostr::nip90::{
    DataVendingDeliveryMode, DataVendingFeedback, DataVendingPreviewPosture, DataVendingResult,
    JobStatus, create_data_vending_feedback_event, create_data_vending_result_event,
};
use nostr::nip99::{ClassifiedListing, ListingStatus, Price};
use nostr::{Event, EventTemplate, NostrIdentity};
use nostr_client::{PoolConfig, RelayMessage, RelayPool};
use openagents_kernel_core::authority::{
    AcceptAccessGrantRequest, CreateAccessGrantRequest, IssueDeliveryBundleRequest,
    KernelAuthority, RegisterDataAssetRequest, RevokeAccessGrantRequest,
};
use openagents_kernel_core::data::{
    AccessGrant, AccessGrantStatus, DataAsset, DeliveryBundle, DeliveryBundleStatus,
    NostrPublicationRef, RevocationReceipt,
};
use openagents_kernel_core::ids::sha256_prefixed_text;
use openagents_kernel_core::receipts::{
    Asset, EvidenceRef, Money, MoneyAmount, PolicyContext, ReceiptHints, TraceContext,
};
use openagents_spark::PaymentSummary;
use serde_json::json;

use crate::app_state::{
    AutopilotRole, DataMarketLifecycleEntry, DataMarketPaneState, DataSellerCodexSessionPhase,
    DataSellerDeliveryState, DataSellerIncomingRequest, DataSellerPaymentState,
    DataSellerRevocationAction, DataSellerRevocationState, DataSellerSkillAttachment,
    RelayDatasetAccessContractProjection, RelayDatasetAccessResultProjection,
    RelayDatasetListingProjection, RelayDatasetOfferProjection, RenderState,
};
use crate::codex_lane::CodexLaneCommand;
use crate::provider_nip90_lane::{
    ProviderNip90DataVendingProfile, ProviderNip90LaneCommand, ProviderNip90PublishOutcome,
    ProviderNip90PublishRole,
};
use crate::spark_wallet::{
    SparkWalletCommand, decode_lightning_invoice_payment_hash, is_settled_wallet_payment_status,
    normalize_lightning_invoice_ref,
};

fn current_session_cwd() -> Option<String> {
    std::env::current_dir()
        .ok()
        .and_then(|value| value.into_os_string().into_string().ok())
}

fn current_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().min(i64::MAX as u128) as i64
        })
}

fn current_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn is_kernel_idempotency_conflict(error: &str) -> bool {
    error.contains("kernel_idempotency_conflict")
}

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
        .map_err(|error| format!("Cannot sign Nostr event template: {error}"))
}

fn sync_data_seller_nip90_profile(state: &mut RenderState) {
    let profile =
        state
            .data_seller
            .derived_nip90_profile()
            .map(|profile| ProviderNip90DataVendingProfile {
                profile_id: profile.profile_id,
                request_kind: profile.request_kind,
                result_kind: profile.result_kind,
                kind_posture: profile.kind_posture,
                targeting_posture: profile.targeting_posture,
                asset_families: profile.asset_families,
                delivery_modes: profile.delivery_modes,
                preview_postures: profile.preview_postures,
            });
    let _ = state.queue_provider_nip90_lane_command(
        ProviderNip90LaneCommand::ConfigureDataVendingProfile { profile },
    );
}

fn request_asset_ref_for_event(request: &DataSellerIncomingRequest) -> Result<String, String> {
    request
        .matched_listing_coordinate
        .clone()
        .or_else(|| request.asset_ref.clone())
        .or_else(|| request.matched_asset_id.clone())
        .ok_or_else(|| {
            format!(
                "Request {} is missing both DS listing linkage and asset bridge metadata",
                request.request_id
            )
        })
}

fn request_listing_ref_for_event(
    request: &DataSellerIncomingRequest,
) -> Result<AddressableEventReference, String> {
    let coordinate = request
        .matched_listing_coordinate
        .as_deref()
        .or(request.asset_ref.as_deref())
        .ok_or_else(|| {
            format!(
                "Request {} is missing a DS listing coordinate for seller publication",
                request.request_id
            )
        })?;
    let coordinate = AddressableEventCoordinate::parse(coordinate)
        .map_err(|error| format!("Invalid DS listing coordinate `{coordinate}`: {error}"))?;
    if coordinate.kind != nostr::KIND_DATASET_LISTING {
        return Err(format!(
            "Expected DS listing coordinate kind {}, got {}",
            nostr::KIND_DATASET_LISTING,
            coordinate.kind
        ));
    }
    Ok(AddressableEventReference::new(coordinate))
}

fn request_offer_ref_for_event(
    request: &DataSellerIncomingRequest,
) -> Result<Option<AddressableEventReference>, String> {
    let Some(coordinate) = request.matched_offer_coordinate.as_deref() else {
        return Ok(None);
    };
    let coordinate = AddressableEventCoordinate::parse(coordinate)
        .map_err(|error| format!("Invalid DS offer coordinate `{coordinate}`: {error}"))?;
    if coordinate.kind != nostr::KIND_DATASET_OFFER {
        return Err(format!(
            "Expected DS offer coordinate kind {}, got {}",
            nostr::KIND_DATASET_OFFER,
            coordinate.kind
        ));
    }
    Ok(Some(AddressableEventReference::new(coordinate)))
}

fn build_data_seller_payment_required_feedback_event(
    identity: &NostrIdentity,
    request: &DataSellerIncomingRequest,
    quoted_price_sats: u64,
    bolt11: &str,
) -> Result<Event, String> {
    let mut feedback = DataVendingFeedback::new(
        JobStatus::PaymentRequired,
        request.request_id.as_str(),
        request.requester.as_str(),
        request_asset_ref_for_event(request)?,
    )
    .with_listing_ref(request_listing_ref_for_event(request)?)
    .with_status_extra("lightning settlement required")
    .with_content("Pay the attached Lightning invoice before delivery can proceed.".to_string())
    .with_amount(
        quoted_price_sats.saturating_mul(1000),
        Some(bolt11.to_string()),
    );
    if let Some(offer_ref) = request_offer_ref_for_event(request)? {
        feedback = feedback.with_offer_ref(offer_ref);
    }
    if let Some(asset_id) = request.matched_asset_id.as_deref() {
        feedback = feedback.with_asset_id(asset_id.to_string());
    }
    if let Some(grant_id) = request.matched_grant_id.as_deref() {
        feedback = feedback.with_grant_id(grant_id.to_string());
    }
    let template = create_data_vending_feedback_event(&feedback)
        .map_err(|error| format!("Cannot build DS-DVM payment-required feedback: {error}"))?;
    sign_event_template(identity, &template)
}

fn build_data_seller_revocation_feedback_event(
    identity: &NostrIdentity,
    request: &DataSellerIncomingRequest,
    revocation: &RevocationReceipt,
) -> Result<Event, String> {
    let mut feedback = DataVendingFeedback::new(
        JobStatus::Error,
        request.request_id.as_str(),
        request.requester.as_str(),
        request_asset_ref_for_event(request)?,
    )
    .with_listing_ref(request_listing_ref_for_event(request)?)
    .with_status_extra("offer-revoked")
    .with_content(format!(
        "Access for dataset request {} has been revoked.",
        request.request_id
    ))
    .with_reason_code(revocation.reason_code.clone())
    .with_revocation_id(revocation.revocation_id.clone());
    if let Some(offer_ref) = request_offer_ref_for_event(request)? {
        feedback = feedback.with_offer_ref(offer_ref);
    }
    if let Some(asset_id) = request.matched_asset_id.as_deref() {
        feedback = feedback.with_asset_id(asset_id.to_string());
    }
    if let Some(grant_id) = request.matched_grant_id.as_deref() {
        feedback = feedback.with_grant_id(grant_id.to_string());
    }
    if let Some(delivery_bundle_id) = request.delivery_bundle_id.as_deref() {
        feedback = feedback.with_delivery_bundle_id(delivery_bundle_id.to_string());
    }
    let template = create_data_vending_feedback_event(&feedback)
        .map_err(|error| format!("Cannot build DS-DVM revocation feedback: {error}"))?;
    sign_event_template(identity, &template)
}

fn normalize_relay_urls(relay_urls: &[String]) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    relay_urls
        .iter()
        .map(|relay| relay.trim().trim_end_matches('/').to_string())
        .filter(|relay| !relay.is_empty())
        .filter(|relay| seen.insert(relay.clone()))
        .collect()
}

async fn verify_published_event_on_relays(
    pool: &RelayPool,
    relay_urls: &[String],
    event_id: &str,
    label: &str,
) -> Result<Vec<String>, String> {
    let relay_urls = normalize_relay_urls(relay_urls);
    if relay_urls.is_empty() {
        return Err(format!(
            "Cannot verify {label}: no accepted relay URLs were supplied."
        ));
    }

    let subscription_id = format!(
        "verify-publish-{}-{}",
        label.replace(' ', "-").to_ascii_lowercase(),
        event_id.chars().take(12).collect::<String>()
    );
    pool.subscribe_filters(
        subscription_id.as_str(),
        vec![json!({
            "ids": [event_id],
            "limit": relay_urls.len().max(1),
        })],
    )
    .await
    .map_err(|error| format!("Cannot verify {label}: {error}"))?;

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let poll_step = Duration::from_millis(150);
    let mut verified = std::collections::BTreeSet::new();

    while std::time::Instant::now() < deadline && verified.len() < relay_urls.len() {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        let relays = pool.relays().await;
        for relay in relays {
            let relay_url = relay.url().trim_end_matches('/').to_string();
            if !relay_urls.iter().any(|candidate| candidate == &relay_url) {
                continue;
            }

            let wait = poll_step.min(remaining);
            let recv = tokio::time::timeout(wait, relay.recv()).await;
            let message = match recv {
                Ok(Ok(Some(message))) => message,
                Ok(Ok(None)) | Ok(Err(_)) | Err(_) => continue,
            };

            if let RelayMessage::Event(_, event) = message
                && event.id == event_id
            {
                verified.insert(relay_url);
            }
        }

        if verified.len() < relay_urls.len() {
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    }

    let _ = pool.unsubscribe(subscription_id.as_str()).await;
    let verified = verified.into_iter().collect::<Vec<_>>();
    if verified.is_empty() {
        return Err(format!(
            "Cannot verify {label}: accepted relays did not return event {event_id}."
        ));
    }
    Ok(verified)
}

fn publish_event_to_relays(
    relay_urls: &[String],
    event: &Event,
    label: &str,
) -> Result<Vec<String>, String> {
    let relay_urls = normalize_relay_urls(relay_urls);
    if relay_urls.is_empty() {
        return Err(format!(
            "Cannot publish {label}: no relay URLs configured for seller publication."
        ));
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Cannot start temporary relay runtime: {error}"))?;
    runtime.block_on(async move {
        let pool = RelayPool::new(PoolConfig::default());
        let mut connected_relays = Vec::new();
        let mut connection_errors = Vec::new();
        for relay_url in relay_urls {
            if let Err(error) = pool.add_relay(relay_url.as_str()).await {
                connection_errors.push(format!("{relay_url}: add relay failed: {error}"));
                continue;
            }
            match pool.connect_relay(relay_url.as_str()).await {
                Ok(()) => connected_relays.push(relay_url),
                Err(error) => {
                    connection_errors.push(format!("{relay_url}: connect failed: {error}"))
                }
            }
        }
        if connected_relays.is_empty() {
            let detail = connection_errors.join(" | ");
            return Err(format!("Cannot publish {label}: {detail}"));
        }
        let confirmations = pool
            .publish(event)
            .await
            .map_err(|error| format!("Cannot publish {label}: {error}"))?;
        let accepted_relays = confirmations
            .into_iter()
            .filter(|confirmation| confirmation.accepted)
            .map(|confirmation| confirmation.relay_url)
            .collect::<Vec<_>>();
        if accepted_relays.is_empty() {
            let detail = connection_errors.join(" | ");
            let fallback = if detail.is_empty() {
                "all relays rejected the event".to_string()
            } else {
                detail
            };
            let _ = pool.disconnect_all().await;
            return Err(format!("Cannot publish {label}: {fallback}"));
        }
        let verified_relays =
            match verify_published_event_on_relays(&pool, accepted_relays.as_slice(), &event.id, label)
                .await
            {
                Ok(verified_relays) => verified_relays,
                Err(_) => normalize_relay_urls(&accepted_relays),
            };
        let _ = pool.disconnect_all().await;
        Ok(verified_relays)
    })
}

fn canonical_component(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    trimmed
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn sats_money(amount_sats: u64) -> Money {
    Money {
        asset: Asset::Btc,
        amount: MoneyAmount::AmountSats(amount_sats),
    }
}

fn ds_created_at_seconds(created_at_ms: i64) -> u64 {
    if created_at_ms > 0 {
        (created_at_ms as u64) / 1000
    } else {
        current_epoch_seconds()
    }
}

fn strip_sha256_prefix(value: Option<&str>) -> Result<String, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err("Dataset publication requires a content digest.".to_string());
    };
    let digest = value
        .strip_prefix("sha256:")
        .or_else(|| value.strip_prefix("SHA256:"))
        .unwrap_or(value)
        .trim()
        .to_ascii_lowercase();
    if digest.len() != 64
        || !digest
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(format!(
            "Dataset publication requires a SHA-256 hex digest, got `{value}`."
        ));
    }
    Ok(digest)
}

fn metadata_string(metadata: &serde_json::Value, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn metadata_u64(metadata: &serde_json::Value, key: &str) -> Option<u64> {
    metadata.get(key).and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str()?.parse::<u64>().ok())
    })
}

fn nested_metadata_string(metadata: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = metadata;
    for segment in path {
        current = current.get(*segment)?;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn ensure_metadata_object(
    metadata: &mut serde_json::Value,
) -> &mut serde_json::Map<String, serde_json::Value> {
    if !metadata.is_object() {
        *metadata = json!({});
    }
    metadata.as_object_mut().expect("metadata object")
}

fn metadata_string_array(metadata: &serde_json::Value, key: &str) -> Vec<String> {
    metadata
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn ds_delivery_modes(metadata: &serde_json::Value) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mapped = metadata_string_array(metadata, "delivery_modes")
        .into_iter()
        .map(|mode| match mode.trim().to_ascii_lowercase().as_str() {
            "encrypted_pointer" | "delivery_bundle_ref" | "inline_preview" => "nip90".to_string(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>();
    let mut delivery_modes = mapped
        .into_iter()
        .filter(|mode| seen.insert(mode.clone()))
        .collect::<Vec<_>>();
    if delivery_modes.is_empty() {
        delivery_modes.push("nip90".to_string());
    }
    delivery_modes
}

fn ds_price_from_money(money: Option<&Money>) -> Option<Price> {
    let money = money?;
    let currency = match money.asset {
        Asset::Btc => match money.amount {
            MoneyAmount::AmountSats(amount) => {
                return Some(Price::one_time(amount.to_string(), "SAT"));
            }
            MoneyAmount::AmountMsats(amount) => {
                return Some(Price::one_time(amount.to_string(), "MSAT"));
            }
        },
        Asset::UsdCents => "USD_CENTS",
        Asset::AssetUnspecified => return None,
    };
    let amount = match money.amount {
        MoneyAmount::AmountSats(amount) => amount.to_string(),
        MoneyAmount::AmountMsats(amount) => amount.to_string(),
    };
    Some(Price::one_time(amount, currency))
}

fn storefront_currency_and_price(money: Option<&Money>) -> Option<(String, f64)> {
    let money = money?;
    match money.asset {
        Asset::Btc => match money.amount {
            MoneyAmount::AmountSats(amount) => Some(("SAT".to_string(), amount as f64)),
            MoneyAmount::AmountMsats(amount) => Some(("MSAT".to_string(), amount as f64)),
        },
        Asset::UsdCents => match money.amount {
            MoneyAmount::AmountSats(amount) => Some(("USD_CENTS".to_string(), amount as f64)),
            MoneyAmount::AmountMsats(amount) => Some(("USD_CENTS".to_string(), amount as f64)),
        },
        Asset::AssetUnspecified => None,
    }
}

fn relay_projection_created_at_ms(created_at_seconds: u64) -> i64 {
    i64::try_from(created_at_seconds)
        .unwrap_or(i64::MAX / 1000)
        .saturating_mul(1000)
}

fn relay_projection_asset_id(listing: &RelayDatasetListingProjection) -> String {
    listing
        .linked_asset_id
        .clone()
        .or_else(|| {
            AddressableEventCoordinate::parse(listing.coordinate.as_str())
                .ok()
                .map(|coordinate| coordinate.identifier)
        })
        .unwrap_or_else(|| listing.coordinate.clone())
}

fn relay_projection_grant_id(offer: &RelayDatasetOfferProjection) -> String {
    offer
        .linked_grant_id
        .clone()
        .or_else(|| {
            AddressableEventCoordinate::parse(offer.coordinate.as_str())
                .ok()
                .map(|coordinate| coordinate.identifier)
        })
        .unwrap_or_else(|| offer.coordinate.clone())
}

fn money_from_projection_price(amount: Option<&str>, currency: Option<&str>) -> Option<Money> {
    let amount = amount?.trim().parse::<u64>().ok()?;
    let currency = currency?.trim().to_ascii_uppercase();
    match currency.as_str() {
        "SAT" => Some(Money {
            asset: Asset::Btc,
            amount: MoneyAmount::AmountSats(amount),
        }),
        "MSAT" => Some(Money {
            asset: Asset::Btc,
            amount: MoneyAmount::AmountMsats(amount),
        }),
        "USD_CENTS" => Some(Money {
            asset: Asset::UsdCents,
            amount: MoneyAmount::AmountSats(amount),
        }),
        _ => None,
    }
}

fn relay_visibility_posture(
    classified_coordinate: Option<&str>,
    storefront_coordinate: Option<&str>,
) -> &'static str {
    if classified_coordinate.is_some() || storefront_coordinate.is_some() {
        "public_catalog"
    } else {
        "targeted_only"
    }
}

fn relay_storefront_stall_name(currency: Option<&str>) -> Option<String> {
    currency
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("OpenAgents datasets ({value})"))
}

fn local_dataset_listing_projection(asset: &DataAsset) -> Option<RelayDatasetListingProjection> {
    let publication_ref = asset.nostr_publications.ds_listing.as_ref()?;
    let coordinate = publication_ref.coordinate.clone()?;
    let price = ds_price_from_money(asset.price_hint.as_ref());
    let storefront_currency =
        storefront_currency_and_price(asset.price_hint.as_ref()).map(|(currency, _)| currency);
    Some(RelayDatasetListingProjection {
        coordinate,
        publisher_pubkey: asset.provider_id.clone(),
        relay_url: publication_ref.relay_url.clone(),
        title: asset.title.clone(),
        summary: asset.description.clone(),
        dataset_kind: Some(asset.asset_kind.clone()),
        access: Some(if asset.price_hint.is_some() {
            "paid".to_string()
        } else {
            "targeted".to_string()
        }),
        delivery_modes: ds_delivery_modes(&asset.metadata),
        created_at_seconds: ds_created_at_seconds(asset.created_at_ms),
        draft: false,
        linked_asset_id: Some(asset.asset_id.clone()),
        classified_coordinate: nested_metadata_string(
            &asset.metadata,
            &["nip99_classified", "coordinate"],
        ),
        classified_event_id: nested_metadata_string(
            &asset.metadata,
            &["nip99_classified", "event_id"],
        ),
        classified_price_amount: price.as_ref().map(|value| value.amount.clone()),
        classified_price_currency: price.as_ref().map(|value| value.currency.clone()),
        storefront_stall_coordinate: nested_metadata_string(
            &asset.metadata,
            &["nip15_storefront", "stall", "coordinate"],
        ),
        storefront_stall_name: relay_storefront_stall_name(storefront_currency.as_deref()),
        storefront_product_coordinate: nested_metadata_string(
            &asset.metadata,
            &["nip15_storefront", "product", "coordinate"],
        ),
        storefront_product_event_id: nested_metadata_string(
            &asset.metadata,
            &["nip15_storefront", "product", "event_id"],
        ),
        storefront_product_title: Some(asset.title.clone()),
        storefront_product_price_amount: price.as_ref().map(|value| value.amount.clone()),
        storefront_product_price_currency: price.as_ref().map(|value| value.currency.clone()),
        discussion_channel_id: None,
        discussion_channel_name: None,
        discussion_channel_relay_url: None,
    })
}

fn local_dataset_offer_projection(
    grant: &AccessGrant,
    asset: &DataAsset,
) -> Option<RelayDatasetOfferProjection> {
    let publication_ref = grant.nostr_publications.ds_offer.as_ref()?;
    let coordinate = publication_ref.coordinate.clone()?;
    let listing_coordinate = asset
        .nostr_publications
        .ds_listing
        .as_ref()
        .and_then(|reference| reference.coordinate.clone())
        .or_else(|| {
            AddressableEventCoordinate::dataset_listing(
                grant.provider_id.clone(),
                asset.asset_id.clone(),
            )
            .ok()
            .map(|coordinate| coordinate.to_string())
        })?;
    let price = ds_price_from_money(grant.offer_price.as_ref().or(asset.price_hint.as_ref()));
    let storefront_currency =
        storefront_currency_and_price(grant.offer_price.as_ref().or(asset.price_hint.as_ref()))
            .map(|(currency, _)| currency);
    Some(RelayDatasetOfferProjection {
        coordinate,
        listing_coordinate,
        publisher_pubkey: grant.provider_id.clone(),
        relay_url: publication_ref.relay_url.clone(),
        status: grant_offer_status(grant).as_str().to_string(),
        policy: Some(if grant.consumer_id.is_some() {
            "targeted_request".to_string()
        } else {
            "licensed_bundle".to_string()
        }),
        delivery_modes: ds_delivery_modes(&grant.metadata),
        targeted_buyer_pubkeys: maybe_targeted_buyer(grant.consumer_id.as_deref())
            .into_iter()
            .collect(),
        price_amount: price.as_ref().map(|value| value.amount.clone()),
        price_currency: price.as_ref().map(|value| value.currency.clone()),
        created_at_seconds: ds_created_at_seconds(grant.created_at_ms),
        linked_asset_id: Some(grant.asset_id.clone()),
        linked_grant_id: Some(grant.grant_id.clone()),
        classified_coordinate: nested_metadata_string(
            &grant.metadata,
            &["nip99_classified", "coordinate"],
        ),
        classified_event_id: nested_metadata_string(
            &grant.metadata,
            &["nip99_classified", "event_id"],
        ),
        storefront_stall_coordinate: nested_metadata_string(
            &grant.metadata,
            &["nip15_storefront", "stall", "coordinate"],
        ),
        storefront_stall_name: relay_storefront_stall_name(storefront_currency.as_deref()),
        storefront_product_coordinate: nested_metadata_string(
            &grant.metadata,
            &["nip15_storefront", "product", "coordinate"],
        ),
        storefront_product_event_id: nested_metadata_string(
            &grant.metadata,
            &["nip15_storefront", "product", "event_id"],
        ),
        storefront_product_title: Some(format!("{} access", asset.title)),
        storefront_product_price_amount: price.as_ref().map(|value| value.amount.clone()),
        storefront_product_price_currency: price.as_ref().map(|value| value.currency.clone()),
        discussion_channel_id: None,
        discussion_channel_name: None,
        discussion_channel_relay_url: None,
    })
}

fn upsert_relay_listing_projection(
    relay_listings: &mut Vec<RelayDatasetListingProjection>,
    projection: RelayDatasetListingProjection,
) {
    if let Some(existing) = relay_listings.iter_mut().find(|existing| {
        existing
            .coordinate
            .eq_ignore_ascii_case(projection.coordinate.as_str())
    }) {
        *existing = projection;
    } else {
        relay_listings.push(projection);
    }
}

fn upsert_relay_offer_projection(
    relay_offers: &mut Vec<RelayDatasetOfferProjection>,
    projection: RelayDatasetOfferProjection,
) {
    if let Some(existing) = relay_offers.iter_mut().find(|existing| {
        existing
            .coordinate
            .eq_ignore_ascii_case(projection.coordinate.as_str())
    }) {
        *existing = projection;
    } else {
        relay_offers.push(projection);
    }
}

fn upsert_relay_access_contract_projection(
    relay_access_contracts: &mut Vec<RelayDatasetAccessContractProjection>,
    projection: RelayDatasetAccessContractProjection,
) {
    if let Some(existing) = relay_access_contracts.iter_mut().find(|existing| {
        existing
            .coordinate
            .eq_ignore_ascii_case(projection.coordinate.as_str())
    }) {
        *existing = projection;
    } else {
        relay_access_contracts.push(projection);
    }
}

fn upsert_relay_result_projection(
    relay_results: &mut Vec<RelayDatasetAccessResultProjection>,
    projection: RelayDatasetAccessResultProjection,
) {
    if let Some(existing) = relay_results.iter_mut().find(|existing| {
        existing
            .event_id
            .eq_ignore_ascii_case(projection.event_id.as_str())
    }) {
        *existing = projection;
    } else {
        relay_results.push(projection);
    }
}

fn request_contract_identifier(request: &DataSellerIncomingRequest) -> String {
    request.request_id.clone()
}

fn request_buyer_pubkey_hex(request: &DataSellerIncomingRequest) -> Result<String, String> {
    let normalized =
        crate::nip90_compute_semantics::normalize_pubkey(request.effective_consumer_id());
    if normalized.len() == 64
        && normalized
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        Ok(normalized.to_ascii_lowercase())
    } else {
        Err(format!(
            "Request {} is missing a valid buyer pubkey for DS access contract publication.",
            request.request_id
        ))
    }
}

fn request_delivery_expiry_epoch_seconds(
    request: &DataSellerIncomingRequest,
    created_at_ms: i64,
) -> Option<u64> {
    request
        .delivery_draft
        .expires_in_hours
        .map(|hours| {
            created_at_ms.saturating_add(
                i64::try_from(hours)
                    .unwrap_or(i64::MAX)
                    .saturating_mul(60)
                    .saturating_mul(60)
                    .saturating_mul(1000),
            )
        })
        .and_then(|expires_at_ms| u64::try_from(expires_at_ms.max(0) / 1000).ok())
        .or(request.expires_at_epoch_seconds)
}

fn optional_contract_delivery_digest(value: Option<&str>) -> Option<String> {
    strip_sha256_prefix(value).ok()
}

fn data_seller_access_contract_content(
    status: &DatasetAccessContractStatus,
    request: &DataSellerIncomingRequest,
) -> String {
    match status {
        DatasetAccessContractStatus::PaymentRequired => format!(
            "Lightning settlement required before dataset request {} can proceed.",
            request.request_id
        ),
        DatasetAccessContractStatus::Paid => format!(
            "Lightning settlement observed for dataset request {}.",
            request.request_id
        ),
        DatasetAccessContractStatus::Delivered => format!(
            "Dataset request {} has been fulfilled and delivery is available.",
            request.request_id
        ),
        DatasetAccessContractStatus::Revoked => format!(
            "Dataset request {} has been revoked by the seller.",
            request.request_id
        ),
        DatasetAccessContractStatus::Expired => format!(
            "Dataset request {} expired after its delivery window elapsed.",
            request.request_id
        ),
        DatasetAccessContractStatus::Refunded => format!(
            "Dataset request {} was refunded after revocation.",
            request.request_id
        ),
    }
}

fn build_data_seller_access_contract(
    request: &DataSellerIncomingRequest,
    seller_pubkey: &str,
    status: DatasetAccessContractStatus,
    result_event_id: Option<&str>,
    reason_code: Option<&str>,
) -> Result<DatasetAccessContract, String> {
    let listing_ref = request_listing_ref_for_event(request)?;
    let buyer = PublicKeyReference::new(request_buyer_pubkey_hex(request)?)
        .map_err(|error| format!("Cannot encode DS contract buyer ref: {error}"))?;
    let request_ref = EventReference::new(request.request_id.clone())
        .map_err(|error| format!("Cannot encode DS contract request ref: {error}"))?;
    let mut contract = DatasetAccessContract::new(
        request_contract_identifier(request),
        data_seller_access_contract_content(&status, request),
        listing_ref,
        request_ref,
        buyer,
    )
    .with_status(status.clone());
    if let Some(offer_ref) = request_offer_ref_for_event(request)? {
        contract = contract.with_offer_ref(offer_ref);
    }
    let amount_msats = request
        .payment_amount_sats
        .or(request.required_price_sats)
        .or((request.price_sats > 0).then_some(request.price_sats))
        .map(|amount_sats| amount_sats.saturating_mul(1000));
    if let Some(amount_msats) = amount_msats {
        contract = contract
            .with_payment_method(PaymentMethod::new("ln"))
            .with_amount_msats(amount_msats, request.pending_bolt11.clone());
    }
    let delivery_mode = delivery_mode_for_request(request);
    if !delivery_mode.as_str().trim().is_empty() {
        contract = contract.with_delivery_mode(delivery_mode.as_str().to_string());
    }
    if let Some(delivery_ref) = request
        .delivery_draft
        .delivery_ref
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        contract = contract.with_delivery_ref(delivery_ref.to_string());
    }
    if let Some(delivery_digest) =
        optional_contract_delivery_digest(request.delivery_draft.delivery_digest.as_deref())
    {
        contract = contract.with_delivery_digest(delivery_digest);
    }
    if matches!(
        &status,
        DatasetAccessContractStatus::Delivered
            | DatasetAccessContractStatus::Revoked
            | DatasetAccessContractStatus::Expired
    ) && let Some(result_event_id) = result_event_id
    {
        contract = contract.with_result_ref(
            EventReference::new(result_event_id.to_string())
                .map_err(|error| format!("Cannot encode DS contract result ref: {error}"))?,
        );
    }
    let expires_at = request_delivery_expiry_epoch_seconds(
        request,
        request
            .payment_observed_at_epoch_seconds
            .and_then(|epoch_seconds| i64::try_from(epoch_seconds).ok())
            .unwrap_or_else(|| i64::try_from(current_epoch_seconds()).unwrap_or(i64::MAX))
            .saturating_mul(1000),
    );
    if let Some(expires_at) = expires_at {
        contract = contract.with_expires_at(expires_at);
    }
    if matches!(
        &status,
        DatasetAccessContractStatus::Revoked | DatasetAccessContractStatus::Expired
    ) {
        let reason_code = reason_code
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or(request.revocation_reason_code.as_deref())
            .unwrap_or(match &status {
                DatasetAccessContractStatus::Revoked => "seller_revoked_access",
                DatasetAccessContractStatus::Expired => "access_window_expired",
                _ => "unspecified",
            });
        contract = contract.with_reason_code(reason_code.to_string());
    }
    contract
        .coordinate(seller_pubkey.to_string())
        .map_err(|error| format!("Cannot derive DS access contract coordinate: {error}"))?;
    Ok(contract)
}

fn local_dataset_access_contract_projection(
    seller_pubkey: &str,
    relay_url: Option<String>,
    contract: &DatasetAccessContract,
    request: &DataSellerIncomingRequest,
    created_at_seconds: u64,
) -> Result<RelayDatasetAccessContractProjection, String> {
    let coordinate = contract
        .coordinate(seller_pubkey.to_string())
        .map_err(|error| format!("Cannot derive DS access contract coordinate: {error}"))?
        .to_string();
    Ok(RelayDatasetAccessContractProjection {
        coordinate,
        seller_pubkey: seller_pubkey.to_string(),
        buyer_pubkey: contract.buyer.pubkey.clone(),
        relay_url,
        listing_coordinate: contract.listing_ref.coordinate.to_string(),
        offer_coordinate: contract
            .offer_ref
            .as_ref()
            .map(|reference| reference.coordinate.to_string()),
        request_event_id: request.request_id.clone(),
        result_event_id: contract
            .result_ref
            .as_ref()
            .map(|reference| reference.event_id.clone()),
        status: contract.status.as_str().to_string(),
        payment_method: contract
            .payment_method
            .as_ref()
            .map(|payment_method| payment_method.rail.clone()),
        amount_msats: contract.amount_msats,
        bolt11: contract.bolt11.clone(),
        payment_hash: contract
            .bolt11
            .as_deref()
            .and_then(decode_lightning_invoice_payment_hash)
            .or_else(|| request.settlement_payment_hash.clone()),
        payment_evidence_event_ids: contract
            .payment_evidence_refs
            .iter()
            .map(|reference| reference.event_id.clone())
            .collect(),
        delivery_mode: contract.delivery_mode.clone(),
        delivery_ref: contract.delivery_ref.clone(),
        delivery_mime_type: contract.delivery_mime_type.clone(),
        delivery_digest: contract.delivery_digest.clone(),
        created_at_seconds,
        expires_at_seconds: contract.expires_at,
        reason_code: contract.reason_code.clone(),
        linked_asset_id: request.matched_asset_id.clone(),
        linked_grant_id: request.matched_grant_id.clone(),
    })
}

fn local_dataset_access_result_projection(
    seller_pubkey: &str,
    relay_url: Option<String>,
    request: &DataSellerIncomingRequest,
    event_id: &str,
    created_at_seconds: u64,
) -> Result<RelayDatasetAccessResultProjection, String> {
    Ok(RelayDatasetAccessResultProjection {
        event_id: event_id.to_string(),
        seller_pubkey: seller_pubkey.to_string(),
        buyer_pubkey: request_buyer_pubkey_hex(request)?,
        relay_url,
        request_event_id: request.request_id.clone(),
        listing_coordinate: request_listing_ref_for_event(request)?.coordinate.to_string(),
        offer_coordinate: request_offer_ref_for_event(request)?
            .map(|reference| reference.coordinate.to_string()),
        asset_ref: request_asset_ref_for_event(request)?,
        asset_id: request.matched_asset_id.clone(),
        grant_id: request.matched_grant_id.clone(),
        delivery_bundle_id: request.delivery_bundle_id.clone().unwrap_or_else(|| {
            format!(
                "delivery_bundle.{}",
                canonical_component(request.request_id.as_str())
            )
        }),
        delivery_mode: delivery_mode_for_request(request).as_str().to_string(),
        preview_posture: preview_posture_for_request(request).as_str().to_string(),
        delivery_ref: request.delivery_draft.delivery_ref.clone(),
        delivery_digest: request.delivery_draft.delivery_digest.clone(),
        amount_msats: request
            .payment_amount_sats
            .or(request.required_price_sats)
            .or((request.price_sats > 0).then_some(request.price_sats))
            .map(|amount_sats| amount_sats.saturating_mul(1000)),
        bolt11: request.pending_bolt11.clone(),
        payment_hash: request
            .settlement_payment_hash
            .clone()
            .or_else(|| {
                request
                    .pending_bolt11
                    .as_deref()
                    .and_then(decode_lightning_invoice_payment_hash)
            }),
        created_at_seconds,
        linked_asset_id: request.matched_asset_id.clone(),
        linked_grant_id: request.matched_grant_id.clone(),
    })
}

fn record_local_access_contract_projection(
    state: &mut RenderState,
    projection: RelayDatasetAccessContractProjection,
) -> Result<(), String> {
    let mut relay_access_contracts = state.data_market.relay_access_contracts.clone();
    upsert_relay_access_contract_projection(&mut relay_access_contracts, projection);
    state.data_market.apply_relay_catalog(
        state.data_market.relay_listings.clone(),
        state.data_market.relay_offers.clone(),
        state.data_market.relay_requests.clone(),
        relay_access_contracts,
        state.data_market.relay_results.clone(),
        state.data_market.relay_settlement_matches.clone(),
        current_epoch_ms(),
    );
    crate::data_market_control::persist_data_market_relay_replica_from_state(state)?;
    state.data_buyer.sync_selection(&state.data_market);
    Ok(())
}

fn record_local_access_result_projection(
    state: &mut RenderState,
    projection: RelayDatasetAccessResultProjection,
) -> Result<(), String> {
    let mut relay_results = state.data_market.relay_results.clone();
    upsert_relay_result_projection(&mut relay_results, projection);
    state.data_market.apply_relay_catalog(
        state.data_market.relay_listings.clone(),
        state.data_market.relay_offers.clone(),
        state.data_market.relay_requests.clone(),
        state.data_market.relay_access_contracts.clone(),
        relay_results,
        state.data_market.relay_settlement_matches.clone(),
        current_epoch_ms(),
    );
    crate::data_market_control::persist_data_market_relay_replica_from_state(state)?;
    state.data_buyer.sync_selection(&state.data_market);
    Ok(())
}

fn publish_data_seller_access_contract(
    state: &mut RenderState,
    request_id: &str,
    status: DatasetAccessContractStatus,
    result_event_id: Option<&str>,
    reason_code: Option<&str>,
) -> Result<String, String> {
    let request = state
        .data_seller
        .request_by_id(request_id)
        .cloned()
        .ok_or_else(|| format!("Unknown data-access request {request_id}"))?;
    let identity = state
        .nostr_identity
        .clone()
        .ok_or_else(|| "Cannot publish DS access contract: Nostr identity unavailable.".to_string())?;
    let contract = build_data_seller_access_contract(
        &request,
        identity.public_key_hex.as_str(),
        status,
        result_event_id,
        reason_code,
    )?;
    let template = contract
        .to_event_template(current_epoch_seconds())
        .map_err(|error| format!("Cannot build DS access contract event: {error}"))?;
    let event = sign_event_template(&identity, &template)?;
    let accepted_relays = publish_event_to_relays(
        state.configured_provider_relay_urls().as_slice(),
        &event,
        "DS access contract",
    )?;
    let projection = local_dataset_access_contract_projection(
        identity.public_key_hex.as_str(),
        accepted_relays.first().cloned(),
        &contract,
        &request,
        event.created_at,
    )?;
    record_local_access_contract_projection(state, projection)?;
    Ok(event.id)
}

fn relay_only_delivery_bundle_for_request(
    request: &DataSellerIncomingRequest,
    provider_id: &str,
    consumer_id: &str,
    created_at_ms: i64,
) -> Result<DeliveryBundle, String> {
    let grant_id = request
        .matched_grant_id
        .as_deref()
        .ok_or_else(|| format!("Request {} is missing a matched grant.", request.request_id))?;
    let asset_id = request
        .matched_asset_id
        .as_deref()
        .ok_or_else(|| format!("Request {} is missing a matched asset.", request.request_id))?;
    Ok(
        build_issue_delivery_bundle_request(
            request,
            grant_id,
            asset_id,
            provider_id,
            consumer_id,
            None,
            created_at_ms,
        )
        .delivery_bundle,
    )
}

fn relay_only_revocation_receipt_for_request(
    request: &DataSellerIncomingRequest,
    provider_id: &str,
    consumer_id: Option<&str>,
    action: DataSellerRevocationAction,
    reason_code: Option<&str>,
    created_at_ms: i64,
) -> Result<RevocationReceipt, String> {
    let grant_id = request
        .matched_grant_id
        .as_deref()
        .ok_or_else(|| format!("Request {} is missing a matched grant.", request.request_id))?;
    let asset_id = request
        .matched_asset_id
        .as_deref()
        .ok_or_else(|| format!("Request {} is missing a matched asset.", request.request_id))?;
    let revoked_delivery_bundle_ids = request
        .delivery_bundle_id
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    Ok(
        build_revoke_access_grant_request(
            request,
            grant_id,
            asset_id,
            provider_id,
            consumer_id,
            revoked_delivery_bundle_ids,
            action,
            reason_code,
            None,
            created_at_ms,
        )
        .revocation,
    )
}

fn reconcile_request_from_relay_catalog(
    request: &DataSellerIncomingRequest,
    market: &DataMarketPaneState,
) -> DataSellerIncomingRequest {
    let mut reconciled = request.clone();
    let contract = market
        .relay_access_contract_for_request(request.request_id.as_str())
        .cloned();
    let result = market
        .relay_delivery_lookup_for_request(request.request_id.as_str())
        .cloned();
    let settlement = market
        .relay_settlement_matches_for_request(request.request_id.as_str())
        .into_iter()
        .max_by_key(|entry| (entry.observed_at_seconds, entry.payment_pointer.as_str()))
        .cloned();

    if let Some(contract) = contract.as_ref() {
        if let Some(bolt11) = contract
            .bolt11
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            reconciled.pending_bolt11 = Some(bolt11.to_string());
            reconciled.pending_bolt11_created_at_epoch_seconds =
                Some(contract.created_at_seconds);
            reconciled.settlement_payment_hash = contract
                .payment_hash
                .clone()
                .or_else(|| decode_lightning_invoice_payment_hash(bolt11));
        }
        if let Some(amount_msats) = contract.amount_msats {
            reconciled.payment_amount_sats = Some(amount_msats / 1000);
        }
        if let Some(delivery_mode) = contract
            .delivery_mode
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            reconciled.delivery_mode = Some(delivery_mode.to_string());
        }
        if let Some(delivery_ref) = contract
            .delivery_ref
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            reconciled.delivery_draft.delivery_ref = Some(delivery_ref.to_string());
        }
        if let Some(delivery_digest) = contract
            .delivery_digest
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            reconciled.delivery_draft.delivery_digest = Some(delivery_digest.to_string());
        }
        if let Some(result_event_id) = contract.result_event_id.as_deref() {
            reconciled.delivery_result_event_id = Some(result_event_id.to_string());
        }

        match contract.status.to_ascii_lowercase().as_str() {
            "payment-required" => {
                if reconciled.payment_state != DataSellerPaymentState::Paid {
                    reconciled.payment_state = DataSellerPaymentState::AwaitingPayment;
                }
            }
            "paid" | "delivered" | "revoked" | "expired" | "refunded" => {
                reconciled.payment_state = DataSellerPaymentState::Paid;
            }
            _ => {}
        }

        match contract.status.to_ascii_lowercase().as_str() {
            "delivered" => {
                reconciled.delivery_state = DataSellerDeliveryState::Delivered;
            }
            "revoked" => {
                reconciled.delivery_state = DataSellerDeliveryState::Revoked;
                reconciled.revocation_state = DataSellerRevocationState::Revoked;
                reconciled.revocation_reason_code = contract.reason_code.clone();
                reconciled.revocation_id = Some(contract.coordinate.clone());
                reconciled.revocation_recorded_at_ms =
                    Some(relay_projection_created_at_ms(contract.created_at_seconds));
            }
            "expired" => {
                reconciled.delivery_state = DataSellerDeliveryState::Expired;
                reconciled.revocation_state = DataSellerRevocationState::Expired;
                reconciled.revocation_reason_code = contract.reason_code.clone();
                reconciled.revocation_id = Some(contract.coordinate.clone());
                reconciled.revocation_recorded_at_ms =
                    Some(relay_projection_created_at_ms(contract.created_at_seconds));
            }
            _ => {}
        }
    }

    if let Some(settlement) = settlement.as_ref() {
        reconciled.payment_pointer = Some(settlement.payment_pointer.clone());
        reconciled.payment_observed_at_epoch_seconds = Some(settlement.observed_at_seconds);
        reconciled.payment_amount_sats = Some(settlement.amount_sats);
        reconciled.settlement_payment_hash = Some(settlement.payment_hash.clone());
        if matches!(
            reconciled.payment_state,
            DataSellerPaymentState::AwaitingPayment | DataSellerPaymentState::Idle
        ) {
            reconciled.payment_state = DataSellerPaymentState::Paid;
        }
    }

    if let Some(result) = result.as_ref() {
        reconciled.delivery_bundle_id = Some(result.delivery_bundle_id.clone());
        reconciled.delivery_result_event_id = Some(result.event_id.clone());
        if let Some(delivery_ref) = result
            .delivery_ref
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            reconciled.delivery_draft.delivery_ref = Some(delivery_ref.to_string());
        }
        if let Some(delivery_digest) = result
            .delivery_digest
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            reconciled.delivery_draft.delivery_digest = Some(delivery_digest.to_string());
        }
        reconciled.delivery_state = match reconciled.revocation_state {
            DataSellerRevocationState::Revoked => DataSellerDeliveryState::Revoked,
            DataSellerRevocationState::Expired => DataSellerDeliveryState::Expired,
            _ => DataSellerDeliveryState::Delivered,
        };
    }

    reconciled
}

pub(crate) fn reconcile_data_seller_request_from_relay_catalog(
    state: &mut RenderState,
    request_id: &str,
) -> bool {
    crate::data_market_control::hydrate_data_market_relay_replica(state);
    let Some(existing) = state.data_seller.request_by_id(request_id).cloned() else {
        return false;
    };
    let reconciled = reconcile_request_from_relay_catalog(&existing, &state.data_market);
    if reconciled == existing {
        return false;
    }
    if let Some(request) = state.data_seller.request_by_id_mut(request_id) {
        *request = reconciled;
        return true;
    }
    false
}

pub(crate) fn reconcile_all_data_seller_requests_from_relay_catalog(
    state: &mut RenderState,
) -> usize {
    let request_ids = state
        .data_seller
        .incoming_requests
        .iter()
        .map(|request| request.request_id.clone())
        .collect::<Vec<_>>();
    request_ids
        .into_iter()
        .filter(|request_id| reconcile_data_seller_request_from_relay_catalog(state, request_id))
        .count()
}

fn record_local_published_asset_projection(
    state: &mut RenderState,
    asset: &DataAsset,
) -> Result<(), String> {
    let Some(projection) = local_dataset_listing_projection(asset) else {
        return Ok(());
    };
    let mut relay_listings = state.data_market.relay_listings.clone();
    upsert_relay_listing_projection(&mut relay_listings, projection);
    state.data_market.apply_relay_catalog(
        relay_listings,
        state.data_market.relay_offers.clone(),
        state.data_market.relay_requests.clone(),
        state.data_market.relay_access_contracts.clone(),
        state.data_market.relay_results.clone(),
        state.data_market.relay_settlement_matches.clone(),
        current_epoch_ms(),
    );
    crate::data_market_control::persist_data_market_relay_replica_from_state(state)?;
    state.data_buyer.sync_selection(&state.data_market);
    Ok(())
}

fn record_local_published_grant_projection(
    state: &mut RenderState,
    asset: &DataAsset,
    grant: &AccessGrant,
) -> Result<(), String> {
    let mut relay_listings = state.data_market.relay_listings.clone();
    if let Some(listing) = local_dataset_listing_projection(asset) {
        upsert_relay_listing_projection(&mut relay_listings, listing);
    }
    let mut relay_offers = state.data_market.relay_offers.clone();
    if let Some(offer) = local_dataset_offer_projection(grant, asset) {
        upsert_relay_offer_projection(&mut relay_offers, offer);
    }
    state.data_market.apply_relay_catalog(
        relay_listings,
        relay_offers,
        state.data_market.relay_requests.clone(),
        state.data_market.relay_access_contracts.clone(),
        state.data_market.relay_results.clone(),
        state.data_market.relay_settlement_matches.clone(),
        current_epoch_ms(),
    );
    crate::data_market_control::persist_data_market_relay_replica_from_state(state)?;
    state.data_buyer.sync_selection(&state.data_market);
    Ok(())
}

fn relay_projected_asset_from_listing(listing: &RelayDatasetListingProjection) -> DataAsset {
    let mut metadata = json!({
        "delivery_modes": listing.delivery_modes,
        "visibility_posture": relay_visibility_posture(
            listing.classified_coordinate.as_deref(),
            listing.storefront_product_coordinate.as_deref(),
        ),
    });
    if let Some(classified_coordinate) = listing.classified_coordinate.as_deref() {
        record_nip99_classified_metadata(
            &mut metadata,
            &NostrPublicationRef {
                coordinate: Some(classified_coordinate.to_string()),
                event_id: listing.classified_event_id.clone(),
                relay_url: listing.relay_url.clone(),
            },
        );
    }
    if let (Some(stall_coordinate), Some(product_coordinate)) = (
        listing.storefront_stall_coordinate.as_deref(),
        listing.storefront_product_coordinate.as_deref(),
    ) {
        record_nip15_storefront_metadata(
            &mut metadata,
            &NostrPublicationRef {
                coordinate: Some(stall_coordinate.to_string()),
                event_id: None,
                relay_url: listing.relay_url.clone(),
            },
            &NostrPublicationRef {
                coordinate: Some(product_coordinate.to_string()),
                event_id: listing.storefront_product_event_id.clone(),
                relay_url: listing.relay_url.clone(),
            },
        );
    }
    DataAsset {
        asset_id: relay_projection_asset_id(listing),
        provider_id: listing.publisher_pubkey.clone(),
        asset_kind: listing
            .dataset_kind
            .clone()
            .unwrap_or_else(|| "dataset".to_string()),
        title: listing.title.clone(),
        description: listing.summary.clone(),
        content_digest: None,
        provenance_ref: None,
        default_policy: None,
        price_hint: None,
        created_at_ms: relay_projection_created_at_ms(listing.created_at_seconds),
        status: openagents_kernel_core::data::DataAssetStatus::Active,
        nostr_publications: openagents_kernel_core::data::DataAssetNostrPublications {
            ds_listing: Some(NostrPublicationRef {
                coordinate: Some(listing.coordinate.clone()),
                event_id: None,
                relay_url: listing.relay_url.clone(),
            }),
            ds_draft_listing: None,
        },
        metadata,
    }
}

fn relay_projected_grant_from_offer(offer: &RelayDatasetOfferProjection) -> AccessGrant {
    let mut metadata = json!({
        "delivery_modes": offer.delivery_modes,
        "visibility_posture": relay_visibility_posture(
            offer.classified_coordinate.as_deref(),
            offer.storefront_product_coordinate.as_deref(),
        ),
    });
    if let Some(classified_coordinate) = offer.classified_coordinate.as_deref() {
        record_nip99_classified_metadata(
            &mut metadata,
            &NostrPublicationRef {
                coordinate: Some(classified_coordinate.to_string()),
                event_id: offer.classified_event_id.clone(),
                relay_url: offer.relay_url.clone(),
            },
        );
    }
    if let (Some(stall_coordinate), Some(product_coordinate)) = (
        offer.storefront_stall_coordinate.as_deref(),
        offer.storefront_product_coordinate.as_deref(),
    ) {
        record_nip15_storefront_metadata(
            &mut metadata,
            &NostrPublicationRef {
                coordinate: Some(stall_coordinate.to_string()),
                event_id: None,
                relay_url: offer.relay_url.clone(),
            },
            &NostrPublicationRef {
                coordinate: Some(product_coordinate.to_string()),
                event_id: offer.storefront_product_event_id.clone(),
                relay_url: offer.relay_url.clone(),
            },
        );
    }
    let created_at_ms = relay_projection_created_at_ms(offer.created_at_seconds);
    AccessGrant {
        grant_id: relay_projection_grant_id(offer),
        asset_id: offer
            .linked_asset_id
            .clone()
            .unwrap_or_else(|| offer.listing_coordinate.clone()),
        provider_id: offer.publisher_pubkey.clone(),
        consumer_id: (offer.targeted_buyer_pubkeys.len() == 1)
            .then(|| offer.targeted_buyer_pubkeys[0].clone()),
        permission_policy: openagents_kernel_core::data::PermissionPolicy {
            policy_id: offer
                .policy
                .clone()
                .unwrap_or_else(|| "targeted_request".to_string()),
            allowed_scopes: offer.delivery_modes.clone(),
            ..Default::default()
        },
        offer_price: money_from_projection_price(
            offer.price_amount.as_deref(),
            offer.price_currency.as_deref(),
        ),
        warranty_window_ms: None,
        created_at_ms,
        expires_at_ms: created_at_ms.saturating_add(30 * 24 * 60 * 60 * 1000),
        accepted_at_ms: None,
        status: match offer.status.to_ascii_lowercase().as_str() {
            "revoked" => AccessGrantStatus::Revoked,
            "expired" => AccessGrantStatus::Expired,
            _ => AccessGrantStatus::Offered,
        },
        nostr_publications: openagents_kernel_core::data::AccessGrantNostrPublications {
            ds_offer: Some(NostrPublicationRef {
                coordinate: Some(offer.coordinate.clone()),
                event_id: None,
                relay_url: offer.relay_url.clone(),
            }),
            ds_access_request: None,
            ds_access_result: None,
        },
        metadata,
    }
}

pub(crate) fn hydrate_data_seller_inventory_from_relay_replica(state: &mut RenderState) -> bool {
    if state.data_seller.last_published_asset.is_some()
        || state.data_seller.last_published_grant.is_some()
        || !state.data_seller.published_assets.is_empty()
        || !state.data_seller.published_grants.is_empty()
    {
        return false;
    }
    crate::data_market_control::hydrate_data_market_relay_replica(state);
    let Some(identity) = state.nostr_identity.as_ref() else {
        return false;
    };
    let listings = state
        .data_market
        .relay_authored_listings_for_publisher(identity.public_key_hex.as_str());
    let offers = state
        .data_market
        .relay_authored_offers_for_publisher(identity.public_key_hex.as_str());
    if listings.is_empty() && offers.is_empty() {
        return false;
    }

    let mut asset_ids_by_listing_coordinate = BTreeMap::<String, String>::new();
    for listing in listings {
        let asset = relay_projected_asset_from_listing(listing);
        asset_ids_by_listing_coordinate.insert(listing.coordinate.clone(), asset.asset_id.clone());
        state.data_seller.sync_relay_projected_asset(asset);
    }
    for offer in offers {
        let mut grant = relay_projected_grant_from_offer(offer);
        if grant.asset_id == offer.listing_coordinate
            && let Some(asset_id) =
                asset_ids_by_listing_coordinate.get(offer.listing_coordinate.as_str())
        {
            grant.asset_id = asset_id.clone();
        }
        state.data_seller.sync_relay_projected_grant(grant);
    }
    true
}

fn storefront_stall_identifier(currency: &str) -> String {
    format!("datasets.{}", currency.trim().to_ascii_lowercase())
}

fn asset_public_catalog_visibility(asset: &DataAsset) -> bool {
    metadata_string(&asset.metadata, "visibility_posture").as_deref() == Some("public_catalog")
}

fn grant_public_catalog_visibility(grant: &AccessGrant) -> bool {
    metadata_string(&grant.metadata, "visibility_posture").as_deref() == Some("public_catalog")
}

fn record_nip99_classified_metadata(
    metadata: &mut serde_json::Value,
    publication_ref: &NostrPublicationRef,
) {
    ensure_metadata_object(metadata).insert(
        "nip99_classified".to_string(),
        json!({
            "coordinate": publication_ref.coordinate,
            "event_id": publication_ref.event_id,
            "relay_url": publication_ref.relay_url,
        }),
    );
}

fn record_nip15_storefront_metadata(
    metadata: &mut serde_json::Value,
    stall_ref: &NostrPublicationRef,
    product_ref: &NostrPublicationRef,
) {
    ensure_metadata_object(metadata).insert(
        "nip15_storefront".to_string(),
        json!({
            "stall": {
                "coordinate": stall_ref.coordinate,
                "event_id": stall_ref.event_id,
                "relay_url": stall_ref.relay_url,
            },
            "product": {
                "coordinate": product_ref.coordinate,
                "event_id": product_ref.event_id,
                "relay_url": product_ref.relay_url,
            },
        }),
    );
}

fn maybe_targeted_buyer(consumer_id: Option<&str>) -> Option<String> {
    let value = consumer_id?.trim();
    if value.is_empty() {
        return None;
    }
    let normalized = crate::nip90_compute_semantics::normalize_pubkey(value);
    if normalized.len() == 64
        && normalized
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        Some(normalized)
    } else {
        None
    }
}

fn build_dataset_listing(
    asset: &DataAsset,
    seller_pubkey: &str,
) -> Result<(DatasetListing, NostrPublicationRef), String> {
    let digest = strip_sha256_prefix(asset.content_digest.as_deref())?;
    let mut listing = DatasetListing::new(
        asset.asset_id.clone(),
        asset.description.clone().unwrap_or_else(|| {
            format!(
                "Dataset listing for {} ({})",
                asset.title.trim(),
                asset.asset_kind.trim()
            )
        }),
        asset.title.clone(),
        digest,
    )
    .with_published_at(ds_created_at_seconds(asset.created_at_ms))
    .with_dataset_kind(asset.asset_kind.clone());

    if let Some(summary) = asset.description.as_deref() {
        listing = listing.with_summary(summary.to_string());
    }
    if let Some(size_bytes) = metadata_u64(&asset.metadata, "packaging_total_bytes") {
        listing = listing.with_size_bytes(size_bytes);
    }
    if let Some(records) = metadata_u64(&asset.metadata, "packaging_file_count") {
        listing = listing.with_records(records);
    }
    if let Some(license) = asset
        .default_policy
        .as_ref()
        .map(|policy| policy.policy_id.clone())
        .filter(|value| !value.trim().is_empty())
    {
        listing = listing.with_license(license);
    }
    if let Some(access) = asset
        .price_hint
        .as_ref()
        .map(|_| "paid".to_string())
        .or_else(|| Some("targeted".to_string()))
    {
        listing = listing.with_access(access);
    }
    for delivery_mode in ds_delivery_modes(&asset.metadata) {
        listing = listing.add_delivery_mode(delivery_mode);
    }
    listing = listing
        .add_topic("dataset")
        .add_topic(asset.asset_kind.clone());
    let coordinate = listing
        .coordinate(seller_pubkey.to_string())
        .map_err(|error| format!("Cannot derive DS listing coordinate: {error}"))?;
    Ok((
        listing,
        NostrPublicationRef {
            coordinate: Some(coordinate.to_string()),
            event_id: None,
            relay_url: None,
        },
    ))
}

fn build_dataset_classified_listing(
    asset: &DataAsset,
    seller_pubkey: &str,
) -> Result<(ClassifiedListing, NostrPublicationRef), String> {
    let listing_coordinate = asset
        .nostr_publications
        .ds_listing
        .as_ref()
        .and_then(|reference| reference.coordinate.clone())
        .or_else(|| {
            AddressableEventCoordinate::dataset_listing(
                seller_pubkey.to_string(),
                asset.asset_id.clone(),
            )
            .ok()
            .map(|coordinate| coordinate.to_string())
        })
        .ok_or_else(|| "Cannot derive DS listing coordinate for NIP-99 wrapper.".to_string())?;
    let mut listing = ClassifiedListing::new(
        format!("catalog.{}", asset.asset_id),
        asset.description.clone().unwrap_or_else(|| {
            format!(
                "Public catalog wrapper for dataset {} ({})",
                asset.title, asset.asset_kind
            )
        }),
        asset.title.clone(),
    )
    .with_published_at(ds_created_at_seconds(asset.created_at_ms))
    .with_status(ListingStatus::Active);
    if let Some(summary) = asset.description.as_deref() {
        listing = listing.with_summary(summary.to_string());
    }
    if let Some(price) = ds_price_from_money(asset.price_hint.as_ref()) {
        listing = listing.with_price(price);
    }
    listing.add_tag("dataset");
    listing.add_tag("nip-ds");
    listing.add_tag(asset.asset_kind.clone());
    listing.add_address_ref(listing_coordinate);
    let coordinate = listing
        .coordinate(seller_pubkey.to_string())
        .map_err(|error| format!("Cannot derive NIP-99 classified coordinate: {error}"))?;
    Ok((
        listing,
        NostrPublicationRef {
            coordinate: Some(coordinate),
            event_id: None,
            relay_url: None,
        },
    ))
}

fn publish_dataset_listing(
    identity: &NostrIdentity,
    relay_urls: &[String],
    asset: &DataAsset,
) -> Result<NostrPublicationRef, String> {
    let (listing, mut publication_ref) =
        build_dataset_listing(asset, identity.public_key_hex.as_str())?;
    let template = listing
        .to_event_template(ds_created_at_seconds(asset.created_at_ms))
        .map_err(|error| format!("Cannot build DS listing event: {error}"))?;
    let event = sign_event_template(identity, &template)?;
    let accepted_relays = publish_event_to_relays(relay_urls, &event, "DS listing")?;
    publication_ref.event_id = Some(event.id);
    publication_ref.relay_url = accepted_relays.first().cloned();
    Ok(publication_ref)
}

fn publish_dataset_classified_listing(
    identity: &NostrIdentity,
    relay_urls: &[String],
    asset: &DataAsset,
) -> Result<NostrPublicationRef, String> {
    let (listing, mut publication_ref) =
        build_dataset_classified_listing(asset, identity.public_key_hex.as_str())?;
    let template = listing
        .to_event_template(ds_created_at_seconds(asset.created_at_ms))
        .map_err(|error| format!("Cannot build NIP-99 classified listing event: {error}"))?;
    let event = sign_event_template(identity, &template)?;
    let accepted_relays =
        publish_event_to_relays(relay_urls, &event, "NIP-99 dataset classified listing")?;
    publication_ref.event_id = Some(event.id);
    publication_ref.relay_url = accepted_relays.first().cloned();
    Ok(publication_ref)
}

fn build_dataset_storefront_stall(
    seller_pubkey: &str,
    currency: &str,
) -> Result<(MarketplaceStall, NostrPublicationRef), String> {
    let stall = MarketplaceStall::new(
        storefront_stall_identifier(currency),
        format!("OpenAgents datasets ({currency})"),
        currency.to_string(),
    )
    .with_description(format!(
        "OpenAgents storefront wrappers for NIP-DS datasets priced in {currency}."
    ));
    let coordinate = stall
        .coordinate(seller_pubkey.to_string())
        .map_err(|error| format!("Cannot derive NIP-15 stall coordinate: {error}"))?;
    Ok((
        stall,
        NostrPublicationRef {
            coordinate: Some(coordinate),
            event_id: None,
            relay_url: None,
        },
    ))
}

fn build_dataset_storefront_product(
    asset: &DataAsset,
    seller_pubkey: &str,
) -> Result<(MarketplaceProduct, NostrPublicationRef), String> {
    let listing_coordinate = asset
        .nostr_publications
        .ds_listing
        .as_ref()
        .and_then(|reference| reference.coordinate.clone())
        .or_else(|| {
            AddressableEventCoordinate::dataset_listing(
                seller_pubkey.to_string(),
                asset.asset_id.clone(),
            )
            .ok()
            .map(|coordinate| coordinate.to_string())
        })
        .ok_or_else(|| "Cannot derive DS listing coordinate for NIP-15 storefront.".to_string())?;
    let (currency, price) = storefront_currency_and_price(asset.price_hint.as_ref())
        .ok_or_else(|| "NIP-15 storefront publication requires a fixed asset price.".to_string())?;
    let mut product = MarketplaceProduct::new(
        format!("storefront.{}", asset.asset_id),
        storefront_stall_identifier(currency.as_str()),
        asset.title.clone(),
        currency,
        price,
    )
    .map_err(|error| format!("Cannot build NIP-15 product: {error}"))?
    .with_description(
        asset
            .description
            .clone()
            .unwrap_or_else(|| format!("Storefront wrapper for dataset {}.", asset.title)),
    )
    .with_quantity(None)
    .add_spec("dataset_kind", asset.asset_kind.clone());
    if let Some(access) = asset
        .price_hint
        .as_ref()
        .map(|_| "paid".to_string())
        .or_else(|| Some("targeted".to_string()))
    {
        product = product.add_spec("access", access);
    }
    product = product.add_spec("delivery", ds_delivery_modes(&asset.metadata).join(","));
    product.add_tag("dataset");
    product.add_tag("nip-ds");
    product.add_tag(asset.asset_kind.clone());
    product.add_address_ref(listing_coordinate);
    let coordinate = product
        .coordinate(seller_pubkey.to_string())
        .map_err(|error| format!("Cannot derive NIP-15 product coordinate: {error}"))?;
    Ok((
        product,
        NostrPublicationRef {
            coordinate: Some(coordinate),
            event_id: None,
            relay_url: None,
        },
    ))
}

fn publish_dataset_storefront_stall(
    identity: &NostrIdentity,
    relay_urls: &[String],
    currency: &str,
    created_at_ms: i64,
) -> Result<NostrPublicationRef, String> {
    let (stall, mut publication_ref) =
        build_dataset_storefront_stall(identity.public_key_hex.as_str(), currency)?;
    let template = stall
        .to_event_template(ds_created_at_seconds(created_at_ms))
        .map_err(|error| format!("Cannot build NIP-15 stall event: {error}"))?;
    let event = sign_event_template(identity, &template)?;
    let accepted_relays = publish_event_to_relays(relay_urls, &event, "NIP-15 dataset stall")?;
    publication_ref.event_id = Some(event.id);
    publication_ref.relay_url = accepted_relays.first().cloned();
    Ok(publication_ref)
}

fn publish_dataset_storefront_product(
    identity: &NostrIdentity,
    relay_urls: &[String],
    asset: &DataAsset,
) -> Result<NostrPublicationRef, String> {
    let (product, mut publication_ref) =
        build_dataset_storefront_product(asset, identity.public_key_hex.as_str())?;
    let template = product
        .to_event_template(ds_created_at_seconds(asset.created_at_ms))
        .map_err(|error| format!("Cannot build NIP-15 storefront product: {error}"))?;
    let event = sign_event_template(identity, &template)?;
    let accepted_relays =
        publish_event_to_relays(relay_urls, &event, "NIP-15 dataset storefront product")?;
    publication_ref.event_id = Some(event.id);
    publication_ref.relay_url = accepted_relays.first().cloned();
    Ok(publication_ref)
}

fn grant_offer_status(grant: &AccessGrant) -> DatasetOfferStatus {
    match grant.status {
        AccessGrantStatus::Offered | AccessGrantStatus::Accepted | AccessGrantStatus::Delivered => {
            DatasetOfferStatus::Active
        }
        AccessGrantStatus::Revoked | AccessGrantStatus::Refunded => DatasetOfferStatus::Revoked,
        AccessGrantStatus::Expired => DatasetOfferStatus::Expired,
    }
}

fn grant_listing_status(grant: &AccessGrant) -> ListingStatus {
    match grant.status {
        AccessGrantStatus::Offered | AccessGrantStatus::Accepted | AccessGrantStatus::Delivered => {
            ListingStatus::Active
        }
        AccessGrantStatus::Revoked | AccessGrantStatus::Refunded | AccessGrantStatus::Expired => {
            ListingStatus::Sold
        }
    }
}

fn build_dataset_offer(
    grant: &AccessGrant,
    asset: &DataAsset,
    seller_pubkey: &str,
) -> Result<(DatasetOffer, NostrPublicationRef), String> {
    let listing_coordinate = asset
        .nostr_publications
        .ds_listing
        .as_ref()
        .and_then(|reference| reference.coordinate.clone())
        .or_else(|| {
            AddressableEventCoordinate::dataset_listing(
                seller_pubkey.to_string(),
                asset.asset_id.clone(),
            )
            .ok()
            .map(|coordinate| coordinate.to_string())
        })
        .ok_or_else(|| "Cannot derive DS offer listing reference.".to_string())?;
    let listing_ref = AddressableEventReference::new(
        listing_coordinate
            .parse::<AddressableEventCoordinate>()
            .map_err(|error| format!("Cannot parse DS listing coordinate: {error}"))?,
    );
    let mut offer = DatasetOffer::new(
        grant.grant_id.clone(),
        format!(
            "Access offer for asset {} under policy {}.",
            grant.asset_id, grant.permission_policy.policy_id
        ),
        listing_ref,
    )
    .with_status(grant_offer_status(grant))
    .with_policy(if grant.consumer_id.is_some() {
        "targeted_request"
    } else {
        "licensed_bundle"
    })
    .with_license(grant.permission_policy.policy_id.clone())
    .with_expiration((grant.expires_at_ms / 1000).to_string())
    .add_payment_method(PaymentMethod::new("ln"))
    .add_topic("dataset");
    if let Some(price) = ds_price_from_money(grant.offer_price.as_ref()) {
        offer = offer.with_price(price);
    }
    for delivery_mode in ds_delivery_modes(&grant.metadata) {
        offer = offer.add_delivery_mode(delivery_mode);
    }
    if let Some(targeted_buyer) = maybe_targeted_buyer(grant.consumer_id.as_deref()) {
        offer = offer.add_targeted_buyer(
            nostr::nip_ds::PublicKeyReference::new(targeted_buyer)
                .map_err(|error| format!("Cannot build DS targeted buyer tag: {error}"))?,
        );
    }
    let coordinate = offer
        .coordinate(seller_pubkey.to_string())
        .map_err(|error| format!("Cannot derive DS offer coordinate: {error}"))?;
    Ok((
        offer,
        NostrPublicationRef {
            coordinate: Some(coordinate.to_string()),
            event_id: None,
            relay_url: None,
        },
    ))
}

fn build_dataset_offer_classified_listing(
    grant: &AccessGrant,
    asset: &DataAsset,
    seller_pubkey: &str,
) -> Result<(ClassifiedListing, NostrPublicationRef), String> {
    let listing_coordinate = asset
        .nostr_publications
        .ds_listing
        .as_ref()
        .and_then(|reference| reference.coordinate.clone())
        .or_else(|| {
            AddressableEventCoordinate::dataset_listing(
                seller_pubkey.to_string(),
                asset.asset_id.clone(),
            )
            .ok()
            .map(|coordinate| coordinate.to_string())
        })
        .ok_or_else(|| {
            "Cannot derive DS listing coordinate for NIP-99 offer wrapper.".to_string()
        })?;
    let offer_coordinate = grant
        .nostr_publications
        .ds_offer
        .as_ref()
        .and_then(|reference| reference.coordinate.clone())
        .or_else(|| {
            AddressableEventCoordinate::dataset_offer(
                seller_pubkey.to_string(),
                grant.grant_id.clone(),
            )
            .ok()
            .map(|coordinate| coordinate.to_string())
        })
        .ok_or_else(|| "Cannot derive DS offer coordinate for NIP-99 offer wrapper.".to_string())?;
    let mut listing = ClassifiedListing::new(
        format!("catalog-offer.{}", grant.grant_id),
        format!(
            "Public access offer for dataset {} under policy {}.",
            asset.title, grant.permission_policy.policy_id
        ),
        format!("{} access", asset.title),
    )
    .with_published_at(ds_created_at_seconds(grant.created_at_ms))
    .with_status(grant_listing_status(grant));
    if let Some(price) =
        ds_price_from_money(grant.offer_price.as_ref().or(asset.price_hint.as_ref()))
    {
        listing = listing.with_price(price);
    }
    listing = listing.with_summary(format!(
        "{} // {}",
        asset.asset_kind, grant.permission_policy.policy_id
    ));
    listing.add_tag("dataset");
    listing.add_tag("nip-ds");
    listing.add_tag("access-offer");
    listing.add_tag(asset.asset_kind.clone());
    listing.add_address_ref(listing_coordinate);
    listing.add_address_ref(offer_coordinate);
    let coordinate = listing
        .coordinate(seller_pubkey.to_string())
        .map_err(|error| format!("Cannot derive NIP-99 offer classified coordinate: {error}"))?;
    Ok((
        listing,
        NostrPublicationRef {
            coordinate: Some(coordinate),
            event_id: None,
            relay_url: None,
        },
    ))
}

fn publish_dataset_offer(
    identity: &NostrIdentity,
    relay_urls: &[String],
    grant: &AccessGrant,
    asset: &DataAsset,
) -> Result<NostrPublicationRef, String> {
    let (offer, mut publication_ref) =
        build_dataset_offer(grant, asset, identity.public_key_hex.as_str())?;
    let template = offer
        .to_event_template(ds_created_at_seconds(grant.created_at_ms))
        .map_err(|error| format!("Cannot build DS offer event: {error}"))?;
    let event = sign_event_template(identity, &template)?;
    let accepted_relays = publish_event_to_relays(relay_urls, &event, "DS offer")?;
    publication_ref.event_id = Some(event.id);
    publication_ref.relay_url = accepted_relays.first().cloned();
    Ok(publication_ref)
}

fn publish_dataset_offer_classified_listing(
    identity: &NostrIdentity,
    relay_urls: &[String],
    grant: &AccessGrant,
    asset: &DataAsset,
) -> Result<NostrPublicationRef, String> {
    let (listing, mut publication_ref) =
        build_dataset_offer_classified_listing(grant, asset, identity.public_key_hex.as_str())?;
    let template = listing
        .to_event_template(ds_created_at_seconds(grant.created_at_ms))
        .map_err(|error| format!("Cannot build NIP-99 offer classified listing: {error}"))?;
    let event = sign_event_template(identity, &template)?;
    let accepted_relays = publish_event_to_relays(
        relay_urls,
        &event,
        "NIP-99 dataset offer classified listing",
    )?;
    publication_ref.event_id = Some(event.id);
    publication_ref.relay_url = accepted_relays.first().cloned();
    Ok(publication_ref)
}

fn build_dataset_offer_storefront_product(
    grant: &AccessGrant,
    asset: &DataAsset,
    seller_pubkey: &str,
) -> Result<(MarketplaceProduct, NostrPublicationRef), String> {
    let listing_coordinate = asset
        .nostr_publications
        .ds_listing
        .as_ref()
        .and_then(|reference| reference.coordinate.clone())
        .or_else(|| {
            AddressableEventCoordinate::dataset_listing(
                seller_pubkey.to_string(),
                asset.asset_id.clone(),
            )
            .ok()
            .map(|coordinate| coordinate.to_string())
        })
        .ok_or_else(|| {
            "Cannot derive DS listing coordinate for NIP-15 offer storefront.".to_string()
        })?;
    let offer_coordinate = grant
        .nostr_publications
        .ds_offer
        .as_ref()
        .and_then(|reference| reference.coordinate.clone())
        .or_else(|| {
            AddressableEventCoordinate::dataset_offer(
                seller_pubkey.to_string(),
                grant.grant_id.clone(),
            )
            .ok()
            .map(|coordinate| coordinate.to_string())
        })
        .ok_or_else(|| {
            "Cannot derive DS offer coordinate for NIP-15 offer storefront.".to_string()
        })?;
    let (currency, price) =
        storefront_currency_and_price(grant.offer_price.as_ref().or(asset.price_hint.as_ref()))
            .ok_or_else(|| {
                "NIP-15 offer storefront publication requires a fixed price.".to_string()
            })?;
    let mut product = MarketplaceProduct::new(
        format!("storefront-offer.{}", grant.grant_id),
        storefront_stall_identifier(currency.as_str()),
        format!("{} access", asset.title),
        currency,
        price,
    )
    .map_err(|error| format!("Cannot build NIP-15 offer product: {error}"))?
    .with_description(format!(
        "Storefront access offer for dataset {} under policy {}.",
        asset.title, grant.permission_policy.policy_id
    ))
    .with_quantity(None)
    .add_spec("dataset_kind", asset.asset_kind.clone())
    .add_spec("policy", grant.permission_policy.policy_id.clone())
    .add_spec("delivery", ds_delivery_modes(&grant.metadata).join(","));
    product.add_tag("dataset");
    product.add_tag("nip-ds");
    product.add_tag("access-offer");
    product.add_tag(asset.asset_kind.clone());
    product.add_address_ref(listing_coordinate);
    product.add_address_ref(offer_coordinate);
    let coordinate = product
        .coordinate(seller_pubkey.to_string())
        .map_err(|error| format!("Cannot derive NIP-15 offer product coordinate: {error}"))?;
    Ok((
        product,
        NostrPublicationRef {
            coordinate: Some(coordinate),
            event_id: None,
            relay_url: None,
        },
    ))
}

fn publish_dataset_offer_storefront_product(
    identity: &NostrIdentity,
    relay_urls: &[String],
    grant: &AccessGrant,
    asset: &DataAsset,
) -> Result<NostrPublicationRef, String> {
    let (product, mut publication_ref) =
        build_dataset_offer_storefront_product(grant, asset, identity.public_key_hex.as_str())?;
    let template = product
        .to_event_template(ds_created_at_seconds(grant.created_at_ms))
        .map_err(|error| format!("Cannot build NIP-15 offer product: {error}"))?;
    let event = sign_event_template(identity, &template)?;
    let accepted_relays = publish_event_to_relays(
        relay_urls,
        &event,
        "NIP-15 dataset offer storefront product",
    )?;
    publication_ref.event_id = Some(event.id);
    publication_ref.relay_url = accepted_relays.first().cloned();
    Ok(publication_ref)
}

fn delivery_mode_for_request(request: &DataSellerIncomingRequest) -> DataVendingDeliveryMode {
    request
        .delivery_mode
        .as_deref()
        .and_then(|value| DataVendingDeliveryMode::from_str(value).ok())
        .unwrap_or_default()
}

fn preview_posture_for_request(request: &DataSellerIncomingRequest) -> DataVendingPreviewPosture {
    if request
        .delivery_draft
        .preview_text
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        DataVendingPreviewPosture::InlinePreview
    } else {
        request
            .preview_posture
            .as_deref()
            .and_then(|value| DataVendingPreviewPosture::from_str(value).ok())
            .unwrap_or(DataVendingPreviewPosture::MetadataOnly)
    }
}

fn build_delivery_result_content(
    request: &DataSellerIncomingRequest,
    delivery: &DeliveryBundle,
) -> String {
    if let Some(preview_text) = request
        .delivery_draft
        .preview_text
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        preview_text.to_string()
    } else {
        json!({
            "dataset": request.matched_listing_coordinate,
            "offer": request.matched_offer_coordinate,
            "delivery_bundle_id": delivery.delivery_bundle_id,
            "delivery_ref": delivery.delivery_ref,
            "delivery_digest": delivery.delivery_digest,
            "grant_id": delivery.grant_id,
            "asset_id": delivery.asset_id,
        })
        .to_string()
    }
}

fn build_data_seller_delivery_result_event(
    identity: &NostrIdentity,
    request: &DataSellerIncomingRequest,
    delivery: &DeliveryBundle,
) -> Result<Event, String> {
    let asset_ref = request_asset_ref_for_event(request)?;
    let mut result = DataVendingResult::new(
        request.request_kind,
        request.request_id.as_str(),
        request.requester.as_str(),
        asset_ref,
        delivery.delivery_bundle_id.as_str(),
        build_delivery_result_content(request, delivery),
    )
    .map_err(|error| format!("Cannot build data-vending result: {error}"))?
    .with_listing_ref(request_listing_ref_for_event(request)?)
    .with_delivery_mode(delivery_mode_for_request(request))
    .with_preview_posture(preview_posture_for_request(request))
    .with_asset_id(delivery.asset_id.clone())
    .with_grant_id(delivery.grant_id.clone())
    .with_delivery_ref(delivery.delivery_ref.clone());
    if let Some(offer_ref) = request_offer_ref_for_event(request)? {
        result = result.with_offer_ref(offer_ref);
    }
    if let Some(delivery_digest) = delivery.delivery_digest.as_deref() {
        result = result.with_delivery_digest(delivery_digest.to_string());
    }
    if request.encrypted
        || matches!(
            delivery_mode_for_request(request),
            DataVendingDeliveryMode::EncryptedPointer
        )
    {
        result = result.with_encrypted_content();
    }
    let template = create_data_vending_result_event(&result)
        .map_err(|error| format!("Cannot build NIP-90 delivery result event: {error}"))?;
    sign_event_template(identity, &template)
}

fn build_accept_access_grant_request(
    request: &DataSellerIncomingRequest,
    grant_id: &str,
    session_id: Option<&str>,
    accepted_at_ms: i64,
) -> AcceptAccessGrantRequest {
    let payment_amount_sats = request
        .payment_amount_sats
        .or(request.required_price_sats)
        .or((request.price_sats > 0).then_some(request.price_sats));
    let payment_pointer = request
        .payment_pointer
        .as_deref()
        .unwrap_or("missing_payment_pointer");
    AcceptAccessGrantRequest {
        idempotency_key: format!(
            "accept_access_grant:{}",
            sha256_prefixed_text(
                format!(
                    "{grant_id}|{}|{payment_pointer}|{}",
                    request.request_id,
                    payment_amount_sats.unwrap_or_default()
                )
                .as_str()
            )
        ),
        trace: TraceContext {
            session_id: session_id.map(str::to_string),
            trajectory_hash: session_id.map(sha256_prefixed_text),
            claim_id: Some(format!(
                "claim.access_grant.accept.{}",
                canonical_component(grant_id)
            )),
            ..Default::default()
        },
        policy: PolicyContext {
            policy_bundle_id: "policy.data_market.delivery.mvp".to_string(),
            policy_version: "v0".to_string(),
            approved_by: "openagents.data_market.seller_pane".to_string(),
        },
        grant_id: grant_id.to_string(),
        consumer_id: request.effective_consumer_id().to_string(),
        accepted_at_ms,
        settlement_price: payment_amount_sats.map(sats_money),
        metadata: json!({
            "request_id": request.request_id,
            "payment_feedback_event_id": request.payment_feedback_event_id,
            "payment_pointer": request.payment_pointer,
            "payment_amount_sats": request.payment_amount_sats,
            "ds_listing_coordinate": request.matched_listing_coordinate,
            "ds_offer_coordinate": request.matched_offer_coordinate,
            "asset_ref": request.asset_ref,
        }),
        evidence: {
            let mut evidence = vec![EvidenceRef::new(
                "nip90_request_event",
                format!("nostr:event:{}", request.request_id),
                request.request_id.as_str(),
            )];
            if let Some(listing_coordinate) = request
                .matched_listing_coordinate
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                evidence.push(EvidenceRef::new(
                    "ds_listing",
                    format!("nostr:a:{listing_coordinate}"),
                    listing_coordinate,
                ));
            }
            if let Some(offer_coordinate) = request
                .matched_offer_coordinate
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                evidence.push(EvidenceRef::new(
                    "ds_offer",
                    format!("nostr:a:{offer_coordinate}"),
                    offer_coordinate,
                ));
            }
            if let Some(payment_pointer) = request
                .payment_pointer
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                evidence.push(EvidenceRef::new(
                    "payment_pointer",
                    format!(
                        "oa://autopilot/payments/{}",
                        canonical_component(payment_pointer)
                    ),
                    payment_pointer,
                ));
            }
            evidence
        },
        hints: ReceiptHints::default(),
    }
}

fn build_issue_delivery_bundle_request(
    request: &DataSellerIncomingRequest,
    grant_id: &str,
    asset_id: &str,
    provider_id: &str,
    consumer_id: &str,
    session_id: Option<&str>,
    created_at_ms: i64,
) -> IssueDeliveryBundleRequest {
    let delivery_ref = request
        .delivery_draft
        .delivery_ref
        .clone()
        .unwrap_or_else(|| {
            format!(
                "oa://deliveries/{}",
                canonical_component(request.request_id.as_str())
            )
        });
    let delivery_bundle_id = format!(
        "delivery_bundle.{}.{}.{}",
        canonical_component(provider_id),
        canonical_component(grant_id),
        canonical_component(request.request_id.as_str())
    );
    let expires_at_ms = request.delivery_draft.expires_in_hours.map(|hours| {
        created_at_ms.saturating_add(
            i64::try_from(hours)
                .unwrap_or(i64::MAX)
                .saturating_mul(60)
                .saturating_mul(60)
                .saturating_mul(1000),
        )
    });
    IssueDeliveryBundleRequest {
        idempotency_key: format!(
            "issue_delivery_bundle:{}",
            sha256_prefixed_text(
                format!(
                    "{grant_id}|{}|{}|{}",
                    request.request_id,
                    delivery_ref,
                    request
                        .delivery_draft
                        .delivery_digest
                        .as_deref()
                        .unwrap_or("missing_delivery_digest")
                )
                .as_str()
            )
        ),
        trace: TraceContext {
            session_id: session_id.map(str::to_string),
            trajectory_hash: session_id.map(sha256_prefixed_text),
            claim_id: Some(format!(
                "claim.data.delivery.issue.{}",
                canonical_component(request.request_id.as_str())
            )),
            ..Default::default()
        },
        policy: PolicyContext {
            policy_bundle_id: "policy.data_market.delivery.mvp".to_string(),
            policy_version: "v0".to_string(),
            approved_by: "openagents.data_market.seller_pane".to_string(),
        },
        delivery_bundle: DeliveryBundle {
            delivery_bundle_id,
            asset_id: asset_id.to_string(),
            grant_id: grant_id.to_string(),
            provider_id: provider_id.to_string(),
            consumer_id: consumer_id.to_string(),
            created_at_ms,
            delivery_ref,
            delivery_digest: request.delivery_draft.delivery_digest.clone(),
            bundle_size_bytes: request.delivery_draft.bundle_size_bytes,
            manifest_refs: request.delivery_draft.manifest_refs.clone(),
            expires_at_ms,
            status: DeliveryBundleStatus::Issued,
            metadata: json!({
                "request_id": request.request_id,
                "payment_pointer": request.payment_pointer,
                "payment_amount_sats": request.payment_amount_sats,
                "delivery_mode": request.delivery_mode,
                "preview_posture": request.preview_posture,
                "preview_text": request.delivery_draft.preview_text,
                "ds_listing_coordinate": request.matched_listing_coordinate,
                "ds_offer_coordinate": request.matched_offer_coordinate,
            }),
        },
        evidence: {
            let mut evidence = vec![EvidenceRef::new(
                "nip90_request_event",
                format!("nostr:event:{}", request.request_id),
                request.request_id.as_str(),
            )];
            if let Some(listing_coordinate) = request
                .matched_listing_coordinate
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                evidence.push(EvidenceRef::new(
                    "ds_listing",
                    format!("nostr:a:{listing_coordinate}"),
                    listing_coordinate,
                ));
            }
            if let Some(offer_coordinate) = request
                .matched_offer_coordinate
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                evidence.push(EvidenceRef::new(
                    "ds_offer",
                    format!("nostr:a:{offer_coordinate}"),
                    offer_coordinate,
                ));
            }
            if let Some(payment_pointer) = request
                .payment_pointer
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                evidence.push(EvidenceRef::new(
                    "payment_pointer",
                    format!(
                        "oa://autopilot/payments/{}",
                        canonical_component(payment_pointer)
                    ),
                    payment_pointer,
                ));
            }
            if let Some(delivery_digest) = request
                .delivery_draft
                .delivery_digest
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                evidence.push(EvidenceRef::new(
                    "delivery_digest",
                    format!(
                        "oa://autopilot/data_deliveries/{}/digest",
                        canonical_component(request.request_id.as_str())
                    ),
                    delivery_digest,
                ));
            }
            evidence
        },
        hints: ReceiptHints::default(),
    }
}

fn default_revocation_reason_code(action: DataSellerRevocationAction) -> &'static str {
    match action {
        DataSellerRevocationAction::Revoke => "seller_revoked_access",
        DataSellerRevocationAction::Expire => "access_window_expired",
    }
}

fn explicit_revocation_reason_code(
    action: DataSellerRevocationAction,
    reason_code: Option<&str>,
) -> String {
    reason_code
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_revocation_reason_code(action).to_string())
}

fn build_revoke_access_grant_request(
    request: &DataSellerIncomingRequest,
    grant_id: &str,
    asset_id: &str,
    provider_id: &str,
    consumer_id: Option<&str>,
    revoked_delivery_bundle_ids: Vec<String>,
    action: DataSellerRevocationAction,
    reason_code: Option<&str>,
    session_id: Option<&str>,
    created_at_ms: i64,
) -> RevokeAccessGrantRequest {
    let canonical_reason_code = explicit_revocation_reason_code(action, reason_code);
    RevokeAccessGrantRequest {
        idempotency_key: format!(
            "revoke_access_grant:{}",
            sha256_prefixed_text(
                format!(
                    "{grant_id}|{}|{}|{}",
                    request.request_id,
                    action.label(),
                    canonical_reason_code
                )
                .as_str()
            )
        ),
        trace: TraceContext {
            session_id: session_id.map(str::to_string),
            trajectory_hash: session_id.map(sha256_prefixed_text),
            claim_id: Some(format!(
                "claim.data.access.{}.{}",
                action.label(),
                canonical_component(request.request_id.as_str())
            )),
            ..Default::default()
        },
        policy: PolicyContext {
            policy_bundle_id: "policy.data_market.revocation.mvp".to_string(),
            policy_version: "v0".to_string(),
            approved_by: "openagents.data_market.seller_pane".to_string(),
        },
        revocation: RevocationReceipt {
            revocation_id: format!(
                "revocation.{}.{}.{}.{}",
                canonical_component(provider_id),
                canonical_component(grant_id),
                canonical_component(request.request_id.as_str()),
                action.label()
            ),
            asset_id: asset_id.to_string(),
            grant_id: grant_id.to_string(),
            provider_id: provider_id.to_string(),
            consumer_id: consumer_id.map(str::to_string),
            created_at_ms,
            reason_code: canonical_reason_code.clone(),
            refund_amount: None,
            revoked_delivery_bundle_ids: revoked_delivery_bundle_ids.clone(),
            replacement_delivery_bundle_id: None,
            status: Default::default(),
            metadata: json!({
                "request_id": request.request_id,
                "control_action": action.label(),
                "delivery_bundle_ids": revoked_delivery_bundle_ids,
                "payment_pointer": request.payment_pointer,
                "payment_amount_sats": request.payment_amount_sats,
                "ds_listing_coordinate": request.matched_listing_coordinate,
                "ds_offer_coordinate": request.matched_offer_coordinate,
            }),
        },
        evidence: {
            let mut evidence = vec![EvidenceRef::new(
                "nip90_request_event",
                format!("nostr:event:{}", request.request_id),
                request.request_id.as_str(),
            )];
            if let Some(listing_coordinate) = request
                .matched_listing_coordinate
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                evidence.push(EvidenceRef::new(
                    "ds_listing",
                    format!("nostr:a:{listing_coordinate}"),
                    listing_coordinate,
                ));
            }
            if let Some(offer_coordinate) = request
                .matched_offer_coordinate
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                evidence.push(EvidenceRef::new(
                    "ds_offer",
                    format!("nostr:a:{offer_coordinate}"),
                    offer_coordinate,
                ));
            }
            if let Some(payment_pointer) = request
                .payment_pointer
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                evidence.push(EvidenceRef::new(
                    "payment_pointer",
                    format!(
                        "oa://autopilot/payments/{}",
                        canonical_component(payment_pointer)
                    ),
                    payment_pointer,
                ));
            }
            for delivery_bundle_id in &request
                .delivery_bundle_id
                .iter()
                .cloned()
                .chain(revoked_delivery_bundle_ids.iter().cloned())
                .collect::<Vec<_>>()
            {
                evidence.push(EvidenceRef::new(
                    "delivery_bundle",
                    format!(
                        "oa://autopilot/data_deliveries/{}",
                        canonical_component(delivery_bundle_id.as_str())
                    ),
                    delivery_bundle_id.as_str(),
                ));
            }
            evidence
        },
        hints: ReceiptHints::default(),
    }
}

fn record_data_market_lifecycle_entry(
    state: &mut RenderState,
    occurred_at_ms: i64,
    stage: impl Into<String>,
    status: impl Into<String>,
    subject_id: impl Into<String>,
    counterparty: Option<String>,
    policy_id: Option<String>,
    receipt_id: Option<String>,
    summary: impl Into<String>,
) {
    state
        .data_market
        .record_lifecycle_entry(DataMarketLifecycleEntry {
            occurred_at_ms,
            stage: stage.into(),
            status: status.into(),
            subject_id: subject_id.into(),
            counterparty,
            policy_id,
            receipt_id,
            summary: summary.into(),
        });
}

fn matched_settled_receive_payment<'a>(
    invoice: Option<&str>,
    payment_hash: Option<&str>,
    recent_payments: &'a [PaymentSummary],
) -> Option<&'a PaymentSummary> {
    let expected_invoice = invoice.and_then(normalize_lightning_invoice_ref);
    let expected_payment_hash = payment_hash
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
        .or_else(|| invoice.and_then(decode_lightning_invoice_payment_hash));

    recent_payments
        .iter()
        .filter(|payment| {
            payment.direction.eq_ignore_ascii_case("receive")
                && is_settled_wallet_payment_status(payment.status.as_str())
                && !payment.id.trim().is_empty()
        })
        .filter(|payment| {
            expected_invoice.as_deref().is_some_and(|expected_invoice| {
                payment
                    .invoice
                    .as_deref()
                    .and_then(normalize_lightning_invoice_ref)
                    .is_some_and(|candidate| candidate == expected_invoice)
            }) || expected_payment_hash
                .as_deref()
                .is_some_and(|expected_payment_hash| {
                    payment
                        .payment_hash
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_ascii_lowercase)
                        .is_some_and(|candidate| candidate == expected_payment_hash)
                })
        })
        .max_by_key(|payment| payment.timestamp)
}

pub(crate) fn ensure_data_seller_codex_session(state: &mut RenderState) -> bool {
    if matches!(
        state.data_seller.codex_session_phase,
        DataSellerCodexSessionPhase::Starting | DataSellerCodexSessionPhase::Resuming
    ) {
        return true;
    }

    let cwd = current_session_cwd();
    match crate::skill_autoload::ensure_required_data_market_skills() {
        Ok(skills) => state.data_seller.set_required_skill_attachments(
            skills
                .into_iter()
                .map(|skill| DataSellerSkillAttachment {
                    name: skill.name,
                    path: skill.path,
                })
                .collect(),
        ),
        Err(error) => {
            tracing::warn!(
                "failed to auto-provision managed Data Market skills before seller session: {}",
                error
            );
            state.data_seller.last_error =
                Some(format!("Failed to auto-provision seller skills: {error}"));
        }
    }
    let model = state.autopilot_chat.selected_model_override();
    let service_tier = state.autopilot_chat.service_tier.request_value();
    let approval_policy = Some(state.autopilot_chat.approval_mode);
    let sandbox = Some(state.autopilot_chat.sandbox_mode);
    let personality = state.data_seller.codex_profile.personality.request_value();

    let command = if let Some(thread_id) = state.data_seller.codex_thread_id.clone() {
        state.data_seller.begin_codex_session_resume(cwd.clone());
        CodexLaneCommand::ThreadResume(ThreadResumeParams {
            thread_id,
            path: None,
            model,
            model_provider: None,
            service_tier,
            cwd,
            approval_policy,
            sandbox,
            personality,
        })
    } else {
        state.data_seller.begin_codex_session_start(cwd.clone());
        CodexLaneCommand::ThreadStart(ThreadStartParams {
            model,
            model_provider: None,
            service_tier,
            cwd,
            approval_policy,
            sandbox,
            personality,
            ephemeral: None,
            dynamic_tools: Some(crate::openagents_dynamic_tools::openagents_dynamic_tool_specs()),
        })
    };

    if let Err(error) = state.queue_codex_command(command) {
        state.data_seller.record_codex_session_error(format!(
            "Failed to queue Data Seller Codex session: {error}"
        ));
    }
    true
}

pub(crate) fn submit_data_seller_prompt_text(state: &mut RenderState, prompt: String) -> bool {
    if state.data_seller.codex_thread_id.is_none() {
        ensure_data_seller_codex_session(state);
    }
    let Some(thread_id) = state.data_seller.codex_thread_id.clone() else {
        state.data_seller.last_error = Some(
            "Data Seller session is still starting. Wait for the dedicated thread, then retry."
                .to_string(),
        );
        return true;
    };

    if prompt.trim().is_empty() {
        return false;
    }

    let mut input = vec![UserInput::Text {
        text: prompt.clone(),
        text_elements: Vec::new(),
    }];
    for skill in &state.data_seller.required_skill_attachments {
        input.push(UserInput::Skill {
            name: skill.name.clone(),
            path: PathBuf::from(skill.path.clone()),
        });
    }

    let command = CodexLaneCommand::TurnStart(TurnStartParams {
        thread_id: thread_id.clone(),
        input,
        cwd: state
            .data_seller
            .codex_session_cwd
            .clone()
            .map(PathBuf::from),
        approval_policy: None,
        sandbox_policy: None,
        model: None,
        service_tier: None,
        effort: None,
        summary: None,
        personality: state.data_seller.codex_profile.personality.request_value(),
        output_schema: None,
        collaboration_mode: None,
    });

    match state.queue_codex_command(command) {
        Ok(command_seq) => {
            state.autopilot_chat.append_cached_thread_message(
                &thread_id,
                AutopilotRole::User,
                prompt,
            );
            state.data_seller_inputs.composer.set_value(String::new());
            state.data_seller.last_error = None;
            state.data_seller.last_action = Some(format!(
                "Queued Data Seller turn on thread {thread_id} (command #{command_seq})"
            ));
            state.data_seller.status_line =
                "Seller prompt sent. Waiting for Codex to normalize the draft or ask follow-up questions."
                    .to_string();
            state.data_seller.set_codex_thread_status("running");
        }
        Err(error) => {
            state.data_seller.last_error =
                Some(format!("Failed to queue Data Seller turn: {error}"));
        }
    }
    true
}

pub(crate) fn submit_data_seller_prompt(state: &mut RenderState) -> bool {
    let prompt = state
        .data_seller_inputs
        .composer
        .get_value()
        .trim()
        .to_string();
    if prompt.is_empty() {
        return false;
    }
    submit_data_seller_prompt_text(state, prompt)
}

pub(crate) fn request_data_seller_preview(state: &mut RenderState) -> bool {
    let provider_id = crate::kernel_control::provider_id_for_state(state);
    state
        .data_seller
        .request_preview(provider_id.as_str(), current_epoch_ms());
    true
}

pub(crate) fn confirm_data_seller_preview(state: &mut RenderState) -> bool {
    state.data_seller.confirm_asset_preview();
    true
}

pub(crate) fn request_data_seller_grant_preview(state: &mut RenderState) -> bool {
    hydrate_data_seller_inventory_from_relay_replica(state);
    let provider_id = crate::kernel_control::provider_id_for_state(state);
    state
        .data_seller
        .request_grant_preview(provider_id.as_str(), current_epoch_ms());
    true
}

pub(crate) fn request_data_seller_payment_required(
    state: &mut RenderState,
    request_id: &str,
) -> bool {
    if state.active_job.payment_required_invoice_requested {
        state.data_seller.last_error = Some(
            "A compute-market settlement invoice is already being created. Wait for it to finish before starting a data-market payment quote."
                .to_string(),
        );
        return true;
    }

    let amount_sats = match state.data_seller.request_payment_required_quote(request_id) {
        Ok(amount_sats) => amount_sats,
        Err(error) => {
            state.data_seller.last_error = Some(error);
            return true;
        }
    };

    state.spark_wallet.last_error = None;
    if let Err(error) = state
        .spark_worker
        .enqueue(SparkWalletCommand::CreateBolt11Invoice {
            amount_sats,
            description: Some(format!("OpenAgents data access {request_id}")),
            expiry_seconds: Some(3600),
        })
    {
        state.data_seller.note_payment_invoice_failed(
            request_id,
            format!("Failed to queue Spark invoice creation: {error}"),
        );
        return true;
    }

    state.provider_runtime.last_result = Some(format!(
        "queued seller Lightning invoice creation for data request {}",
        request_id
    ));
    true
}

pub(crate) fn issue_data_seller_delivery(state: &mut RenderState, request_id: &str) -> bool {
    if let Err(error) = state.data_seller.request_issue_delivery(request_id) {
        state.data_seller.last_error = Some(error);
        return true;
    }

    let Some(request) = state.data_seller.request_by_id(request_id).cloned() else {
        state.data_seller.last_error = Some(format!(
            "Unknown data-access request {request_id} after delivery issue start."
        ));
        return true;
    };
    let Some(identity) = state.nostr_identity.clone() else {
        state.data_seller.note_delivery_issue_failed(
            request_id,
            "Cannot publish delivery result: Nostr identity unavailable.",
        );
        return true;
    };
    let consumer_id = request.effective_consumer_id().to_string();
    let created_at_ms = current_epoch_ms();
    let delivery = match relay_only_delivery_bundle_for_request(
        &request,
        identity.public_key_hex.as_str(),
        consumer_id.as_str(),
        created_at_ms,
    ) {
        Ok(delivery) => delivery,
        Err(error) => {
            state.data_seller.note_delivery_issue_failed(request_id, error);
            return true;
        }
    };
    if let Err(error) =
        state
            .data_seller
            .note_delivery_bundle_issued(request_id, delivery.clone(), None)
    {
        state.data_seller.note_delivery_issue_failed(request_id, error);
        return true;
    }
    state
        .data_market
        .note_published_delivery(delivery.clone(), current_epoch_ms());
    state.data_buyer.sync_selection(&state.data_market);
    record_data_market_lifecycle_entry(
        state,
        delivery.created_at_ms,
        "delivery_issued",
        delivery.status.label(),
        delivery.delivery_bundle_id.clone(),
        Some(delivery.consumer_id.clone()),
        state.data_seller.policy_id_for_request(request_id),
        None,
        format!(
            "Prepared relay-native delivery for grant {} with ref {}.",
            delivery.grant_id, delivery.delivery_ref
        ),
    );

    let event = match build_data_seller_delivery_result_event(&identity, &request, &delivery) {
        Ok(event) => event,
        Err(error) => {
            state
                .data_seller
                .note_delivery_issue_failed(request_id, error);
            return true;
        }
    };
    if let Err(error) =
        state.queue_provider_nip90_lane_command(ProviderNip90LaneCommand::PublishEvent {
            request_id: request_id.to_string(),
            role: ProviderNip90PublishRole::Result,
            event: Box::new(event),
        })
    {
        state.data_seller.note_delivery_issue_failed(
            request_id,
            format!("Cannot queue NIP-90 delivery result publish: {error}"),
        );
        return true;
    }

    if let Some(request) = state.data_seller.request_by_id_mut(request_id) {
        request.delivery_state = crate::app_state::DataSellerDeliveryState::PublishingResult;
        request.delivery_error = None;
    }
    state.data_seller.last_error = None;
    state.data_seller.last_action = Some(format!(
        "Queued NIP-90 delivery result publication for data request {}",
        request_id
    ));
    state.data_seller.status_line = format!(
        "Publishing NIP-90 delivery result for request {}.",
        request_id
    );

    state.provider_runtime.last_result = Some(format!(
        "queued delivery result publication for data request {}",
        request_id
    ));
    true
}

pub(crate) fn revoke_data_seller_access(
    state: &mut RenderState,
    request_id: &str,
    action: DataSellerRevocationAction,
    reason_code: Option<&str>,
) -> bool {
    if let Err(error) = state.data_seller.request_revoke_access(request_id, action) {
        state.data_seller.last_error = Some(error);
        return true;
    }

    let Some(request) = state.data_seller.request_by_id(request_id).cloned() else {
        state.data_seller.last_error = Some(format!(
            "Unknown data-access request {request_id} after revocation start."
        ));
        return true;
    };
    if request.matched_grant_id.is_none() {
        state
            .data_seller
            .note_revocation_failed(request_id, "Matched grant is missing for revocation.");
        return true;
    }

    let now_ms = current_epoch_ms();
    if action == DataSellerRevocationAction::Expire {
        let now_epoch_seconds = current_epoch_seconds();
        let contract_expired = state
            .data_market
            .relay_access_contract_for_request(request_id)
            .and_then(|contract| contract.expires_at_seconds)
            .is_some_and(|expires_at_seconds| now_epoch_seconds >= expires_at_seconds);
        let request_expired = request
            .expires_at_epoch_seconds
            .is_some_and(|expires_at_seconds| now_epoch_seconds >= expires_at_seconds);
        let delivery_expired = request
            .delivery_draft
            .expires_in_hours
            .and_then(|hours| {
                request
                    .payment_observed_at_epoch_seconds
                    .or(request.created_at_epoch_seconds)
                    .map(|started_at| {
                        now_epoch_seconds
                            >= started_at.saturating_add(hours.saturating_mul(60 * 60))
                    })
            })
            .unwrap_or(false);
        if !contract_expired && !request_expired && !delivery_expired {
            state.data_seller.note_revocation_failed(
                request_id,
                format!(
                    "Request {} cannot be expired yet because neither the relay-native contract nor the known delivery window is past expiry.",
                    request_id
                ),
            );
            return true;
        }
    }
    let Some(identity) = state.nostr_identity.clone() else {
        state.data_seller.note_revocation_failed(
            request_id,
            "Cannot record revocation without a Nostr identity.",
        );
        return true;
    };
    let consumer_id = request.effective_consumer_id().to_string();
    let revocation = match relay_only_revocation_receipt_for_request(
        &request,
        identity.public_key_hex.as_str(),
        Some(consumer_id.as_str()),
        action,
        reason_code,
        now_ms,
    ) {
        Ok(revocation) => revocation,
        Err(error) => {
            state.data_seller.note_revocation_failed(request_id, error);
            return true;
        }
    };
    let mut deliveries = Vec::new();
    if request.delivery_bundle_id.is_some() || request.delivery_draft.delivery_ref.is_some() {
        match relay_only_delivery_bundle_for_request(
            &request,
            identity.public_key_hex.as_str(),
            consumer_id.as_str(),
            request
                .payment_observed_at_epoch_seconds
                .and_then(|epoch_seconds| i64::try_from(epoch_seconds).ok())
                .unwrap_or_else(|| now_ms / 1000)
                .saturating_mul(1000),
        ) {
            Ok(mut delivery) => {
                delivery.status = match action {
                    DataSellerRevocationAction::Revoke => DeliveryBundleStatus::Revoked,
                    DataSellerRevocationAction::Expire => DeliveryBundleStatus::Expired,
                };
                deliveries.push(delivery);
            }
            Err(error) => {
                state.data_seller.note_revocation_failed(request_id, error);
                return true;
            }
        }
    }
    let grant = state
        .data_seller
        .published_grants_for_display()
        .into_iter()
        .find(|grant| {
            request
                .matched_grant_id
                .as_deref()
                .is_some_and(|grant_id| grant.grant_id.eq_ignore_ascii_case(grant_id))
        })
        .cloned()
        .unwrap_or_else(|| AccessGrant {
            grant_id: request
                .matched_grant_id
                .clone()
                .unwrap_or_else(|| format!("grant.{}", canonical_component(request.request_id.as_str()))),
            asset_id: request
                .matched_asset_id
                .clone()
                .unwrap_or_else(|| format!("asset.{}", canonical_component(request.request_id.as_str()))),
            provider_id: identity.public_key_hex.clone(),
            consumer_id: Some(consumer_id.clone()),
            permission_policy: openagents_kernel_core::data::PermissionPolicy {
                policy_id: state
                    .data_seller
                    .policy_id_for_request(request_id)
                    .unwrap_or_else(|| "targeted_request".to_string()),
                allowed_scopes: request.permission_scopes.clone(),
                ..Default::default()
            },
            offer_price: request
                .required_price_sats
                .or((request.price_sats > 0).then_some(request.price_sats))
                .map(sats_money),
            warranty_window_ms: None,
            created_at_ms: request
                .created_at_epoch_seconds
                .and_then(|epoch_seconds| i64::try_from(epoch_seconds).ok())
                .unwrap_or(now_ms / 1000)
                .saturating_mul(1000),
            expires_at_ms: request
                .expires_at_epoch_seconds
                .and_then(|epoch_seconds| i64::try_from(epoch_seconds).ok())
                .unwrap_or(now_ms / 1000)
                .saturating_mul(1000),
            accepted_at_ms: request
                .payment_observed_at_epoch_seconds
                .and_then(|epoch_seconds| i64::try_from(epoch_seconds).ok())
                .map(|epoch_seconds| epoch_seconds.saturating_mul(1000)),
            status: match action {
                DataSellerRevocationAction::Revoke => AccessGrantStatus::Revoked,
                DataSellerRevocationAction::Expire => AccessGrantStatus::Expired,
            },
            nostr_publications: openagents_kernel_core::data::AccessGrantNostrPublications {
                ds_offer: request.matched_offer_coordinate.as_ref().map(|coordinate| NostrPublicationRef {
                    coordinate: Some(coordinate.clone()),
                    event_id: None,
                    relay_url: request.source_relay_url.clone(),
                }),
                ds_access_request: None,
                ds_access_result: None,
            },
            metadata: json!({
                "relay_only": true,
                "request_id": request.request_id.clone(),
            }),
        });
    let reflected_at_ms = current_epoch_ms();
    if let Err(error) = state.data_seller.note_revocation_recorded(
        request_id,
        action,
        revocation.clone(),
        None,
        grant.clone(),
        deliveries.as_slice(),
    ) {
        state.data_seller.note_revocation_failed(request_id, error);
        return true;
    }
    state
        .data_market
        .note_published_grant(grant, reflected_at_ms);
    state.data_buyer.sync_selection(&state.data_market);
    for delivery in deliveries {
        state
            .data_market
            .note_published_delivery(delivery, reflected_at_ms);
        state.data_buyer.sync_selection(&state.data_market);
    }
    state
        .data_market
        .note_published_revocation(revocation.clone(), reflected_at_ms);
    state.data_buyer.sync_selection(&state.data_market);
    let revocation_feedback_event_id = {
        match build_data_seller_revocation_feedback_event(&identity, &request, &revocation) {
            Ok(event) => {
                let event_id = event.id.clone();
                match publish_event_to_relays(
                    state.configured_provider_relay_urls().as_slice(),
                    &event,
                    "DS-DVM revocation feedback",
                ) {
                    Ok(_) => Some(event_id),
                    Err(error) => {
                        state.data_seller.last_action = Some(format!(
                            "Recorded revocation for request {} but DS-DVM revocation feedback publish failed: {}",
                            request_id, error
                        ));
                        None
                    }
                }
            }
            Err(error) => {
                state.data_seller.last_action = Some(format!(
                    "Recorded revocation for request {} but could not build DS-DVM revocation feedback: {}",
                    request_id, error
                ));
                None
            }
        }
    };
    if let Err(error) = publish_data_seller_access_contract(
        state,
        request_id,
        match action {
            DataSellerRevocationAction::Revoke => DatasetAccessContractStatus::Revoked,
            DataSellerRevocationAction::Expire => DatasetAccessContractStatus::Expired,
        },
        request.delivery_result_event_id.as_deref(),
        reason_code,
    ) {
        state.data_seller.last_error = Some(format!(
            "Recorded relay-native revocation for request {} but DS access contract publish failed: {}",
            request_id, error
        ));
    }
    if let Some((
        revocation_state,
        revocation_id,
        requester,
        policy_id,
        receipt_id,
        grant_id,
        reason_code,
    )) = state.data_seller.request_by_id(request_id).map(|request| {
        (
            request.revocation_state,
            request
                .revocation_id
                .clone()
                .unwrap_or_else(|| format!("revocation_for_{request_id}")),
            request.requester.clone(),
            state.data_seller.policy_id_for_request(request_id),
            request.revocation_receipt_id.clone(),
            request
                .matched_grant_id
                .clone()
                .unwrap_or_else(|| "unknown_grant".to_string()),
            request
                .revocation_reason_code
                .clone()
                .unwrap_or_else(|| "unspecified".to_string()),
        )
    }) {
        record_data_market_lifecycle_entry(
            state,
            reflected_at_ms,
            format!("access_{}", action.past_tense_label()),
            revocation_state.label(),
            revocation_id,
            Some(requester),
            policy_id,
            receipt_id,
            format!(
                "{} access for grant {} with reason {}.{}",
                action.past_tense_label(),
                grant_id,
                reason_code,
                revocation_feedback_event_id
                    .as_deref()
                    .map(|event_id| format!(" DS-DVM notice {event_id}."))
                    .unwrap_or_default()
            ),
        );
    }
    state.provider_runtime.last_result = Some(format!(
        "seller {} access for data request {}",
        action.past_tense_label(),
        request_id
    ));
    true
}

fn queue_data_seller_payment_required_feedback(
    state: &mut RenderState,
    request_id: &str,
) -> Result<(), String> {
    let request = state
        .data_seller
        .request_by_id(request_id)
        .ok_or_else(|| format!("Unknown data-access request {request_id}"))?;
    let request_id = request.request_id.clone();
    let quoted_price_sats = request
        .required_price_sats
        .or((request.price_sats > 0).then_some(request.price_sats))
        .ok_or_else(|| {
            format!(
                "Request {} does not have a non-zero quoted price",
                request_id
            )
        })?;
    let bolt11 = request
        .pending_bolt11
        .as_deref()
        .ok_or_else(|| {
            format!(
                "Request {} is missing the pending Lightning invoice",
                request_id
            )
        })?
        .to_string();
    let identity = state.nostr_identity.as_ref().ok_or_else(|| {
        "Cannot publish payment-required feedback: Nostr identity unavailable".to_string()
    })?;
    let event = build_data_seller_payment_required_feedback_event(
        identity,
        request,
        quoted_price_sats,
        bolt11.as_str(),
    )?;
    state
        .queue_provider_nip90_lane_command(ProviderNip90LaneCommand::PublishEvent {
            request_id: request_id.clone(),
            role: ProviderNip90PublishRole::Feedback,
            event: Box::new(event),
        })
        .map_err(|error| format!("Cannot queue data-market payment-required feedback: {error}"))?;
    state.provider_runtime.last_result = Some(format!(
        "queued payment-required feedback for data request {}",
        request_id
    ));
    Ok(())
}

pub(crate) fn reconcile_data_seller_wallet_update(
    state: &mut RenderState,
    previous_invoice: Option<&str>,
    previous_error: Option<&str>,
) {
    if let Some(request_id) = state
        .data_seller
        .pending_payment_invoice_request_id()
        .map(str::to_string)
    {
        if state.spark_wallet.last_invoice.as_deref() != previous_invoice
            && let Some(invoice) = state.spark_wallet.last_invoice.as_deref()
        {
            if let Err(error) = state.data_seller.note_payment_invoice_created(
                request_id.as_str(),
                invoice,
                state.spark_wallet.last_invoice_created_at_epoch_seconds,
            ) {
                state
                    .data_seller
                    .note_payment_invoice_failed(request_id.as_str(), error);
                return;
            }
            if let Err(error) =
                queue_data_seller_payment_required_feedback(state, request_id.as_str())
            {
                state
                    .data_seller
                    .note_payment_invoice_failed(request_id.as_str(), error);
                return;
            }
        }

        if state.spark_wallet.last_error.as_deref() != previous_error
            && let Some(error) = state.spark_wallet.last_error.as_deref()
        {
            state
                .data_seller
                .note_payment_invoice_failed(request_id.as_str(), error);
        }
    }

    let now_epoch_seconds = current_epoch_seconds();
    let settled = state
        .data_seller
        .incoming_requests
        .iter()
        .filter(|request| {
            request.payment_state == crate::app_state::DataSellerPaymentState::AwaitingPayment
        })
        .filter(|request| request.payment_pointer.is_none())
        .filter_map(|request| {
            matched_settled_receive_payment(
                request.pending_bolt11.as_deref(),
                request.settlement_payment_hash.as_deref(),
                state.spark_wallet.recent_payments.as_slice(),
            )
            .map(|payment| {
                (
                    request.request_id.clone(),
                    payment.id.clone(),
                    payment.amount_sats,
                    payment.timestamp.max(now_epoch_seconds),
                )
            })
        })
        .collect::<Vec<_>>();
    for (request_id, payment_pointer, amount_sats, observed_at_epoch_seconds) in settled {
        if state.data_seller.note_payment_observed(
            request_id.as_str(),
            payment_pointer.as_str(),
            amount_sats,
            observed_at_epoch_seconds,
        ) {
            if let Some((request_id_owned, requester, payment_state, policy_id)) = state
                .data_seller
                .request_by_id(request_id.as_str())
                .map(|request| {
                    (
                        request.request_id.clone(),
                        request.requester.clone(),
                        request.payment_state,
                        state.data_seller.policy_id_for_request(request_id.as_str()),
                    )
                })
            {
                record_data_market_lifecycle_entry(
                    state,
                    i64::try_from(observed_at_epoch_seconds).unwrap_or(i64::MAX) * 1000,
                    "payment_settled",
                    payment_state.label(),
                    request_id_owned.clone(),
                    Some(requester),
                    policy_id,
                    Some(payment_pointer.clone()),
                    format!(
                        "Settled {} sats for data request {}.",
                        amount_sats, request_id_owned
                    ),
                );
            }
            state.provider_runtime.last_result = Some(format!(
                "seller payment settled for data request {}",
                request_id
            ));
            if let Err(error) = publish_data_seller_access_contract(
                state,
                request_id.as_str(),
                DatasetAccessContractStatus::Paid,
                None,
                None,
            ) {
                state.data_seller.last_error = Some(format!(
                    "Observed seller payment for request {} but failed to publish DS access contract: {}",
                    request_id, error
                ));
            }
        }
    }
}

pub(crate) fn apply_data_seller_publish_outcome(
    state: &mut RenderState,
    outcome: &ProviderNip90PublishOutcome,
) -> bool {
    let Some((payment_state, delivery_state)) = state
        .data_seller
        .request_by_id(outcome.request_id.as_str())
        .map(|request| (request.payment_state, request.delivery_state))
    else {
        return false;
    };
    let published = outcome.accepted_relays > 0;
    let handled = match outcome.role {
        ProviderNip90PublishRole::Feedback => {
            if payment_state != crate::app_state::DataSellerPaymentState::PublishingFeedback {
                return false;
            }
            let handled = state.data_seller.note_payment_feedback_publish_outcome(
                outcome.request_id.as_str(),
                published,
                published.then_some(outcome.event_id.as_str()),
                outcome.first_error.as_deref(),
            );
            if handled && published {
                if let Err(error) = publish_data_seller_access_contract(
                    state,
                    outcome.request_id.as_str(),
                    DatasetAccessContractStatus::PaymentRequired,
                    None,
                    None,
                ) {
                    state.data_seller.last_error = Some(format!(
                        "Published payment-required feedback for request {} but DS access contract publish failed: {}",
                        outcome.request_id, error
                    ));
                }
                if let Some((request_id_owned, requester, payment_state, policy_id, quoted_sats)) =
                    state
                        .data_seller
                        .request_by_id(outcome.request_id.as_str())
                        .map(|request| {
                            (
                                request.request_id.clone(),
                                request.requester.clone(),
                                request.payment_state,
                                state
                                    .data_seller
                                    .policy_id_for_request(outcome.request_id.as_str()),
                                request.required_price_sats.unwrap_or(request.price_sats),
                            )
                        })
                {
                    record_data_market_lifecycle_entry(
                        state,
                        current_epoch_ms(),
                        "payment_required_published",
                        payment_state.label(),
                        request_id_owned,
                        Some(requester),
                        policy_id,
                        Some(outcome.event_id.clone()),
                        format!(
                            "Published payment-required feedback for {} sats.",
                            quoted_sats
                        ),
                    );
                }
                state.provider_runtime.last_result = Some(format!(
                    "seller requested Lightning payment for data request {}",
                    outcome.request_id
                ));
            }
            handled
        }
        ProviderNip90PublishRole::Result => {
            if delivery_state != crate::app_state::DataSellerDeliveryState::PublishingResult {
                return false;
            }
            let handled = state.data_seller.note_delivery_result_publish_outcome(
                outcome.request_id.as_str(),
                published,
                published.then_some(outcome.event_id.as_str()),
                outcome.first_error.as_deref(),
            );
            if handled && published {
                let seller_pubkey = state
                    .nostr_identity
                    .as_ref()
                    .map(|identity| identity.public_key_hex.clone());
                let request = state.data_seller.request_by_id(outcome.request_id.as_str()).cloned();
                if let (Some(seller_pubkey), Some(request)) = (seller_pubkey, request) {
                    match local_dataset_access_result_projection(
                        seller_pubkey.as_str(),
                        outcome.accepted_relay_urls.first().cloned(),
                        &request,
                        outcome.event_id.as_str(),
                        current_epoch_seconds(),
                    ) {
                        Ok(projection) => {
                            if let Err(error) = record_local_access_result_projection(state, projection)
                            {
                                state.data_seller.last_error = Some(format!(
                                    "Published delivery result for request {} but failed to persist the local relay result projection: {}",
                                    outcome.request_id, error
                                ));
                            }
                        }
                        Err(error) => {
                            state.data_seller.last_error = Some(format!(
                                "Published delivery result for request {} but could not encode the relay result projection: {}",
                                outcome.request_id, error
                            ));
                        }
                    }
                }
                if let Err(error) = publish_data_seller_access_contract(
                    state,
                    outcome.request_id.as_str(),
                    DatasetAccessContractStatus::Delivered,
                    Some(outcome.event_id.as_str()),
                    None,
                ) {
                    state.data_seller.last_error = Some(format!(
                        "Published delivery result for request {} but DS access contract publish failed: {}",
                        outcome.request_id, error
                    ));
                }
                state.provider_runtime.last_result = Some(format!(
                    "seller published delivery result for data request {}",
                    outcome.request_id
                ));
            }
            handled
        }
        _ => return false,
    };
    if handled && !published {
        state.provider_runtime.last_result = Some(format!(
            "seller publish failed for data request {}",
            outcome.request_id
        ));
    }
    handled
}

pub(crate) fn publish_data_seller_asset(state: &mut RenderState) -> bool {
    state.data_seller.request_publish();
    if !state.data_seller.publish_is_armed() {
        return true;
    }

    let preview_payload = match state
        .data_seller
        .active_draft
        .last_previewed_asset_payload
        .clone()
    {
        Some(payload) => payload,
        None => {
            state.data_seller.last_error =
                Some("Publish is armed but the exact preview payload is missing.".to_string());
            return true;
        }
    };
    let request: RegisterDataAssetRequest = match serde_json::from_value(preview_payload) {
        Ok(request) => request,
        Err(error) => {
            state.data_seller.last_error = Some(format!(
                "Failed to decode the exact preview payload into RegisterDataAssetRequest: {error}"
            ));
            state.data_seller.status_line =
                "Publish blocked because the preview payload is no longer valid.".to_string();
            return true;
        }
    };
    let Some(identity) = state.nostr_identity.as_ref() else {
        state.data_seller.last_error =
            Some("Asset publish requires a Nostr identity for DS publication.".to_string());
        state.data_seller.status_line =
            "Asset publish blocked because Nostr identity is unavailable.".to_string();
        return true;
    };
    let relay_urls = state.configured_provider_relay_urls();
    let mut request = request;
    let mut optional_publication_warning = None::<String>;
    let listing_ref = match publish_dataset_listing(identity, relay_urls.as_slice(), &request.asset)
    {
        Ok(reference) => reference,
        Err(error) => {
            state.data_seller.last_error = Some(error);
            state.data_seller.status_line =
                "Asset publish blocked because DS listing publication failed.".to_string();
            return true;
        }
    };
    request.asset.nostr_publications.ds_listing = Some(listing_ref);
    if asset_public_catalog_visibility(&request.asset) {
        let classified_ref = match publish_dataset_classified_listing(
            identity,
            relay_urls.as_slice(),
            &request.asset,
        ) {
            Ok(reference) => reference,
            Err(error) => {
                state.data_seller.last_error = Some(error);
                state.data_seller.status_line =
                    "Asset publish blocked because NIP-99 catalog publication failed.".to_string();
                return true;
            }
        };
        record_nip99_classified_metadata(&mut request.asset.metadata, &classified_ref);
        if let Some((currency, _)) =
            storefront_currency_and_price(request.asset.price_hint.as_ref())
        {
            match publish_dataset_storefront_stall(
                identity,
                relay_urls.as_slice(),
                currency.as_str(),
                request.asset.created_at_ms,
            ) {
                Ok(stall_ref) => match publish_dataset_storefront_product(
                    identity,
                    relay_urls.as_slice(),
                    &request.asset,
                ) {
                    Ok(product_ref) => record_nip15_storefront_metadata(
                        &mut request.asset.metadata,
                        &stall_ref,
                        &product_ref,
                    ),
                    Err(error) => {
                        optional_publication_warning = Some(format!(
                            "Asset published without NIP-15 storefront product wrapper: {error}"
                        ));
                    }
                },
                Err(error) => {
                    optional_publication_warning = Some(format!(
                        "Asset published without NIP-15 storefront stall wrapper: {error}"
                    ));
                }
            }
        }
    }

    if let Err(error) = record_local_published_asset_projection(state, &request.asset) {
        optional_publication_warning = Some(match optional_publication_warning.take() {
            Some(existing) => format!("{existing} | local relay replica update failed: {error}"),
            None => format!(
                "Published asset to DS relays but failed to update the local relay replica: {error}"
            ),
        });
    }

    let published_asset = request.asset.clone();
    let authority_client = crate::kernel_control::remote_authority_client_for_state(state).ok();
    if authority_client.is_none() {
        state
            .data_seller
            .note_asset_published(published_asset.clone(), None);
        record_data_market_lifecycle_entry(
            state,
            published_asset.created_at_ms,
            "asset_published",
            published_asset.status.label(),
            published_asset.asset_id.clone(),
            Some(published_asset.provider_id.clone()),
            published_asset
                .default_policy
                .as_ref()
                .map(|policy| policy.policy_id.clone()),
            None,
            format!(
                "Published asset {} to DS relays without kernel authority.",
                published_asset.title
            ),
        );
        if let Some(warning) = optional_publication_warning {
            state.data_seller.status_line = format!(
                "Published asset {} to DS relays. {}",
                published_asset.asset_id, warning
            );
        } else {
            state.data_seller.status_line = format!(
                "Published asset {} to DS relays without kernel authority.",
                published_asset.asset_id
            );
        }
        sync_data_seller_nip90_profile(state);
        return true;
    }
    let client = authority_client.expect("authority client already checked");

    let expected_asset_id = request.asset.asset_id.clone();
    let response = match crate::kernel_control::run_kernel_call(client.register_data_asset(request))
    {
        Ok(response) => response,
        Err(error) => {
            if is_kernel_idempotency_conflict(error.as_str()) {
                let readback_asset = match crate::kernel_control::run_kernel_call(
                    client.get_data_asset(expected_asset_id.as_str()),
                ) {
                    Ok(asset) => asset,
                    Err(readback_error) => {
                        state
                            .data_seller
                            .note_asset_published(published_asset.clone(), None);
                        state.data_seller.status_line = format!(
                            "Published asset {} to DS relays; kernel authority replay read-back failed: {readback_error}",
                            published_asset.asset_id
                        );
                        return true;
                    }
                };
                state
                    .data_seller
                    .note_asset_published(readback_asset.clone(), None);
                state
                    .data_market
                    .note_published_asset(readback_asset, current_epoch_ms());
                state.data_buyer.sync_selection(&state.data_market);
                if let Some(asset) = state.data_seller.last_published_asset.clone() {
                    record_data_market_lifecycle_entry(
                        state,
                        asset.created_at_ms,
                        "asset_published",
                        asset.status.label(),
                        asset.asset_id.clone(),
                        Some(asset.provider_id.clone()),
                        asset
                            .default_policy
                            .as_ref()
                            .map(|policy| policy.policy_id.clone()),
                        None,
                        format!(
                            "Re-synced existing asset {} from kernel after idempotent replay.",
                            asset.title
                        ),
                    );
                }
                sync_data_seller_nip90_profile(state);
                return true;
            }
            state
                .data_seller
                .note_asset_published(published_asset.clone(), None);
            state.data_seller.status_line = format!(
                "Published asset {} to DS relays; kernel authority sync failed: {error}",
                published_asset.asset_id
            );
            return true;
        }
    };
    let asset_id = response.asset.asset_id.clone();
    let receipt_id = Some(response.receipt.receipt_id.clone());
    let readback_asset =
        match crate::kernel_control::run_kernel_call(client.get_data_asset(asset_id.as_str())) {
            Ok(asset) => asset,
            Err(error) => {
                let fallback_asset = response.asset.clone();
                state
                    .data_seller
                    .note_asset_published(response.asset, receipt_id);
                if let Some(asset) = state.data_seller.last_published_asset.clone() {
                    state
                        .data_market
                        .note_published_asset(asset, current_epoch_ms());
                    state.data_buyer.sync_selection(&state.data_market);
                }
                record_data_market_lifecycle_entry(
                    state,
                    current_epoch_ms(),
                    "asset_published",
                    "published",
                    asset_id,
                    Some(fallback_asset.provider_id.clone()),
                    fallback_asset
                        .default_policy
                        .as_ref()
                        .map(|policy| policy.policy_id.clone()),
                    state.data_seller.last_publish_receipt_id.clone(),
                    format!("Published asset {} from seller lane.", fallback_asset.title),
                );
                sync_data_seller_nip90_profile(state);
                state.data_seller.status_line = format!(
                    "Published asset {} to DS relays; kernel read-back failed: {error}",
                    fallback_asset.asset_id
                );
                return true;
            }
        };

    state
        .data_seller
        .note_asset_published(readback_asset.clone(), receipt_id);
    state
        .data_market
        .note_published_asset(readback_asset, current_epoch_ms());
    state.data_buyer.sync_selection(&state.data_market);
    if let Some(asset) = state.data_seller.last_published_asset.clone() {
        record_data_market_lifecycle_entry(
            state,
            asset.created_at_ms,
            "asset_published",
            asset.status.label(),
            asset.asset_id.clone(),
            Some(asset.provider_id.clone()),
            asset
                .default_policy
                .as_ref()
                .map(|policy| policy.policy_id.clone()),
            state.data_seller.last_publish_receipt_id.clone(),
            format!("Published asset {} from seller lane.", asset.title),
        );
    }
    if let Some(warning) = optional_publication_warning {
        state.data_seller.status_line = format!("{} {}", state.data_seller.status_line, warning);
    }
    sync_data_seller_nip90_profile(state);
    true
}

pub(crate) fn publish_data_seller_grant(state: &mut RenderState) -> bool {
    state.data_seller.request_publish_grant();
    if !state.data_seller.grant_publish_is_armed() {
        return true;
    }

    let preview_payload = match state
        .data_seller
        .active_draft
        .last_previewed_grant_payload
        .clone()
    {
        Some(payload) => payload,
        None => {
            state.data_seller.last_error = Some(
                "Grant publish is armed but the exact preview payload is missing.".to_string(),
            );
            return true;
        }
    };
    let request: CreateAccessGrantRequest = match serde_json::from_value(preview_payload) {
        Ok(request) => request,
        Err(error) => {
            state.data_seller.last_error = Some(format!(
                "Failed to decode the exact grant preview payload into CreateAccessGrantRequest: {error}"
            ));
            state.data_seller.status_line =
                "Grant publish blocked because the preview payload is no longer valid.".to_string();
            return true;
        }
    };
    hydrate_data_seller_inventory_from_relay_replica(state);
    let Some(identity) = state.nostr_identity.as_ref() else {
        state.data_seller.last_error =
            Some("Grant publish requires a Nostr identity for DS publication.".to_string());
        state.data_seller.status_line =
            "Grant publish blocked because Nostr identity is unavailable.".to_string();
        return true;
    };
    let relay_urls = state.configured_provider_relay_urls();
    let mut request = request;
    let mut optional_publication_warning = None::<String>;
    let authority_client = crate::kernel_control::remote_authority_client_for_state(state).ok();
    let asset_for_offer = state
        .data_seller
        .published_assets_for_display()
        .into_iter()
        .find(|asset| asset.asset_id == request.grant.asset_id)
        .cloned()
        .or_else(|| {
            authority_client.as_ref().and_then(|client| {
                crate::kernel_control::run_kernel_call(
                    client.get_data_asset(request.grant.asset_id.as_str()),
                )
                .ok()
            })
        });
    let Some(asset_for_offer) = asset_for_offer else {
        state.data_seller.last_error = Some(
            "Grant publish requires the seller asset to be present in local relay inventory or readable from authority."
                .to_string(),
        );
        state.data_seller.status_line =
            "Grant publish blocked because the published asset inventory is unavailable."
                .to_string();
        return true;
    };
    let offer_ref = match publish_dataset_offer(
        identity,
        relay_urls.as_slice(),
        &request.grant,
        &asset_for_offer,
    ) {
        Ok(reference) => reference,
        Err(error) => {
            state.data_seller.last_error = Some(error);
            state.data_seller.status_line =
                "Grant publish blocked because DS offer publication failed.".to_string();
            return true;
        }
    };
    request.grant.nostr_publications.ds_offer = Some(offer_ref);
    if grant_public_catalog_visibility(&request.grant) && request.grant.consumer_id.is_none() {
        let classified_ref = match publish_dataset_offer_classified_listing(
            identity,
            relay_urls.as_slice(),
            &request.grant,
            &asset_for_offer,
        ) {
            Ok(reference) => reference,
            Err(error) => {
                state.data_seller.last_error = Some(error);
                state.data_seller.status_line =
                    "Grant publish blocked because NIP-99 catalog publication failed.".to_string();
                return true;
            }
        };
        record_nip99_classified_metadata(&mut request.grant.metadata, &classified_ref);
        if let Some((currency, _)) = storefront_currency_and_price(
            request
                .grant
                .offer_price
                .as_ref()
                .or(asset_for_offer.price_hint.as_ref()),
        ) {
            match publish_dataset_storefront_stall(
                identity,
                relay_urls.as_slice(),
                currency.as_str(),
                request.grant.created_at_ms,
            ) {
                Ok(stall_ref) => match publish_dataset_offer_storefront_product(
                    identity,
                    relay_urls.as_slice(),
                    &request.grant,
                    &asset_for_offer,
                ) {
                    Ok(product_ref) => record_nip15_storefront_metadata(
                        &mut request.grant.metadata,
                        &stall_ref,
                        &product_ref,
                    ),
                    Err(error) => {
                        optional_publication_warning = Some(format!(
                            "Grant published without NIP-15 storefront product wrapper: {error}"
                        ));
                    }
                },
                Err(error) => {
                    optional_publication_warning = Some(format!(
                        "Grant published without NIP-15 storefront stall wrapper: {error}"
                    ));
                }
            }
        }
    }

    if let Err(error) =
        record_local_published_grant_projection(state, &asset_for_offer, &request.grant)
    {
        optional_publication_warning = Some(match optional_publication_warning.take() {
            Some(existing) => format!("{existing} | local relay replica update failed: {error}"),
            None => format!(
                "Published grant to DS relays but failed to update the local relay replica: {error}"
            ),
        });
    }

    let published_grant = request.grant.clone();
    if authority_client.is_none() {
        state
            .data_seller
            .note_grant_published(published_grant.clone(), None);
        record_data_market_lifecycle_entry(
            state,
            published_grant.created_at_ms,
            "grant_published",
            published_grant.status.label(),
            published_grant.grant_id.clone(),
            published_grant.consumer_id.clone(),
            Some(published_grant.permission_policy.policy_id.clone()),
            None,
            format!(
                "Published grant {} to DS relays without kernel authority.",
                published_grant.grant_id
            ),
        );
        if let Some(warning) = optional_publication_warning {
            state.data_seller.status_line = format!(
                "Published grant {} to DS relays. {}",
                published_grant.grant_id, warning
            );
        } else {
            state.data_seller.status_line = format!(
                "Published grant {} to DS relays without kernel authority.",
                published_grant.grant_id
            );
        }
        sync_data_seller_nip90_profile(state);
        return true;
    }
    let client = authority_client.expect("authority client already checked");

    let expected_grant_id = request.grant.grant_id.clone();
    let response = match crate::kernel_control::run_kernel_call(client.create_access_grant(request))
    {
        Ok(response) => response,
        Err(error) => {
            if is_kernel_idempotency_conflict(error.as_str()) {
                let readback_grant = match crate::kernel_control::run_kernel_call(
                    client.get_access_grant(expected_grant_id.as_str()),
                ) {
                    Ok(grant) => grant,
                    Err(readback_error) => {
                        state
                            .data_seller
                            .note_grant_published(published_grant.clone(), None);
                        state.data_seller.status_line = format!(
                            "Published grant {} to DS relays; kernel authority replay read-back failed: {readback_error}",
                            published_grant.grant_id
                        );
                        return true;
                    }
                };
                state
                    .data_seller
                    .note_grant_published(readback_grant.clone(), None);
                state
                    .data_market
                    .note_published_grant(readback_grant, current_epoch_ms());
                state.data_buyer.sync_selection(&state.data_market);
                if let Some(grant) = state.data_seller.last_published_grant.clone() {
                    record_data_market_lifecycle_entry(
                        state,
                        grant.created_at_ms,
                        "grant_published",
                        grant.status.label(),
                        grant.grant_id.clone(),
                        grant.consumer_id.clone(),
                        Some(grant.permission_policy.policy_id.clone()),
                        None,
                        format!(
                            "Re-synced existing grant {} from kernel after idempotent replay.",
                            grant.grant_id
                        ),
                    );
                }
                sync_data_seller_nip90_profile(state);
                return true;
            }
            state
                .data_seller
                .note_grant_published(published_grant.clone(), None);
            state.data_seller.status_line = format!(
                "Published grant {} to DS relays; kernel authority sync failed: {error}",
                published_grant.grant_id
            );
            return true;
        }
    };
    let grant_id = response.grant.grant_id.clone();
    let receipt_id = Some(response.receipt.receipt_id.clone());
    let readback_grant =
        match crate::kernel_control::run_kernel_call(client.get_access_grant(grant_id.as_str())) {
            Ok(grant) => grant,
            Err(error) => {
                let fallback_grant = response.grant.clone();
                state
                    .data_seller
                    .note_grant_published(response.grant, receipt_id);
                if let Some(grant) = state.data_seller.last_published_grant.clone() {
                    state
                        .data_market
                        .note_published_grant(grant, current_epoch_ms());
                    state.data_buyer.sync_selection(&state.data_market);
                }
                record_data_market_lifecycle_entry(
                    state,
                    current_epoch_ms(),
                    "grant_published",
                    fallback_grant.status.label(),
                    grant_id,
                    fallback_grant.consumer_id.clone(),
                    Some(fallback_grant.permission_policy.policy_id.clone()),
                    state.data_seller.last_grant_publish_receipt_id.clone(),
                    format!(
                        "Published grant for asset {} with expiry {}.",
                        fallback_grant.asset_id, fallback_grant.expires_at_ms
                    ),
                );
                sync_data_seller_nip90_profile(state);
                state.data_seller.status_line = format!(
                    "Published grant {} to DS relays; kernel read-back failed: {error}",
                    fallback_grant.grant_id
                );
                return true;
            }
        };

    state
        .data_seller
        .note_grant_published(readback_grant.clone(), receipt_id);
    state
        .data_market
        .note_published_grant(readback_grant, current_epoch_ms());
    state.data_buyer.sync_selection(&state.data_market);
    if let Some(grant) = state.data_seller.last_published_grant.clone() {
        record_data_market_lifecycle_entry(
            state,
            grant.created_at_ms,
            "grant_published",
            grant.status.label(),
            grant.grant_id.clone(),
            grant.consumer_id.clone(),
            Some(grant.permission_policy.policy_id.clone()),
            state.data_seller.last_grant_publish_receipt_id.clone(),
            format!(
                "Published grant for asset {} with expiry {}.",
                grant.asset_id, grant.expires_at_ms
            ),
        );
    }
    if let Some(warning) = optional_publication_warning {
        state.data_seller.status_line = format!("{} {}", state.data_seller.status_line, warning);
    }
    sync_data_seller_nip90_profile(state);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::{
        DataMarketPaneState, DataSellerDeliveryDraft, DataSellerDeliveryState,
        DataSellerPaymentState, DataSellerRequestEvaluationDisposition,
        DataSellerRevocationState,
    };
    use nostr::nip90::{DataVendingFeedback, DataVendingResult};
    use openagents_kernel_core::data::PermissionPolicy;

    const SELLER_PUBKEY: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const BUYER_PUBKEY: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const REQUEST_EVENT_ID: &str =
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn fixture_policy() -> PermissionPolicy {
        PermissionPolicy {
            policy_id: "seller-license-v1".to_string(),
            export_allowed: true,
            derived_outputs_allowed: true,
            ..Default::default()
        }
    }

    fn fixture_asset() -> DataAsset {
        DataAsset {
            asset_id: "data_asset.example.corpus.001".to_string(),
            provider_id: SELLER_PUBKEY.to_string(),
            asset_kind: "corpus".to_string(),
            title: "Example corpus".to_string(),
            description: Some("Example dataset for DS seller publication.".to_string()),
            content_digest: Some(format!("sha256:{DIGEST}")),
            provenance_ref: None,
            default_policy: Some(fixture_policy()),
            price_hint: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(5),
            }),
            created_at_ms: 1_774_080_000_000,
            status: Default::default(),
            nostr_publications: Default::default(),
            metadata: json!({
                "packaging_total_bytes": 2048,
                "packaging_file_count": 7,
                "delivery_modes": ["encrypted_pointer", "inline_preview"],
            }),
        }
    }

    fn fixture_grant(asset_id: &str) -> AccessGrant {
        AccessGrant {
            grant_id: "grant.example.corpus.001".to_string(),
            asset_id: asset_id.to_string(),
            provider_id: SELLER_PUBKEY.to_string(),
            consumer_id: Some(BUYER_PUBKEY.to_string()),
            permission_policy: fixture_policy(),
            offer_price: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(5),
            }),
            warranty_window_ms: Some(60_000),
            created_at_ms: 1_774_080_010_000,
            expires_at_ms: 1_774_080_130_000,
            accepted_at_ms: None,
            status: AccessGrantStatus::Offered,
            nostr_publications: Default::default(),
            metadata: json!({
                "delivery_modes": ["encrypted_pointer", "giftwrap"],
            }),
        }
    }

    fn fixture_request() -> DataSellerIncomingRequest {
        DataSellerIncomingRequest {
            request_id: REQUEST_EVENT_ID.to_string(),
            requester: BUYER_PUBKEY.to_string(),
            requested_consumer_id: Some(BUYER_PUBKEY.to_string()),
            source_relay_url: Some("wss://relay.example".to_string()),
            request_kind: 5960,
            profile_id: Some(nostr::nip90::OPENAGENTS_DATA_VENDING_PROFILE.to_string()),
            asset_ref: Some(format!(
                "30404:{SELLER_PUBKEY}:data_asset.example.corpus.001"
            )),
            permission_scopes: vec!["read.context".to_string()],
            delivery_mode: Some("encrypted_pointer".to_string()),
            preview_posture: Some("metadata_only".to_string()),
            price_sats: 5,
            ttl_seconds: 120,
            created_at_epoch_seconds: Some(1_774_080_020),
            expires_at_epoch_seconds: Some(1_774_080_140),
            encrypted: false,
            preview_only: false,
            validation_label: "valid".to_string(),
            content_preview: Some("Need access to the dataset.".to_string()),
            matched_asset_id: Some("data_asset.example.corpus.001".to_string()),
            matched_grant_id: Some("grant.example.corpus.001".to_string()),
            matched_listing_coordinate: Some(format!(
                "30404:{SELLER_PUBKEY}:data_asset.example.corpus.001"
            )),
            matched_offer_coordinate: Some(format!(
                "30406:{SELLER_PUBKEY}:grant.example.corpus.001"
            )),
            asset_match_posture: Some("ds_offer".to_string()),
            required_price_sats: Some(5),
            evaluation_disposition: DataSellerRequestEvaluationDisposition::ReadyForPaymentQuote,
            evaluation_summary: "Matched DS listing and offer.".to_string(),
            payment_state: DataSellerPaymentState::AwaitingPayment,
            payment_feedback_event_id: None,
            pending_bolt11: Some("lnbc50n1pexample".to_string()),
            pending_bolt11_created_at_epoch_seconds: Some(1_774_080_021),
            settlement_payment_hash: None,
            payment_pointer: Some("spark-payment-001".to_string()),
            payment_observed_at_epoch_seconds: Some(1_774_080_025),
            payment_amount_sats: Some(5),
            payment_error: None,
            delivery_state: DataSellerDeliveryState::DraftReady,
            delivery_draft: DataSellerDeliveryDraft {
                delivery_ref: Some("oa://deliveries/example-001".to_string()),
                delivery_digest: Some("sha256:delivery-example-001".to_string()),
                manifest_refs: vec!["oa://deliveries/example-001/manifest".to_string()],
                bundle_size_bytes: Some(2048),
                expires_in_hours: Some(1),
                ..Default::default()
            },
            delivery_bundle_id: Some("delivery_bundle.example.001".to_string()),
            delivery_receipt_id: None,
            delivery_result_event_id: None,
            delivery_error: None,
            revocation_state: DataSellerRevocationState::Idle,
            revocation_id: None,
            revocation_receipt_id: None,
            revocation_reason_code: None,
            revocation_recorded_at_ms: None,
            revocation_error: None,
        }
    }

    fn fixture_delivery() -> DeliveryBundle {
        DeliveryBundle {
            delivery_bundle_id: "delivery_bundle.example.001".to_string(),
            asset_id: "data_asset.example.corpus.001".to_string(),
            grant_id: "grant.example.corpus.001".to_string(),
            provider_id: SELLER_PUBKEY.to_string(),
            consumer_id: BUYER_PUBKEY.to_string(),
            created_at_ms: 1_774_080_030_000,
            delivery_ref: "oa://deliveries/example-001".to_string(),
            delivery_digest: Some("sha256:delivery-example-001".to_string()),
            bundle_size_bytes: Some(2048),
            manifest_refs: vec!["oa://deliveries/example-001/manifest".to_string()],
            expires_at_ms: Some(1_774_083_630_000),
            status: DeliveryBundleStatus::Issued,
            metadata: json!({}),
        }
    }

    fn fixture_contract_projection(
        status: &str,
    ) -> crate::app_state::RelayDatasetAccessContractProjection {
        crate::app_state::RelayDatasetAccessContractProjection {
            coordinate: format!("30407:{SELLER_PUBKEY}:{REQUEST_EVENT_ID}"),
            seller_pubkey: SELLER_PUBKEY.to_string(),
            buyer_pubkey: BUYER_PUBKEY.to_string(),
            relay_url: Some("wss://relay.example".to_string()),
            listing_coordinate: format!("30404:{SELLER_PUBKEY}:data_asset.example.corpus.001"),
            offer_coordinate: Some(format!("30406:{SELLER_PUBKEY}:grant.example.corpus.001")),
            request_event_id: REQUEST_EVENT_ID.to_string(),
            result_event_id: Some("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string()),
            status: status.to_string(),
            payment_method: Some("ln".to_string()),
            amount_msats: Some(5_000),
            bolt11: Some("lnbc50n1pexample".to_string()),
            payment_hash: Some("dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string()),
            payment_evidence_event_ids: Vec::new(),
            delivery_mode: Some("encrypted_pointer".to_string()),
            delivery_ref: Some("oa://deliveries/example-001".to_string()),
            delivery_mime_type: None,
            delivery_digest: Some(DIGEST.to_string()),
            created_at_seconds: 1_774_080_040,
            expires_at_seconds: Some(1_774_083_640),
            reason_code: None,
            linked_asset_id: Some("data_asset.example.corpus.001".to_string()),
            linked_grant_id: Some("grant.example.corpus.001".to_string()),
        }
    }

    fn fixture_result_projection() -> crate::app_state::RelayDatasetAccessResultProjection {
        crate::app_state::RelayDatasetAccessResultProjection {
            event_id: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
            seller_pubkey: SELLER_PUBKEY.to_string(),
            buyer_pubkey: BUYER_PUBKEY.to_string(),
            relay_url: Some("wss://relay.example".to_string()),
            request_event_id: REQUEST_EVENT_ID.to_string(),
            listing_coordinate: format!("30404:{SELLER_PUBKEY}:data_asset.example.corpus.001"),
            offer_coordinate: Some(format!("30406:{SELLER_PUBKEY}:grant.example.corpus.001")),
            asset_ref: format!("30404:{SELLER_PUBKEY}:data_asset.example.corpus.001"),
            asset_id: Some("data_asset.example.corpus.001".to_string()),
            grant_id: Some("grant.example.corpus.001".to_string()),
            delivery_bundle_id: "delivery_bundle.example.001".to_string(),
            delivery_mode: "encrypted_pointer".to_string(),
            preview_posture: "metadata_only".to_string(),
            delivery_ref: Some("oa://deliveries/example-001".to_string()),
            delivery_digest: Some(format!("sha256:{DIGEST}")),
            amount_msats: Some(5_000),
            bolt11: Some("lnbc50n1pexample".to_string()),
            payment_hash: Some("dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string()),
            created_at_seconds: 1_774_080_041,
            linked_asset_id: Some("data_asset.example.corpus.001".to_string()),
            linked_grant_id: Some("grant.example.corpus.001".to_string()),
        }
    }

    fn fixture_settlement_match() -> crate::app_state::RelayDatasetSettlementMatchProjection {
        crate::app_state::RelayDatasetSettlementMatchProjection {
            payment_pointer: "spark-payment-001".to_string(),
            payment_hash: "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string(),
            direction: "receive".to_string(),
            status: "settled".to_string(),
            amount_sats: 5,
            observed_at_seconds: 1_774_080_042,
            contract_coordinate: Some(format!("30407:{SELLER_PUBKEY}:{REQUEST_EVENT_ID}")),
            request_event_id: Some(REQUEST_EVENT_ID.to_string()),
            result_event_id: Some(
                "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
            ),
        }
    }

    #[test]
    fn detects_kernel_idempotency_conflicts() {
        assert!(is_kernel_idempotency_conflict(
            "kernel authority call failed: status=409 error=kernel_error reason=kernel_idempotency_conflict"
        ));
        assert!(!is_kernel_idempotency_conflict(
            "kernel authority call failed: status=500 error=kernel_error reason=storage_unavailable"
        ));
    }

    #[test]
    fn strips_sha256_prefix_for_dataset_publication() {
        assert_eq!(
            strip_sha256_prefix(Some(&format!("sha256:{DIGEST}"))).unwrap(),
            DIGEST
        );
        assert_eq!(strip_sha256_prefix(Some(DIGEST)).unwrap(), DIGEST);
        assert!(strip_sha256_prefix(Some("not-a-digest")).is_err());
    }

    #[test]
    fn builds_dataset_listing_from_asset_metadata() {
        let asset = fixture_asset();
        let (listing, publication_ref) =
            build_dataset_listing(&asset, SELLER_PUBKEY).expect("dataset listing");

        assert_eq!(listing.identifier, "data_asset.example.corpus.001");
        assert_eq!(listing.title, "Example corpus");
        assert_eq!(listing.summary.as_deref(), asset.description.as_deref());
        assert_eq!(listing.digest, DIGEST);
        assert_eq!(listing.published_at, Some(1_774_080_000));
        assert_eq!(listing.dataset_kind.as_deref(), Some("corpus"));
        assert_eq!(listing.size_bytes, Some(2048));
        assert_eq!(listing.records, Some(7));
        assert_eq!(listing.license.as_deref(), Some("seller-license-v1"));
        assert_eq!(listing.access.as_deref(), Some("paid"));
        assert_eq!(listing.delivery_modes, vec!["nip90".to_string()]);
        assert_eq!(
            listing.topics,
            vec!["dataset".to_string(), "corpus".to_string()]
        );
        assert_eq!(
            publication_ref.coordinate.as_deref(),
            Some(
                "30404:1111111111111111111111111111111111111111111111111111111111111111:data_asset.example.corpus.001"
            )
        );
        assert!(publication_ref.event_id.is_none());
        assert!(publication_ref.relay_url.is_none());
    }

    #[test]
    fn builds_dataset_offer_from_grant_and_listing_reference() {
        let mut asset = fixture_asset();
        asset.nostr_publications.ds_listing = Some(NostrPublicationRef {
            coordinate: Some(format!("30404:{SELLER_PUBKEY}:{}", asset.asset_id)),
            event_id: Some(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            ),
            relay_url: Some("wss://relay.example".to_string()),
        });
        let grant = fixture_grant(asset.asset_id.as_str());
        let (offer, publication_ref) =
            build_dataset_offer(&grant, &asset, SELLER_PUBKEY).expect("dataset offer");

        assert_eq!(offer.identifier, "grant.example.corpus.001");
        assert_eq!(
            offer.listing_ref.coordinate.to_string(),
            format!("30404:{SELLER_PUBKEY}:{}", asset.asset_id)
        );
        assert_eq!(offer.status, DatasetOfferStatus::Active);
        assert_eq!(offer.policy.as_deref(), Some("targeted_request"));
        assert_eq!(offer.license.as_deref(), Some("seller-license-v1"));
        assert_eq!(offer.expiration.as_deref(), Some("1774080130"));
        assert_eq!(
            offer.delivery_modes,
            vec!["nip90".to_string(), "giftwrap".to_string()]
        );
        assert_eq!(offer.payment_methods, vec![PaymentMethod::new("ln")]);
        assert_eq!(offer.targeted_buyers.len(), 1);
        assert_eq!(offer.targeted_buyers[0].pubkey, BUYER_PUBKEY);
        let price = offer.price.expect("offer price");
        assert_eq!(price.amount, "5");
        assert_eq!(price.currency, "SAT");
        assert_eq!(
            publication_ref.coordinate.as_deref(),
            Some(
                "30406:1111111111111111111111111111111111111111111111111111111111111111:grant.example.corpus.001"
            )
        );
        assert!(publication_ref.event_id.is_none());
        assert!(publication_ref.relay_url.is_none());
    }

    #[test]
    fn local_relay_listing_projection_keeps_local_wrapper_refs() {
        let mut asset = fixture_asset();
        asset.nostr_publications.ds_listing = Some(NostrPublicationRef {
            coordinate: Some(format!("30404:{SELLER_PUBKEY}:{}", asset.asset_id)),
            event_id: Some(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            ),
            relay_url: Some("wss://relay.example".to_string()),
        });
        record_nip99_classified_metadata(
            &mut asset.metadata,
            &NostrPublicationRef {
                coordinate: Some(format!("30402:{SELLER_PUBKEY}:catalog.{}", asset.asset_id)),
                event_id: Some(
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                ),
                relay_url: Some("wss://relay.example".to_string()),
            },
        );
        record_nip15_storefront_metadata(
            &mut asset.metadata,
            &NostrPublicationRef {
                coordinate: Some(format!("30017:{SELLER_PUBKEY}:datasets.sat")),
                event_id: Some(
                    "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
                ),
                relay_url: Some("wss://relay.example".to_string()),
            },
            &NostrPublicationRef {
                coordinate: Some(format!(
                    "30018:{SELLER_PUBKEY}:storefront.{}",
                    asset.asset_id
                )),
                event_id: Some(
                    "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string(),
                ),
                relay_url: Some("wss://relay.example".to_string()),
            },
        );

        let projection =
            local_dataset_listing_projection(&asset).expect("local relay listing projection");

        assert_eq!(
            projection.coordinate,
            format!("30404:{SELLER_PUBKEY}:{}", asset.asset_id)
        );
        assert_eq!(
            projection.linked_asset_id.as_deref(),
            Some(asset.asset_id.as_str())
        );
        assert_eq!(
            projection.classified_coordinate.as_deref(),
            Some(format!("30402:{SELLER_PUBKEY}:catalog.{}", asset.asset_id).as_str())
        );
        assert_eq!(
            projection.storefront_product_coordinate.as_deref(),
            Some(format!("30018:{SELLER_PUBKEY}:storefront.{}", asset.asset_id).as_str())
        );
        assert_eq!(
            projection.storefront_product_title.as_deref(),
            Some("Example corpus")
        );
    }

    #[test]
    fn relay_projected_offer_round_trips_into_local_grant_inventory() {
        let mut asset = fixture_asset();
        asset.nostr_publications.ds_listing = Some(NostrPublicationRef {
            coordinate: Some(format!("30404:{SELLER_PUBKEY}:{}", asset.asset_id)),
            event_id: None,
            relay_url: Some("wss://relay.example".to_string()),
        });
        let mut grant = fixture_grant(asset.asset_id.as_str());
        grant.nostr_publications.ds_offer = Some(NostrPublicationRef {
            coordinate: Some(format!("30406:{SELLER_PUBKEY}:{}", grant.grant_id)),
            event_id: None,
            relay_url: Some("wss://relay.example".to_string()),
        });

        let projection =
            local_dataset_offer_projection(&grant, &asset).expect("local relay offer projection");
        let projected_grant = relay_projected_grant_from_offer(&projection);

        assert_eq!(projected_grant.grant_id, grant.grant_id);
        assert_eq!(projected_grant.asset_id, asset.asset_id);
        assert_eq!(projected_grant.provider_id, SELLER_PUBKEY);
        assert_eq!(projected_grant.consumer_id.as_deref(), Some(BUYER_PUBKEY));
        assert_eq!(
            projected_grant
                .offer_price
                .as_ref()
                .map(|money| money.amount.clone()),
            Some(MoneyAmount::AmountSats(5))
        );
        assert_eq!(
            projected_grant
                .nostr_publications
                .ds_offer
                .as_ref()
                .and_then(|reference| reference.coordinate.as_deref()),
            Some(format!("30406:{SELLER_PUBKEY}:{}", grant.grant_id).as_str())
        );
    }

    #[test]
    fn builds_nip99_classified_listing_from_public_asset() {
        let mut asset = fixture_asset();
        asset.metadata["visibility_posture"] = json!("public_catalog");
        let (classified, publication_ref) =
            build_dataset_classified_listing(&asset, SELLER_PUBKEY).expect("classified listing");

        assert_eq!(
            classified.identifier,
            "catalog.data_asset.example.corpus.001"
        );
        assert_eq!(classified.title, "Example corpus");
        assert_eq!(
            classified.summary.as_deref(),
            Some("Example dataset for DS seller publication.")
        );
        assert_eq!(
            classified.price.as_ref().map(|price| price.amount.as_str()),
            Some("5")
        );
        assert!(classified.tags.iter().any(|tag| tag == "dataset"));
        assert!(classified.tags.iter().any(|tag| tag == "nip-ds"));
        assert_eq!(
            classified.address_refs,
            vec![format!(
                "30404:{SELLER_PUBKEY}:data_asset.example.corpus.001"
            )]
        );
        assert_eq!(
            publication_ref.coordinate.as_deref(),
            Some(
                "30402:1111111111111111111111111111111111111111111111111111111111111111:catalog.data_asset.example.corpus.001"
            )
        );
    }

    #[test]
    fn builds_nip99_classified_offer_from_open_grant() {
        let mut asset = fixture_asset();
        asset.nostr_publications.ds_listing = Some(NostrPublicationRef {
            coordinate: Some(format!("30404:{SELLER_PUBKEY}:{}", asset.asset_id)),
            event_id: None,
            relay_url: None,
        });
        let mut grant = fixture_grant(asset.asset_id.as_str());
        grant.consumer_id = None;
        grant.metadata["visibility_posture"] = json!("public_catalog");
        grant.nostr_publications.ds_offer = Some(NostrPublicationRef {
            coordinate: Some(format!("30406:{SELLER_PUBKEY}:{}", grant.grant_id)),
            event_id: None,
            relay_url: None,
        });

        let (classified, publication_ref) =
            build_dataset_offer_classified_listing(&grant, &asset, SELLER_PUBKEY)
                .expect("classified offer listing");

        assert_eq!(
            classified.identifier,
            "catalog-offer.grant.example.corpus.001"
        );
        assert_eq!(classified.title, "Example corpus access");
        assert_eq!(classified.status, Some(ListingStatus::Active));
        assert_eq!(
            classified.address_refs,
            vec![
                format!("30404:{SELLER_PUBKEY}:data_asset.example.corpus.001"),
                format!("30406:{SELLER_PUBKEY}:grant.example.corpus.001"),
            ]
        );
        assert_eq!(
            publication_ref.coordinate.as_deref(),
            Some(
                "30402:1111111111111111111111111111111111111111111111111111111111111111:catalog-offer.grant.example.corpus.001"
            )
        );
    }

    #[test]
    fn builds_nip15_storefront_stall_for_asset_currency() {
        let (stall, publication_ref) =
            build_dataset_storefront_stall(SELLER_PUBKEY, "SAT").expect("storefront stall");

        assert_eq!(stall.identifier, "datasets.sat");
        assert_eq!(stall.name, "OpenAgents datasets (SAT)");
        assert_eq!(stall.currency, "SAT");
        assert_eq!(
            publication_ref.coordinate.as_deref(),
            Some(
                "30017:1111111111111111111111111111111111111111111111111111111111111111:datasets.sat"
            )
        );
    }

    #[test]
    fn builds_nip15_storefront_product_from_public_asset() {
        let mut asset = fixture_asset();
        asset.metadata["visibility_posture"] = json!("public_catalog");
        let (product, publication_ref) =
            build_dataset_storefront_product(&asset, SELLER_PUBKEY).expect("storefront product");

        assert_eq!(
            product.identifier,
            "storefront.data_asset.example.corpus.001"
        );
        assert_eq!(product.stall_id, "datasets.sat");
        assert_eq!(product.name, "Example corpus");
        assert_eq!(product.currency, "SAT");
        assert_eq!(product.price, 5.0);
        assert!(product.tags.iter().any(|tag| tag == "dataset"));
        assert!(product.tags.iter().any(|tag| tag == "nip-ds"));
        assert_eq!(
            product.address_refs,
            vec![format!(
                "30404:{SELLER_PUBKEY}:data_asset.example.corpus.001"
            )]
        );
        assert_eq!(
            publication_ref.coordinate.as_deref(),
            Some(
                "30018:1111111111111111111111111111111111111111111111111111111111111111:storefront.data_asset.example.corpus.001"
            )
        );
    }

    #[test]
    fn builds_nip15_storefront_offer_product_from_open_grant() {
        let mut asset = fixture_asset();
        asset.nostr_publications.ds_listing = Some(NostrPublicationRef {
            coordinate: Some(format!("30404:{SELLER_PUBKEY}:{}", asset.asset_id)),
            event_id: None,
            relay_url: None,
        });
        let mut grant = fixture_grant(asset.asset_id.as_str());
        grant.consumer_id = None;
        grant.metadata["visibility_posture"] = json!("public_catalog");
        grant.nostr_publications.ds_offer = Some(NostrPublicationRef {
            coordinate: Some(format!("30406:{SELLER_PUBKEY}:{}", grant.grant_id)),
            event_id: None,
            relay_url: None,
        });

        let (product, publication_ref) =
            build_dataset_offer_storefront_product(&grant, &asset, SELLER_PUBKEY)
                .expect("storefront offer product");

        assert_eq!(
            product.identifier,
            "storefront-offer.grant.example.corpus.001"
        );
        assert_eq!(product.stall_id, "datasets.sat");
        assert_eq!(product.name, "Example corpus access");
        assert_eq!(product.currency, "SAT");
        assert_eq!(product.price, 5.0);
        assert!(product.tags.iter().any(|tag| tag == "access-offer"));
        assert_eq!(
            product.address_refs,
            vec![
                format!("30404:{SELLER_PUBKEY}:data_asset.example.corpus.001"),
                format!("30406:{SELLER_PUBKEY}:grant.example.corpus.001"),
            ]
        );
        assert_eq!(
            publication_ref.coordinate.as_deref(),
            Some(
                "30018:1111111111111111111111111111111111111111111111111111111111111111:storefront-offer.grant.example.corpus.001"
            )
        );
    }

    #[test]
    fn payment_feedback_event_carries_ds_refs() {
        let identity = nostr::regenerate_identity().expect("identity");
        let event = build_data_seller_payment_required_feedback_event(
            &identity,
            &fixture_request(),
            5,
            "lnbc50n1pexample",
        )
        .expect("payment feedback event");
        let feedback = DataVendingFeedback::from_event(&event).expect("parse feedback");

        assert_eq!(feedback.status, JobStatus::PaymentRequired);
        assert_eq!(
            feedback
                .listing_ref
                .as_ref()
                .map(|reference| reference.coordinate.to_string())
                .as_deref(),
            Some(
                "30404:1111111111111111111111111111111111111111111111111111111111111111:data_asset.example.corpus.001"
            )
        );
        assert_eq!(
            feedback
                .offer_ref
                .as_ref()
                .map(|reference| reference.coordinate.to_string())
                .as_deref(),
            Some(
                "30406:1111111111111111111111111111111111111111111111111111111111111111:grant.example.corpus.001"
            )
        );
        assert_eq!(
            feedback.asset_id.as_deref(),
            Some("data_asset.example.corpus.001")
        );
        assert_eq!(
            feedback.grant_id.as_deref(),
            Some("grant.example.corpus.001")
        );
    }

    #[test]
    fn delivery_result_event_carries_ds_refs_and_digest() {
        let identity = nostr::regenerate_identity().expect("identity");
        let event = build_data_seller_delivery_result_event(
            &identity,
            &fixture_request(),
            &fixture_delivery(),
        )
        .expect("delivery result event");
        let result = DataVendingResult::from_event(&event).expect("parse result");

        assert_eq!(
            result
                .listing_ref
                .as_ref()
                .map(|reference| reference.coordinate.to_string())
                .as_deref(),
            Some(
                "30404:1111111111111111111111111111111111111111111111111111111111111111:data_asset.example.corpus.001"
            )
        );
        assert_eq!(
            result
                .offer_ref
                .as_ref()
                .map(|reference| reference.coordinate.to_string())
                .as_deref(),
            Some(
                "30406:1111111111111111111111111111111111111111111111111111111111111111:grant.example.corpus.001"
            )
        );
        assert_eq!(
            result.asset_id.as_deref(),
            Some("data_asset.example.corpus.001")
        );
        assert_eq!(result.grant_id.as_deref(), Some("grant.example.corpus.001"));
        assert_eq!(
            result.delivery_digest.as_deref(),
            Some("sha256:delivery-example-001")
        );
    }

    #[test]
    fn revocation_feedback_event_carries_ds_refs_and_reason() {
        let identity = nostr::regenerate_identity().expect("identity");
        let event = build_data_seller_revocation_feedback_event(
            &identity,
            &fixture_request(),
            &RevocationReceipt {
                revocation_id: "revocation.example.001".to_string(),
                asset_id: "data_asset.example.corpus.001".to_string(),
                grant_id: "grant.example.corpus.001".to_string(),
                provider_id: SELLER_PUBKEY.to_string(),
                consumer_id: Some(BUYER_PUBKEY.to_string()),
                created_at_ms: 1_774_080_040_000,
                reason_code: "seller_revoked_access".to_string(),
                refund_amount: None,
                revoked_delivery_bundle_ids: vec!["delivery_bundle.example.001".to_string()],
                replacement_delivery_bundle_id: None,
                status: Default::default(),
                metadata: json!({}),
            },
        )
        .expect("revocation feedback event");
        let feedback = DataVendingFeedback::from_event(&event).expect("parse feedback");

        assert_eq!(feedback.status, JobStatus::Error);
        assert_eq!(
            feedback.reason_code.as_deref(),
            Some("seller_revoked_access")
        );
        assert_eq!(
            feedback.revocation_id.as_deref(),
            Some("revocation.example.001")
        );
        assert_eq!(
            feedback
                .listing_ref
                .as_ref()
                .map(|reference| reference.coordinate.to_string())
                .as_deref(),
            Some(
                "30404:1111111111111111111111111111111111111111111111111111111111111111:data_asset.example.corpus.001"
            )
        );
    }

    #[test]
    fn dataset_access_contract_builder_strips_delivery_digest_and_preserves_refs() {
        let mut request = fixture_request();
        request.delivery_draft.delivery_digest = Some(format!("sha256:{DIGEST}"));
        let contract = build_data_seller_access_contract(
            &request,
            SELLER_PUBKEY,
            DatasetAccessContractStatus::Delivered,
            Some("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"),
            None,
        )
        .expect("contract");

        assert_eq!(contract.status, DatasetAccessContractStatus::Delivered);
        assert_eq!(contract.buyer.pubkey, BUYER_PUBKEY);
        assert_eq!(
            contract.listing_ref.coordinate.to_string(),
            format!("30404:{SELLER_PUBKEY}:data_asset.example.corpus.001")
        );
        assert_eq!(
            contract
                .offer_ref
                .as_ref()
                .map(|reference| reference.coordinate.to_string())
                .as_deref(),
            Some("30406:1111111111111111111111111111111111111111111111111111111111111111:grant.example.corpus.001")
        );
        assert_eq!(contract.amount_msats, Some(5_000));
        assert_eq!(contract.delivery_ref.as_deref(), Some("oa://deliveries/example-001"));
        assert_eq!(contract.delivery_digest.as_deref(), Some(DIGEST));
        assert_eq!(
            contract
                .result_ref
                .as_ref()
                .map(|reference| reference.event_id.as_str()),
            Some("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc")
        );
    }

    #[test]
    fn relay_reconciled_request_restores_paid_and_delivered_state() {
        let mut request = fixture_request();
        request.payment_state = DataSellerPaymentState::Idle;
        request.payment_pointer = None;
        request.payment_observed_at_epoch_seconds = None;
        request.payment_amount_sats = None;
        request.delivery_state = DataSellerDeliveryState::Idle;
        request.delivery_bundle_id = None;
        request.delivery_result_event_id = None;
        request.revocation_state = DataSellerRevocationState::Idle;

        let mut market = DataMarketPaneState::default();
        market.apply_relay_catalog(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![fixture_contract_projection("delivered")],
            vec![fixture_result_projection()],
            vec![fixture_settlement_match()],
            1_774_080_043_000,
        );

        let reconciled = reconcile_request_from_relay_catalog(&request, &market);
        assert_eq!(reconciled.payment_state, DataSellerPaymentState::Paid);
        assert_eq!(reconciled.payment_pointer.as_deref(), Some("spark-payment-001"));
        assert_eq!(reconciled.payment_amount_sats, Some(5));
        assert_eq!(
            reconciled.payment_observed_at_epoch_seconds,
            Some(1_774_080_042)
        );
        assert_eq!(reconciled.delivery_state, DataSellerDeliveryState::Delivered);
        assert_eq!(
            reconciled.delivery_bundle_id.as_deref(),
            Some("delivery_bundle.example.001")
        );
        assert_eq!(
            reconciled.delivery_result_event_id.as_deref(),
            Some("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc")
        );
    }

    #[test]
    fn relay_reconciled_request_restores_revoked_state_from_contract() {
        let mut request = fixture_request();
        request.payment_state = DataSellerPaymentState::Idle;
        request.delivery_state = DataSellerDeliveryState::Idle;
        request.revocation_state = DataSellerRevocationState::Idle;
        let mut contract = fixture_contract_projection("revoked");
        contract.reason_code = Some("seller_revoked_access".to_string());

        let mut market = DataMarketPaneState::default();
        market.apply_relay_catalog(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![contract],
            vec![fixture_result_projection()],
            vec![fixture_settlement_match()],
            1_774_080_043_000,
        );

        let reconciled = reconcile_request_from_relay_catalog(&request, &market);
        assert_eq!(reconciled.payment_state, DataSellerPaymentState::Paid);
        assert_eq!(reconciled.delivery_state, DataSellerDeliveryState::Revoked);
        assert_eq!(reconciled.revocation_state, DataSellerRevocationState::Revoked);
        assert_eq!(
            reconciled.revocation_reason_code.as_deref(),
            Some("seller_revoked_access")
        );
        assert_eq!(
            reconciled.revocation_id.as_deref(),
            Some("30407:1111111111111111111111111111111111111111111111111111111111111111:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
    }

    #[test]
    fn relay_reconciled_request_restores_expired_state_from_contract() {
        let mut request = fixture_request();
        request.payment_state = DataSellerPaymentState::Idle;
        request.delivery_state = DataSellerDeliveryState::Idle;
        request.revocation_state = DataSellerRevocationState::Idle;
        let mut contract = fixture_contract_projection("expired");
        contract.reason_code = Some("access_window_expired".to_string());

        let mut market = DataMarketPaneState::default();
        market.apply_relay_catalog(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![contract],
            vec![fixture_result_projection()],
            vec![fixture_settlement_match()],
            1_774_080_043_000,
        );

        let reconciled = reconcile_request_from_relay_catalog(&request, &market);
        assert_eq!(reconciled.payment_state, DataSellerPaymentState::Paid);
        assert_eq!(reconciled.delivery_state, DataSellerDeliveryState::Expired);
        assert_eq!(reconciled.revocation_state, DataSellerRevocationState::Expired);
        assert_eq!(
            reconciled.revocation_reason_code.as_deref(),
            Some("access_window_expired")
        );
    }
}
