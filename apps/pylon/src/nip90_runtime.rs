use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use nostr::nip89::{HandlerInfo, HandlerMetadata, HandlerType, KIND_HANDLER_INFO, PricingInfo};
use nostr::{Event, EventTemplate, finalize_event};
use nostr_client::{PoolConfig, RelayConfig, RelayPool};
use openagents_provider_substrate::ProviderAdvertisedProduct;
use serde::Serialize;

use crate::{
    PylonConfig, PylonLedgerAnnouncement, ensure_identity, load_config_and_status, load_ledger,
    mutate_ledger, products_from_status,
};

const ANNOUNCEMENT_KIND_TEXT_GENERATION: u16 = nostr::nip90::KIND_JOB_TEXT_GENERATION;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnouncementAction {
    Show,
    Publish,
    Refresh,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AnnouncementReport {
    pub node_label: String,
    pub provider_pubkey: String,
    pub status: String,
    pub request_kind: u16,
    pub handler_event_id: Option<String>,
    pub model: Option<String>,
    pub backend: Option<String>,
    pub capabilities: Vec<String>,
    pub price_msats: Option<u64>,
    pub relay_urls: Vec<String>,
    pub published_at_ms: Option<u64>,
    pub fingerprint: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug)]
struct AnnouncementSpec {
    model: String,
    backend: String,
    capabilities: Vec<String>,
    price_msats: Option<u64>,
    fingerprint: String,
}

pub async fn load_announcement_report(config_path: &Path) -> Result<AnnouncementReport> {
    let config = crate::ensure_local_setup(config_path)?;
    let identity = ensure_identity(config.identity_path.as_path())?;
    let (_, status) = load_config_and_status(config_path).await?;
    let ledger = load_ledger(config_path)?;
    let latest = ledger.announcements.first().cloned();
    let spec = announcement_spec(&config, &status);
    Ok(build_announcement_report(
        &config,
        identity.public_key_hex.as_str(),
        latest.as_ref(),
        spec.as_ref(),
        None,
    ))
}

pub async fn publish_announcement_report(
    config_path: &Path,
    refresh_only: bool,
) -> Result<AnnouncementReport> {
    let config = crate::ensure_local_setup(config_path)?;
    let identity = ensure_identity(config.identity_path.as_path())?;
    let (_, status) = load_config_and_status(config_path).await?;
    let spec = announcement_spec(&config, &status)
        .ok_or_else(|| anyhow::anyhow!("no eligible local text-generation supply is ready"))?;
    let existing = load_ledger(config_path)?.announcements.first().cloned();
    if refresh_only
        && existing
            .as_ref()
            .is_some_and(|existing| existing.fingerprint == spec.fingerprint)
    {
        return Ok(build_announcement_report(
            &config,
            identity.public_key_hex.as_str(),
            existing.as_ref(),
            Some(&spec),
            Some("announcement already matches the current local supply".to_string()),
        ));
    }

    let event = publish_announcement_event(&config, &identity, &spec).await?;
    let now = crate::now_epoch_ms() as u64;
    let record = PylonLedgerAnnouncement {
        announcement_id: format!("handler:{}", ANNOUNCEMENT_KIND_TEXT_GENERATION),
        event_id: event.id.clone(),
        request_kind: ANNOUNCEMENT_KIND_TEXT_GENERATION,
        model: Some(spec.model.clone()),
        backend: Some(spec.backend.clone()),
        capabilities: spec.capabilities.clone(),
        price_msats: spec.price_msats,
        relay_urls: config.relay_urls.clone(),
        fingerprint: spec.fingerprint.clone(),
        published_at_ms: now,
        updated_at_ms: now,
    };
    mutate_ledger(config_path, |ledger| {
        ledger.upsert_announcement(record.clone());
        ledger.push_relay_activity(crate::PylonRelayActivity {
            at_ms: now,
            url: None,
            kind: "announcement.published".to_string(),
            detail: format!(
                "published kind:{} handler {} for model {}",
                ANNOUNCEMENT_KIND_TEXT_GENERATION, event.id, spec.model
            ),
        });
        Ok(())
    })?;

    Ok(build_announcement_report(
        &config,
        identity.public_key_hex.as_str(),
        Some(&record),
        Some(&spec),
        Some(format!("published handler event {}", event.id)),
    ))
}

