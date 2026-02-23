use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use nostr::{
    AppData, Event, EventTemplate, HandlerInfo, HandlerMetadata, HandlerType, KIND_APP_DATA,
    KIND_HANDLER_INFO, Nip01Error, PricingInfo, finalize_event, verify_event,
};
use nostr_client::{PoolConfig, RelayPool};

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("invalid bridge input: {0}")]
    InvalidInput(String),

    #[error("nostr error: {0}")]
    Nostr(#[from] Nip01Error),

    #[error("nostr client error: {0}")]
    NostrClient(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Bridge boundary (v1) surfaces.
///
/// Phase 0 mirrors only:
/// - NIP-89 handler info events for provider ads (kind 31990)
/// - NIP-78 app data events for receipt pointers (kind 30078)
///
/// Phase 1+ may additionally mirror low-rate marketplace commerce messages via NIP-78
/// (see `docs/protocol/marketplace-commerce-grammar-v1.md`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeEventKind {
    ProviderAd,
    ReceiptPointer,
    CommerceMessage,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PricingStageV1 {
    Fixed,
    Banded,
    Bidding,
}

impl Default for PricingStageV1 {
    fn default() -> Self {
        Self::Fixed
    }
}

impl PricingStageV1 {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Banded => "banded",
            Self::Bidding => "bidding",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PricingBandV1 {
    pub capability: String,
    pub min_price_msats: u64,
    pub max_price_msats: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_msats: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderAdV1 {
    pub provider_id: String,
    pub name: String,
    pub description: String,
    pub website: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heartbeat_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caps: Option<serde_json::Value>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub min_price_msats: u64,
    #[serde(default)]
    pub pricing_stage: PricingStageV1,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pricing_bands: Vec<PricingBandV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReceiptPointerV1 {
    pub provider_id: String,
    pub run_id: String,
    pub job_hash: String,
    pub receipt_sha256: String,
    pub settlement_status: String,
    pub receipt_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommerceMessageKindV1 {
    Rfq,
    Offer,
    Quote,
    Accept,
    Cancel,
    Receipt,
    Refund,
    Dispute,
}

impl CommerceMessageKindV1 {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rfq => "rfq",
            Self::Offer => "offer",
            Self::Quote => "quote",
            Self::Accept => "accept",
            Self::Cancel => "cancel",
            Self::Receipt => "receipt",
            Self::Refund => "refund",
            Self::Dispute => "dispute",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommerceMessageV1 {
    pub message_id: String,
    pub kind: CommerceMessageKindV1,
    pub marketplace_id: String,
    pub actor_id: String,
    pub created_at_unix: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub rfq_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objective_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,

    pub body: serde_json::Value,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn receipt_sha256_from_utf8(receipt_json: &str) -> String {
    sha256_hex(receipt_json.as_bytes())
}

fn now_unix_seconds() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn build_provider_ad_event(
    secret_key: &[u8; 32],
    created_at: Option<u64>,
    payload: &ProviderAdV1,
) -> Result<Event, BridgeError> {
    if payload.provider_id.trim().is_empty() {
        return Err(BridgeError::InvalidInput(
            "provider_id must not be empty".to_string(),
        ));
    }
    if payload.name.trim().is_empty() {
        return Err(BridgeError::InvalidInput(
            "name must not be empty".to_string(),
        ));
    }

    let mut metadata = HandlerMetadata::new(payload.name.clone(), payload.description.clone());
    if let Some(website) = payload.website.as_ref().filter(|v| !v.trim().is_empty()) {
        metadata = metadata.with_website(website.clone());
    }

    let mut info = HandlerInfo::new(
        // pubkey is derived from secret key during signing; this is informational only.
        "npub:bridge",
        HandlerType::ComputeProvider,
        metadata,
    )
    .with_pricing(
        PricingInfo::new(payload.min_price_msats)
            .with_model("per-job")
            .with_currency("msats"),
    )
    // NIP-89 handler info is parameterized replaceable (kind 31990); `d` identifies the handler.
    .add_custom_tag(
        "d",
        format!("openagents:compute_provider:{}", payload.provider_id),
    )
    .add_custom_tag("oa_schema", "openagents.bridge.provider_ad.v1")
    .add_custom_tag("oa_provider_id", payload.provider_id.clone())
    .add_custom_tag("oa_pricing_stage", payload.pricing_stage.as_str());

    if let Some(value) = payload
        .availability
        .as_ref()
        .filter(|v| !v.trim().is_empty())
    {
        info = info.add_custom_tag("oa_availability", value.clone());
    }
    if let Some(value) = payload
        .worker_status
        .as_ref()
        .filter(|v| !v.trim().is_empty())
    {
        info = info.add_custom_tag("oa_worker_status", value.clone());
    }
    if let Some(value) = payload
        .heartbeat_state
        .as_ref()
        .filter(|v| !v.trim().is_empty())
    {
        info = info.add_custom_tag("oa_heartbeat_state", value.clone());
    }
    if let Some(caps) = payload.caps.as_ref().filter(|value| !value.is_null()) {
        let caps_json = serde_json::to_string(caps)
            .map_err(|err| BridgeError::Serialization(err.to_string()))?;
        info = info.add_custom_tag("oa_caps", caps_json);
    }

    for cap in &payload.capabilities {
        if !cap.trim().is_empty() {
            info = info.add_capability(cap.clone());
        }
    }

    for band in &payload.pricing_bands {
        if band.capability.trim().is_empty() {
            continue;
        }
        let band_json = serde_json::to_string(band)
            .map_err(|err| BridgeError::Serialization(err.to_string()))?;
        info = info.add_custom_tag("oa_pricing_band", band_json);
    }

    let template = EventTemplate {
        created_at: created_at.unwrap_or_else(now_unix_seconds),
        kind: KIND_HANDLER_INFO,
        tags: info.to_tags(),
        content: serde_json::to_string(&info.metadata)
            .map_err(|err| BridgeError::Serialization(err.to_string()))?,
    };

    let event = finalize_event(&template, secret_key)?;
    Ok(event)
}

pub fn build_receipt_pointer_event(
    secret_key: &[u8; 32],
    created_at: Option<u64>,
    payload: &ReceiptPointerV1,
) -> Result<Event, BridgeError> {
    if payload.provider_id.trim().is_empty() {
        return Err(BridgeError::InvalidInput(
            "provider_id must not be empty".to_string(),
        ));
    }
    if payload.run_id.trim().is_empty() {
        return Err(BridgeError::InvalidInput(
            "run_id must not be empty".to_string(),
        ));
    }
    if payload.job_hash.trim().is_empty() {
        return Err(BridgeError::InvalidInput(
            "job_hash must not be empty".to_string(),
        ));
    }
    if payload.receipt_sha256.trim().is_empty() {
        return Err(BridgeError::InvalidInput(
            "receipt_sha256 must not be empty".to_string(),
        ));
    }
    if payload.receipt_url.trim().is_empty() {
        return Err(BridgeError::InvalidInput(
            "receipt_url must not be empty".to_string(),
        ));
    }

    let identifier = format!("openagents:receipt_ptr:{}", payload.receipt_sha256);
    let content = serde_json::to_string(payload)
        .map_err(|err| BridgeError::Serialization(err.to_string()))?;
    let mut app = AppData::new(identifier, content);
    app.add_tag(vec![
        "oa_schema".to_string(),
        "openagents.bridge.receipt_ptr.v1".to_string(),
    ]);
    app.add_tag(vec![
        "oa_provider_id".to_string(),
        payload.provider_id.clone(),
    ]);
    app.add_tag(vec!["oa_run_id".to_string(), payload.run_id.clone()]);
    app.add_tag(vec!["oa_job_hash".to_string(), payload.job_hash.clone()]);
    app.add_tag(vec![
        "oa_settlement".to_string(),
        payload.settlement_status.clone(),
    ]);
    app.add_tag(vec![
        "oa_receipt_sha256".to_string(),
        payload.receipt_sha256.clone(),
    ]);

    let template = EventTemplate {
        created_at: created_at.unwrap_or_else(now_unix_seconds),
        kind: KIND_APP_DATA as u16,
        tags: app.to_tags(),
        content: app.content.clone(),
    };
    let event = finalize_event(&template, secret_key)?;
    Ok(event)
}

pub fn build_commerce_message_event(
    secret_key: &[u8; 32],
    created_at: Option<u64>,
    payload: &CommerceMessageV1,
) -> Result<Event, BridgeError> {
    if payload.message_id.trim().is_empty() {
        return Err(BridgeError::InvalidInput(
            "message_id must not be empty".to_string(),
        ));
    }
    if payload.marketplace_id.trim().is_empty() {
        return Err(BridgeError::InvalidInput(
            "marketplace_id must not be empty".to_string(),
        ));
    }
    if payload.actor_id.trim().is_empty() {
        return Err(BridgeError::InvalidInput(
            "actor_id must not be empty".to_string(),
        ));
    }

    let identifier = format!(
        "openagents:commerce:{}:{}",
        payload.kind.as_str(),
        payload.message_id
    );
    let content = serde_json::to_string(payload)
        .map_err(|err| BridgeError::Serialization(err.to_string()))?;
    let body_sha256 = receipt_sha256_from_utf8(&content);

    let mut app = AppData::new(identifier, content);
    app.add_tag(vec![
        "oa_schema".to_string(),
        "openagents.bridge.commerce_message.v1".to_string(),
    ]);
    app.add_tag(vec![
        "oa_commerce_kind".to_string(),
        payload.kind.as_str().to_string(),
    ]);
    app.add_tag(vec![
        "oa_marketplace_id".to_string(),
        payload.marketplace_id.clone(),
    ]);
    app.add_tag(vec!["oa_actor_id".to_string(), payload.actor_id.clone()]);
    app.add_tag(vec!["oa_body_sha256".to_string(), body_sha256.clone()]);

    let linkage_tags: [(&str, &Option<String>); 7] = [
        ("oa_rfq_id", &payload.rfq_id),
        ("oa_offer_id", &payload.offer_id),
        ("oa_quote_id", &payload.quote_id),
        ("oa_order_id", &payload.order_id),
        ("oa_receipt_id", &payload.receipt_id),
        ("oa_objective_hash", &payload.objective_hash),
        ("oa_run_id", &payload.run_id),
    ];
    for (tag, value) in linkage_tags {
        if let Some(value) = value.as_ref().filter(|v| !v.trim().is_empty()) {
            app.add_tag(vec![tag.to_string(), value.clone()]);
        }
    }

    let template = EventTemplate {
        created_at: created_at.unwrap_or_else(now_unix_seconds),
        kind: KIND_APP_DATA as u16,
        tags: app.to_tags(),
        content: app.content.clone(),
    };
    let event = finalize_event(&template, secret_key)?;
    Ok(event)
}

pub struct BridgeNostrPublisher {
    relays: Vec<String>,
    pool: RelayPool,
}

impl BridgeNostrPublisher {
    pub fn new(relays: Vec<String>) -> Self {
        Self {
            relays,
            pool: RelayPool::new(PoolConfig::default()),
        }
    }

    pub async fn connect(&self) -> Result<(), BridgeError> {
        for relay in &self.relays {
            self.pool
                .add_relay(relay)
                .await
                .map_err(|err| BridgeError::NostrClient(err.to_string()))?;
        }
        self.pool
            .connect_all()
            .await
            .map_err(|err| BridgeError::NostrClient(err.to_string()))?;
        Ok(())
    }

    pub async fn publish(&self, event: &Event) -> Result<(), BridgeError> {
        self.pool
            .publish(event)
            .await
            .map(|_| ())
            .map_err(|err| BridgeError::NostrClient(err.to_string()))
    }
}

pub fn validate_phase0_bridge_event(event: &Event) -> Result<BridgeEventKind, BridgeError> {
    if !verify_event(event)? {
        return Err(BridgeError::InvalidInput(
            "event signature/id verification failed".to_string(),
        ));
    }

    if event.kind == KIND_HANDLER_INFO {
        return Ok(BridgeEventKind::ProviderAd);
    }
    if u64::from(event.kind) == KIND_APP_DATA {
        if event.tags.iter().any(|t| {
            t.len() >= 2 && t[0] == "oa_schema" && t[1] == "openagents.bridge.receipt_ptr.v1"
        }) {
            return Ok(BridgeEventKind::ReceiptPointer);
        }
        return Err(BridgeError::InvalidInput(
            "unsupported Phase-0 app-data schema (expected receipt_ptr)".to_string(),
        ));
    }

    Err(BridgeError::InvalidInput(format!(
        "unsupported Phase-0 bridge event kind: {}",
        event.kind
    )))
}

pub fn validate_bridge_event_v1(event: &Event) -> Result<BridgeEventKind, BridgeError> {
    if !verify_event(event)? {
        return Err(BridgeError::InvalidInput(
            "event signature/id verification failed".to_string(),
        ));
    }

    if event.kind == KIND_HANDLER_INFO {
        return Ok(BridgeEventKind::ProviderAd);
    }

    if u64::from(event.kind) != KIND_APP_DATA {
        return Err(BridgeError::InvalidInput(format!(
            "unsupported bridge event kind: {}",
            event.kind
        )));
    }

    if event
        .tags
        .iter()
        .any(|t| t.len() >= 2 && t[0] == "oa_schema" && t[1] == "openagents.bridge.receipt_ptr.v1")
    {
        return Ok(BridgeEventKind::ReceiptPointer);
    }
    if event.tags.iter().any(|t| {
        t.len() >= 2 && t[0] == "oa_schema" && t[1] == "openagents.bridge.commerce_message.v1"
    }) {
        return Ok(BridgeEventKind::CommerceMessage);
    }

    Err(BridgeError::InvalidInput(
        "unsupported bridge app-data schema".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_ad_event_is_signed_and_phase0_valid() {
        let secret = nostr::generate_secret_key();
        let payload = ProviderAdV1 {
            provider_id: "provider-local-1".to_string(),
            name: "OpenAgents Compute Provider".to_string(),
            description: "Local sandbox provider for Phase 0".to_string(),
            website: Some("https://openagents.com".to_string()),
            availability: Some("available".to_string()),
            worker_status: Some("running".to_string()),
            heartbeat_state: Some("fresh".to_string()),
            caps: Some(serde_json::json!({"max_timeout_secs": 120})),
            capabilities: vec!["oa.sandbox_run.v1".to_string()],
            min_price_msats: 1000,
            pricing_stage: PricingStageV1::Fixed,
            pricing_bands: vec![PricingBandV1 {
                capability: "oa.sandbox_run.v1".to_string(),
                min_price_msats: 1000,
                max_price_msats: 2000,
                step_msats: Some(100),
            }],
        };

        let event = match build_provider_ad_event(&secret, Some(1_700_000_000), &payload) {
            Ok(event) => event,
            Err(err) => panic!("build_provider_ad_event failed: {err}"),
        };

        let kind = match validate_phase0_bridge_event(&event) {
            Ok(kind) => kind,
            Err(err) => panic!("validate_phase0_bridge_event failed: {err}"),
        };

        assert!(matches!(kind, BridgeEventKind::ProviderAd));
        assert_eq!(event.kind, KIND_HANDLER_INFO);
        assert!(
            event
                .tags
                .iter()
                .any(|t| t.len() >= 2 && t[0] == "handler" && t[1] == "compute_provider")
        );
        assert!(event.tags.iter().any(|t| t.len() >= 2 && t[0] == "d"));
        assert!(
            event
                .tags
                .iter()
                .any(|t| t.len() >= 2 && t[0] == "capability" && t[1] == "oa.sandbox_run.v1")
        );
        assert!(
            event
                .tags
                .iter()
                .any(|t| t.len() >= 2 && t[0] == "oa_pricing_stage" && t[1] == "fixed")
        );
        assert!(
            event
                .tags
                .iter()
                .any(|t| t.len() >= 2 && t[0] == "oa_pricing_band")
        );
        assert!(
            event
                .tags
                .iter()
                .any(|t| t.len() >= 2 && t[0] == "oa_availability" && t[1] == "available")
        );
        assert!(
            event
                .tags
                .iter()
                .any(|t| t.len() >= 2 && t[0] == "oa_worker_status" && t[1] == "running")
        );
        assert!(
            event
                .tags
                .iter()
                .any(|t| t.len() >= 2 && t[0] == "oa_heartbeat_state" && t[1] == "fresh")
        );
        assert!(event.tags.iter().any(|t| t.len() >= 2 && t[0] == "oa_caps"));
    }

    #[test]
    fn receipt_pointer_event_is_signed_and_phase0_valid() {
        let secret = nostr::generate_secret_key();
        let receipt_sha256 = receipt_sha256_from_utf8(r#"{"schema":"openagents.receipt.v1"}"#);
        let payload = ReceiptPointerV1 {
            provider_id: "provider-local-1".to_string(),
            run_id: "00000000-0000-0000-0000-000000000000".to_string(),
            job_hash: "jobhash".to_string(),
            receipt_sha256: receipt_sha256.clone(),
            settlement_status: "released".to_string(),
            receipt_url: "http://127.0.0.1:8787/internal/v1/runs/000/receipt".to_string(),
        };

        let event = match build_receipt_pointer_event(&secret, Some(1_700_000_001), &payload) {
            Ok(event) => event,
            Err(err) => panic!("build_receipt_pointer_event failed: {err}"),
        };

        let kind = match validate_phase0_bridge_event(&event) {
            Ok(kind) => kind,
            Err(err) => panic!("validate_phase0_bridge_event failed: {err}"),
        };

        assert!(matches!(kind, BridgeEventKind::ReceiptPointer));
        assert_eq!(u64::from(event.kind), KIND_APP_DATA);
        assert!(
            event.tags.iter().any(|t| t.len() >= 2
                && t[0] == "d"
                && t[1].starts_with("openagents:receipt_ptr:"))
        );
        assert!(
            event
                .tags
                .iter()
                .any(|t| t.len() >= 2 && t[0] == "oa_receipt_sha256" && t[1] == receipt_sha256)
        );
    }

    #[test]
    fn commerce_message_event_is_signed_and_v1_valid() {
        let secret = nostr::generate_secret_key();
        let payload = CommerceMessageV1 {
            message_id: "msg-1".to_string(),
            kind: CommerceMessageKindV1::Quote,
            marketplace_id: "market-openagents".to_string(),
            actor_id: "provider-local-1".to_string(),
            created_at_unix: 1_700_000_002,
            rfq_id: Some("rfq-1".to_string()),
            offer_id: Some("offer-1".to_string()),
            quote_id: Some("quote-1".to_string()),
            order_id: None,
            receipt_id: None,
            objective_hash: Some("sha256:jobhash".to_string()),
            run_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
            body: serde_json::json!({
                "total_msats": 1234,
                "valid_until_unix": 1_700_000_999
            }),
        };

        let event = match build_commerce_message_event(&secret, Some(1_700_000_002), &payload) {
            Ok(event) => event,
            Err(err) => panic!("build_commerce_message_event failed: {err}"),
        };

        assert!(
            validate_phase0_bridge_event(&event).is_err(),
            "Phase-0 validator must reject commerce messages"
        );

        let kind = match validate_bridge_event_v1(&event) {
            Ok(kind) => kind,
            Err(err) => panic!("validate_bridge_event_v1 failed: {err}"),
        };

        assert!(matches!(kind, BridgeEventKind::CommerceMessage));
        assert_eq!(u64::from(event.kind), KIND_APP_DATA);
        assert!(event.tags.iter().any(|t| t.len() >= 2
            && t[0] == "d"
            && t[1].starts_with("openagents:commerce:quote:")));
        assert!(
            event
                .tags
                .iter()
                .any(|t| t.len() >= 2 && t[0] == "oa_quote_id" && t[1] == "quote-1")
        );

        let content_sha = receipt_sha256_from_utf8(&event.content);
        assert!(
            event
                .tags
                .iter()
                .any(|t| t.len() >= 2 && t[0] == "oa_body_sha256" && t[1] == content_sha)
        );
    }
}
