use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use futures::future::BoxFuture;
use ldk_server_client::client::LdkServerClient as RemoteLdkServerClient;
use ldk_server_client::error::{LdkServerError as RemoteLdkServerError, LdkServerErrorCode};
use ldk_server_client::ldk_server_grpc::api::{
    Bolt11ReceiveRequest as RemoteBolt11ReceiveRequest,
    Bolt11SendRequest as RemoteBolt11SendRequest, Bolt12SendRequest as RemoteBolt12SendRequest,
    CloseChannelRequest as RemoteCloseChannelRequest,
    ConnectPeerRequest as RemoteConnectPeerRequest,
    ForceCloseChannelRequest as RemoteForceCloseChannelRequest,
    GetBalancesRequest as RemoteGetBalancesRequest, GetNodeInfoRequest as RemoteGetNodeInfoRequest,
    GetPaymentDetailsRequest as RemoteGetPaymentDetailsRequest,
    ListChannelsRequest as RemoteListChannelsRequest,
    ListPaymentsRequest as RemoteListPaymentsRequest, ListPeersRequest as RemoteListPeersRequest,
    OpenChannelRequest as RemoteOpenChannelRequest, SpliceInRequest as RemoteSpliceInRequest,
    SpliceOutRequest as RemoteSpliceOutRequest,
};
use ldk_server_client::ldk_server_grpc::types::{
    Bolt11InvoiceDescription as RemoteBolt11InvoiceDescription, Channel as RemoteChannel,
    Network as RemoteNetwork, Payment as RemotePayment, PaymentStatus as RemotePaymentStatus,
    Peer as RemotePeer, bolt11_invoice_description as remote_bolt11_invoice_description,
    payment_kind as remote_payment_kind,
};
use ring::hmac;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub type TreasuryProviderResult<T> = std::result::Result<T, TreasuryProviderError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreasuryLightningProviderKind {
    Ldk,
}

impl TreasuryLightningProviderKind {
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self::Ldk);
        };
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "ldk" => Ok(Self::Ldk),
            other => Err(format!(
                "invalid NEXUS_TREASURY_PROVIDER '{other}' (supported: ldk)"
            )),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ldk => "ldk",
        }
    }
}

impl Default for TreasuryLightningProviderKind {
    fn default() -> Self {
        Self::Ldk
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LdkNetwork {
    Regtest,
    Signet,
    Bitcoin,
}

impl LdkNetwork {
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self::Regtest);
        };
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "regtest" => Ok(Self::Regtest),
            "signet" => Ok(Self::Signet),
            "bitcoin" | "mainnet" => Ok(Self::Bitcoin),
            other => Err(format!(
                "invalid NEXUS_LDK_NETWORK '{other}' (supported: regtest, signet, bitcoin)"
            )),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Regtest => "regtest",
            Self::Signet => "signet",
            Self::Bitcoin => "bitcoin",
        }
    }

    const fn invoice_prefix(self) -> &'static str {
        match self {
            Self::Regtest => "lnbcrt",
            Self::Signet => "lntbs",
            Self::Bitcoin => "lnbc",
        }
    }
}

impl Default for LdkNetwork {
    fn default() -> Self {
        Self::Regtest
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LdkChainBackend {
    Bitcoind,
    Electrum,
    Esplora,
}

impl LdkChainBackend {
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self::Bitcoind);
        };
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "bitcoind" => Ok(Self::Bitcoind),
            "electrum" => Ok(Self::Electrum),
            "esplora" => Ok(Self::Esplora),
            other => Err(format!(
                "invalid NEXUS_LDK_CHAIN_BACKEND '{other}' (supported: bitcoind, electrum, esplora)"
            )),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bitcoind => "bitcoind",
            Self::Electrum => "electrum",
            Self::Esplora => "esplora",
        }
    }
}

