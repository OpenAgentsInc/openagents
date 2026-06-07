use std::collections::BTreeSet;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow, bail};
use bip39::{Language, Mnemonic};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use ldk_node::Builder as LdkNodeBuilder;
use ldk_node::bitcoin::{Address as LdkBitcoinAddress, Network as LdkBitcoinNetwork};
use ldk_node::lightning::ln::channelmanager::PaymentId as LdkPaymentId;
use ldk_node::lightning::offers::offer::{Amount as LdkOfferAmount, Offer as LdkOffer};
use ldk_node::lightning_invoice::{Bolt11Invoice, Bolt11InvoiceDescription, Description};
use ldk_node::payment::{
    PaymentDetails as LdkPaymentDetails, PaymentDirection as LdkPaymentDirection,
    PaymentKind as LdkPaymentKind, PaymentStatus as LdkPaymentStatus,
};
use scrypt::{Params as ScryptParams, scrypt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    PylonLedger, PylonWalletCreditSummary, PylonWalletInvoiceRecord, PylonWalletPaymentRecord,
    PylonWalletReceiptRecord, ensure_local_setup, load_ledger, mutate_ledger, now_epoch_ms,
};

type HmacSha256 = Hmac<Sha256>;

const LDK_EXTERNAL_WALLET_DETAIL: &str = "Pylon is using an advanced external_payout_target override. The built-in LDK wallet is the default earnings path.";
const MOCK_WALLET_DETAIL: &str = "Pylon is using the deterministic mock wallet runtime for tests.";
const WALLET_NODE_ENTROPY_DERIVATION_VERSION: &str = "pylon-ldk-node-entropy-v1";
const WALLET_NODE_ENTROPY_LABEL_PREFIX: &str = "openagents-pylon/ldk-node/v1";
const WALLET_NODE_ENTROPY_HKDF_SALT: &[u8] = b"openagents-pylon/ldk-node/node-entropy";
const WALLET_STORAGE_SCHEMA_VERSION: u32 = 1;
const WALLET_BACKUP_SCHEMA_VERSION: u32 = 1;
const WALLET_BACKUP_KIND: &str = "pylon.wallet.backup.encrypted.v1";
const WALLET_BACKUP_PLAINTEXT_KIND: &str = "pylon.wallet.backup.plaintext.v1";
const WALLET_BACKUP_MANIFEST_KIND: &str = "pylon.wallet.backup_manifest";
const WALLET_BACKUP_DEFAULT_PASSPHRASE_ENV: &str = "PYLON_WALLET_BACKUP_PASSPHRASE";
const WALLET_BACKUP_STALE_AFTER_MS: u64 = 7 * 24 * 60 * 60 * 1_000;
const WALLET_BACKUP_SCRYPT_LOG_N: u8 = 15;
const WALLET_BACKUP_SCRYPT_R: u32 = 8;
const WALLET_BACKUP_SCRYPT_P: u32 = 1;
const WALLET_BACKUP_KEY_LEN: usize = 32;
const DEFAULT_BOLT11_EXPIRY_SECONDS: u32 = 3_600;
const DEFAULT_RECEIVE_DESCRIPTION: &str = "OpenAgents Pylon receive";
const SATS_PER_BTC: u64 = 100_000_000;
const MONEYDEVKIT_AGENT_WALLET_PACKAGE: &str = "@moneydevkit/agent-wallet@latest";
const MONEYDEVKIT_WALLET_DETAIL: &str = "Pylon is using MoneyDevKit's local agent-wallet runtime for self-custodial Lightning payments.";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonWalletRuntimeKind {
    #[default]
    ExternalTarget,
    Mock,
    LdkNode,
    #[serde(rename = "moneydevkit", alias = "money_dev_kit", alias = "mdk")]
    MoneyDevKit,
}

impl PylonWalletRuntimeKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::ExternalTarget => "external_target",
            Self::Mock => "mock",
            Self::LdkNode => "ldk_node",
            Self::MoneyDevKit => "moneydevkit",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "external_target" | "external" | "ldk_external" => Ok(Self::ExternalTarget),
            "mock" => Ok(Self::Mock),
            "ldk_node" | "ldknode" => Ok(Self::LdkNode),
            "moneydevkit" | "money_dev_kit" | "mdk" | "agent_wallet" | "mdk_agent_wallet" => {
                Ok(Self::MoneyDevKit)
            }
            other => bail!(
                "unsupported wallet_runtime_kind '{other}'; expected external_target, mock, ldk_node, or moneydevkit"
            ),
        }
    }
}

impl std::fmt::Display for PylonWalletRuntimeKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonWalletLiquidityProviderKind {
    #[default]
    None,
    #[serde(rename = "moneydevkit", alias = "money_dev_kit", alias = "mdk")]
    MoneyDevKit,
    CustomLsps,
}

impl PylonWalletLiquidityProviderKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::MoneyDevKit => "moneydevkit",
            Self::CustomLsps => "custom_lsps",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "" | "none" | "off" | "disabled" => Ok(Self::None),
            "moneydevkit" | "money_dev_kit" | "mdk" => Ok(Self::MoneyDevKit),
            "custom_lsps" | "custom_lsp" | "lsps" | "lsps1" | "lsps2" | "lsps4" => {
                Ok(Self::CustomLsps)
            }
            other => bail!(
                "unsupported wallet_liquidity_provider_kind '{other}'; expected none, moneydevkit, or custom_lsps"
            ),
        }
    }
}

