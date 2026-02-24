use std::env;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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

#[derive(Debug, Clone)]
pub struct LightningNodePolicy {
    pub peer_allowlist: Vec<String>,
    pub max_channel_sats_per_peer: u64,
    pub max_daily_rebalance_sats: u64,
}

impl Default for LightningNodePolicy {
    fn default() -> Self {
        Self {
            peer_allowlist: Vec::new(),
            max_channel_sats_per_peer: 5_000_000,
            max_daily_rebalance_sats: 500_000,
        }
    }
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
        "cln" => {
            let rpc_path = env::var("RUNTIME_LLP_CLN_RPC_PATH")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            let Some(rpc_path) = rpc_path else {
                return Arc::new(UnavailableLightningNode::new(
                    "cln",
                    "RUNTIME_LLP_CLN_RPC_PATH is required when backend=cln",
                ));
            };

            let peer_allowlist = env::var("RUNTIME_LLP_PEER_ALLOWLIST")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_ascii_lowercase())
                .collect::<Vec<_>>();
            let max_channel_sats_per_peer = env::var("RUNTIME_LLP_MAX_CHANNEL_SATS_PER_PEER")
                .ok()
                .and_then(|value| value.trim().parse::<u64>().ok())
                .unwrap_or(LightningNodePolicy::default().max_channel_sats_per_peer);
            let max_daily_rebalance_sats = env::var("RUNTIME_LLP_MAX_DAILY_REBALANCE_SATS")
                .ok()
                .and_then(|value| value.trim().parse::<u64>().ok())
                .unwrap_or(LightningNodePolicy::default().max_daily_rebalance_sats);

            Arc::new(ClnLightningNode::new(
                PathBuf::from(rpc_path),
                LightningNodePolicy {
                    peer_allowlist,
                    max_channel_sats_per_peer,
                    max_daily_rebalance_sats,
                },
            ))
        }
        _ => Arc::new(NoopLightningNode),
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
pub struct ClnLightningNode {
    rpc_path: PathBuf,
    policy: LightningNodePolicy,
}

impl ClnLightningNode {
    pub fn new(rpc_path: PathBuf, policy: LightningNodePolicy) -> Self {
        Self { rpc_path, policy }
    }

    fn enforce_peer_allowlist(&self, peer_id: &str) -> Result<(), LightningNodeError> {
        if self.policy.peer_allowlist.is_empty() {
            return Ok(());
        }
        let peer_id = peer_id.trim().to_ascii_lowercase();
        if self
            .policy
            .peer_allowlist
            .iter()
            .any(|allowed| allowed == &peer_id)
        {
            return Ok(());
        }
        Err(LightningNodeError::InvalidConfig(
            "peer_id is not allowlisted".to_string(),
        ))
    }

    async fn with_rpc<F, T>(&self, f: F) -> Result<T, LightningNodeError>
    where
        F: for<'a> FnOnce(&'a mut cln_rpc::ClnRpc) -> BoxFuture<'a, Result<T, LightningNodeError>>
            + Send,
        T: Send,
    {
        let path = Path::new(&self.rpc_path);
        let mut rpc = cln_rpc::ClnRpc::new(path)
            .await
            .map_err(|error| LightningNodeError::DependencyUnavailable(error.to_string()))?;
        f(&mut rpc).await
    }
}