pub fn render_announcement_report(report: &AnnouncementReport) -> String {
    let mut lines = vec![
        format!("node_label: {}", report.node_label),
        format!("provider_pubkey: {}", report.provider_pubkey),
        format!("status: {}", report.status),
        format!("request_kind: {}", report.request_kind),
        format!(
            "handler_event_id: {}",
            report.handler_event_id.as_deref().unwrap_or("none")
        ),
        format!("model: {}", report.model.as_deref().unwrap_or("none")),
        format!("backend: {}", report.backend.as_deref().unwrap_or("none")),
        format!(
            "capabilities: {}",
            comma_or_none(report.capabilities.as_slice())
        ),
        format!(
            "price_msats: {}",
            report
                .price_msats
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("relays: {}", comma_or_none(report.relay_urls.as_slice())),
        format!(
            "published_at_ms: {}",
            report
                .published_at_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("fingerprint: {}", report.fingerprint),
    ];
    if let Some(detail) = report.detail.as_deref() {
        lines.push(format!("detail: {detail}"));
    }
    lines.join("\n")
}

fn build_announcement_report(
    config: &PylonConfig,
    provider_pubkey: &str,
    existing: Option<&PylonLedgerAnnouncement>,
    spec: Option<&AnnouncementSpec>,
    detail: Option<String>,
) -> AnnouncementReport {
    let status = match (existing, spec) {
        (Some(_), Some(_)) => "ready".to_string(),
        (Some(_), None) => "stale".to_string(),
        (None, Some(_)) => "unpublished".to_string(),
        (None, None) => "not_ready".to_string(),
    };
    AnnouncementReport {
        node_label: config.node_label.clone(),
        provider_pubkey: provider_pubkey.to_string(),
        status,
        request_kind: ANNOUNCEMENT_KIND_TEXT_GENERATION,
        handler_event_id: existing.map(|entry| entry.event_id.clone()),
        model: spec
            .map(|entry| entry.model.clone())
            .or_else(|| existing.and_then(|entry| entry.model.clone())),
        backend: spec
            .map(|entry| entry.backend.clone())
            .or_else(|| existing.and_then(|entry| entry.backend.clone())),
        capabilities: spec
            .map(|entry| entry.capabilities.clone())
            .unwrap_or_else(|| {
                existing
                    .map(|entry| entry.capabilities.clone())
                    .unwrap_or_default()
            }),
        price_msats: spec
            .map(|entry| entry.price_msats)
            .unwrap_or_else(|| existing.and_then(|entry| entry.price_msats)),
        relay_urls: config.relay_urls.clone(),
        published_at_ms: existing.map(|entry| entry.published_at_ms),
        fingerprint: spec
            .map(|entry| entry.fingerprint.clone())
            .unwrap_or_else(|| {
                existing
                    .map(|entry| entry.fingerprint.clone())
                    .unwrap_or_else(|| "none".to_string())
            }),
        detail,
    }
}

fn announcement_spec(
    config: &PylonConfig,
    status: &openagents_provider_substrate::ProviderStatusResponse,
) -> Option<AnnouncementSpec> {
    let products = products_from_status(config, status);
    let product = first_text_generation_product(products.as_slice())?;
    let backend = product.product.backend_label().to_string();
    let model = match backend.as_str() {
        "gpt_oss" => status
            .snapshot
            .as_ref()?
            .availability
            .gpt_oss
            .ready_model
            .clone()?,
        "apple_foundation_models" => status
            .snapshot
            .as_ref()?
            .availability
            .apple_foundation_models
            .ready_model
            .clone()?,
        _ => return None,
    };
    let price_msats = product.price_floor_sats.checked_mul(1000);
    let capabilities = vec![
        "nip90.5050".to_string(),
        "text_generation".to_string(),
        format!("backend:{backend}"),
        format!("model:{model}"),
        format!("product:{}", product.product.product_id()),
    ];
    let fingerprint = [
        ANNOUNCEMENT_KIND_TEXT_GENERATION.to_string(),
        backend.clone(),
        model.clone(),
        price_msats.unwrap_or(0).to_string(),
    ]
    .join("|");
    Some(AnnouncementSpec {
        model,
        backend,
        capabilities,
        price_msats,
        fingerprint,
    })
}

fn first_text_generation_product(
    products: &[ProviderAdvertisedProduct],
) -> Option<ProviderAdvertisedProduct> {
    products
        .iter()
        .find(|product| {
            product.eligible
                && product.product.compute_family_label() == "inference"
                && matches!(
                    product.product.backend_label(),
                    "gpt_oss" | "apple_foundation_models"
                )
        })
        .cloned()
}

async fn publish_announcement_event(
    config: &PylonConfig,
    identity: &nostr::NostrIdentity,
    spec: &AnnouncementSpec,
) -> Result<Event> {
    let metadata = HandlerMetadata::new(
        format!("Pylon {}", config.node_label),
        format!(
            "Standalone OpenAgents compute provider for local {} text generation",
            spec.model
        ),
    );
    let mut info = HandlerInfo::new(
        identity.public_key_hex.as_str(),
        HandlerType::ComputeProvider,
        metadata.clone(),
    )
    .add_custom_tag(
        "d",
        format!(
            "pylon:{}:{}",
            config.node_label, ANNOUNCEMENT_KIND_TEXT_GENERATION
        ),
    )
    .add_custom_tag("k", ANNOUNCEMENT_KIND_TEXT_GENERATION.to_string())
    .add_custom_tag("model", spec.model.clone())
    .add_custom_tag("backend", spec.backend.clone())
    .add_custom_tag("node", config.node_label.clone());
    for capability in &spec.capabilities {
        info = info.add_capability(capability.clone());
    }
    if let Some(price_msats) = spec.price_msats {
        info = info.with_pricing(
            PricingInfo::new(price_msats)
                .with_model("per_request")
                .with_currency("msats"),
        );
    }
    let template = EventTemplate {
        kind: KIND_HANDLER_INFO,
        tags: info.to_tags(),
        content: serde_json::to_string(&metadata)?,
        created_at: nostr::nip01::unix_now_secs().unwrap_or(0),
    };
    let event = finalize_event(
        &template,
        &decode_private_key_hex(identity.private_key_hex.as_str())?,
    )
    .context("failed to sign Pylon handler announcement")?;
    let pool = build_relay_pool(config, identity).await?;
    let confirmations = pool.publish(&event).await?;
    if !confirmations
        .iter()
        .any(|confirmation| confirmation.accepted)
    {
        let detail = confirmations
            .iter()
            .map(|confirmation| format!("{}:{}", confirmation.relay_url, confirmation.message))
            .collect::<Vec<_>>()
            .join("; ");
        bail!("no relay accepted the announcement event: {detail}");
    }
    Ok(event)
}

async fn build_relay_pool(
    config: &PylonConfig,
    identity: &nostr::NostrIdentity,
) -> Result<RelayPool> {
    let relay_config = RelayConfig {
        connect_timeout: Duration::from_secs(config.relay_connect_timeout_seconds),
        nip42_identity: config
            .relay_auth_enabled
            .then(|| nostr_client::RelayAuthIdentity {
                private_key_hex: identity.private_key_hex.clone(),
            }),
    };
    let pool = RelayPool::new(PoolConfig {
        max_relays: config.relay_urls.len().max(1),
        relay_config,
    });
    for relay in &config.relay_urls {
        pool.add_relay(relay.as_str())
            .await
            .with_context(|| format!("failed to add relay {}", relay))?;
    }
    pool.connect_all()
        .await
        .context("failed to connect relays")?;
    Ok(pool)
}

fn decode_private_key_hex(value: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(value).with_context(|| "invalid pylon private key hex")?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected 32-byte private key"))?;
    Ok(bytes)
}

fn comma_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}