impl std::fmt::Display for PylonWalletLiquidityProviderKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WalletSubcommand {
    Status {
        json: bool,
    },
    Sync {
        json: bool,
    },
    Balance {
        json: bool,
    },
    Channels {
        json: bool,
    },
    Address {
        json: bool,
    },
    Invoice {
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u32>,
        json: bool,
    },
    Offer {
        amount_sats: Option<u64>,
        description: Option<String>,
        expiry_seconds: Option<u32>,
        json: bool,
    },
    Pay {
        payment_request: String,
        amount_sats: Option<u64>,
        yes: bool,
        json: bool,
    },
    Telemetry {
        json: bool,
    },
    History {
        limit: Option<u32>,
        json: bool,
    },
    BackupExport {
        path: PathBuf,
        passphrase_env: Option<String>,
        include_identity_mnemonic: bool,
        json: bool,
    },
    BackupInspect {
        path: PathBuf,
        json: bool,
    },
    RestorePhrase {
        mnemonic_env: Option<String>,
        mnemonic_file: Option<PathBuf>,
        wallet_network: Option<String>,
        yes: bool,
        json: bool,
    },
    RestoreBackup {
        path: PathBuf,
        passphrase_env: Option<String>,
        yes: bool,
        json: bool,
    },
    EntropyStatus {
        json: bool,
    },
    EntropyExport {
        path: PathBuf,
        json: bool,
    },
    EntropyImport {
        path: PathBuf,
        json: bool,
    },
    LockStatus {
        json: bool,
    },
    LockClear {
        json: bool,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletRuntimeSurface {
    pub runtime_kind: PylonWalletRuntimeKind,
    pub liquidity_provider_kind: PylonWalletLiquidityProviderKind,
    pub network: String,
    pub identity_path: String,
    pub storage_dir: String,
    pub api_key_env: Option<String>,
    pub api_key_source: String,
    pub node_entropy: WalletNodeEntropyMetadata,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct WalletNodeEntropyMetadata {
    pub source: String,
    pub derivation_version: String,
    pub domain_label: String,
    pub digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_path: Option<String>,
}

#[derive(Clone, Debug)]
struct WalletNodeEntropyMaterial {
    bytes: [u8; 64],
    metadata: WalletNodeEntropyMetadata,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PylonWalletChannelRecord {
    pub channel_id: String,
    pub status: String,
    pub counterparty_node_id: Option<String>,
    pub funding_txo: Option<String>,
    pub channel_value_sats: u64,
    pub inbound_sats: u64,
    pub outbound_sats: u64,
    pub inbound_htlc_maximum_sats: Option<u64>,
    pub next_outbound_htlc_limit_sats: u64,
    pub confirmations: Option<u32>,
    pub confirmations_required: Option<u32>,
    pub is_outbound: bool,
    pub is_public: bool,
    pub peer_connected: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletBalanceSnapshot {
    pub credited_sats: u64,
    pub lightning_sats: u64,
    pub onchain_sats: u64,
    pub spendable_onchain_sats: u64,
    pub anchor_reserve_sats: u64,
    pub total_sats: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletStatusReport {
    pub runtime: WalletRuntimeSurface,
    pub runtime_status: String,
    pub runtime_detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ldk_node: Option<WalletLdkNodeStatus>,
    pub balance: WalletBalanceSnapshot,
    pub channels: Vec<PylonWalletChannelRecord>,
    pub lightning_readiness: WalletLightningReadiness,
    pub recent_payments: Vec<PylonWalletPaymentRecord>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletLdkNodeStatus {
    pub node_id: Option<String>,
    pub network: String,
    pub storage_dir: String,
    pub storage_schema_version: u32,
    pub storage_generation: String,
    pub chain_source_kind: String,
    pub chain_source_url: Option<String>,
    pub rgs_url: Option<String>,
    pub backup_status: String,
    pub backup_manifest_present: bool,
    pub backup_artifact_count: usize,
    pub backup_stale: bool,
    pub backup_stale_after_ms: u64,
    pub last_backup_exported_at_ms: Option<u64>,
    pub last_backup_file_digest: Option<String>,
    pub is_running: bool,
    pub latest_lightning_wallet_sync_timestamp: Option<u64>,
    pub latest_onchain_wallet_sync_timestamp: Option<u64>,
    pub latest_rgs_snapshot_timestamp: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletAddressReport {
    pub runtime: WalletRuntimeSurface,
    pub payout_destination: Option<String>,
    pub bitcoin_address: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WalletInvoiceReport {
    pub runtime: WalletRuntimeSurface,
    pub invoice: PylonWalletInvoiceRecord,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WalletOfferReport {
    pub runtime: WalletRuntimeSurface,
    pub offer: String,
    pub amount_sats: Option<u64>,
    pub description: Option<String>,
    pub created_at_ms: u64,
    pub expires_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WalletPayReport {
    pub runtime: WalletRuntimeSurface,
    pub payment_id: String,
    pub payment: PylonWalletPaymentRecord,
    pub post_balance: WalletBalanceSnapshot,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletHistoryReport {
    pub runtime: WalletRuntimeSurface,
    pub channels: Vec<PylonWalletChannelRecord>,
    pub lightning_readiness: WalletLightningReadiness,
    pub payments: Vec<PylonWalletPaymentRecord>,
    pub receipts: Vec<PylonWalletReceiptRecord>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletChannelsReport {
    pub runtime: WalletRuntimeSurface,
    pub channels: Vec<PylonWalletChannelRecord>,
    pub liquidity: WalletTelemetryLiquidity,
    pub channel_summary: WalletTelemetryChannels,
    pub lightning_readiness: WalletLightningReadiness,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletCreditSummaryReport {
    pub runtime: WalletRuntimeSurface,
    pub credits: PylonWalletCreditSummary,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletTelemetryReport {
    pub schema: String,
    pub generated_at_ms: u64,
    pub runtime: WalletRuntimeSurface,
    pub runtime_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_detail: Option<String>,
    pub node_id: Option<String>,
    pub health: WalletTelemetryHealth,
    pub sources: WalletTelemetrySources,
    pub sync: WalletTelemetrySync,
    pub balances: WalletBalanceSnapshot,
    pub channels: WalletTelemetryChannels,
    pub liquidity: WalletTelemetryLiquidity,
    pub lsp: WalletLspReadiness,
    pub backup: WalletTelemetryBackup,
    pub warnings: Vec<WalletTelemetrySignal>,
    pub errors: Vec<WalletTelemetrySignal>,
    pub redaction: WalletTelemetryRedaction,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletTelemetryHealth {
    pub state: String,
    pub payable: bool,
    pub receive_ready: bool,
    pub send_ready: bool,
    pub backup_ready: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletTelemetrySources {
    pub network: String,
    pub chain_source_kind: String,
    pub chain_source_url: Option<String>,
    pub gossip_source_kind: String,
    pub gossip_source_url: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletTelemetrySync {
    pub state: String,
    pub is_running: bool,
    pub latest_lightning_wallet_sync_timestamp: Option<u64>,
    pub latest_onchain_wallet_sync_timestamp: Option<u64>,
    pub latest_rgs_snapshot_timestamp: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletTelemetryChannels {
    pub total_count: usize,
    pub usable_count: usize,
    pub ready_count: usize,
    pub pending_count: usize,
    pub inactive_count: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletTelemetryLiquidity {
    pub inbound_sats: u64,
    pub outbound_sats: u64,
    pub inbound_bucket: String,
    pub outbound_bucket: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletLightningReadiness {
    pub state: String,
    pub can_receive_lightning: bool,
    pub can_receive_onchain: bool,
    pub can_send_lightning: bool,
    pub inbound_liquidity_sats: u64,
    pub outbound_liquidity_sats: u64,
    pub usable_channel_count: usize,
    pub pending_channel_count: usize,
    pub peer_connected_count: usize,
    pub warning_code: Option<String>,
    pub warning: Option<String>,
    pub remediation: Vec<String>,
    pub lsp: WalletLspReadiness,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletLspReadiness {
    pub provider_kind: PylonWalletLiquidityProviderKind,
    pub supported_protocols: Vec<String>,
    pub configured: bool,
    pub state: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletTelemetryBackup {
    pub status: String,
    pub manifest_present: bool,
    pub artifact_count: usize,
    pub stale: bool,
    pub stale_after_ms: u64,
    pub last_exported_at_ms: Option<u64>,
    pub last_file_digest: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletTelemetrySignal {
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletTelemetryRedaction {
    pub policy: String,
    pub forbidden_secret_classes: Vec<String>,
    pub endpoint_credentials_redacted: bool,
    pub raw_channel_state_excluded: bool,
    pub raw_key_material_excluded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WalletEntropyReport {
    pub runtime: WalletRuntimeSurface,
    pub operation: String,
    pub metadata: WalletNodeEntropyMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WalletBackupExportReport {
    pub runtime: WalletRuntimeSurface,
    pub operation: String,
    pub path: String,
    pub file_digest: String,
    pub exported_at_ms: u64,
    pub backup_status: String,
    pub manifest: WalletBackupPublicManifest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WalletBackupInspectReport {
    pub path: String,
    pub file_digest: String,
    pub valid: bool,
    pub manifest: WalletBackupPublicManifest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WalletRestoreReport {
    pub runtime: WalletRuntimeSurface,
    pub operation: String,
    pub mode: String,
    pub status: String,
    pub network: String,
    pub restored_component_count: usize,
    pub backup_file_digest: Option<String>,
    pub plaintext_manifest_digest: Option<String>,
    pub identity_mnemonic_restored: bool,
    pub recovery_mode: String,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WalletBackupPublicManifest {
    pub schema_version: u32,
    pub kind: String,
    pub exported_at_ms: u64,
    pub runtime_kind: String,
    pub network: String,
    pub wallet_derivation_version: String,
    pub node_entropy_digest: String,
    pub storage_generation: String,
    pub encryption_algorithm: String,
    pub kdf: String,
    pub plaintext_manifest_digest: String,
    pub plaintext_component_count: usize,
    pub plaintext_total_bytes: u64,
    pub snapshot_kinds: Vec<String>,
    pub identity_mnemonic_included: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct WalletBackupEncryptedFile {
    schema_version: u32,
    kind: String,
    public_manifest: WalletBackupPublicManifest,
    encryption: WalletBackupEncryptionMetadata,
    ciphertext_hex: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct WalletBackupEncryptionMetadata {
    algorithm: String,
    kdf: String,
    salt_hex: String,
    nonce_hex: String,
    scrypt_log_n: u8,
    scrypt_r: u32,
    scrypt_p: u32,
    key_len: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct WalletBackupPlaintextManifest {
    schema_version: u32,
    kind: String,
    exported_at_ms: u64,
    runtime_kind: String,
    network: String,
    wallet_derivation: WalletNodeEntropyMetadata,
    storage_generation: String,
    identity_mnemonic_included: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    identity_mnemonic: Option<String>,
    ldk_storage_snapshot: Vec<WalletBackupSnapshotFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct WalletBackupSnapshotFile {
    relative_path: String,
    kind: String,
    size_bytes: u64,
    sha256: String,
    content_hex: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
struct WalletBackupStatusManifest {
    schema_version: u32,
    kind: String,
    status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_exported_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_export_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_export_file_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_plaintext_manifest_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_component_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_total_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    encryption_algorithm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kdf: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WalletStorageLayoutReport {
    pub schema_version: u32,
    pub root_dir: String,
    pub ldk_dir: String,
    pub node_dir: String,
    pub sqlite_dir: String,
    pub backup_staging_dir: String,
    pub lock_path: String,
    pub backup_manifest_path: String,
    pub last_registration_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WalletLockOwner {
    pub pid: u32,
    pub machine_id: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WalletLockReport {
    pub runtime: WalletRuntimeSurface,
    pub lock_path: String,
    pub locked: bool,
    pub stale: bool,
    pub cleared: bool,
    pub owner: Option<WalletLockOwner>,
    pub detail: String,
}

#[derive(Clone, Debug)]
struct WalletStorageLayout {
    root_dir: PathBuf,
    ldk_dir: PathBuf,
    node_dir: PathBuf,
    sqlite_dir: PathBuf,
    backup_staging_dir: PathBuf,
    lock_path: PathBuf,
    backup_manifest_path: PathBuf,
    last_registration_path: PathBuf,
}

#[derive(Debug)]
struct PylonWalletStorageLock {
    path: PathBuf,
}

impl Drop for PylonWalletStorageLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.path.as_path());
    }
}

#[derive(Clone, Debug)]
struct WalletRuntimeContext {
    runtime: SelectedPylonWalletRuntime,
}

pub trait PylonWalletRuntime {
    fn surface(&self) -> &WalletRuntimeSurface;
    fn start(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn sync(
        &self,
        ledger: &PylonLedger,
        include_recent_payments: bool,
    ) -> Result<WalletStatusReport>;
    fn status(
        &self,
        ledger: &PylonLedger,
        include_recent_payments: bool,
    ) -> Result<WalletStatusReport>;
    fn balance(&self, ledger: &PylonLedger) -> Result<WalletBalanceSnapshot>;
    fn address(&self) -> Result<WalletAddressReport>;
    fn invoice(
        &self,
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    ) -> Result<WalletInvoiceReport>;
    fn offer(
        &self,
        amount_sats: Option<u64>,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    ) -> Result<WalletOfferReport>;
    fn pay(&self, payment_request: &str, amount_sats: Option<u64>) -> Result<WalletPayReport>;
    fn withdraw(&self, payment_request: &str, amount_sats: Option<u64>) -> Result<WalletPayReport>;
    fn list_payments(
        &self,
        ledger: &PylonLedger,
        limit: Option<u32>,
    ) -> Result<Vec<PylonWalletPaymentRecord>>;
    fn list_channels(&self) -> Result<Vec<PylonWalletChannelRecord>>;
}

#[derive(Clone, Debug)]
enum SelectedPylonWalletRuntime {
    ExternalTarget(ExternalTargetWalletRuntime),
    Mock(MockPylonWalletRuntime),
    LdkNode(LdkNodeWalletRuntime),
    MoneyDevKit(MoneyDevKitWalletRuntime),
}

#[derive(Clone, Debug)]
struct ExternalTargetWalletRuntime {
    surface: WalletRuntimeSurface,
    payout_destination: Option<String>,
}

#[derive(Clone, Debug)]
struct MockPylonWalletRuntime {
    surface: WalletRuntimeSurface,
    payout_destination: Option<String>,
}

#[derive(Clone, Debug)]
struct MoneyDevKitWalletRuntime {
    surface: WalletRuntimeSurface,
    home_dir: PathBuf,
    network: String,
}

#[derive(Clone)]
struct LdkNodeWalletRuntime {
    surface: WalletRuntimeSurface,
    settings: LdkNodeWalletSettings,
    entropy: [u8; 64],
    lock: Arc<Mutex<Option<PylonWalletStorageLock>>>,
    node: Arc<Mutex<Option<ldk_node::Node>>>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl std::fmt::Debug for LdkNodeWalletRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LdkNodeWalletRuntime")
            .field("surface", &self.surface)
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct LdkNodeWalletSettings {
    chain_source_kind: String,
    esplora_url: Option<String>,
    electrum_url: Option<String>,
    rgs_url: Option<String>,
}

impl PylonWalletRuntime for SelectedPylonWalletRuntime {
    fn surface(&self) -> &WalletRuntimeSurface {
        match self {
            Self::ExternalTarget(runtime) => runtime.surface(),
            Self::Mock(runtime) => runtime.surface(),
            Self::LdkNode(runtime) => runtime.surface(),
            Self::MoneyDevKit(runtime) => runtime.surface(),
        }
    }

    fn start(&self) -> Result<()> {
        match self {
            Self::ExternalTarget(runtime) => runtime.start(),
            Self::Mock(runtime) => runtime.start(),
            Self::LdkNode(runtime) => runtime.start(),
            Self::MoneyDevKit(runtime) => runtime.start(),
        }
    }

    fn stop(&self) -> Result<()> {
        match self {
            Self::ExternalTarget(runtime) => runtime.stop(),
            Self::Mock(runtime) => runtime.stop(),
            Self::LdkNode(runtime) => runtime.stop(),
            Self::MoneyDevKit(runtime) => runtime.stop(),
        }
    }

    fn sync(
        &self,
        ledger: &PylonLedger,
        include_recent_payments: bool,
    ) -> Result<WalletStatusReport> {
        match self {
            Self::ExternalTarget(runtime) => runtime.sync(ledger, include_recent_payments),
            Self::Mock(runtime) => runtime.sync(ledger, include_recent_payments),
            Self::LdkNode(runtime) => runtime.sync(ledger, include_recent_payments),
            Self::MoneyDevKit(runtime) => runtime.sync(ledger, include_recent_payments),
        }
    }

    fn status(
        &self,
        ledger: &PylonLedger,
        include_recent_payments: bool,
    ) -> Result<WalletStatusReport> {
        match self {
            Self::ExternalTarget(runtime) => runtime.status(ledger, include_recent_payments),
            Self::Mock(runtime) => runtime.status(ledger, include_recent_payments),
            Self::LdkNode(runtime) => runtime.status(ledger, include_recent_payments),
            Self::MoneyDevKit(runtime) => runtime.status(ledger, include_recent_payments),
        }
    }

    fn balance(&self, ledger: &PylonLedger) -> Result<WalletBalanceSnapshot> {
        match self {
            Self::ExternalTarget(runtime) => runtime.balance(ledger),
            Self::Mock(runtime) => runtime.balance(ledger),
            Self::LdkNode(runtime) => runtime.balance(ledger),
            Self::MoneyDevKit(runtime) => runtime.balance(ledger),
        }
    }

    fn address(&self) -> Result<WalletAddressReport> {
        match self {
            Self::ExternalTarget(runtime) => runtime.address(),
            Self::Mock(runtime) => runtime.address(),
            Self::LdkNode(runtime) => runtime.address(),
            Self::MoneyDevKit(runtime) => runtime.address(),
        }
    }

    fn invoice(
        &self,
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    ) -> Result<WalletInvoiceReport> {
        match self {
            Self::ExternalTarget(runtime) => {
                runtime.invoice(amount_sats, description, expiry_seconds)
            }
            Self::Mock(runtime) => runtime.invoice(amount_sats, description, expiry_seconds),
            Self::LdkNode(runtime) => runtime.invoice(amount_sats, description, expiry_seconds),
            Self::MoneyDevKit(runtime) => runtime.invoice(amount_sats, description, expiry_seconds),
        }
    }

    fn offer(
        &self,
        amount_sats: Option<u64>,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    ) -> Result<WalletOfferReport> {
        match self {
            Self::ExternalTarget(runtime) => {
                runtime.offer(amount_sats, description, expiry_seconds)
            }
            Self::Mock(runtime) => runtime.offer(amount_sats, description, expiry_seconds),
            Self::LdkNode(runtime) => runtime.offer(amount_sats, description, expiry_seconds),
            Self::MoneyDevKit(runtime) => runtime.offer(amount_sats, description, expiry_seconds),
        }
    }

    fn pay(&self, payment_request: &str, amount_sats: Option<u64>) -> Result<WalletPayReport> {
        match self {
            Self::ExternalTarget(runtime) => runtime.pay(payment_request, amount_sats),
            Self::Mock(runtime) => runtime.pay(payment_request, amount_sats),
            Self::LdkNode(runtime) => runtime.pay(payment_request, amount_sats),
            Self::MoneyDevKit(runtime) => runtime.pay(payment_request, amount_sats),
        }
    }

    fn withdraw(&self, payment_request: &str, amount_sats: Option<u64>) -> Result<WalletPayReport> {
        match self {
            Self::ExternalTarget(runtime) => runtime.withdraw(payment_request, amount_sats),
            Self::Mock(runtime) => runtime.withdraw(payment_request, amount_sats),
            Self::LdkNode(runtime) => runtime.withdraw(payment_request, amount_sats),
            Self::MoneyDevKit(runtime) => runtime.withdraw(payment_request, amount_sats),
        }
    }

    fn list_payments(
        &self,
        ledger: &PylonLedger,
        limit: Option<u32>,
    ) -> Result<Vec<PylonWalletPaymentRecord>> {
        match self {
            Self::ExternalTarget(runtime) => runtime.list_payments(ledger, limit),
            Self::Mock(runtime) => runtime.list_payments(ledger, limit),
            Self::LdkNode(runtime) => runtime.list_payments(ledger, limit),
            Self::MoneyDevKit(runtime) => runtime.list_payments(ledger, limit),
        }
    }

    fn list_channels(&self) -> Result<Vec<PylonWalletChannelRecord>> {
        match self {
            Self::ExternalTarget(runtime) => runtime.list_channels(),
            Self::Mock(runtime) => runtime.list_channels(),
            Self::LdkNode(runtime) => runtime.list_channels(),
            Self::MoneyDevKit(runtime) => runtime.list_channels(),
        }
    }
}

impl PylonWalletRuntime for ExternalTargetWalletRuntime {
    fn surface(&self) -> &WalletRuntimeSurface {
        &self.surface
    }

    fn start(&self) -> Result<()> {
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }

    fn sync(
        &self,
        ledger: &PylonLedger,
        include_recent_payments: bool,
    ) -> Result<WalletStatusReport> {
        self.status(ledger, include_recent_payments)
    }

    fn status(
        &self,
        ledger: &PylonLedger,
        include_recent_payments: bool,
    ) -> Result<WalletStatusReport> {
        let channels = self.list_channels()?;
        let lightning_readiness = wallet_lightning_readiness(
            self.surface.runtime_kind,
            self.surface.liquidity_provider_kind,
            channels.as_slice(),
            true,
        );
        Ok(WalletStatusReport {
            runtime: self.surface.clone(),
            runtime_status: "external_target".to_string(),
            runtime_detail: Some(LDK_EXTERNAL_WALLET_DETAIL.to_string()),
            ldk_node: None,
            balance: self.balance(ledger)?,
            channels,
            lightning_readiness,
            recent_payments: ledger_payments(ledger, include_recent_payments.then_some(10)),
        })
    }

    fn balance(&self, ledger: &PylonLedger) -> Result<WalletBalanceSnapshot> {
        Ok(ledger_balance(ledger))
    }

    fn address(&self) -> Result<WalletAddressReport> {
        let _ = self.payout_destination.as_deref();
        bail!("{LDK_EXTERNAL_WALLET_DETAIL}")
    }

    fn invoice(
        &self,
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    ) -> Result<WalletInvoiceReport> {
        let _ = (amount_sats, description, expiry_seconds);
        bail!("{LDK_EXTERNAL_WALLET_DETAIL}")
    }

    fn offer(
        &self,
        amount_sats: Option<u64>,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    ) -> Result<WalletOfferReport> {
        let _ = (amount_sats, description, expiry_seconds);
        bail!("{LDK_EXTERNAL_WALLET_DETAIL}")
    }

    fn pay(&self, payment_request: &str, amount_sats: Option<u64>) -> Result<WalletPayReport> {
        let _ = (payment_request, amount_sats);
        bail!("{LDK_EXTERNAL_WALLET_DETAIL}")
    }

    fn withdraw(&self, payment_request: &str, amount_sats: Option<u64>) -> Result<WalletPayReport> {
        self.pay(payment_request, amount_sats)
    }

    fn list_payments(
        &self,
        ledger: &PylonLedger,
        limit: Option<u32>,
    ) -> Result<Vec<PylonWalletPaymentRecord>> {
        Ok(ledger_payments(ledger, limit.map(|value| value as usize)))
    }

    fn list_channels(&self) -> Result<Vec<PylonWalletChannelRecord>> {
        Ok(Vec::new())
    }
}

impl PylonWalletRuntime for MockPylonWalletRuntime {
    fn surface(&self) -> &WalletRuntimeSurface {
        &self.surface
    }

    fn start(&self) -> Result<()> {
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }

    fn sync(
        &self,
        ledger: &PylonLedger,
        include_recent_payments: bool,
    ) -> Result<WalletStatusReport> {
        self.status(ledger, include_recent_payments)
    }

    fn status(
        &self,
        ledger: &PylonLedger,
        include_recent_payments: bool,
    ) -> Result<WalletStatusReport> {
        let channels = self.list_channels()?;
        let lightning_readiness = wallet_lightning_readiness(
            self.surface.runtime_kind,
            self.surface.liquidity_provider_kind,
            channels.as_slice(),
            true,
        );
        Ok(WalletStatusReport {
            runtime: self.surface.clone(),
            runtime_status: "connected".to_string(),
            runtime_detail: Some(MOCK_WALLET_DETAIL.to_string()),
            ldk_node: None,
            balance: self.balance(ledger)?,
            channels,
            lightning_readiness,
            recent_payments: self.list_payments(ledger, include_recent_payments.then_some(10))?,
        })
    }

    fn balance(&self, ledger: &PylonLedger) -> Result<WalletBalanceSnapshot> {
        let ledger_balance = ledger_balance(ledger);
        if ledger_balance.total_sats > 0 {
            return Ok(ledger_balance);
        }
        Ok(WalletBalanceSnapshot {
            credited_sats: 1_000,
            lightning_sats: 1_000,
            onchain_sats: 0,
            spendable_onchain_sats: 0,
            anchor_reserve_sats: 0,
            total_sats: 1_000,
        })
    }

    fn address(&self) -> Result<WalletAddressReport> {
        Ok(WalletAddressReport {
            runtime: self.surface.clone(),
            payout_destination: self
                .payout_destination
                .clone()
                .or_else(|| Some("lno1mockpylonwallet".to_string())),
            bitcoin_address: "bcrt1pmockpylonwalletaddress0000000000000000000000".to_string(),
        })
    }

    fn invoice(
        &self,
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    ) -> Result<WalletInvoiceReport> {
        let _ = expiry_seconds;
        let now = now_epoch_ms() as u64;
        Ok(WalletInvoiceReport {
            runtime: self.surface.clone(),
            invoice: PylonWalletInvoiceRecord {
                invoice_id: format!("mock-invoice-{amount_sats}"),
                amount_sats,
                status: "open".to_string(),
                payment_request: format!("lnbc{amount_sats}mockpyloninvoice"),
                description,
                payment_hash: None,
                runtime_kind: Some(self.surface.runtime_kind.to_string()),
                expires_at_ms: expiry_seconds
                    .map(|seconds| now.saturating_add(u64::from(seconds).saturating_mul(1000))),
                created_at_ms: now,
                updated_at_ms: now,
            },
        })
    }

    fn offer(
        &self,
        amount_sats: Option<u64>,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    ) -> Result<WalletOfferReport> {
        let now = now_epoch_ms() as u64;
        Ok(WalletOfferReport {
            runtime: self.surface.clone(),
            offer: "lno1mockpylonoffer".to_string(),
            amount_sats,
            description,
            created_at_ms: now,
            expires_at_ms: expiry_seconds
                .map(|seconds| now.saturating_add(u64::from(seconds).saturating_mul(1000))),
        })
    }

    fn pay(&self, payment_request: &str, amount_sats: Option<u64>) -> Result<WalletPayReport> {
        let now = now_epoch_ms() as u64;
        let amount_sats = amount_sats.unwrap_or(21);
        let payment = PylonWalletPaymentRecord {
            payment_id: format!("mock-payment-{amount_sats}"),
            direction: "send".to_string(),
            status: "completed".to_string(),
            amount_sats,
            fees_sats: 1,
            method: "lightning".to_string(),
            description: Some("mock wallet payment".to_string()),
            invoice: Some(payment_request.to_string()),
            payment_hash: None,
            txid: None,
            operation_id: None,
            receipt_id: None,
            failure_code: None,
            created_at_ms: now,
            updated_at_ms: now,
        };
        Ok(WalletPayReport {
            runtime: self.surface.clone(),
            payment_id: payment.payment_id.clone(),
            payment,
            post_balance: WalletBalanceSnapshot {
                credited_sats: 1_000u64.saturating_sub(amount_sats),
                lightning_sats: 1_000u64.saturating_sub(amount_sats),
                onchain_sats: 0,
                spendable_onchain_sats: 0,
                anchor_reserve_sats: 0,
                total_sats: 1_000u64.saturating_sub(amount_sats),
            },
        })
    }

    fn withdraw(&self, payment_request: &str, amount_sats: Option<u64>) -> Result<WalletPayReport> {
        self.pay(payment_request, amount_sats)
    }

    fn list_payments(
        &self,
        ledger: &PylonLedger,
        limit: Option<u32>,
    ) -> Result<Vec<PylonWalletPaymentRecord>> {
        let payments = ledger_payments(ledger, limit.map(|value| value as usize));
        if !payments.is_empty() {
            return Ok(payments);
        }
        let now = now_epoch_ms() as u64;
        Ok(vec![PylonWalletPaymentRecord {
            payment_id: "mock-receive-1".to_string(),
            direction: "receive".to_string(),
            status: "completed".to_string(),
            amount_sats: 1_000,
            fees_sats: 0,
            method: "lightning".to_string(),
            description: Some("mock wallet receive".to_string()),
            invoice: Some("lnbc1000mockpyloninvoice".to_string()),
            payment_hash: None,
            txid: None,
            operation_id: None,
            receipt_id: None,
            failure_code: None,
            created_at_ms: now,
            updated_at_ms: now,
        }])
    }

    fn list_channels(&self) -> Result<Vec<PylonWalletChannelRecord>> {
        Ok(vec![PylonWalletChannelRecord {
            channel_id: "mock-channel-1".to_string(),
            status: "usable".to_string(),
            counterparty_node_id: Some("02mockcounterparty".to_string()),
            funding_txo: Some("mock-txid:0".to_string()),
            channel_value_sats: 1_000,
            inbound_sats: 500,
            outbound_sats: 500,
            inbound_htlc_maximum_sats: Some(500),
            next_outbound_htlc_limit_sats: 500,
            confirmations: Some(6),
            confirmations_required: Some(1),
            is_outbound: true,
            is_public: false,
            peer_connected: true,
        }])
    }
}

impl MoneyDevKitWalletRuntime {
    fn agent_wallet_home(&self) -> PathBuf {
        self.home_dir.join(".mdk-wallet")
    }

    fn mdk_network(&self) -> &'static str {
        match self
            .network
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")
            .as_str()
        {
            "bitcoin" | "mainnet" | "btc" => "mainnet",
            "signet" | "testnet" | "regtest" | "mutinynet" => "signet",
            _ => "signet",
        }
    }

    fn ensure_private_home(&self) -> Result<()> {
        create_private_dir(self.home_dir.as_path(), "MoneyDevKit wallet home")?;
        Ok(())
    }

    fn ensure_initialized(&self) -> Result<()> {
        self.ensure_private_home()?;
        if self.agent_wallet_home().join("config.json").exists() {
            return Ok(());
        }
        let _ = self.run_agent_wallet_command(&["init", "--network", self.mdk_network()])?;
        Ok(())
    }

    fn run_agent_wallet_command(&self, args: &[&str]) -> Result<Value> {
        self.ensure_private_home()?;
        let output = Command::new("npx")
            .arg("-y")
            .arg(MONEYDEVKIT_AGENT_WALLET_PACKAGE)
            .args(args)
            .env("HOME", self.home_dir.as_os_str())
            .env("MDK_WALLET_NETWORK", self.mdk_network())
            .output()
            .with_context(|| {
                format!(
                    "failed to run MoneyDevKit agent wallet command '{}'",
                    args.join(" ")
                )
            })?;
        let stdout = String::from_utf8_lossy(output.stdout.as_slice()).to_string();
        let stderr = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
        if !output.status.success() {
            let detail = redact_mdk_command_output(format!("{stdout}\n{stderr}").as_str());
            bail!(
                "MoneyDevKit agent wallet command '{}' failed: {}",
                args.join(" "),
                detail.trim()
            );
        }
        parse_mdk_stdout_json(stdout.as_str()).with_context(|| {
            format!(
                "MoneyDevKit command '{}' did not return JSON",
                args.join(" ")
            )
        })
    }

    fn maybe_run_agent_wallet_command(&self, args: &[&str]) -> Result<Option<Value>> {
        match self.run_agent_wallet_command(args) {
            Ok(value) => Ok(Some(value)),
            Err(error) => {
                let detail = error.to_string();
                if detail.contains("Not initialized")
                    || detail.contains("not initialized")
                    || detail.contains("No such file")
                {
                    return Ok(None);
                }
                Err(error)
            }
        }
    }
}

impl PylonWalletRuntime for MoneyDevKitWalletRuntime {
    fn surface(&self) -> &WalletRuntimeSurface {
        &self.surface
    }

    fn start(&self) -> Result<()> {
        self.ensure_initialized()?;
        let _ = self.run_agent_wallet_command(&["start"])?;
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        match self.run_agent_wallet_command(&["stop"]) {
            Ok(_) => Ok(()),
            Err(error) => {
                let detail = error.to_string();
                if detail.contains("not_running") || detail.contains("not running") {
                    Ok(())
                } else {
                    Err(error)
                }
            }
        }
    }

    fn sync(
        &self,
        ledger: &PylonLedger,
        include_recent_payments: bool,
    ) -> Result<WalletStatusReport> {
        self.status(ledger, include_recent_payments)
    }

    fn status(
        &self,
        ledger: &PylonLedger,
        include_recent_payments: bool,
    ) -> Result<WalletStatusReport> {
        self.ensure_initialized()?;
        let status = self.run_agent_wallet_command(&["status"])?;
        let running = mdk_json_bool(&status, &["running"]).unwrap_or(false);
        let channels = self.list_channels()?;
        let lightning_readiness = wallet_lightning_readiness(
            self.surface.runtime_kind,
            self.surface.liquidity_provider_kind,
            channels.as_slice(),
            false,
        );
        let recent_payments = if include_recent_payments {
            self.list_payments(ledger, Some(10))?
        } else {
            Vec::new()
        };
        Ok(WalletStatusReport {
            runtime: self.surface.clone(),
            runtime_status: if running { "connected" } else { "configured" }.to_string(),
            runtime_detail: Some(MONEYDEVKIT_WALLET_DETAIL.to_string()),
            ldk_node: None,
            balance: self.balance(ledger)?,
            channels,
            lightning_readiness,
            recent_payments,
        })
    }

    fn balance(&self, ledger: &PylonLedger) -> Result<WalletBalanceSnapshot> {
        self.ensure_initialized()?;
        let value = self.run_agent_wallet_command(&["balance"])?;
        let lightning_sats =
            mdk_json_u64(&value, &["balance_sats", "balanceSats"]).unwrap_or_default();
        Ok(WalletBalanceSnapshot {
            credited_sats: ledger.wallet.last_balance_sats.unwrap_or_default(),
            lightning_sats,
            onchain_sats: 0,
            spendable_onchain_sats: 0,
            anchor_reserve_sats: 0,
            total_sats: lightning_sats,
        })
    }

    fn address(&self) -> Result<WalletAddressReport> {
        bail!(
            "MoneyDevKit agent-wallet does not expose an on-chain receive address through its CLI; use wallet invoice or wallet offer for Lightning receives"
        )
    }

    fn invoice(
        &self,
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    ) -> Result<WalletInvoiceReport> {
        self.ensure_initialized()?;
        let description_text = description
            .clone()
            .unwrap_or_else(|| DEFAULT_RECEIVE_DESCRIPTION.to_string());
        let amount_text = amount_sats.to_string();
        let value = self.run_agent_wallet_command(&[
            "receive",
            amount_text.as_str(),
            "--description",
            description_text.as_str(),
        ])?;
        let invoice = mdk_json_string(&value, &["invoice"])
            .ok_or_else(|| anyhow!("MoneyDevKit receive response did not include invoice"))?;
        let payment_hash = mdk_json_string(&value, &["payment_hash", "paymentHash"]);
        let now = now_epoch_ms() as u64;
        let expiry_seconds = expiry_seconds.unwrap_or(DEFAULT_BOLT11_EXPIRY_SECONDS);
        let invoice_id = payment_hash
            .as_ref()
            .map(|hash| format!("mdk-bolt11-{hash}"))
            .unwrap_or_else(|| short_wallet_digest_id("mdk-bolt11", invoice.as_str()));
        Ok(WalletInvoiceReport {
            runtime: self.surface.clone(),
            invoice: PylonWalletInvoiceRecord {
                invoice_id,
                amount_sats,
                status: "open".to_string(),
                payment_request: invoice,
                description: Some(description_text),
                payment_hash,
                runtime_kind: Some(self.surface.runtime_kind.to_string()),
                expires_at_ms: Some(now.saturating_add(u64::from(expiry_seconds) * 1000)),
                created_at_ms: now,
                updated_at_ms: now,
            },
        })
    }

    fn offer(
        &self,
        amount_sats: Option<u64>,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    ) -> Result<WalletOfferReport> {
        self.ensure_initialized()?;
        let description_text = description
            .clone()
            .unwrap_or_else(|| DEFAULT_RECEIVE_DESCRIPTION.to_string());
        let value = self.run_agent_wallet_command(&[
            "receive-bolt12",
            "--description",
            description_text.as_str(),
        ])?;
        let offer = mdk_json_string(&value, &["offer"])
            .ok_or_else(|| anyhow!("MoneyDevKit receive-bolt12 response did not include offer"))?;
        let now = now_epoch_ms() as u64;
        Ok(WalletOfferReport {
            runtime: self.surface.clone(),
            offer,
            amount_sats,
            description: Some(description_text),
            created_at_ms: now,
            expires_at_ms: expiry_seconds
                .map(|seconds| now.saturating_add(u64::from(seconds) * 1000)),
        })
    }

    fn pay(&self, payment_request: &str, amount_sats: Option<u64>) -> Result<WalletPayReport> {
        self.ensure_initialized()?;
        let mut args = vec!["send", payment_request];
        let amount_text;
        if let Some(amount_sats) = amount_sats {
            amount_text = amount_sats.to_string();
            args.push(amount_text.as_str());
        }
        let value = self.run_agent_wallet_command(args.as_slice())?;
        let now = now_epoch_ms() as u64;
        let payment_id = mdk_json_string(&value, &["payment_id", "paymentId"])
            .unwrap_or_else(|| short_wallet_digest_id("mdk-send", payment_request));
        let payment_hash = mdk_json_string(&value, &["payment_hash", "paymentHash"]);
        let status =
            mdk_json_string(&value, &["status"]).unwrap_or_else(|| "completed".to_string());
        let amount_sats = amount_sats
            .or_else(|| infer_payment_request_amount_sats(payment_request))
            .unwrap_or_default();
        let payment = PylonWalletPaymentRecord {
            payment_id: payment_id.clone(),
            direction: "send".to_string(),
            status,
            amount_sats,
            fees_sats: 0,
            method: infer_wallet_payment_method(payment_request),
            description: Some("MoneyDevKit agent-wallet send".to_string()),
            invoice: Some(payment_request.to_string()),
            payment_hash,
            txid: None,
            operation_id: None,
            receipt_id: Some(format!("wallet:moneydevkit:{payment_id}")),
            failure_code: None,
            created_at_ms: now,
            updated_at_ms: now,
        };
        Ok(WalletPayReport {
            runtime: self.surface.clone(),
            payment_id,
            payment,
            post_balance: self.balance(&PylonLedger::default())?,
        })
    }

    fn withdraw(&self, payment_request: &str, amount_sats: Option<u64>) -> Result<WalletPayReport> {
        self.pay(payment_request, amount_sats)
    }

    fn list_payments(
        &self,
        ledger: &PylonLedger,
        limit: Option<u32>,
    ) -> Result<Vec<PylonWalletPaymentRecord>> {
        self.ensure_initialized()?;
        let runtime_records = match self.maybe_run_agent_wallet_command(&["payments"])? {
            Some(value) => value
                .get("payments")
                .and_then(Value::as_array)
                .map(|payments| {
                    payments
                        .iter()
                        .filter_map(mdk_payment_record_from_value)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            None => Vec::new(),
        };
        Ok(merge_wallet_payment_records(
            ledger_payments(ledger, None),
            runtime_records,
            limit.map(|value| value as usize),
        ))
    }

    fn list_channels(&self) -> Result<Vec<PylonWalletChannelRecord>> {
        Ok(Vec::new())
    }
}

impl PylonWalletRuntime for LdkNodeWalletRuntime {
    fn surface(&self) -> &WalletRuntimeSurface {
        &self.surface
    }

    fn start(&self) -> Result<()> {
        let layout = ensure_wallet_storage_layout_for_root(PathBuf::from(
            self.surface.storage_dir.as_str(),
        ))?;
        ensure_private_file_permissions(
            Path::new(self.surface.identity_path.as_str()),
            "identity mnemonic",
        )?;
        if let Some(path) = self.surface.node_entropy.override_path.as_deref() {
            ensure_private_file_permissions(Path::new(path), "wallet entropy override")?;
        }
        let mut lock = self
            .lock
            .lock()
            .map_err(|_| anyhow!("wallet storage lock state is poisoned"))?;
        if lock.is_none() {
            *lock = Some(acquire_wallet_storage_lock(&layout)?);
        }
        drop(lock);

        let mut node = self
            .node
            .lock()
            .map_err(|_| anyhow!("ldk node state is poisoned"))?;
        if node.is_none() {
            *node = Some(self.build_node(&layout)?);
        }
        let should_start = self.settings.chain_source_kind != "none";
        if should_start {
            if let Some(node) = node.as_ref() {
                if !node.status().is_running {
                    match node.start() {
                        Ok(()) => self.set_last_error(None)?,
                        Err(error) => self.set_last_error(Some(error.to_string()))?,
                    }
                }
            }
        } else {
            self.set_last_error(None)?;
        }
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let mut node = self
            .node
            .lock()
            .map_err(|_| anyhow!("ldk node state is poisoned"))?;
        if let Some(node) = node.as_ref() {
            if node.status().is_running {
                node.stop()
                    .map_err(|error| anyhow!("failed to stop ldk node: {error}"))?;
            }
        }
        let _ = node.take();
        drop(node);

        let mut lock = self
            .lock
            .lock()
            .map_err(|_| anyhow!("wallet storage lock state is poisoned"))?;
        let _ = lock.take();
        Ok(())
    }

    fn sync(
        &self,
        ledger: &PylonLedger,
        include_recent_payments: bool,
    ) -> Result<WalletStatusReport> {
        let should_sync = self
            .with_node(|node| node.status().is_running)?
            .unwrap_or(false);
        if should_sync {
            let sync_result = self.with_node(|node| node.sync_wallets())?;
            if let Some(Err(error)) = sync_result {
                self.set_last_error(Some(error.to_string()))?;
            }
        }
        self.status(ledger, include_recent_payments)
    }

    fn status(
        &self,
        ledger: &PylonLedger,
        include_recent_payments: bool,
    ) -> Result<WalletStatusReport> {
        let ldk_node = self.ldk_node_status()?;
        let runtime_status = if ldk_node.last_error.is_some() {
            "error"
        } else if ldk_node.is_running {
            "connected"
        } else {
            "configured"
        };
        let runtime_detail = if let Some(error) = ldk_node.last_error.as_ref() {
            Some(format!("ldk_node error: {error}"))
        } else if self.settings.chain_source_kind == "none" {
            Some("ldk_node built with no chain source; set wallet_chain_source_kind=esplora or electrum to sync/start the live node".to_string())
        } else {
            Some("ldk_node runtime initialized".to_string())
        };
        let channels = self.list_channels()?;
        let lightning_readiness = wallet_lightning_readiness(
            self.surface.runtime_kind,
            self.surface.liquidity_provider_kind,
            channels.as_slice(),
            true,
        );
        Ok(WalletStatusReport {
            runtime: self.surface.clone(),
            runtime_status: runtime_status.to_string(),
            runtime_detail,
            ldk_node: Some(ldk_node),
            balance: self.balance(ledger)?,
            channels,
            lightning_readiness,
            recent_payments: self.list_payments(ledger, include_recent_payments.then_some(10))?,
        })
    }

    fn balance(&self, ledger: &PylonLedger) -> Result<WalletBalanceSnapshot> {
        let node_balance = self.with_node(|node| node.list_balances())?;
        if let Some(balance) = node_balance {
            return Ok(WalletBalanceSnapshot {
                credited_sats: ledger.wallet.last_balance_sats.unwrap_or_default(),
                lightning_sats: balance.total_lightning_balance_sats,
                onchain_sats: balance.total_onchain_balance_sats,
                spendable_onchain_sats: balance.spendable_onchain_balance_sats,
                anchor_reserve_sats: balance.total_anchor_channels_reserve_sats,
                total_sats: balance
                    .total_lightning_balance_sats
                    .saturating_add(balance.total_onchain_balance_sats),
            });
        }
        Ok(ledger_balance(ledger))
    }

    fn address(&self) -> Result<WalletAddressReport> {
        let address = self
            .with_node(|node| node.onchain_payment().new_address())?
            .ok_or_else(|| anyhow!("ldk_node is not initialized; run wallet status first"))?
            .map_err(|error| anyhow!("failed to create ldk_node on-chain address: {error}"))?;
        Ok(WalletAddressReport {
            runtime: self.surface.clone(),
            payout_destination: None,
            bitcoin_address: address.to_string(),
        })
    }

    fn invoice(
        &self,
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    ) -> Result<WalletInvoiceReport> {
        let amount_msat = amount_sats
            .checked_mul(1_000)
            .ok_or_else(|| anyhow!("wallet invoice amount is too large"))?;
        let description_text = description
            .clone()
            .unwrap_or_else(|| DEFAULT_RECEIVE_DESCRIPTION.to_string());
        let invoice_description = Bolt11InvoiceDescription::Direct(
            Description::new(description_text.clone())
                .map_err(|error| anyhow!("invalid wallet invoice description: {error}"))?,
        );
        let expiry_seconds = expiry_seconds.unwrap_or(DEFAULT_BOLT11_EXPIRY_SECONDS);
        let invoice = self
            .with_node(|node| {
                node.bolt11_payment()
                    .receive(amount_msat, &invoice_description, expiry_seconds)
            })?
            .ok_or_else(|| anyhow!("ldk_node is not initialized; run wallet status first"))?
            .map_err(|error| anyhow!("failed to create ldk_node BOLT11 invoice: {error}"))?;
        let now = now_epoch_ms() as u64;
        let payment_hash = invoice.payment_hash().to_string();
        Ok(WalletInvoiceReport {
            runtime: self.surface.clone(),
            invoice: PylonWalletInvoiceRecord {
                invoice_id: format!("bolt11-{payment_hash}"),
                amount_sats,
                status: "open".to_string(),
                payment_request: invoice.to_string(),
                description: Some(description_text),
                payment_hash: Some(payment_hash),
                runtime_kind: Some(self.surface.runtime_kind.to_string()),
                expires_at_ms: Some(now.saturating_add(u64::from(expiry_seconds) * 1000)),
                created_at_ms: now,
                updated_at_ms: now,
            },
        })
    }

    fn offer(
        &self,
        amount_sats: Option<u64>,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    ) -> Result<WalletOfferReport> {
        let description_text = description
            .clone()
            .unwrap_or_else(|| DEFAULT_RECEIVE_DESCRIPTION.to_string());
        let amount_msat = amount_sats
            .map(|amount_sats| {
                amount_sats
                    .checked_mul(1_000)
                    .ok_or_else(|| anyhow!("wallet offer amount is too large"))
            })
            .transpose()?;
        let offer = self
            .with_node(|node| {
                let payment = node.bolt12_payment();
                if let Some(amount_msat) = amount_msat {
                    payment.receive(amount_msat, description_text.as_str(), expiry_seconds, None)
                } else {
                    payment.receive_variable_amount(description_text.as_str(), expiry_seconds)
                }
            })?
            .ok_or_else(|| anyhow!("ldk_node is not initialized; run wallet status first"))?
            .map_err(|error| {
                anyhow!(
                    "bolt12_offer_unavailable: failed to create ldk_node BOLT12 offer: {error}; use `wallet invoice <amount_sats>` as the required BOLT11 fallback"
                )
            })?;
        let now = now_epoch_ms() as u64;
        Ok(WalletOfferReport {
            runtime: self.surface.clone(),
            offer: offer.to_string(),
            amount_sats,
            description: Some(description_text),
            created_at_ms: now,
            expires_at_ms: expiry_seconds
                .map(|seconds| now.saturating_add(u64::from(seconds).saturating_mul(1000))),
        })
    }

    fn pay(&self, payment_request: &str, amount_sats: Option<u64>) -> Result<WalletPayReport> {
        let payment_request = payment_request.trim();
        if payment_request.is_empty() {
            bail!("wallet pay requires a payment request");
        }
        if let Ok(invoice) = Bolt11Invoice::from_str(payment_request) {
            return self.send_bolt11(invoice, payment_request, amount_sats);
        }
        if let Ok(offer) = LdkOffer::from_str(payment_request) {
            return self.send_bolt12_offer(offer, payment_request, amount_sats);
        }
        if is_bitcoin_uri(payment_request) {
            return self.send_bitcoin_uri_or_onchain(payment_request, amount_sats);
        }
        if looks_like_bitcoin_address(payment_request) {
            return self.send_onchain_address(payment_request, amount_sats);
        }
        if looks_like_bip353(payment_request) {
            bail!(
                "bip353_send_unavailable: ldk_node 0.7 does not expose BIP353 name resolution in this build; resolve `{payment_request}` to a BOLT12 offer, BOLT11 invoice, BIP21 URI, or on-chain address first"
            );
        }
        bail!(
            "unsupported_payment_request: expected BOLT11 invoice, BOLT12 offer, BIP21 bitcoin URI, BIP353 name, or on-chain address"
        )
    }

    fn withdraw(&self, payment_request: &str, amount_sats: Option<u64>) -> Result<WalletPayReport> {
        let payment_request = payment_request.trim();
        if is_bitcoin_uri(payment_request) {
            return self.send_bitcoin_uri_or_onchain(payment_request, amount_sats);
        }
        if looks_like_bitcoin_address(payment_request) {
            return self.send_onchain_address(payment_request, amount_sats);
        }
        self.pay(payment_request, amount_sats)
    }

    fn list_payments(
        &self,
        ledger: &PylonLedger,
        limit: Option<u32>,
    ) -> Result<Vec<PylonWalletPaymentRecord>> {
        let ldk_payments = self
            .with_node(|node| {
                node.list_payments()
                    .into_iter()
                    .map(ldk_payment_details_to_wallet_record)
                    .collect::<Vec<_>>()
            })?
            .unwrap_or_default();
        Ok(merge_wallet_payment_records(
            ledger_payments(ledger, None),
            ldk_payments,
            limit.map(|value| value as usize),
        ))
    }

    fn list_channels(&self) -> Result<Vec<PylonWalletChannelRecord>> {
        let channels = self.with_node(|node| node.list_channels())?;
        Ok(channels
            .unwrap_or_default()
            .into_iter()
            .map(|channel| PylonWalletChannelRecord {
                channel_id: channel.channel_id.to_string(),
                status: if channel.is_usable {
                    "usable".to_string()
                } else if channel.is_channel_ready {
                    "ready".to_string()
                } else {
                    "pending".to_string()
                },
                counterparty_node_id: Some(channel.counterparty_node_id.to_string()),
                funding_txo: channel.funding_txo.map(|outpoint| outpoint.to_string()),
                channel_value_sats: channel.channel_value_sats,
                inbound_sats: channel.inbound_capacity_msat / 1000,
                outbound_sats: channel.outbound_capacity_msat / 1000,
                inbound_htlc_maximum_sats: channel.inbound_htlc_maximum_msat.map(msat_to_sats),
                next_outbound_htlc_limit_sats: msat_to_sats(channel.next_outbound_htlc_limit_msat),
                confirmations: channel.confirmations,
                confirmations_required: channel.confirmations_required,
                is_outbound: channel.is_outbound,
                is_public: channel.is_announced,
                peer_connected: channel.is_usable,
            })
            .collect())
    }
}

impl LdkNodeWalletRuntime {
    fn build_node(&self, layout: &WalletStorageLayout) -> Result<ldk_node::Node> {
        let mut builder = LdkNodeBuilder::new();
        builder.set_network(parse_ldk_bitcoin_network(self.surface.network.as_str())?);
        builder.set_entropy_seed_bytes(self.entropy);
        builder.set_storage_dir_path(layout.sqlite_dir.display().to_string());
        match self.settings.chain_source_kind.as_str() {
            "none" => {}
            "esplora" => {
                let url = self
                    .settings
                    .esplora_url
                    .clone()
                    .ok_or_else(|| anyhow!("wallet_esplora_url must be set"))?;
                builder.set_chain_source_esplora(url, None);
            }
            "electrum" => {
                let url = self
                    .settings
                    .electrum_url
                    .clone()
                    .ok_or_else(|| anyhow!("wallet_electrum_url must be set"))?;
                builder.set_chain_source_electrum(url, None);
            }
            other => bail!(
                "unsupported wallet_chain_source_kind '{other}'; expected none, esplora, or electrum"
            ),
        }
        if let Some(rgs_url) = self.settings.rgs_url.clone() {
            builder.set_gossip_source_rgs(rgs_url);
        }
        builder
            .build()
            .map_err(|error| anyhow!("failed to build ldk node: {error}"))
    }

    fn with_node<T>(&self, operation: impl FnOnce(&ldk_node::Node) -> T) -> Result<Option<T>> {
        let node = self
            .node
            .lock()
            .map_err(|_| anyhow!("ldk node state is poisoned"))?;
        Ok(node.as_ref().map(operation))
    }

    fn require_node<T>(&self, operation: impl FnOnce(&ldk_node::Node) -> T) -> Result<T> {
        self.with_node(operation)?
            .ok_or_else(|| anyhow!("ldk_node is not initialized; run wallet status first"))
    }

    fn send_bolt11(
        &self,
        invoice: Bolt11Invoice,
        payment_request: &str,
        amount_sats: Option<u64>,
    ) -> Result<WalletPayReport> {
        let invoice_amount_msat = invoice.amount_milli_satoshis();
        let explicit_amount_msat = amount_sats.map(sats_to_msat).transpose()?;
        let amount_msat = explicit_amount_msat.or(invoice_amount_msat);
        if amount_msat.is_none() {
            bail!("ambiguous_amount: BOLT11 invoice has no amount; pass --amount-sats");
        }
        let payment_id_result = self.require_node(|node| {
            let payment = node.bolt11_payment();
            if let Some(explicit_amount_msat) = explicit_amount_msat {
                payment.send_using_amount(&invoice, explicit_amount_msat, None)
            } else {
                payment.send(&invoice, None)
            }
        })?;
        let payment_id = payment_id_result.map_err(|error| {
            anyhow!("ldk_wallet_send_failed: BOLT11 payment send failed: {error}")
        })?;
        let amount_sats = msat_to_sats(amount_msat.unwrap_or_default());
        self.outbound_payment_report(
            payment_id_to_string(payment_id),
            "pending",
            amount_sats,
            0,
            "bolt11",
            Some("ldk_node BOLT11 payment submitted".to_string()),
            Some(payment_request.to_string()),
        )
    }

    fn send_bolt12_offer(
        &self,
        offer: LdkOffer,
        payment_request: &str,
        amount_sats: Option<u64>,
    ) -> Result<WalletPayReport> {
        let offer_amount_msat = match offer.amount() {
            Some(LdkOfferAmount::Bitcoin { amount_msats }) => Some(amount_msats),
            Some(_) => bail!("unsupported_currency: BOLT12 offer is not denominated in bitcoin"),
            None => None,
        };
        let explicit_amount_msat = amount_sats.map(sats_to_msat).transpose()?;
        let amount_msat = explicit_amount_msat.or(offer_amount_msat);
        if amount_msat.is_none() {
            bail!("ambiguous_amount: BOLT12 offer has no amount; pass --amount-sats");
        }
        let payment_id_result = self.require_node(|node| {
            let payment = node.bolt12_payment();
            if let Some(explicit_amount_msat) = explicit_amount_msat {
                payment.send_using_amount(&offer, explicit_amount_msat, None, None, None)
            } else {
                payment.send(&offer, None, None, None)
            }
        })?;
        let payment_id = payment_id_result.map_err(|error| {
            anyhow!("ldk_wallet_send_failed: BOLT12 offer payment send failed: {error}")
        })?;
        let amount_sats = msat_to_sats(amount_msat.unwrap_or_default());
        self.outbound_payment_report(
            payment_id_to_string(payment_id),
            "pending",
            amount_sats,
            0,
            "bolt12",
            Some("ldk_node BOLT12 offer payment submitted".to_string()),
            Some(payment_request.to_string()),
        )
    }

    fn send_bitcoin_uri_or_onchain(
        &self,
        payment_request: &str,
        amount_sats: Option<u64>,
    ) -> Result<WalletPayReport> {
        let uri = parse_bitcoin_uri(payment_request)?;
        let effective_amount_sats = match (amount_sats, uri.amount_sats) {
            (Some(explicit), Some(embedded)) if explicit != embedded => bail!(
                "amount_mismatch: --amount-sats {explicit} does not match BIP21 amount {embedded}"
            ),
            (Some(explicit), _) => Some(explicit),
            (None, embedded) => embedded,
        };
        let has_lightning = uri.has_lightning_offer || uri.has_lightning_invoice;
        if has_lightning {
            if amount_sats.is_some() && uri.amount_sats.is_none() {
                bail!(
                    "unified_qr_amount_override_unavailable: BIP21 lightning sends require an embedded amount; use a direct BOLT11/BOLT12 request with --amount-sats or include amount= in the URI"
                );
            }
            return self.send_unified_qr(payment_request, effective_amount_sats);
        }
        self.send_onchain_parts(uri.address.as_str(), effective_amount_sats, payment_request)
    }

    fn send_unified_qr(
        &self,
        payment_request: &str,
        amount_sats: Option<u64>,
    ) -> Result<WalletPayReport> {
        let result = self
            .require_node(|node| node.unified_qr_payment().send(payment_request, None))?
            .map_err(|error| {
                anyhow!("ldk_wallet_send_failed: BIP21 unified payment send failed: {error}")
            })?;
        match result {
            ldk_node::payment::QrPaymentResult::Bolt11 { payment_id } => self
                .outbound_payment_report(
                    payment_id_to_string(payment_id),
                    "pending",
                    amount_sats.unwrap_or_default(),
                    0,
                    "bolt11",
                    Some("ldk_node BIP21 BOLT11 payment submitted".to_string()),
                    Some(payment_request.to_string()),
                ),
            ldk_node::payment::QrPaymentResult::Bolt12 { payment_id } => self
                .outbound_payment_report(
                    payment_id_to_string(payment_id),
                    "pending",
                    amount_sats.unwrap_or_default(),
                    0,
                    "bolt12",
                    Some("ldk_node BIP21 BOLT12 payment submitted".to_string()),
                    Some(payment_request.to_string()),
                ),
            ldk_node::payment::QrPaymentResult::Onchain { txid } => self.outbound_payment_report(
                txid.to_string(),
                "pending",
                amount_sats.unwrap_or_default(),
                0,
                "onchain",
                Some("ldk_node BIP21 on-chain withdrawal submitted".to_string()),
                Some(payment_request.to_string()),
            ),
        }
    }

    fn send_onchain_address(
        &self,
        address: &str,
        amount_sats: Option<u64>,
    ) -> Result<WalletPayReport> {
        self.send_onchain_parts(address, amount_sats, address)
    }

    fn send_onchain_parts(
        &self,
        address: &str,
        amount_sats: Option<u64>,
        original_request: &str,
    ) -> Result<WalletPayReport> {
        let amount_sats = amount_sats.ok_or_else(|| {
            anyhow!(
                "ambiguous_amount: on-chain withdrawal requires --amount-sats or a BIP21 amount"
            )
        })?;
        if amount_sats == 0 {
            bail!("invalid_amount: on-chain withdrawal amount must be greater than 0");
        }
        let address = self.parse_onchain_address(address)?;
        let balance = self.node_balance_snapshot()?;
        if amount_sats > balance.spendable_onchain_sats {
            bail!(
                "insufficient_spendable_onchain_balance: requested {amount_sats} sats but only {} sats are spendable after retaining {} sats of anchor channel reserve",
                balance.spendable_onchain_sats,
                balance.anchor_reserve_sats
            );
        }
        let txid = self
            .require_node(|node| {
                node.onchain_payment()
                    .send_to_address(&address, amount_sats, None)
            })?
            .map_err(|error| {
                anyhow!("ldk_wallet_send_failed: on-chain withdrawal send failed: {error}")
            })?;
        self.outbound_payment_report(
            txid.to_string(),
            "pending",
            amount_sats,
            0,
            "onchain",
            Some("ldk_node on-chain withdrawal submitted".to_string()),
            Some(original_request.to_string()),
        )
    }

    fn parse_onchain_address(&self, address: &str) -> Result<LdkBitcoinAddress> {
        let network = parse_ldk_bitcoin_network(self.surface.network.as_str())?;
        LdkBitcoinAddress::from_str(address.trim())
            .with_context(|| "invalid on-chain address")?
            .require_network(network)
            .with_context(|| {
                format!(
                    "on-chain address is not valid for wallet_network {}",
                    self.surface.network
                )
            })
    }

    fn outbound_payment_report(
        &self,
        payment_id: String,
        status: &str,
        amount_sats: u64,
        fees_sats: u64,
        method: &str,
        description: Option<String>,
        invoice: Option<String>,
    ) -> Result<WalletPayReport> {
        let now = now_epoch_ms() as u64;
        let post_balance = self.node_balance_snapshot().unwrap_or_default();
        let txid = method
            .eq_ignore_ascii_case("onchain")
            .then(|| payment_id.clone());
        let payment = PylonWalletPaymentRecord {
            payment_id: payment_id.clone(),
            direction: "send".to_string(),
            status: status.to_string(),
            amount_sats,
            fees_sats,
            method: method.to_string(),
            description,
            invoice,
            payment_hash: None,
            txid,
            operation_id: None,
            receipt_id: None,
            failure_code: None,
            created_at_ms: now,
            updated_at_ms: now,
        };
        Ok(WalletPayReport {
            runtime: self.surface.clone(),
            payment_id,
            payment,
            post_balance,
        })
    }

    fn node_balance_snapshot(&self) -> Result<WalletBalanceSnapshot> {
        let balance = self.require_node(|node| node.list_balances())?;
        Ok(WalletBalanceSnapshot {
            credited_sats: 0,
            lightning_sats: balance.total_lightning_balance_sats,
            onchain_sats: balance.total_onchain_balance_sats,
            spendable_onchain_sats: balance.spendable_onchain_balance_sats,
            anchor_reserve_sats: balance.total_anchor_channels_reserve_sats,
            total_sats: balance
                .total_lightning_balance_sats
                .saturating_add(balance.total_onchain_balance_sats),
        })
    }

    fn set_last_error(&self, error: Option<String>) -> Result<()> {
        let mut last_error = self
            .last_error
            .lock()
            .map_err(|_| anyhow!("ldk node error state is poisoned"))?;
        *last_error = error;
        Ok(())
    }

    fn last_error(&self) -> Result<Option<String>> {
        let last_error = self
            .last_error
            .lock()
            .map_err(|_| anyhow!("ldk node error state is poisoned"))?;
        Ok(last_error.clone())
    }

    fn ldk_node_status(&self) -> Result<WalletLdkNodeStatus> {
        let layout =
            wallet_storage_layout_from_root(PathBuf::from(self.surface.storage_dir.as_str()));
        let backup_summary = load_wallet_backup_status_summary(&layout)?;
        let status = self.with_node(|node| (node.node_id().to_string(), node.status()))?;
        let (node_id, node_status) = match status {
            Some((node_id, status)) => (Some(node_id), Some(status)),
            None => (None, None),
        };
        Ok(WalletLdkNodeStatus {
            node_id,
            network: self.surface.network.clone(),
            storage_dir: layout.sqlite_dir.display().to_string(),
            storage_schema_version: WALLET_STORAGE_SCHEMA_VERSION,
            storage_generation: wallet_storage_generation_id(&layout, &self.surface),
            chain_source_kind: self.settings.chain_source_kind.clone(),
            chain_source_url: self
                .configured_chain_source_url()
                .map(|url| redact_wallet_telemetry_endpoint(url.as_str())),
            rgs_url: self
                .settings
                .rgs_url
                .as_deref()
                .map(redact_wallet_telemetry_endpoint),
            backup_status: backup_summary.status,
            backup_manifest_present: backup_summary.manifest_present,
            backup_artifact_count: backup_summary.artifact_count,
            backup_stale: backup_summary.stale,
            backup_stale_after_ms: WALLET_BACKUP_STALE_AFTER_MS,
            last_backup_exported_at_ms: backup_summary.last_exported_at_ms,
            last_backup_file_digest: backup_summary.last_file_digest,
            is_running: node_status
                .as_ref()
                .map(|status| status.is_running)
                .unwrap_or(false),
            latest_lightning_wallet_sync_timestamp: node_status
                .as_ref()
                .and_then(|status| status.latest_lightning_wallet_sync_timestamp),
            latest_onchain_wallet_sync_timestamp: node_status
                .as_ref()
                .and_then(|status| status.latest_onchain_wallet_sync_timestamp),
            latest_rgs_snapshot_timestamp: node_status
                .as_ref()
                .and_then(|status| status.latest_rgs_snapshot_timestamp),
            last_error: self
                .last_error()?
                .map(|error| redact_wallet_secret_text(error.as_str())),
        })
    }

    fn configured_chain_source_url(&self) -> Option<String> {
        match self.settings.chain_source_kind.as_str() {
            "esplora" => self.settings.esplora_url.clone(),
            "electrum" => self.settings.electrum_url.clone(),
            _ => None,
        }
    }
}

fn parse_ldk_bitcoin_network(network: &str) -> Result<LdkBitcoinNetwork> {
    network
        .parse::<LdkBitcoinNetwork>()
        .with_context(|| format!("unsupported ldk_node wallet_network '{network}'"))
}

fn wallet_storage_generation_id(
    layout: &WalletStorageLayout,
    surface: &WalletRuntimeSurface,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(WALLET_STORAGE_SCHEMA_VERSION.to_be_bytes());
    hasher.update(surface.runtime_kind.id().as_bytes());
    hasher.update(surface.network.as_bytes());
    hasher.update(surface.node_entropy.digest.as_bytes());
    hasher.update(layout.ldk_dir.to_string_lossy().as_bytes());
    let digest = hex::encode(hasher.finalize());
    format!("sha256:{}", &digest[..16])
}

fn wallet_backup_artifact_count(layout: &WalletStorageLayout) -> Result<usize> {
    if !layout.backup_staging_dir.is_dir() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in std::fs::read_dir(layout.backup_staging_dir.as_path()).with_context(|| {
        format!(
            "failed to inspect wallet backup staging directory {}",
            layout.backup_staging_dir.display()
        )
    })? {
        let entry = entry.with_context(|| {
            format!(
                "failed to inspect wallet backup staging directory {}",
                layout.backup_staging_dir.display()
            )
        })?;
        if entry.file_type().map(|file_type| file_type.is_file())? {
            count = count.saturating_add(1);
        }
    }
    Ok(count)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct WalletBackupStatusSummary {
    status: String,
    manifest_present: bool,
    artifact_count: usize,
    stale: bool,
    last_exported_at_ms: Option<u64>,
    last_file_digest: Option<String>,
}

fn load_wallet_backup_status_summary(
    layout: &WalletStorageLayout,
) -> Result<WalletBackupStatusSummary> {
    let manifest_present = layout.backup_manifest_path.is_file();
    let artifact_count = wallet_backup_artifact_count(layout)?;
    if !manifest_present {
        return Ok(WalletBackupStatusSummary {
            status: "backup_missing_manifest".to_string(),
            manifest_present,
            artifact_count,
            stale: true,
            last_exported_at_ms: None,
            last_file_digest: None,
        });
    }
    let manifest = read_wallet_backup_status_manifest(layout)?;
    let now_ms = now_epoch_ms() as u64;
    let stale = manifest
        .last_exported_at_ms
        .map(|exported_at_ms| now_ms.saturating_sub(exported_at_ms) > WALLET_BACKUP_STALE_AFTER_MS)
        .unwrap_or(true);
    let status = match (manifest.last_exported_at_ms, stale) {
        (None, _) => "backup_missing",
        (Some(_), true) => "backup_stale",
        (Some(_), false) => "backup_current",
    }
    .to_string();
    Ok(WalletBackupStatusSummary {
        status,
        manifest_present,
        artifact_count,
        stale,
        last_exported_at_ms: manifest.last_exported_at_ms,
        last_file_digest: manifest.last_export_file_digest,
    })
}

fn read_wallet_backup_status_manifest(
    layout: &WalletStorageLayout,
) -> Result<WalletBackupStatusManifest> {
    let raw =
        std::fs::read_to_string(layout.backup_manifest_path.as_path()).with_context(|| {
            format!(
                "failed to read wallet backup manifest {}",
                layout.backup_manifest_path.display()
            )
        })?;
    let mut manifest: WalletBackupStatusManifest = serde_json::from_str(raw.as_str())
        .with_context(|| {
            format!(
                "failed to parse wallet backup manifest {}",
                layout.backup_manifest_path.display()
            )
        })?;
    if manifest.schema_version == 0 {
        manifest.schema_version = WALLET_BACKUP_SCHEMA_VERSION;
    }
    if manifest.kind.is_empty() {
        manifest.kind = WALLET_BACKUP_MANIFEST_KIND.to_string();
    }
    if manifest.status.is_empty() {
        manifest.status = "none".to_string();
    }
    Ok(manifest)
}

fn write_wallet_backup_status_manifest(
    layout: &WalletStorageLayout,
    report: &WalletBackupExportReport,
) -> Result<()> {
    let manifest = WalletBackupStatusManifest {
        schema_version: WALLET_BACKUP_SCHEMA_VERSION,
        kind: WALLET_BACKUP_MANIFEST_KIND.to_string(),
        status: "exported".to_string(),
        last_exported_at_ms: Some(report.exported_at_ms),
        last_export_path: Some(report.path.clone()),
        last_export_file_digest: Some(report.file_digest.clone()),
        last_plaintext_manifest_digest: Some(report.manifest.plaintext_manifest_digest.clone()),
        last_component_count: Some(report.manifest.plaintext_component_count),
        last_total_bytes: Some(report.manifest.plaintext_total_bytes),
        encryption_algorithm: Some(report.manifest.encryption_algorithm.clone()),
        kdf: Some(report.manifest.kdf.clone()),
    };
    write_private_json(
        layout.backup_manifest_path.as_path(),
        &manifest,
        "wallet backup manifest",
    )
}

fn ledger_balance(ledger: &PylonLedger) -> WalletBalanceSnapshot {
    WalletBalanceSnapshot {
        total_sats: ledger.wallet.last_balance_sats.unwrap_or_default(),
        ..WalletBalanceSnapshot::default()
    }
}

fn ledger_payments(ledger: &PylonLedger, limit: Option<usize>) -> Vec<PylonWalletPaymentRecord> {
    ledger
        .wallet
        .payments
        .iter()
        .take(limit.unwrap_or(usize::MAX))
        .cloned()
        .collect()
}

fn merge_wallet_payment_records(
    mut ledger_payments: Vec<PylonWalletPaymentRecord>,
    runtime_payments: Vec<PylonWalletPaymentRecord>,
    limit: Option<usize>,
) -> Vec<PylonWalletPaymentRecord> {
    for runtime_payment in runtime_payments {
        if let Some(existing) = ledger_payments
            .iter_mut()
            .find(|payment| payment.payment_id == runtime_payment.payment_id)
        {
            let created_at_ms = existing.created_at_ms;
            let mut merged = runtime_payment;
            if merged.description.is_none() {
                merged.description = existing.description.clone();
            }
            if merged.invoice.is_none() {
                merged.invoice = existing.invoice.clone();
            }
            if merged.operation_id.is_none() {
                merged.operation_id = existing.operation_id.clone();
            }
            if merged.receipt_id.is_none() {
                merged.receipt_id = existing.receipt_id.clone();
            }
            *existing = merged;
            existing.created_at_ms = created_at_ms;
        } else {
            ledger_payments.push(runtime_payment);
        }
    }
    ledger_payments.sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
    if let Some(limit) = limit {
        ledger_payments.truncate(limit);
    }
    ledger_payments
}

fn parse_mdk_stdout_json(stdout: &str) -> Result<Value> {
    if let Ok(value) = serde_json::from_str::<Value>(stdout.trim()) {
        return Ok(value);
    }
    for line in stdout.lines().rev() {
        let line = line.trim();
        if line.starts_with('{') || line.starts_with('[') {
            return serde_json::from_str::<Value>(line)
                .with_context(|| "failed to parse MoneyDevKit JSON line");
        }
    }
    bail!("MoneyDevKit command did not emit a JSON object on stdout")
}

fn mdk_json_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn mdk_json_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
                .or_else(|| value.as_str().and_then(|value| value.parse::<u64>().ok()))
        })
}

fn mdk_json_bool(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(|value| {
            value.as_bool().or_else(|| {
                value.as_str().and_then(|value| match value {
                    "true" | "1" | "yes" => Some(true),
                    "false" | "0" | "no" => Some(false),
                    _ => None,
                })
            })
        })
}

fn short_wallet_digest_id(prefix: &str, value: &str) -> String {
    let digest = hex::encode(Sha256::digest(value.as_bytes()));
    format!("{prefix}:{}", &digest[..16])
}

fn redact_mdk_command_output(value: &str) -> String {
    let mut redacted = redact_wallet_secret_text(value);
    if let Ok(mut json) = parse_mdk_stdout_json(value) {
        redact_mdk_json_value(&mut json);
        if let Ok(rendered) = serde_json::to_string(&json) {
            redacted = rendered;
        }
    }
    redacted
}

fn redact_mdk_json_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if matches!(
                    key.as_str(),
                    "mnemonic" | "preimage" | "private_key" | "secret" | "token"
                ) {
                    *value = Value::String("[redacted]".to_string());
                } else {
                    redact_mdk_json_value(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_mdk_json_value(value);
            }
        }
        _ => {}
    }
}

fn mdk_payment_record_from_value(value: &Value) -> Option<PylonWalletPaymentRecord> {
    let amount_sats = mdk_json_u64(value, &["amountSats", "amount_sats"])?;
    let timestamp = mdk_json_u64(value, &["timestamp", "createdAt", "created_at"])
        .unwrap_or_else(|| now_epoch_ms() as u64);
    let timestamp_ms = if timestamp < 10_000_000_000 {
        timestamp.saturating_mul(1_000)
    } else {
        timestamp
    };
    let payment_hash = mdk_json_string(value, &["paymentHash", "payment_hash"]);
    let payment_id = mdk_json_string(value, &["paymentId", "payment_id"])
        .or_else(|| payment_hash.clone())
        .unwrap_or_else(|| short_wallet_digest_id("mdk-payment", value.to_string().as_str()));
    let direction = match mdk_json_string(value, &["direction"])?.as_str() {
        "inbound" | "receive" => "receive",
        "outbound" | "send" => "send",
        _ => "unknown",
    }
    .to_string();
    let destination = mdk_json_string(value, &["destination"]);
    let payer_note = mdk_json_string(value, &["payerNote", "payer_note"]);
    let method = destination
        .as_deref()
        .map(infer_wallet_payment_method)
        .unwrap_or_else(|| "lightning".to_string());
    Some(PylonWalletPaymentRecord {
        payment_id: payment_id.clone(),
        direction,
        status: mdk_json_string(value, &["status"]).unwrap_or_else(|| "completed".to_string()),
        amount_sats,
        fees_sats: 0,
        method,
        description: payer_note.or_else(|| Some("MoneyDevKit agent-wallet payment".to_string())),
        invoice: destination,
        payment_hash,
        txid: None,
        operation_id: None,
        receipt_id: Some(format!("wallet:moneydevkit:{payment_id}")),
        failure_code: None,
        created_at_ms: timestamp_ms,
        updated_at_ms: timestamp_ms,
    })
}

fn ldk_payment_details_to_wallet_record(details: LdkPaymentDetails) -> PylonWalletPaymentRecord {
    let (method, payment_hash, txid, detail) = ldk_payment_kind_metadata(&details.kind);
    let status = match details.status {
        LdkPaymentStatus::Pending => "pending",
        LdkPaymentStatus::Succeeded => "succeeded",
        LdkPaymentStatus::Failed => "failed",
    };
    let direction = match details.direction {
        LdkPaymentDirection::Inbound => "receive",
        LdkPaymentDirection::Outbound => "send",
    };
    let updated_at_ms = details.latest_update_timestamp.saturating_mul(1_000);
    let payment_id = payment_id_to_string(details.id);
    let receipt_id = Some(format!("wallet:ldk_node:{method}:{payment_id}"));
    PylonWalletPaymentRecord {
        payment_id,
        direction: direction.to_string(),
        status: status.to_string(),
        amount_sats: details.amount_msat.map(msat_to_sats).unwrap_or_default(),
        fees_sats: details.fee_paid_msat.map(msat_to_sats).unwrap_or_default(),
        method,
        description: detail,
        invoice: None,
        payment_hash,
        txid,
        operation_id: None,
        receipt_id,
        failure_code: matches!(details.status, LdkPaymentStatus::Failed)
            .then(|| "ldk_payment_failed".to_string()),
        created_at_ms: updated_at_ms,
        updated_at_ms,
    }
}

fn ldk_payment_kind_metadata(
    kind: &LdkPaymentKind,
) -> (String, Option<String>, Option<String>, Option<String>) {
    match kind {
        LdkPaymentKind::Onchain { txid, .. } => (
            "onchain".to_string(),
            None,
            Some(txid.to_string()),
            Some("ldk_node on-chain payment".to_string()),
        ),
        LdkPaymentKind::Bolt11 { hash, .. } => (
            "bolt11".to_string(),
            Some(hex::encode(hash.0)),
            None,
            Some("ldk_node BOLT11 payment".to_string()),
        ),
        LdkPaymentKind::Bolt11Jit { hash, .. } => (
            "bolt11_jit".to_string(),
            Some(hex::encode(hash.0)),
            None,
            Some("ldk_node BOLT11 JIT payment".to_string()),
        ),
        LdkPaymentKind::Bolt12Offer { hash, offer_id, .. } => (
            "bolt12".to_string(),
            hash.as_ref().map(|hash| hex::encode(hash.0)),
            None,
            Some(format!("ldk_node BOLT12 offer payment {offer_id}")),
        ),
        LdkPaymentKind::Bolt12Refund { hash, .. } => (
            "bolt12_refund".to_string(),
            hash.as_ref().map(|hash| hex::encode(hash.0)),
            None,
            Some("ldk_node BOLT12 refund payment".to_string()),
        ),
        LdkPaymentKind::Spontaneous { hash, .. } => (
            "keysend".to_string(),
            Some(hex::encode(hash.0)),
            None,
            Some("ldk_node spontaneous payment".to_string()),
        ),
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct BitcoinUriParts {
    address: String,
    amount_sats: Option<u64>,
    has_lightning_invoice: bool,
    has_lightning_offer: bool,
}

fn payment_id_to_string(payment_id: LdkPaymentId) -> String {
    hex::encode(payment_id.0)
}

fn sats_to_msat(amount_sats: u64) -> Result<u64> {
    amount_sats
        .checked_mul(1_000)
        .ok_or_else(|| anyhow!("payment amount is too large"))
}

fn msat_to_sats(amount_msat: u64) -> u64 {
    amount_msat.saturating_add(999) / 1_000
}

fn is_bitcoin_uri(value: &str) -> bool {
    value
        .get(..8)
        .map(|prefix| prefix.eq_ignore_ascii_case("bitcoin:"))
        .unwrap_or(false)
}

fn parse_bitcoin_uri(value: &str) -> Result<BitcoinUriParts> {
    if !is_bitcoin_uri(value) {
        bail!("invalid BIP21 bitcoin URI");
    }
    let body = value
        .get(8..)
        .ok_or_else(|| anyhow!("invalid BIP21 bitcoin URI"))?;
    let (address, query) = match body.split_once('?') {
        Some((address, query)) => (address.trim(), Some(query)),
        None => (body.trim(), None),
    };
    if address.is_empty() {
        bail!("invalid BIP21 bitcoin URI: missing address");
    }
    let mut parts = BitcoinUriParts {
        address: address.to_string(),
        ..BitcoinUriParts::default()
    };
    if let Some(query) = query {
        for pair in query.split('&') {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            match key.to_ascii_lowercase().as_str() {
                "amount" => parts.amount_sats = Some(parse_bip21_amount_sats(value)?),
                "lightning" => parts.has_lightning_invoice = true,
                "lno" => parts.has_lightning_offer = true,
                _ => {}
            }
        }
    }
    Ok(parts)
}

fn parse_bip21_amount_sats(value: &str) -> Result<u64> {
    let value = value.trim();
    if value.is_empty() || value.starts_with('-') {
        bail!("invalid BIP21 amount");
    }
    let (whole, fractional) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty() || !whole.chars().all(|character| character.is_ascii_digit()) {
        bail!("invalid BIP21 amount");
    }
    if !fractional
        .chars()
        .all(|character| character.is_ascii_digit())
        || fractional.len() > 8
    {
        bail!("invalid BIP21 amount precision");
    }
    let whole_sats = whole
        .parse::<u64>()
        .with_context(|| "invalid BIP21 amount")?
        .checked_mul(SATS_PER_BTC)
        .ok_or_else(|| anyhow!("BIP21 amount is too large"))?;
    let mut fractional_text = fractional.to_string();
    while fractional_text.len() < 8 {
        fractional_text.push('0');
    }
    let fractional_sats = if fractional_text.is_empty() {
        0
    } else {
        fractional_text
            .parse::<u64>()
            .with_context(|| "invalid BIP21 amount")?
    };
    whole_sats
        .checked_add(fractional_sats)
        .ok_or_else(|| anyhow!("BIP21 amount is too large"))
}

fn looks_like_bip353(value: &str) -> bool {
    let value = value.trim();
    let Some((name, domain)) = value.split_once('@') else {
        return false;
    };
    !name.is_empty()
        && domain.contains('.')
        && domain
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '.'))
}

fn looks_like_bitcoin_address(value: &str) -> bool {
    value
        .trim()
        .parse::<ldk_node::bitcoin::Address<ldk_node::bitcoin::address::NetworkUnchecked>>()
        .is_ok()
}

fn infer_wallet_payment_method(payment_request: &str) -> String {
    let payment_request = payment_request.trim();
    let lower = payment_request.to_ascii_lowercase();
    if lower.starts_with("ln") && !lower.starts_with("lno") {
        "bolt11".to_string()
    } else if lower.starts_with("lno") {
        "bolt12".to_string()
    } else if lower.starts_with("bitcoin:") {
        "bip21".to_string()
    } else if looks_like_bip353(payment_request) {
        "bip353".to_string()
    } else if looks_like_bitcoin_address(payment_request) {
        "onchain".to_string()
    } else {
        "unknown".to_string()
    }
}

fn infer_payment_request_amount_sats(payment_request: &str) -> Option<u64> {
    let payment_request = payment_request.trim();
    if let Ok(invoice) = Bolt11Invoice::from_str(payment_request) {
        return invoice.amount_milli_satoshis().map(msat_to_sats);
    }
    if let Ok(offer) = LdkOffer::from_str(payment_request) {
        return match offer.amount() {
            Some(LdkOfferAmount::Bitcoin { amount_msats }) => Some(msat_to_sats(amount_msats)),
            _ => None,
        };
    }
    if is_bitcoin_uri(payment_request) {
        return parse_bitcoin_uri(payment_request)
            .ok()
            .and_then(|uri| uri.amount_sats);
    }
    None
}

fn first_failure_code(error: &str) -> Option<String> {
    let code = error
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .find(|part| !part.is_empty())?;
    Some(code.to_ascii_lowercase())
}

fn wallet_telemetry_from_status(
    status: &WalletStatusReport,
    channels: &[PylonWalletChannelRecord],
    channel_error: Option<&str>,
) -> WalletTelemetryReport {
    let ldk_node = status.ldk_node.as_ref();
    let node_id = ldk_node.and_then(|node| node.node_id.clone());
    let channel_summary = wallet_telemetry_channel_summary(channels);
    let liquidity = wallet_telemetry_liquidity(channels);
    let backup = wallet_telemetry_backup(status);
    let sync = wallet_telemetry_sync(status);
    let errors = wallet_telemetry_errors(status, channel_error);
    let mut warnings = wallet_telemetry_warnings(status, &channel_summary, &backup, &sync);
    if let Some(channel_error) = channel_error {
        warnings.push(WalletTelemetrySignal {
            code: first_failure_code(channel_error)
                .unwrap_or_else(|| "wallet_channel_telemetry_unavailable".to_string()),
            detail: channel_error.to_string(),
        });
    }
    let receive_ready = status.lightning_readiness.can_receive_lightning;
    let send_ready =
        status.lightning_readiness.can_send_lightning || status.balance.spendable_onchain_sats > 0;
    let backup_ready = backup.status == "backup_current";
    let has_error = !errors.is_empty() || status.runtime_status.eq_ignore_ascii_case("error");
    let payable = status.runtime.runtime_kind == PylonWalletRuntimeKind::LdkNode
        && sync.is_running
        && receive_ready
        && send_ready
        && !has_error;
    let payable = payable
        || (status.runtime.runtime_kind == PylonWalletRuntimeKind::MoneyDevKit && !has_error);
    let health_state = if has_error {
        "error"
    } else if payable {
        "payable"
    } else if status.runtime.runtime_kind != PylonWalletRuntimeKind::LdkNode {
        "external"
    } else if !sync.is_running {
        "configured_not_running"
    } else if !receive_ready {
        "needs_inbound_liquidity"
    } else {
        "degraded"
    };
    WalletTelemetryReport {
        schema: "pylon.wallet.telemetry.v1".to_string(),
        generated_at_ms: now_epoch_ms() as u64,
        runtime: status.runtime.clone(),
        runtime_status: status.runtime_status.clone(),
        runtime_detail: status.runtime_detail.clone(),
        node_id,
        health: WalletTelemetryHealth {
            state: health_state.to_string(),
            payable,
            receive_ready,
            send_ready,
            backup_ready,
        },
        sources: WalletTelemetrySources {
            network: ldk_node
                .map(|node| node.network.clone())
                .unwrap_or_else(|| status.runtime.network.clone()),
            chain_source_kind: ldk_node
                .map(|node| node.chain_source_kind.clone())
                .unwrap_or_else(|| "not_applicable".to_string()),
            chain_source_url: ldk_node.and_then(|node| node.chain_source_url.clone()),
            gossip_source_kind: ldk_node
                .and_then(|node| node.rgs_url.as_ref())
                .map(|_| "rgs".to_string())
                .unwrap_or_else(|| "none".to_string()),
            gossip_source_url: ldk_node.and_then(|node| node.rgs_url.clone()),
        },
        sync,
        balances: status.balance.clone(),
        channels: channel_summary,
        liquidity,
        lsp: status.lightning_readiness.lsp.clone(),
        backup,
        warnings,
        errors,
        redaction: WalletTelemetryRedaction {
            policy: "pylon.wallet.telemetry.redacted.v1".to_string(),
            forbidden_secret_classes: vec![
                "recovery_phrase".to_string(),
                "node_entropy".to_string(),
                "private_key".to_string(),
                "payment_preimage".to_string(),
                "bearer_token".to_string(),
                "raw_channel_state".to_string(),
            ],
            endpoint_credentials_redacted: true,
            raw_channel_state_excluded: true,
            raw_key_material_excluded: true,
        },
    }
}

fn wallet_telemetry_channel_summary(
    channels: &[PylonWalletChannelRecord],
) -> WalletTelemetryChannels {
    let mut summary = WalletTelemetryChannels {
        total_count: channels.len(),
        ..WalletTelemetryChannels::default()
    };
    for channel in channels {
        match channel.status.as_str() {
            "usable" => summary.usable_count = summary.usable_count.saturating_add(1),
            "ready" => summary.ready_count = summary.ready_count.saturating_add(1),
            "pending" => summary.pending_count = summary.pending_count.saturating_add(1),
            _ => summary.inactive_count = summary.inactive_count.saturating_add(1),
        }
    }
    summary
}

fn wallet_channels_report_from_channels(
    runtime: &WalletRuntimeSurface,
    channels: Vec<PylonWalletChannelRecord>,
) -> WalletChannelsReport {
    WalletChannelsReport {
        runtime: runtime.clone(),
        channel_summary: wallet_telemetry_channel_summary(channels.as_slice()),
        liquidity: wallet_telemetry_liquidity(channels.as_slice()),
        lightning_readiness: wallet_lightning_readiness(
            runtime.runtime_kind,
            runtime.liquidity_provider_kind,
            channels.as_slice(),
            true,
        ),
        channels,
    }
}

fn wallet_telemetry_liquidity(channels: &[PylonWalletChannelRecord]) -> WalletTelemetryLiquidity {
    let inbound_sats = channels
        .iter()
        .map(|channel| channel.inbound_sats)
        .sum::<u64>();
    let outbound_sats = channels
        .iter()
        .map(|channel| channel.outbound_sats)
        .sum::<u64>();
    WalletTelemetryLiquidity {
        inbound_sats,
        outbound_sats,
        inbound_bucket: wallet_liquidity_bucket(inbound_sats),
        outbound_bucket: wallet_liquidity_bucket(outbound_sats),
    }
}

fn wallet_lightning_readiness(
    runtime_kind: PylonWalletRuntimeKind,
    liquidity_provider_kind: PylonWalletLiquidityProviderKind,
    channels: &[PylonWalletChannelRecord],
    can_receive_onchain: bool,
) -> WalletLightningReadiness {
    let channel_summary = wallet_telemetry_channel_summary(channels);
    let liquidity = wallet_telemetry_liquidity(channels);
    let peer_connected_count = channels
        .iter()
        .filter(|channel| channel.peer_connected)
        .count();
    let lsp = wallet_lsp_readiness(runtime_kind, liquidity_provider_kind);
    let can_receive_lightning =
        channel_summary.usable_count > 0 && liquidity.inbound_sats > 0 && peer_connected_count > 0;
    let can_send_lightning =
        channel_summary.usable_count > 0 && liquidity.outbound_sats > 0 && peer_connected_count > 0;
    let (state, warning_code, warning, remediation) = if runtime_kind
        == PylonWalletRuntimeKind::MoneyDevKit
    {
        (
                "moneydevkit_agent_wallet_ready".to_string(),
                None,
                None,
                vec![
                    "MoneyDevKit agent-wallet manages Lightning liquidity behind the local daemon; raw channels are intentionally not exposed through Pylon."
                        .to_string(),
                ],
            )
    } else if runtime_kind == PylonWalletRuntimeKind::ExternalTarget {
        (
            "external_target".to_string(),
            None,
            None,
            vec![
                "Select wallet_runtime_kind=ldk_node to inspect built-in Lightning channels."
                    .to_string(),
            ],
        )
    } else if channel_summary.total_count == 0 {
        (
            "onchain_only_no_channels".to_string(),
            Some("lightning_receive_unavailable_no_channels".to_string()),
            Some(
                "Can receive on-chain, but Lightning receive is not viable yet because no channels are visible."
                    .to_string(),
            ),
            vec![
                "Fund the on-chain wallet, then open a channel or configure an LSPS1/LSPS2-capable LSP."
                    .to_string(),
                "Use wallet address for on-chain funding while Lightning inbound liquidity is unavailable."
                    .to_string(),
            ],
        )
    } else if channel_summary.usable_count == 0 && channel_summary.pending_count > 0 {
        (
            "channel_pending".to_string(),
            Some("lightning_receive_pending_channel".to_string()),
            Some("Can receive on-chain, but Lightning receive is waiting for the pending channel to become usable.".to_string()),
            vec![
                "Wait for funding confirmations and peer reconnection, then rerun wallet channels."
                    .to_string(),
            ],
        )
    } else if peer_connected_count == 0 {
        (
            "peer_disconnected".to_string(),
            Some("lightning_receive_peer_disconnected".to_string()),
            Some("A channel exists, but no channel peer is currently connected.".to_string()),
            vec![
                "Start the wallet with a live chain source and reconnect to the channel peer."
                    .to_string(),
            ],
        )
    } else if liquidity.inbound_sats == 0 {
        (
            "needs_inbound_liquidity".to_string(),
            Some("lightning_receive_needs_inbound_liquidity".to_string()),
            Some("Can receive on-chain, but Lightning receive needs inbound liquidity.".to_string()),
            vec![
                "Ask a peer/LSP to open inbound capacity, rebalance, or use an LSPS2 JIT receive path when configured."
                    .to_string(),
            ],
        )
    } else if liquidity.outbound_sats == 0 {
        (
            "receive_ready_send_limited".to_string(),
            Some("lightning_send_needs_outbound_liquidity".to_string()),
            Some("Lightning receive is viable, but Lightning sends need outbound liquidity.".to_string()),
            vec![
                "Add local channel balance or receive payments that move capacity outbound before sending."
                    .to_string(),
            ],
        )
    } else {
        ("lightning_ready".to_string(), None, None, Vec::new())
    };
    WalletLightningReadiness {
        state,
        can_receive_lightning: if runtime_kind == PylonWalletRuntimeKind::MoneyDevKit {
            true
        } else {
            can_receive_lightning
        },
        can_receive_onchain: if runtime_kind == PylonWalletRuntimeKind::MoneyDevKit {
            false
        } else {
            can_receive_onchain
        },
        can_send_lightning: if runtime_kind == PylonWalletRuntimeKind::MoneyDevKit {
            true
        } else {
            can_send_lightning
        },
        inbound_liquidity_sats: liquidity.inbound_sats,
        outbound_liquidity_sats: liquidity.outbound_sats,
        usable_channel_count: channel_summary.usable_count,
        pending_channel_count: channel_summary.pending_count,
        peer_connected_count,
        warning_code,
        warning,
        remediation,
        lsp,
    }
}

fn wallet_lsp_readiness(
    runtime_kind: PylonWalletRuntimeKind,
    provider_kind: PylonWalletLiquidityProviderKind,
) -> WalletLspReadiness {
    if !matches!(
        runtime_kind,
        PylonWalletRuntimeKind::LdkNode | PylonWalletRuntimeKind::MoneyDevKit
    ) {
        return WalletLspReadiness {
            provider_kind,
            supported_protocols: Vec::new(),
            configured: false,
            state: "not_applicable".to_string(),
            detail: "External wallet targets do not expose built-in Pylon LSP readiness."
                .to_string(),
        };
    }
    match provider_kind {
        PylonWalletLiquidityProviderKind::None => WalletLspReadiness {
            provider_kind,
            supported_protocols: vec!["lsps1".to_string(), "lsps2".to_string()],
            configured: false,
            state: "not_configured".to_string(),
            detail: "No wallet liquidity provider is selected. The linked ldk-node build exposes LSPS1 and LSPS2 hooks, but Pylon will not use a provider until wallet_liquidity_provider_kind is set.".to_string(),
        },
        PylonWalletLiquidityProviderKind::MoneyDevKit => WalletLspReadiness {
            provider_kind,
            supported_protocols: vec![
                "lsps1".to_string(),
                "lsps2".to_string(),
                "lsps4".to_string(),
                "jit_receive".to_string(),
                "vss_optional".to_string(),
            ],
            configured: true,
            state: "moneydevkit_selected".to_string(),
            detail: "MoneyDevKit agent-wallet is selected as the wrapped Pylon wallet runtime for LSP/JIT receive readiness. Pylon still owns Nexus payout target registration, accepted-work eligibility, and payout receipts.".to_string(),
        },
        PylonWalletLiquidityProviderKind::CustomLsps => WalletLspReadiness {
            provider_kind,
            supported_protocols: vec![
                "lsps1".to_string(),
                "lsps2".to_string(),
                "lsps4".to_string(),
                "jit_receive".to_string(),
            ],
            configured: true,
            state: "custom_lsps_selected".to_string(),
            detail: "A custom LSPS liquidity provider is selected. Pylon still owns the local LDK wallet, Nexus payout target registration, accepted-work eligibility, and payout receipts.".to_string(),
        },
    }
}

fn wallet_telemetry_backup(status: &WalletStatusReport) -> WalletTelemetryBackup {
    status
        .ldk_node
        .as_ref()
        .map(|node| WalletTelemetryBackup {
            status: node.backup_status.clone(),
            manifest_present: node.backup_manifest_present,
            artifact_count: node.backup_artifact_count,
            stale: node.backup_stale,
            stale_after_ms: node.backup_stale_after_ms,
            last_exported_at_ms: node.last_backup_exported_at_ms,
            last_file_digest: node.last_backup_file_digest.clone(),
        })
        .unwrap_or_else(|| WalletTelemetryBackup {
            status: "not_applicable".to_string(),
            ..WalletTelemetryBackup::default()
        })
}

fn wallet_telemetry_sync(status: &WalletStatusReport) -> WalletTelemetrySync {
    let Some(node) = status.ldk_node.as_ref() else {
        return WalletTelemetrySync {
            state: "not_applicable".to_string(),
            ..WalletTelemetrySync::default()
        };
    };
    let has_any_sync = node.latest_lightning_wallet_sync_timestamp.is_some()
        || node.latest_onchain_wallet_sync_timestamp.is_some()
        || node.latest_rgs_snapshot_timestamp.is_some();
    let state = if !node.is_running {
        "not_running"
    } else if has_any_sync {
        "synced"
    } else {
        "running_not_synced"
    };
    WalletTelemetrySync {
        state: state.to_string(),
        is_running: node.is_running,
        latest_lightning_wallet_sync_timestamp: node.latest_lightning_wallet_sync_timestamp,
        latest_onchain_wallet_sync_timestamp: node.latest_onchain_wallet_sync_timestamp,
        latest_rgs_snapshot_timestamp: node.latest_rgs_snapshot_timestamp,
    }
}

fn wallet_telemetry_errors(
    status: &WalletStatusReport,
    channel_error: Option<&str>,
) -> Vec<WalletTelemetrySignal> {
    let mut errors = Vec::new();
    if let Some(error) = status
        .ldk_node
        .as_ref()
        .and_then(|node| node.last_error.as_ref())
    {
        errors.push(WalletTelemetrySignal {
            code: first_failure_code(error).unwrap_or_else(|| "wallet_runtime_error".to_string()),
            detail: error.clone(),
        });
    } else if status.runtime_status.eq_ignore_ascii_case("error") {
        let detail = status
            .runtime_detail
            .as_deref()
            .unwrap_or("wallet runtime reported error");
        errors.push(WalletTelemetrySignal {
            code: first_failure_code(detail).unwrap_or_else(|| "wallet_runtime_error".to_string()),
            detail: detail.to_string(),
        });
    }
    if let Some(channel_error) = channel_error {
        errors.push(WalletTelemetrySignal {
            code: first_failure_code(channel_error)
                .unwrap_or_else(|| "wallet_channel_telemetry_unavailable".to_string()),
            detail: channel_error.to_string(),
        });
    }
    errors
}

fn wallet_telemetry_warnings(
    status: &WalletStatusReport,
    channels: &WalletTelemetryChannels,
    backup: &WalletTelemetryBackup,
    sync: &WalletTelemetrySync,
) -> Vec<WalletTelemetrySignal> {
    let mut warnings = Vec::new();
    if let Some(node) = status.ldk_node.as_ref() {
        if let (Some(code), Some(detail)) = (
            status.lightning_readiness.warning_code.as_ref(),
            status.lightning_readiness.warning.as_ref(),
        ) {
            warnings.push(WalletTelemetrySignal {
                code: code.clone(),
                detail: detail.clone(),
            });
        }
        if node.chain_source_kind == "none" {
            warnings.push(WalletTelemetrySignal {
                code: "wallet_chain_source_not_configured".to_string(),
                detail: "set wallet_chain_source_kind=esplora or electrum before expecting live Lightning sync".to_string(),
            });
        }
        if !sync.is_running {
            warnings.push(WalletTelemetrySignal {
                code: "wallet_node_not_running".to_string(),
                detail: "wallet node is configured but not connected to a chain source".to_string(),
            });
        }
        if backup.stale || backup.status != "backup_current" {
            warnings.push(WalletTelemetrySignal {
                code: backup.status.clone(),
                detail: "export an encrypted wallet backup to refresh recoverable Lightning state"
                    .to_string(),
            });
        }
        if channels.total_count == 0 {
            warnings.push(WalletTelemetrySignal {
                code: "wallet_no_channels".to_string(),
                detail: "no Lightning channels are visible, so routed receive/send readiness depends on channel setup".to_string(),
            });
        }
    }
    warnings
}

fn wallet_liquidity_bucket(sats: u64) -> String {
    match sats {
        0 => "zero",
        1..=9_999 => "low",
        10_000..=999_999 => "ready",
        _ => "large",
    }
    .to_string()
}

fn redact_wallet_telemetry_endpoint(value: &str) -> String {
    let mut redacted = value.trim().to_string();
    let endpoint = redacted.clone();
    if let Some((prefix, rest)) = endpoint.split_once("://") {
        let slash_index = rest.find('/').unwrap_or(rest.len());
        let authority = &rest[..slash_index];
        if let Some((_userinfo, host)) = authority.rsplit_once('@') {
            redacted = format!("{prefix}://[redacted]@{host}{}", &rest[slash_index..]);
        }
    }
    if let Some((base, _query)) = redacted.split_once('?') {
        redacted = format!("{base}?[redacted]");
    }
    if let Some((base, _fragment)) = redacted.split_once('#') {
        redacted = format!("{base}#[redacted]");
    }
    redacted
}

fn redact_wallet_secret_text(value: &str) -> String {
    let mut redacted = value.to_string();
    for marker in [
        "access_token=",
        "api_key=",
        "auth_token=",
        "bearer=",
        "entropy=",
        "mnemonic=",
        "preimage=",
        "private_key=",
        "refresh_token=",
        "secret=",
        "token=",
    ] {
        redacted = redact_assignment_marker(redacted.as_str(), marker);
    }
    redacted = redact_bearer_token(redacted.as_str());
    redacted
}

fn redact_assignment_marker(value: &str, marker: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(index) = rest.to_ascii_lowercase().find(marker) {
        output.push_str(&rest[..index]);
        output.push_str(marker);
        output.push_str("[redacted]");
        let value_start = index + marker.len();
        let value_tail = &rest[value_start..];
        let value_end = value_tail
            .find(|character: char| matches!(character, '&' | ' ' | '\n' | '\r' | '\t' | ',' | ';'))
            .unwrap_or(value_tail.len());
        rest = &value_tail[value_end..];
    }
    output.push_str(rest);
    output
}

fn redact_bearer_token(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(index) = rest.to_ascii_lowercase().find("bearer ") {
        output.push_str(&rest[..index]);
        output.push_str("Bearer [redacted]");
        let value_start = index + "bearer ".len();
        let value_tail = &rest[value_start..];
        let value_end = value_tail
            .find(|character: char| matches!(character, ' ' | '\n' | '\r' | '\t' | ',' | ';'))
            .unwrap_or(value_tail.len());
        rest = &value_tail[value_end..];
    }
    output.push_str(rest);
    output
}

fn failed_wallet_payment_record(
    payment_request: &str,
    amount_sats: Option<u64>,
    error: &str,
) -> PylonWalletPaymentRecord {
    let now = now_epoch_ms() as u64;
    let mut hasher = Sha256::new();
    hasher.update(payment_request.as_bytes());
    hasher.update(now.to_be_bytes());
    let digest = hex::encode(hasher.finalize());
    PylonWalletPaymentRecord {
        payment_id: format!("wallet-send-failed:{}", &digest[..16]),
        direction: "send".to_string(),
        status: "failed".to_string(),
        amount_sats: amount_sats
            .or_else(|| infer_payment_request_amount_sats(payment_request))
            .unwrap_or_default(),
        fees_sats: 0,
        method: infer_wallet_payment_method(payment_request),
        description: Some(redact_wallet_secret_text(error)),
        invoice: Some(payment_request.to_string()),
        payment_hash: None,
        txid: None,
        operation_id: None,
        receipt_id: None,
        failure_code: first_failure_code(error).or_else(|| Some("wallet_send_failed".to_string())),
        created_at_ms: now,
        updated_at_ms: now,
    }
}

fn wallet_status_receipt(
    runtime: &WalletRuntimeSurface,
    runtime_status: &str,
    detail: Option<&str>,
) -> PylonWalletReceiptRecord {
    let now = now_epoch_ms() as u64;
    PylonWalletReceiptRecord {
        receipt_id: format!(
            "wallet:status:{}:{}",
            runtime.runtime_kind.id(),
            runtime.network
        ),
        receipt_type: "wallet.status.v1".to_string(),
        status: runtime_status.to_string(),
        direction: None,
        method: Some(runtime.runtime_kind.id().to_string()),
        amount_sats: None,
        fees_sats: None,
        payment_id: None,
        payment_hash: None,
        txid: None,
        operation_id: Some(runtime.node_entropy.derivation_version.clone()),
        settlement_id: None,
        failure_code: runtime_status
            .eq_ignore_ascii_case("error")
            .then(|| detail.and_then(first_failure_code))
            .flatten(),
        detail: detail.map(ToString::to_string),
        created_at_ms: now,
        updated_at_ms: now,
    }
}

pub(crate) fn require_explicit_send_confirmation(
    command: &str,
    json: bool,
    yes: bool,
) -> Result<()> {
    if json || yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        bail!("{command} submits funds from the local Pylon wallet; rerun with --yes or --json")
    }
    eprint!("{command} submits funds from the local Pylon wallet. Type YES to continue: ");
    std::io::stderr()
        .flush()
        .with_context(|| "failed to flush confirmation prompt")?;
    let mut response = String::new();
    std::io::stdin()
        .read_line(&mut response)
        .with_context(|| "failed to read confirmation response")?;
    if response.trim() == "YES" {
        return Ok(());
    }
    bail!("{command} cancelled: confirmation did not match YES")
}

fn require_explicit_restore_confirmation(command: &str, yes: bool) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        bail!("{command} overwrites local wallet recovery state; rerun with --yes")
    }
    eprint!("{command} overwrites local wallet recovery state. Type YES to continue: ");
    std::io::stderr()
        .flush()
        .with_context(|| "failed to flush confirmation prompt")?;
    let mut response = String::new();
    std::io::stdin()
        .read_line(&mut response)
        .with_context(|| "failed to read confirmation response")?;
    if response.trim() == "YES" {
        return Ok(());
    }
    bail!("{command} cancelled: confirmation did not match YES")
}

pub async fn run_wallet_command(config_path: &Path, command: &WalletSubcommand) -> Result<String> {
    match command {
        WalletSubcommand::Status { json } => {
            let report = load_wallet_status_report(config_path).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_status_report(&report))
        }
        WalletSubcommand::Sync { json } => {
            let report = load_wallet_status_report(config_path).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_status_report(&report))
        }
        WalletSubcommand::Balance { json } => {
            let report = load_wallet_status_report(config_path).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report.balance)?);
            }
            Ok(render_wallet_balance_report(&report))
        }
        WalletSubcommand::Channels { json } => {
            let report = load_wallet_channels_report(config_path).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_channels_report(&report))
        }
        WalletSubcommand::Address { json } => {
            let report = create_wallet_address_report(config_path).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_address_report(&report))
        }
        WalletSubcommand::Invoice {
            amount_sats,
            description,
            expiry_seconds,
            json,
        } => {
            let report = create_wallet_invoice_report(
                config_path,
                *amount_sats,
                description.clone(),
                *expiry_seconds,
            )
            .await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_invoice_report(&report))
        }
        WalletSubcommand::Offer {
            amount_sats,
            description,
            expiry_seconds,
            json,
        } => {
            let report = create_wallet_offer_report(
                config_path,
                *amount_sats,
                description.clone(),
                *expiry_seconds,
            )
            .await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_offer_report(&report))
        }
        WalletSubcommand::Pay {
            payment_request,
            amount_sats,
            yes,
            json,
        } => {
            require_explicit_send_confirmation("wallet pay", *json, *yes)?;
            let report =
                pay_wallet_invoice_report(config_path, payment_request.as_str(), *amount_sats)
                    .await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_pay_report(&report))
        }
        WalletSubcommand::Telemetry { json } => {
            let report = load_wallet_telemetry_report(config_path).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_telemetry_report(&report))
        }
        WalletSubcommand::History { limit, json } => {
            let report = load_wallet_history_report(config_path, *limit).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_history_report(&report))
        }
        WalletSubcommand::BackupExport {
            path,
            passphrase_env,
            include_identity_mnemonic,
            json,
        } => {
            let report = export_wallet_backup_report(
                config_path,
                path.as_path(),
                passphrase_env.as_deref(),
                *include_identity_mnemonic,
            )
            .await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_backup_export_report(&report))
        }
        WalletSubcommand::BackupInspect { path, json } => {
            let report = inspect_wallet_backup_report(path.as_path())?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_backup_inspect_report(&report))
        }
        WalletSubcommand::RestorePhrase {
            mnemonic_env,
            mnemonic_file,
            wallet_network,
            yes,
            json,
        } => {
            require_explicit_restore_confirmation("wallet restore phrase", *yes)?;
            let report = restore_wallet_phrase_report(
                config_path,
                mnemonic_env.as_deref(),
                mnemonic_file.as_deref(),
                wallet_network.as_deref(),
            )
            .await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_restore_report(&report))
        }
        WalletSubcommand::RestoreBackup {
            path,
            passphrase_env,
            yes,
            json,
        } => {
            require_explicit_restore_confirmation("wallet restore backup", *yes)?;
            let report = restore_wallet_backup_report(
                config_path,
                path.as_path(),
                passphrase_env.as_deref(),
            )
            .await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_restore_report(&report))
        }
        WalletSubcommand::EntropyStatus { json } => {
            let report = load_wallet_entropy_status_report(config_path).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_entropy_report(&report))
        }
        WalletSubcommand::EntropyExport { path, json } => {
            let report = export_wallet_entropy_report(config_path, path.as_path()).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_entropy_report(&report))
        }
        WalletSubcommand::EntropyImport { path, json } => {
            let report = import_wallet_entropy_report(config_path, path.as_path()).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_entropy_report(&report))
        }
        WalletSubcommand::LockStatus { json } => {
            let report = inspect_wallet_lock_report(config_path).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_lock_report(&report))
        }
        WalletSubcommand::LockClear { json } => {
            let report = clear_wallet_lock_report(config_path).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_lock_report(&report))
        }
    }
}

pub fn parse_wallet_command(args: &[String], start_index: usize) -> Result<WalletSubcommand> {
    let subcommand = args
        .get(start_index + 1)
        .ok_or_else(|| anyhow!("missing wallet subcommand"))?;
    match subcommand.as_str() {
        "status" => Ok(WalletSubcommand::Status {
            json: parse_json_only(args, start_index + 2, "wallet status")?,
        }),
        "sync" => Ok(WalletSubcommand::Sync {
            json: parse_json_only(args, start_index + 2, "wallet sync")?,
        }),
        "balance" => Ok(WalletSubcommand::Balance {
            json: parse_json_only(args, start_index + 2, "wallet balance")?,
        }),
        "channels" => Ok(WalletSubcommand::Channels {
            json: parse_json_only(args, start_index + 2, "wallet channels")?,
        }),
        "telemetry" => Ok(WalletSubcommand::Telemetry {
            json: parse_json_only(args, start_index + 2, "wallet telemetry")?,
        }),
        "address" => Ok(WalletSubcommand::Address {
            json: parse_json_only(args, start_index + 2, "wallet address")?,
        }),
        "invoice" => {
            let amount_raw = args
                .get(start_index + 2)
                .ok_or_else(|| anyhow!("missing <amount_sats> for wallet invoice"))?;
            let amount_sats = amount_raw
                .parse::<u64>()
                .map_err(|error| anyhow!("invalid amount '{}': {error}", amount_raw))?;
            if amount_sats == 0 {
                bail!("wallet invoice amount must be greater than 0");
            }
            let mut description = None;
            let mut expiry_seconds = None;
            let mut json = false;
            let mut index = start_index + 3;
            while index < args.len() {
                match args[index].as_str() {
                    "--description" => {
                        index += 1;
                        let value = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --description"))?;
                        if value.trim().is_empty() {
                            bail!("--description cannot be empty");
                        }
                        description = Some(value.trim().to_string());
                        index += 1;
                    }
                    "--expiry-seconds" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --expiry-seconds"))?;
                        let value = raw.parse::<u32>().map_err(|error| {
                            anyhow!("invalid --expiry-seconds '{}': {error}", raw)
                        })?;
                        if value == 0 {
                            bail!("--expiry-seconds must be greater than 0");
                        }
                        expiry_seconds = Some(value);
                        index += 1;
                    }
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    other => bail!("unexpected argument for wallet invoice: {other}"),
                }
            }
            Ok(WalletSubcommand::Invoice {
                amount_sats,
                description,
                expiry_seconds,
                json,
            })
        }
        "offer" => {
            let mut amount_sats = None;
            let mut description = None;
            let mut expiry_seconds = None;
            let mut json = false;
            let mut index = start_index + 2;
            while index < args.len() {
                match args[index].as_str() {
                    "--amount-sats" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --amount-sats"))?;
                        let value = raw
                            .parse::<u64>()
                            .map_err(|error| anyhow!("invalid --amount-sats '{}': {error}", raw))?;
                        if value == 0 {
                            bail!("--amount-sats must be greater than 0");
                        }
                        amount_sats = Some(value);
                        index += 1;
                    }
                    "--description" => {
                        index += 1;
                        let value = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --description"))?;
                        if value.trim().is_empty() {
                            bail!("--description cannot be empty");
                        }
                        description = Some(value.trim().to_string());
                        index += 1;
                    }
                    "--expiry-seconds" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --expiry-seconds"))?;
                        let value = raw.parse::<u32>().map_err(|error| {
                            anyhow!("invalid --expiry-seconds '{}': {error}", raw)
                        })?;
                        if value == 0 {
                            bail!("--expiry-seconds must be greater than 0");
                        }
                        expiry_seconds = Some(value);
                        index += 1;
                    }
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    other => bail!("unexpected argument for wallet offer: {other}"),
                }
            }
            Ok(WalletSubcommand::Offer {
                amount_sats,
                description,
                expiry_seconds,
                json,
            })
        }
        "pay" => {
            let payment_request = args
                .get(start_index + 2)
                .ok_or_else(|| anyhow!("missing <payment_request> for wallet pay"))?
                .trim()
                .to_string();
            if payment_request.is_empty() {
                bail!("payment request cannot be empty");
            }
            let mut amount_sats = None;
            let mut yes = false;
            let mut json = false;
            let mut index = start_index + 3;
            while index < args.len() {
                match args[index].as_str() {
                    "--amount-sats" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --amount-sats"))?;
                        let value = raw
                            .parse::<u64>()
                            .map_err(|error| anyhow!("invalid --amount-sats '{}': {error}", raw))?;
                        if value == 0 {
                            bail!("--amount-sats must be greater than 0");
                        }
                        amount_sats = Some(value);
                        index += 1;
                    }
                    "--yes" => {
                        yes = true;
                        index += 1;
                    }
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    other => bail!("unexpected argument for wallet pay: {other}"),
                }
            }
            Ok(WalletSubcommand::Pay {
                payment_request,
                amount_sats,
                yes,
                json,
            })
        }
        "history" => {
            let mut limit = None;
            let mut json = false;
            let mut index = start_index + 2;
            while index < args.len() {
                match args[index].as_str() {
                    "--limit" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --limit"))?;
                        let value = raw
                            .parse::<u32>()
                            .map_err(|error| anyhow!("invalid --limit '{}': {error}", raw))?;
                        if value == 0 {
                            bail!("--limit must be greater than 0");
                        }
                        limit = Some(value);
                        index += 1;
                    }
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    other => bail!("unexpected argument for wallet history: {other}"),
                }
            }
            Ok(WalletSubcommand::History { limit, json })
        }
        "backup" => parse_wallet_backup_command(args, start_index + 2),
        "restore" => parse_wallet_restore_command(args, start_index + 2),
        "entropy" => parse_wallet_entropy_command(args, start_index + 2),
        "lock" => parse_wallet_lock_command(args, start_index + 2),
        other => bail!("unsupported wallet subcommand '{other}'"),
    }
}

fn parse_wallet_backup_command(args: &[String], start_index: usize) -> Result<WalletSubcommand> {
    let action = args
        .get(start_index)
        .ok_or_else(|| anyhow!("missing wallet backup action"))?;
    match action.as_str() {
        "export" => {
            let path = args
                .get(start_index + 1)
                .ok_or_else(|| anyhow!("missing <path> for wallet backup export"))?;
            let mut passphrase_env = None;
            let mut include_identity_mnemonic = false;
            let mut json = false;
            let mut index = start_index + 2;
            while index < args.len() {
                match args[index].as_str() {
                    "--passphrase-env" => {
                        index += 1;
                        let value = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --passphrase-env"))?;
                        if value.trim().is_empty() {
                            bail!("--passphrase-env cannot be empty");
                        }
                        passphrase_env = Some(value.trim().to_string());
                        index += 1;
                    }
                    "--include-identity-mnemonic" => {
                        include_identity_mnemonic = true;
                        index += 1;
                    }
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    other => bail!("unexpected argument for wallet backup export: {other}"),
                }
            }
            Ok(WalletSubcommand::BackupExport {
                path: PathBuf::from(path),
                passphrase_env,
                include_identity_mnemonic,
                json,
            })
        }
        "inspect" => {
            let path = args
                .get(start_index + 1)
                .ok_or_else(|| anyhow!("missing <path> for wallet backup inspect"))?;
            Ok(WalletSubcommand::BackupInspect {
                path: PathBuf::from(path),
                json: parse_json_only(args, start_index + 2, "wallet backup inspect")?,
            })
        }
        other => bail!("unsupported wallet backup action '{other}'"),
    }
}

fn parse_wallet_restore_command(args: &[String], start_index: usize) -> Result<WalletSubcommand> {
    let action = args
        .get(start_index)
        .ok_or_else(|| anyhow!("missing wallet restore action"))?;
    match action.as_str() {
        "phrase" => {
            let mut mnemonic_env = None;
            let mut mnemonic_file = None;
            let mut wallet_network = None;
            let mut yes = false;
            let mut json = false;
            let mut index = start_index + 1;
            while index < args.len() {
                match args[index].as_str() {
                    "--mnemonic-env" => {
                        index += 1;
                        let value = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --mnemonic-env"))?;
                        if value.trim().is_empty() {
                            bail!("--mnemonic-env cannot be empty");
                        }
                        mnemonic_env = Some(value.trim().to_string());
                        index += 1;
                    }
                    "--mnemonic-file" => {
                        index += 1;
                        let value = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --mnemonic-file"))?;
                        mnemonic_file = Some(PathBuf::from(value));
                        index += 1;
                    }
                    "--wallet-network" => {
                        index += 1;
                        let value = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --wallet-network"))?;
                        if value.trim().is_empty() {
                            bail!("--wallet-network cannot be empty");
                        }
                        wallet_network = Some(value.trim().to_string());
                        index += 1;
                    }
                    "--yes" => {
                        yes = true;
                        index += 1;
                    }
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    other => bail!("unexpected argument for wallet restore phrase: {other}"),
                }
            }
            if mnemonic_env.is_some() == mnemonic_file.is_some() {
                bail!(
                    "wallet restore phrase requires exactly one of --mnemonic-env or --mnemonic-file"
                );
            }
            Ok(WalletSubcommand::RestorePhrase {
                mnemonic_env,
                mnemonic_file,
                wallet_network,
                yes,
                json,
            })
        }
        "backup" => {
            let path = args
                .get(start_index + 1)
                .ok_or_else(|| anyhow!("missing <path> for wallet restore backup"))?;
            let mut passphrase_env = None;
            let mut yes = false;
            let mut json = false;
            let mut index = start_index + 2;
            while index < args.len() {
                match args[index].as_str() {
                    "--passphrase-env" => {
                        index += 1;
                        let value = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --passphrase-env"))?;
                        if value.trim().is_empty() {
                            bail!("--passphrase-env cannot be empty");
                        }
                        passphrase_env = Some(value.trim().to_string());
                        index += 1;
                    }
                    "--yes" => {
                        yes = true;
                        index += 1;
                    }
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    other => bail!("unexpected argument for wallet restore backup: {other}"),
                }
            }
            Ok(WalletSubcommand::RestoreBackup {
                path: PathBuf::from(path),
                passphrase_env,
                yes,
                json,
            })
        }
        other => bail!("unsupported wallet restore action '{other}'"),
    }
}

fn parse_wallet_entropy_command(args: &[String], start_index: usize) -> Result<WalletSubcommand> {
    let action = args
        .get(start_index)
        .ok_or_else(|| anyhow!("missing wallet entropy action"))?;
    match action.as_str() {
        "status" => Ok(WalletSubcommand::EntropyStatus {
            json: parse_json_only(args, start_index + 1, "wallet entropy status")?,
        }),
        "export" => {
            let path = args
                .get(start_index + 1)
                .ok_or_else(|| anyhow!("missing <path> for wallet entropy export"))?;
            Ok(WalletSubcommand::EntropyExport {
                path: PathBuf::from(path),
                json: parse_json_only(args, start_index + 2, "wallet entropy export")?,
            })
        }
        "import" => {
            let path = args
                .get(start_index + 1)
                .ok_or_else(|| anyhow!("missing <path> for wallet entropy import"))?;
            Ok(WalletSubcommand::EntropyImport {
                path: PathBuf::from(path),
                json: parse_json_only(args, start_index + 2, "wallet entropy import")?,
            })
        }
        other => bail!("unsupported wallet entropy action '{other}'"),
    }
}

fn parse_wallet_lock_command(args: &[String], start_index: usize) -> Result<WalletSubcommand> {
    let action = args
        .get(start_index)
        .ok_or_else(|| anyhow!("missing wallet lock action"))?;
    match action.as_str() {
        "status" => Ok(WalletSubcommand::LockStatus {
            json: parse_json_only(args, start_index + 1, "wallet lock status")?,
        }),
        "clear" => Ok(WalletSubcommand::LockClear {
            json: parse_json_only(args, start_index + 1, "wallet lock clear")?,
        }),
        other => bail!("unsupported wallet lock action '{other}'"),
    }
}

pub async fn load_wallet_status_report(config_path: &Path) -> Result<WalletStatusReport> {
    load_wallet_status_report_internal(config_path, true).await
}

pub async fn load_wallet_balance_status_report(config_path: &Path) -> Result<WalletStatusReport> {
    load_wallet_status_report_internal(config_path, false).await
}

async fn load_wallet_status_report_internal(
    config_path: &Path,
    include_recent_payments: bool,
) -> Result<WalletStatusReport> {
    let context = prepare_wallet_context(config_path)?;
    let ledger = load_ledger(config_path)?;
    context.runtime.start()?;
    let report = context.runtime.sync(&ledger, include_recent_payments)?;
    sync_wallet_status(
        config_path,
        &report.runtime,
        report.runtime_status.as_str(),
        report.runtime_detail.clone(),
        Some(&report.balance),
        None,
        report.recent_payments.as_slice(),
    )?;
    Ok(report)
}

pub async fn create_wallet_address_report(config_path: &Path) -> Result<WalletAddressReport> {
    let context = prepare_wallet_context(config_path)?;
    context.runtime.start()?;
    match context.runtime.address() {
        Ok(report) => {
            sync_wallet_status(
                config_path,
                &report.runtime,
                "address_ready",
                None,
                None,
                Some(report.bitcoin_address.as_str()),
                &[],
            )?;
            Ok(report)
        }
        Err(error) => {
            sync_wallet_error(config_path, context.runtime.surface(), error.to_string())?;
            Err(error)
        }
    }
}

pub async fn create_wallet_invoice_report(
    config_path: &Path,
    amount_sats: u64,
    description: Option<String>,
    expiry_seconds: Option<u32>,
) -> Result<WalletInvoiceReport> {
    let context = prepare_wallet_context(config_path)?;
    context.runtime.start()?;
    match context
        .runtime
        .invoice(amount_sats, description, expiry_seconds)
    {
        Ok(report) => {
            mutate_ledger(config_path, |ledger| {
                ledger.upsert_wallet_invoice(report.invoice.clone());
                Ok(())
            })?;
            Ok(report)
        }
        Err(error) => {
            sync_wallet_error(config_path, context.runtime.surface(), error.to_string())?;
            Err(error)
        }
    }
}

pub async fn create_wallet_offer_report(
    config_path: &Path,
    amount_sats: Option<u64>,
    description: Option<String>,
    expiry_seconds: Option<u32>,
) -> Result<WalletOfferReport> {
    let context = prepare_wallet_context(config_path)?;
    context.runtime.start()?;
    match context
        .runtime
        .offer(amount_sats, description, expiry_seconds)
    {
        Ok(report) => Ok(report),
        Err(error) => {
            sync_wallet_error(config_path, context.runtime.surface(), error.to_string())?;
            Err(error)
        }
    }
}

pub async fn pay_wallet_invoice_report(
    config_path: &Path,
    payment_request: &str,
    amount_sats: Option<u64>,
) -> Result<WalletPayReport> {
    let context = prepare_wallet_context(config_path)?;
    context.runtime.start()?;
    match context.runtime.pay(payment_request, amount_sats) {
        Ok(report) => {
            mutate_ledger(config_path, |ledger| {
                ledger.wallet.last_balance_sats = Some(report.post_balance.total_sats);
                ledger.wallet.last_balance_at_ms = Some(now_epoch_ms() as u64);
                ledger.upsert_wallet_payment(report.payment.clone());
                Ok(())
            })?;
            Ok(report)
        }
        Err(error) => {
            let error_string = error.to_string();
            let failed_payment =
                failed_wallet_payment_record(payment_request, amount_sats, error_string.as_str());
            mutate_ledger(config_path, |ledger| {
                ledger.upsert_wallet_payment(failed_payment);
                Ok(())
            })?;
            sync_wallet_error(config_path, context.runtime.surface(), error_string)?;
            Err(error)
        }
    }
}

pub async fn load_wallet_history_report(
    config_path: &Path,
    limit: Option<u32>,
) -> Result<WalletHistoryReport> {
    let context = prepare_wallet_context(config_path)?;
    context.runtime.start()?;
    let ledger = load_ledger(config_path)?;
    let records = context.runtime.list_payments(&ledger, limit)?;
    let channels = context.runtime.list_channels()?;
    let lightning_readiness = wallet_lightning_readiness(
        context.runtime.surface().runtime_kind,
        context.runtime.surface().liquidity_provider_kind,
        channels.as_slice(),
        true,
    );
    mutate_ledger(config_path, |ledger| {
        for record in &records {
            ledger.upsert_wallet_payment(record.clone());
        }
        Ok(())
    })?;
    let receipts = load_ledger(config_path)?.wallet.receipts;
    Ok(WalletHistoryReport {
        runtime: context.runtime.surface().clone(),
        channels,
        lightning_readiness,
        payments: records,
        receipts,
    })
}

pub async fn load_wallet_channels_report(config_path: &Path) -> Result<WalletChannelsReport> {
    let context = prepare_wallet_context(config_path)?;
    context.runtime.start()?;
    let channels = context.runtime.list_channels()?;
    Ok(wallet_channels_report_from_channels(
        context.runtime.surface(),
        channels,
    ))
}

pub async fn load_wallet_credit_summary_report(
    config_path: &Path,
) -> Result<WalletCreditSummaryReport> {
    let context = prepare_wallet_context(config_path)?;
    let records = load_ledger(config_path)?.wallet.payments;
    let credits = compute_wallet_credit_summary(records.as_slice(), now_epoch_ms() as u64);
    sync_wallet_credit_summary(config_path, &credits)?;
    Ok(WalletCreditSummaryReport {
        runtime: context.runtime.surface().clone(),
        credits,
    })
}

pub async fn load_wallet_telemetry_report(config_path: &Path) -> Result<WalletTelemetryReport> {
    let context = prepare_wallet_context(config_path)?;
    let ledger = load_ledger(config_path)?;
    context.runtime.start()?;
    let status = context.runtime.sync(&ledger, false)?;
    let mut channel_error = None;
    let channels = match context.runtime.list_channels() {
        Ok(channels) => channels,
        Err(error) => {
            channel_error = Some(redact_wallet_secret_text(error.to_string().as_str()));
            Vec::new()
        }
    };
    sync_wallet_status(
        config_path,
        &status.runtime,
        status.runtime_status.as_str(),
        status.runtime_detail.clone(),
        Some(&status.balance),
        None,
        status.recent_payments.as_slice(),
    )?;
    Ok(wallet_telemetry_from_status(
        &status,
        channels.as_slice(),
        channel_error.as_deref(),
    ))
}

pub async fn load_wallet_entropy_status_report(config_path: &Path) -> Result<WalletEntropyReport> {
    let context = prepare_wallet_context(config_path)?;
    Ok(WalletEntropyReport {
        runtime: context.runtime.surface().clone(),
        operation: "status".to_string(),
        metadata: context.runtime.surface().node_entropy.clone(),
        path: None,
    })
}

pub async fn export_wallet_entropy_report(
    config_path: &Path,
    export_path: &Path,
) -> Result<WalletEntropyReport> {
    let config = ensure_local_setup(config_path)?;
    let material = load_wallet_node_entropy_material(&config)?;
    write_explicit_wallet_entropy(export_path, &material.bytes)?;
    let context = prepare_wallet_context(config_path)?;
    Ok(WalletEntropyReport {
        runtime: context.runtime.surface().clone(),
        operation: "export".to_string(),
        metadata: material.metadata,
        path: Some(export_path.display().to_string()),
    })
}

pub async fn import_wallet_entropy_report(
    config_path: &Path,
    import_path: &Path,
) -> Result<WalletEntropyReport> {
    let mut config = ensure_local_setup(config_path)?;
    let _ = read_explicit_wallet_entropy(import_path)?;
    config.wallet_entropy_override_path = Some(import_path.to_path_buf());
    crate::save_config(config_path, &config)?;
    let context = prepare_wallet_context(config_path)?;
    Ok(WalletEntropyReport {
        runtime: context.runtime.surface().clone(),
        operation: "import".to_string(),
        metadata: context.runtime.surface().node_entropy.clone(),
        path: Some(import_path.display().to_string()),
    })
}

pub async fn export_wallet_backup_report(
    config_path: &Path,
    export_path: &Path,
    passphrase_env: Option<&str>,
    include_identity_mnemonic: bool,
) -> Result<WalletBackupExportReport> {
    let config = ensure_local_setup(config_path)?;
    if config.wallet_runtime_kind != PylonWalletRuntimeKind::LdkNode {
        bail!("wallet backup export requires wallet_runtime_kind=ldk_node");
    }
    ensure_private_file_permissions(config.identity_path.as_path(), "identity mnemonic")?;
    if let Some(path) = config.wallet_entropy_override_path.as_ref() {
        ensure_private_file_permissions(path.as_path(), "wallet entropy override")?;
    }
    let context = prepare_wallet_context(config_path)?;
    let layout = ensure_wallet_storage_layout(&config)?;
    let _lock = acquire_wallet_storage_lock(&layout)?;
    let passphrase = wallet_backup_passphrase(passphrase_env)?;
    let exported_at_ms = now_epoch_ms() as u64;
    let storage_generation = wallet_storage_generation_id(&layout, context.runtime.surface());
    let plaintext = build_wallet_backup_plaintext_manifest(
        &config,
        &layout,
        context.runtime.surface(),
        storage_generation.as_str(),
        exported_at_ms,
        include_identity_mnemonic,
        export_path,
    )?;
    let encrypted = encrypt_wallet_backup_plaintext(&plaintext, &passphrase)?;
    write_encrypted_wallet_backup(export_path, &encrypted)?;
    let file_digest = sha256_file_digest(export_path)?;
    let report = WalletBackupExportReport {
        runtime: context.runtime.surface().clone(),
        operation: "export".to_string(),
        path: export_path.display().to_string(),
        file_digest,
        exported_at_ms,
        backup_status: "backup_current".to_string(),
        manifest: encrypted.public_manifest,
    };
    write_wallet_backup_status_manifest(&layout, &report)?;
    mutate_ledger(config_path, |ledger| {
        ledger.upsert_wallet_receipt(PylonWalletReceiptRecord {
            receipt_id: format!(
                "wallet:backup:{}",
                report.manifest.plaintext_manifest_digest
            ),
            receipt_type: "wallet.backup.export.v1".to_string(),
            status: report.backup_status.clone(),
            direction: None,
            method: Some("encrypted_backup".to_string()),
            amount_sats: None,
            fees_sats: None,
            payment_id: None,
            payment_hash: None,
            txid: None,
            operation_id: Some(report.file_digest.clone()),
            settlement_id: None,
            failure_code: None,
            detail: Some(format!(
                "encrypted wallet backup exported with {} components",
                report.manifest.plaintext_component_count
            )),
            created_at_ms: report.exported_at_ms,
            updated_at_ms: report.exported_at_ms,
        });
        Ok(())
    })?;
    Ok(report)
}

pub fn inspect_wallet_backup_report(path: &Path) -> Result<WalletBackupInspectReport> {
    let encrypted = read_encrypted_wallet_backup(path)?;
    let file_digest = sha256_file_digest(path)?;
    Ok(WalletBackupInspectReport {
        path: path.display().to_string(),
        file_digest,
        valid: encrypted.schema_version == WALLET_BACKUP_SCHEMA_VERSION
            && encrypted.kind == WALLET_BACKUP_KIND
            && encrypted.public_manifest.schema_version == WALLET_BACKUP_SCHEMA_VERSION
            && encrypted.public_manifest.kind == WALLET_BACKUP_KIND,
        manifest: encrypted.public_manifest,
    })
}

pub async fn restore_wallet_phrase_report(
    config_path: &Path,
    mnemonic_env: Option<&str>,
    mnemonic_file: Option<&Path>,
    wallet_network: Option<&str>,
) -> Result<WalletRestoreReport> {
    let mut config = ensure_local_setup(config_path)?;
    let mnemonic = read_wallet_restore_mnemonic(mnemonic_env, mnemonic_file)?;
    let parsed = Mnemonic::parse_in_normalized(Language::English, mnemonic.as_str())
        .context("wallet restore phrase is not a valid BIP39 English mnemonic")?;
    let normalized_mnemonic = parsed.to_string();
    if let Some(network) = wallet_network {
        parse_ldk_bitcoin_network(network)?;
        config.wallet_network = network.to_string();
    }
    config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
    config.wallet_entropy_override_path = None;
    let layout = ensure_wallet_storage_layout(&config)?;
    refuse_active_wallet_restore(&layout)?;
    reset_wallet_storage_for_restore(&layout)?;
    write_wallet_identity_mnemonic(config.identity_path.as_path(), normalized_mnemonic.as_str())?;
    crate::save_config(config_path, &config)?;
    let context = prepare_wallet_context(config_path)?;
    let report = WalletRestoreReport {
        runtime: context.runtime.surface().clone(),
        operation: "restore".to_string(),
        mode: "phrase_only".to_string(),
        status: "restored".to_string(),
        network: config.wallet_network.clone(),
        restored_component_count: 0,
        backup_file_digest: None,
        plaintext_manifest_digest: None,
        identity_mnemonic_restored: true,
        recovery_mode: "phrase_only_onchain_rescan_required".to_string(),
        limitations: vec![
            "Restores OpenAgents identity and deterministic LDK node entropy.".to_string(),
            "Does not restore Lightning channel monitors, channel manager, or payment continuity; use wallet restore backup for full Lightning state.".to_string(),
        ],
    };
    sync_wallet_restore_receipt(config_path, &report)?;
    Ok(report)
}

pub async fn restore_wallet_backup_report(
    config_path: &Path,
    backup_path: &Path,
    passphrase_env: Option<&str>,
) -> Result<WalletRestoreReport> {
    let mut config = ensure_local_setup(config_path)?;
    let encrypted = read_encrypted_wallet_backup(backup_path)?;
    let backup_file_digest = sha256_file_digest(backup_path)?;
    let passphrase = wallet_backup_passphrase(passphrase_env)?;
    let plaintext = decrypt_wallet_backup_plaintext(&encrypted, passphrase.as_str())?;
    validate_wallet_backup_plaintext_for_restore(&encrypted, &plaintext)?;
    if config.wallet_network != plaintext.network {
        bail!(
            "wallet backup network mismatch: config wallet_network={} backup network={}; set wallet_network before restore or use phrase restore with --wallet-network",
            config.wallet_network,
            plaintext.network
        );
    }
    config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
    config.wallet_entropy_override_path = None;
    let layout = ensure_wallet_storage_layout(&config)?;
    refuse_active_wallet_restore(&layout)?;
    let identity_mnemonic_restored = if let Some(mnemonic) = plaintext.identity_mnemonic.as_deref()
    {
        write_wallet_identity_mnemonic(config.identity_path.as_path(), mnemonic)?;
        true
    } else {
        false
    };
    let material = load_wallet_node_entropy_material(&config)?;
    if material.metadata.derivation_version != plaintext.wallet_derivation.derivation_version {
        bail!(
            "wallet backup derivation version mismatch: local={} backup={}",
            material.metadata.derivation_version,
            plaintext.wallet_derivation.derivation_version
        );
    }
    if material.metadata.digest != plaintext.wallet_derivation.digest {
        bail!(
            "wallet backup entropy digest mismatch; restore the matching Pylon recovery phrase first or use an all-in-one encrypted backup"
        );
    }
    reset_wallet_storage_for_restore(&layout)?;
    restore_wallet_backup_snapshot(&layout, plaintext.ldk_storage_snapshot.as_slice())?;
    write_restored_wallet_backup_status_manifest(
        &layout,
        &encrypted,
        &plaintext,
        backup_path,
        backup_file_digest.as_str(),
    )?;
    crate::save_config(config_path, &config)?;
    let context = prepare_wallet_context(config_path)?;
    let report = WalletRestoreReport {
        runtime: context.runtime.surface().clone(),
        operation: "restore".to_string(),
        mode: "full_backup".to_string(),
        status: "restored".to_string(),
        network: plaintext.network,
        restored_component_count: plaintext.ldk_storage_snapshot.len(),
        backup_file_digest: Some(backup_file_digest),
        plaintext_manifest_digest: Some(encrypted.public_manifest.plaintext_manifest_digest),
        identity_mnemonic_restored,
        recovery_mode: "full_backup_ldk_state_restored".to_string(),
        limitations: vec![
            "Restores the encrypted local LDK state snapshot; chain sync may still need to rescan or update after first start.".to_string(),
        ],
    };
    sync_wallet_restore_receipt(config_path, &report)?;
    Ok(report)
}

pub async fn inspect_wallet_lock_report(config_path: &Path) -> Result<WalletLockReport> {
    let context = prepare_wallet_context(config_path)?;
    let layout = ensure_wallet_storage_layout_for_config_path(config_path)?;
    Ok(wallet_lock_report(
        context.runtime.surface().clone(),
        &layout,
        false,
    ))
}

pub async fn clear_wallet_lock_report(config_path: &Path) -> Result<WalletLockReport> {
    let context = prepare_wallet_context(config_path)?;
    let layout = ensure_wallet_storage_layout_for_config_path(config_path)?;
    let before = wallet_lock_report(context.runtime.surface().clone(), &layout, false);
    if before.locked && !before.stale {
        bail!(
            "wallet lock is active for pid {}; stop the other Pylon process before clearing {}",
            before
                .owner
                .as_ref()
                .map(|owner| owner.pid)
                .unwrap_or_default(),
            layout.lock_path.display()
        );
    }
    if before.locked {
        std::fs::remove_file(layout.lock_path.as_path()).with_context(|| {
            format!("failed to clear wallet lock {}", layout.lock_path.display())
        })?;
    }
    Ok(wallet_lock_report(
        context.runtime.surface().clone(),
        &layout,
        before.locked,
    ))
}

pub fn render_wallet_status_report(report: &WalletStatusReport) -> String {
    let mut lines = vec![
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("runtime_status: {}", report.runtime_status),
        format!("network: {}", report.runtime.network),
        format!("api_key_source: {}", report.runtime.api_key_source),
        format!("identity_path: {}", report.runtime.identity_path),
        format!("storage_dir: {}", report.runtime.storage_dir),
        format!("entropy_source: {}", report.runtime.node_entropy.source),
        format!(
            "entropy_derivation_version: {}",
            report.runtime.node_entropy.derivation_version
        ),
        format!(
            "entropy_domain_label: {}",
            report.runtime.node_entropy.domain_label
        ),
        format!(
            "node_entropy_digest: {}",
            report.runtime.node_entropy.digest
        ),
        format!("credited_sats: {}", report.balance.credited_sats),
        format!("lightning_sats: {}", report.balance.lightning_sats),
        format!("onchain_sats: {}", report.balance.onchain_sats),
        format!(
            "spendable_onchain_sats: {}",
            report.balance.spendable_onchain_sats
        ),
        format!(
            "anchor_reserve_sats: {}",
            report.balance.anchor_reserve_sats
        ),
        format!("total_sats: {}", report.balance.total_sats),
        format!(
            "lightning_receive_state: {}",
            report.lightning_readiness.state
        ),
        format!(
            "can_receive_lightning: {}",
            report.lightning_readiness.can_receive_lightning
        ),
        format!(
            "can_receive_onchain: {}",
            report.lightning_readiness.can_receive_onchain
        ),
        format!(
            "can_send_lightning: {}",
            report.lightning_readiness.can_send_lightning
        ),
        format!("channels_total: {}", report.channels.len()),
        format!(
            "usable_channels: {}",
            report.lightning_readiness.usable_channel_count
        ),
        format!(
            "inbound_liquidity_sats: {}",
            report.lightning_readiness.inbound_liquidity_sats
        ),
        format!(
            "outbound_liquidity_sats: {}",
            report.lightning_readiness.outbound_liquidity_sats
        ),
    ];
    if let Some(warning) = report.lightning_readiness.warning.as_deref() {
        lines.push(format!("lightning_receive_warning: {warning}"));
    }
    if let Some(detail) = report.runtime_detail.as_deref() {
        lines.push(format!("runtime_detail: {detail}"));
    }
    if let Some(ldk_node) = report.ldk_node.as_ref() {
        lines.push(format!(
            "ldk_node_id: {}",
            ldk_node.node_id.as_deref().unwrap_or("none")
        ));
        lines.push(format!("ldk_storage_dir: {}", ldk_node.storage_dir));
        lines.push(format!(
            "ldk_storage_schema_version: {}",
            ldk_node.storage_schema_version
        ));
        lines.push(format!(
            "ldk_storage_generation: {}",
            ldk_node.storage_generation
        ));
        lines.push(format!("ldk_chain_source: {}", ldk_node.chain_source_kind));
        if let Some(url) = ldk_node.chain_source_url.as_deref() {
            lines.push(format!("ldk_chain_source_url: {url}"));
        }
        if let Some(url) = ldk_node.rgs_url.as_deref() {
            lines.push(format!("ldk_rgs_url: {url}"));
        }
        lines.push(format!("ldk_backup_status: {}", ldk_node.backup_status));
        lines.push(format!(
            "ldk_backup_manifest_present: {}",
            ldk_node.backup_manifest_present
        ));
        lines.push(format!(
            "ldk_backup_artifact_count: {}",
            ldk_node.backup_artifact_count
        ));
        lines.push(format!("ldk_backup_stale: {}", ldk_node.backup_stale));
        lines.push(format!(
            "ldk_backup_stale_after_ms: {}",
            ldk_node.backup_stale_after_ms
        ));
        if let Some(value) = ldk_node.last_backup_exported_at_ms {
            lines.push(format!("ldk_last_backup_exported_at_ms: {value}"));
        }
        if let Some(value) = ldk_node.last_backup_file_digest.as_deref() {
            lines.push(format!("ldk_last_backup_file_digest: {value}"));
        }
        lines.push(format!("ldk_is_running: {}", ldk_node.is_running));
        if let Some(value) = ldk_node.latest_lightning_wallet_sync_timestamp {
            lines.push(format!(
                "ldk_latest_lightning_wallet_sync_timestamp: {value}"
            ));
        }
        if let Some(value) = ldk_node.latest_onchain_wallet_sync_timestamp {
            lines.push(format!("ldk_latest_onchain_wallet_sync_timestamp: {value}"));
        }
        if let Some(value) = ldk_node.latest_rgs_snapshot_timestamp {
            lines.push(format!("ldk_latest_rgs_snapshot_timestamp: {value}"));
        }
        if let Some(error) = ldk_node.last_error.as_deref() {
            lines.push(format!("ldk_last_error: {error}"));
        }
    }
    for channel in &report.channels {
        lines.push(String::new());
        lines.push(format!("channel_id: {}", channel.channel_id));
        lines.push(format!("channel_status: {}", channel.status));
        lines.push(format!(
            "channel_peer_connected: {}",
            channel.peer_connected
        ));
        lines.push(format!("channel_inbound_sats: {}", channel.inbound_sats));
        lines.push(format!("channel_outbound_sats: {}", channel.outbound_sats));
        if let Some(peer) = channel.counterparty_node_id.as_deref() {
            lines.push(format!("channel_counterparty_node_id: {peer}"));
        }
    }
    if report.recent_payments.is_empty() {
        lines.push(String::new());
        lines.push("recent_payments: none".to_string());
        return lines.join("\n");
    }
    for payment in &report.recent_payments {
        lines.push(String::new());
        lines.push(format!("payment_id: {}", payment.payment_id));
        lines.push(format!("direction: {}", payment.direction));
        lines.push(format!("status: {}", payment.status));
        lines.push(format!("amount_sats: {}", payment.amount_sats));
        lines.push(format!("fees_sats: {}", payment.fees_sats));
        lines.push(format!("method: {}", payment.method));
        if let Some(description) = payment.description.as_deref() {
            lines.push(format!("description: {description}"));
        }
        if let Some(invoice) = payment.invoice.as_deref() {
            lines.push(format!("invoice: {invoice}"));
        }
    }
    lines.join("\n")
}

pub fn render_wallet_balance_report(report: &WalletStatusReport) -> String {
    let mut lines = vec![
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("network: {}", report.runtime.network),
        format!("runtime_status: {}", report.runtime_status),
        format!("credited_sats: {}", report.balance.credited_sats),
        format!("lightning_sats: {}", report.balance.lightning_sats),
        format!("onchain_sats: {}", report.balance.onchain_sats),
        format!(
            "spendable_onchain_sats: {}",
            report.balance.spendable_onchain_sats
        ),
        format!(
            "anchor_reserve_sats: {}",
            report.balance.anchor_reserve_sats
        ),
        format!("total_sats: {}", report.balance.total_sats),
    ];
    if let Some(detail) = report.runtime_detail.as_deref() {
        lines.push(format!("runtime_detail: {detail}"));
    }
    lines.join("\n")
}

pub fn render_wallet_channels_report(report: &WalletChannelsReport) -> String {
    let mut lines = vec![
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("network: {}", report.runtime.network),
        format!(
            "lightning_receive_state: {}",
            report.lightning_readiness.state
        ),
        format!(
            "can_receive_lightning: {}",
            report.lightning_readiness.can_receive_lightning
        ),
        format!(
            "can_receive_onchain: {}",
            report.lightning_readiness.can_receive_onchain
        ),
        format!(
            "can_send_lightning: {}",
            report.lightning_readiness.can_send_lightning
        ),
        format!("channels_total: {}", report.channel_summary.total_count),
        format!("channels_usable: {}", report.channel_summary.usable_count),
        format!("channels_pending: {}", report.channel_summary.pending_count),
        format!("inbound_liquidity_sats: {}", report.liquidity.inbound_sats),
        format!(
            "outbound_liquidity_sats: {}",
            report.liquidity.outbound_sats
        ),
        format!("lsp_state: {}", report.lightning_readiness.lsp.state),
    ];
    if let Some(warning) = report.lightning_readiness.warning.as_deref() {
        lines.push(format!("warning: {warning}"));
    }
    for remediation in &report.lightning_readiness.remediation {
        lines.push(format!("remediation: {remediation}"));
    }
    if report.channels.is_empty() {
        lines.push(String::new());
        lines.push("channels: none".to_string());
        return lines.join("\n");
    }
    for channel in &report.channels {
        lines.push(String::new());
        lines.push(format!("channel_id: {}", channel.channel_id));
        lines.push(format!("status: {}", channel.status));
        lines.push(format!("peer_connected: {}", channel.peer_connected));
        lines.push(format!("inbound_sats: {}", channel.inbound_sats));
        lines.push(format!("outbound_sats: {}", channel.outbound_sats));
        lines.push(format!(
            "channel_value_sats: {}",
            channel.channel_value_sats
        ));
        if let Some(peer) = channel.counterparty_node_id.as_deref() {
            lines.push(format!("counterparty_node_id: {peer}"));
        }
        if let Some(confirmations) = channel.confirmations {
            lines.push(format!("confirmations: {confirmations}"));
        }
        if let Some(required) = channel.confirmations_required {
            lines.push(format!("confirmations_required: {required}"));
        }
    }
    lines.join("\n")
}

pub fn render_wallet_address_report(report: &WalletAddressReport) -> String {
    let external_payout_target = report
        .payout_destination
        .as_deref()
        .unwrap_or("not_configured");
    [
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("network: {}", report.runtime.network),
        format!("external_payout_target: {external_payout_target}"),
        format!("bitcoin_address: {}", report.bitcoin_address),
    ]
    .join("\n")
}

pub fn render_wallet_invoice_report(report: &WalletInvoiceReport) -> String {
    let mut lines = vec![
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("network: {}", report.runtime.network),
        format!("invoice_id: {}", report.invoice.invoice_id),
        format!("amount_sats: {}", report.invoice.amount_sats),
        format!("status: {}", report.invoice.status),
        format!("payment_request: {}", report.invoice.payment_request),
    ];
    if let Some(payment_hash) = report.invoice.payment_hash.as_deref() {
        lines.push(format!("payment_hash: {payment_hash}"));
    }
    if let Some(description) = report.invoice.description.as_deref() {
        lines.push(format!("description: {description}"));
    }
    if let Some(expires_at_ms) = report.invoice.expires_at_ms {
        lines.push(format!("expires_at_ms: {expires_at_ms}"));
    }
    lines.join("\n")
}

pub fn render_wallet_offer_report(report: &WalletOfferReport) -> String {
    let mut lines = vec![
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("network: {}", report.runtime.network),
        "offer:".to_string(),
        report.offer.clone(),
    ];
    if let Some(amount_sats) = report.amount_sats {
        lines.push(format!("amount_sats: {amount_sats}"));
    }
    if let Some(description) = report.description.as_deref() {
        lines.push(format!("description: {description}"));
    }
    if let Some(expires_at_ms) = report.expires_at_ms {
        lines.push(format!("expires_at_ms: {expires_at_ms}"));
    }
    lines.join("\n")
}

pub fn render_wallet_pay_report(report: &WalletPayReport) -> String {
    let mut lines = vec![
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("network: {}", report.runtime.network),
        format!("payment_id: {}", report.payment_id),
        format!("status: {}", report.payment.status),
        format!("amount_sats: {}", report.payment.amount_sats),
        format!("fees_sats: {}", report.payment.fees_sats),
        format!("total_sats: {}", report.post_balance.total_sats),
    ];
    if let Some(description) = report.payment.description.as_deref() {
        lines.push(format!("description: {description}"));
    }
    if let Some(invoice) = report.payment.invoice.as_deref() {
        lines.push(format!("invoice: {invoice}"));
    }
    lines.join("\n")
}

pub fn render_wallet_telemetry_report(report: &WalletTelemetryReport) -> String {
    let mut lines = vec![
        format!("schema: {}", report.schema),
        format!("generated_at_ms: {}", report.generated_at_ms),
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("runtime_status: {}", report.runtime_status),
        format!("health_state: {}", report.health.state),
        format!("payable: {}", report.health.payable),
        format!("receive_ready: {}", report.health.receive_ready),
        format!("send_ready: {}", report.health.send_ready),
        format!("backup_ready: {}", report.health.backup_ready),
        format!("network: {}", report.sources.network),
        format!("chain_source_kind: {}", report.sources.chain_source_kind),
        format!("gossip_source_kind: {}", report.sources.gossip_source_kind),
        format!("sync_state: {}", report.sync.state),
        format!("is_running: {}", report.sync.is_running),
        format!("total_sats: {}", report.balances.total_sats),
        format!("lightning_sats: {}", report.balances.lightning_sats),
        format!("onchain_sats: {}", report.balances.onchain_sats),
        format!("channels_total: {}", report.channels.total_count),
        format!("channels_usable: {}", report.channels.usable_count),
        format!("inbound_liquidity_sats: {}", report.liquidity.inbound_sats),
        format!(
            "outbound_liquidity_sats: {}",
            report.liquidity.outbound_sats
        ),
        format!(
            "inbound_liquidity_bucket: {}",
            report.liquidity.inbound_bucket
        ),
        format!(
            "outbound_liquidity_bucket: {}",
            report.liquidity.outbound_bucket
        ),
        format!("backup_status: {}", report.backup.status),
        format!("backup_stale: {}", report.backup.stale),
        format!("lsp_state: {}", report.lsp.state),
        format!("lsp_configured: {}", report.lsp.configured),
        format!("warnings: {}", report.warnings.len()),
        format!("errors: {}", report.errors.len()),
        format!("redaction_policy: {}", report.redaction.policy),
    ];
    if let Some(detail) = report.runtime_detail.as_deref() {
        lines.push(format!("runtime_detail: {detail}"));
    }
    if let Some(node_id) = report.node_id.as_deref() {
        lines.push(format!("ldk_node_id: {node_id}"));
    }
    if let Some(url) = report.sources.chain_source_url.as_deref() {
        lines.push(format!("chain_source_url: {url}"));
    }
    if let Some(url) = report.sources.gossip_source_url.as_deref() {
        lines.push(format!("gossip_source_url: {url}"));
    }
    for warning in &report.warnings {
        lines.push(format!("warning_code: {}", warning.code));
        lines.push(format!("warning_detail: {}", warning.detail));
    }
    for error in &report.errors {
        lines.push(format!("error_code: {}", error.code));
        lines.push(format!("error_detail: {}", error.detail));
    }
    lines.join("\n")
}

pub fn render_wallet_history_report(report: &WalletHistoryReport) -> String {
    let mut lines = vec![
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("network: {}", report.runtime.network),
        format!(
            "lightning_receive_state: {}",
            report.lightning_readiness.state
        ),
        format!("channels: {}", report.channels.len()),
        format!("payments: {}", report.payments.len()),
        format!("receipts: {}", report.receipts.len()),
    ];
    if report.payments.is_empty() {
        lines.push(String::new());
        lines.push("history: none".to_string());
        return lines.join("\n");
    }
    for payment in &report.payments {
        lines.push(String::new());
        lines.push(format!("payment_id: {}", payment.payment_id));
        lines.push(format!("direction: {}", payment.direction));
        lines.push(format!("status: {}", payment.status));
        lines.push(format!("amount_sats: {}", payment.amount_sats));
        lines.push(format!("fees_sats: {}", payment.fees_sats));
        lines.push(format!("method: {}", payment.method));
        if let Some(receipt_id) = payment.receipt_id.as_deref() {
            lines.push(format!("receipt_id: {receipt_id}"));
        }
        if let Some(operation_id) = payment.operation_id.as_deref() {
            lines.push(format!("operation_id: {operation_id}"));
        }
        if let Some(failure_code) = payment.failure_code.as_deref() {
            lines.push(format!("failure_code: {failure_code}"));
        }
        if let Some(description) = payment.description.as_deref() {
            lines.push(format!("description: {description}"));
        }
        if let Some(invoice) = payment.invoice.as_deref() {
            lines.push(format!("invoice: {invoice}"));
        }
    }
    lines.join("\n")
}

pub fn render_wallet_entropy_report(report: &WalletEntropyReport) -> String {
    let mut lines = vec![
        format!("operation: {}", report.operation),
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("network: {}", report.runtime.network),
        format!("entropy_source: {}", report.metadata.source),
        format!(
            "entropy_derivation_version: {}",
            report.metadata.derivation_version
        ),
        format!("entropy_domain_label: {}", report.metadata.domain_label),
        format!("node_entropy_digest: {}", report.metadata.digest),
    ];
    if let Some(path) = report.path.as_deref() {
        lines.push(format!("path: {path}"));
    }
    if let Some(path) = report.metadata.override_path.as_deref() {
        lines.push(format!("override_path: {path}"));
    }
    lines.join("\n")
}

pub fn render_wallet_backup_export_report(report: &WalletBackupExportReport) -> String {
    [
        format!("operation: {}", report.operation),
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("network: {}", report.runtime.network),
        format!("path: {}", report.path),
        format!("file_digest: {}", report.file_digest),
        format!("backup_status: {}", report.backup_status),
        format!(
            "plaintext_manifest_digest: {}",
            report.manifest.plaintext_manifest_digest
        ),
        format!("components: {}", report.manifest.plaintext_component_count),
        format!("total_bytes: {}", report.manifest.plaintext_total_bytes),
        format!(
            "identity_mnemonic_included: {}",
            report.manifest.identity_mnemonic_included
        ),
    ]
    .join("\n")
}

pub fn render_wallet_backup_inspect_report(report: &WalletBackupInspectReport) -> String {
    [
        format!("path: {}", report.path),
        format!("file_digest: {}", report.file_digest),
        format!("valid: {}", report.valid),
        format!("kind: {}", report.manifest.kind),
        format!("runtime_kind: {}", report.manifest.runtime_kind),
        format!("network: {}", report.manifest.network),
        format!(
            "plaintext_manifest_digest: {}",
            report.manifest.plaintext_manifest_digest
        ),
        format!("components: {}", report.manifest.plaintext_component_count),
        format!("total_bytes: {}", report.manifest.plaintext_total_bytes),
        format!(
            "identity_mnemonic_included: {}",
            report.manifest.identity_mnemonic_included
        ),
    ]
    .join("\n")
}

pub fn render_wallet_restore_report(report: &WalletRestoreReport) -> String {
    let mut lines = vec![
        format!("operation: {}", report.operation),
        format!("mode: {}", report.mode),
        format!("status: {}", report.status),
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("network: {}", report.network),
        format!(
            "restored_component_count: {}",
            report.restored_component_count
        ),
        format!("recovery_mode: {}", report.recovery_mode),
        format!(
            "identity_mnemonic_restored: {}",
            report.identity_mnemonic_restored
        ),
    ];
    if let Some(value) = report.backup_file_digest.as_deref() {
        lines.push(format!("backup_file_digest: {value}"));
    }
    if let Some(value) = report.plaintext_manifest_digest.as_deref() {
        lines.push(format!("plaintext_manifest_digest: {value}"));
    }
    for limitation in &report.limitations {
        lines.push(format!("limitation: {limitation}"));
    }
    lines.join("\n")
}

pub fn render_wallet_lock_report(report: &WalletLockReport) -> String {
    let mut lines = vec![
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("lock_path: {}", report.lock_path),
        format!("locked: {}", report.locked),
        format!("stale: {}", report.stale),
        format!("cleared: {}", report.cleared),
        format!("detail: {}", report.detail),
    ];
    if let Some(owner) = report.owner.as_ref() {
        lines.push(format!("owner_pid: {}", owner.pid));
        lines.push(format!("owner_machine_id: {}", owner.machine_id));
        lines.push(format!("owner_created_at_ms: {}", owner.created_at_ms));
    }
    lines.join("\n")
}

fn parse_json_only(args: &[String], start_index: usize, label: &str) -> Result<bool> {
    let mut json = false;
    let mut index = start_index;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            other => bail!("unexpected argument for {label}: {other}"),
        }
    }
    Ok(json)
}

fn load_wallet_node_entropy_material(
    config: &crate::PylonConfig,
) -> Result<WalletNodeEntropyMaterial> {
    if let Some(path) = config.wallet_entropy_override_path.as_ref() {
        let bytes = read_explicit_wallet_entropy(path.as_path())?;
        return Ok(WalletNodeEntropyMaterial {
            metadata: WalletNodeEntropyMetadata {
                source: "explicit_entropy_file".to_string(),
                derivation_version: "explicit-node-entropy-v1".to_string(),
                domain_label: wallet_node_entropy_domain_label(config.wallet_network.as_str()),
                digest: node_entropy_digest(&bytes),
                override_path: Some(path.display().to_string()),
            },
            bytes,
        });
    }

    let mnemonic = std::fs::read_to_string(config.identity_path.as_path())
        .with_context(|| {
            format!(
                "failed to read identity mnemonic {}",
                config.identity_path.display()
            )
        })?
        .trim()
        .to_string();
    if mnemonic.is_empty() {
        bail!(
            "identity mnemonic is empty at {}",
            config.identity_path.display()
        );
    }
    let parsed = Mnemonic::parse_in_normalized(Language::English, mnemonic.as_str())
        .context("failed to parse identity mnemonic for wallet entropy derivation")?;
    let seed = parsed.to_seed("");
    let domain_label = wallet_node_entropy_domain_label(config.wallet_network.as_str());
    let bytes = hkdf_sha256_64(seed.as_slice(), domain_label.as_bytes());
    Ok(WalletNodeEntropyMaterial {
        metadata: WalletNodeEntropyMetadata {
            source: "identity_mnemonic_hkdf".to_string(),
            derivation_version: WALLET_NODE_ENTROPY_DERIVATION_VERSION.to_string(),
            domain_label,
            digest: node_entropy_digest(&bytes),
            override_path: None,
        },
        bytes,
    })
}

fn wallet_node_entropy_domain_label(network: &str) -> String {
    format!(
        "{WALLET_NODE_ENTROPY_LABEL_PREFIX}/{}",
        network.trim().to_ascii_lowercase()
    )
}

fn hkdf_sha256_64(input_key_material: &[u8], info: &[u8]) -> [u8; 64] {
    let mut extract = <HmacSha256 as Mac>::new_from_slice(WALLET_NODE_ENTROPY_HKDF_SALT)
        .expect("HKDF salt is valid");
    extract.update(input_key_material);
    let pseudorandom_key = extract.finalize().into_bytes();

    let mut output = [0u8; 64];
    let mut previous = Vec::<u8>::new();
    let mut written = 0usize;
    for counter in 1u8..=2 {
        let mut expand = <HmacSha256 as Mac>::new_from_slice(pseudorandom_key.as_slice())
            .expect("HKDF pseudorandom key is valid");
        expand.update(previous.as_slice());
        expand.update(info);
        expand.update(&[counter]);
        previous = expand.finalize().into_bytes().to_vec();
        let remaining = output.len() - written;
        let copy_len = remaining.min(previous.len());
        output[written..written + copy_len].copy_from_slice(&previous[..copy_len]);
        written += copy_len;
        if written == output.len() {
            break;
        }
    }
    output
}

fn node_entropy_digest(bytes: &[u8; 64]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn read_explicit_wallet_entropy(path: &Path) -> Result<[u8; 64]> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read wallet entropy file {}", path.display()))?;
    let normalized = raw
        .trim()
        .strip_prefix("hex:")
        .unwrap_or_else(|| raw.trim())
        .trim();
    let decoded = hex::decode(normalized)
        .with_context(|| format!("wallet entropy file {} is not hex", path.display()))?;
    if decoded.len() != 64 {
        bail!(
            "wallet entropy file {} must contain exactly 64 bytes encoded as hex",
            path.display()
        );
    }
    let mut bytes = [0u8; 64];
    bytes.copy_from_slice(decoded.as_slice());
    Ok(bytes)
}

fn write_explicit_wallet_entropy(path: &Path, bytes: &[u8; 64]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create wallet entropy dir {}", parent.display()))?;
    }
    std::fs::write(path, format!("{}\n", hex::encode(bytes)))
        .with_context(|| format!("failed to write wallet entropy file {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).with_context(
            || {
                format!(
                    "failed to set wallet entropy permissions {}",
                    path.display()
                )
            },
        )?;
    }
    Ok(())
}

fn wallet_backup_passphrase(passphrase_env: Option<&str>) -> Result<String> {
    let env_name = passphrase_env.unwrap_or(WALLET_BACKUP_DEFAULT_PASSPHRASE_ENV);
    let passphrase = std::env::var(env_name)
        .with_context(|| format!("missing wallet backup passphrase env {env_name}"))?;
    if passphrase.len() < 12 {
        bail!("wallet backup passphrase from {env_name} must be at least 12 bytes");
    }
    Ok(passphrase)
}

fn build_wallet_backup_plaintext_manifest(
    config: &crate::PylonConfig,
    layout: &WalletStorageLayout,
    surface: &WalletRuntimeSurface,
    storage_generation: &str,
    exported_at_ms: u64,
    include_identity_mnemonic: bool,
    export_path: &Path,
) -> Result<WalletBackupPlaintextManifest> {
    if export_path.starts_with(layout.ldk_dir.as_path()) {
        bail!(
            "wallet backup export path {} must be outside the wallet storage directory {}",
            export_path.display(),
            layout.ldk_dir.display()
        );
    }
    let identity_mnemonic = if include_identity_mnemonic {
        ensure_private_file_permissions(config.identity_path.as_path(), "identity mnemonic")?;
        Some(
            std::fs::read_to_string(config.identity_path.as_path())
                .with_context(|| {
                    format!(
                        "failed to read identity mnemonic {}",
                        config.identity_path.display()
                    )
                })?
                .trim()
                .to_string(),
        )
    } else {
        None
    };
    let snapshot = collect_wallet_backup_snapshot(layout, export_path)?;
    Ok(WalletBackupPlaintextManifest {
        schema_version: WALLET_BACKUP_SCHEMA_VERSION,
        kind: WALLET_BACKUP_PLAINTEXT_KIND.to_string(),
        exported_at_ms,
        runtime_kind: surface.runtime_kind.to_string(),
        network: surface.network.clone(),
        wallet_derivation: surface.node_entropy.clone(),
        storage_generation: storage_generation.to_string(),
        identity_mnemonic_included: identity_mnemonic.is_some(),
        identity_mnemonic,
        ldk_storage_snapshot: snapshot,
    })
}

fn collect_wallet_backup_snapshot(
    layout: &WalletStorageLayout,
    export_path: &Path,
) -> Result<Vec<WalletBackupSnapshotFile>> {
    let mut files = Vec::new();
    collect_wallet_backup_snapshot_dir(layout.ldk_dir.as_path(), layout, export_path, &mut files)?;
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

fn collect_wallet_backup_snapshot_dir(
    dir: &Path,
    layout: &WalletStorageLayout,
    export_path: &Path,
    files: &mut Vec<WalletBackupSnapshotFile>,
) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut entries = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read wallet backup source dir {}", dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| {
            format!(
                "failed to inspect wallet backup source dir {}",
                dir.display()
            )
        })?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path == layout.lock_path || path == export_path {
            continue;
        }
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect wallet path {}", path.display()))?;
        if file_type.is_dir() {
            collect_wallet_backup_snapshot_dir(path.as_path(), layout, export_path, files)?;
        } else if file_type.is_file() {
            let relative_path = wallet_backup_relative_path(layout.ldk_dir.as_path(), &path)?;
            let contents = std::fs::read(path.as_path())
                .with_context(|| format!("failed to read wallet backup file {}", path.display()))?;
            files.push(WalletBackupSnapshotFile {
                kind: wallet_backup_component_kind(relative_path.as_str()).to_string(),
                relative_path,
                size_bytes: contents.len() as u64,
                sha256: sha256_digest(contents.as_slice()),
                content_hex: hex::encode(contents),
            });
        }
    }
    Ok(())
}

