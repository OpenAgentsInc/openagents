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

/// Phase-0 Bridge boundary (v1) produces only:
/// - NIP-89 handler info events for provider ads (kind 31990)
/// - NIP-78 app data events for receipt pointers (kind 30078)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeEventKind {
    ProviderAd,
    ReceiptPointer,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderAdV1 {
    pub provider_id: String,
    pub name: String,
    pub description: String,
    pub website: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub min_price_msats: u64,
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
    .add_custom_tag("oa_schema", "openagents.bridge.provider_ad.v1")
    .add_custom_tag("oa_provider_id", payload.provider_id.clone());

    for cap in &payload.capabilities {
        if !cap.trim().is_empty() {
            info = info.add_capability(cap.clone());
        }
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
    app.add_tag(vec!["oa_schema".to_string(), "openagents.bridge.receipt_ptr.v1".to_string()]);
    app.add_tag(vec!["oa_provider_id".to_string(), payload.provider_id.clone()]);
    app.add_tag(vec!["oa_run_id".to_string(), payload.run_id.clone()]);
    app.add_tag(vec!["oa_job_hash".to_string(), payload.job_hash.clone()]);
    app.add_tag(vec!["oa_settlement".to_string(), payload.settlement_status.clone()]);
    app.add_tag(vec!["oa_receipt_sha256".to_string(), payload.receipt_sha256.clone()]);

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
        return Ok(BridgeEventKind::ReceiptPointer);
    }

    Err(BridgeError::InvalidInput(format!(
        "unsupported Phase-0 bridge event kind: {}",
        event.kind
    )))
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
            capabilities: vec!["oa.sandbox_run.v1".to_string()],
            min_price_msats: 1000,
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
        assert!(event
            .tags
            .iter()
            .any(|t| t.len() >= 2 && t[0] == "handler" && t[1] == "compute_provider"));
        assert!(event
            .tags
            .iter()
            .any(|t| t.len() >= 2 && t[0] == "capability" && t[1] == "oa.sandbox_run.v1"));
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
        assert!(event
            .tags
            .iter()
            .any(|t| t.len() >= 2 && t[0] == "d" && t[1].starts_with("openagents:receipt_ptr:")));
        assert!(event
            .tags
            .iter()
            .any(|t| t.len() >= 2 && t[0] == "oa_receipt_sha256" && t[1] == receipt_sha256));
    }
}
