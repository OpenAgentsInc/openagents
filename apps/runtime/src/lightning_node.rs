use std::env;
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum LightningNodeError {
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("dependency unavailable: {0}")]
    DependencyUnavailable(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl LightningNodeError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidConfig(_) => "invalid_config",
            Self::DependencyUnavailable(_) => "dependency_unavailable",
            Self::Unsupported(_) => "unsupported",
            Self::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::InvalidConfig(message)
            | Self::DependencyUnavailable(message)
            | Self::Unsupported(message)
            | Self::Internal(message) => message.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightningBalancesV1 {
    pub schema: String,
    pub backend: String,
    pub onchain_sats: u64,
    pub channel_total_sats: u64,
    pub channel_outbound_sats: u64,
    pub channel_inbound_sats: u64,
    pub as_of: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelHealthSnapshotV1 {
    pub schema: String,
    pub backend: String,
    pub channel_count: u64,
    pub connected_channel_count: u64,
    pub as_of: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightningPayResultV1 {
    pub schema: String,
    pub preimage_sha256: Option<String>,
    pub fee_msats: Option<u64>,
    pub paid_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightningOpenChannelResultV1 {
    pub schema: String,
    pub channel_id: String,
    pub txid: String,
    pub opened_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightningCloseChannelResultV1 {
    pub schema: String,
    pub txids: Vec<String>,
    pub closed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightningRebalanceResultV1 {
    pub schema: String,
    pub status: String,
    pub spent_sats: u64,
    pub completed_at: DateTime<Utc>,
}

#[async_trait]
pub trait LightningNode: Send + Sync {
    fn backend(&self) -> &'static str;

    async fn get_balances(&self) -> Result<LightningBalancesV1, LightningNodeError>;

    async fn channel_health_snapshot(&self) -> Result<ChannelHealthSnapshotV1, LightningNodeError>;

    async fn pay_bolt11(
        &self,
        invoice: &str,
        max_fee_msats: u64,
        label: Option<String>,
    ) -> Result<LightningPayResultV1, LightningNodeError>;

    async fn open_channel(
        &self,
        peer_id: &str,
        amount_sats: u64,
    ) -> Result<LightningOpenChannelResultV1, LightningNodeError>;

    async fn close_channel(
        &self,
        channel_id: &str,
    ) -> Result<LightningCloseChannelResultV1, LightningNodeError>;

    async fn rebalance(
        &self,
        budget_sats: u64,
    ) -> Result<LightningRebalanceResultV1, LightningNodeError>;
}

pub fn from_env() -> Arc<dyn LightningNode> {
    let backend = env::var("RUNTIME_LLP_LIGHTNING_BACKEND")
        .unwrap_or_else(|_| "noop".to_string())
        .trim()
        .to_ascii_lowercase();

    match backend.as_str() {
        "noop" => Arc::new(NoopLightningNode),
        "lnd" => {
            let Some(base_url) =
                env_non_empty_any(&["RUNTIME_LLP_LND_REST_BASE_URL", "LND_REST_BASE_URL"])
            else {
                return Arc::new(UnavailableLightningNode::new(
                    "lnd",
                    "RUNTIME_LLP_LND_REST_BASE_URL (or LND_REST_BASE_URL) is required when backend=lnd",
                ));
            };

            let Some(macaroon_hex) =
                env_non_empty_any(&["RUNTIME_LLP_LND_REST_MACAROON_HEX", "LND_REST_MACAROON_HEX"])
            else {
                return Arc::new(UnavailableLightningNode::new(
                    "lnd",
                    "RUNTIME_LLP_LND_REST_MACAROON_HEX (or LND_REST_MACAROON_HEX) is required when backend=lnd",
                ));
            };

            let tls_cert_base64 = env_non_empty_any(&[
                "RUNTIME_LLP_LND_REST_TLS_CERT_BASE64",
                "LND_REST_TLS_CERT_BASE64",
            ]);
            let tls_verify = env_bool_any(
                &["RUNTIME_LLP_LND_REST_TLS_VERIFY", "LND_REST_TLS_VERIFY"],
                true,
            );
            let timeout_ms =
                env_u64_any(&["RUNTIME_LLP_LND_REST_TIMEOUT_MS"], 10_000).clamp(250, 120_000);

            match LndRestLightningNode::new(
                base_url,
                macaroon_hex,
                tls_cert_base64,
                tls_verify,
                timeout_ms,
            ) {
                Ok(node) => Arc::new(node),
                Err(error) => Arc::new(UnavailableLightningNode::new(
                    "lnd",
                    error.message().as_str(),
                )),
            }
        }
        value => Arc::new(UnavailableLightningNode::new(
            "unknown",
            format!("unsupported RUNTIME_LLP_LIGHTNING_BACKEND value: {value}").as_str(),
        )),
    }
}

#[derive(Debug)]
pub struct NoopLightningNode;

#[async_trait]
impl LightningNode for NoopLightningNode {
    fn backend(&self) -> &'static str {
        "noop"
    }

    async fn get_balances(&self) -> Result<LightningBalancesV1, LightningNodeError> {
        let now = Utc::now();
        Ok(LightningBalancesV1 {
            schema: "openagents.lightning.node_balances.v1".to_string(),
            backend: self.backend().to_string(),
            onchain_sats: 0,
            channel_total_sats: 0,
            channel_outbound_sats: 0,
            channel_inbound_sats: 0,
            as_of: now,
        })
    }

    async fn channel_health_snapshot(&self) -> Result<ChannelHealthSnapshotV1, LightningNodeError> {
        let now = Utc::now();
        Ok(ChannelHealthSnapshotV1 {
            schema: "openagents.lightning.channel_health.v1".to_string(),
            backend: self.backend().to_string(),
            channel_count: 0,
            connected_channel_count: 0,
            as_of: now,
        })
    }

    async fn pay_bolt11(
        &self,
        _invoice: &str,
        _max_fee_msats: u64,
        _label: Option<String>,
    ) -> Result<LightningPayResultV1, LightningNodeError> {
        Err(LightningNodeError::Unsupported(
            "noop backend cannot pay invoices".to_string(),
        ))
    }

    async fn open_channel(
        &self,
        _peer_id: &str,
        _amount_sats: u64,
    ) -> Result<LightningOpenChannelResultV1, LightningNodeError> {
        Err(LightningNodeError::Unsupported(
            "noop backend cannot open channels".to_string(),
        ))
    }

    async fn close_channel(
        &self,
        _channel_id: &str,
    ) -> Result<LightningCloseChannelResultV1, LightningNodeError> {
        Err(LightningNodeError::Unsupported(
            "noop backend cannot close channels".to_string(),
        ))
    }

    async fn rebalance(
        &self,
        _budget_sats: u64,
    ) -> Result<LightningRebalanceResultV1, LightningNodeError> {
        Err(LightningNodeError::Unsupported(
            "noop backend cannot rebalance".to_string(),
        ))
    }
}

#[derive(Debug)]
pub struct UnavailableLightningNode {
    backend: &'static str,
    reason: String,
}

impl UnavailableLightningNode {
    pub fn new(backend: &'static str, reason: &str) -> Self {
        Self {
            backend,
            reason: reason.to_string(),
        }
    }

    fn err(&self) -> LightningNodeError {
        LightningNodeError::DependencyUnavailable(self.reason.clone())
    }
}

#[async_trait]
impl LightningNode for UnavailableLightningNode {
    fn backend(&self) -> &'static str {
        self.backend
    }

    async fn get_balances(&self) -> Result<LightningBalancesV1, LightningNodeError> {
        Err(self.err())
    }

    async fn channel_health_snapshot(&self) -> Result<ChannelHealthSnapshotV1, LightningNodeError> {
        Err(self.err())
    }

    async fn pay_bolt11(
        &self,
        _invoice: &str,
        _max_fee_msats: u64,
        _label: Option<String>,
    ) -> Result<LightningPayResultV1, LightningNodeError> {
        Err(self.err())
    }

    async fn open_channel(
        &self,
        _peer_id: &str,
        _amount_sats: u64,
    ) -> Result<LightningOpenChannelResultV1, LightningNodeError> {
        Err(self.err())
    }

    async fn close_channel(
        &self,
        _channel_id: &str,
    ) -> Result<LightningCloseChannelResultV1, LightningNodeError> {
        Err(self.err())
    }

    async fn rebalance(
        &self,
        _budget_sats: u64,
    ) -> Result<LightningRebalanceResultV1, LightningNodeError> {
        Err(self.err())
    }
}

#[derive(Debug)]
struct LndChannelView {
    capacity_sats: u64,
    local_balance_sats: u64,
    active: bool,
}

#[derive(Debug)]
pub struct LndRestLightningNode {
    client: reqwest::Client,
    base_url: String,
    macaroon_hex: String,
}

impl LndRestLightningNode {
    pub fn new(
        base_url: String,
        macaroon_hex: String,
        tls_cert_base64: Option<String>,
        tls_verify: bool,
        timeout_ms: u64,
    ) -> Result<Self, LightningNodeError> {
        let base_url = base_url.trim().trim_end_matches('/').to_string();
        if base_url.is_empty() {
            return Err(LightningNodeError::InvalidConfig(
                "LND REST base URL cannot be empty".to_string(),
            ));
        }

        let macaroon_hex = macaroon_hex.trim().to_string();
        if macaroon_hex.is_empty() {
            return Err(LightningNodeError::InvalidConfig(
                "LND REST macaroon cannot be empty".to_string(),
            ));
        }

        let mut builder =
            reqwest::Client::builder().timeout(std::time::Duration::from_millis(timeout_ms));
        if let Some(cert_b64) = tls_cert_base64.as_ref().map(|value| value.trim()) {
            if !cert_b64.is_empty() {
                let decoded = STANDARD.decode(cert_b64).map_err(|error| {
                    LightningNodeError::InvalidConfig(format!(
                        "invalid RUNTIME_LLP_LND_REST_TLS_CERT_BASE64/LND_REST_TLS_CERT_BASE64: {error}"
                    ))
                })?;
                let cert = reqwest::Certificate::from_pem(&decoded)
                    .or_else(|_| reqwest::Certificate::from_der(&decoded))
                    .map_err(|error| {
                        LightningNodeError::InvalidConfig(format!(
                            "invalid LND REST TLS certificate bytes: {error}"
                        ))
                    })?;
                builder = builder.add_root_certificate(cert);
            }
        } else if !tls_verify {
            builder = builder.danger_accept_invalid_certs(true);
        }

        let client = builder.build().map_err(|error| {
            LightningNodeError::InvalidConfig(format!("failed to build LND REST client: {error}"))
        })?;

        Ok(Self {
            client,
            base_url,
            macaroon_hex,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    async fn get_json(&self, path: &str) -> Result<Value, LightningNodeError> {
        let url = self.endpoint(path);
        let response = self
            .client
            .get(url.as_str())
            .header("Grpc-Metadata-macaroon", self.macaroon_hex.as_str())
            .send()
            .await
            .map_err(|error| {
                LightningNodeError::DependencyUnavailable(format!(
                    "LND REST request failed for {path}: {error}"
                ))
            })?;

        let status = response.status();
        let body_text = response.text().await.map_err(|error| {
            LightningNodeError::DependencyUnavailable(format!(
                "LND REST response read failed for {path}: {error}"
            ))
        })?;
        if !status.is_success() {
            return Err(LightningNodeError::DependencyUnavailable(format!(
                "LND REST request failed for {path}: HTTP {} {body_text}",
                status.as_u16()
            )));
        }

        serde_json::from_str::<Value>(&body_text).map_err(|error| {
            LightningNodeError::DependencyUnavailable(format!(
                "LND REST response JSON parse failed for {path}: {error}"
            ))
        })
    }

    async fn list_channels(&self) -> Result<Vec<LndChannelView>, LightningNodeError> {
        let payload = self.get_json("/v1/channels").await?;
        let channels = payload
            .get("channels")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                LightningNodeError::DependencyUnavailable(
                    "LND REST /v1/channels did not return channels array".to_string(),
                )
            })?;

        let mut out = Vec::with_capacity(channels.len());
        for channel in channels {
            let capacity_sats = parse_u64_field(channel, "capacity").unwrap_or(0);
            let local_balance_sats = parse_u64_field(channel, "local_balance").unwrap_or(0);
            let active = parse_bool_field(channel, "active").unwrap_or(false);

            out.push(LndChannelView {
                capacity_sats,
                local_balance_sats,
                active,
            });
        }

        Ok(out)
    }
}

#[async_trait]
impl LightningNode for LndRestLightningNode {
    fn backend(&self) -> &'static str {
        "lnd"
    }

    async fn get_balances(&self) -> Result<LightningBalancesV1, LightningNodeError> {
        let now = Utc::now();
        let chain_payload = self.get_json("/v1/balance/blockchain").await?;
        let onchain_sats = parse_u64_field(&chain_payload, "total_balance")
            .or_else(|| parse_u64_field(&chain_payload, "confirmed_balance"))
            .unwrap_or(0);

        let channels = self.list_channels().await?;
        let mut channel_total_sats = 0_u64;
        let mut channel_outbound_sats = 0_u64;
        let mut channel_inbound_sats = 0_u64;

        for channel in channels {
            let total_sats = if channel.capacity_sats > 0 {
                channel.capacity_sats
            } else {
                channel.local_balance_sats
            };
            let outbound_sats = channel.local_balance_sats.min(total_sats);
            let inbound_sats = total_sats.saturating_sub(outbound_sats);

            channel_total_sats = channel_total_sats.saturating_add(total_sats);
            channel_outbound_sats = channel_outbound_sats.saturating_add(outbound_sats);
            channel_inbound_sats = channel_inbound_sats.saturating_add(inbound_sats);
        }

        Ok(LightningBalancesV1 {
            schema: "openagents.lightning.node_balances.v1".to_string(),
            backend: self.backend().to_string(),
            onchain_sats,
            channel_total_sats,
            channel_outbound_sats,
            channel_inbound_sats,
            as_of: now,
        })
    }

    async fn channel_health_snapshot(&self) -> Result<ChannelHealthSnapshotV1, LightningNodeError> {
        let now = Utc::now();
        let channels = self.list_channels().await?;
        let channel_count = channels.len() as u64;
        let connected_channel_count = channels.iter().filter(|entry| entry.active).count() as u64;

        Ok(ChannelHealthSnapshotV1 {
            schema: "openagents.lightning.channel_health.v1".to_string(),
            backend: self.backend().to_string(),
            channel_count,
            connected_channel_count,
            as_of: now,
        })
    }

    async fn pay_bolt11(
        &self,
        _invoice: &str,
        _max_fee_msats: u64,
        _label: Option<String>,
    ) -> Result<LightningPayResultV1, LightningNodeError> {
        Err(LightningNodeError::Unsupported(
            "LND backend pay_bolt11 is not implemented in Phase 0 (snapshot telemetry only)"
                .to_string(),
        ))
    }

    async fn open_channel(
        &self,
        _peer_id: &str,
        _amount_sats: u64,
    ) -> Result<LightningOpenChannelResultV1, LightningNodeError> {
        Err(LightningNodeError::Unsupported(
            "LND backend open_channel is not implemented in Phase 0 (snapshot telemetry only)"
                .to_string(),
        ))
    }

    async fn close_channel(
        &self,
        _channel_id: &str,
    ) -> Result<LightningCloseChannelResultV1, LightningNodeError> {
        Err(LightningNodeError::Unsupported(
            "LND backend close_channel is not implemented in Phase 0 (snapshot telemetry only)"
                .to_string(),
        ))
    }

    async fn rebalance(
        &self,
        _budget_sats: u64,
    ) -> Result<LightningRebalanceResultV1, LightningNodeError> {
        Err(LightningNodeError::Unsupported(
            "LND backend rebalance is not implemented in Phase 0 (snapshot telemetry only)"
                .to_string(),
        ))
    }
}

fn parse_u64_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(parse_u64_value)
}

fn parse_u64_value(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(raw) => raw.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn parse_bool_field(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(parse_bool_value)
}

fn parse_bool_value(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::Number(number) => number.as_u64().map(|v| v > 0),
        Value::String(raw) => {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "1" | "true" | "yes" => Some(true),
                "0" | "false" | "no" => Some(false),
                _ => None,
            }
        }
        _ => None,
    }
}

fn env_non_empty(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_non_empty_any(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| env_non_empty(key))
}

fn env_bool_any(keys: &[&str], default: bool) -> bool {
    keys.iter()
        .find_map(|key| env::var(key).ok())
        .map(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" => true,
            "0" | "false" | "no" => false,
            _ => default,
        })
        .unwrap_or(default)
}

fn env_u64_any(keys: &[&str], default: u64) -> u64 {
    keys.iter()
        .find_map(|key| env::var(key).ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
}