fn wallet_backup_relative_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).with_context(|| {
        format!(
            "wallet backup path {} is outside root {}",
            path.display(),
            root.display()
        )
    })?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/"))
}

fn wallet_backup_component_kind(relative_path: &str) -> &'static str {
    if relative_path == "backup-manifest.json" {
        "backup_manifest"
    } else if relative_path == "last-registration.json" {
        "registration_metadata"
    } else if relative_path.starts_with("node/") {
        "ldk_node_state"
    } else if relative_path.starts_with("sqlite/") {
        "ldk_sqlite_state"
    } else if relative_path.starts_with("backup-staging/") {
        "ldk_backup_staging"
    } else {
        "ldk_metadata"
    }
}

fn encrypt_wallet_backup_plaintext(
    plaintext: &WalletBackupPlaintextManifest,
    passphrase: &str,
) -> Result<WalletBackupEncryptedFile> {
    let plaintext_bytes = serde_json::to_vec(plaintext)?;
    let plaintext_manifest_digest = sha256_digest(plaintext_bytes.as_slice());
    let salt: [u8; 16] = rand::random();
    let nonce: [u8; 24] = rand::random();
    let mut key = [0u8; WALLET_BACKUP_KEY_LEN];
    let params = ScryptParams::new(
        WALLET_BACKUP_SCRYPT_LOG_N,
        WALLET_BACKUP_SCRYPT_R,
        WALLET_BACKUP_SCRYPT_P,
        WALLET_BACKUP_KEY_LEN,
    )
    .context("failed to configure wallet backup scrypt params")?;
    scrypt(passphrase.as_bytes(), &salt, &params, &mut key)
        .context("failed to derive wallet backup encryption key")?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key)
        .map_err(|_| anyhow!("failed to initialize wallet backup cipher"))?;
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext_bytes.as_slice())
        .map_err(|_| anyhow!("failed to encrypt wallet backup"))?;
    let mut kinds = plaintext
        .ldk_storage_snapshot
        .iter()
        .map(|file| file.kind.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    kinds.sort();
    Ok(WalletBackupEncryptedFile {
        schema_version: WALLET_BACKUP_SCHEMA_VERSION,
        kind: WALLET_BACKUP_KIND.to_string(),
        public_manifest: WalletBackupPublicManifest {
            schema_version: WALLET_BACKUP_SCHEMA_VERSION,
            kind: WALLET_BACKUP_KIND.to_string(),
            exported_at_ms: plaintext.exported_at_ms,
            runtime_kind: plaintext.runtime_kind.clone(),
            network: plaintext.network.clone(),
            wallet_derivation_version: plaintext.wallet_derivation.derivation_version.clone(),
            node_entropy_digest: plaintext.wallet_derivation.digest.clone(),
            storage_generation: plaintext.storage_generation.clone(),
            encryption_algorithm: "XChaCha20Poly1305".to_string(),
            kdf: "scrypt".to_string(),
            plaintext_manifest_digest,
            plaintext_component_count: plaintext.ldk_storage_snapshot.len(),
            plaintext_total_bytes: plaintext
                .ldk_storage_snapshot
                .iter()
                .map(|file| file.size_bytes)
                .sum(),
            snapshot_kinds: kinds,
            identity_mnemonic_included: plaintext.identity_mnemonic_included,
        },
        encryption: WalletBackupEncryptionMetadata {
            algorithm: "XChaCha20Poly1305".to_string(),
            kdf: "scrypt".to_string(),
            salt_hex: hex::encode(salt),
            nonce_hex: hex::encode(nonce),
            scrypt_log_n: WALLET_BACKUP_SCRYPT_LOG_N,
            scrypt_r: WALLET_BACKUP_SCRYPT_R,
            scrypt_p: WALLET_BACKUP_SCRYPT_P,
            key_len: WALLET_BACKUP_KEY_LEN,
        },
        ciphertext_hex: hex::encode(ciphertext),
    })
}