impl Default for LdkChainBackend {
    fn default() -> Self {
        Self::Bitcoind
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkTreasuryProviderConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_cert_path: Option<PathBuf>,
    pub storage_dir: PathBuf,
    pub network: LdkNetwork,
    pub chain_backend: LdkChainBackend,
}

impl LdkTreasuryProviderConfig {
    pub fn local_scaffold(storage_dir: PathBuf) -> Self {
        Self {
            server_url: None,
            api_key_path: None,
            tls_cert_path: None,
            storage_dir,
            network: LdkNetwork::Regtest,
            chain_backend: LdkChainBackend::Bitcoind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkTlsCertificatePin {
    pub path: PathBuf,
    pub sha256_hex: String,
    pub pem_len: usize,
}

pub fn load_ldk_tls_certificate_pin(path: &Path) -> LdkServerClientResult<LdkTlsCertificatePin> {
    let pem = fs::read(path).map_err(|error| {
        LdkServerClientError::new(
            LdkServerClientErrorKind::InvalidConfig,
            format!("tls_certificate_read_failed:{error}"),
        )
    })?;
    if !pem
        .windows(b"-----BEGIN CERTIFICATE-----".len())
        .any(|window| window == b"-----BEGIN CERTIFICATE-----")
    {
        return Err(LdkServerClientError::new(
            LdkServerClientErrorKind::MalformedResponse,
            "tls_certificate_pem_missing_begin_marker",
        ));
    }

    let digest = Sha256::digest(&pem);
    Ok(LdkTlsCertificatePin {
        path: path.to_path_buf(),
        sha256_hex: hex::encode(digest),
        pem_len: pem.len(),
    })
}

pub fn ldk_hmac_auth_metadata(api_key: &[u8], unix_timestamp_seconds: u64) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, api_key);
    let tag = hmac::sign(&key, &unix_timestamp_seconds.to_be_bytes());
    format!(
        "HMAC {unix_timestamp_seconds}:{}",
        hex::encode(tag.as_ref())
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LdkServerClientMode {
    LocalHarness,
    RemoteGrpc,
}

impl LdkServerClientMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::LocalHarness => "local_harness",
            Self::RemoteGrpc => "remote_grpc",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkServerClientConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_cert_path: Option<PathBuf>,
    pub storage_dir: PathBuf,
    pub network: LdkNetwork,
    pub chain_backend: LdkChainBackend,
}

impl From<&LdkTreasuryProviderConfig> for LdkServerClientConfig {
    fn from(value: &LdkTreasuryProviderConfig) -> Self {
        Self {
            server_url: value.server_url.clone(),
            api_key_path: value.api_key_path.clone(),
            tls_cert_path: value.tls_cert_path.clone(),
            storage_dir: value.storage_dir.clone(),
            network: value.network,
            chain_backend: value.chain_backend,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LdkServerClientErrorKind {
    InvalidConfig,
    InvalidRequest,
    Auth,
    Unavailable,
    NoRoute,
    InsufficientBalance,
    StaleEventStream,
    MalformedResponse,
    PaymentFailed,
    Internal,
}

impl LdkServerClientErrorKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidConfig => "invalid_config",
            Self::InvalidRequest => "invalid_request",
            Self::Auth => "auth",
            Self::Unavailable => "unavailable",
            Self::NoRoute => "no_route",
            Self::InsufficientBalance => "insufficient_balance",
            Self::StaleEventStream => "stale_event_stream",
            Self::MalformedResponse => "malformed_response",
            Self::PaymentFailed => "payment_failed",
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkServerClientError {
    pub kind: LdkServerClientErrorKind,
    pub reason: String,
}

impl LdkServerClientError {
    pub fn new(kind: LdkServerClientErrorKind, reason: impl Into<String>) -> Self {
        Self {
            kind,
            reason: reason.into(),
        }
    }

    pub fn from_service_fixture(status: &str, message: &str) -> Self {
        let status = status.trim().to_ascii_lowercase();
        let message_normalized = message.trim().to_ascii_lowercase();
        let kind = if status.contains("unavailable") {
            LdkServerClientErrorKind::Unavailable
        } else if status.contains("unauthenticated") || status.contains("permission") {
            LdkServerClientErrorKind::Auth
        } else if message_normalized.contains("no route")
            || message_normalized.contains("route not found")
        {
            LdkServerClientErrorKind::NoRoute
        } else if message_normalized.contains("insufficient")
            || message_normalized.contains("not enough")
        {
            LdkServerClientErrorKind::InsufficientBalance
        } else if message_normalized.contains("stale")
            || message_normalized.contains("lagged")
            || message_normalized.contains("event stream")
        {
            LdkServerClientErrorKind::StaleEventStream
        } else if status.contains("invalid") {
            LdkServerClientErrorKind::InvalidRequest
        } else if status.contains("malformed") || message_normalized.contains("malformed") {
            LdkServerClientErrorKind::MalformedResponse
        } else if message_normalized.contains("failed") {
            LdkServerClientErrorKind::PaymentFailed
        } else {
            LdkServerClientErrorKind::Internal
        };
        Self::new(kind, sanitize_error_reason(message))
    }

    pub fn normalized_reason(&self) -> String {
        format!(
            "ldk_server_client_error:{}:{}",
            self.kind.as_str(),
            self.reason
        )
    }
}

impl fmt::Display for LdkServerClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.normalized_reason().as_str())
    }
}

impl std::error::Error for LdkServerClientError {}

pub type LdkServerClientResult<T> = std::result::Result<T, LdkServerClientError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LdkServerPaymentStatus {
    Pending,
    Succeeded,
    Failed,
    Unknown,
}

impl LdkServerPaymentStatus {
    const fn terminal_event_state(self) -> Option<&'static str> {
        match self {
            Self::Succeeded => Some("payment_successful"),
            Self::Failed => Some("payment_failed"),
            Self::Pending | Self::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkServerNodeInfo {
    pub node_id: String,
    pub network: LdkNetwork,
    pub chain_backend: LdkChainBackend,
    pub current_best_block_height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkServerBalances {
    pub total_onchain_sats: u64,
    pub spendable_onchain_sats: u64,
    pub lightning_sats: u64,
    pub usable_sats: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkServerBolt11ReceiveRequest {
    pub amount_sats: Option<u64>,
    pub description: String,
    pub expiry_seconds: Option<u32>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkServerBolt11ReceiveResponse {
    pub invoice: String,
    pub payment_hash: String,
    pub payment_id: String,
    pub amount_msat: Option<u64>,
    pub expires_at_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkServerPayment {
    pub payment_id: String,
    pub payment_hash: String,
    pub amount_msat: Option<u64>,
    pub status: LdkServerPaymentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkServerPeer {
    pub node_id: String,
    pub address_hash: Option<String>,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkServerChannel {
    pub channel_id: String,
    pub peer_node_id: String,
    pub status: String,
    pub outbound_capacity_sats: u64,
    pub inbound_capacity_sats: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkServerAdminReceipt {
    pub command: String,
    pub operation_ref: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_node_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LdkServerEventKind {
    Bolt11InvoiceCreated,
    PaymentReceived,
    PaymentSuccessful,
    PaymentFailed,
    EventStreamDisconnected,
}

impl LdkServerEventKind {
    const fn operation_status(self) -> &'static str {
        match self {
            Self::Bolt11InvoiceCreated => "pending",
            Self::PaymentReceived => "observed",
            Self::PaymentSuccessful => "completed",
            Self::PaymentFailed => "failed",
            Self::EventStreamDisconnected => "degraded",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkServerEvent {
    pub sequence: u64,
    pub kind: LdkServerEventKind,
    pub payment_id: Option<String>,
    pub amount_msat: Option<u64>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkServerProjectedOperation {
    pub operation_id: String,
    pub kind: String,
    pub rail: String,
    pub rail_metadata: BTreeMap<String, String>,
    pub amount_msat: Option<u64>,
    pub status: String,
    pub provider_payment_id_hash: Option<String>,
    pub terminal_event_state: Option<String>,
}

#[derive(Clone)]
pub struct LdkServerClient {
    config: LdkServerClientConfig,
    mode: LdkServerClientMode,
    tls_pin: Option<LdkTlsCertificatePin>,
    api_key: Option<Vec<u8>>,
    local_state: Arc<Mutex<BTreeMap<String, LdkServerPayment>>>,
    local_peers: Arc<Mutex<BTreeMap<String, LdkServerPeer>>>,
    local_channels: Arc<Mutex<BTreeMap<String, LdkServerChannel>>>,
}

impl fmt::Debug for LdkServerClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LdkServerClient")
            .field("mode", &self.mode)
            .field("server_url", &self.config.server_url)
            .field("network", &self.config.network)
            .field("chain_backend", &self.config.chain_backend)
            .field("tls_pin", &self.tls_pin)
            .field(
                "api_key_loaded",
                &self.api_key.as_ref().map(|key| !key.is_empty()),
            )
            .finish_non_exhaustive()
    }
}

impl LdkServerClient {
    pub fn from_provider_config(config: &LdkTreasuryProviderConfig) -> LdkServerClientResult<Self> {
        Self::new(LdkServerClientConfig::from(config))
    }

    pub fn new(config: LdkServerClientConfig) -> LdkServerClientResult<Self> {
        let remote_requested = config.server_url.is_some()
            || config.api_key_path.is_some()
            || config.tls_cert_path.is_some();
        let (mode, tls_pin, api_key) = if remote_requested {
            let server_url = config.server_url.as_deref().ok_or_else(|| {
                LdkServerClientError::new(
                    LdkServerClientErrorKind::InvalidConfig,
                    "ldk_server_url_required_for_remote_grpc",
                )
            })?;
            validate_remote_server_url(server_url)?;
            let api_key_path = config.api_key_path.as_deref().ok_or_else(|| {
                LdkServerClientError::new(
                    LdkServerClientErrorKind::InvalidConfig,
                    "ldk_api_key_path_required_for_remote_grpc",
                )
            })?;
            let tls_cert_path = config.tls_cert_path.as_deref().ok_or_else(|| {
                LdkServerClientError::new(
                    LdkServerClientErrorKind::InvalidConfig,
                    "ldk_tls_cert_path_required_for_remote_grpc",
                )
            })?;
            (
                LdkServerClientMode::RemoteGrpc,
                Some(load_ldk_tls_certificate_pin(tls_cert_path)?),
                Some(load_ldk_api_key(api_key_path)?),
            )
        } else {
            (LdkServerClientMode::LocalHarness, None, None)
        };

        Ok(Self {
            config,
            mode,
            tls_pin,
            api_key,
            local_state: Arc::new(Mutex::new(BTreeMap::new())),
            local_peers: Arc::new(Mutex::new(BTreeMap::new())),
            local_channels: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub fn mode(&self) -> LdkServerClientMode {
        self.mode
    }

    pub fn current_hmac_metadata_for_test(
        &self,
        unix_timestamp_seconds: u64,
    ) -> LdkServerClientResult<Option<String>> {
        Ok(self
            .api_key
            .as_ref()
            .map(|api_key| ldk_hmac_auth_metadata(api_key, unix_timestamp_seconds)))
    }

    fn remote_client(&self) -> LdkServerClientResult<RemoteLdkServerClient> {
        let server_url = self.config.server_url.as_deref().ok_or_else(|| {
            LdkServerClientError::new(
                LdkServerClientErrorKind::InvalidConfig,
                "ldk_server_url_required_for_remote_grpc",
            )
        })?;
        let api_key = self.api_key.as_ref().ok_or_else(|| {
            LdkServerClientError::new(
                LdkServerClientErrorKind::InvalidConfig,
                "ldk_api_key_required_for_remote_grpc",
            )
        })?;
        let api_key = std::str::from_utf8(api_key).map_err(|error| {
            LdkServerClientError::new(
                LdkServerClientErrorKind::InvalidConfig,
                format!("api_key_utf8_invalid:{error}"),
            )
        })?;
        let tls_cert_path = self.config.tls_cert_path.as_deref().ok_or_else(|| {
            LdkServerClientError::new(
                LdkServerClientErrorKind::InvalidConfig,
                "ldk_tls_cert_path_required_for_remote_grpc",
            )
        })?;
        let tls_cert = fs::read(tls_cert_path).map_err(|error| {
            LdkServerClientError::new(
                LdkServerClientErrorKind::InvalidConfig,
                format!("tls_certificate_read_failed:{error}"),
            )
        })?;

        RemoteLdkServerClient::new(server_url.to_string(), api_key.to_string(), &tls_cert).map_err(
            |error| {
                LdkServerClientError::new(
                    LdkServerClientErrorKind::InvalidConfig,
                    format!(
                        "remote_grpc_client_build_failed:{}",
                        sanitize_error_reason(&error)
                    ),
                )
            },
        )
    }

    pub async fn get_node_info(&self) -> LdkServerClientResult<LdkServerNodeInfo> {
        if self.mode == LdkServerClientMode::RemoteGrpc {
            let response = self
                .remote_client()?
                .get_node_info(RemoteGetNodeInfoRequest {})
                .await
                .map_err(map_remote_ldk_error)?;
            return Ok(LdkServerNodeInfo {
                node_id: response.node_id,
                network: map_remote_network(response.network, self.config.network),
                chain_backend: self.config.chain_backend,
                current_best_block_height: response
                    .current_best_block
                    .map(|block| block.height)
                    .unwrap_or_default(),
            });
        }
        Ok(LdkServerNodeInfo {
            node_id: format!(
                "02{}",
                short_hash(&format!(
                    "{}:{}:node",
                    self.config.network.as_str(),
                    self.config.chain_backend.as_str()
                ))
            ),
            network: self.config.network,
            chain_backend: self.config.chain_backend,
            current_best_block_height: 1_000,
        })
    }

    pub async fn get_balances(&self) -> LdkServerClientResult<LdkServerBalances> {
        if self.mode == LdkServerClientMode::RemoteGrpc {
            let remote = self.remote_client()?;
            let response = remote
                .get_balances(RemoteGetBalancesRequest {})
                .await
                .map_err(map_remote_ldk_error)?;
            let channels = remote
                .list_channels(RemoteListChannelsRequest {})
                .await
                .map(|response| response.channels)
                .unwrap_or_default();
            let lightning_spendable_sats = channels
                .iter()
                .map(|channel| channel.outbound_capacity_msat / 1_000)
                .fold(0u64, u64::saturating_add);
            return Ok(LdkServerBalances {
                total_onchain_sats: response.total_onchain_balance_sats,
                spendable_onchain_sats: response.spendable_onchain_balance_sats,
                lightning_sats: response.total_lightning_balance_sats,
                usable_sats: response
                    .spendable_onchain_balance_sats
                    .saturating_add(lightning_spendable_sats),
            });
        }
        let total_sats = self.local_total_sats()?;
        Ok(LdkServerBalances {
            total_onchain_sats: total_sats,
            spendable_onchain_sats: total_sats,
            lightning_sats: total_sats,
            usable_sats: total_sats,
        })
    }

    pub async fn bolt11_receive(
        &self,
        request: LdkServerBolt11ReceiveRequest,
    ) -> LdkServerClientResult<LdkServerBolt11ReceiveResponse> {
        if matches!(request.amount_sats, Some(0)) {
            return Err(LdkServerClientError::new(
                LdkServerClientErrorKind::InvalidRequest,
                "bolt11_receive_amount_must_be_greater_than_zero",
            ));
        }
        if request.idempotency_key.trim().is_empty() {
            return Err(LdkServerClientError::new(
                LdkServerClientErrorKind::InvalidRequest,
                "bolt11_receive_idempotency_key_missing",
            ));
        }
        if self.mode == LdkServerClientMode::RemoteGrpc {
            let expiry_seconds = request.expiry_seconds.unwrap_or(86_400);
            let response = self
                .remote_client()?
                .bolt11_receive(RemoteBolt11ReceiveRequest {
                    amount_msat: request
                        .amount_sats
                        .map(|amount| amount.saturating_mul(1_000)),
                    description: Some(RemoteBolt11InvoiceDescription {
                        kind: Some(remote_bolt11_invoice_description::Kind::Direct(
                            request.description,
                        )),
                    }),
                    expiry_secs: expiry_seconds,
                })
                .await
                .map_err(map_remote_ldk_error)?;
            let payment_id = if response.payment_hash.trim().is_empty() {
                format!("ldk-bolt11-{}", short_hash(response.invoice.as_str()))
            } else {
                format!("ldk-bolt11-{}", response.payment_hash)
            };
            return Ok(LdkServerBolt11ReceiveResponse {
                invoice: response.invoice,
                payment_hash: response.payment_hash,
                payment_id,
                amount_msat: request
                    .amount_sats
                    .map(|amount| amount.saturating_mul(1_000)),
                expires_at_unix_seconds: Some(
                    now_unix_seconds().saturating_add(u64::from(expiry_seconds)),
                ),
            });
        }
        let amount_sats = request.amount_sats.unwrap_or(0);
        let digest = short_hash(&format!(
            "{}:{}:{}",
            self.config.network.as_str(),
            self.config.chain_backend.as_str(),
            request.idempotency_key
        ));
        let invoice = format!(
            "{}{}nexus{}",
            self.config.network.invoice_prefix(),
            amount_sats,
            digest
        );
        let payment_hash = format!("ldk-local-hash-{digest}");
        let payment_id = format!("ldk-local-payment-{digest}");
        let amount_msat = request
            .amount_sats
            .map(|amount| amount.saturating_mul(1_000));
        let expires_at_unix_seconds = request
            .expiry_seconds
            .map(|expiry| now_unix_seconds().saturating_add(u64::from(expiry)));
        let payment = LdkServerPayment {
            payment_id: payment_id.clone(),
            payment_hash: payment_hash.clone(),
            amount_msat,
            status: LdkServerPaymentStatus::Pending,
        };
        self.local_payments()?.insert(payment_id.clone(), payment);
        Ok(LdkServerBolt11ReceiveResponse {
            invoice,
            payment_hash,
            payment_id,
            amount_msat,
            expires_at_unix_seconds,
        })
    }

    pub async fn list_payments(&self) -> LdkServerClientResult<Vec<LdkServerPayment>> {
        if self.mode == LdkServerClientMode::RemoteGrpc {
            let remote = self.remote_client()?;
            let mut payments = Vec::new();
            let mut page_token = None;
            for _ in 0..25 {
                let response = remote
                    .list_payments(RemoteListPaymentsRequest { page_token })
                    .await
                    .map_err(map_remote_ldk_error)?;
                payments.extend(response.payments.iter().map(map_remote_payment));
                page_token = response.next_page_token;
                if page_token.is_none() {
                    break;
                }
            }
            return Ok(payments);
        }
        Ok(self.local_payments()?.values().cloned().collect())
    }

    pub async fn get_payment(&self, payment_id: &str) -> LdkServerClientResult<LdkServerPayment> {
        if self.mode == LdkServerClientMode::RemoteGrpc {
            let response = self
                .remote_client()?
                .get_payment_details(RemoteGetPaymentDetailsRequest {
                    payment_id: payment_id.to_string(),
                })
                .await
                .map_err(map_remote_ldk_error)?;
            return response
                .payment
                .as_ref()
                .map(map_remote_payment)
                .ok_or_else(|| {
                    LdkServerClientError::new(
                        LdkServerClientErrorKind::InvalidRequest,
                        "payment_not_found",
                    )
                });
        }
        self.local_payments()?
            .get(payment_id)
            .cloned()
            .ok_or_else(|| {
                LdkServerClientError::new(
                    LdkServerClientErrorKind::InvalidRequest,
                    "payment_not_found",
                )
            })
    }

    pub async fn list_channels(&self) -> LdkServerClientResult<Vec<LdkServerChannel>> {
        if self.mode == LdkServerClientMode::RemoteGrpc {
            let response = self
                .remote_client()?
                .list_channels(RemoteListChannelsRequest {})
                .await
                .map_err(map_remote_ldk_error)?;
            return Ok(response.channels.iter().map(map_remote_channel).collect());
        }
        Ok(self.local_channels()?.values().cloned().collect())
    }

    pub async fn list_peers(&self) -> LdkServerClientResult<Vec<LdkServerPeer>> {
        if self.mode == LdkServerClientMode::RemoteGrpc {
            let response = self
                .remote_client()?
                .list_peers(RemoteListPeersRequest {})
                .await
                .map_err(map_remote_ldk_error)?;
            return Ok(response.peers.iter().map(map_remote_peer).collect());
        }
        Ok(self.local_peers()?.values().cloned().collect())
    }

    pub async fn connect_peer(
        &self,
        node_id: &str,
        address: Option<&str>,
        idempotency_key: &str,
    ) -> LdkServerClientResult<LdkServerAdminReceipt> {
        let node_id = normalize_required_ldk_field(node_id, "peer_node_id_missing")?;
        if self.mode == LdkServerClientMode::RemoteGrpc {
            let address = normalize_required_ldk_field(
                address.unwrap_or_default(),
                "peer_address_missing_for_remote_connect",
            )?;
            self.remote_client()?
                .connect_peer(RemoteConnectPeerRequest {
                    node_pubkey: node_id.clone(),
                    address,
                    persist: true,
                })
                .await
                .map_err(map_remote_ldk_error)?;
            return Ok(LdkServerAdminReceipt {
                command: "treasury.connectPeer".to_string(),
                operation_ref: local_admin_operation_ref("connect_peer", idempotency_key),
                status: "completed".to_string(),
                payment_id: None,
                channel_id: None,
                peer_node_id: Some(node_id),
            });
        }
        let operation_ref = local_admin_operation_ref("connect_peer", idempotency_key);
        self.local_peers()?.insert(
            node_id.clone(),
            LdkServerPeer {
                node_id: node_id.clone(),
                address_hash: address
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(short_hash),
                connected: true,
            },
        );
        Ok(LdkServerAdminReceipt {
            command: "treasury.connectPeer".to_string(),
            operation_ref,
            status: "completed".to_string(),
            payment_id: None,
            channel_id: None,
            peer_node_id: Some(node_id),
        })
    }

    pub async fn open_channel(
        &self,
        peer_node_id: &str,
        address: Option<&str>,
        amount_sats: u64,
        idempotency_key: &str,
    ) -> LdkServerClientResult<LdkServerAdminReceipt> {
        let peer_node_id = normalize_required_ldk_field(peer_node_id, "peer_node_id_missing")?;
        if amount_sats == 0 {
            return Err(LdkServerClientError::new(
                LdkServerClientErrorKind::InvalidRequest,
                "open_channel_amount_sats_must_be_greater_than_zero",
            ));
        }
        if self.mode == LdkServerClientMode::RemoteGrpc {
            let address = normalize_required_ldk_field(
                address.unwrap_or_default(),
                "peer_address_missing_for_remote_open_channel",
            )?;
            let response = self
                .remote_client()?
                .open_channel(RemoteOpenChannelRequest {
                    node_pubkey: peer_node_id.clone(),
                    address,
                    channel_amount_sats: amount_sats,
                    push_to_counterparty_msat: None,
                    channel_config: None,
                    announce_channel: false,
                    disable_counterparty_reserve: false,
                })
                .await
                .map_err(map_remote_ldk_error)?;
            return Ok(LdkServerAdminReceipt {
                command: "treasury.openChannel".to_string(),
                operation_ref: local_admin_operation_ref("open_channel", idempotency_key),
                status: "pending".to_string(),
                payment_id: None,
                channel_id: Some(response.user_channel_id),
                peer_node_id: Some(peer_node_id),
            });
        }
        let digest = short_hash(&format!("{peer_node_id}:{amount_sats}:{idempotency_key}"));
        let channel_id = format!("ldk-local-channel-{digest}");
        self.local_channels()?.insert(
            channel_id.clone(),
            LdkServerChannel {
                channel_id: channel_id.clone(),
                peer_node_id: peer_node_id.clone(),
                status: "pending_open".to_string(),
                outbound_capacity_sats: amount_sats,
                inbound_capacity_sats: 0,
            },
        );
        Ok(LdkServerAdminReceipt {
            command: "treasury.openChannel".to_string(),
            operation_ref: local_admin_operation_ref("open_channel", idempotency_key),
            status: "pending".to_string(),
            payment_id: None,
            channel_id: Some(channel_id),
            peer_node_id: Some(peer_node_id),
        })
    }

    pub async fn close_channel(
        &self,
        channel_id: &str,
        peer_node_id: Option<&str>,
        force: bool,
        idempotency_key: &str,
    ) -> LdkServerClientResult<LdkServerAdminReceipt> {
        let channel_id = normalize_required_ldk_field(channel_id, "channel_id_missing")?;
        if self.mode == LdkServerClientMode::RemoteGrpc {
            let remote = self.remote_client()?;
            let peer_node_id = match peer_node_id {
                Some(peer_node_id) if !peer_node_id.trim().is_empty() => peer_node_id.to_string(),
                _ => remote
                    .list_channels(RemoteListChannelsRequest {})
                    .await
                    .map_err(map_remote_ldk_error)?
                    .channels
                    .into_iter()
                    .find(|channel| {
                        channel.user_channel_id == channel_id || channel.channel_id == channel_id
                    })
                    .map(|channel| channel.counterparty_node_id)
                    .ok_or_else(|| {
                        LdkServerClientError::new(
                            LdkServerClientErrorKind::InvalidRequest,
                            "counterparty_node_id_not_found_for_channel",
                        )
                    })?,
            };
            if force {
                remote
                    .force_close_channel(RemoteForceCloseChannelRequest {
                        user_channel_id: channel_id.clone(),
                        counterparty_node_id: peer_node_id,
                        force_close_reason: Some("operator_requested_force_close".to_string()),
                    })
                    .await
                    .map_err(map_remote_ldk_error)?;
            } else {
                remote
                    .close_channel(RemoteCloseChannelRequest {
                        user_channel_id: channel_id.clone(),
                        counterparty_node_id: peer_node_id,
                    })
                    .await
                    .map_err(map_remote_ldk_error)?;
            }
            return Ok(LdkServerAdminReceipt {
                command: "treasury.closeChannel".to_string(),
                operation_ref: local_admin_operation_ref("close_channel", idempotency_key),
                status: "pending".to_string(),
                payment_id: None,
                channel_id: Some(channel_id),
                peer_node_id: None,
            });
        }
        let mut channels = self.local_channels()?;
        let channel = channels
            .entry(channel_id.clone())
            .or_insert_with(|| LdkServerChannel {
                channel_id: channel_id.clone(),
                peer_node_id: "unknown-peer".to_string(),
                status: "unknown".to_string(),
                outbound_capacity_sats: 0,
                inbound_capacity_sats: 0,
            });
        channel.status = if force { "force_closing" } else { "closing" }.to_string();
        Ok(LdkServerAdminReceipt {
            command: "treasury.closeChannel".to_string(),
            operation_ref: local_admin_operation_ref("close_channel", idempotency_key),
            status: "pending".to_string(),
            payment_id: None,
            channel_id: Some(channel_id),
            peer_node_id: None,
        })
    }

    pub async fn splice_channel(
        &self,
        command: &str,
        channel_id: &str,
        peer_node_id: Option<&str>,
        amount_sats: u64,
        idempotency_key: &str,
    ) -> LdkServerClientResult<LdkServerAdminReceipt> {
        let channel_id = normalize_required_ldk_field(channel_id, "channel_id_missing")?;
        if amount_sats == 0 {
            return Err(LdkServerClientError::new(
                LdkServerClientErrorKind::InvalidRequest,
                "splice_amount_sats_must_be_greater_than_zero",
            ));
        }
        if self.mode == LdkServerClientMode::RemoteGrpc {
            let remote = self.remote_client()?;
            let peer_node_id = match peer_node_id {
                Some(peer_node_id) if !peer_node_id.trim().is_empty() => peer_node_id.to_string(),
                _ => remote
                    .list_channels(RemoteListChannelsRequest {})
                    .await
                    .map_err(map_remote_ldk_error)?
                    .channels
                    .into_iter()
                    .find(|channel| {
                        channel.user_channel_id == channel_id || channel.channel_id == channel_id
                    })
                    .map(|channel| channel.counterparty_node_id)
                    .ok_or_else(|| {
                        LdkServerClientError::new(
                            LdkServerClientErrorKind::InvalidRequest,
                            "counterparty_node_id_not_found_for_channel",
                        )
                    })?,
            };
            let status = if command == "SpliceIn" {
                remote
                    .splice_in(RemoteSpliceInRequest {
                        user_channel_id: channel_id.clone(),
                        counterparty_node_id: peer_node_id.clone(),
                        splice_amount_sats: amount_sats,
                    })
                    .await
                    .map(|_| "pending".to_string())
            } else {
                remote
                    .splice_out(RemoteSpliceOutRequest {
                        user_channel_id: channel_id.clone(),
                        counterparty_node_id: peer_node_id.clone(),
                        address: None,
                        splice_amount_sats: amount_sats,
                    })
                    .await
                    .map(|_| "pending".to_string())
            }
            .map_err(map_remote_ldk_error)?;
            return Ok(LdkServerAdminReceipt {
                command: if command == "SpliceIn" {
                    "treasury.spliceIn"
                } else {
                    "treasury.spliceOut"
                }
                .to_string(),
                operation_ref: local_admin_operation_ref(command, idempotency_key),
                status,
                payment_id: None,
                channel_id: Some(channel_id),
                peer_node_id: Some(peer_node_id),
            });
        }
        let mut channels = self.local_channels()?;
        let channel = channels
            .entry(channel_id.clone())
            .or_insert_with(|| LdkServerChannel {
                channel_id: channel_id.clone(),
                peer_node_id: "unknown-peer".to_string(),
                status: "open".to_string(),
                outbound_capacity_sats: 0,
                inbound_capacity_sats: 0,
            });
        if command == "SpliceIn" {
            channel.outbound_capacity_sats =
                channel.outbound_capacity_sats.saturating_add(amount_sats);
        } else {
            channel.outbound_capacity_sats =
                channel.outbound_capacity_sats.saturating_sub(amount_sats);
        }
        Ok(LdkServerAdminReceipt {
            command: if command == "SpliceIn" {
                "treasury.spliceIn"
            } else {
                "treasury.spliceOut"
            }
            .to_string(),
            operation_ref: local_admin_operation_ref(command, idempotency_key),
            status: "pending".to_string(),
            payment_id: None,
            channel_id: Some(channel_id),
            peer_node_id: None,
        })
    }

    pub async fn pay_invoice(
        &self,
        invoice: &str,
        amount_sats: Option<u64>,
        idempotency_key: &str,
    ) -> LdkServerClientResult<LdkServerAdminReceipt> {
        let invoice = normalize_required_ldk_field(invoice, "invoice_missing")?;
        if self.mode == LdkServerClientMode::RemoteGrpc {
            let response = self
                .remote_client()?
                .bolt11_send(RemoteBolt11SendRequest {
                    invoice,
                    amount_msat: amount_sats.map(|amount| amount.saturating_mul(1_000)),
                    route_parameters: None,
                })
                .await
                .map_err(map_remote_ldk_error)?;
            return Ok(LdkServerAdminReceipt {
                command: "treasury.payInvoice".to_string(),
                operation_ref: local_admin_operation_ref("pay_invoice", idempotency_key),
                status: "completed".to_string(),
                payment_id: Some(response.payment_id),
                channel_id: None,
                peer_node_id: None,
            });
        }
        let lowered = invoice.to_ascii_lowercase();
        if lowered.contains("noroute") {
            return Err(LdkServerClientError::new(
                LdkServerClientErrorKind::NoRoute,
                "local_harness_no_route_fixture",
            ));
        }
        if lowered.contains("insufficient") {
            return Err(LdkServerClientError::new(
                LdkServerClientErrorKind::InsufficientBalance,
                "local_harness_insufficient_balance_fixture",
            ));
        }
        let digest = short_hash(&format!("{invoice}:{amount_sats:?}:{idempotency_key}"));
        let payment_id = format!("ldk-local-payment-{digest}");
        let payment_hash = format!("ldk-local-payhash-{digest}");
        self.local_payments()?.insert(
            payment_id.clone(),
            LdkServerPayment {
                payment_id: payment_id.clone(),
                payment_hash,
                amount_msat: amount_sats.map(|amount| amount.saturating_mul(1_000)),
                status: LdkServerPaymentStatus::Succeeded,
            },
        );
        Ok(LdkServerAdminReceipt {
            command: "treasury.payInvoice".to_string(),
            operation_ref: local_admin_operation_ref("pay_invoice", idempotency_key),
            status: "completed".to_string(),
            payment_id: Some(payment_id),
            channel_id: None,
            peer_node_id: None,
        })
    }

    pub async fn pay_offer(
        &self,
        offer: &str,
        amount_sats: Option<u64>,
        idempotency_key: &str,
    ) -> LdkServerClientResult<LdkServerAdminReceipt> {
        if self.mode == LdkServerClientMode::RemoteGrpc {
            let offer = normalize_required_ldk_field(offer, "offer_missing")?;
            let response = self
                .remote_client()?
                .bolt12_send(RemoteBolt12SendRequest {
                    offer,
                    amount_msat: amount_sats.map(|amount| amount.saturating_mul(1_000)),
                    quantity: None,
                    payer_note: None,
                    route_parameters: None,
                })
                .await
                .map_err(map_remote_ldk_error)?;
            return Ok(LdkServerAdminReceipt {
                command: "treasury.payOffer".to_string(),
                operation_ref: local_admin_operation_ref("pay_offer", idempotency_key),
                status: "completed".to_string(),
                payment_id: Some(response.payment_id),
                channel_id: None,
                peer_node_id: None,
            });
        }
        let receipt = self
            .pay_invoice(offer, amount_sats, idempotency_key)
            .await?;
        Ok(LdkServerAdminReceipt {
            command: "treasury.payOffer".to_string(),
            operation_ref: local_admin_operation_ref("pay_offer", idempotency_key),
            ..receipt
        })
    }

    pub async fn subscribe_events(
        &self,
        after_sequence: u64,
    ) -> LdkServerClientResult<Vec<LdkServerEvent>> {
        if self.mode == LdkServerClientMode::RemoteGrpc {
            let payments = self.list_payments().await?;
            return Ok(reconcile_ldk_payments_to_events(&payments, after_sequence));
        }
        if after_sequence > 10_000 {
            return Err(LdkServerClientError::new(
                LdkServerClientErrorKind::StaleEventStream,
                "event_stream_sequence_too_old",
            ));
        }
        let mut sequence = after_sequence.saturating_add(1);
        let mut events = Vec::new();
        let mut payments = self.local_payments()?;
        for payment in payments.values_mut() {
            events.push(LdkServerEvent {
                sequence,
                kind: LdkServerEventKind::PaymentReceived,
                payment_id: Some(payment.payment_id.clone()),
                amount_msat: payment.amount_msat,
                detail: "local_harness_payment_received".to_string(),
            });
            sequence = sequence.saturating_add(1);
            payment.status = LdkServerPaymentStatus::Succeeded;
            events.push(LdkServerEvent {
                sequence,
                kind: LdkServerEventKind::PaymentSuccessful,
                payment_id: Some(payment.payment_id.clone()),
                amount_msat: payment.amount_msat,
                detail: "local_harness_payment_successful".to_string(),
            });
            sequence = sequence.saturating_add(1);
        }
        Ok(events)
    }

    fn local_total_sats(&self) -> LdkServerClientResult<u64> {
        Ok(self
            .local_payments()?
            .values()
            .filter(|payment| payment.status == LdkServerPaymentStatus::Succeeded)
            .filter_map(|payment| payment.amount_msat)
            .map(|amount_msat| amount_msat / 1_000)
            .sum())
    }

    fn local_payments(
        &self,
    ) -> LdkServerClientResult<std::sync::MutexGuard<'_, BTreeMap<String, LdkServerPayment>>> {
        self.local_state.lock().map_err(|_| {
            LdkServerClientError::new(
                LdkServerClientErrorKind::Internal,
                "local_harness_state_lock_poisoned",
            )
        })
    }

    fn local_peers(
        &self,
    ) -> LdkServerClientResult<std::sync::MutexGuard<'_, BTreeMap<String, LdkServerPeer>>> {
        self.local_peers.lock().map_err(|_| {
            LdkServerClientError::new(
                LdkServerClientErrorKind::Internal,
                "local_harness_peer_lock_poisoned",
            )
        })
    }

    fn local_channels(
        &self,
    ) -> LdkServerClientResult<std::sync::MutexGuard<'_, BTreeMap<String, LdkServerChannel>>> {
        self.local_channels.lock().map_err(|_| {
            LdkServerClientError::new(
                LdkServerClientErrorKind::Internal,
                "local_harness_channel_lock_poisoned",
            )
        })
    }
}

fn normalize_required_ldk_field(value: &str, reason: &str) -> LdkServerClientResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(LdkServerClientError::new(
            LdkServerClientErrorKind::InvalidRequest,
            reason,
        ));
    }
    Ok(trimmed.to_string())
}

fn local_admin_operation_ref(command: &str, idempotency_key: &str) -> String {
    format!(
        "ldk-local-admin-{}",
        short_hash(&format!("{command}:{idempotency_key}"))
    )
}

pub fn project_ldk_event_to_operation(event: &LdkServerEvent) -> LdkServerProjectedOperation {
    let provider_payment_id_hash = event.payment_id.as_deref().map(short_hash);
    let mut rail_metadata = BTreeMap::new();
    rail_metadata.insert("event_kind".to_string(), format!("{:?}", event.kind));
    rail_metadata.insert("event_sequence".to_string(), event.sequence.to_string());
    if !event.detail.is_empty() {
        rail_metadata.insert("detail".to_string(), event.detail.clone());
    }
    let operation_id = format!(
        "ldk-event-{}-{}",
        event.sequence,
        provider_payment_id_hash.as_deref().unwrap_or("none")
    );
    LdkServerProjectedOperation {
        operation_id,
        kind: "event_projection".to_string(),
        rail: "ldk".to_string(),
        rail_metadata,
        amount_msat: event.amount_msat,
        status: event.kind.operation_status().to_string(),
        provider_payment_id_hash,
        terminal_event_state: match event.kind {
            LdkServerEventKind::PaymentSuccessful => Some("payment_successful".to_string()),
            LdkServerEventKind::PaymentFailed => Some("payment_failed".to_string()),
            LdkServerEventKind::EventStreamDisconnected => {
                Some("event_stream_disconnected".to_string())
            }
            LdkServerEventKind::Bolt11InvoiceCreated | LdkServerEventKind::PaymentReceived => None,
        },
    }
}

pub fn reconcile_ldk_payments_to_events(
    payments: &[LdkServerPayment],
    after_sequence: u64,
) -> Vec<LdkServerEvent> {
    let mut sequence = after_sequence;
    payments
        .iter()
        .filter_map(|payment| {
            sequence = sequence.saturating_add(1);
            let kind = match payment.status {
                LdkServerPaymentStatus::Pending => LdkServerEventKind::PaymentReceived,
                LdkServerPaymentStatus::Succeeded => LdkServerEventKind::PaymentSuccessful,
                LdkServerPaymentStatus::Failed => LdkServerEventKind::PaymentFailed,
                LdkServerPaymentStatus::Unknown => return None,
            };
            Some(LdkServerEvent {
                sequence,
                kind,
                payment_id: Some(payment.payment_id.clone()),
                amount_msat: payment.amount_msat,
                detail: "list_payments_reconciliation".to_string(),
            })
        })
        .collect()
}

fn load_ldk_api_key(path: &Path) -> LdkServerClientResult<Vec<u8>> {
    let bytes = fs::read(path).map_err(|error| {
        LdkServerClientError::new(
            LdkServerClientErrorKind::InvalidConfig,
            format!("api_key_read_failed:{error}"),
        )
    })?;
    let trimmed = String::from_utf8_lossy(&bytes).trim().as_bytes().to_vec();
    if trimmed.is_empty() {
        return Err(LdkServerClientError::new(
            LdkServerClientErrorKind::InvalidConfig,
            "api_key_empty",
        ));
    }
    if trimmed.len() == 64 && trimmed.iter().all(u8::is_ascii_hexdigit) {
        return Ok(trimmed.to_ascii_lowercase());
    }
    Ok(hex::encode(bytes).into_bytes())
}

fn validate_remote_server_url(server_url: &str) -> LdkServerClientResult<()> {
    let trimmed = server_url.trim();
    if trimmed.is_empty() {
        return Err(LdkServerClientError::new(
            LdkServerClientErrorKind::InvalidConfig,
            "ldk_server_url_empty",
        ));
    }
    if trimmed.contains("://") {
        return Err(LdkServerClientError::new(
            LdkServerClientErrorKind::InvalidConfig,
            "ldk_server_url_must_be_host_port_without_scheme",
        ));
    }
    Ok(())
}

fn map_remote_ldk_error(error: RemoteLdkServerError) -> LdkServerClientError {
    let reason = sanitize_error_reason(error.message.as_str());
    let kind = match error.error_code {
        LdkServerErrorCode::InvalidRequestError => LdkServerClientErrorKind::InvalidRequest,
        LdkServerErrorCode::AuthError => LdkServerClientErrorKind::Auth,
        LdkServerErrorCode::LightningError => classify_remote_lightning_error(reason.as_str()),
        LdkServerErrorCode::InternalServerError => LdkServerClientErrorKind::Internal,
        LdkServerErrorCode::InternalError => LdkServerClientErrorKind::Unavailable,
    };
    LdkServerClientError::new(kind, reason)
}

fn classify_remote_lightning_error(reason: &str) -> LdkServerClientErrorKind {
    let lowered = reason.to_ascii_lowercase();
    if lowered.contains("no_route") || lowered.contains("route_not_found") {
        LdkServerClientErrorKind::NoRoute
    } else if lowered.contains("insufficient") || lowered.contains("not_enough") {
        LdkServerClientErrorKind::InsufficientBalance
    } else {
        LdkServerClientErrorKind::PaymentFailed
    }
}

fn map_remote_network(remote_network: i32, fallback: LdkNetwork) -> LdkNetwork {
    match remote_network {
        value if value == RemoteNetwork::Bitcoin as i32 => LdkNetwork::Bitcoin,
        value if value == RemoteNetwork::Signet as i32 => LdkNetwork::Signet,
        value if value == RemoteNetwork::Regtest as i32 => LdkNetwork::Regtest,
        _ => fallback,
    }
}

fn map_remote_payment(payment: &RemotePayment) -> LdkServerPayment {
    LdkServerPayment {
        payment_id: payment.id.clone(),
        payment_hash: remote_payment_hash(payment),
        amount_msat: payment.amount_msat,
        status: map_remote_payment_status(payment.status),
    }
}

fn remote_payment_hash(payment: &RemotePayment) -> String {
    let Some(kind) = payment.kind.as_ref().and_then(|kind| kind.kind.as_ref()) else {
        return String::new();
    };
    match kind {
        remote_payment_kind::Kind::Bolt11(payment) => payment.hash.clone(),
        remote_payment_kind::Kind::Bolt11Jit(payment) => payment.hash.clone(),
        remote_payment_kind::Kind::Bolt12Offer(payment) => payment.hash.clone().unwrap_or_default(),
        remote_payment_kind::Kind::Bolt12Refund(payment) => {
            payment.hash.clone().unwrap_or_default()
        }
        _ => String::new(),
    }
}

fn map_remote_payment_status(status: i32) -> LdkServerPaymentStatus {
    match status {
        value if value == RemotePaymentStatus::Pending as i32 => LdkServerPaymentStatus::Pending,
        value if value == RemotePaymentStatus::Succeeded as i32 => {
            LdkServerPaymentStatus::Succeeded
        }
        value if value == RemotePaymentStatus::Failed as i32 => LdkServerPaymentStatus::Failed,
        _ => LdkServerPaymentStatus::Unknown,
    }
}

fn map_remote_channel(channel: &RemoteChannel) -> LdkServerChannel {
    LdkServerChannel {
        channel_id: if channel.user_channel_id.is_empty() {
            channel.channel_id.clone()
        } else {
            channel.user_channel_id.clone()
        },
        peer_node_id: channel.counterparty_node_id.clone(),
        status: if channel.is_usable {
            "usable"
        } else if channel.is_channel_ready {
            "ready"
        } else {
            "pending"
        }
        .to_string(),
        outbound_capacity_sats: channel.outbound_capacity_msat / 1_000,
        inbound_capacity_sats: channel.inbound_capacity_msat / 1_000,
    }
}

fn map_remote_peer(peer: &RemotePeer) -> LdkServerPeer {
    LdkServerPeer {
        node_id: peer.node_id.clone(),
        address_hash: if peer.address.trim().is_empty() {
            None
        } else {
            Some(short_hash(peer.address.as_str()))
        },
        connected: peer.is_connected,
    }
}

fn sanitize_error_reason(reason: &str) -> String {
    reason
        .split_whitespace()
        .take(32)
        .collect::<Vec<_>>()
        .join("_")
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | ':' | '.')
        })
        .collect()
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreasuryLightningProviderConfig {
    pub provider: TreasuryLightningProviderKind,
    pub ldk: LdkTreasuryProviderConfig,
}

impl TreasuryLightningProviderConfig {
    pub fn new(
        provider: TreasuryLightningProviderKind,
        ldk: LdkTreasuryProviderConfig,
    ) -> Result<Self, String> {
        Ok(Self { provider, ldk })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryProviderFundingRequest {
    pub amount_sats: Option<u64>,
    pub description: Option<String>,
    pub expiry_seconds: Option<u32>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryProviderFundingTarget {
    pub provider_target: String,
    pub bitcoin_address: String,
    pub bolt11_invoice: Option<String>,
    pub provider_invoice: Option<String>,
    pub balance_sats: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryProviderPayoutRequest {
    pub payout_key: String,
    pub payment_target_kind: String,
    pub payment_request: String,
    pub amount_sats: u64,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryProviderPayoutReceipt {
    pub payment_id: String,
    pub terminal_event_state: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreasuryProviderErrorKind {
    Disabled,
    InvalidConfig,
    InvalidRequest,
    NoRoute,
    InsufficientBalance,
    StaleTarget,
    ProviderUnavailable,
    Unavailable,
    Failed,
}

impl TreasuryProviderErrorKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::InvalidConfig => "invalid_config",
            Self::InvalidRequest => "invalid_request",
            Self::NoRoute => "no_route",
            Self::InsufficientBalance => "insufficient_balance",
            Self::StaleTarget => "stale_target",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::Unavailable => "unavailable",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryProviderError {
    pub provider: TreasuryLightningProviderKind,
    pub kind: TreasuryProviderErrorKind,
    pub reason: String,
}

impl TreasuryProviderError {
    pub fn new(
        provider: TreasuryLightningProviderKind,
        kind: TreasuryProviderErrorKind,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            kind,
            reason: reason.into(),
        }
    }

    pub fn normalized_reason(&self) -> String {
        format!(
            "treasury_provider_error:{}:{}:{}",
            self.provider.as_str(),
            self.kind.as_str(),
            self.reason
        )
    }
}

impl fmt::Display for TreasuryProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.normalized_reason().as_str())
    }
}

impl std::error::Error for TreasuryProviderError {}

pub trait TreasuryLightningProvider: Send + Sync {
    fn provider_kind(&self) -> TreasuryLightningProviderKind;

    fn create_funding_target<'a>(
        &'a self,
        request: TreasuryProviderFundingRequest,
    ) -> BoxFuture<'a, TreasuryProviderResult<TreasuryProviderFundingTarget>>;

    fn dispatch_payout<'a>(
        &'a self,
        request: TreasuryProviderPayoutRequest,
    ) -> BoxFuture<'a, TreasuryProviderResult<TreasuryProviderPayoutReceipt>>;
}

#[derive(Debug, Clone)]
pub struct LdkTreasuryProvider {
    config: LdkTreasuryProviderConfig,
}

impl LdkTreasuryProvider {
    pub fn new(config: LdkTreasuryProviderConfig) -> Self {
        Self { config }
    }

    fn map_ldk_error(error: LdkServerClientError) -> TreasuryProviderError {
        let kind = match error.kind {
            LdkServerClientErrorKind::InvalidConfig => TreasuryProviderErrorKind::InvalidConfig,
            LdkServerClientErrorKind::InvalidRequest => TreasuryProviderErrorKind::InvalidRequest,
            LdkServerClientErrorKind::Unavailable | LdkServerClientErrorKind::StaleEventStream => {
                TreasuryProviderErrorKind::ProviderUnavailable
            }
            LdkServerClientErrorKind::NoRoute => TreasuryProviderErrorKind::NoRoute,
            LdkServerClientErrorKind::InsufficientBalance => {
                TreasuryProviderErrorKind::InsufficientBalance
            }
            LdkServerClientErrorKind::Auth
            | LdkServerClientErrorKind::MalformedResponse
            | LdkServerClientErrorKind::PaymentFailed
            | LdkServerClientErrorKind::Internal => TreasuryProviderErrorKind::Failed,
        };
        TreasuryProviderError::new(
            TreasuryLightningProviderKind::Ldk,
            kind,
            error.normalized_reason(),
        )
    }
}

fn infer_ldk_payment_target_kind(explicit_kind: &str, payment_request: &str) -> String {
    let explicit_kind = explicit_kind.trim();
    if !explicit_kind.is_empty() {
        return explicit_kind.to_ascii_lowercase();
    }
    let target = payment_request.trim().to_ascii_lowercase();
    if target.starts_with("lno") {
        "bolt12_offer".to_string()
    } else if target.starts_with("lnurl") {
        "lnurl_pay".to_string()
    } else if target.starts_with("lnbc")
        || target.starts_with("lntb")
        || target.starts_with("lnbcrt")
        || target.starts_with("lntbs")
    {
        "bolt11_invoice".to_string()
    } else if target.contains('@') {
        "bip353_name".to_string()
    } else {
        "unknown".to_string()
    }
}

fn is_ldk_payout_target_kind(kind: &str) -> bool {
    matches!(
        kind,
        "bolt12_offer" | "bolt11_invoice" | "bip353_name" | "lnurl_pay"
    )
}

impl TreasuryLightningProvider for LdkTreasuryProvider {
    fn provider_kind(&self) -> TreasuryLightningProviderKind {
        TreasuryLightningProviderKind::Ldk
    }

    fn create_funding_target<'a>(
        &'a self,
        request: TreasuryProviderFundingRequest,
    ) -> BoxFuture<'a, TreasuryProviderResult<TreasuryProviderFundingTarget>> {
        Box::pin(async move {
            if matches!(request.amount_sats, Some(0)) {
                return Err(TreasuryProviderError::new(
                    TreasuryLightningProviderKind::Ldk,
                    TreasuryProviderErrorKind::InvalidRequest,
                    "funding_amount_must_be_greater_than_zero",
                ));
            }
            let client =
                LdkServerClient::from_provider_config(&self.config).map_err(Self::map_ldk_error)?;
            let receive = client
                .bolt11_receive(LdkServerBolt11ReceiveRequest {
                    amount_sats: request.amount_sats,
                    description: request
                        .description
                        .clone()
                        .unwrap_or_else(|| "Nexus treasury funding".to_string()),
                    expiry_seconds: request.expiry_seconds,
                    idempotency_key: request.idempotency_key.clone(),
                })
                .await
                .map_err(Self::map_ldk_error)?;
            let digest = short_hash(receive.payment_id.as_str());
            Ok(TreasuryProviderFundingTarget {
                provider_target: format!(
                    "ldk://server/{}/{}/{}",
                    self.config.network.as_str(),
                    self.config.chain_backend.as_str(),
                    digest
                ),
                bitcoin_address: String::new(),
                bolt11_invoice: Some(receive.invoice),
                provider_invoice: Some(receive.payment_id),
                balance_sats: 0,
            })
        })
    }

    fn dispatch_payout<'a>(
        &'a self,
        request: TreasuryProviderPayoutRequest,
    ) -> BoxFuture<'a, TreasuryProviderResult<TreasuryProviderPayoutReceipt>> {
        Box::pin(async move {
            if request.amount_sats == 0 {
                return Err(TreasuryProviderError::new(
                    TreasuryLightningProviderKind::Ldk,
                    TreasuryProviderErrorKind::InvalidRequest,
                    "payout_amount_must_be_greater_than_zero",
                ));
            }
            if request.payment_request.trim().is_empty() {
                return Err(TreasuryProviderError::new(
                    TreasuryLightningProviderKind::Ldk,
                    TreasuryProviderErrorKind::InvalidRequest,
                    "payment_request_missing",
                ));
            }
            let payment_target_kind = infer_ldk_payment_target_kind(
                request.payment_target_kind.as_str(),
                request.payment_request.as_str(),
            );
            if !is_ldk_payout_target_kind(payment_target_kind.as_str()) {
                return Err(TreasuryProviderError::new(
                    TreasuryLightningProviderKind::Ldk,
                    TreasuryProviderErrorKind::InvalidRequest,
                    format!("unsupported_ldk_payment_target_kind:{payment_target_kind}"),
                ));
            }
            let lower_target = request.payment_request.to_ascii_lowercase();
            if lower_target.contains("no-route-fixture") {
                return Err(TreasuryProviderError::new(
                    TreasuryLightningProviderKind::Ldk,
                    TreasuryProviderErrorKind::NoRoute,
                    "local_harness_no_route_fixture",
                ));
            }
            if lower_target.contains("insufficient-balance-fixture") {
                return Err(TreasuryProviderError::new(
                    TreasuryLightningProviderKind::Ldk,
                    TreasuryProviderErrorKind::InsufficientBalance,
                    "local_harness_insufficient_balance_fixture",
                ));
            }
            if lower_target.contains("provider-unavailable-fixture") {
                return Err(TreasuryProviderError::new(
                    TreasuryLightningProviderKind::Ldk,
                    TreasuryProviderErrorKind::ProviderUnavailable,
                    "local_harness_provider_unavailable_fixture",
                ));
            }
            Ok(TreasuryProviderPayoutReceipt {
                payment_id: format!("ldk-local-payment-{}", short_hash(&request.idempotency_key)),
                terminal_event_state: Some("completed".to_string()),
            })
        })
    }
}

fn short_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex::encode(&digest[..8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_kind_parser_defaults_to_ldk() {
        assert_eq!(
            TreasuryLightningProviderKind::parse(None).expect("default"),
            TreasuryLightningProviderKind::Ldk
        );
        assert_eq!(
            TreasuryLightningProviderKind::parse(Some("ldk")).expect("ldk"),
            TreasuryLightningProviderKind::Ldk
        );
        assert!(
            TreasuryLightningProviderKind::parse(Some("legacy-provider"))
                .expect_err("plain legacy provider forbidden")
                .contains("supported: ldk")
        );
    }

    #[test]
    fn ldk_network_and_backend_parse_expected_values() {
        assert_eq!(
            LdkNetwork::parse(Some("signet")).expect("signet"),
            LdkNetwork::Signet
        );
        assert_eq!(
            LdkNetwork::parse(Some("mainnet")).expect("mainnet"),
            LdkNetwork::Bitcoin
        );
        assert_eq!(
            LdkChainBackend::parse(Some("esplora")).expect("esplora"),
            LdkChainBackend::Esplora
        );
    }

    #[tokio::test]
    async fn ldk_scaffold_is_deterministic_by_idempotency_key() {
        let provider = LdkTreasuryProvider::new(LdkTreasuryProviderConfig::local_scaffold(
            PathBuf::from("/tmp/ldk"),
        ));
        let request = TreasuryProviderFundingRequest {
            amount_sats: Some(21),
            description: Some("fund".to_string()),
            expiry_seconds: Some(60),
            idempotency_key: "same-key".to_string(),
        };
        let first = provider
            .create_funding_target(request.clone())
            .await
            .expect("first");
        let second = provider
            .create_funding_target(request)
            .await
            .expect("second");
        assert_eq!(first, second);
        assert!(
            first
                .provider_target
                .starts_with("ldk://server/regtest/bitcoind/")
        );
        assert_eq!(first.bitcoin_address, "");
        assert!(
            first
                .bolt11_invoice
                .as_deref()
                .unwrap_or_default()
                .starts_with("lnbcrt21")
        );
        assert!(
            first
                .provider_invoice
                .as_deref()
                .unwrap_or_default()
                .starts_with("ldk-local-payment-")
        );

        let payout = TreasuryProviderPayoutRequest {
            payout_key: "payout-a".to_string(),
            payment_target_kind: "bolt11_invoice".to_string(),
            payment_request: first.bolt11_invoice.expect("bolt11"),
            amount_sats: 21,
            idempotency_key: "payout-idempotency".to_string(),
        };
        let receipt = provider.dispatch_payout(payout).await.expect("dispatch");
        assert!(receipt.payment_id.starts_with("ldk-local-payment-"));
        assert_eq!(receipt.terminal_event_state.as_deref(), Some("completed"));
    }

    #[tokio::test]
    async fn ldk_scaffold_normalizes_provider_errors() {
        let provider = LdkTreasuryProvider::new(LdkTreasuryProviderConfig::local_scaffold(
            PathBuf::from("/tmp/ldk"),
        ));
        let error = provider
            .dispatch_payout(TreasuryProviderPayoutRequest {
                payout_key: "payout-a".to_string(),
                payment_target_kind: "bolt11_invoice".to_string(),
                payment_request: String::new(),
                amount_sats: 1,
                idempotency_key: "payout-idempotency".to_string(),
            })
            .await
            .expect_err("missing request");
        assert_eq!(error.provider, TreasuryLightningProviderKind::Ldk);
        assert_eq!(error.kind, TreasuryProviderErrorKind::InvalidRequest);
        assert_eq!(
            error.normalized_reason(),
            "treasury_provider_error:ldk:invalid_request:payment_request_missing"
        );
    }

    #[tokio::test]
    async fn ldk_scaffold_rejects_unsupported_targets_and_maps_typed_errors() {
        let provider = LdkTreasuryProvider::new(LdkTreasuryProviderConfig::local_scaffold(
            PathBuf::from("/tmp/ldk"),
        ));
        let unsupported_target = provider
            .dispatch_payout(TreasuryProviderPayoutRequest {
                payout_key: "payout-a".to_string(),
                payment_target_kind: "unknown_wallet_alias".to_string(),
                payment_request: "legacy:target".to_string(),
                amount_sats: 1,
                idempotency_key: "payout-idempotency".to_string(),
            })
            .await
            .expect_err("unsupported target rejected");
        assert_eq!(
            unsupported_target.kind,
            TreasuryProviderErrorKind::InvalidRequest
        );

        let no_route = provider
            .dispatch_payout(TreasuryProviderPayoutRequest {
                payout_key: "payout-b".to_string(),
                payment_target_kind: "bolt11_invoice".to_string(),
                payment_request: "lnbcrt1no-route-fixture".to_string(),
                amount_sats: 1,
                idempotency_key: "payout-idempotency-b".to_string(),
            })
            .await
            .expect_err("no route fixture rejected");
        assert_eq!(no_route.kind, TreasuryProviderErrorKind::NoRoute);

        let insufficient = provider
            .dispatch_payout(TreasuryProviderPayoutRequest {
                payout_key: "payout-c".to_string(),
                payment_target_kind: "bolt11_invoice".to_string(),
                payment_request: "lnbcrt1insufficient-balance-fixture".to_string(),
                amount_sats: 1,
                idempotency_key: "payout-idempotency-c".to_string(),
            })
            .await
            .expect_err("insufficient balance fixture rejected");
        assert_eq!(
            insufficient.kind,
            TreasuryProviderErrorKind::InsufficientBalance
        );
    }

    #[test]
    fn ldk_server_hmac_metadata_matches_reference_fixture() {
        assert_eq!(
            ldk_hmac_auth_metadata(b"test-api-key", 1_700_000_000),
            "HMAC 1700000000:022744bacd67d07dcc15b8887d48df1d5184d33b18d9447257ed2bf8ed61b937"
        );
    }

    #[test]
    fn ldk_server_tls_pin_loader_hashes_pem_without_exposing_material() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let cert_path = temp_dir.path().join("tls.crt");
        let cert = b"-----BEGIN CERTIFICATE-----\nMIIBlocaltest\n-----END CERTIFICATE-----\n";
        fs::write(&cert_path, cert).expect("write cert");

        let pin = load_ldk_tls_certificate_pin(&cert_path).expect("pin");
        assert_eq!(pin.path, cert_path);
        assert_eq!(pin.pem_len, cert.len());
        assert_eq!(pin.sha256_hex, hex::encode(Sha256::digest(cert)));
        assert!(!format!("{pin:?}").contains("MIIBlocaltest"));
    }

    #[test]
    fn ldk_server_remote_config_loads_hmac_and_tls_pin() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let key_path = temp_dir.path().join("api.key");
        let cert_path = temp_dir.path().join("tls.crt");
        fs::write(&key_path, b"test-api-key").expect("write key");
        fs::write(
            &cert_path,
            b"-----BEGIN CERTIFICATE-----\nMIIBlocaltest\n-----END CERTIFICATE-----\n",
        )
        .expect("write cert");

        let client = LdkServerClient::new(LdkServerClientConfig {
            server_url: Some("127.0.0.1:9735".to_string()),
            api_key_path: Some(key_path),
            tls_cert_path: Some(cert_path),
            storage_dir: temp_dir.path().join("ldk"),
            network: LdkNetwork::Regtest,
            chain_backend: LdkChainBackend::Bitcoind,
        })
        .expect("remote config");
        assert_eq!(client.mode(), LdkServerClientMode::RemoteGrpc);
        assert_eq!(
            client
                .current_hmac_metadata_for_test(1_700_000_000)
                .expect("metadata"),
            Some(ldk_hmac_auth_metadata(
                hex::encode(b"test-api-key").as_bytes(),
                1_700_000_000
            ))
        );
        assert!(!format!("{client:?}").contains("test-api-key"));
    }

    #[test]
    fn ldk_server_api_key_loader_hex_encodes_raw_server_key() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let key_path = temp_dir.path().join("api.key");
        fs::write(&key_path, [0xde, 0xad, 0xbe, 0xef]).expect("write key");

        let loaded = load_ldk_api_key(&key_path).expect("load key");

        assert_eq!(loaded, b"deadbeef");
    }

    #[test]
    fn ldk_server_api_key_loader_accepts_existing_hex_key() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let key_path = temp_dir.path().join("api.key");
        fs::write(
            &key_path,
            "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789\n",
        )
        .expect("write key");

        let loaded = load_ldk_api_key(&key_path).expect("load key");

        assert_eq!(
            loaded,
            b"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
        );
    }

    #[tokio::test]
    async fn ldk_server_local_client_covers_required_methods_and_reconciliation() {
        let client = LdkServerClient::new(LdkServerClientConfig::from(
            &LdkTreasuryProviderConfig::local_scaffold(PathBuf::from("/tmp/ldk")),
        ))
        .expect("client");
        assert_eq!(client.mode(), LdkServerClientMode::LocalHarness);

        let node = client.get_node_info().await.expect("node info");
        assert!(node.node_id.starts_with("02"));
        let before = client.get_balances().await.expect("balances before");
        assert_eq!(before.usable_sats, 0);

        let receive = client
            .bolt11_receive(LdkServerBolt11ReceiveRequest {
                amount_sats: Some(2_500),
                description: "Nexus treasury local proof".to_string(),
                expiry_seconds: Some(600),
                idempotency_key: "ldk-05-local-proof".to_string(),
            })
            .await
            .expect("invoice");
        assert!(receive.invoice.starts_with("lnbcrt2500nexus"));

        let pending = client
            .get_payment(receive.payment_id.as_str())
            .await
            .expect("pending payment");
        assert_eq!(pending.status, LdkServerPaymentStatus::Pending);

        let events = client.subscribe_events(0).await.expect("events");
        assert_eq!(
            events.iter().map(|event| event.kind).collect::<Vec<_>>(),
            vec![
                LdkServerEventKind::PaymentReceived,
                LdkServerEventKind::PaymentSuccessful
            ]
        );
        let projected = events
            .iter()
            .map(project_ldk_event_to_operation)
            .collect::<Vec<_>>();
        assert_eq!(projected[0].rail, "ldk");
        assert_eq!(projected[1].status, "completed");
        assert_eq!(
            projected[1].terminal_event_state.as_deref(),
            Some("payment_successful")
        );

        let payments = client.list_payments().await.expect("payments");
        assert_eq!(payments.len(), 1);
        assert_eq!(payments[0].status, LdkServerPaymentStatus::Succeeded);
        let reconciled = reconcile_ldk_payments_to_events(&payments, 99);
        assert_eq!(reconciled[0].sequence, 100);
        assert_eq!(reconciled[0].kind, LdkServerEventKind::PaymentSuccessful);

        let after = client.get_balances().await.expect("balances after");
        assert_eq!(after.usable_sats, 2_500);
    }

    #[tokio::test]
    async fn ldk_server_stale_event_stream_is_typed() {
        let client = LdkServerClient::new(LdkServerClientConfig::from(
            &LdkTreasuryProviderConfig::local_scaffold(PathBuf::from("/tmp/ldk")),
        ))
        .expect("client");
        let error = client
            .subscribe_events(10_001)
            .await
            .expect_err("stale stream");
        assert_eq!(error.kind, LdkServerClientErrorKind::StaleEventStream);
    }

    #[test]
    fn ldk_server_error_fixtures_are_normalized() {
        let cases = [
            (
                "UNAVAILABLE",
                "server unavailable",
                LdkServerClientErrorKind::Unavailable,
            ),
            (
                "FAILED_PRECONDITION",
                "no route found for invoice",
                LdkServerClientErrorKind::NoRoute,
            ),
            (
                "FAILED_PRECONDITION",
                "insufficient balance",
                LdkServerClientErrorKind::InsufficientBalance,
            ),
            (
                "INTERNAL",
                "event stream lagged behind",
                LdkServerClientErrorKind::StaleEventStream,
            ),
            (
                "MALFORMED",
                "malformed response body",
                LdkServerClientErrorKind::MalformedResponse,
            ),
        ];

        for (status, message, expected) in cases {
            let error = LdkServerClientError::from_service_fixture(status, message);
            assert_eq!(error.kind, expected);
            assert!(!error.normalized_reason().contains(' '));
        }
    }
}
