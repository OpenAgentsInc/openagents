use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow, bail};
use bip39::{Language, Mnemonic};
use hmac::{Hmac, Mac};
use ldk_node::Builder as LdkNodeBuilder;
use ldk_node::bitcoin::Network as LdkBitcoinNetwork;
use ldk_node::lightning_invoice::{Bolt11InvoiceDescription, Description};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    PylonLedger, PylonWalletCreditSummary, PylonWalletInvoiceRecord, PylonWalletPaymentRecord,
    ensure_local_setup, load_ledger, mutate_ledger, now_epoch_ms,
};

type HmacSha256 = Hmac<Sha256>;

const LDK_EXTERNAL_WALLET_DETAIL: &str = "Pylon uses an external LDK-compatible payout destination. Configure payout_destination for earnings.";
const MOCK_WALLET_DETAIL: &str = "Pylon is using the deterministic mock wallet runtime for tests.";
const LDK_NODE_UNAVAILABLE_DETAIL: &str = "Pylon ldk_node payment APIs are not wired yet; this issue only initializes the local node runtime.";
const WALLET_NODE_ENTROPY_DERIVATION_VERSION: &str = "pylon-ldk-node-entropy-v1";
const WALLET_NODE_ENTROPY_LABEL_PREFIX: &str = "openagents-pylon/ldk-node/v1";
const WALLET_NODE_ENTROPY_HKDF_SALT: &[u8] = b"openagents-pylon/ldk-node/node-entropy";
const WALLET_STORAGE_SCHEMA_VERSION: u32 = 1;
const DEFAULT_BOLT11_EXPIRY_SECONDS: u32 = 3_600;
const DEFAULT_RECEIVE_DESCRIPTION: &str = "OpenAgents Pylon receive";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonWalletRuntimeKind {
    #[default]
    ExternalTarget,
    Mock,
    LdkNode,
}

impl PylonWalletRuntimeKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::ExternalTarget => "external_target",
            Self::Mock => "mock",
            Self::LdkNode => "ldk_node",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "external_target" | "external" | "ldk_external" => Ok(Self::ExternalTarget),
            "mock" => Ok(Self::Mock),
            "ldk_node" | "ldknode" => Ok(Self::LdkNode),
            other => bail!(
                "unsupported wallet_runtime_kind '{other}'; expected external_target, mock, or ldk_node"
            ),
        }
    }
}