fn decrypt_wallet_backup_plaintext(
    encrypted: &WalletBackupEncryptedFile,
    passphrase: &str,
) -> Result<WalletBackupPlaintextManifest> {
    if encrypted.schema_version != WALLET_BACKUP_SCHEMA_VERSION
        || encrypted.kind != WALLET_BACKUP_KIND
    {
        bail!("unsupported wallet backup file kind or schema version");
    }
    let salt = hex::decode(encrypted.encryption.salt_hex.as_str())
        .context("wallet backup salt is not hex")?;
    let nonce = hex::decode(encrypted.encryption.nonce_hex.as_str())
        .context("wallet backup nonce is not hex")?;
    if nonce.len() != 24 {
        bail!("wallet backup nonce must be 24 bytes");
    }
    let ciphertext = hex::decode(encrypted.ciphertext_hex.as_str())
        .context("wallet backup ciphertext is not hex")?;
    let mut key = vec![0u8; encrypted.encryption.key_len];
    let params = ScryptParams::new(
        encrypted.encryption.scrypt_log_n,
        encrypted.encryption.scrypt_r,
        encrypted.encryption.scrypt_p,
        encrypted.encryption.key_len,
    )
    .context("failed to configure wallet backup scrypt params")?;
    scrypt(
        passphrase.as_bytes(),
        salt.as_slice(),
        &params,
        key.as_mut_slice(),
    )
    .context("failed to derive wallet backup decryption key")?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_slice())
        .map_err(|_| anyhow!("failed to initialize wallet backup cipher"))?;
    let plaintext = cipher
        .decrypt(XNonce::from_slice(nonce.as_slice()), ciphertext.as_slice())
        .map_err(|_| anyhow!("failed to decrypt wallet backup; passphrase or file is wrong"))?;
    let digest = sha256_digest(plaintext.as_slice());
    if digest != encrypted.public_manifest.plaintext_manifest_digest {
        bail!("wallet backup plaintext digest mismatch");
    }
    serde_json::from_slice(plaintext.as_slice()).context("failed to parse wallet backup plaintext")
}