#[async_trait]
impl LightningNode for ClnLightningNode {
    fn backend(&self) -> &'static str {
        "cln"
    }

    async fn get_balances(&self) -> Result<LightningBalancesV1, LightningNodeError> {
        let now = Utc::now();
        let response = self
            .with_rpc(|rpc| {
                Box::pin(async move {
                    let req = cln_rpc::model::requests::ListfundsRequest { spent: None };
                    rpc.call_typed(&req)
                        .await
                        .map_err(|error| {
                            LightningNodeError::DependencyUnavailable(error.to_string())
                        })
                })
            })
            .await?;

        let mut onchain_msats: u64 = 0;
        for output in &response.outputs {
            if output.status.to_string() != "CONFIRMED" {
                continue;
            }
            onchain_msats = onchain_msats.saturating_add(output.amount_msat.msat());
        }

        let mut channel_total_msats: u64 = 0;
        let mut channel_outbound_msats: u64 = 0;
        let mut channel_inbound_msats: u64 = 0;
        for chan in &response.channels {
            let total = chan.amount_msat.msat();
            let ours = chan.our_amount_msat.msat().min(total);
            channel_total_msats = channel_total_msats.saturating_add(total);
            channel_outbound_msats = channel_outbound_msats.saturating_add(ours);
            channel_inbound_msats = channel_inbound_msats.saturating_add(total.saturating_sub(ours));
        }

        Ok(LightningBalancesV1 {
            schema: "openagents.lightning.node_balances.v1".to_string(),
            backend: self.backend().to_string(),
            onchain_sats: onchain_msats / 1000,
            channel_total_sats: channel_total_msats / 1000,
            channel_outbound_sats: channel_outbound_msats / 1000,
            channel_inbound_sats: channel_inbound_msats / 1000,
            as_of: now,
        })
    }

    async fn channel_health_snapshot(&self) -> Result<ChannelHealthSnapshotV1, LightningNodeError> {
        let now = Utc::now();
        let response = self
            .with_rpc(|rpc| {
                Box::pin(async move {
                    let req = cln_rpc::model::requests::ListfundsRequest { spent: None };
                    rpc.call_typed(&req)
                        .await
                        .map_err(|error| {
                            LightningNodeError::DependencyUnavailable(error.to_string())
                        })
                })
            })
            .await?;

        let channel_count = response.channels.len() as u64;
        let connected_channel_count = response
            .channels
            .iter()
            .filter(|chan| chan.connected)
            .count() as u64;

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
        invoice: &str,
        max_fee_msats: u64,
        label: Option<String>,
    ) -> Result<LightningPayResultV1, LightningNodeError> {
        let invoice = invoice.trim().to_string();
        if invoice.is_empty() {
            return Err(LightningNodeError::InvalidConfig(
                "invoice is required".to_string(),
            ));
        }

        let response = self
            .with_rpc(|rpc| {
                Box::pin(async move {
                    let req = cln_rpc::model::requests::PayRequest {
                        amount_msat: None,
                        description: None,
                        exemptfee: None,
                        label,
                        localinvreqid: None,
                        maxdelay: None,
                        maxfee: Some(cln_rpc::primitives::Amount::from_msat(max_fee_msats)),
                        maxfeepercent: None,
                        partial_msat: None,
                        retry_for: Some(30),
                        riskfactor: None,
                        exclude: None,
                        bolt11: invoice,
                    };
                    rpc.call_typed(&req)
                        .await
                        .map_err(|error| {
                            LightningNodeError::DependencyUnavailable(error.to_string())
                        })
                })
            })
            .await?;

        let fee_msats = response
            .amount_sent_msat
            .msat()
            .saturating_sub(response.amount_msat.msat());
        let preimage_bytes = response.payment_preimage.to_vec();
        let preimage_sha256 = hex::encode(Sha256::digest(preimage_bytes));

        Ok(LightningPayResultV1 {
            schema: "openagents.lightning.pay_result.v1".to_string(),
            preimage_sha256: Some(preimage_sha256),
            fee_msats: Some(fee_msats),
            paid_at: Utc::now(),
        })
    }

    async fn open_channel(
        &self,
        peer_id: &str,
        amount_sats: u64,
    ) -> Result<LightningOpenChannelResultV1, LightningNodeError> {
        let peer_id_raw = peer_id.trim().to_ascii_lowercase();
        if peer_id_raw.is_empty() {
            return Err(LightningNodeError::InvalidConfig(
                "peer_id is required".to_string(),
            ));
        }
        if amount_sats == 0 {
            return Err(LightningNodeError::InvalidConfig(
                "amount_sats must be > 0".to_string(),
            ));
        }
        if amount_sats > self.policy.max_channel_sats_per_peer {
            return Err(LightningNodeError::InvalidConfig(format!(
                "amount_sats exceeds max_channel_sats_per_peer ({})",
                self.policy.max_channel_sats_per_peer
            )));
        }
        self.enforce_peer_allowlist(peer_id_raw.as_str())?;

        let opened_at = Utc::now();
        let response = self
            .with_rpc(|rpc| {
                Box::pin(async move {
                    let peer = cln_rpc::primitives::PublicKey::from_str(peer_id_raw.as_str())
                        .map_err(|error| LightningNodeError::InvalidConfig(error.to_string()))?;
                    let req = cln_rpc::model::requests::FundchannelRequest {
                        announce: Some(false),
                        close_to: None,
                        compact_lease: None,
                        feerate: None,
                        minconf: None,
                        mindepth: None,
                        push_msat: None,
                        request_amt: None,
                        reserve: None,
                        channel_type: None,
                        utxos: None,
                        amount: cln_rpc::primitives::AmountOrAll::Amount(
                            cln_rpc::primitives::Amount::from_sat(amount_sats),
                        ),
                        id: peer,
                    };
                    rpc.call_typed(&req)
                        .await
                        .map_err(|error| {
                            LightningNodeError::DependencyUnavailable(error.to_string())
                        })
                })
            })
            .await?;

        Ok(LightningOpenChannelResultV1 {
            schema: "openagents.lightning.open_channel_result.v1".to_string(),
            channel_id: response.channel_id.to_string(),
            txid: response.txid,
            opened_at,
        })
    }

    async fn close_channel(
        &self,
        channel_id: &str,
    ) -> Result<LightningCloseChannelResultV1, LightningNodeError> {
        let channel_id = channel_id.trim().to_string();
        if channel_id.is_empty() {
            return Err(LightningNodeError::InvalidConfig(
                "channel_id is required".to_string(),
            ));
        }

        let closed_at = Utc::now();
        let response = self
            .with_rpc(|rpc| {
                Box::pin(async move {
                    let req = cln_rpc::model::requests::CloseRequest {
                        destination: None,
                        fee_negotiation_step: None,
                        force_lease_closed: None,
                        unilateraltimeout: None,
                        wrong_funding: None,
                        feerange: None,
                        id: channel_id,
                    };
                    rpc.call_typed(&req)
                        .await
                        .map_err(|error| {
                            LightningNodeError::DependencyUnavailable(error.to_string())
                        })
                })
            })
            .await?;

        Ok(LightningCloseChannelResultV1 {
            schema: "openagents.lightning.close_channel_result.v1".to_string(),
            txids: response.txids.unwrap_or_default(),
            closed_at,
        })
    }

    async fn rebalance(
        &self,
        budget_sats: u64,
    ) -> Result<LightningRebalanceResultV1, LightningNodeError> {
        if budget_sats > self.policy.max_daily_rebalance_sats {
            return Err(LightningNodeError::InvalidConfig(format!(
                "budget_sats exceeds max_daily_rebalance_sats ({})",
                self.policy.max_daily_rebalance_sats
            )));
        }
        Err(LightningNodeError::Unsupported(
            "CLN backend rebalancing is not implemented in Phase 0 (requires a plugin)".to_string(),
        ))
    }
}