impl std::fmt::Display for PylonWalletRuntimeKind {
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
        json: bool,
    },
    History {
        limit: Option<u32>,
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
    pub network: String,
    pub identity_path: String,
    pub storage_dir: String,
    pub api_key_env: Option<String>,
    pub api_key_source: String,
    pub node_entropy: WalletNodeEntropyMetadata,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
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
    pub inbound_sats: u64,
    pub outbound_sats: u64,
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
    pub payments: Vec<PylonWalletPaymentRecord>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletCreditSummaryReport {
    pub runtime: WalletRuntimeSurface,
    pub credits: PylonWalletCreditSummary,
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
        }
    }

    fn start(&self) -> Result<()> {
        match self {
            Self::ExternalTarget(runtime) => runtime.start(),
            Self::Mock(runtime) => runtime.start(),
            Self::LdkNode(runtime) => runtime.start(),
        }
    }

    fn stop(&self) -> Result<()> {
        match self {
            Self::ExternalTarget(runtime) => runtime.stop(),
            Self::Mock(runtime) => runtime.stop(),
            Self::LdkNode(runtime) => runtime.stop(),
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
        }
    }

    fn balance(&self, ledger: &PylonLedger) -> Result<WalletBalanceSnapshot> {
        match self {
            Self::ExternalTarget(runtime) => runtime.balance(ledger),
            Self::Mock(runtime) => runtime.balance(ledger),
            Self::LdkNode(runtime) => runtime.balance(ledger),
        }
    }

    fn address(&self) -> Result<WalletAddressReport> {
        match self {
            Self::ExternalTarget(runtime) => runtime.address(),
            Self::Mock(runtime) => runtime.address(),
            Self::LdkNode(runtime) => runtime.address(),
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
        }
    }

    fn pay(&self, payment_request: &str, amount_sats: Option<u64>) -> Result<WalletPayReport> {
        match self {
            Self::ExternalTarget(runtime) => runtime.pay(payment_request, amount_sats),
            Self::Mock(runtime) => runtime.pay(payment_request, amount_sats),
            Self::LdkNode(runtime) => runtime.pay(payment_request, amount_sats),
        }
    }

    fn withdraw(&self, payment_request: &str, amount_sats: Option<u64>) -> Result<WalletPayReport> {
        match self {
            Self::ExternalTarget(runtime) => runtime.withdraw(payment_request, amount_sats),
            Self::Mock(runtime) => runtime.withdraw(payment_request, amount_sats),
            Self::LdkNode(runtime) => runtime.withdraw(payment_request, amount_sats),
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
        }
    }

    fn list_channels(&self) -> Result<Vec<PylonWalletChannelRecord>> {
        match self {
            Self::ExternalTarget(runtime) => runtime.list_channels(),
            Self::Mock(runtime) => runtime.list_channels(),
            Self::LdkNode(runtime) => runtime.list_channels(),
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
        Ok(WalletStatusReport {
            runtime: self.surface.clone(),
            runtime_status: "external_target".to_string(),
            runtime_detail: Some(LDK_EXTERNAL_WALLET_DETAIL.to_string()),
            ldk_node: None,
            balance: self.balance(ledger)?,
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
        Ok(WalletStatusReport {
            runtime: self.surface.clone(),
            runtime_status: "connected".to_string(),
            runtime_detail: Some(MOCK_WALLET_DETAIL.to_string()),
            ldk_node: None,
            balance: self.balance(ledger)?,
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
            created_at_ms: now,
            updated_at_ms: now,
        }])
    }

    fn list_channels(&self) -> Result<Vec<PylonWalletChannelRecord>> {
        Ok(vec![PylonWalletChannelRecord {
            channel_id: "mock-channel-1".to_string(),
            status: "ready".to_string(),
            inbound_sats: 500,
            outbound_sats: 500,
        }])
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
        Ok(WalletStatusReport {
            runtime: self.surface.clone(),
            runtime_status: runtime_status.to_string(),
            runtime_detail,
            ldk_node: Some(ldk_node),
            balance: self.balance(ledger)?,
            recent_payments: ledger_payments(ledger, include_recent_payments.then_some(10)),
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
        let _ = (payment_request, amount_sats);
        bail!("{LDK_NODE_UNAVAILABLE_DETAIL}")
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
                inbound_sats: channel.inbound_capacity_msat / 1000,
                outbound_sats: channel.outbound_capacity_msat / 1000,
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
        let backup_manifest_present = layout.backup_manifest_path.is_file();
        let backup_artifact_count = wallet_backup_artifact_count(&layout)?;
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
            chain_source_url: self.configured_chain_source_url(),
            rgs_url: self.settings.rgs_url.clone(),
            backup_status: wallet_backup_status(backup_manifest_present, backup_artifact_count),
            backup_manifest_present,
            backup_artifact_count,
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
            last_error: self.last_error()?,
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

fn wallet_backup_status(manifest_present: bool, artifact_count: usize) -> String {
    if artifact_count > 0 {
        "backup_artifacts_present".to_string()
    } else if manifest_present {
        "manifest_ready".to_string()
    } else {
        "missing_manifest".to_string()
    }
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
            json,
        } => {
            let report =
                pay_wallet_invoice_report(config_path, payment_request.as_str(), *amount_sats)
                    .await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_pay_report(&report))
        }
        WalletSubcommand::History { limit, json } => {
            let report = load_wallet_history_report(config_path, *limit).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_history_report(&report))
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
        "entropy" => parse_wallet_entropy_command(args, start_index + 2),
        "lock" => parse_wallet_lock_command(args, start_index + 2),
        other => bail!("unsupported wallet subcommand '{other}'"),
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
            sync_wallet_error(config_path, context.runtime.surface(), error.to_string())?;
            Err(error)
        }
    }
}

pub async fn load_wallet_history_report(
    config_path: &Path,
    limit: Option<u32>,
) -> Result<WalletHistoryReport> {
    let context = prepare_wallet_context(config_path)?;
    let ledger = load_ledger(config_path)?;
    let records = context.runtime.list_payments(&ledger, limit)?;
    Ok(WalletHistoryReport {
        runtime: context.runtime.surface().clone(),
        payments: records,
    })
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
    ];
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

pub fn render_wallet_address_report(report: &WalletAddressReport) -> String {
    let payout_destination = report
        .payout_destination
        .as_deref()
        .unwrap_or("not_configured");
    [
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("network: {}", report.runtime.network),
        format!("payout_destination: {payout_destination}"),
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

pub fn render_wallet_history_report(report: &WalletHistoryReport) -> String {
    let mut lines = vec![
        format!("runtime_kind: {}", report.runtime.runtime_kind),
        format!("network: {}", report.runtime.network),
        format!("payments: {}", report.payments.len()),
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
    let mut extract =
        HmacSha256::new_from_slice(WALLET_NODE_ENTROPY_HKDF_SALT).expect("HKDF salt is valid");
    extract.update(input_key_material);
    let pseudorandom_key = extract.finalize().into_bytes();

    let mut output = [0u8; 64];
    let mut previous = Vec::<u8>::new();
    let mut written = 0usize;
    for counter in 1u8..=2 {
        let mut expand = HmacSha256::new_from_slice(pseudorandom_key.as_slice())
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
            "{{\"schema_version\":{WALLET_STORAGE_SCHEMA_VERSION},\"kind\":\"pylon.wallet.backup_manifest\"}}\n"
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
    let surface = WalletRuntimeSurface {
        runtime_kind,
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
                payout_destination: config.payout_destination.clone(),
            })
        }
        PylonWalletRuntimeKind::Mock => SelectedPylonWalletRuntime::Mock(MockPylonWalletRuntime {
            surface,
            payout_destination: config.payout_destination.clone(),
        }),
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
    mutate_ledger(config_path, |ledger| {
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
        PylonWalletRuntime, PylonWalletRuntimeKind, WalletLockOwner, WalletSubcommand,
        compute_wallet_credit_summary, create_wallet_address_report, create_wallet_invoice_report,
        create_wallet_offer_report, load_wallet_history_report, load_wallet_node_entropy_material,
        load_wallet_status_report, parse_wallet_command, pay_wallet_invoice_report,
        render_wallet_entropy_report, render_wallet_status_report, run_wallet_command,
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
                    String::from("--json"),
                ],
                0,
            )
            .expect("wallet pay should parse"),
            WalletSubcommand::Pay {
                payment_request: String::from("lnbc1example"),
                amount_sats: Some(8),
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
        config.payout_destination = Some("lno1externaltarget".to_string());
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
        assert_eq!(node_status.backup_status, "manifest_ready");
        assert!(node_status.backup_manifest_present);
        assert_eq!(node_status.backup_artifact_count, 0);
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
        assert!(status_json.contains("\"backup_status\": \"manifest_ready\""));
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
}