fn write_encrypted_wallet_backup(path: &Path, encrypted: &WalletBackupEncryptedFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create wallet backup dir {}", parent.display()))?;
    }
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(encrypted)?),
    )
    .with_context(|| format!("failed to write wallet backup {}", path.display()))?;
    set_private_file_permissions(path, "wallet backup")
}

fn read_encrypted_wallet_backup(path: &Path) -> Result<WalletBackupEncryptedFile> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read wallet backup {}", path.display()))?;
    serde_json::from_str(raw.as_str())
        .with_context(|| format!("failed to parse wallet backup {}", path.display()))
}

fn sha256_file_digest(path: &Path) -> Result<String> {
    let contents = std::fs::read(path)
        .with_context(|| format!("failed to read wallet backup {}", path.display()))?;
    Ok(sha256_digest(contents.as_slice()))
}

fn sha256_digest(contents: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(contents)))
}

fn read_wallet_restore_mnemonic(
    mnemonic_env: Option<&str>,
    mnemonic_file: Option<&Path>,
) -> Result<String> {
    match (mnemonic_env, mnemonic_file) {
        (Some(env_name), None) => {
            let value = std::env::var(env_name)
                .with_context(|| format!("missing wallet restore mnemonic env {env_name}"))?;
            normalize_wallet_restore_mnemonic(value.as_str())
        }
        (None, Some(path)) => {
            ensure_private_file_permissions(path, "wallet restore mnemonic file")?;
            let value = std::fs::read_to_string(path).with_context(|| {
                format!(
                    "failed to read wallet restore mnemonic file {}",
                    path.display()
                )
            })?;
            normalize_wallet_restore_mnemonic(value.as_str())
        }
        _ => bail!("wallet restore phrase requires exactly one mnemonic source"),
    }
}

fn normalize_wallet_restore_mnemonic(value: &str) -> Result<String> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        bail!("wallet restore mnemonic is empty");
    }
    Ok(normalized)
}

fn write_wallet_identity_mnemonic(path: &Path, mnemonic: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create identity dir {}", parent.display()))?;
    }
    std::fs::write(path, format!("{mnemonic}\n"))
        .with_context(|| format!("failed to write identity mnemonic {}", path.display()))?;
    set_private_file_permissions(path, "identity mnemonic")
}

fn refuse_active_wallet_restore(layout: &WalletStorageLayout) -> Result<()> {
    let report = wallet_lock_report(
        WalletRuntimeSurface {
            runtime_kind: PylonWalletRuntimeKind::LdkNode,
            liquidity_provider_kind: PylonWalletLiquidityProviderKind::None,
            network: String::new(),
            identity_path: String::new(),
            storage_dir: layout.root_dir.display().to_string(),
            api_key_env: None,
            api_key_source: String::new(),
            node_entropy: WalletNodeEntropyMetadata::default(),
        },
        layout,
        false,
    );
    if report.locked && !report.stale {
        bail!(
            "wallet restore refused because {} is locked by active pid {}; stop the live wallet first",
            layout.lock_path.display(),
            report
                .owner
                .as_ref()
                .map(|owner| owner.pid)
                .unwrap_or_default()
        );
    }
    if report.locked {
        std::fs::remove_file(layout.lock_path.as_path()).with_context(|| {
            format!(
                "failed to clear stale wallet lock before restore {}",
                layout.lock_path.display()
            )
        })?;
    }
    Ok(())
}

fn reset_wallet_storage_for_restore(layout: &WalletStorageLayout) -> Result<()> {
    for path in [
        layout.node_dir.as_path(),
        layout.sqlite_dir.as_path(),
        layout.backup_staging_dir.as_path(),
    ] {
        if path.exists() {
            std::fs::remove_dir_all(path).with_context(|| {
                format!("failed to reset wallet restore path {}", path.display())
            })?;
        }
    }
    for path in [
        layout.backup_manifest_path.as_path(),
        layout.last_registration_path.as_path(),
        layout.lock_path.as_path(),
    ] {
        if path.exists() {
            std::fs::remove_file(path).with_context(|| {
                format!("failed to reset wallet restore file {}", path.display())
            })?;
        }
    }
    create_private_dir(layout.node_dir.as_path(), "wallet node state")?;
    create_private_dir(layout.sqlite_dir.as_path(), "wallet sqlite state")?;
    create_private_dir(layout.backup_staging_dir.as_path(), "wallet backup staging")?;
    write_private_file_if_missing(
        layout.backup_manifest_path.as_path(),
        format!(
            "{{\"schema_version\":{WALLET_BACKUP_SCHEMA_VERSION},\"kind\":\"{WALLET_BACKUP_MANIFEST_KIND}\",\"status\":\"none\"}}\n"
        )
        .as_bytes(),
    )?;
    write_private_file_if_missing(
        layout.last_registration_path.as_path(),
        b"{\"schema_version\":1,\"kind\":\"pylon.wallet.last_registration\",\"status\":\"none\"}\n",
    )?;
    Ok(())
}

fn validate_wallet_backup_plaintext_for_restore(
    encrypted: &WalletBackupEncryptedFile,
    plaintext: &WalletBackupPlaintextManifest,
) -> Result<()> {
    if encrypted.public_manifest.runtime_kind != PylonWalletRuntimeKind::LdkNode.to_string()
        || plaintext.runtime_kind != PylonWalletRuntimeKind::LdkNode.to_string()
    {
        bail!("wallet backup restore requires an ldk_node backup");
    }
    if plaintext.wallet_derivation.derivation_version != WALLET_NODE_ENTROPY_DERIVATION_VERSION {
        bail!(
            "unsupported wallet backup derivation version {}",
            plaintext.wallet_derivation.derivation_version
        );
    }
    if plaintext.kind != WALLET_BACKUP_PLAINTEXT_KIND {
        bail!(
            "unsupported wallet backup plaintext kind {}",
            plaintext.kind
        );
    }
    if plaintext.schema_version != WALLET_BACKUP_SCHEMA_VERSION {
        bail!(
            "unsupported wallet backup plaintext schema {}",
            plaintext.schema_version
        );
    }
    if encrypted.public_manifest.exported_at_ms != plaintext.exported_at_ms
        || encrypted.public_manifest.network != plaintext.network
        || encrypted.public_manifest.runtime_kind != plaintext.runtime_kind
        || encrypted.public_manifest.wallet_derivation_version
            != plaintext.wallet_derivation.derivation_version
        || encrypted.public_manifest.node_entropy_digest != plaintext.wallet_derivation.digest
        || encrypted.public_manifest.storage_generation != plaintext.storage_generation
        || encrypted.public_manifest.identity_mnemonic_included
            != plaintext.identity_mnemonic_included
        || encrypted.public_manifest.plaintext_component_count
            != plaintext.ldk_storage_snapshot.len()
        || encrypted.public_manifest.plaintext_total_bytes
            != plaintext
                .ldk_storage_snapshot
                .iter()
                .map(|file| file.size_bytes)
                .sum::<u64>()
    {
        bail!("wallet backup public manifest does not match encrypted plaintext");
    }
    Ok(())
}

fn restore_wallet_backup_snapshot(
    layout: &WalletStorageLayout,
    files: &[WalletBackupSnapshotFile],
) -> Result<()> {
    for file in files {
        let relative = safe_wallet_backup_relative_path(file.relative_path.as_str())?;
        let path = layout.ldk_dir.join(relative);
        if let Some(parent) = path.parent() {
            create_private_dir(parent, "wallet restore parent")?;
        }
        let contents = hex::decode(file.content_hex.as_str()).with_context(|| {
            format!(
                "wallet backup component {} is not valid hex",
                file.relative_path
            )
        })?;
        if contents.len() as u64 != file.size_bytes {
            bail!(
                "wallet backup component {} size mismatch",
                file.relative_path
            );
        }
        if sha256_digest(contents.as_slice()) != file.sha256 {
            bail!(
                "wallet backup component {} digest mismatch",
                file.relative_path
            );
        }
        std::fs::write(path.as_path(), contents)
            .with_context(|| format!("failed to restore wallet file {}", path.display()))?;
        set_private_file_permissions(path.as_path(), "wallet restored file")?;
    }
    Ok(())
}

fn safe_wallet_backup_relative_path(relative_path: &str) -> Result<PathBuf> {
    let path = Path::new(relative_path);
    if path.is_absolute() {
        bail!("wallet backup component path must be relative");
    }
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(value) => clean.push(value),
            _ => bail!(
                "wallet backup component path contains unsafe segment: {}",
                relative_path
            ),
        }
    }
    if clean.as_os_str().is_empty() {
        bail!("wallet backup component path cannot be empty");
    }
    Ok(clean)
}

fn write_restored_wallet_backup_status_manifest(
    layout: &WalletStorageLayout,
    encrypted: &WalletBackupEncryptedFile,
    plaintext: &WalletBackupPlaintextManifest,
    backup_path: &Path,
    backup_file_digest: &str,
) -> Result<()> {
    let manifest = WalletBackupStatusManifest {
        schema_version: WALLET_BACKUP_SCHEMA_VERSION,
        kind: WALLET_BACKUP_MANIFEST_KIND.to_string(),
        status: "restored".to_string(),
        last_exported_at_ms: Some(plaintext.exported_at_ms),
        last_export_path: Some(backup_path.display().to_string()),
        last_export_file_digest: Some(backup_file_digest.to_string()),
        last_plaintext_manifest_digest: Some(
            encrypted.public_manifest.plaintext_manifest_digest.clone(),
        ),
        last_component_count: Some(encrypted.public_manifest.plaintext_component_count),
        last_total_bytes: Some(encrypted.public_manifest.plaintext_total_bytes),
        encryption_algorithm: Some(encrypted.public_manifest.encryption_algorithm.clone()),
        kdf: Some(encrypted.public_manifest.kdf.clone()),
    };
    write_private_json(
        layout.backup_manifest_path.as_path(),
        &manifest,
        "wallet backup manifest",
    )
}

fn sync_wallet_restore_receipt(config_path: &Path, report: &WalletRestoreReport) -> Result<()> {
    mutate_ledger(config_path, |ledger| {
        ledger.upsert_wallet_receipt(PylonWalletReceiptRecord {
            receipt_id: format!(
                "wallet:restore:{}:{}",
                report.mode,
                report
                    .plaintext_manifest_digest
                    .as_deref()
                    .or(report.backup_file_digest.as_deref())
                    .unwrap_or(report.runtime.node_entropy.digest.as_str())
            ),
            receipt_type: format!("wallet.restore.{}.v1", report.mode),
            status: report.status.clone(),
            direction: None,
            method: Some(report.mode.clone()),
            amount_sats: None,
            fees_sats: None,
            payment_id: None,
            payment_hash: None,
            txid: None,
            operation_id: report.backup_file_digest.clone(),
            settlement_id: None,
            failure_code: None,
            detail: Some(report.recovery_mode.clone()),
            created_at_ms: now_epoch_ms() as u64,
            updated_at_ms: now_epoch_ms() as u64,
        });
        Ok(())
    })
}

fn ensure_wallet_storage_layout_for_config_path(config_path: &Path) -> Result<WalletStorageLayout> {
    let config = ensure_local_setup(config_path)?;
    ensure_wallet_storage_layout(&config)
}

fn ensure_wallet_storage_layout(config: &crate::PylonConfig) -> Result<WalletStorageLayout> {
    ensure_wallet_storage_layout_for_root(config.wallet_storage_dir.clone())
}

fn ensure_wallet_storage_layout_for_root(root_dir: PathBuf) -> Result<WalletStorageLayout> {
    let layout = wallet_storage_layout_from_root(root_dir);
    create_private_dir(layout.root_dir.as_path(), "wallet root")?;
    create_private_dir(layout.ldk_dir.as_path(), "wallet ldk root")?;
    create_private_dir(layout.node_dir.as_path(), "wallet node state")?;
    create_private_dir(layout.sqlite_dir.as_path(), "wallet sqlite state")?;
    create_private_dir(layout.backup_staging_dir.as_path(), "wallet backup staging")?;
    write_private_file_if_missing(
        layout.backup_manifest_path.as_path(),
        format!(
            "{{\"schema_version\":{WALLET_BACKUP_SCHEMA_VERSION},\"kind\":\"{WALLET_BACKUP_MANIFEST_KIND}\",\"status\":\"none\"}}\n"
        )
            .as_bytes(),
    )?;
    write_private_file_if_missing(
        layout.last_registration_path.as_path(),
        b"{\"schema_version\":1,\"kind\":\"pylon.wallet.last_registration\",\"status\":\"none\"}\n",
    )?;
    Ok(layout)
}

fn wallet_storage_layout_from_root(root_dir: PathBuf) -> WalletStorageLayout {
    let ldk_dir = root_dir.join("ldk");
    WalletStorageLayout {
        root_dir,
        node_dir: ldk_dir.join("node"),
        sqlite_dir: ldk_dir.join("sqlite"),
        backup_staging_dir: ldk_dir.join("backup-staging"),
        lock_path: ldk_dir.join("wallet-lock"),
        backup_manifest_path: ldk_dir.join("backup-manifest.json"),
        last_registration_path: ldk_dir.join("last-registration.json"),
        ldk_dir,
    }
}

#[cfg(test)]
fn wallet_storage_layout_report(layout: &WalletStorageLayout) -> WalletStorageLayoutReport {
    WalletStorageLayoutReport {
        schema_version: WALLET_STORAGE_SCHEMA_VERSION,
        root_dir: layout.root_dir.display().to_string(),
        ldk_dir: layout.ldk_dir.display().to_string(),
        node_dir: layout.node_dir.display().to_string(),
        sqlite_dir: layout.sqlite_dir.display().to_string(),
        backup_staging_dir: layout.backup_staging_dir.display().to_string(),
        lock_path: layout.lock_path.display().to_string(),
        backup_manifest_path: layout.backup_manifest_path.display().to_string(),
        last_registration_path: layout.last_registration_path.display().to_string(),
    }
}

fn acquire_wallet_storage_lock(layout: &WalletStorageLayout) -> Result<PylonWalletStorageLock> {
    let owner = current_wallet_lock_owner();
    let payload = serde_json::to_string(&owner)?;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(layout.lock_path.as_path())
    {
        Ok(mut file) => {
            file.write_all(payload.as_bytes()).with_context(|| {
                format!("failed to write wallet lock {}", layout.lock_path.display())
            })?;
            file.write_all(b"\n").with_context(|| {
                format!(
                    "failed to finalize wallet lock {}",
                    layout.lock_path.display()
                )
            })?;
            set_private_file_permissions(layout.lock_path.as_path(), "wallet lock")?;
            Ok(PylonWalletStorageLock {
                path: layout.lock_path.clone(),
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let report = wallet_lock_report(WalletRuntimeSurface::default(), layout, false);
            let detail = if report.stale {
                format!(
                    "stale wallet lock at {}; inspect with `pylon wallet lock status` and clear with `pylon wallet lock clear`",
                    layout.lock_path.display()
                )
            } else {
                format!(
                    "wallet storage is locked by another active Pylon process at {}; stop that process before starting this wallet",
                    layout.lock_path.display()
                )
            };
            bail!("{detail}")
        }
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to create wallet lock {}",
                layout.lock_path.display()
            )
        }),
    }
}

fn wallet_lock_report(
    runtime: WalletRuntimeSurface,
    layout: &WalletStorageLayout,
    cleared: bool,
) -> WalletLockReport {
    let locked = layout.lock_path.exists();
    let owner = read_wallet_lock_owner(layout.lock_path.as_path())
        .ok()
        .flatten();
    let stale = locked
        && owner
            .as_ref()
            .map_or(true, |owner| !wallet_lock_owner_is_active(owner));
    let detail = if cleared {
        "stale wallet lock cleared".to_string()
    } else if !locked {
        "wallet storage is unlocked".to_string()
    } else if stale {
        "wallet lock is stale and can be cleared explicitly".to_string()
    } else {
        "wallet storage is locked by an active process".to_string()
    };
    WalletLockReport {
        runtime,
        lock_path: layout.lock_path.display().to_string(),
        locked,
        stale,
        cleared,
        owner,
        detail,
    }
}

fn read_wallet_lock_owner(path: &Path) -> Result<Option<WalletLockOwner>> {
    if !path.exists() {
        return Ok(None);
    }
    let payload = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read wallet lock {}", path.display()))?;
    let owner = serde_json::from_str::<WalletLockOwner>(payload.as_str())
        .with_context(|| format!("failed to parse wallet lock {}", path.display()))?;
    Ok(Some(owner))
}

fn current_wallet_lock_owner() -> WalletLockOwner {
    WalletLockOwner {
        pid: std::process::id(),
        machine_id: local_machine_id(),
        created_at_ms: now_epoch_ms() as u64,
    }
}

fn local_machine_id() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

fn wallet_lock_owner_is_active(owner: &WalletLockOwner) -> bool {
    if owner.machine_id != local_machine_id() {
        return true;
    }
    process_is_active(owner.pid)
}

#[cfg(unix)]
fn process_is_active(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let result = unsafe { libc::kill(pid as i32, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn process_is_active(pid: u32) -> bool {
    pid == std::process::id()
}

fn create_private_dir(path: &Path, label: &str) -> Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("failed to create {label} {}", path.display()))?;
    set_private_dir_permissions(path, label)
}

fn write_private_file_if_missing(path: &Path, contents: &[u8]) -> Result<()> {
    if path.exists() {
        ensure_private_file_permissions(path, "wallet metadata")?;
        return Ok(());
    }
    std::fs::write(path, contents)
        .with_context(|| format!("failed to write wallet metadata {}", path.display()))?;
    set_private_file_permissions(path, "wallet metadata")
}

fn write_private_json<T: Serialize>(path: &Path, value: &T, label: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_private_dir(parent, label)?;
    }
    let contents = format!("{}\n", serde_json::to_string_pretty(value)?);
    std::fs::write(path, contents)
        .with_context(|| format!("failed to write {label} {}", path.display()))?;
    set_private_file_permissions(path, label)
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path, label: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("failed to set {label} permissions {}", path.display()))
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path, _label: &str) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path, label: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to set {label} permissions {}", path.display()))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path, _label: &str) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn ensure_private_file_permissions(path: &Path, label: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path)
        .with_context(|| format!("failed to read {label} permissions {}", path.display()))?
        .permissions()
        .mode()
        & 0o777;
    if mode & 0o077 != 0 {
        bail!(
            "{label} {} must not be readable or writable by group/other; chmod 600 and retry",
            path.display()
        );
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_private_file_permissions(_path: &Path, _label: &str) -> Result<()> {
    Ok(())
}

fn prepare_wallet_context(config_path: &Path) -> Result<WalletRuntimeContext> {
    let config = ensure_local_setup(config_path)?;
    let _layout = ensure_wallet_storage_layout(&config)?;
    let runtime_kind = config.wallet_runtime_kind;
    let node_entropy = load_wallet_node_entropy_material(&config)?;
    let liquidity_provider_kind = match runtime_kind {
        PylonWalletRuntimeKind::MoneyDevKit => PylonWalletLiquidityProviderKind::MoneyDevKit,
        _ => PylonWalletLiquidityProviderKind::None,
    };
    let surface = WalletRuntimeSurface {
        runtime_kind,
        liquidity_provider_kind,
        network: config.wallet_network.clone(),
        identity_path: config.identity_path.display().to_string(),
        storage_dir: config.wallet_storage_dir.display().to_string(),
        api_key_env: config.wallet_api_key_env.clone(),
        api_key_source: config
            .wallet_api_key_env
            .as_deref()
            .map(|name| format!("env:{name}"))
            .unwrap_or_else(|| format!("none:{}", runtime_kind.id())),
        node_entropy: node_entropy.metadata,
    };
    let runtime = match runtime_kind {
        PylonWalletRuntimeKind::ExternalTarget => {
            SelectedPylonWalletRuntime::ExternalTarget(ExternalTargetWalletRuntime {
                surface,
                payout_destination: crate::configured_external_payout_target(&config),
            })
        }
        PylonWalletRuntimeKind::Mock => SelectedPylonWalletRuntime::Mock(MockPylonWalletRuntime {
            surface,
            payout_destination: crate::configured_external_payout_target(&config),
        }),
        PylonWalletRuntimeKind::MoneyDevKit => {
            SelectedPylonWalletRuntime::MoneyDevKit(MoneyDevKitWalletRuntime {
                surface,
                home_dir: config.wallet_storage_dir.join("moneydevkit-home"),
                network: config.wallet_network.clone(),
            })
        }
        PylonWalletRuntimeKind::LdkNode => {
            SelectedPylonWalletRuntime::LdkNode(LdkNodeWalletRuntime {
                surface,
                settings: LdkNodeWalletSettings {
                    chain_source_kind: config.wallet_chain_source_kind.clone(),
                    esplora_url: config.wallet_esplora_url.clone(),
                    electrum_url: config.wallet_electrum_url.clone(),
                    rgs_url: config.wallet_rgs_url.clone(),
                },
                entropy: node_entropy.bytes,
                lock: Arc::new(Mutex::new(None)),
                node: Arc::new(Mutex::new(None)),
                last_error: Arc::new(Mutex::new(None)),
            })
        }
    };
    Ok(WalletRuntimeContext { runtime })
}

fn compute_wallet_credit_summary(
    payments: &[PylonWalletPaymentRecord],
    now_ms: u64,
) -> PylonWalletCreditSummary {
    let current_day = now_ms / 86_400_000;
    let mut credits = PylonWalletCreditSummary::default();
    for payment in payments {
        if !wallet_payment_counts_as_credit(payment) {
            continue;
        }
        credits.credited_lifetime_sats = credits
            .credited_lifetime_sats
            .saturating_add(payment.amount_sats);
        if payment.created_at_ms / 86_400_000 == current_day {
            credits.credited_today_sats = credits
                .credited_today_sats
                .saturating_add(payment.amount_sats);
            credits.credited_today_count = credits.credited_today_count.saturating_add(1);
        }
        credits.last_credit_at_ms = Some(
            credits
                .last_credit_at_ms
                .unwrap_or(0)
                .max(payment.created_at_ms),
        );
    }
    credits.last_full_sync_at_ms = Some(now_ms);
    credits
}

fn wallet_payment_counts_as_credit(payment: &PylonWalletPaymentRecord) -> bool {
    payment.direction.eq_ignore_ascii_case("receive")
        && matches!(
            payment.status.to_ascii_lowercase().as_str(),
            "succeeded" | "success" | "settled" | "completed" | "confirmed"
        )
}

fn sync_wallet_status(
    config_path: &Path,
    runtime: &WalletRuntimeSurface,
    runtime_status: &str,
    runtime_detail: Option<String>,
    balance: Option<&WalletBalanceSnapshot>,
    bitcoin_address: Option<&str>,
    payments: &[PylonWalletPaymentRecord],
) -> Result<()> {
    let runtime_detail = runtime_detail.map(|detail| redact_wallet_secret_text(detail.as_str()));
    mutate_ledger(config_path, |ledger| {
        let detail_for_receipt = runtime_detail.clone();
        ledger.wallet.runtime_status = Some(runtime_status.to_string());
        ledger.wallet.last_error = runtime_detail;
        ledger.wallet.network = Some(runtime.network.clone());
        ledger.wallet.entropy_source = Some(runtime.node_entropy.source.clone());
        ledger.wallet.entropy_derivation_version =
            Some(runtime.node_entropy.derivation_version.clone());
        ledger.wallet.entropy_domain_label = Some(runtime.node_entropy.domain_label.clone());
        ledger.wallet.node_entropy_digest = Some(runtime.node_entropy.digest.clone());
        if let Some(balance) = balance {
            ledger.wallet.last_balance_sats = Some(balance.total_sats);
            ledger.wallet.last_balance_at_ms = Some(now_epoch_ms() as u64);
        }
        if let Some(bitcoin_address) = bitcoin_address {
            ledger.wallet.bitcoin_address = Some(bitcoin_address.to_string());
        }
        for payment in payments {
            ledger.upsert_wallet_payment(payment.clone());
        }
        ledger.upsert_wallet_receipt(wallet_status_receipt(
            runtime,
            runtime_status,
            detail_for_receipt.as_deref(),
        ));
        Ok(())
    })
}

fn sync_wallet_credit_summary(
    config_path: &Path,
    credits: &PylonWalletCreditSummary,
) -> Result<()> {
    mutate_ledger(config_path, |ledger| {
        ledger.wallet.credits = credits.clone();
        Ok(())
    })
}

fn sync_wallet_error(
    config_path: &Path,
    runtime: &WalletRuntimeSurface,
    error: String,
) -> Result<()> {
    let error = redact_wallet_secret_text(error.as_str());
    mutate_ledger(config_path, |ledger| {
        ledger.wallet.runtime_status = Some("error".to_string());
        ledger.wallet.last_error = Some(error);
        ledger.wallet.network = Some(runtime.network.clone());
        ledger.wallet.entropy_source = Some(runtime.node_entropy.source.clone());
        ledger.wallet.entropy_derivation_version =
            Some(runtime.node_entropy.derivation_version.clone());
        ledger.wallet.entropy_domain_label = Some(runtime.node_entropy.domain_label.clone());
        ledger.wallet.node_entropy_digest = Some(runtime.node_entropy.digest.clone());
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use bip39::{Language, Mnemonic};

    use super::{
        PylonWalletChannelRecord, PylonWalletLiquidityProviderKind, PylonWalletRuntime,
        PylonWalletRuntimeKind, WalletLockOwner, WalletSubcommand, compute_wallet_credit_summary,
        create_wallet_address_report, create_wallet_invoice_report, create_wallet_offer_report,
        load_wallet_channels_report, load_wallet_history_report, load_wallet_node_entropy_material,
        load_wallet_status_report, load_wallet_telemetry_report, parse_wallet_command,
        pay_wallet_invoice_report, redact_wallet_secret_text, redact_wallet_telemetry_endpoint,
        render_wallet_channels_report, render_wallet_entropy_report, render_wallet_status_report,
        render_wallet_telemetry_report, run_wallet_command, wallet_lightning_readiness,
        wallet_node_entropy_domain_label,
    };
    use crate::PylonWalletPaymentRecord;

    #[test]
    fn parse_wallet_command_supports_balance_and_history() {
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("balance"),
                    String::from("--json"),
                ],
                0,
            )
            .expect("wallet balance should parse"),
            WalletSubcommand::Balance { json: true }
        );
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("sync"),
                    String::from("--json"),
                ],
                0,
            )
            .expect("wallet sync should parse"),
            WalletSubcommand::Sync { json: true }
        );
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("channels"),
                    String::from("--json"),
                ],
                0,
            )
            .expect("wallet channels should parse"),
            WalletSubcommand::Channels { json: true }
        );
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("telemetry"),
                    String::from("--json"),
                ],
                0,
            )
            .expect("wallet telemetry should parse"),
            WalletSubcommand::Telemetry { json: true }
        );
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("history"),
                    String::from("--limit"),
                    String::from("5"),
                ],
                0,
            )
            .expect("wallet history should parse"),
            WalletSubcommand::History {
                limit: Some(5),
                json: false,
            }
        );
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("backup"),
                    String::from("export"),
                    String::from("/tmp/pylon-wallet-backup.json"),
                    String::from("--passphrase-env"),
                    String::from("PYLON_TEST_BACKUP_PASSPHRASE"),
                    String::from("--include-identity-mnemonic"),
                    String::from("--json"),
                ],
                0,
            )
            .expect("wallet backup export should parse"),
            WalletSubcommand::BackupExport {
                path: PathBuf::from("/tmp/pylon-wallet-backup.json"),
                passphrase_env: Some("PYLON_TEST_BACKUP_PASSPHRASE".to_string()),
                include_identity_mnemonic: true,
                json: true,
            }
        );
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("backup"),
                    String::from("inspect"),
                    String::from("/tmp/pylon-wallet-backup.json"),
                    String::from("--json"),
                ],
                0,
            )
            .expect("wallet backup inspect should parse"),
            WalletSubcommand::BackupInspect {
                path: PathBuf::from("/tmp/pylon-wallet-backup.json"),
                json: true,
            }
        );
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("restore"),
                    String::from("phrase"),
                    String::from("--mnemonic-env"),
                    String::from("PYLON_TEST_RESTORE_MNEMONIC"),
                    String::from("--wallet-network"),
                    String::from("regtest"),
                    String::from("--yes"),
                    String::from("--json"),
                ],
                0,
            )
            .expect("wallet restore phrase should parse"),
            WalletSubcommand::RestorePhrase {
                mnemonic_env: Some("PYLON_TEST_RESTORE_MNEMONIC".to_string()),
                mnemonic_file: None,
                wallet_network: Some("regtest".to_string()),
                yes: true,
                json: true,
            }
        );
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("restore"),
                    String::from("backup"),
                    String::from("/tmp/pylon-wallet-backup.json"),
                    String::from("--passphrase-env"),
                    String::from("PYLON_TEST_BACKUP_PASSPHRASE"),
                    String::from("--yes"),
                    String::from("--json"),
                ],
                0,
            )
            .expect("wallet restore backup should parse"),
            WalletSubcommand::RestoreBackup {
                path: PathBuf::from("/tmp/pylon-wallet-backup.json"),
                passphrase_env: Some("PYLON_TEST_BACKUP_PASSPHRASE".to_string()),
                yes: true,
                json: true,
            }
        );
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("entropy"),
                    String::from("export"),
                    String::from("/tmp/pylon-entropy.hex"),
                    String::from("--json"),
                ],
                0,
            )
            .expect("wallet entropy export should parse"),
            WalletSubcommand::EntropyExport {
                path: PathBuf::from("/tmp/pylon-entropy.hex"),
                json: true,
            }
        );
    }

    #[test]
    fn parse_wallet_command_supports_invoice_and_pay() {
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("invoice"),
                    String::from("21"),
                    String::from("--description"),
                    String::from("earn"),
                ],
                0,
            )
            .expect("wallet invoice should parse"),
            WalletSubcommand::Invoice {
                amount_sats: 21,
                description: Some(String::from("earn")),
                expiry_seconds: None,
                json: false,
            }
        );
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("offer"),
                    String::from("--amount-sats"),
                    String::from("21"),
                    String::from("--description"),
                    String::from("earn"),
                    String::from("--expiry-seconds"),
                    String::from("60"),
                    String::from("--json"),
                ],
                0,
            )
            .expect("wallet offer should parse"),
            WalletSubcommand::Offer {
                amount_sats: Some(21),
                description: Some(String::from("earn")),
                expiry_seconds: Some(60),
                json: true,
            }
        );
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("pay"),
                    String::from("lnbc1example"),
                    String::from("--amount-sats"),
                    String::from("8"),
                    String::from("--yes"),
                    String::from("--json"),
                ],
                0,
            )
            .expect("wallet pay should parse"),
            WalletSubcommand::Pay {
                payment_request: String::from("lnbc1example"),
                amount_sats: Some(8),
                yes: true,
                json: true,
            }
        );
    }

    #[test]
    fn wallet_credit_summary_uses_created_at_for_today_and_counts_all_history() {
        let now_ms = 1_762_580_000_000u64;
        let current_day = now_ms / 86_400_000;
        let today_created_at_ms = current_day * 86_400_000 + 1_000;
        let yesterday_created_at_ms = today_created_at_ms.saturating_sub(86_400_000);
        let credits = compute_wallet_credit_summary(
            &[
                PylonWalletPaymentRecord {
                    payment_id: "credit-today".to_string(),
                    direction: "receive".to_string(),
                    status: "settled".to_string(),
                    amount_sats: 21,
                    fees_sats: 0,
                    method: "lightning".to_string(),
                    description: None,
                    invoice: None,
                    payment_hash: None,
                    txid: None,
                    operation_id: None,
                    receipt_id: None,
                    failure_code: None,
                    created_at_ms: today_created_at_ms,
                    updated_at_ms: yesterday_created_at_ms,
                },
                PylonWalletPaymentRecord {
                    payment_id: "credit-old".to_string(),
                    direction: "receive".to_string(),
                    status: "settled".to_string(),
                    amount_sats: 34,
                    fees_sats: 0,
                    method: "lightning".to_string(),
                    description: None,
                    invoice: None,
                    payment_hash: None,
                    txid: None,
                    operation_id: None,
                    receipt_id: None,
                    failure_code: None,
                    created_at_ms: yesterday_created_at_ms,
                    updated_at_ms: now_ms,
                },
                PylonWalletPaymentRecord {
                    payment_id: "send".to_string(),
                    direction: "send".to_string(),
                    status: "settled".to_string(),
                    amount_sats: 13,
                    fees_sats: 1,
                    method: "lightning".to_string(),
                    description: None,
                    invoice: None,
                    payment_hash: None,
                    txid: None,
                    operation_id: None,
                    receipt_id: None,
                    failure_code: None,
                    created_at_ms: today_created_at_ms,
                    updated_at_ms: today_created_at_ms,
                },
            ],
            now_ms,
        );

        assert_eq!(credits.credited_lifetime_sats, 55);
        assert_eq!(credits.credited_today_sats, 21);
        assert_eq!(credits.credited_today_count, 1);
        assert_eq!(credits.last_credit_at_ms, Some(today_created_at_ms));
        assert_eq!(credits.last_full_sync_at_ms, Some(now_ms));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn external_target_runtime_selection_reports_runtime_kind() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_path = temp_dir.path().join("config.json");
        let mut config = crate::default_config(temp_dir.path());
        config.wallet_runtime_kind = PylonWalletRuntimeKind::ExternalTarget;
        config.wallet_network = "ldk-external".to_string();
        config.external_payout_target = Some("lno1externaltarget".to_string());
        crate::save_config(config_path.as_path(), &config).expect("save config");

        let report = load_wallet_status_report(config_path.as_path())
            .await
            .expect("status report");
        assert_eq!(
            report.runtime.runtime_kind,
            PylonWalletRuntimeKind::ExternalTarget
        );
        assert_eq!(report.runtime_status, "external_target");

        let json = run_wallet_command(
            config_path.as_path(),
            &WalletSubcommand::Status { json: true },
        )
        .await
        .expect("status json");
        assert!(json.contains("\"runtime_kind\": \"external_target\""));
    }

    #[test]
    fn identity_mnemonic_derives_stable_domain_separated_node_entropy() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let identity_path = temp_dir.path().join("identity.mnemonic");
        std::fs::write(
            identity_path.as_path(),
            "legal winner thank year wave sausage worth useful legal winner thank yellow\n",
        )
        .expect("write identity phrase");
        let mut config = crate::default_config(temp_dir.path());
        config.identity_path = identity_path;
        config.wallet_network = "regtest".to_string();

        let material = load_wallet_node_entropy_material(&config).expect("derive entropy");
        assert_eq!(
            hex::encode(material.bytes),
            "4a416223cc838ebf2cf4d73a1dce0cf8e737efed66885313d08b3adccb4657cfeb237528c9fb18c4fe080dc2d4027215f7bb92bf79b34ccab7ca6cda4af3a522"
        );
        assert_eq!(
            material.metadata.digest,
            "sha256:c68d883864e25cd51b513586b985900041d6631d82114bce7d0802848779a601"
        );
        assert_eq!(material.metadata.source, "identity_mnemonic_hkdf");
        assert_eq!(
            material.metadata.derivation_version,
            "pylon-ldk-node-entropy-v1"
        );
        assert_eq!(
            material.metadata.domain_label,
            "openagents-pylon/ldk-node/v1/regtest"
        );

        let mut mainnet = config.clone();
        mainnet.wallet_network = "mainnet".to_string();
        let mainnet_material =
            load_wallet_node_entropy_material(&mainnet).expect("derive mainnet entropy");
        assert_ne!(material.bytes, mainnet_material.bytes);
        assert_ne!(material.metadata.digest, mainnet_material.metadata.digest);

        let alternate = super::hkdf_sha256_64(
            Mnemonic::parse_in_normalized(
                Language::English,
                "legal winner thank year wave sausage worth useful legal winner thank yellow",
            )
            .expect("parse mnemonic")
            .to_seed("")
            .as_slice(),
            b"openagents-pylon/ldk-node/v2/regtest",
        );
        assert_ne!(material.bytes, alternate);
        assert_ne!(
            wallet_node_entropy_domain_label("regtest"),
            "openagents-pylon/ldk-node/v2/regtest"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn wallet_status_redacts_mnemonic_and_raw_node_entropy() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_path = temp_dir.path().join("config.json");
        let phrase = "legal winner thank year wave sausage worth useful legal winner thank yellow";
        let mut config = crate::default_config(temp_dir.path());
        config.wallet_network = "regtest".to_string();
        std::fs::write(config.identity_path.as_path(), format!("{phrase}\n"))
            .expect("write identity phrase");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                config.identity_path.as_path(),
                std::fs::Permissions::from_mode(0o600),
            )
            .expect("set identity permissions");
        }
        crate::save_config(config_path.as_path(), &config).expect("save config");

        let status = load_wallet_status_report(config_path.as_path())
            .await
            .expect("status");
        let rendered = render_wallet_status_report(&status);
        let json = run_wallet_command(
            config_path.as_path(),
            &WalletSubcommand::Status { json: true },
        )
        .await
        .expect("status json");
        let raw_entropy_hex = "4a416223cc838ebf2cf4d73a1dce0cf8e737efed66885313d08b3adccb4657cfeb237528c9fb18c4fe080dc2d4027215f7bb92bf79b34ccab7ca6cda4af3a522";

        assert!(rendered.contains("entropy_derivation_version: pylon-ldk-node-entropy-v1"));
        assert!(json.contains("\"derivation_version\": \"pylon-ldk-node-entropy-v1\""));
        let ledger = crate::load_ledger(config_path.as_path()).expect("ledger");
        assert_eq!(
            ledger.wallet.entropy_derivation_version.as_deref(),
            Some("pylon-ldk-node-entropy-v1")
        );
        assert_eq!(
            ledger.wallet.node_entropy_digest.as_deref(),
            Some("sha256:c68d883864e25cd51b513586b985900041d6631d82114bce7d0802848779a601")
        );
        assert!(!rendered.contains(phrase));
        assert!(!json.contains(phrase));
        assert!(!rendered.contains(raw_entropy_hex));
        assert!(!json.contains(raw_entropy_hex));
        assert!(
            json.contains(
                "sha256:c68d883864e25cd51b513586b985900041d6631d82114bce7d0802848779a601"
            )
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn wallet_telemetry_reports_health_and_redacts_secret_material() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_path = temp_dir.path().join("config.json");
        let phrase = "legal winner thank year wave sausage worth useful legal winner thank yellow";
        let mut config = crate::default_config(temp_dir.path());
        config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
        config.wallet_network = "regtest".to_string();
        config.wallet_chain_source_kind = "none".to_string();
        config.wallet_rgs_url =
            Some("https://rgs.example.invalid/snapshot?access_token=rgs-secret".to_string());
        std::fs::write(config.identity_path.as_path(), format!("{phrase}\n"))
            .expect("write identity phrase");
        #[cfg(unix)]
        super::set_private_file_permissions(config.identity_path.as_path(), "identity mnemonic")
            .expect("set identity permissions");
        crate::save_config(config_path.as_path(), &config).expect("save config");

        let report = load_wallet_telemetry_report(config_path.as_path())
            .await
            .expect("wallet telemetry");
        let rendered = render_wallet_telemetry_report(&report);
        let json = run_wallet_command(
            config_path.as_path(),
            &WalletSubcommand::Telemetry { json: true },
        )
        .await
        .expect("wallet telemetry json");

        assert_eq!(report.schema, "pylon.wallet.telemetry.v1");
        assert_eq!(report.runtime.runtime_kind, PylonWalletRuntimeKind::LdkNode);
        assert_eq!(report.sources.chain_source_kind, "none");
        assert_eq!(report.sources.gossip_source_kind, "rgs");
        assert_eq!(
            report.sources.gossip_source_url.as_deref(),
            Some("https://rgs.example.invalid/snapshot?[redacted]")
        );
        assert_eq!(report.backup.status, "backup_missing");
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.code == "wallet_chain_source_not_configured")
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.code == "backup_missing")
        );
        assert!(rendered.contains("health_state: configured_not_running"));
        assert!(json.contains("\"policy\": \"pylon.wallet.telemetry.redacted.v1\""));
        for secret in [
            phrase,
            "rgs-secret",
            "private-key-secret",
            "payment-preimage-secret",
            "raw-channel-state-secret",
            "4a416223cc838ebf2cf4d73a1dce0cf8e737efed66885313d08b3adccb4657cfeb237528c9fb18c4fe080dc2d4027215f7bb92bf79b34ccab7ca6cda4af3a522",
        ] {
            assert!(!rendered.contains(secret), "rendered leaked {secret}");
            assert!(!json.contains(secret), "json leaked {secret}");
        }
    }

    #[test]
    fn wallet_telemetry_redacts_endpoint_credentials_and_error_tokens() {
        let endpoint = redact_wallet_telemetry_endpoint(
            "https://alice:secret@example.invalid/rgs?token=abc#frag",
        );
        assert_eq!(
            endpoint,
            "https://[redacted]@example.invalid/rgs?[redacted]"
        );

        let detail = redact_wallet_secret_text(
            "ldk failed with token=abc private_key=def preimage=ghi mnemonic=legal winner Bearer xyz",
        );
        assert!(!detail.contains("abc"));
        assert!(!detail.contains("def"));
        assert!(!detail.contains("ghi"));
        assert!(!detail.contains("legal"));
        assert!(!detail.contains("xyz"));
        assert!(detail.contains("token=[redacted]"));
        assert!(detail.contains("private_key=[redacted]"));
        assert!(detail.contains("preimage=[redacted]"));
        assert!(detail.contains("mnemonic=[redacted]"));
        assert!(detail.contains("Bearer [redacted]"));
    }

    #[test]
    fn wallet_lightning_readiness_projects_channel_states() {
        let no_channels = wallet_lightning_readiness(
            PylonWalletRuntimeKind::LdkNode,
            PylonWalletLiquidityProviderKind::None,
            &[],
            true,
        );
        assert_eq!(no_channels.state, "onchain_only_no_channels");
        assert!(!no_channels.can_receive_lightning);
        assert!(no_channels.can_receive_onchain);
        assert_eq!(
            no_channels.warning_code.as_deref(),
            Some("lightning_receive_unavailable_no_channels")
        );

        let pending = wallet_lightning_readiness(
            PylonWalletRuntimeKind::LdkNode,
            PylonWalletLiquidityProviderKind::None,
            &[test_channel("pending", 0, 0, false)],
            true,
        );
        assert_eq!(pending.state, "channel_pending");
        assert_eq!(
            pending.warning_code.as_deref(),
            Some("lightning_receive_pending_channel")
        );

        let usable = wallet_lightning_readiness(
            PylonWalletRuntimeKind::LdkNode,
            PylonWalletLiquidityProviderKind::None,
            &[test_channel("usable", 4_000, 6_000, true)],
            true,
        );
        assert_eq!(usable.state, "lightning_ready");
        assert!(usable.can_receive_lightning);
        assert!(usable.can_send_lightning);
        assert_eq!(usable.peer_connected_count, 1);
        assert_eq!(usable.inbound_liquidity_sats, 4_000);
        assert_eq!(usable.outbound_liquidity_sats, 6_000);

        let route_limited = wallet_lightning_readiness(
            PylonWalletRuntimeKind::LdkNode,
            PylonWalletLiquidityProviderKind::None,
            &[test_channel("usable", 0, 6_000, true)],
            true,
        );
        assert_eq!(route_limited.state, "needs_inbound_liquidity");
        assert_eq!(
            route_limited.warning_code.as_deref(),
            Some("lightning_receive_needs_inbound_liquidity")
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn wallet_channels_command_reports_mock_liquidity_readiness() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_path = temp_dir.path().join("config.json");
        let mut config = crate::default_config(temp_dir.path());
        config.wallet_runtime_kind = PylonWalletRuntimeKind::Mock;
        crate::save_config(config_path.as_path(), &config).expect("save config");

        let report = load_wallet_channels_report(config_path.as_path())
            .await
            .expect("channels report");
        assert_eq!(report.channel_summary.usable_count, 1);
        assert!(report.lightning_readiness.can_receive_lightning);
        assert!(report.lightning_readiness.can_send_lightning);
        assert_eq!(report.liquidity.inbound_sats, 500);
        assert_eq!(report.liquidity.outbound_sats, 500);

        let rendered = render_wallet_channels_report(&report);
        let json = run_wallet_command(
            config_path.as_path(),
            &WalletSubcommand::Channels { json: true },
        )
        .await
        .expect("channels json");
        assert!(rendered.contains("lightning_receive_state: lightning_ready"));
        assert!(json.contains("\"peer_connected\": true"));
    }

    fn test_channel(
        status: &str,
        inbound_sats: u64,
        outbound_sats: u64,
        peer_connected: bool,
    ) -> PylonWalletChannelRecord {
        PylonWalletChannelRecord {
            channel_id: format!("test-channel-{status}"),
            status: status.to_string(),
            counterparty_node_id: Some("02testpeer".to_string()),
            funding_txo: Some("test-txid:0".to_string()),
            channel_value_sats: inbound_sats.saturating_add(outbound_sats),
            inbound_sats,
            outbound_sats,
            inbound_htlc_maximum_sats: Some(inbound_sats),
            next_outbound_htlc_limit_sats: outbound_sats,
            confirmations: Some(6),
            confirmations_required: Some(1),
            is_outbound: true,
            is_public: false,
            peer_connected,
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn entropy_import_export_is_explicit_and_redacted() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_path = temp_dir.path().join("config.json");
        let export_path = temp_dir.path().join("wallet-entropy.hex");
        let mut config = crate::default_config(temp_dir.path());
        config.wallet_network = "regtest".to_string();
        std::fs::write(
            config.identity_path.as_path(),
            "legal winner thank year wave sausage worth useful legal winner thank yellow\n",
        )
        .expect("write identity phrase");
        crate::save_config(config_path.as_path(), &config).expect("save config");

        let export =
            super::export_wallet_entropy_report(config_path.as_path(), export_path.as_path())
                .await
                .expect("export entropy");
        let exported_hex = std::fs::read_to_string(export_path.as_path())
            .expect("read exported entropy")
            .trim()
            .to_string();
        assert_eq!(exported_hex.len(), 128);
        let rendered_export = render_wallet_entropy_report(&export);
        assert!(!rendered_export.contains(exported_hex.as_str()));
        assert!(rendered_export.contains("node_entropy_digest: sha256:"));

        let import =
            super::import_wallet_entropy_report(config_path.as_path(), export_path.as_path())
                .await
                .expect("import entropy");
        assert_eq!(import.metadata.source, "explicit_entropy_file");
        assert_eq!(
            import.metadata.override_path,
            Some(export_path.display().to_string())
        );
        let saved = crate::load_config(config_path.as_path()).expect("load config");
        assert_eq!(
            saved.wallet_entropy_override_path.as_deref(),
            Some(export_path.as_path())
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn wallet_backup_export_encrypts_snapshot_and_updates_status() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_path = temp_dir.path().join("config.json");
        let backup_path = temp_dir.path().join("pylon-wallet-backup.json");
        let phrase = "legal winner thank year wave sausage worth useful legal winner thank yellow";
        let passphrase = "correct horse battery backup";
        let mut config = crate::default_config(temp_dir.path());
        config.wallet_network = "regtest".to_string();
        config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
        std::fs::write(config.identity_path.as_path(), format!("{phrase}\n"))
            .expect("write identity phrase");
        super::set_private_file_permissions(config.identity_path.as_path(), "identity mnemonic")
            .expect("set identity permissions");
        crate::save_config(config_path.as_path(), &config).expect("save config");

        let layout = super::ensure_wallet_storage_layout(&config).expect("storage layout");
        std::fs::write(
            layout.node_dir.join("channel_manager"),
            b"channel-secret-state",
        )
        .expect("write node state");
        std::fs::write(
            layout.sqlite_dir.join("payments.sqlite"),
            b"payment-secret-state",
        )
        .expect("write sqlite state");
        std::fs::write(
            layout.backup_staging_dir.join("monitor-1"),
            b"monitor-secret-state",
        )
        .expect("write monitor state");

        unsafe {
            std::env::set_var("PYLON_TEST_BACKUP_PASSPHRASE", passphrase);
        }
        let report = super::export_wallet_backup_report(
            config_path.as_path(),
            backup_path.as_path(),
            Some("PYLON_TEST_BACKUP_PASSPHRASE"),
            true,
        )
        .await
        .expect("export backup");
        assert_eq!(report.backup_status, "backup_current");
        assert!(report.manifest.identity_mnemonic_included);
        assert!(
            report
                .manifest
                .snapshot_kinds
                .contains(&"ldk_node_state".to_string())
        );
        assert!(
            report
                .manifest
                .snapshot_kinds
                .contains(&"ldk_sqlite_state".to_string())
        );
        assert!(
            report
                .manifest
                .snapshot_kinds
                .contains(&"ldk_backup_staging".to_string())
        );

        let backup_file = std::fs::read_to_string(backup_path.as_path()).expect("read backup");
        assert!(!backup_file.contains(phrase));
        assert!(!backup_file.contains("channel-secret-state"));
        assert!(!backup_file.contains("payment-secret-state"));
        assert!(!backup_file.contains("monitor-secret-state"));

        let inspect =
            super::inspect_wallet_backup_report(backup_path.as_path()).expect("inspect backup");
        assert!(inspect.valid);
        assert_eq!(
            inspect.manifest.plaintext_manifest_digest,
            report.manifest.plaintext_manifest_digest
        );

        let encrypted =
            super::read_encrypted_wallet_backup(backup_path.as_path()).expect("read encrypted");
        let plaintext =
            super::decrypt_wallet_backup_plaintext(&encrypted, passphrase).expect("decrypt backup");
        assert_eq!(plaintext.identity_mnemonic.as_deref(), Some(phrase));
        assert!(plaintext.ldk_storage_snapshot.iter().any(|file| {
            file.relative_path == "node/channel_manager"
                && hex::decode(file.content_hex.as_str())
                    .expect("content hex")
                    .as_slice()
                    == b"channel-secret-state"
        }));
        assert!(
            super::decrypt_wallet_backup_plaintext(&encrypted, "wrong horse battery backup")
                .is_err()
        );

        let mut corrupted = encrypted.clone();
        corrupted.ciphertext_hex.push_str("00");
        assert!(super::decrypt_wallet_backup_plaintext(&corrupted, passphrase).is_err());

        let status = load_wallet_status_report(config_path.as_path())
            .await
            .expect("status");
        let ldk_status = status.ldk_node.as_ref().expect("ldk status");
        assert_eq!(ldk_status.backup_status, "backup_current");
        assert!(!ldk_status.backup_stale);
        assert_eq!(
            ldk_status.last_backup_file_digest.as_deref(),
            Some(report.file_digest.as_str())
        );
        unsafe {
            std::env::remove_var("PYLON_TEST_BACKUP_PASSPHRASE");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn wallet_phrase_restore_recreates_identity_entropy_and_warns_about_limits() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_path = temp_dir.path().join("config.json");
        let phrase = "legal winner thank year wave sausage worth useful legal winner thank yellow";
        let mut config = crate::default_config(temp_dir.path());
        config.wallet_network = "mainnet".to_string();
        std::fs::write(config.identity_path.as_path(), "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\n")
            .expect("write old identity");
        super::set_private_file_permissions(config.identity_path.as_path(), "identity mnemonic")
            .expect("set identity permissions");
        crate::save_config(config_path.as_path(), &config).expect("save config");

        unsafe {
            std::env::set_var("PYLON_TEST_RESTORE_MNEMONIC", phrase);
        }
        let refused = run_wallet_command(
            config_path.as_path(),
            &WalletSubcommand::RestorePhrase {
                mnemonic_env: Some("PYLON_TEST_RESTORE_MNEMONIC".to_string()),
                mnemonic_file: None,
                wallet_network: Some("regtest".to_string()),
                yes: false,
                json: true,
            },
        )
        .await
        .expect_err("restore without --yes should fail");
        assert!(refused.to_string().contains("--yes"));
        let report = super::restore_wallet_phrase_report(
            config_path.as_path(),
            Some("PYLON_TEST_RESTORE_MNEMONIC"),
            None,
            Some("regtest"),
        )
        .await
        .expect("restore phrase");
        assert_eq!(report.mode, "phrase_only");
        assert_eq!(report.network, "regtest");
        assert!(report.identity_mnemonic_restored);
        assert!(report.recovery_mode.contains("onchain_rescan"));
        assert!(
            report
                .limitations
                .iter()
                .any(|value| value.contains("Does not restore Lightning channel monitors"))
        );
        assert_eq!(
            std::fs::read_to_string(config.identity_path.as_path())
                .expect("read identity")
                .trim(),
            phrase
        );
        let saved = crate::load_config(config_path.as_path()).expect("load config");
        assert_eq!(saved.wallet_network, "regtest");
        assert_eq!(saved.wallet_runtime_kind, PylonWalletRuntimeKind::LdkNode);
        let material = load_wallet_node_entropy_material(&saved).expect("derive entropy");
        assert_eq!(
            material.metadata.digest,
            "sha256:c68d883864e25cd51b513586b985900041d6631d82114bce7d0802848779a601"
        );
        let rendered = super::render_wallet_restore_report(&report);
        let json = serde_json::to_string_pretty(&report).expect("serialize report");
        assert!(!rendered.contains(phrase));
        assert!(!json.contains(phrase));
        unsafe {
            std::env::remove_var("PYLON_TEST_RESTORE_MNEMONIC");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn wallet_backup_restore_validates_network_passphrase_and_lock() {
        let source_dir = tempfile::tempdir().expect("source tempdir");
        let source_config_path = source_dir.path().join("config.json");
        let backup_path = source_dir.path().join("pylon-wallet-backup.json");
        let phrase = "legal winner thank year wave sausage worth useful legal winner thank yellow";
        let passphrase = "correct horse battery backup";
        let mut source_config = crate::default_config(source_dir.path());
        source_config.wallet_network = "regtest".to_string();
        source_config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
        std::fs::write(source_config.identity_path.as_path(), format!("{phrase}\n"))
            .expect("write source identity");
        super::set_private_file_permissions(
            source_config.identity_path.as_path(),
            "identity mnemonic",
        )
        .expect("set source identity permissions");
        crate::save_config(source_config_path.as_path(), &source_config).expect("save source");
        let source_layout =
            super::ensure_wallet_storage_layout(&source_config).expect("source layout");
        std::fs::write(
            source_layout.node_dir.join("channel_manager"),
            b"channel-secret-state",
        )
        .expect("write source node state");
        std::fs::write(
            source_layout.sqlite_dir.join("payments.sqlite"),
            b"payment-secret-state",
        )
        .expect("write source sqlite state");
        std::fs::write(
            source_layout.last_registration_path.as_path(),
            b"{\"schema_version\":1,\"kind\":\"pylon.wallet.last_registration\",\"status\":\"registered\"}\n",
        )
        .expect("write source registration");
        unsafe {
            std::env::set_var("PYLON_TEST_BACKUP_PASSPHRASE_RESTORE", passphrase);
        }
        super::export_wallet_backup_report(
            source_config_path.as_path(),
            backup_path.as_path(),
            Some("PYLON_TEST_BACKUP_PASSPHRASE_RESTORE"),
            true,
        )
        .await
        .expect("export source backup");

        let wrong_network_dir = tempfile::tempdir().expect("wrong network tempdir");
        let wrong_network_config_path = wrong_network_dir.path().join("config.json");
        let mut wrong_network_config = crate::default_config(wrong_network_dir.path());
        wrong_network_config.wallet_network = "bitcoin".to_string();
        wrong_network_config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
        std::fs::write(
            wrong_network_config.identity_path.as_path(),
            format!("{phrase}\n"),
        )
        .expect("write wrong-network identity");
        super::set_private_file_permissions(
            wrong_network_config.identity_path.as_path(),
            "identity mnemonic",
        )
        .expect("set wrong-network identity permissions");
        crate::save_config(wrong_network_config_path.as_path(), &wrong_network_config)
            .expect("save wrong-network config");
        assert!(
            super::restore_wallet_backup_report(
                wrong_network_config_path.as_path(),
                backup_path.as_path(),
                Some("PYLON_TEST_BACKUP_PASSPHRASE_RESTORE"),
            )
            .await
            .expect_err("wrong network should fail")
            .to_string()
            .contains("network mismatch")
        );

        unsafe {
            std::env::set_var(
                "PYLON_TEST_BACKUP_PASSPHRASE_RESTORE",
                "wrong horse battery backup",
            );
        }
        let wrong_pass_dir = tempfile::tempdir().expect("wrong pass tempdir");
        let wrong_pass_config_path = wrong_pass_dir.path().join("config.json");
        let mut wrong_pass_config = crate::default_config(wrong_pass_dir.path());
        wrong_pass_config.wallet_network = "regtest".to_string();
        wrong_pass_config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
        std::fs::write(
            wrong_pass_config.identity_path.as_path(),
            format!("{phrase}\n"),
        )
        .expect("write wrong-pass identity");
        super::set_private_file_permissions(
            wrong_pass_config.identity_path.as_path(),
            "identity mnemonic",
        )
        .expect("set wrong-pass identity permissions");
        crate::save_config(wrong_pass_config_path.as_path(), &wrong_pass_config)
            .expect("save wrong-pass config");
        assert!(
            super::restore_wallet_backup_report(
                wrong_pass_config_path.as_path(),
                backup_path.as_path(),
                Some("PYLON_TEST_BACKUP_PASSPHRASE_RESTORE"),
            )
            .await
            .expect_err("wrong passphrase should fail")
            .to_string()
            .contains("decrypt")
        );

        unsafe {
            std::env::set_var("PYLON_TEST_BACKUP_PASSPHRASE_RESTORE", passphrase);
        }
        let restore_dir = tempfile::tempdir().expect("restore tempdir");
        let restore_config_path = restore_dir.path().join("config.json");
        let mut restore_config = crate::default_config(restore_dir.path());
        restore_config.wallet_network = "regtest".to_string();
        restore_config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
        std::fs::write(
            restore_config.identity_path.as_path(),
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\n",
        )
        .expect("write restore identity");
        super::set_private_file_permissions(
            restore_config.identity_path.as_path(),
            "identity mnemonic",
        )
        .expect("set restore identity permissions");
        crate::save_config(restore_config_path.as_path(), &restore_config).expect("save restore");
        let restore_layout =
            super::ensure_wallet_storage_layout(&restore_config).expect("restore layout");
        let lock = super::acquire_wallet_storage_lock(&restore_layout).expect("active lock");
        assert!(
            super::restore_wallet_backup_report(
                restore_config_path.as_path(),
                backup_path.as_path(),
                Some("PYLON_TEST_BACKUP_PASSPHRASE_RESTORE"),
            )
            .await
            .expect_err("active lock should fail")
            .to_string()
            .contains("locked by active pid")
        );
        drop(lock);

        let report = super::restore_wallet_backup_report(
            restore_config_path.as_path(),
            backup_path.as_path(),
            Some("PYLON_TEST_BACKUP_PASSPHRASE_RESTORE"),
        )
        .await
        .expect("restore backup");
        assert_eq!(report.mode, "full_backup");
        assert!(report.identity_mnemonic_restored);
        assert!(report.restored_component_count >= 3);
        assert_eq!(
            std::fs::read_to_string(restore_config.identity_path.as_path())
                .expect("read restored identity")
                .trim(),
            phrase
        );
        assert_eq!(
            std::fs::read(restore_layout.node_dir.join("channel_manager"))
                .expect("read restored node"),
            b"channel-secret-state"
        );
        assert_eq!(
            std::fs::read(restore_layout.sqlite_dir.join("payments.sqlite"))
                .expect("read restored sqlite"),
            b"payment-secret-state"
        );
        assert!(
            std::fs::read_to_string(restore_layout.last_registration_path.as_path())
                .expect("read restored registration")
                .contains("\"registered\"")
        );
        let status = load_wallet_status_report(restore_config_path.as_path())
            .await
            .expect("status after restore");
        let ldk_status = status.ldk_node.as_ref().expect("ldk status");
        assert_eq!(ldk_status.backup_status, "backup_current");
        assert!(!ldk_status.backup_stale);

        let stale_backup_path = source_dir.path().join("pylon-wallet-stale-backup.json");
        let current_backup =
            super::read_encrypted_wallet_backup(backup_path.as_path()).expect("read backup");
        let mut stale_plaintext =
            super::decrypt_wallet_backup_plaintext(&current_backup, passphrase)
                .expect("decrypt current backup");
        stale_plaintext.exported_at_ms = 1;
        let stale_backup = super::encrypt_wallet_backup_plaintext(&stale_plaintext, passphrase)
            .expect("encrypt stale backup");
        super::write_encrypted_wallet_backup(stale_backup_path.as_path(), &stale_backup)
            .expect("write stale backup");
        let stale_restore_dir = tempfile::tempdir().expect("stale restore tempdir");
        let stale_restore_config_path = stale_restore_dir.path().join("config.json");
        let mut stale_restore_config = crate::default_config(stale_restore_dir.path());
        stale_restore_config.wallet_network = "regtest".to_string();
        stale_restore_config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
        std::fs::write(
            stale_restore_config.identity_path.as_path(),
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\n",
        )
        .expect("write stale restore identity");
        super::set_private_file_permissions(
            stale_restore_config.identity_path.as_path(),
            "identity mnemonic",
        )
        .expect("set stale restore identity permissions");
        crate::save_config(stale_restore_config_path.as_path(), &stale_restore_config)
            .expect("save stale restore");
        super::restore_wallet_backup_report(
            stale_restore_config_path.as_path(),
            stale_backup_path.as_path(),
            Some("PYLON_TEST_BACKUP_PASSPHRASE_RESTORE"),
        )
        .await
        .expect("restore stale backup");
        let stale_status = load_wallet_status_report(stale_restore_config_path.as_path())
            .await
            .expect("stale status after restore");
        let stale_ldk_status = stale_status.ldk_node.as_ref().expect("stale ldk status");
        assert_eq!(stale_ldk_status.backup_status, "backup_stale");
        assert!(stale_ldk_status.backup_stale);
        unsafe {
            std::env::remove_var("PYLON_TEST_BACKUP_PASSPHRASE_RESTORE");
        }
    }

    #[test]
    fn wallet_storage_layout_creates_private_ldk_paths() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config = crate::default_config(temp_dir.path());
        let layout = super::ensure_wallet_storage_layout(&config).expect("storage layout");
        let report = super::wallet_storage_layout_report(&layout);

        assert!(layout.ldk_dir.ends_with("ldk"));
        assert!(layout.node_dir.exists());
        assert!(layout.sqlite_dir.exists());
        assert!(layout.backup_staging_dir.exists());
        assert!(layout.backup_manifest_path.exists());
        assert!(layout.last_registration_path.exists());
        assert_eq!(report.schema_version, 1);
        assert!(report.lock_path.ends_with("wallet-lock"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir_mode = std::fs::metadata(layout.ldk_dir.as_path())
                .expect("ldk dir metadata")
                .permissions()
                .mode()
                & 0o777;
            let file_mode = std::fs::metadata(layout.backup_manifest_path.as_path())
                .expect("manifest metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(dir_mode, 0o700);
            assert_eq!(file_mode, 0o600);
        }
    }

    #[test]
    fn wallet_storage_lock_rejects_second_writer_and_releases_on_drop() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config = crate::default_config(temp_dir.path());
        let layout = super::ensure_wallet_storage_layout(&config).expect("storage layout");

        let lock = super::acquire_wallet_storage_lock(&layout).expect("first lock");
        let second = super::acquire_wallet_storage_lock(&layout)
            .expect_err("second active writer should fail");
        assert!(second.to_string().contains("wallet storage is locked"));

        drop(lock);
        super::acquire_wallet_storage_lock(&layout).expect("lock should release on drop");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ldk_node_runtime_holds_lock_until_stop() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_path = temp_dir.path().join("config.json");
        let mut config = crate::default_config(temp_dir.path());
        config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
        config.wallet_network = "regtest".to_string();
        std::fs::write(
            config.identity_path.as_path(),
            "legal winner thank year wave sausage worth useful legal winner thank yellow\n",
        )
        .expect("write identity phrase");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                config.identity_path.as_path(),
                std::fs::Permissions::from_mode(0o600),
            )
            .expect("set identity permissions");
        }
        crate::save_config(config_path.as_path(), &config).expect("save config");

        let first = super::prepare_wallet_context(config_path.as_path()).expect("first context");
        first.runtime.start().expect("first runtime start");
        let second = super::prepare_wallet_context(config_path.as_path()).expect("second context");
        let error = second
            .runtime
            .start()
            .expect_err("second active runtime should fail");
        assert!(error.to_string().contains("wallet storage is locked"));

        first.runtime.stop().expect("first runtime stop");
        second
            .runtime
            .start()
            .expect("second runtime should start after stop");
        second.runtime.stop().expect("second runtime stop");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn stale_wallet_lock_can_be_inspected_and_cleared_explicitly() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_path = temp_dir.path().join("config.json");
        let config = crate::default_config(temp_dir.path());
        crate::save_config(config_path.as_path(), &config).expect("save config");
        let layout = super::ensure_wallet_storage_layout(&config).expect("storage layout");
        let stale_owner = WalletLockOwner {
            pid: 0,
            machine_id: super::local_machine_id(),
            created_at_ms: 1,
        };
        std::fs::write(
            layout.lock_path.as_path(),
            serde_json::to_string(&stale_owner).expect("serialize stale owner"),
        )
        .expect("write stale lock");

        let status = super::inspect_wallet_lock_report(config_path.as_path())
            .await
            .expect("inspect lock");
        assert!(status.locked);
        assert!(status.stale);

        let cleared = super::clear_wallet_lock_report(config_path.as_path())
            .await
            .expect("clear stale lock");
        assert!(cleared.cleared);
        assert!(!layout.lock_path.exists());
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "current_thread")]
    async fn ldk_node_runtime_refuses_world_readable_recovery_material() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_path = temp_dir.path().join("config.json");
        let mut config = crate::default_config(temp_dir.path());
        config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
        std::fs::write(
            config.identity_path.as_path(),
            "legal winner thank year wave sausage worth useful legal winner thank yellow\n",
        )
        .expect("write identity phrase");
        std::fs::set_permissions(
            config.identity_path.as_path(),
            std::fs::Permissions::from_mode(0o644),
        )
        .expect("set world-readable identity permissions");
        crate::save_config(config_path.as_path(), &config).expect("save config");

        let error = load_wallet_status_report(config_path.as_path())
            .await
            .expect_err("world-readable identity should fail");
        assert!(error.to_string().contains("must not be readable"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mock_runtime_returns_deterministic_wallet_reports() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_path = temp_dir.path().join("config.json");
        let mut config = crate::default_config(temp_dir.path());
        config.wallet_runtime_kind = PylonWalletRuntimeKind::Mock;
        config.wallet_network = "regtest".to_string();
        crate::save_config(config_path.as_path(), &config).expect("save config");

        let status = load_wallet_status_report(config_path.as_path())
            .await
            .expect("mock status");
        assert_eq!(status.runtime.runtime_kind, PylonWalletRuntimeKind::Mock);
        assert_eq!(status.runtime_status, "connected");
        assert_eq!(status.balance.total_sats, 1_000);

        let address = create_wallet_address_report(config_path.as_path())
            .await
            .expect("mock address");
        assert_eq!(address.runtime.runtime_kind, PylonWalletRuntimeKind::Mock);
        assert!(address.bitcoin_address.starts_with("bcrt1pmock"));

        let invoice =
            create_wallet_invoice_report(config_path.as_path(), 42, Some("test".to_string()), None)
                .await
                .expect("mock invoice");
        assert_eq!(invoice.invoice.payment_request, "lnbc42mockpyloninvoice");

        let payment = pay_wallet_invoice_report(config_path.as_path(), "lnbc1mockpay", Some(21))
            .await
            .expect("mock payment");
        assert_eq!(payment.payment_id, "mock-payment-21");
        assert_eq!(payment.post_balance.total_sats, 979);

        let json_payment = run_wallet_command(
            config_path.as_path(),
            &WalletSubcommand::Pay {
                payment_request: "lnbc1mockjsonpay".to_string(),
                amount_sats: Some(8),
                yes: false,
                json: true,
            },
        )
        .await
        .expect("mock json pay");
        assert!(json_payment.contains("\"payment_id\": \"mock-payment-8\""));
        let interactive_error = run_wallet_command(
            config_path.as_path(),
            &WalletSubcommand::Pay {
                payment_request: "lnbc1mockinteractive".to_string(),
                amount_sats: Some(8),
                yes: false,
                json: false,
            },
        )
        .await
        .expect_err("interactive pay should require confirmation");
        assert!(interactive_error.to_string().contains("--yes or --json"));

        let history = load_wallet_history_report(config_path.as_path(), Some(10))
            .await
            .expect("mock history");
        assert_eq!(history.runtime.runtime_kind, PylonWalletRuntimeKind::Mock);
        assert!(!history.payments.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ldk_node_runtime_builds_stable_node_without_chain_source() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_path = temp_dir.path().join("config.json");
        let mut config = crate::default_config(temp_dir.path());
        config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
        config.wallet_network = "regtest".to_string();
        crate::save_config(config_path.as_path(), &config).expect("save config");

        let status = load_wallet_status_report(config_path.as_path())
            .await
            .expect("ldk_node status");
        assert_eq!(status.runtime.runtime_kind, PylonWalletRuntimeKind::LdkNode);
        assert_eq!(status.runtime_status, "configured");
        let first_node_id = status
            .ldk_node
            .as_ref()
            .and_then(|node| node.node_id.clone())
            .expect("node id");
        assert!(
            status
                .runtime_detail
                .as_deref()
                .unwrap_or_default()
                .contains("no chain source")
        );
        assert_eq!(
            status
                .ldk_node
                .as_ref()
                .map(|node| node.chain_source_kind.as_str()),
            Some("none")
        );
        let node_status = status.ldk_node.as_ref().expect("ldk node status");
        assert_eq!(node_status.backup_status, "backup_missing");
        assert!(node_status.backup_manifest_present);
        assert_eq!(node_status.backup_artifact_count, 0);
        assert!(node_status.backup_stale);
        assert!(node_status.last_backup_exported_at_ms.is_none());
        assert_eq!(node_status.storage_schema_version, 1);
        assert!(node_status.storage_generation.starts_with("sha256:"));
        assert_eq!(status.balance.onchain_sats, 0);
        assert_eq!(status.balance.spendable_onchain_sats, 0);
        assert_eq!(status.balance.anchor_reserve_sats, 0);

        let restarted = load_wallet_status_report(config_path.as_path())
            .await
            .expect("ldk_node restart status");
        assert_eq!(
            restarted
                .ldk_node
                .as_ref()
                .and_then(|node| node.node_id.clone()),
            Some(first_node_id)
        );

        let address = create_wallet_address_report(config_path.as_path())
            .await
            .expect("ldk_node address");
        let parsed = address
            .bitcoin_address
            .parse::<ldk_node::bitcoin::Address<ldk_node::bitcoin::address::NetworkUnchecked>>()
            .expect("parse address");
        assert!(parsed.is_valid_for_network(ldk_node::bitcoin::Network::Regtest));

        let status_json = run_wallet_command(
            config_path.as_path(),
            &WalletSubcommand::Status { json: true },
        )
        .await
        .expect("status json");
        assert!(status_json.contains("\"runtime_kind\": \"ldk_node\""));
        assert!(status_json.contains("\"backup_status\": \"backup_missing\""));
        assert!(status_json.contains("\"storage_generation\": \"sha256:"));
        assert!(!status_json.contains("legal winner thank"));

        let sync_json = run_wallet_command(
            config_path.as_path(),
            &WalletSubcommand::Sync { json: true },
        )
        .await
        .expect("sync json");
        assert!(sync_json.contains("\"runtime_status\": \"configured\""));

        let balance_json = run_wallet_command(
            config_path.as_path(),
            &WalletSubcommand::Balance { json: true },
        )
        .await
        .expect("balance json");
        assert!(balance_json.contains("\"onchain_sats\": 0"));
        assert!(balance_json.contains("\"spendable_onchain_sats\": 0"));

        let address_json = run_wallet_command(
            config_path.as_path(),
            &WalletSubcommand::Address { json: true },
        )
        .await
        .expect("address json");
        let address_value: serde_json::Value =
            serde_json::from_str(address_json.as_str()).expect("address json should parse");
        let json_address = address_value
            .get("bitcoin_address")
            .and_then(serde_json::Value::as_str)
            .expect("json address");
        let parsed_json_address = json_address
            .parse::<ldk_node::bitcoin::Address<ldk_node::bitcoin::address::NetworkUnchecked>>()
            .expect("parse json address");
        assert!(parsed_json_address.is_valid_for_network(ldk_node::bitcoin::Network::Regtest));

        let invoice = create_wallet_invoice_report(
            config_path.as_path(),
            42,
            Some("pylon receive".to_string()),
            Some(120),
        )
        .await
        .expect("ldk_node BOLT11 invoice");
        assert_eq!(invoice.invoice.amount_sats, 42);
        assert_eq!(invoice.invoice.runtime_kind.as_deref(), Some("ldk_node"));
        assert_eq!(
            invoice.invoice.description.as_deref(),
            Some("pylon receive")
        );
        assert!(invoice.invoice.payment_hash.is_some());
        assert!(invoice.invoice.expires_at_ms.is_some());
        let parsed_invoice = invoice
            .invoice
            .payment_request
            .parse::<ldk_node::lightning_invoice::Bolt11Invoice>()
            .expect("parse bolt11 invoice");
        assert_eq!(
            parsed_invoice.currency(),
            ldk_node::lightning_invoice::Currency::Regtest
        );
        assert_eq!(parsed_invoice.amount_milli_satoshis(), Some(42_000));
        let parsed_payment_hash = parsed_invoice.payment_hash().to_string();
        assert_eq!(
            invoice.invoice.payment_hash.as_deref(),
            Some(parsed_payment_hash.as_str())
        );
        let ledger = crate::load_ledger(config_path.as_path()).expect("ledger");
        assert!(ledger.wallet.invoices.iter().any(|entry| {
            entry.payment_hash == invoice.invoice.payment_hash
                && entry.runtime_kind.as_deref() == Some("ldk_node")
        }));

        let invoice_json = run_wallet_command(
            config_path.as_path(),
            &WalletSubcommand::Invoice {
                amount_sats: 21,
                description: Some("json receive".to_string()),
                expiry_seconds: Some(60),
                json: true,
            },
        )
        .await
        .expect("invoice json");
        assert!(invoice_json.contains("\"payment_hash\":"));
        assert!(invoice_json.contains("\"runtime_kind\": \"ldk_node\""));
        assert!(!invoice_json.contains("preimage"));

        match create_wallet_offer_report(
            config_path.as_path(),
            Some(42),
            Some("pylon offer".to_string()),
            Some(120),
        )
        .await
        {
            Ok(offer) => {
                assert!(offer.offer.starts_with("lno"));
                assert_eq!(offer.amount_sats, Some(42));
                assert_eq!(offer.description.as_deref(), Some("pylon offer"));
            }
            Err(error) => {
                let detail = error.to_string();
                assert!(
                    detail.contains("bolt12_offer_unavailable")
                        || detail.contains("Failed to create offer")
                );
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ldk_node_failed_bolt11_send_records_wallet_receipt() {
        let receiver_dir = tempfile::tempdir().expect("receiver tempdir");
        let receiver_config_path = receiver_dir.path().join("config.json");
        let mut receiver_config = crate::default_config(receiver_dir.path());
        receiver_config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
        receiver_config.wallet_network = "regtest".to_string();
        crate::save_config(receiver_config_path.as_path(), &receiver_config).expect("save config");
        let invoice = create_wallet_invoice_report(receiver_config_path.as_path(), 21, None, None)
            .await
            .expect("receiver invoice");

        let payer_dir = tempfile::tempdir().expect("payer tempdir");
        let payer_config_path = payer_dir.path().join("config.json");
        let mut payer_config = crate::default_config(payer_dir.path());
        payer_config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
        payer_config.wallet_network = "regtest".to_string();
        crate::save_config(payer_config_path.as_path(), &payer_config).expect("save config");

        let error = pay_wallet_invoice_report(
            payer_config_path.as_path(),
            invoice.invoice.payment_request.as_str(),
            None,
        )
        .await
        .expect_err("send should fail without a running chain source");
        assert!(error.to_string().contains("ldk_wallet_send_failed"));

        let ledger = crate::load_ledger(payer_config_path.as_path()).expect("payer ledger");
        let payment = ledger
            .wallet
            .payments
            .iter()
            .find(|payment| payment.status == "failed")
            .expect("failed payment receipt");
        assert_eq!(payment.method, "bolt11");
        assert_eq!(payment.amount_sats, 21);
        assert_eq!(
            payment.invoice.as_deref(),
            Some(invoice.invoice.payment_request.as_str())
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ldk_node_wallet_history_projects_ldk_payments_idempotently() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_path = temp_dir.path().join("config.json");
        let mut config = crate::default_config(temp_dir.path());
        config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
        config.wallet_network = "regtest".to_string();
        crate::save_config(config_path.as_path(), &config).expect("save config");

        let invoice = create_wallet_invoice_report(config_path.as_path(), 21, None, None)
            .await
            .expect("wallet invoice");
        let payment_hash = invoice
            .invoice
            .payment_hash
            .clone()
            .expect("invoice payment hash");

        let first_history = load_wallet_history_report(config_path.as_path(), None)
            .await
            .expect("first history");
        let projected = first_history
            .payments
            .iter()
            .find(|payment| payment.payment_hash.as_deref() == Some(payment_hash.as_str()))
            .expect("ldk payment projection");
        assert_eq!(projected.direction, "receive");
        assert_eq!(projected.method, "bolt11");
        assert_eq!(projected.status, "pending");
        assert_eq!(projected.amount_sats, 21);
        assert!(projected.receipt_id.is_some());

        let ledger_after_first =
            crate::load_ledger(config_path.as_path()).expect("ledger after first history");
        let payment_count = ledger_after_first.wallet.payments.len();
        let receipt_count = ledger_after_first.wallet.receipts.len();
        assert!(ledger_after_first.wallet.receipts.iter().any(|receipt| {
            receipt.payment_hash.as_deref() == Some(payment_hash.as_str())
                && receipt.receipt_type == "wallet.payment.receive.v1"
        }));

        let _second_history = load_wallet_history_report(config_path.as_path(), None)
            .await
            .expect("second history");
        let ledger_after_second =
            crate::load_ledger(config_path.as_path()).expect("ledger after second history");
        assert_eq!(ledger_after_second.wallet.payments.len(), payment_count);
        assert_eq!(ledger_after_second.wallet.receipts.len(), receipt_count);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ldk_node_onchain_withdrawal_checks_spendable_balance_and_records_wallet_receipt() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_path = temp_dir.path().join("config.json");
        let mut config = crate::default_config(temp_dir.path());
        config.wallet_runtime_kind = PylonWalletRuntimeKind::LdkNode;
        config.wallet_network = "regtest".to_string();
        crate::save_config(config_path.as_path(), &config).expect("save config");
        let address = create_wallet_address_report(config_path.as_path())
            .await
            .expect("wallet address")
            .bitcoin_address;

        let error = pay_wallet_invoice_report(config_path.as_path(), address.as_str(), Some(21))
            .await
            .expect_err("withdrawal should fail without spendable funds");
        assert!(
            error
                .to_string()
                .contains("insufficient_spendable_onchain_balance")
        );

        let ledger = crate::load_ledger(config_path.as_path()).expect("ledger");
        let payment = ledger
            .wallet
            .payments
            .iter()
            .find(|payment| payment.status == "failed")
            .expect("failed payment receipt");
        assert_eq!(payment.method, "onchain");
        assert_eq!(payment.amount_sats, 21);
        assert_eq!(payment.fees_sats, 0);
    }
}
