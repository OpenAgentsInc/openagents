mod ledger;
mod nip90_runtime;
mod wallet_runtime;

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use bip39::{Language, Mnemonic};
use nostr::{NostrIdentity, derive_keypair, load_identity_from_path};
use nostr_client::{
    ConnectionState, RelayAuthIdentity, RelayConfig, RelayConnection, RelayMessage,
};
use openagents_provider_substrate::{
    ProviderAdapterTrainingContributorAvailability, ProviderAdminConfig, ProviderAdminRuntime,
    ProviderAdminUpdate, ProviderAdvertisedProduct, ProviderAppleAdapterHostingAvailability,
    ProviderAvailability, ProviderBackendHealth, ProviderControlAction, ProviderDesiredMode,
    ProviderDiagnosticSummary, ProviderEarningsSummary, ProviderFailureClass, ProviderHealthEvent,
    ProviderIdentityMetadata, ProviderInventoryControls, ProviderInventoryRow, ProviderJsonEntry,
    ProviderMode, ProviderPersistedSnapshot, ProviderPersistenceStore,
    ProviderPooledInferenceAvailability, ProviderReceiptSummary, ProviderRecentJob,
    ProviderRuntimeStatusSnapshot, ProviderSandboxDetectionConfig, ProviderSandboxProfile,
    ProviderSandboxProfileSpec, ProviderSandboxRuntimeHealth, ProviderSnapshotParts,
    ProviderStatusResponse, assemble_provider_persisted_snapshot, derive_provider_products,
    detect_sandbox_supply, provider_runtime_state_label, validate_provider_control_action,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::process::Command as TokioCommand;

pub use ledger::{
    PylonLedger, PylonLedgerAnnouncement, PylonLedgerJob, PylonLedgerPayout, PylonLedgerSummary,
    PylonRelayActivity, PylonRelayConfigSnapshot, PylonRelayState, PylonSettlementRecord,
    PylonWalletInvoiceRecord, PylonWalletLedger, PylonWalletPaymentRecord, default_ledger_path,
    ensure_local_ledger, load_ledger, load_ledger_summary, mutate_ledger, save_ledger,
};
pub use nip90_runtime::{
    AnnouncementAction, AnnouncementReport, BuyerJobHistoryReport, BuyerJobPaymentReport,
    BuyerJobReplayReport, BuyerJobSubmitReport, BuyerJobSubmitRequest, BuyerJobWatchEntry,
    BuyerJobWatchReport, BuyerPaymentPolicyMode, BuyerPaymentPolicyReport, ProviderIntakeReport,
    ProviderRunReport, apply_buyer_payment_policy, approve_buyer_job_payment,
    deny_buyer_job_payment, load_announcement_report, load_buyer_job_history,
    load_buyer_job_replay, publish_announcement_report, render_announcement_report,
    render_buyer_job_history_report, render_buyer_job_payment_report,
    render_buyer_job_replay_report, render_buyer_job_submit_report, render_buyer_job_watch_report,
    render_buyer_payment_policy_report, render_provider_intake_report, render_provider_run_report,
    run_provider_requests, scan_provider_requests, submit_buyer_job, watch_buyer_jobs,
};
pub use wallet_runtime::{
    WalletAddressReport, WalletBalanceSnapshot, WalletHistoryReport, WalletInvoiceReport,
    WalletPayReport, WalletRuntimeSurface, WalletStatusReport, WalletSubcommand,
    create_wallet_address_report, create_wallet_invoice_report, load_wallet_history_report,
    load_wallet_status_report, parse_wallet_command, pay_wallet_invoice_report,
    render_wallet_address_report, render_wallet_balance_report, render_wallet_history_report,
    render_wallet_invoice_report, render_wallet_pay_report, render_wallet_status_report,
    run_wallet_command,
};

pub const ENV_PYLON_HOME: &str = "OPENAGENTS_PYLON_HOME";
pub const ENV_PYLON_CONFIG_PATH: &str = "OPENAGENTS_PYLON_CONFIG_PATH";
pub const ENV_PSIONIC_REPO: &str = "OPENAGENTS_PSIONIC_REPO";
const DEFAULT_PROVIDER_PRESENCE_HEARTBEAT_INTERVAL_MS: u64 = 5_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonConfig {
    pub schema_version: u32,
    pub node_label: String,
    pub payout_destination: Option<String>,
    pub identity_path: PathBuf,
    pub admin_db_path: PathBuf,
    pub admin_listen_addr: String,
    #[serde(default = "default_nexus_control_base_url")]
    pub nexus_control_base_url: String,
    #[serde(default = "default_relay_urls")]
    pub relay_urls: Vec<String>,
    #[serde(default = "default_relay_connect_timeout_seconds")]
    pub relay_connect_timeout_seconds: u64,
    #[serde(default = "default_relay_auth_enabled")]
    pub relay_auth_enabled: bool,
    #[serde(default = "default_wallet_network")]
    pub wallet_network: String,
    #[serde(default = "default_wallet_api_key_env")]
    pub wallet_api_key_env: Option<String>,
    #[serde(default = "default_buyer_auto_pay_enabled")]
    pub buyer_auto_pay_enabled: bool,
    pub wallet_storage_dir: PathBuf,
    #[serde(alias = "ollama_base_url")]
    pub local_gemma_base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple_fm_base_url: Option<String>,
    pub inventory_controls: ProviderInventoryControls,
    pub declared_sandbox_profiles: Vec<ProviderSandboxProfileSpec>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PylonPublicInventoryControls {
    local_gemma_inference_enabled: bool,
    local_gemma_embeddings_enabled: bool,
    sandbox_container_exec_enabled: bool,
    sandbox_python_exec_enabled: bool,
    sandbox_node_exec_enabled: bool,
    sandbox_posix_exec_enabled: bool,
}

impl From<&ProviderInventoryControls> for PylonPublicInventoryControls {
    fn from(value: &ProviderInventoryControls) -> Self {
        Self {
            local_gemma_inference_enabled: value.local_gemma_inference_enabled,
            local_gemma_embeddings_enabled: value.local_gemma_embeddings_enabled,
            sandbox_container_exec_enabled: value.sandbox_container_exec_enabled,
            sandbox_python_exec_enabled: value.sandbox_python_exec_enabled,
            sandbox_node_exec_enabled: value.sandbox_node_exec_enabled,
            sandbox_posix_exec_enabled: value.sandbox_posix_exec_enabled,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PylonPublicConfig {
    schema_version: u32,
    node_label: String,
    payout_destination: Option<String>,
    identity_path: PathBuf,
    admin_db_path: PathBuf,
    admin_listen_addr: String,
    nexus_control_base_url: String,
    relay_urls: Vec<String>,
    relay_connect_timeout_seconds: u64,
    relay_auth_enabled: bool,
    wallet_network: String,
    wallet_api_key_env: Option<String>,
    buyer_auto_pay_enabled: bool,
    wallet_storage_dir: PathBuf,
    local_gemma_base_url: String,
    inventory_controls: PylonPublicInventoryControls,
    declared_sandbox_profiles: Vec<ProviderSandboxProfileSpec>,
}

impl From<&PylonConfig> for PylonPublicConfig {
    fn from(value: &PylonConfig) -> Self {
        Self {
            schema_version: value.schema_version,
            node_label: value.node_label.clone(),
            payout_destination: value.payout_destination.clone(),
            identity_path: value.identity_path.clone(),
            admin_db_path: value.admin_db_path.clone(),
            admin_listen_addr: value.admin_listen_addr.clone(),
            nexus_control_base_url: value.nexus_control_base_url.clone(),
            relay_urls: value.relay_urls.clone(),
            relay_connect_timeout_seconds: value.relay_connect_timeout_seconds,
            relay_auth_enabled: value.relay_auth_enabled,
            wallet_network: value.wallet_network.clone(),
            wallet_api_key_env: value.wallet_api_key_env.clone(),
            buyer_auto_pay_enabled: value.buyer_auto_pay_enabled,
            wallet_storage_dir: value.wallet_storage_dir.clone(),
            local_gemma_base_url: value.local_gemma_base_url.clone(),
            inventory_controls: PylonPublicInventoryControls::from(&value.inventory_controls),
            declared_sandbox_profiles: value.declared_sandbox_profiles.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Init,
    Doctor,
    Serve,
    Status {
        json: bool,
    },
    Backends {
        json: bool,
    },
    Inventory {
        json: bool,
        limit: Option<usize>,
    },
    Products {
        json: bool,
    },
    Relays {
        json: bool,
    },
    Sandbox {
        json: bool,
        limit: Option<usize>,
    },
    Jobs {
        json: bool,
        limit: Option<usize>,
    },
    Earnings {
        json: bool,
    },
    Receipts {
        json: bool,
        limit: Option<usize>,
    },
    Activity {
        json: bool,
        limit: Option<usize>,
    },
    RelayAdd {
        url: String,
    },
    RelayRemove {
        url: String,
    },
    RelayRefresh {
        json: bool,
    },
    Announcement {
        action: AnnouncementAction,
        json: bool,
    },
    ProviderScan {
        seconds: u64,
        json: bool,
    },
    ProviderRun {
        seconds: u64,
        json: bool,
    },
    JobSubmit {
        request: BuyerJobSubmitRequest,
        json: bool,
    },
    JobWatch {
        request_event_id: Option<String>,
        seconds: u64,
        json: bool,
    },
    JobHistory {
        limit: Option<usize>,
        json: bool,
    },
    JobReplay {
        request_event_id: String,
        json: bool,
    },
    JobApprove {
        request_event_id: String,
        json: bool,
    },
    JobDeny {
        request_event_id: String,
        json: bool,
    },
    JobPolicy {
        mode: BuyerPaymentPolicyMode,
        json: bool,
    },
    Payout {
        limit: Option<u32>,
        json: bool,
    },
    PayoutWithdraw {
        payment_request: String,
        amount_sats: Option<u64>,
        json: bool,
    },
    Wallet {
        command: WalletSubcommand,
    },
    Gemma {
        command: GemmaCommand,
    },
    Online,
    Offline,
    Pause,
    Resume,
    ConfigShow,
    ConfigSet {
        key: String,
        value: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GemmaCommand {
    List {
        json: bool,
    },
    Download {
        selector: GemmaSelector,
        transport: GemmaDownloadTransport,
        json: bool,
    },
    Diagnose {
        selector: GemmaBenchmarkSelector,
        request: GemmaDiagnosticRequest,
        json: bool,
    },
    Benchmark {
        selector: GemmaBenchmarkSelector,
        request: GemmaBenchmarkRequest,
        json: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GemmaSelector {
    Model(String),
    All,
    Remaining,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GemmaDownloadTransport {
    Auto,
    Reqwest,
    Curl,
}

impl GemmaDownloadTransport {
    fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Reqwest => "reqwest",
            Self::Curl => "curl",
        }
    }
}

impl GemmaSelector {
    fn label(&self) -> String {
        match self {
            Self::Model(model_id) => model_id.clone(),
            Self::All => String::from("all"),
            Self::Remaining => String::from("remaining"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GemmaBenchmarkSelector {
    Model(String),
    All,
}

impl GemmaBenchmarkSelector {
    fn label(&self) -> String {
        match self {
            Self::Model(model_id) => model_id.clone(),
            Self::All => String::from("all"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GemmaBenchmarkMode {
    Single,
    DistributedDense,
    DistributedSparse,
    Matrix,
}

impl GemmaBenchmarkMode {
    fn parse(value: &str) -> Result<Self> {
        match value.trim() {
            "single" => Ok(Self::Single),
            "distributed-dense" => Ok(Self::DistributedDense),
            "distributed-sparse" => Ok(Self::DistributedSparse),
            "matrix" => Ok(Self::Matrix),
            other => bail!(
                "unsupported Gemma benchmark mode `{other}`; expected one of: single, distributed-dense, distributed-sparse, matrix"
            ),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::DistributedDense => "distributed-dense",
            Self::DistributedSparse => "distributed-sparse",
            Self::Matrix => "matrix",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GemmaBenchmarkRequest {
    pub mode: GemmaBenchmarkMode,
    pub backend: Option<String>,
    pub peer_base_url: Option<String>,
    pub split_layer: Option<usize>,
    pub prompt: String,
    pub max_output_tokens: usize,
    pub repeats: usize,
    pub download_missing: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GemmaDiagnosticRequest {
    pub diagnostic_id: String,
    pub prompt: String,
    pub max_output_tokens: usize,
    pub repeats: usize,
    pub download_missing: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cli {
    pub command: Command,
    pub config_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct InitReport {
    config_path: String,
    ledger_path: String,
    identity_path: String,
    npub: String,
    payout_destination: Option<String>,
    admin_listen_addr: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct DoctorReport {
    config_path: String,
    node_label: String,
    payout_destination: Option<String>,
    identity: ProviderIdentityMetadata,
    availability: ProviderAvailability,
    products: Vec<ProductEntry>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct ReportContext {
    state: String,
    desired_mode: String,
    listen_addr: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct BackendReport {
    context: ReportContext,
    backends: Vec<BackendEntry>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct BackendEntry {
    backend_id: String,
    display_label: String,
    health_state: String,
    reachable: bool,
    ready: bool,
    ready_model: Option<String>,
    available_models: Vec<String>,
    availability_message: Option<String>,
    launch_product_ids: Vec<String>,
    eligible_product_ids: Vec<String>,
    supported_execution_classes: Vec<String>,
    ready_execution_classes: Vec<String>,
    runtime_kinds: Vec<String>,
    ready_runtime_kinds: Vec<String>,
    profile_ids: Vec<String>,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct ProductReport {
    context: ReportContext,
    products: Vec<ProductEntry>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RelayReport {
    pub relay_config: RelayConfigReport,
    pub relays: Vec<RelayEntry>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RelayConfigReport {
    pub connect_timeout_seconds: u64,
    pub auth_enabled: bool,
    pub ledger_path: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RelayEntry {
    pub url: String,
    pub state: String,
    pub auth_state: String,
    pub detail: Option<String>,
    pub last_error: Option<String>,
    pub last_connected_at_ms: Option<u64>,
    pub updated_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct ProductEntry {
    product_id: String,
    display_label: String,
    compute_family: String,
    backend: String,
    enabled: bool,
    backend_ready: bool,
    eligible: bool,
    capability_summary: String,
    price_floor_sats: u64,
    terms_label: String,
    forward_terms_label: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct InventoryReport {
    context: ReportContext,
    rows: Vec<ProviderInventoryRow>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct JobsReport {
    context: ReportContext,
    jobs: Vec<ProviderRecentJob>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct EarningsReport {
    context: ReportContext,
    earnings: Option<ProviderEarningsSummary>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ReceiptsReport {
    context: ReportContext,
    receipts: Vec<ProviderReceiptSummary>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RelayActivityReport {
    entries: Vec<PylonRelayActivity>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PayoutReport {
    payout_destination: Option<String>,
    wallet_balance: WalletBalanceSnapshot,
    earnings_lifetime_sats: u64,
    earnings_sats_today: u64,
    jobs_today: u64,
    last_job_result: String,
    withdrawals: Vec<PylonLedgerPayout>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PayoutWithdrawalReport {
    payout_destination: Option<String>,
    payment_id: String,
    status: String,
    amount_sats: u64,
    fees_sats: u64,
    invoice: String,
    post_balance: WalletBalanceSnapshot,
    detail: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct SandboxReport {
    context: ReportContext,
    supported_execution_classes: Vec<String>,
    ready_execution_classes: Vec<String>,
    last_scan_error: Option<String>,
    runtimes: Vec<ProviderSandboxRuntimeHealth>,
    profiles: Vec<ProviderSandboxProfile>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalGemmaChatBackend {
    LocalRuntime,
}

impl LocalGemmaChatBackend {
    pub const fn label(self) -> &'static str {
        match self {
            Self::LocalRuntime => "local_runtime",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalGemmaChatTarget {
    pub backend: LocalGemmaChatBackend,
    pub model: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalGemmaChatMessageRole {
    System,
    User,
    Assistant,
}

impl LocalGemmaChatMessageRole {
    fn api_label(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalGemmaChatMessage {
    pub role: LocalGemmaChatMessageRole,
    pub content: String,
}

impl LocalGemmaChatMessage {
    #[must_use]
    pub fn new(role: LocalGemmaChatMessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }

    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(LocalGemmaChatMessageRole::System, content)
    }

    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(LocalGemmaChatMessageRole::User, content)
    }

    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(LocalGemmaChatMessageRole::Assistant, content)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalGemmaChatEvent {
    Started { target: LocalGemmaChatTarget },
    Delta(String),
    Finished { target: LocalGemmaChatTarget },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GemmaDownloadSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub quantization: &'static str,
    pub psionic_model_id: &'static str,
    pub repo_id: &'static str,
    pub filename: &'static str,
    pub runtime_shape: GemmaRuntimeShape,
}

impl GemmaDownloadSpec {
    fn download_url(self, base_url: &str) -> String {
        format!(
            "{}/{}/resolve/main/{}?download=true",
            base_url.trim_end_matches('/'),
            self.repo_id,
            self.filename,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GemmaRuntimeShape {
    Dense,
    SparseDistributedOnly,
}

impl GemmaRuntimeShape {
    pub const fn supports_single_node(self) -> bool {
        matches!(self, Self::Dense)
    }

    pub const fn supports_dense_split(self) -> bool {
        matches!(self, Self::Dense)
    }

    pub const fn supports_sparse_distributed(self) -> bool {
        matches!(self, Self::SparseDistributedOnly)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Dense => "dense",
            Self::SparseDistributedOnly => "sparse_distributed_only",
        }
    }

    pub fn supported_mode_labels(self) -> Vec<String> {
        let mut modes = Vec::new();
        if self.supports_single_node() {
            modes.push(String::from("single"));
        }
        if self.supports_dense_split() {
            modes.push(String::from("distributed-dense"));
        }
        if self.supports_sparse_distributed() {
            modes.push(String::from("distributed-sparse"));
        }
        modes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GemmaLocalInstallation {
    pub spec: GemmaDownloadSpec,
    pub path: PathBuf,
    pub installed: bool,
    pub file_bytes: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GemmaDownloadEvent {
    Started {
        spec: GemmaDownloadSpec,
        total_bytes: Option<u64>,
    },
    Progress {
        spec: GemmaDownloadSpec,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Finished {
        spec: GemmaDownloadSpec,
        path: PathBuf,
        file_bytes: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GemmaCatalogReport {
    pub models_root: String,
    pub models: Vec<GemmaCatalogEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GemmaCatalogEntry {
    pub id: String,
    pub label: String,
    pub psionic_model_id: String,
    pub quantization: String,
    pub runtime_shape: String,
    pub supported_modes: Vec<String>,
    pub installed: bool,
    pub file_bytes: Option<u64>,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GemmaDownloadReport {
    pub selector: String,
    pub models_root: String,
    pub results: Vec<GemmaDownloadResult>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GemmaDownloadResult {
    pub model_id: String,
    pub label: String,
    pub status: String,
    pub transport: String,
    pub file_bytes: Option<u64>,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GemmaBenchmarkReport {
    pub selector: String,
    pub psionic_repo: String,
    pub results: Vec<GemmaBenchmarkResult>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GemmaBenchmarkResult {
    pub model_id: String,
    pub label: String,
    pub psionic_model_id: String,
    pub runtime_shape: String,
    pub mode: String,
    pub status: String,
    pub reason: Option<String>,
    pub path: Option<String>,
    pub command: Option<Vec<String>>,
    pub receipt: Option<GemmaBenchReceipt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaDiagnosticReport {
    pub schema_version: u32,
    pub report_kind: String,
    pub selector: String,
    pub diagnostic_id: String,
    pub measured_at_unix_ms: u64,
    #[serde(default)]
    pub repeats: usize,
    pub report_path: String,
    pub results: Vec<GemmaDiagnosticResult>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaDiagnosticResult {
    pub model_id: String,
    pub label: String,
    pub runtime_model: Option<String>,
    pub runtime_backend: String,
    pub status: String,
    pub reason: Option<String>,
    pub model_cached: bool,
    pub ready_in_runtime: bool,
    pub receipt: Option<GemmaDiagnosticReceipt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaDiagnosticReceipt {
    pub schema_version: u32,
    pub report_kind: String,
    pub diagnostic_id: String,
    pub measured_at_unix_ms: u64,
    pub model_id: String,
    pub runtime_model: String,
    pub runtime_backend: String,
    pub load_s: Option<f64>,
    pub mean_total_s: f64,
    pub mean_ttft_s: Option<f64>,
    pub mean_decode_tok_s: Option<f64>,
    pub output_tokens: usize,
    pub repeats: usize,
    pub runs: Vec<GemmaDiagnosticRunReceipt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaDiagnosticRunReceipt {
    pub run_index: usize,
    pub output_tokens: usize,
    pub total_s: f64,
    pub ttft_s: Option<f64>,
    pub decode_tok_s: Option<f64>,
    pub load_s: Option<f64>,
    pub output_text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaBenchReceipt {
    pub schema_version: u32,
    pub report_kind: String,
    pub mode: String,
    pub model_id: String,
    pub model_path: String,
    pub runtime_backend: String,
    pub sparse_expert_topology: bool,
    pub peer_base_url: Option<String>,
    pub split_layer: Option<usize>,
    pub prompt: String,
    pub max_output_tokens: usize,
    pub repeats: usize,
    pub load_s: f64,
    pub cluster_topology: Option<String>,
    pub runs: Vec<GemmaBenchRunReceipt>,
    pub mean_output_tokens: f64,
    pub mean_total_s: f64,
    pub mean_ttft_s: Option<f64>,
    pub mean_decode_tok_s: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaBenchRunReceipt {
    pub run_index: usize,
    pub output_tokens: usize,
    pub total_s: f64,
    pub prompt_s: Option<f64>,
    pub decode_s: Option<f64>,
    pub ttft_s: Option<f64>,
    pub decode_tok_s: Option<f64>,
    pub termination: String,
    pub output_text: String,
}

const GEMMA_DOWNLOAD_SPECS: [GemmaDownloadSpec; 4] = [
    GemmaDownloadSpec {
        id: "gemma-4-e2b",
        label: "Gemma 4 E2B",
        quantization: "Q8_0",
        psionic_model_id: "gemma4:e2b",
        repo_id: "ggml-org/gemma-4-E2B-it-GGUF",
        filename: "gemma-4-e2b-it-Q8_0.gguf",
        runtime_shape: GemmaRuntimeShape::Dense,
    },
    GemmaDownloadSpec {
        id: "gemma-4-e4b",
        label: "Gemma 4 E4B",
        quantization: "Q4_K_M",
        psionic_model_id: "gemma4:e4b",
        repo_id: "ggml-org/gemma-4-E4B-it-GGUF",
        filename: "gemma-4-e4b-it-Q4_K_M.gguf",
        runtime_shape: GemmaRuntimeShape::Dense,
    },
    GemmaDownloadSpec {
        id: "gemma-4-26b-a4b",
        label: "Gemma 4 26B A4B",
        quantization: "Q4_K_M",
        psionic_model_id: "gemma4:26b",
        repo_id: "ggml-org/gemma-4-26B-A4B-it-GGUF",
        filename: "gemma-4-26B-A4B-it-Q4_K_M.gguf",
        runtime_shape: GemmaRuntimeShape::SparseDistributedOnly,
    },
    GemmaDownloadSpec {
        id: "gemma-4-31b",
        label: "Gemma 4 31B",
        quantization: "Q4_K_M",
        psionic_model_id: "gemma4:31b",
        repo_id: "ggml-org/gemma-4-31B-it-GGUF",
        filename: "gemma-4-31B-it-Q4_K_M.gguf",
        runtime_shape: GemmaRuntimeShape::Dense,
    },
];

const DEFAULT_GEMMA_BENCH_PROMPT: &str =
    "Write one short sentence about decentralized Gemma inference.";
const DEFAULT_GEMMA_DIAGNOSTIC_ID: &str = "openagents.pylon.first_run.v1";
const GEMMA_DIAGNOSTIC_REPORT_KIND: &str = "pylon.gemma_diagnostic.report.v1";
const GEMMA_DIAGNOSTIC_RECEIPT_KIND: &str = "pylon.gemma_diagnostic.receipt.v1";
const GEMMA_DIAGNOSTIC_SCHEMA_VERSION: u32 = 1;

pub fn gemma_download_specs() -> &'static [GemmaDownloadSpec] {
    &GEMMA_DOWNLOAD_SPECS
}

pub fn gemma_download_spec(model_id: &str) -> Option<GemmaDownloadSpec> {
    GEMMA_DOWNLOAD_SPECS
        .iter()
        .copied()
        .find(|spec| spec.id == model_id.trim())
}

pub fn gemma_models_root(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(default_home_dir)
        .join("models")
        .join("huggingface")
}

pub fn gemma_model_path(config_path: &Path, spec: GemmaDownloadSpec) -> PathBuf {
    gemma_models_root(config_path)
        .join(spec.id)
        .join(spec.filename)
}

pub fn gemma_local_installations(config_path: &Path) -> Vec<GemmaLocalInstallation> {
    gemma_download_specs()
        .iter()
        .copied()
        .map(|spec| {
            let path = gemma_model_path(config_path, spec);
            let file_bytes = std::fs::metadata(path.as_path())
                .ok()
                .map(|metadata| metadata.len());
            GemmaLocalInstallation {
                spec,
                path,
                installed: file_bytes.is_some(),
                file_bytes,
            }
        })
        .collect()
}

pub async fn download_gemma_model<F>(config_path: &Path, model_id: &str, emit: F) -> Result<PathBuf>
where
    F: FnMut(GemmaDownloadEvent),
{
    download_gemma_model_from_base_url(config_path, model_id, "https://huggingface.co", emit).await
}

pub async fn download_gemma_model_with_transport<F>(
    config_path: &Path,
    model_id: &str,
    transport: GemmaDownloadTransport,
    emit: F,
) -> Result<PathBuf>
where
    F: FnMut(GemmaDownloadEvent),
{
    download_gemma_model_from_base_url_with_transport(
        config_path,
        model_id,
        "https://huggingface.co",
        transport,
        emit,
    )
    .await
}

async fn download_gemma_model_from_base_url<F>(
    config_path: &Path,
    model_id: &str,
    base_url: &str,
    emit: F,
) -> Result<PathBuf>
where
    F: FnMut(GemmaDownloadEvent),
{
    download_gemma_model_from_base_url_with_transport(
        config_path,
        model_id,
        base_url,
        GemmaDownloadTransport::Auto,
        emit,
    )
    .await
}

fn build_gemma_download_client(bind_ipv4_unspecified: bool) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .user_agent("openagents-pylon/0.1.1")
        .connect_timeout(Duration::from_secs(15));
    if bind_ipv4_unspecified {
        builder = builder
            .local_address(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
            .http1_only();
    }
    builder
        .build()
        .context("failed to build Gemma download client")
}

fn gemma_download_transport_error(error: &anyhow::Error) -> bool {
    error.chain().any(|source| {
        source
            .downcast_ref::<reqwest::Error>()
            .is_some_and(|reqwest_error| reqwest_error.is_connect() || reqwest_error.is_timeout())
    }) || error.to_string().contains("Can't assign requested address")
        || error.to_string().contains("os error 49")
}

async fn download_gemma_via_reqwest<F>(
    url: &str,
    partial_path: &Path,
    spec: GemmaDownloadSpec,
    bind_ipv4_unspecified: bool,
    emit: &mut F,
) -> Result<u64>
where
    F: FnMut(GemmaDownloadEvent),
{
    use tokio::io::AsyncWriteExt;

    let client = build_gemma_download_client(bind_ipv4_unspecified)?;
    let mut response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to request {url}"))?
        .error_for_status()
        .with_context(|| format!("download endpoint rejected {url}"))?;
    let total_bytes = response.content_length();
    emit(GemmaDownloadEvent::Started { spec, total_bytes });

    let mut file = tokio::fs::File::create(partial_path)
        .await
        .with_context(|| format!("failed to create {}", partial_path.display()))?;
    let mut downloaded_bytes = 0_u64;
    let mut last_progress_emit = Instant::now()
        .checked_sub(Duration::from_secs(1))
        .unwrap_or_else(Instant::now);

    while let Some(chunk) = response
        .chunk()
        .await
        .with_context(|| format!("failed reading {url}"))?
    {
        file.write_all(&chunk).await?;
        downloaded_bytes = downloaded_bytes.saturating_add(chunk.len() as u64);
        let should_emit = last_progress_emit.elapsed() >= Duration::from_millis(100)
            || total_bytes == Some(downloaded_bytes);
        if should_emit {
            emit(GemmaDownloadEvent::Progress {
                spec,
                downloaded_bytes,
                total_bytes,
            });
            last_progress_emit = Instant::now();
        }
    }

    file.flush().await?;
    Ok(downloaded_bytes)
}

async fn download_gemma_via_curl<F>(
    url: &str,
    partial_path: &Path,
    spec: GemmaDownloadSpec,
    emit: &mut F,
) -> Result<u64>
where
    F: FnMut(GemmaDownloadEvent),
{
    emit(GemmaDownloadEvent::Started {
        spec,
        total_bytes: None,
    });
    let output = TokioCommand::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--connect-timeout",
            "15",
            "--retry",
            "2",
            "--retry-delay",
            "1",
            "--output",
        ])
        .arg(partial_path)
        .arg(url)
        .output()
        .await
        .context("failed to start curl for Gemma download")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            format!("curl exited with {}", output.status)
        } else {
            stderr
        };
        bail!("curl fallback failed for {url}: {detail}");
    }
    let file_bytes = tokio::fs::metadata(partial_path)
        .await
        .with_context(|| format!("failed to stat {}", partial_path.display()))?
        .len();
    emit(GemmaDownloadEvent::Progress {
        spec,
        downloaded_bytes: file_bytes,
        total_bytes: Some(file_bytes),
    });
    Ok(file_bytes)
}

async fn download_gemma_model_from_base_url_with_transport<F>(
    config_path: &Path,
    model_id: &str,
    base_url: &str,
    transport: GemmaDownloadTransport,
    mut emit: F,
) -> Result<PathBuf>
where
    F: FnMut(GemmaDownloadEvent),
{
    let spec =
        gemma_download_spec(model_id).ok_or_else(|| anyhow!("unknown Gemma model `{model_id}`"))?;
    let final_path = gemma_model_path(config_path, spec);
    if let Some(parent) = final_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if final_path.exists() {
        let file_bytes = tokio::fs::metadata(final_path.as_path())
            .await
            .with_context(|| format!("failed to stat {}", final_path.display()))?
            .len();
        emit(GemmaDownloadEvent::Finished {
            spec,
            path: final_path.clone(),
            file_bytes,
        });
        return Ok(final_path);
    }

    let partial_path = final_path.with_extension(format!(
        "{}.part",
        final_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("download"),
    ));
    let download_url = spec.download_url(base_url);

    let result = async {
        let mut attempt_errors = Vec::new();
        let downloaded_bytes = match transport {
            GemmaDownloadTransport::Reqwest => {
                match download_gemma_via_reqwest(
                    download_url.as_str(),
                    partial_path.as_path(),
                    spec,
                    false,
                    &mut emit,
                )
                .await
                {
                    Ok(file_bytes) => file_bytes,
                    Err(error) if gemma_download_transport_error(&error) => {
                        attempt_errors.push(format!("reqwest: {error}"));
                        let _ = tokio::fs::remove_file(partial_path.as_path()).await;
                        let file_bytes = download_gemma_via_reqwest(
                            download_url.as_str(),
                            partial_path.as_path(),
                            spec,
                            true,
                            &mut emit,
                        )
                        .await
                        .with_context(|| {
                            format!(
                                "Gemma download transport reqwest failed for {download_url}; retry with `pylon gemma download {model_id} --transport curl` if this host is in an SSH/VPN-constrained network context"
                            )
                        })?;
                        file_bytes
                    }
                    Err(error) => return Err(error),
                }
            }
            GemmaDownloadTransport::Curl => {
                let file_bytes = download_gemma_via_curl(
                    download_url.as_str(),
                    partial_path.as_path(),
                    spec,
                    &mut emit,
                )
                .await?;
                file_bytes
            }
            GemmaDownloadTransport::Auto => {
                match download_gemma_via_reqwest(
                    download_url.as_str(),
                    partial_path.as_path(),
                    spec,
                    false,
                    &mut emit,
                )
                .await
                {
                    Ok(file_bytes) => file_bytes,
                    Err(error) if gemma_download_transport_error(&error) => {
                        attempt_errors.push(format!("reqwest: {error}"));
                        let _ = tokio::fs::remove_file(partial_path.as_path()).await;
                        match download_gemma_via_reqwest(
                            download_url.as_str(),
                            partial_path.as_path(),
                            spec,
                            true,
                            &mut emit,
                        )
                        .await
                        {
                            Ok(file_bytes) => file_bytes,
                            Err(ipv4_error) => {
                                attempt_errors.push(format!("reqwest-ipv4: {ipv4_error}"));
                                let _ = tokio::fs::remove_file(partial_path.as_path()).await;
                                let file_bytes = download_gemma_via_curl(
                                    download_url.as_str(),
                                    partial_path.as_path(),
                                    spec,
                                    &mut emit,
                                )
                                .await
                                .with_context(|| {
                                    format!(
                                        "Gemma download failed across reqwest and curl transports for {download_url}: {}",
                                        attempt_errors.join(" | ")
                                    )
                                })?;
                                file_bytes
                            }
                        }
                    }
                    Err(error) => return Err(error),
                }
            }
        };

        tokio::fs::rename(partial_path.as_path(), final_path.as_path())
            .await
            .with_context(|| {
                format!(
                    "failed to move {} into {}",
                    partial_path.display(),
                    final_path.display()
                )
            })?;
        emit(GemmaDownloadEvent::Finished {
            spec,
            path: final_path.clone(),
            file_bytes: downloaded_bytes,
        });
        Ok::<(), anyhow::Error>(())
    }
    .await;

    if result.is_err() {
        let _ = tokio::fs::remove_file(partial_path.as_path()).await;
    }
    result?;
    Ok(final_path)
}

fn gemma_catalog_report(config_path: &Path) -> GemmaCatalogReport {
    let models_root = gemma_models_root(config_path);
    let models = gemma_local_installations(config_path)
        .into_iter()
        .map(|installation| GemmaCatalogEntry {
            id: installation.spec.id.to_string(),
            label: installation.spec.label.to_string(),
            psionic_model_id: installation.spec.psionic_model_id.to_string(),
            quantization: installation.spec.quantization.to_string(),
            runtime_shape: installation.spec.runtime_shape.label().to_string(),
            supported_modes: installation.spec.runtime_shape.supported_mode_labels(),
            installed: installation.installed,
            file_bytes: installation.file_bytes,
            path: installation.path.display().to_string(),
        })
        .collect();
    GemmaCatalogReport {
        models_root: models_root.display().to_string(),
        models,
    }
}

fn render_gemma_catalog_report(report: &GemmaCatalogReport) -> String {
    let mut lines = vec![
        format!("Gemma models root: {}", report.models_root),
        String::new(),
    ];
    for entry in &report.models {
        let installed = if entry.installed {
            "installed"
        } else {
            "missing"
        };
        let size = entry
            .file_bytes
            .map(render_byte_size)
            .unwrap_or_else(|| String::from("n/a"));
        lines.push(format!(
            "{}  {}  {}  {}  modes={}  size={}",
            entry.id,
            installed,
            entry.quantization,
            entry.runtime_shape,
            entry.supported_modes.join(","),
            size
        ));
    }
    lines.join("\n")
}

fn resolve_gemma_selector(
    selector: &GemmaSelector,
    config_path: &Path,
) -> Result<Vec<GemmaDownloadSpec>> {
    match selector {
        GemmaSelector::Model(model_id) => {
            Ok(vec![gemma_download_spec(model_id).ok_or_else(|| {
                anyhow!("unknown Gemma model `{model_id}`")
            })?])
        }
        GemmaSelector::All => Ok(gemma_download_specs().to_vec()),
        GemmaSelector::Remaining => Ok(gemma_local_installations(config_path)
            .into_iter()
            .filter(|installation| !installation.installed)
            .map(|installation| installation.spec)
            .collect()),
    }
}

fn resolve_gemma_benchmark_selector(
    selector: &GemmaBenchmarkSelector,
) -> Result<Vec<GemmaDownloadSpec>> {
    match selector {
        GemmaBenchmarkSelector::Model(model_id) => {
            Ok(vec![gemma_download_spec(model_id).ok_or_else(|| {
                anyhow!("unknown Gemma model `{model_id}`")
            })?])
        }
        GemmaBenchmarkSelector::All => Ok(gemma_download_specs().to_vec()),
    }
}

async fn run_gemma_download_command(
    config_path: &Path,
    selector: &GemmaSelector,
    transport: GemmaDownloadTransport,
) -> Result<GemmaDownloadReport> {
    let _ = ensure_local_setup(config_path)?;
    let mut results = Vec::new();
    for spec in resolve_gemma_selector(selector, config_path)? {
        let already_installed = gemma_model_path(config_path, spec).exists();
        let final_path =
            download_gemma_model_with_transport(config_path, spec.id, transport, |_| {}).await?;
        let file_bytes = tokio::fs::metadata(final_path.as_path())
            .await
            .ok()
            .map(|value| value.len());
        results.push(GemmaDownloadResult {
            model_id: spec.id.to_string(),
            label: spec.label.to_string(),
            status: if already_installed {
                String::from("already_installed")
            } else {
                String::from("downloaded")
            },
            transport: if already_installed {
                String::from("cache")
            } else {
                transport.label().to_string()
            },
            file_bytes,
            path: final_path.display().to_string(),
        });
    }
    Ok(GemmaDownloadReport {
        selector: selector.label(),
        models_root: gemma_models_root(config_path).display().to_string(),
        results,
    })
}

fn render_gemma_download_report(report: &GemmaDownloadReport) -> String {
    let mut lines = vec![format!("selector: {}", report.selector)];
    for result in &report.results {
        let size = result
            .file_bytes
            .map(render_byte_size)
            .unwrap_or_else(|| String::from("n/a"));
        lines.push(format!(
            "{}  {}  transport={}  size={}  {}",
            result.model_id, result.status, result.transport, size, result.path
        ));
    }
    if report.results.is_empty() {
        lines.push(String::from("no matching Gemma models"));
    }
    lines.join("\n")
}

pub fn gemma_diagnostics_root(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(default_home_dir)
        .join("diagnostics")
        .join("gemma")
}

pub fn gemma_diagnostic_latest_report_path(config_path: &Path) -> PathBuf {
    gemma_diagnostics_root(config_path).join("latest.json")
}

pub fn load_latest_gemma_diagnostic_report(
    config_path: &Path,
) -> Result<Option<GemmaDiagnosticReport>> {
    let path = gemma_diagnostic_latest_report_path(config_path);
    if !path.exists() {
        return Ok(None);
    }
    let payload = std::fs::read_to_string(path.as_path())
        .with_context(|| format!("failed to read {}", path.display()))?;
    let report = serde_json::from_str::<GemmaDiagnosticReport>(payload.as_str())
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(report))
}

fn provider_diagnostic_summaries_from_report(
    report: &GemmaDiagnosticReport,
) -> Vec<ProviderDiagnosticSummary> {
    report
        .results
        .iter()
        .map(|result| ProviderDiagnosticSummary {
            diagnostic_id: report.diagnostic_id.clone(),
            model_id: result.model_id.clone(),
            runtime_backend: result.runtime_backend.clone(),
            status: result.status.clone(),
            reason: result.reason.clone(),
            measured_at_unix_ms: result
                .receipt
                .as_ref()
                .map(|receipt| receipt.measured_at_unix_ms)
                .unwrap_or(report.measured_at_unix_ms),
            load_s: result.receipt.as_ref().and_then(|receipt| receipt.load_s),
            mean_total_s: result.receipt.as_ref().map(|receipt| receipt.mean_total_s),
            mean_ttft_s: result
                .receipt
                .as_ref()
                .and_then(|receipt| receipt.mean_ttft_s),
            mean_decode_tok_s: result
                .receipt
                .as_ref()
                .and_then(|receipt| receipt.mean_decode_tok_s),
            repeats: u64::try_from(
                result
                    .receipt
                    .as_ref()
                    .map(|receipt| receipt.repeats)
                    .unwrap_or(report.repeats),
            )
            .unwrap_or(u64::MAX),
        })
        .collect()
}

fn load_latest_provider_diagnostic_summaries(config_path: &Path) -> Vec<ProviderDiagnosticSummary> {
    match load_latest_gemma_diagnostic_report(config_path) {
        Ok(Some(report)) => provider_diagnostic_summaries_from_report(&report),
        Ok(None) => Vec::new(),
        Err(error) => {
            eprintln!(
                "warning: failed to load latest Gemma diagnostic report for Nexus heartbeat: {error}"
            );
            Vec::new()
        }
    }
}

fn save_gemma_diagnostic_report(
    config_path: &Path,
    report: &mut GemmaDiagnosticReport,
) -> Result<()> {
    let path = gemma_diagnostic_latest_report_path(config_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    report.report_path = path.display().to_string();
    let payload = serde_json::to_string_pretty(report)?;
    std::fs::write(path.as_path(), format!("{payload}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq)]
struct LocalGemmaChatCompletionMetrics {
    total_s: Option<f64>,
    load_s: Option<f64>,
    prompt_s: Option<f64>,
    decode_s: Option<f64>,
    output_tokens: Option<usize>,
    decode_tok_s: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct LocalGemmaChatChunk {
    delta: Option<String>,
    completion: Option<LocalGemmaChatCompletionMetrics>,
}

fn parse_ollama_duration_seconds(payload: &Value, key: &str) -> Option<f64> {
    payload
        .get(key)
        .and_then(Value::as_u64)
        .map(|value| value as f64 / 1_000_000_000.0)
}

fn normalize_model_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn runtime_model_matches_spec(runtime_model: &str, spec: GemmaDownloadSpec) -> bool {
    let normalized_runtime = normalize_model_key(runtime_model);
    [spec.id, spec.psionic_model_id]
        .into_iter()
        .map(normalize_model_key)
        .any(|candidate| {
            !candidate.is_empty()
                && (normalized_runtime == candidate
                    || normalized_runtime.contains(candidate.as_str()))
        })
}

fn runtime_model_for_spec(
    health: &ProviderBackendHealth,
    spec: GemmaDownloadSpec,
) -> Option<String> {
    let mut models = Vec::new();
    if let Some(model) = health.ready_model.as_ref() {
        models.push(model.clone());
    }
    for model in &health.available_models {
        if !models.iter().any(|existing| existing == model) {
            models.push(model.clone());
        }
    }
    models
        .into_iter()
        .find(|model| runtime_model_matches_spec(model.as_str(), spec))
}

async fn run_gemma_diagnostic_command(
    config_path: &Path,
    selector: &GemmaBenchmarkSelector,
    request: &GemmaDiagnosticRequest,
) -> Result<GemmaDiagnosticReport> {
    let _ = ensure_local_setup(config_path)?;
    let specs = resolve_gemma_benchmark_selector(selector)?;
    if request.download_missing {
        for spec in &specs {
            let _ = download_gemma_model(config_path, spec.id, |_| {}).await?;
        }
    }

    let (config, status) = load_config_and_status(config_path).await?;
    let measured_at_unix_ms = u64::try_from(now_epoch_ms()).unwrap_or(0);
    let local_gemma_health = status
        .snapshot
        .as_ref()
        .map(|snapshot| &snapshot.availability.local_gemma);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .context("failed to build pylon diagnostic client")?;

    let mut results = Vec::new();
    for spec in specs {
        let model_cached = gemma_model_path(config_path, spec).exists();
        let runtime_backend = LocalGemmaChatBackend::LocalRuntime.label().to_string();
        let Some(health) = local_gemma_health else {
            results.push(GemmaDiagnosticResult {
                model_id: spec.id.to_string(),
                label: spec.label.to_string(),
                runtime_model: None,
                runtime_backend,
                status: "failed".to_string(),
                reason: Some("local Gemma runtime status is unavailable".to_string()),
                model_cached,
                ready_in_runtime: false,
                receipt: None,
            });
            continue;
        };

        if !health.reachable {
            results.push(GemmaDiagnosticResult {
                model_id: spec.id.to_string(),
                label: spec.label.to_string(),
                runtime_model: None,
                runtime_backend,
                status: "failed".to_string(),
                reason: health
                    .last_error
                    .clone()
                    .or_else(|| Some("local Gemma runtime is unreachable".to_string())),
                model_cached,
                ready_in_runtime: false,
                receipt: None,
            });
            continue;
        }

        let runtime_model = runtime_model_for_spec(health, spec);
        let ready_in_runtime = runtime_model.is_some() && health.ready;
        let Some(runtime_model) = runtime_model else {
            results.push(GemmaDiagnosticResult {
                model_id: spec.id.to_string(),
                label: spec.label.to_string(),
                runtime_model: None,
                runtime_backend,
                status: "skipped".to_string(),
                reason: Some(format!(
                    "{} is not loaded in the local runtime; downloaded GGUF files alone do not make supply eligible",
                    spec.id
                )),
                model_cached,
                ready_in_runtime,
                receipt: None,
            });
            continue;
        };

        let target = LocalGemmaChatTarget {
            backend: LocalGemmaChatBackend::LocalRuntime,
            model: runtime_model.clone(),
        };
        match run_local_gemma_diagnostic_target(
            &client,
            &config,
            &target,
            spec,
            request,
            measured_at_unix_ms,
        )
        .await
        {
            Ok(receipt) => results.push(GemmaDiagnosticResult {
                model_id: spec.id.to_string(),
                label: spec.label.to_string(),
                runtime_model: Some(runtime_model),
                runtime_backend,
                status: "completed".to_string(),
                reason: None,
                model_cached,
                ready_in_runtime,
                receipt: Some(receipt),
            }),
            Err(error) => results.push(GemmaDiagnosticResult {
                model_id: spec.id.to_string(),
                label: spec.label.to_string(),
                runtime_model: Some(runtime_model),
                runtime_backend,
                status: "failed".to_string(),
                reason: Some(error.to_string()),
                model_cached,
                ready_in_runtime,
                receipt: None,
            }),
        }
    }

    let mut report = GemmaDiagnosticReport {
        schema_version: GEMMA_DIAGNOSTIC_SCHEMA_VERSION,
        report_kind: GEMMA_DIAGNOSTIC_REPORT_KIND.to_string(),
        selector: selector.label(),
        diagnostic_id: request.diagnostic_id.clone(),
        measured_at_unix_ms,
        repeats: request.repeats,
        report_path: String::new(),
        results,
    };
    save_gemma_diagnostic_report(config_path, &mut report)?;
    Ok(report)
}

async fn run_local_gemma_diagnostic_target(
    client: &reqwest::Client,
    config: &PylonConfig,
    target: &LocalGemmaChatTarget,
    spec: GemmaDownloadSpec,
    request: &GemmaDiagnosticRequest,
    measured_at_unix_ms: u64,
) -> Result<GemmaDiagnosticReceipt> {
    let mut runs = Vec::with_capacity(request.repeats);
    for run_index in 0..request.repeats {
        runs.push(
            run_local_gemma_diagnostic_once(
                client,
                config,
                target,
                request.prompt.as_str(),
                request.max_output_tokens,
                run_index,
            )
            .await?,
        );
    }

    let mean_total_s = runs.iter().map(|run| run.total_s).sum::<f64>() / runs.len() as f64;
    let mean_ttft_s = mean_f64_option(runs.iter().filter_map(|run| run.ttft_s));
    let mean_decode_tok_s = mean_f64_option(runs.iter().filter_map(|run| run.decode_tok_s));
    let load_s = mean_f64_option(runs.iter().filter_map(|run| run.load_s));
    let output_tokens = (runs.iter().map(|run| run.output_tokens).sum::<usize>() as f64
        / runs.len() as f64)
        .round() as usize;

    Ok(GemmaDiagnosticReceipt {
        schema_version: GEMMA_DIAGNOSTIC_SCHEMA_VERSION,
        report_kind: GEMMA_DIAGNOSTIC_RECEIPT_KIND.to_string(),
        diagnostic_id: request.diagnostic_id.clone(),
        measured_at_unix_ms,
        model_id: spec.id.to_string(),
        runtime_model: target.model.clone(),
        runtime_backend: target.backend.label().to_string(),
        load_s,
        mean_total_s,
        mean_ttft_s,
        mean_decode_tok_s,
        output_tokens,
        repeats: request.repeats,
        runs,
    })
}

async fn run_local_gemma_diagnostic_once(
    client: &reqwest::Client,
    config: &PylonConfig,
    target: &LocalGemmaChatTarget,
    prompt: &str,
    max_output_tokens: usize,
    run_index: usize,
) -> Result<GemmaDiagnosticRunReceipt> {
    let url = format!(
        "{}/api/chat",
        config.local_gemma_base_url.trim_end_matches('/')
    );
    let payload = json!({
        "model": target.model,
        "messages": [
            {
                "role": "user",
                "content": prompt,
            }
        ],
        "stream": true,
        "options": {
            "num_predict": max_output_tokens,
        },
    });
    let started_at = Instant::now();
    let mut response = client
        .post(url.as_str())
        .json(&payload)
        .send()
        .await
        .with_context(|| {
            format!(
                "failed to send local Gemma diagnostic request to {url}; verify the local runtime is reachable and the model is loaded"
            )
        })?;
    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!(
            "local Gemma diagnostic failed: {}",
            http_error_message(body.as_str())
        );
    }

    let mut pending = String::new();
    let mut output_text = String::new();
    let mut first_delta_at = None::<f64>;
    let mut completion = None::<LocalGemmaChatCompletionMetrics>;
    while let Some(chunk) = response
        .chunk()
        .await
        .context("failed reading local Gemma diagnostic stream")?
    {
        pending.push_str(String::from_utf8_lossy(&chunk).as_ref());
        while let Some(line_end) = pending.find('\n') {
            let line = pending[..line_end].trim().to_string();
            pending.drain(..=line_end);
            if line.is_empty() {
                continue;
            }
            let chunk = decode_ollama_chat_chunk(line.as_str())?;
            if let Some(delta) = chunk.delta {
                if first_delta_at.is_none() {
                    first_delta_at = Some(started_at.elapsed().as_secs_f64());
                }
                output_text.push_str(delta.as_str());
            }
            if let Some(chunk_completion) = chunk.completion {
                completion = Some(chunk_completion);
            }
        }
    }

    let trailing = pending.trim();
    if !trailing.is_empty() {
        let chunk = decode_ollama_chat_chunk(trailing)?;
        if let Some(delta) = chunk.delta {
            if first_delta_at.is_none() {
                first_delta_at = Some(started_at.elapsed().as_secs_f64());
            }
            output_text.push_str(delta.as_str());
        }
        if let Some(chunk_completion) = chunk.completion {
            completion = Some(chunk_completion);
        }
    }

    let total_s = completion
        .as_ref()
        .and_then(|value| value.total_s)
        .unwrap_or_else(|| started_at.elapsed().as_secs_f64());
    let output_tokens = completion
        .as_ref()
        .and_then(|value| value.output_tokens)
        .unwrap_or(0);
    let decode_tok_s = completion
        .as_ref()
        .and_then(|value| value.decode_tok_s)
        .or_else(|| {
            let ttft_s = first_delta_at?;
            let decode_window_s = total_s - ttft_s;
            (output_tokens > 0 && decode_window_s > 0.0)
                .then_some(output_tokens as f64 / decode_window_s)
        });

    Ok(GemmaDiagnosticRunReceipt {
        run_index,
        output_tokens,
        total_s,
        ttft_s: first_delta_at,
        decode_tok_s,
        load_s: completion.as_ref().and_then(|value| value.load_s),
        output_text,
    })
}

fn mean_f64_option(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut total = 0.0;
    let mut count = 0usize;
    for value in values {
        total += value;
        count += 1;
    }
    (count > 0).then_some(total / count as f64)
}

fn render_gemma_diagnostic_report(report: &GemmaDiagnosticReport) -> String {
    let mut lines = vec![
        format!("selector: {}", report.selector),
        format!("diagnostic_id: {}", report.diagnostic_id),
        format!("report: {}", report.report_path),
    ];
    if report.results.is_empty() {
        lines.push(String::from("no matching Gemma diagnostic rows"));
        return lines.join("\n");
    }
    for result in &report.results {
        match result.receipt.as_ref() {
            Some(receipt) => {
                let ttft = receipt
                    .mean_ttft_s
                    .map(|value| format!("{value:.3}s"))
                    .unwrap_or_else(|| String::from("n/a"));
                let decode = receipt
                    .mean_decode_tok_s
                    .map(|value| format!("{value:.2} tok/s"))
                    .unwrap_or_else(|| String::from("n/a"));
                let load = receipt
                    .load_s
                    .map(|value| format!("{value:.3}s"))
                    .unwrap_or_else(|| String::from("n/a"));
                lines.push(format!(
                    "{} {} total={:.3}s ttft={} tok/s={} load={} runtime={}",
                    result.model_id,
                    result.status,
                    receipt.mean_total_s,
                    ttft,
                    decode,
                    load,
                    receipt.runtime_model
                ));
            }
            None => lines.push(format!(
                "{} {} {}",
                result.model_id,
                result.status,
                result.reason.as_deref().unwrap_or("no detail")
            )),
        }
    }
    lines.join("\n")
}

fn default_psionic_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../psionic")
}

fn resolve_psionic_repo_root() -> Result<PathBuf> {
    let repo_root = std::env::var(ENV_PSIONIC_REPO)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_psionic_repo_root);
    let manifest_path = repo_root.join("Cargo.toml");
    if !manifest_path.exists() {
        bail!(
            "missing Psionic checkout at {}; clone ../psionic or set {}",
            repo_root.display(),
            ENV_PSIONIC_REPO
        );
    }
    let example_path = repo_root
        .join("crates")
        .join("psionic-serve")
        .join("examples")
        .join("gemma4_bench.rs");
    if !example_path.exists() {
        bail!(
            "Psionic checkout at {} does not contain crates/psionic-serve/examples/gemma4_bench.rs",
            repo_root.display()
        );
    }
    Ok(repo_root)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GemmaBenchExecutionMode {
    Single,
    DistributedDense,
    DistributedSparse,
}

impl GemmaBenchExecutionMode {
    const fn label(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::DistributedDense => "distributed-dense",
            Self::DistributedSparse => "distributed-sparse",
        }
    }
}

fn planned_gemma_benchmark_modes(
    spec: GemmaDownloadSpec,
    request: &GemmaBenchmarkRequest,
) -> Result<Vec<(GemmaBenchExecutionMode, Option<String>)>> {
    match request.mode {
        GemmaBenchmarkMode::Single => {
            if !spec.runtime_shape.supports_single_node() {
                bail!("{} does not support single-node execution", spec.id);
            }
            Ok(vec![(GemmaBenchExecutionMode::Single, None)])
        }
        GemmaBenchmarkMode::DistributedDense => {
            if !spec.runtime_shape.supports_dense_split() {
                bail!("{} does not support distributed dense execution", spec.id);
            }
            if request.peer_base_url.is_none() {
                bail!("distributed-dense benchmarks require --peer-base-url");
            }
            Ok(vec![(GemmaBenchExecutionMode::DistributedDense, None)])
        }
        GemmaBenchmarkMode::DistributedSparse => {
            if !spec.runtime_shape.supports_sparse_distributed() {
                bail!("{} does not support distributed sparse execution", spec.id);
            }
            if request.backend.is_some() {
                bail!("distributed-sparse benchmarks do not accept --backend");
            }
            Ok(vec![(GemmaBenchExecutionMode::DistributedSparse, None)])
        }
        GemmaBenchmarkMode::Matrix => {
            let mut plans = Vec::new();
            if spec.runtime_shape.supports_single_node() {
                plans.push((GemmaBenchExecutionMode::Single, None));
            }
            if spec.runtime_shape.supports_dense_split() {
                let reason = request
                    .peer_base_url
                    .is_none()
                    .then(|| String::from("distributed-dense requires --peer-base-url"));
                plans.push((GemmaBenchExecutionMode::DistributedDense, reason));
            }
            if spec.runtime_shape.supports_sparse_distributed() {
                plans.push((GemmaBenchExecutionMode::DistributedSparse, None));
            }
            Ok(plans)
        }
    }
}

async fn run_gemma_benchmark_command(
    config_path: &Path,
    selector: &GemmaBenchmarkSelector,
    request: &GemmaBenchmarkRequest,
) -> Result<GemmaBenchmarkReport> {
    let _ = ensure_local_setup(config_path)?;
    let psionic_repo = resolve_psionic_repo_root()?;
    let mut results = Vec::new();
    for spec in resolve_gemma_benchmark_selector(selector)? {
        let model_path = gemma_model_path(config_path, spec);
        if !model_path.exists() {
            if request.download_missing {
                let _ = download_gemma_model(config_path, spec.id, |_| {}).await?;
            }
            if !model_path.exists() {
                results.push(GemmaBenchmarkResult {
                    model_id: spec.id.to_string(),
                    label: spec.label.to_string(),
                    psionic_model_id: spec.psionic_model_id.to_string(),
                    runtime_shape: spec.runtime_shape.label().to_string(),
                    mode: request.mode.label().to_string(),
                    status: String::from("skipped"),
                    reason: Some(String::from("model is not installed")),
                    path: Some(model_path.display().to_string()),
                    command: None,
                    receipt: None,
                });
                continue;
            }
        }

        for (mode, skip_reason) in planned_gemma_benchmark_modes(spec, request)? {
            if let Some(reason) = skip_reason {
                results.push(GemmaBenchmarkResult {
                    model_id: spec.id.to_string(),
                    label: spec.label.to_string(),
                    psionic_model_id: spec.psionic_model_id.to_string(),
                    runtime_shape: spec.runtime_shape.label().to_string(),
                    mode: mode.label().to_string(),
                    status: String::from("skipped"),
                    reason: Some(reason),
                    path: Some(model_path.display().to_string()),
                    command: None,
                    receipt: None,
                });
                continue;
            }
            results.push(
                run_psionic_gemma_benchmark(
                    &psionic_repo,
                    spec,
                    model_path.as_path(),
                    mode,
                    request,
                )
                .await,
            );
        }
    }
    Ok(GemmaBenchmarkReport {
        selector: selector.label(),
        psionic_repo: psionic_repo.display().to_string(),
        results,
    })
}

async fn run_psionic_gemma_benchmark(
    psionic_repo: &Path,
    spec: GemmaDownloadSpec,
    model_path: &Path,
    mode: GemmaBenchExecutionMode,
    request: &GemmaBenchmarkRequest,
) -> GemmaBenchmarkResult {
    let receipt_path = std::env::temp_dir().join(format!(
        "pylon-gemma-bench-{}-{}-{}-{}.json",
        spec.id,
        mode.label(),
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    let args = psionic_gemma_benchmark_command_args(
        psionic_repo,
        model_path,
        mode,
        request,
        receipt_path.as_path(),
    );

    let output = TokioCommand::new("cargo")
        .args(args.iter())
        .current_dir(psionic_repo)
        .stdin(Stdio::null())
        .output()
        .await;
    let command = Some(args);
    match output {
        Err(error) => GemmaBenchmarkResult {
            model_id: spec.id.to_string(),
            label: spec.label.to_string(),
            psionic_model_id: spec.psionic_model_id.to_string(),
            runtime_shape: spec.runtime_shape.label().to_string(),
            mode: mode.label().to_string(),
            status: String::from("failed"),
            reason: Some(format!("failed to start cargo benchmark: {error}")),
            path: Some(model_path.display().to_string()),
            command,
            receipt: None,
        },
        Ok(output) if !output.status.success() => {
            let stderr = String::from_utf8_lossy(output.stderr.as_slice())
                .trim()
                .to_string();
            let stdout = String::from_utf8_lossy(output.stdout.as_slice())
                .trim()
                .to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("cargo exited with status {}", output.status)
            };
            let _ = tokio::fs::remove_file(receipt_path.as_path()).await;
            GemmaBenchmarkResult {
                model_id: spec.id.to_string(),
                label: spec.label.to_string(),
                psionic_model_id: spec.psionic_model_id.to_string(),
                runtime_shape: spec.runtime_shape.label().to_string(),
                mode: mode.label().to_string(),
                status: String::from("failed"),
                reason: Some(detail),
                path: Some(model_path.display().to_string()),
                command,
                receipt: None,
            }
        }
        Ok(_) => {
            let receipt = tokio::fs::read(receipt_path.as_path())
                .await
                .ok()
                .and_then(|bytes| {
                    serde_json::from_slice::<GemmaBenchReceipt>(bytes.as_slice()).ok()
                });
            let _ = tokio::fs::remove_file(receipt_path.as_path()).await;
            GemmaBenchmarkResult {
                model_id: spec.id.to_string(),
                label: spec.label.to_string(),
                psionic_model_id: spec.psionic_model_id.to_string(),
                runtime_shape: spec.runtime_shape.label().to_string(),
                mode: mode.label().to_string(),
                status: if receipt.is_some() {
                    String::from("completed")
                } else {
                    String::from("failed")
                },
                reason: receipt
                    .is_none()
                    .then(|| String::from("benchmark finished without a readable JSON receipt")),
                path: Some(model_path.display().to_string()),
                command,
                receipt,
            }
        }
    }
}

fn psionic_gemma_benchmark_command_args(
    psionic_repo: &Path,
    model_path: &Path,
    mode: GemmaBenchExecutionMode,
    request: &GemmaBenchmarkRequest,
    receipt_path: &Path,
) -> Vec<String> {
    let manifest_path = psionic_repo.join("Cargo.toml");
    let mut args = vec![
        String::from("run"),
        String::from("--quiet"),
        String::from("--manifest-path"),
        manifest_path.display().to_string(),
        String::from("-p"),
        String::from("psionic-serve"),
        String::from("--example"),
        String::from("gemma4_bench"),
        String::from("--"),
        String::from("--model-path"),
        model_path.display().to_string(),
        String::from("--mode"),
        mode.label().to_string(),
        String::from("--prompt"),
        request.prompt.clone(),
        String::from("--max-output-tokens"),
        request.max_output_tokens.to_string(),
        String::from("--repeats"),
        request.repeats.to_string(),
    ];
    if let Some(backend) = request.backend.as_ref() {
        args.push(String::from("--backend"));
        args.push(backend.clone());
    }
    if let Some(peer_base_url) = request.peer_base_url.as_ref() {
        args.push(String::from("--peer-base-url"));
        args.push(peer_base_url.clone());
    }
    if let Some(split_layer) = request.split_layer {
        args.push(String::from("--split-layer"));
        args.push(split_layer.to_string());
    }
    args.push(String::from("--json-out"));
    args.push(receipt_path.display().to_string());
    args
}

fn render_gemma_benchmark_report(report: &GemmaBenchmarkReport) -> String {
    let mut lines = vec![
        format!("selector: {}", report.selector),
        format!("psionic: {}", report.psionic_repo),
    ];
    if report.results.is_empty() {
        lines.push(String::from("no matching Gemma benchmark rows"));
        return lines.join("\n");
    }
    for result in &report.results {
        match result.receipt.as_ref() {
            Some(receipt) => {
                let ttft = receipt
                    .mean_ttft_s
                    .map(|value| format!("{value:.3}s"))
                    .unwrap_or_else(|| String::from("n/a"));
                let tok_s = receipt
                    .mean_decode_tok_s
                    .map(|value| format!("{value:.2} tok/s"))
                    .unwrap_or_else(|| String::from("n/a"));
                let topology = receipt.cluster_topology.as_deref().unwrap_or("single_node");
                lines.push(format!(
                    "{} {} {} total={:.3}s ttft={} tok/s={} backend={} topology={}",
                    result.model_id,
                    result.mode,
                    result.status,
                    receipt.mean_total_s,
                    ttft,
                    tok_s,
                    receipt.runtime_backend,
                    topology
                ));
            }
            None => lines.push(format!(
                "{} {} {} {}",
                result.model_id,
                result.mode,
                result.status,
                result.reason.as_deref().unwrap_or("no detail")
            )),
        }
    }
    lines.join("\n")
}

fn render_byte_size(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GiB", bytes as f64 / GIB)
    } else {
        format!("{:.2} MiB", bytes as f64 / MIB)
    }
}

pub fn parse_args(args: Vec<String>) -> Result<Cli> {
    if args.is_empty() {
        return Err(anyhow!("missing command"));
    }

    let mut index = 0usize;
    let mut config_path = default_config_path();
    while index < args.len() {
        match args[index].as_str() {
            "--config-path" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --config-path"))?;
                config_path = PathBuf::from(value);
                index += 1;
            }
            "--help" | "-h" => {
                return Err(anyhow!(usage()));
            }
            _ => break,
        }
    }

    let command = parse_command(args.as_slice(), index)?;
    Ok(Cli {
        command,
        config_path,
    })
}

pub async fn run_cli(cli: Cli) -> Result<Option<String>> {
    match cli.command {
        Command::Init => {
            let config = load_or_create_config(cli.config_path.as_path())?;
            let identity = ensure_identity(config.identity_path.as_path())?;
            let ledger_path = ensure_local_ledger(cli.config_path.as_path())?;
            Ok(Some(serde_json::to_string_pretty(&InitReport {
                config_path: cli.config_path.display().to_string(),
                ledger_path: ledger_path.display().to_string(),
                identity_path: config.identity_path.display().to_string(),
                npub: identity.npub,
                payout_destination: config.payout_destination.clone(),
                admin_listen_addr: config.admin_listen_addr.clone(),
            })?))
        }
        Command::Doctor => {
            let config = load_or_create_config(cli.config_path.as_path())?;
            let identity = ensure_identity(config.identity_path.as_path())?;
            let availability = detect_availability(&config).await?;
            let products =
                public_product_entries(products_from_availability(&config, &availability));
            Ok(Some(serde_json::to_string_pretty(&DoctorReport {
                config_path: cli.config_path.display().to_string(),
                node_label: config.node_label.clone(),
                payout_destination: config.payout_destination.clone(),
                identity: identity_metadata(&identity, config.node_label.as_str()),
                availability,
                products,
            })?))
        }
        Command::Serve => {
            let config = load_or_create_config(cli.config_path.as_path())?;
            serve(cli.config_path.as_path(), config).await?;
            Ok(None)
        }
        Command::Status { json } => {
            let status = load_status_or_detect(cli.config_path.as_path()).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&status)?));
            }
            Ok(Some(render_human_status(&status)))
        }
        Command::Backends { json } => {
            let report = load_backend_report(cli.config_path.as_path()).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_backend_report(&report)))
        }
        Command::Inventory { json, limit } => {
            let report = load_inventory_report(cli.config_path.as_path(), limit).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_inventory_report(&report)))
        }
        Command::Products { json } => {
            let report = load_product_report(cli.config_path.as_path()).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_product_report(&report)))
        }
        Command::Relays { json } => {
            let report = load_relay_report(cli.config_path.as_path())?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_relay_report(&report)))
        }
        Command::Sandbox { json, limit } => {
            let report = load_sandbox_report(cli.config_path.as_path(), limit).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_sandbox_report(&report)))
        }
        Command::Jobs { json, limit } => {
            let report = load_jobs_report(cli.config_path.as_path(), limit).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_jobs_report(&report)))
        }
        Command::Earnings { json } => {
            let report = load_earnings_report(cli.config_path.as_path()).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_earnings_report(&report)))
        }
        Command::Receipts { json, limit } => {
            let report = load_receipts_report(cli.config_path.as_path(), limit).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_receipts_report(&report)))
        }
        Command::Activity { json, limit } => {
            let report = load_relay_activity_report(cli.config_path.as_path(), limit)?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_relay_activity_report(&report)))
        }
        Command::RelayAdd { url } => {
            let report = add_configured_relay(cli.config_path.as_path(), url.as_str())?;
            Ok(Some(render_relay_report(&report)))
        }
        Command::RelayRemove { url } => {
            let report = remove_configured_relay(cli.config_path.as_path(), url.as_str())?;
            Ok(Some(render_relay_report(&report)))
        }
        Command::RelayRefresh { json } => {
            let report = refresh_relay_report(cli.config_path.as_path()).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_relay_report(&report)))
        }
        Command::Announcement { action, json } => {
            let report = match action {
                AnnouncementAction::Show => {
                    load_announcement_report(cli.config_path.as_path()).await?
                }
                AnnouncementAction::Publish => {
                    publish_announcement_report(cli.config_path.as_path(), false).await?
                }
                AnnouncementAction::Refresh => {
                    publish_announcement_report(cli.config_path.as_path(), true).await?
                }
            };
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_announcement_report(&report)))
        }
        Command::ProviderScan { seconds, json } => {
            let report = scan_provider_requests(cli.config_path.as_path(), seconds).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_provider_intake_report(&report)))
        }
        Command::ProviderRun { seconds, json } => {
            let report = run_provider_requests(cli.config_path.as_path(), seconds).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_provider_run_report(&report)))
        }
        Command::JobSubmit { request, json } => {
            let report = submit_buyer_job(cli.config_path.as_path(), request).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_buyer_job_submit_report(&report)))
        }
        Command::JobWatch {
            request_event_id,
            seconds,
            json,
        } => {
            let report = watch_buyer_jobs(
                cli.config_path.as_path(),
                request_event_id.as_deref(),
                seconds,
                |_| {},
            )
            .await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_buyer_job_watch_report(&report)))
        }
        Command::JobHistory { limit, json } => {
            let report = load_buyer_job_history(cli.config_path.as_path(), limit)?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_buyer_job_history_report(&report)))
        }
        Command::JobReplay {
            request_event_id,
            json,
        } => {
            let report =
                load_buyer_job_replay(cli.config_path.as_path(), request_event_id.as_str())?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_buyer_job_replay_report(&report)))
        }
        Command::JobApprove {
            request_event_id,
            json,
        } => {
            let report =
                approve_buyer_job_payment(cli.config_path.as_path(), request_event_id.as_str())
                    .await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_buyer_job_payment_report(&report)))
        }
        Command::JobDeny {
            request_event_id,
            json,
        } => {
            let report =
                deny_buyer_job_payment(cli.config_path.as_path(), request_event_id.as_str())?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_buyer_job_payment_report(&report)))
        }
        Command::JobPolicy { mode, json } => {
            let report = apply_buyer_payment_policy(cli.config_path.as_path(), mode)?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_buyer_payment_policy_report(&report)))
        }
        Command::Payout { limit, json } => {
            let report = load_payout_report(cli.config_path.as_path(), limit).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_payout_report(&report)))
        }
        Command::PayoutWithdraw {
            payment_request,
            amount_sats,
            json,
        } => {
            let report = run_payout_withdrawal(
                cli.config_path.as_path(),
                payment_request.as_str(),
                amount_sats,
            )
            .await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_payout_withdrawal_report(&report)))
        }
        Command::Wallet { command } => Ok(Some(
            run_wallet_command(cli.config_path.as_path(), &command).await?,
        )),
        Command::Gemma { command } => match command {
            GemmaCommand::List { json } => {
                let _ = ensure_local_setup(cli.config_path.as_path())?;
                let report = gemma_catalog_report(cli.config_path.as_path());
                if json {
                    return Ok(Some(serde_json::to_string_pretty(&report)?));
                }
                Ok(Some(render_gemma_catalog_report(&report)))
            }
            GemmaCommand::Download {
                selector,
                transport,
                json,
            } => {
                let report =
                    run_gemma_download_command(cli.config_path.as_path(), &selector, transport)
                        .await?;
                if json {
                    return Ok(Some(serde_json::to_string_pretty(&report)?));
                }
                Ok(Some(render_gemma_download_report(&report)))
            }
            GemmaCommand::Diagnose {
                selector,
                request,
                json,
            } => {
                let report =
                    run_gemma_diagnostic_command(cli.config_path.as_path(), &selector, &request)
                        .await?;
                if json {
                    return Ok(Some(serde_json::to_string_pretty(&report)?));
                }
                Ok(Some(render_gemma_diagnostic_report(&report)))
            }
            GemmaCommand::Benchmark {
                selector,
                request,
                json,
            } => {
                let report =
                    run_gemma_benchmark_command(cli.config_path.as_path(), &selector, &request)
                        .await?;
                if json {
                    return Ok(Some(serde_json::to_string_pretty(&report)?));
                }
                Ok(Some(render_gemma_benchmark_report(&report)))
            }
        },
        Command::Online => {
            let status =
                apply_control_command(cli.config_path.as_path(), ProviderControlAction::Online)
                    .await?;
            Ok(Some(render_human_status(&status)))
        }
        Command::Offline => {
            let status =
                apply_control_command(cli.config_path.as_path(), ProviderControlAction::Offline)
                    .await?;
            Ok(Some(render_human_status(&status)))
        }
        Command::Pause => {
            let status =
                apply_control_command(cli.config_path.as_path(), ProviderControlAction::Pause)
                    .await?;
            Ok(Some(render_human_status(&status)))
        }
        Command::Resume => {
            let status =
                apply_control_command(cli.config_path.as_path(), ProviderControlAction::Resume)
                    .await?;
            Ok(Some(render_human_status(&status)))
        }
        Command::ConfigShow => {
            let config = load_or_create_config(cli.config_path.as_path())?;
            Ok(Some(render_public_config_json(&config)?))
        }
        Command::ConfigSet { key, value } => {
            let mut config = load_or_create_config(cli.config_path.as_path())?;
            apply_config_set(&mut config, key.as_str(), value.as_str())?;
            save_config(cli.config_path.as_path(), &config)?;
            Ok(Some(render_public_config_json(&config)?))
        }
    }
}

pub fn usage() -> &'static str {
    "Standalone Pylon CLI.\n\
Bare `pylon` launches the terminal UI.\n\
Use the commands below for headless provider control.\n\
From this repo, run them with `cargo pylon-headless <command>` or invoke the `pylon` binary directly.\n\
\n\
Usage: pylon [--config-path <path>] <command>\n\
Commands:\n\
  init\n\
  doctor\n\
  serve\n\
  status [--json]\n\
  backends [--json]\n\
  inventory [--json] [--limit <n>]\n\
  products [--json]\n\
  relays [--json]\n\
  sandbox [--json] [--limit <n>]\n\
  jobs [--json] [--limit <n>]\n\
  earnings [--json]\n\
  receipts [--json] [--limit <n>]\n\
  activity [--json] [--limit <n>]\n\
  relay add <url>\n\
  relay remove <url>\n\
  relay refresh [--json]\n\
  announce [show|publish|refresh] [--json]\n\
  provider scan [--seconds <n>] [--json]\n\
  provider run [--seconds <n>] [--json]\n\
  job submit [--bid-msats <n>] [--model <id>] [--provider <pubkey>] [--output <mime>] [--request-json <json>] <prompt> [--json]\n\
  job watch [<request_event_id>] [--seconds <n>] [--json]\n\
  job history [--limit <n>] [--json]\n\
  job replay <request_event_id> [--json]\n\
  job approve <request_event_id> [--json]\n\
  job deny <request_event_id> [--json]\n\
  job policy [show|auto|manual] [--json]\n\
  payout [--limit <n>] [--json]\n\
  payout withdraw <payment_request> [--amount-sats <n>] [--json]\n\
  wallet status [--json]\n\
  wallet balance [--json]\n\
  wallet address [--json]\n\
  wallet invoice <amount_sats> [--description <text>] [--expiry-seconds <n>] [--json]\n\
  wallet pay <payment_request> [--amount-sats <n>] [--json]\n\
  wallet history [--limit <n>] [--json]\n\
  gemma [list] [--json]\n\
  gemma download <model|all|remaining> [--transport auto|reqwest|curl] [--json]\n\
  gemma diagnose <model|all> [--max-output-tokens <n>] [--repeats <n>] [--download-missing] [--json]\n\
  gemma benchmark <model|all> [--mode single|distributed-dense|distributed-sparse|matrix] [--backend auto|metal|cuda] [--peer-base-url <url>] [--split-layer <n>] [--prompt <text>] [--max-output-tokens <n>] [--repeats <n>] [--download-missing] [--json]\n\
  online\n\
  offline\n\
  pause\n\
  resume\n\
  config show\n\
  config set <key> <value>\n"
}

fn parse_command(args: &[String], start_index: usize) -> Result<Command> {
    let command = args
        .get(start_index)
        .ok_or_else(|| anyhow!("missing command"))?;
    match command.as_str() {
        "init" => {
            if start_index + 1 != args.len() {
                bail!("init does not accept positional arguments");
            }
            Ok(Command::Init)
        }
        "doctor" => {
            if start_index + 1 != args.len() {
                bail!("doctor does not accept positional arguments");
            }
            Ok(Command::Doctor)
        }
        "serve" => {
            if start_index + 1 != args.len() {
                bail!("serve does not accept positional arguments");
            }
            Ok(Command::Serve)
        }
        "status" => {
            let json = match args.get(start_index + 1) {
                None => false,
                Some(value) if value == "--json" => true,
                Some(other) => bail!("unexpected argument for status: {other}"),
            };
            if json && start_index + 2 != args.len() {
                bail!("status --json does not accept additional arguments");
            }
            if !json && start_index + 1 != args.len() {
                bail!("status does not accept additional arguments");
            }
            Ok(Command::Status { json })
        }
        "backends" => {
            let (json, limit) =
                parse_observability_flags(args, start_index + 1, "backends", false)?;
            if limit.is_some() {
                bail!("backends does not support --limit");
            }
            Ok(Command::Backends { json })
        }
        "inventory" => {
            let (json, limit) =
                parse_observability_flags(args, start_index + 1, "inventory", true)?;
            Ok(Command::Inventory { json, limit })
        }
        "products" => {
            let (json, limit) =
                parse_observability_flags(args, start_index + 1, "products", false)?;
            if limit.is_some() {
                bail!("products does not support --limit");
            }
            Ok(Command::Products { json })
        }
        "relays" => {
            let (json, limit) = parse_observability_flags(args, start_index + 1, "relays", false)?;
            if limit.is_some() {
                bail!("relays does not support --limit");
            }
            Ok(Command::Relays { json })
        }
        "sandbox" => {
            let (json, limit) = parse_observability_flags(args, start_index + 1, "sandbox", true)?;
            Ok(Command::Sandbox { json, limit })
        }
        "jobs" => {
            let (json, limit) = parse_observability_flags(args, start_index + 1, "jobs", true)?;
            Ok(Command::Jobs { json, limit })
        }
        "earnings" => {
            let (json, limit) =
                parse_observability_flags(args, start_index + 1, "earnings", false)?;
            if limit.is_some() {
                bail!("earnings does not support --limit");
            }
            Ok(Command::Earnings { json })
        }
        "receipts" => {
            let (json, limit) = parse_observability_flags(args, start_index + 1, "receipts", true)?;
            Ok(Command::Receipts { json, limit })
        }
        "activity" => {
            let (json, limit) = parse_observability_flags(args, start_index + 1, "activity", true)?;
            Ok(Command::Activity { json, limit })
        }
        "relay" => match args.get(start_index + 1).map(String::as_str) {
            Some("add") => {
                let url = args
                    .get(start_index + 2)
                    .ok_or_else(|| anyhow!("missing <url> for relay add"))?;
                if start_index + 3 != args.len() {
                    bail!("relay add accepts exactly <url>");
                }
                Ok(Command::RelayAdd { url: url.clone() })
            }
            Some("remove") => {
                let url = args
                    .get(start_index + 2)
                    .ok_or_else(|| anyhow!("missing <url> for relay remove"))?;
                if start_index + 3 != args.len() {
                    bail!("relay remove accepts exactly <url>");
                }
                Ok(Command::RelayRemove { url: url.clone() })
            }
            Some("refresh") => {
                let json = match args.get(start_index + 2) {
                    None => false,
                    Some(value) if value == "--json" => true,
                    Some(other) => bail!("unexpected argument for relay refresh: {other}"),
                };
                if json && start_index + 3 != args.len() {
                    bail!("relay refresh --json does not accept additional arguments");
                }
                if !json && start_index + 2 != args.len() {
                    bail!("relay refresh does not accept additional arguments");
                }
                Ok(Command::RelayRefresh { json })
            }
            Some(other) => bail!("unknown relay command: {other}"),
            None => bail!("missing relay subcommand"),
        },
        "announce" => match args.get(start_index + 1).map(String::as_str) {
            None => Ok(Command::Announcement {
                action: AnnouncementAction::Show,
                json: false,
            }),
            Some("show") => Ok(Command::Announcement {
                action: AnnouncementAction::Show,
                json: parse_json_only(args, start_index + 2, "announce show")?,
            }),
            Some("publish") => Ok(Command::Announcement {
                action: AnnouncementAction::Publish,
                json: parse_json_only(args, start_index + 2, "announce publish")?,
            }),
            Some("refresh") => Ok(Command::Announcement {
                action: AnnouncementAction::Refresh,
                json: parse_json_only(args, start_index + 2, "announce refresh")?,
            }),
            Some("--json") => {
                if start_index + 2 != args.len() {
                    bail!("announce --json does not accept additional arguments");
                }
                Ok(Command::Announcement {
                    action: AnnouncementAction::Show,
                    json: true,
                })
            }
            Some(other) => bail!("unknown announce command: {other}"),
        },
        "provider" => match args.get(start_index + 1).map(String::as_str) {
            Some("scan") => {
                let (json, seconds) =
                    parse_provider_scan_flags(args, start_index + 2, "provider scan")?;
                Ok(Command::ProviderScan {
                    seconds: seconds.unwrap_or(5),
                    json,
                })
            }
            Some("run") => {
                let (json, seconds) =
                    parse_provider_scan_flags(args, start_index + 2, "provider run")?;
                Ok(Command::ProviderRun {
                    seconds: seconds.unwrap_or(5),
                    json,
                })
            }
            Some(other) => bail!("unknown provider command: {other}"),
            None => bail!("missing provider subcommand"),
        },
        "job" => match args.get(start_index + 1).map(String::as_str) {
            Some("submit") => {
                let (request, json) = parse_job_submit_command(args, start_index + 2)?;
                Ok(Command::JobSubmit { request, json })
            }
            Some("watch") => {
                let (request_event_id, seconds, json) =
                    parse_job_watch_command(args, start_index + 2)?;
                Ok(Command::JobWatch {
                    request_event_id,
                    seconds,
                    json,
                })
            }
            Some("history") => {
                let (limit, json) = parse_job_history_command(args, start_index + 2)?;
                Ok(Command::JobHistory { limit, json })
            }
            Some("replay") => {
                let (request_event_id, json) =
                    parse_job_request_id_with_json(args, start_index + 2, "job replay")?;
                Ok(Command::JobReplay {
                    request_event_id,
                    json,
                })
            }
            Some("approve") => {
                let (request_event_id, json) =
                    parse_job_request_id_with_json(args, start_index + 2, "job approve")?;
                Ok(Command::JobApprove {
                    request_event_id,
                    json,
                })
            }
            Some("deny") => {
                let (request_event_id, json) =
                    parse_job_request_id_with_json(args, start_index + 2, "job deny")?;
                Ok(Command::JobDeny {
                    request_event_id,
                    json,
                })
            }
            Some("policy") => {
                let (mode, json) = parse_job_policy_command(args, start_index + 2)?;
                Ok(Command::JobPolicy { mode, json })
            }
            Some(other) => bail!("unknown job command: {other}"),
            None => bail!("missing job subcommand"),
        },
        "payout" => match args.get(start_index + 1).map(String::as_str) {
            None => {
                let (limit, json) = parse_payout_flags(args, start_index + 1, "payout")?;
                Ok(Command::Payout { limit, json })
            }
            Some("withdraw") => {
                let (payment_request, amount_sats, json) =
                    parse_payout_withdraw_command(args, start_index + 2)?;
                Ok(Command::PayoutWithdraw {
                    payment_request,
                    amount_sats,
                    json,
                })
            }
            Some(value) if value.starts_with("--") => {
                let (limit, json) = parse_payout_flags(args, start_index + 1, "payout")?;
                Ok(Command::Payout { limit, json })
            }
            Some(other) => bail!("unknown payout command: {other}"),
        },
        "wallet" => Ok(Command::Wallet {
            command: parse_wallet_command(args, start_index)?,
        }),
        "gemma" => parse_gemma_command(args, start_index + 1),
        "online" => {
            if start_index + 1 != args.len() {
                bail!("online does not accept positional arguments");
            }
            Ok(Command::Online)
        }
        "offline" => {
            if start_index + 1 != args.len() {
                bail!("offline does not accept positional arguments");
            }
            Ok(Command::Offline)
        }
        "pause" => {
            if start_index + 1 != args.len() {
                bail!("pause does not accept positional arguments");
            }
            Ok(Command::Pause)
        }
        "resume" => {
            if start_index + 1 != args.len() {
                bail!("resume does not accept positional arguments");
            }
            Ok(Command::Resume)
        }
        "config" => match args.get(start_index + 1).map(String::as_str) {
            Some("show") => {
                if start_index + 2 != args.len() {
                    bail!("config show does not accept additional arguments");
                }
                Ok(Command::ConfigShow)
            }
            Some("set") => {
                let key = args
                    .get(start_index + 2)
                    .ok_or_else(|| anyhow!("missing <key> for config set"))?;
                let value = args
                    .get(start_index + 3)
                    .ok_or_else(|| anyhow!("missing <value> for config set"))?;
                if start_index + 4 != args.len() {
                    bail!("config set accepts exactly <key> <value>");
                }
                Ok(Command::ConfigSet {
                    key: key.clone(),
                    value: value.clone(),
                })
            }
            Some(other) => bail!("unknown config command: {other}"),
            None => bail!("missing config subcommand"),
        },
        other => bail!("unknown command: {other}"),
    }
}

fn parse_observability_flags(
    args: &[String],
    mut index: usize,
    command: &str,
    allow_limit: bool,
) -> Result<(bool, Option<usize>)> {
    let mut json = false;
    let mut limit = None;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--limit" if allow_limit => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --limit"))?;
                limit =
                    Some(value.parse::<usize>().with_context(|| {
                        format!("invalid numeric limit for {command}: {value}")
                    })?);
                index += 1;
            }
            other => bail!("unexpected argument for {command}: {other}"),
        }
    }
    Ok((json, limit))
}

fn parse_json_only(args: &[String], start_index: usize, command: &str) -> Result<bool> {
    match args.get(start_index) {
        None => Ok(false),
        Some(value) if value == "--json" => {
            if start_index + 1 != args.len() {
                bail!("{command} --json does not accept additional arguments");
            }
            Ok(true)
        }
        Some(other) => bail!("unexpected argument for {command}: {other}"),
    }
}

fn parse_gemma_command(args: &[String], start_index: usize) -> Result<Command> {
    match args.get(start_index).map(String::as_str) {
        None => Ok(Command::Gemma {
            command: GemmaCommand::List { json: false },
        }),
        Some("list") => Ok(Command::Gemma {
            command: GemmaCommand::List {
                json: parse_json_only(args, start_index + 1, "gemma list")?,
            },
        }),
        Some("download") => {
            let (selector, transport, json) = parse_gemma_download_command(args, start_index + 1)?;
            Ok(Command::Gemma {
                command: GemmaCommand::Download {
                    selector,
                    transport,
                    json,
                },
            })
        }
        Some("diagnose") => {
            let (selector, request, json) = parse_gemma_diagnostic_command(args, start_index + 1)?;
            Ok(Command::Gemma {
                command: GemmaCommand::Diagnose {
                    selector,
                    request,
                    json,
                },
            })
        }
        Some("benchmark") => {
            let (selector, request, json) = parse_gemma_benchmark_command(args, start_index + 1)?;
            Ok(Command::Gemma {
                command: GemmaCommand::Benchmark {
                    selector,
                    request,
                    json,
                },
            })
        }
        Some("--json") => {
            if start_index + 1 != args.len() {
                bail!("gemma --json does not accept additional arguments");
            }
            Ok(Command::Gemma {
                command: GemmaCommand::List { json: true },
            })
        }
        Some(other) => bail!("unknown gemma command: {other}"),
    }
}

fn parse_gemma_download_command(
    args: &[String],
    mut index: usize,
) -> Result<(GemmaSelector, GemmaDownloadTransport, bool)> {
    let selector = parse_gemma_selector(
        args.get(index)
            .ok_or_else(|| anyhow!("missing <model|all|remaining> for gemma download"))?
            .as_str(),
    )?;
    index += 1;
    let mut transport = GemmaDownloadTransport::Auto;
    let mut json = false;
    while index < args.len() {
        match args[index].as_str() {
            "--transport" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --transport"))?;
                transport = match value.as_str() {
                    "auto" => GemmaDownloadTransport::Auto,
                    "reqwest" => GemmaDownloadTransport::Reqwest,
                    "curl" => GemmaDownloadTransport::Curl,
                    other => {
                        bail!(
                            "unsupported Gemma download transport `{other}`; expected one of: auto, reqwest, curl"
                        );
                    }
                };
                index += 1;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            other => bail!("unexpected argument for gemma download: {other}"),
        }
    }
    Ok((selector, transport, json))
}

fn parse_gemma_selector(value: &str) -> Result<GemmaSelector> {
    match value.trim() {
        "all" => Ok(GemmaSelector::All),
        "remaining" => Ok(GemmaSelector::Remaining),
        model_id => {
            if gemma_download_spec(model_id).is_none() {
                bail!("unknown Gemma model `{model_id}`");
            }
            Ok(GemmaSelector::Model(model_id.to_string()))
        }
    }
}

fn parse_gemma_benchmark_selector(value: &str) -> Result<GemmaBenchmarkSelector> {
    match value.trim() {
        "all" => Ok(GemmaBenchmarkSelector::All),
        model_id => {
            if gemma_download_spec(model_id).is_none() {
                bail!("unknown Gemma model `{model_id}`");
            }
            Ok(GemmaBenchmarkSelector::Model(model_id.to_string()))
        }
    }
}

fn parse_gemma_diagnostic_command(
    args: &[String],
    mut index: usize,
) -> Result<(GemmaBenchmarkSelector, GemmaDiagnosticRequest, bool)> {
    let selector = parse_gemma_benchmark_selector(
        args.get(index)
            .ok_or_else(|| anyhow!("missing <model|all> for gemma diagnose"))?
            .as_str(),
    )?;
    index += 1;
    let mut json = false;
    let mut max_output_tokens = 96usize;
    let mut repeats = 3usize;
    let mut download_missing = false;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--max-output-tokens" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --max-output-tokens"))?;
                max_output_tokens = value
                    .parse::<usize>()
                    .with_context(|| format!("invalid Gemma max output tokens: {value}"))?;
                index += 1;
            }
            "--repeats" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --repeats"))?;
                repeats = value
                    .parse::<usize>()
                    .with_context(|| format!("invalid Gemma diagnostic repeats: {value}"))?;
                index += 1;
            }
            "--download-missing" => {
                download_missing = true;
                index += 1;
            }
            other => bail!("unexpected argument for gemma diagnose: {other}"),
        }
    }
    if repeats == 0 {
        bail!("Gemma diagnostic repeats must be at least 1");
    }
    Ok((
        selector,
        GemmaDiagnosticRequest {
            diagnostic_id: DEFAULT_GEMMA_DIAGNOSTIC_ID.to_string(),
            prompt: DEFAULT_GEMMA_BENCH_PROMPT.to_string(),
            max_output_tokens,
            repeats,
            download_missing,
        },
        json,
    ))
}

fn parse_gemma_benchmark_command(
    args: &[String],
    mut index: usize,
) -> Result<(GemmaBenchmarkSelector, GemmaBenchmarkRequest, bool)> {
    let selector = parse_gemma_benchmark_selector(
        args.get(index)
            .ok_or_else(|| anyhow!("missing <model|all> for gemma benchmark"))?
            .as_str(),
    )?;
    index += 1;
    let mut json = false;
    let mut mode = GemmaBenchmarkMode::Matrix;
    let mut backend = None::<String>;
    let mut peer_base_url = None::<String>;
    let mut split_layer = None::<usize>;
    let mut prompt = String::from(DEFAULT_GEMMA_BENCH_PROMPT);
    let mut max_output_tokens = 96usize;
    let mut repeats = 3usize;
    let mut download_missing = false;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--mode" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --mode"))?;
                mode = GemmaBenchmarkMode::parse(value.as_str())?;
                index += 1;
            }
            "--backend" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --backend"))?;
                match value.as_str() {
                    "auto" | "metal" | "cuda" => backend = Some(value.clone()),
                    other => bail!(
                        "unsupported Gemma benchmark backend `{other}`; expected one of: auto, metal, cuda"
                    ),
                }
                index += 1;
            }
            "--peer-base-url" => {
                index += 1;
                peer_base_url = Some(
                    args.get(index)
                        .ok_or_else(|| anyhow!("missing value for --peer-base-url"))?
                        .clone(),
                );
                index += 1;
            }
            "--split-layer" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --split-layer"))?;
                split_layer = Some(
                    value
                        .parse::<usize>()
                        .with_context(|| format!("invalid Gemma split layer: {value}"))?,
                );
                index += 1;
            }
            "--prompt" => {
                index += 1;
                prompt = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --prompt"))?
                    .clone();
                index += 1;
            }
            "--max-output-tokens" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --max-output-tokens"))?;
                max_output_tokens = value
                    .parse::<usize>()
                    .with_context(|| format!("invalid Gemma max output tokens: {value}"))?;
                index += 1;
            }
            "--repeats" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --repeats"))?;
                repeats = value
                    .parse::<usize>()
                    .with_context(|| format!("invalid Gemma benchmark repeats: {value}"))?;
                index += 1;
            }
            "--download-missing" => {
                download_missing = true;
                index += 1;
            }
            other => bail!("unexpected argument for gemma benchmark: {other}"),
        }
    }
    if repeats == 0 {
        bail!("Gemma benchmark repeats must be at least 1");
    }
    if matches!(mode, GemmaBenchmarkMode::DistributedDense) && peer_base_url.is_none() {
        bail!("distributed-dense benchmarks require --peer-base-url");
    }
    if matches!(mode, GemmaBenchmarkMode::DistributedSparse) && backend.is_some() {
        bail!("distributed-sparse benchmarks do not accept --backend");
    }
    Ok((
        selector,
        GemmaBenchmarkRequest {
            mode,
            backend,
            peer_base_url,
            split_layer,
            prompt,
            max_output_tokens,
            repeats,
            download_missing,
        },
        json,
    ))
}

fn parse_job_submit_command(
    args: &[String],
    mut index: usize,
) -> Result<(BuyerJobSubmitRequest, bool)> {
    let mut prompt_parts = Vec::new();
    let mut request_json = None::<String>;
    let mut bid_msats = None::<u64>;
    let mut model = None::<String>;
    let mut provider_pubkey = None::<String>;
    let mut output_mime = None::<String>;
    let mut json = false;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--request-json" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --request-json"))?;
                request_json = Some(value.clone());
                index += 1;
            }
            "--bid-msats" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --bid-msats"))?;
                bid_msats = Some(
                    value
                        .parse::<u64>()
                        .with_context(|| format!("invalid buyer bid millisats: {value}"))?,
                );
                index += 1;
            }
            "--model" => {
                index += 1;
                model = Some(
                    args.get(index)
                        .ok_or_else(|| anyhow!("missing value for --model"))?
                        .clone(),
                );
                index += 1;
            }
            "--provider" => {
                index += 1;
                provider_pubkey = Some(
                    args.get(index)
                        .ok_or_else(|| anyhow!("missing value for --provider"))?
                        .clone(),
                );
                index += 1;
            }
            "--output" => {
                index += 1;
                output_mime = Some(
                    args.get(index)
                        .ok_or_else(|| anyhow!("missing value for --output"))?
                        .clone(),
                );
                index += 1;
            }
            value => {
                prompt_parts.push(value.to_string());
                index += 1;
            }
        }
    }
    let prompt = (!prompt_parts.is_empty()).then(|| prompt_parts.join(" "));
    if prompt.is_none() && request_json.is_none() {
        bail!("job submit requires prompt text or --request-json");
    }
    if prompt.is_some() && request_json.is_some() {
        bail!("job submit accepts either prompt text or --request-json, not both");
    }
    Ok((
        BuyerJobSubmitRequest {
            prompt,
            request_json,
            bid_msats,
            model,
            provider_pubkey,
            output_mime,
        },
        json,
    ))
}

fn parse_job_watch_command(
    args: &[String],
    mut index: usize,
) -> Result<(Option<String>, u64, bool)> {
    let mut request_event_id = None::<String>;
    let mut seconds = 30u64;
    let mut json = false;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--seconds" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --seconds"))?;
                seconds = value
                    .parse::<u64>()
                    .with_context(|| format!("invalid buyer watch seconds: {value}"))?;
                index += 1;
            }
            value if value.starts_with("--") => {
                bail!("unexpected argument for job watch: {value}");
            }
            value => {
                if request_event_id.is_some() {
                    bail!("job watch accepts at most one <request_event_id>");
                }
                request_event_id = Some(value.to_string());
                index += 1;
            }
        }
    }
    Ok((request_event_id, seconds.max(1), json))
}

fn parse_job_history_command(args: &[String], mut index: usize) -> Result<(Option<usize>, bool)> {
    let mut limit = None;
    let mut json = false;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--limit" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --limit"))?;
                limit = Some(
                    value
                        .parse::<usize>()
                        .with_context(|| format!("invalid buyer history limit: {value}"))?,
                );
                index += 1;
            }
            other => bail!("unexpected argument for job history: {other}"),
        }
    }
    Ok((limit, json))
}

fn parse_job_request_id_with_json(
    args: &[String],
    start_index: usize,
    command: &str,
) -> Result<(String, bool)> {
    let request_event_id = args
        .get(start_index)
        .ok_or_else(|| anyhow!("missing <request_event_id> for {command}"))?
        .clone();
    let json = parse_json_only(args, start_index + 1, command)?;
    Ok((request_event_id, json))
}

fn parse_job_policy_command(
    args: &[String],
    start_index: usize,
) -> Result<(BuyerPaymentPolicyMode, bool)> {
    match args.get(start_index).map(String::as_str) {
        None => Ok((BuyerPaymentPolicyMode::Show, false)),
        Some("show") => Ok((
            BuyerPaymentPolicyMode::Show,
            parse_json_only(args, start_index + 1, "job policy show")?,
        )),
        Some("auto") => Ok((
            BuyerPaymentPolicyMode::Auto,
            parse_json_only(args, start_index + 1, "job policy auto")?,
        )),
        Some("manual") => Ok((
            BuyerPaymentPolicyMode::Manual,
            parse_json_only(args, start_index + 1, "job policy manual")?,
        )),
        Some("--json") => {
            if start_index + 1 != args.len() {
                bail!("job policy --json does not accept additional arguments");
            }
            Ok((BuyerPaymentPolicyMode::Show, true))
        }
        Some(other) => bail!("unknown job policy mode: {other}"),
    }
}

fn parse_payout_flags(
    args: &[String],
    mut index: usize,
    command: &str,
) -> Result<(Option<u32>, bool)> {
    let mut limit = None;
    let mut json = false;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--limit" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --limit"))?;
                limit = Some(
                    value
                        .parse::<u32>()
                        .with_context(|| format!("invalid {command} limit: {value}"))?,
                );
                index += 1;
            }
            other => bail!("unexpected argument for {command}: {other}"),
        }
    }
    Ok((limit, json))
}

fn parse_payout_withdraw_command(
    args: &[String],
    mut index: usize,
) -> Result<(String, Option<u64>, bool)> {
    let payment_request = args
        .get(index)
        .ok_or_else(|| anyhow!("missing <payment_request> for payout withdraw"))?
        .clone();
    index += 1;
    let mut amount_sats = None;
    let mut json = false;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--amount-sats" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --amount-sats"))?;
                amount_sats =
                    Some(value.parse::<u64>().with_context(|| {
                        format!("invalid payout withdraw amount_sats: {value}")
                    })?);
                index += 1;
            }
            other => bail!("unexpected argument for payout withdraw: {other}"),
        }
    }
    Ok((payment_request, amount_sats, json))
}

fn parse_provider_scan_flags(
    args: &[String],
    mut index: usize,
    command: &str,
) -> Result<(bool, Option<u64>)> {
    let mut json = false;
    let mut seconds = None;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--seconds" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --seconds"))?;
                seconds = Some(
                    value
                        .parse::<u64>()
                        .with_context(|| format!("invalid seconds for {command}: {value}"))?,
                );
                index += 1;
            }
            other => bail!("unexpected argument for {command}: {other}"),
        }
    }
    Ok((json, seconds))
}

fn load_or_create_config(path: &Path) -> Result<PylonConfig> {
    if path.exists() {
        return load_config(path);
    }
    let base_dir = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(default_home_dir);
    let config = default_config(base_dir.as_path());
    save_config(path, &config)?;
    Ok(config)
}

pub fn ensure_local_setup(config_path: &Path) -> Result<PylonConfig> {
    let config = load_or_create_config(config_path)?;
    let _ = ensure_identity(config.identity_path.as_path())?;
    let _ = ensure_local_ledger(config_path)?;
    Ok(config)
}

fn load_config(path: &Path) -> Result<PylonConfig> {
    let payload = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read pylon config {}", path.display()))?;
    let base_dir = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(default_home_dir);
    let mut merged = serde_json::to_value(default_config(base_dir.as_path()))
        .context("failed to serialize default pylon config")?;
    let mut parsed = serde_json::from_str::<Value>(payload.as_str())
        .with_context(|| format!("failed to parse pylon config {}", path.display()))?;
    normalize_legacy_config_value(&mut parsed);
    merge_json_value(&mut merged, &parsed);
    serde_json::from_value(merged)
        .with_context(|| format!("failed to hydrate pylon config {}", path.display()))
}

fn load_config_required(path: &Path) -> Result<PylonConfig> {
    if !path.exists() {
        bail!("pylon is unconfigured; run `pylon init` first");
    }
    load_config(path)
}

fn save_config(path: &Path, config: &PylonConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create pylon config dir {}", parent.display()))?;
    }
    std::fs::write(path, format!("{}\n", render_public_config_json(config)?))
        .with_context(|| format!("failed to write pylon config {}", path.display()))?;
    Ok(())
}

fn render_public_config_json(config: &PylonConfig) -> Result<String> {
    Ok(serde_json::to_string_pretty(&PylonPublicConfig::from(
        config,
    ))?)
}

fn merge_json_value(target: &mut Value, source: &Value) {
    match (target, source) {
        (Value::Object(target), Value::Object(source)) => {
            for (key, value) in source {
                match target.get_mut(key) {
                    Some(existing) => merge_json_value(existing, value),
                    None => {
                        target.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (target, source) => {
            *target = source.clone();
        }
    }
}

fn normalize_legacy_config_value(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };

    if !object.contains_key("local_gemma_base_url") {
        if let Some(legacy_value) = object.remove("ollama_base_url") {
            object.insert("local_gemma_base_url".to_string(), legacy_value);
        }
    } else {
        object.remove("ollama_base_url");
    }

    if let Some(inventory_controls) = object
        .get_mut("inventory_controls")
        .and_then(Value::as_object_mut)
    {
        normalize_legacy_object_key(
            inventory_controls,
            "local_gemma_inference_enabled",
            &["gpt_oss_inference_enabled", "ollama_inference_enabled"],
        );
        normalize_legacy_object_key(
            inventory_controls,
            "local_gemma_embeddings_enabled",
            &["gpt_oss_embeddings_enabled", "ollama_embeddings_enabled"],
        );
    }
}

fn normalize_legacy_object_key(
    object: &mut serde_json::Map<String, Value>,
    canonical_key: &str,
    legacy_keys: &[&str],
) {
    if object.contains_key(canonical_key) {
        for legacy_key in legacy_keys {
            object.remove(*legacy_key);
        }
        return;
    }

    for legacy_key in legacy_keys {
        if let Some(value) = object.remove(*legacy_key) {
            object.insert(canonical_key.to_string(), value);
            break;
        }
    }
}

fn default_config(base_dir: &Path) -> PylonConfig {
    let mut inventory_controls = ProviderInventoryControls::default();
    inventory_controls.apple_fm_inference_enabled = false;
    inventory_controls.apple_fm_adapter_hosting_enabled = false;
    PylonConfig {
        schema_version: 1,
        node_label: "pylon".to_string(),
        payout_destination: None,
        identity_path: base_dir.join("identity.mnemonic"),
        admin_db_path: base_dir.join("provider-admin.sqlite"),
        admin_listen_addr: "127.0.0.1:9468".to_string(),
        nexus_control_base_url: default_nexus_control_base_url(),
        relay_urls: default_relay_urls(),
        relay_connect_timeout_seconds: default_relay_connect_timeout_seconds(),
        relay_auth_enabled: default_relay_auth_enabled(),
        wallet_network: default_wallet_network(),
        wallet_api_key_env: default_wallet_api_key_env(),
        buyer_auto_pay_enabled: default_buyer_auto_pay_enabled(),
        wallet_storage_dir: base_dir.join("spark"),
        local_gemma_base_url: "http://127.0.0.1:11434".to_string(),
        apple_fm_base_url: None,
        inventory_controls,
        declared_sandbox_profiles: Vec::new(),
    }
}

fn default_relay_urls() -> Vec<String> {
    vec![
        "wss://nexus.openagents.com".to_string(),
        "wss://relay.damus.io".to_string(),
        "wss://nos.lol".to_string(),
    ]
}

fn default_nexus_control_base_url() -> String {
    "https://nexus.openagents.com".to_string()
}

const fn default_relay_connect_timeout_seconds() -> u64 {
    10
}

const fn default_relay_auth_enabled() -> bool {
    true
}

fn default_wallet_network() -> String {
    "mainnet".to_string()
}

fn default_wallet_api_key_env() -> Option<String> {
    Some("OPENAGENTS_SPARK_API_KEY".to_string())
}

const fn default_buyer_auto_pay_enabled() -> bool {
    false
}

pub fn provider_presence_heartbeat_interval() -> Duration {
    Duration::from_millis(DEFAULT_PROVIDER_PRESENCE_HEARTBEAT_INTERVAL_MS)
}

pub fn new_provider_presence_session_id() -> String {
    format!("pylon_{}", random_token())
}

pub fn default_config_path() -> PathBuf {
    if let Ok(path) = std::env::var(ENV_PYLON_CONFIG_PATH) {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    default_home_dir().join("config.json")
}

fn default_home_dir() -> PathBuf {
    if let Ok(path) = std::env::var(ENV_PYLON_HOME) {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("pylon")
}

fn ensure_identity(path: &Path) -> Result<NostrIdentity> {
    if path.exists() {
        return load_identity_from_path(path);
    }
    let entropy: [u8; 16] = rand::random();
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
        .context("failed to generate pylon mnemonic")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create identity dir {}", parent.display()))?;
    }
    std::fs::write(path, format!("{mnemonic}\n"))
        .with_context(|| format!("failed to write identity file {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to set identity permissions {}", path.display()))?;
    }
    let keypair = derive_keypair(mnemonic.to_string().as_str())
        .context("failed to derive pylon nostr identity")?;
    Ok(NostrIdentity {
        identity_path: path.to_path_buf(),
        mnemonic: mnemonic.to_string(),
        npub: keypair.npub()?,
        nsec: keypair.nsec()?,
        public_key_hex: keypair.public_key_hex(),
        private_key_hex: keypair.private_key_hex(),
    })
}

async fn serve(config_path: &Path, config: PylonConfig) -> Result<()> {
    let admin_config = provider_admin_config(&config)?;
    let mut desired_mode = ProviderPersistenceStore::open(&admin_config)
        .map_err(anyhow::Error::msg)?
        .desired_mode()
        .map_err(anyhow::Error::msg)?;
    let mut runtime = ProviderAdminRuntime::spawn(admin_config).map_err(anyhow::Error::msg)?;
    let presence_client = provider_presence_client()?;
    let identity = ensure_identity(config.identity_path.as_path())?;
    let provider_presence_session_id = new_provider_presence_session_id();
    let provider_presence_heartbeat_interval = provider_presence_heartbeat_interval();
    let mut next_provider_presence_heartbeat_at = Instant::now();
    let mut provider_presence_online = false;
    let mut previous_snapshot = None::<ProviderPersistedSnapshot>;
    let mut needs_sync = true;
    loop {
        for update in runtime.drain_updates() {
            match update {
                ProviderAdminUpdate::ControlEvent(event) => {
                    desired_mode = event.desired_mode;
                    needs_sync = true;
                }
                ProviderAdminUpdate::WorkerError(error) => {
                    if provider_presence_online {
                        if let Err(report_error) = report_provider_presence_offline(
                            &presence_client,
                            &config,
                            &identity,
                            provider_presence_session_id.as_str(),
                        )
                        .await
                        {
                            eprintln!(
                                "warning: failed to report pylon provider offline to Nexus: {report_error}"
                            );
                        }
                    }
                    let snapshot = build_error_snapshot(
                        &config,
                        Some(&identity),
                        desired_mode,
                        previous_snapshot.as_ref(),
                        error.clone(),
                    );
                    let _ = runtime.sync_snapshot(snapshot);
                    return Err(anyhow!("provider admin runtime error: {error}"));
                }
            }
        }

        if desired_mode != ProviderDesiredMode::Online && provider_presence_online {
            if let Err(error) = report_provider_presence_offline(
                &presence_client,
                &config,
                &identity,
                provider_presence_session_id.as_str(),
            )
            .await
            {
                eprintln!("warning: failed to report pylon provider offline to Nexus: {error}");
            }
            provider_presence_online = false;
            next_provider_presence_heartbeat_at = Instant::now();
        }

        if needs_sync {
            let snapshot = match build_snapshot(
                &config,
                &identity,
                desired_mode,
                previous_snapshot.as_ref(),
            )
            .await
            {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    if provider_presence_online {
                        if let Err(report_error) = report_provider_presence_offline(
                            &presence_client,
                            &config,
                            &identity,
                            provider_presence_session_id.as_str(),
                        )
                        .await
                        {
                            eprintln!(
                                "warning: failed to report pylon provider offline to Nexus: {report_error}"
                            );
                        }
                    }
                    return Err(error);
                }
            };
            runtime
                .sync_snapshot(snapshot.clone())
                .map_err(anyhow::Error::msg)?;
            previous_snapshot = Some(snapshot);
            needs_sync = false;

            if desired_mode == ProviderDesiredMode::Online
                && Instant::now() >= next_provider_presence_heartbeat_at
            {
                if let Some(snapshot) = previous_snapshot.as_ref() {
                    if let Err(error) = report_provider_presence_heartbeat(
                        &presence_client,
                        config_path,
                        &config,
                        &identity,
                        provider_presence_session_id.as_str(),
                        snapshot,
                    )
                    .await
                    {
                        eprintln!(
                            "warning: failed to report pylon provider heartbeat to Nexus: {error}"
                        );
                    } else {
                        provider_presence_online = true;
                    }
                }
                next_provider_presence_heartbeat_at =
                    Instant::now() + provider_presence_heartbeat_interval;
            }
        }

        let sleep_duration = if needs_sync {
            Duration::from_millis(250)
        } else {
            Duration::from_secs(2)
        };
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result.context("failed waiting for ctrl-c")?;
                if provider_presence_online {
                    if let Err(error) = report_provider_presence_offline(
                        &presence_client,
                        &config,
                        &identity,
                        provider_presence_session_id.as_str(),
                    )
                    .await
                    {
                        eprintln!(
                            "warning: failed to report pylon provider offline to Nexus: {error}"
                        );
                    }
                }
                break;
            }
            () = tokio::time::sleep(sleep_duration) => {
                needs_sync = true;
            }
        }
    }

    Ok(())
}

async fn apply_control_command(
    config_path: &Path,
    action: ProviderControlAction,
) -> Result<ProviderStatusResponse> {
    let config = load_config_required(config_path)?;
    if try_live_control(&config, action).await? {
        let admin_config = provider_admin_config(&config)?;
        let store = ProviderPersistenceStore::open(&admin_config).map_err(anyhow::Error::msg)?;
        return load_status_with_store(&config, Some(&store), None).await;
    }
    apply_control_locally(&config, action).await
}

async fn build_snapshot(
    config: &PylonConfig,
    identity: &NostrIdentity,
    desired_mode: ProviderDesiredMode,
    previous_snapshot: Option<&ProviderPersistedSnapshot>,
) -> Result<ProviderPersistedSnapshot> {
    let availability = detect_availability(config).await?;
    Ok(build_snapshot_from_availability(
        config,
        Some(identity),
        desired_mode,
        previous_snapshot,
        availability,
        None,
    ))
}

fn inventory_rows(
    products: &[ProviderAdvertisedProduct],
    desired_mode: ProviderDesiredMode,
) -> Vec<ProviderInventoryRow> {
    products
        .iter()
        .map(|product| {
            let active = desired_mode == ProviderDesiredMode::Online && product.eligible;
            ProviderInventoryRow {
                target: product.product,
                enabled: product.enabled,
                backend_ready: product.backend_ready,
                eligible: product.eligible,
                capability_summary: product.capability_summary.clone(),
                market_receipt_class: product.market_receipt_class.clone(),
                earnings_summary: product.earnings_summary.clone(),
                source_badge: if active {
                    "pylon.serve".to_string()
                } else {
                    "pylon.local_preview".to_string()
                },
                capacity_lot_id: None,
                total_quantity: u64::from(active),
                reserved_quantity: 0,
                available_quantity: u64::from(active),
                delivery_state: if !product.enabled {
                    "disabled".to_string()
                } else if !product.backend_ready {
                    "backend_unavailable".to_string()
                } else if desired_mode == ProviderDesiredMode::Online {
                    "idle".to_string()
                } else {
                    "offline".to_string()
                },
                price_floor_sats: product.price_floor_sats,
                terms_label: product.terms_label.clone(),
                forward_capacity_lot_id: None,
                forward_delivery_window_label: None,
                forward_total_quantity: 0,
                forward_reserved_quantity: 0,
                forward_available_quantity: 0,
                forward_terms_label: Some(product.forward_terms_label.clone()),
            }
        })
        .collect()
}

async fn load_status_or_detect(config_path: &Path) -> Result<ProviderStatusResponse> {
    if !config_path.exists() {
        return Ok(build_unconfigured_status_for_path(config_path));
    }
    let config = load_config(config_path)?;
    if let Some(status) = try_live_status(&config).await? {
        return Ok(status);
    }
    let admin_config = provider_admin_config(&config)?;
    let store = if config.admin_db_path.exists() {
        Some(ProviderPersistenceStore::open(&admin_config).map_err(anyhow::Error::msg)?)
    } else {
        None
    };
    load_status_with_store(&config, store.as_ref(), None).await
}

async fn load_status_with_store(
    config: &PylonConfig,
    store: Option<&ProviderPersistenceStore>,
    desired_mode_override: Option<ProviderDesiredMode>,
) -> Result<ProviderStatusResponse> {
    let stored_status = store
        .map(ProviderPersistenceStore::load_status)
        .transpose()
        .map_err(anyhow::Error::msg)?;
    let desired_mode = desired_mode_override
        .or_else(|| stored_status.as_ref().map(|status| status.desired_mode))
        .unwrap_or(ProviderDesiredMode::Offline);
    let previous_snapshot = stored_status
        .as_ref()
        .and_then(|status| status.snapshot.as_ref());

    if !config.identity_path.exists() {
        return Ok(ProviderStatusResponse {
            listen_addr: Some(config.admin_listen_addr.clone()),
            desired_mode,
            snapshot: Some(build_unconfigured_snapshot(
                Some(config),
                desired_mode,
                previous_snapshot,
                "identity file missing",
            )),
        });
    }

    let identity = match load_identity_from_path(config.identity_path.as_path()) {
        Ok(identity) => identity,
        Err(error) => {
            return Ok(ProviderStatusResponse {
                listen_addr: Some(config.admin_listen_addr.clone()),
                desired_mode,
                snapshot: Some(build_error_snapshot(
                    config,
                    None,
                    desired_mode,
                    previous_snapshot,
                    error.to_string(),
                )),
            });
        }
    };

    let snapshot = match detect_availability(config).await {
        Ok(availability) => build_snapshot_from_availability(
            config,
            Some(&identity),
            desired_mode,
            previous_snapshot,
            availability,
            None,
        ),
        Err(error) => build_error_snapshot(
            config,
            Some(&identity),
            desired_mode,
            previous_snapshot,
            error.to_string(),
        ),
    };
    Ok(ProviderStatusResponse {
        listen_addr: Some(config.admin_listen_addr.clone()),
        desired_mode,
        snapshot: Some(snapshot),
    })
}

async fn apply_control_locally(
    config: &PylonConfig,
    action: ProviderControlAction,
) -> Result<ProviderStatusResponse> {
    let admin_config = provider_admin_config(config)?;
    let mut store = ProviderPersistenceStore::open(&admin_config).map_err(anyhow::Error::msg)?;
    store
        .set_listen_addr(config.admin_listen_addr.as_str())
        .map_err(anyhow::Error::msg)?;
    let current_status = load_status_with_store(config, Some(&store), None).await?;
    let desired_mode = validate_provider_control_action(&current_status, action)?;
    store
        .set_desired_mode(desired_mode)
        .map_err(anyhow::Error::msg)?;
    let updated_status = load_status_with_store(config, Some(&store), Some(desired_mode)).await?;
    if let Some(snapshot) = updated_status.snapshot.as_ref() {
        store
            .persist_snapshot(snapshot)
            .map_err(anyhow::Error::msg)?;
    }
    Ok(updated_status)
}

fn provider_admin_config(config: &PylonConfig) -> Result<ProviderAdminConfig> {
    let listen_addr = config
        .admin_listen_addr
        .parse()
        .with_context(|| format!("invalid admin listen addr {}", config.admin_listen_addr))?;
    Ok(ProviderAdminConfig::new(
        config.admin_db_path.clone(),
        listen_addr,
    ))
}

fn build_snapshot_from_availability(
    config: &PylonConfig,
    identity: Option<&NostrIdentity>,
    desired_mode: ProviderDesiredMode,
    previous_snapshot: Option<&ProviderPersistedSnapshot>,
    availability: ProviderAvailability,
    runtime_error: Option<String>,
) -> ProviderPersistedSnapshot {
    let captured_at_ms = now_epoch_ms();
    let products = products_from_availability(config, &availability);
    let runtime = derive_runtime_snapshot(
        desired_mode,
        previous_snapshot.map(|snapshot| &snapshot.runtime),
        &availability,
        products.as_slice(),
        runtime_error.clone(),
    );
    let mut earnings = previous_snapshot
        .and_then(|snapshot| snapshot.earnings.clone())
        .unwrap_or_else(default_earnings_summary);
    earnings.online_uptime_seconds = runtime.online_uptime_seconds;

    assemble_provider_persisted_snapshot(ProviderSnapshotParts {
        captured_at_ms,
        config_metadata: vec![
            ProviderJsonEntry {
                key: "node_label".to_string(),
                value: Value::String(config.node_label.clone()),
            },
            ProviderJsonEntry {
                key: "payout_destination".to_string(),
                value: json!(config.payout_destination),
            },
            ProviderJsonEntry {
                key: "local_gemma_base_url".to_string(),
                value: Value::String(config.local_gemma_base_url.clone()),
            },
        ],
        identity: identity.map(|identity| identity_metadata(identity, config.node_label.as_str())),
        runtime,
        availability,
        inventory_rows: inventory_rows(products.as_slice(), desired_mode),
        recent_jobs: previous_snapshot
            .map(|snapshot| snapshot.recent_jobs.clone())
            .unwrap_or_default(),
        receipts: previous_snapshot
            .map(|snapshot| snapshot.receipts.clone())
            .unwrap_or_default(),
        payouts: previous_snapshot
            .map(|snapshot| snapshot.payouts.clone())
            .unwrap_or_default(),
        health_events: build_health_events(products.as_slice(), runtime_error.as_deref()),
        earnings: Some(earnings),
    })
}

fn derive_runtime_snapshot(
    desired_mode: ProviderDesiredMode,
    previous_runtime: Option<&ProviderRuntimeStatusSnapshot>,
    availability: &ProviderAvailability,
    products: &[ProviderAdvertisedProduct],
    runtime_error: Option<String>,
) -> ProviderRuntimeStatusSnapshot {
    let eligible_products = products.iter().filter(|product| product.eligible).count();
    let enabled_products = products.iter().filter(|product| product.enabled).count();
    let queue_depth = previous_runtime.map_or(0, |runtime| runtime.queue_depth);
    let state = if runtime_error.is_some() {
        "error".to_string()
    } else {
        match desired_mode {
            ProviderDesiredMode::Paused => "paused".to_string(),
            ProviderDesiredMode::Offline if queue_depth > 0 => "draining".to_string(),
            ProviderDesiredMode::Offline if eligible_products > 0 => "ready".to_string(),
            ProviderDesiredMode::Offline => "offline".to_string(),
            ProviderDesiredMode::Online if eligible_products > 0 => "online".to_string(),
            ProviderDesiredMode::Online => "degraded".to_string(),
        }
    };
    let mode = match state.as_str() {
        "online" => ProviderMode::Online,
        "degraded" | "draining" | "error" => ProviderMode::Degraded,
        _ => ProviderMode::Offline,
    };
    let degraded_reason_code = match state.as_str() {
        "degraded" => Some("NO_ELIGIBLE_SUPPLY".to_string()),
        "error" => Some("STATUS_BUILD_ERROR".to_string()),
        "draining" => Some("DRAINING_PENDING_WORK".to_string()),
        _ => None,
    };
    let last_error = runtime_error.or_else(|| match state.as_str() {
        "degraded" | "offline" => first_backend_error(availability),
        _ => None,
    });
    let last_action = match state.as_str() {
        "online" => format!("pylon is online with {eligible_products} sellable launch products"),
        "ready" => format!("pylon is ready with {eligible_products} sellable launch products"),
        "paused" => "pylon is paused".to_string(),
        "draining" => "pylon is draining in-flight work".to_string(),
        "degraded" => format!(
            "pylon cannot go online because {enabled_products} enabled products are not sellable"
        ),
        "error" => "pylon hit a local control or status error".to_string(),
        _ => "pylon is initialized but offline".to_string(),
    };
    ProviderRuntimeStatusSnapshot {
        mode,
        last_action: Some(last_action),
        last_error,
        degraded_reason_code,
        authoritative_status: Some(state.clone()),
        authoritative_error_class: if state == "error" {
            Some(ProviderFailureClass::Reconciliation)
        } else if state == "degraded" {
            Some(ProviderFailureClass::Execution)
        } else {
            None
        },
        queue_depth,
        online_uptime_seconds: previous_runtime
            .map(|runtime| runtime.online_uptime_seconds)
            .unwrap_or(0),
        inventory_session_started_at_ms: if state == "online" {
            previous_runtime
                .and_then(|runtime| runtime.inventory_session_started_at_ms)
                .or(Some(now_epoch_ms()))
        } else {
            None
        },
        last_completed_job_at_epoch_ms: previous_runtime
            .and_then(|runtime| runtime.last_completed_job_at_epoch_ms),
        last_authoritative_event_id: previous_runtime
            .and_then(|runtime| runtime.last_authoritative_event_id.clone()),
        execution_backend_label: execution_backend_label(availability, products),
        provider_blocker_codes: provider_blocker_codes(availability, products, state.as_str()),
    }
}

fn build_unconfigured_status_for_path(config_path: &Path) -> ProviderStatusResponse {
    let base_dir = config_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(default_home_dir);
    let config = default_config(base_dir.as_path());
    ProviderStatusResponse {
        listen_addr: Some(config.admin_listen_addr.clone()),
        desired_mode: ProviderDesiredMode::Offline,
        snapshot: Some(build_unconfigured_snapshot(
            None,
            ProviderDesiredMode::Offline,
            None,
            "config missing",
        )),
    }
}

fn build_unconfigured_snapshot(
    config: Option<&PylonConfig>,
    _desired_mode: ProviderDesiredMode,
    previous_snapshot: Option<&ProviderPersistedSnapshot>,
    detail: &str,
) -> ProviderPersistedSnapshot {
    let availability = ProviderAvailability::default();
    let runtime = ProviderRuntimeStatusSnapshot {
        mode: ProviderMode::Offline,
        last_action: Some("pylon is not initialized".to_string()),
        last_error: Some(detail.to_string()),
        degraded_reason_code: Some("UNCONFIGURED".to_string()),
        authoritative_status: Some("unconfigured".to_string()),
        authoritative_error_class: Some(ProviderFailureClass::Reconciliation),
        queue_depth: previous_snapshot.map_or(0, |snapshot| snapshot.runtime.queue_depth),
        online_uptime_seconds: 0,
        inventory_session_started_at_ms: None,
        last_completed_job_at_epoch_ms: previous_snapshot
            .and_then(|snapshot| snapshot.runtime.last_completed_job_at_epoch_ms),
        last_authoritative_event_id: previous_snapshot
            .and_then(|snapshot| snapshot.runtime.last_authoritative_event_id.clone()),
        execution_backend_label: "not configured".to_string(),
        provider_blocker_codes: vec!["CONFIG_MISSING".to_string(), "IDENTITY_MISSING".to_string()],
    };
    assemble_provider_persisted_snapshot(ProviderSnapshotParts {
        captured_at_ms: now_epoch_ms(),
        config_metadata: config
            .map(|config| {
                vec![ProviderJsonEntry {
                    key: "node_label".to_string(),
                    value: Value::String(config.node_label.clone()),
                }]
            })
            .unwrap_or_default(),
        identity: None,
        runtime,
        availability,
        inventory_rows: Vec::new(),
        recent_jobs: previous_snapshot
            .map(|snapshot| snapshot.recent_jobs.clone())
            .unwrap_or_default(),
        receipts: previous_snapshot
            .map(|snapshot| snapshot.receipts.clone())
            .unwrap_or_default(),
        payouts: previous_snapshot
            .map(|snapshot| snapshot.payouts.clone())
            .unwrap_or_default(),
        health_events: build_health_events(&[], Some(detail)),
        earnings: Some(default_earnings_summary()),
    })
}

fn build_error_snapshot(
    config: &PylonConfig,
    identity: Option<&NostrIdentity>,
    desired_mode: ProviderDesiredMode,
    previous_snapshot: Option<&ProviderPersistedSnapshot>,
    error_detail: String,
) -> ProviderPersistedSnapshot {
    build_snapshot_from_availability(
        config,
        identity,
        desired_mode,
        previous_snapshot,
        ProviderAvailability::default(),
        Some(error_detail),
    )
}

fn default_earnings_summary() -> ProviderEarningsSummary {
    ProviderEarningsSummary {
        sats_today: 0,
        lifetime_sats: 0,
        jobs_today: 0,
        online_uptime_seconds: 0,
        last_job_result: "none".to_string(),
        first_job_latency_seconds: None,
        completion_ratio_bps: None,
        payout_success_ratio_bps: None,
        avg_wallet_confirmation_latency_seconds: None,
    }
}

fn build_health_events(
    products: &[ProviderAdvertisedProduct],
    runtime_error: Option<&str>,
) -> Vec<ProviderHealthEvent> {
    let mut events = Vec::new();
    if let Some(runtime_error) = runtime_error {
        events.push(ProviderHealthEvent {
            event_id: "runtime_error".to_string(),
            occurred_at_ms: now_epoch_ms(),
            severity: "error".to_string(),
            code: "STATUS_BUILD_ERROR".to_string(),
            detail: runtime_error.to_string(),
            source: "pylon".to_string(),
        });
    }
    events.extend(
        products
            .iter()
            .filter(|product| product.enabled && !product.backend_ready)
            .map(|product| ProviderHealthEvent {
                event_id: format!("backend_unavailable:{}", product.product.product_id()),
                occurred_at_ms: now_epoch_ms(),
                severity: "warn".to_string(),
                code: "BACKEND_UNAVAILABLE".to_string(),
                detail: format!(
                    "{} is enabled but not ready",
                    product.product.display_label()
                ),
                source: "pylon".to_string(),
            }),
    );
    events
}

fn provider_blocker_codes(
    availability: &ProviderAvailability,
    products: &[ProviderAdvertisedProduct],
    state: &str,
) -> Vec<String> {
    let mut codes = Vec::new();
    if !availability.local_gemma.ready {
        codes.push("LOCAL_GEMMA_UNAVAILABLE".to_string());
    }
    if !products.iter().any(|product| product.eligible)
        && matches!(state, "degraded" | "draining" | "offline")
    {
        codes.push("NO_ELIGIBLE_SUPPLY".to_string());
    }
    codes
}

fn execution_backend_label(
    availability: &ProviderAvailability,
    products: &[ProviderAdvertisedProduct],
) -> String {
    if products.iter().any(|product| {
        product.eligible
            && product.product.compute_family_label() == "inference"
            && product.product.backend_label() == "local_gemma"
    }) {
        return availability.execution_backend_label().to_string();
    }
    if products
        .iter()
        .any(|product| product.product.compute_family_label() == "sandbox_execution")
    {
        return "sandbox runtime".to_string();
    }
    "no active runtime".to_string()
}

fn first_backend_error(availability: &ProviderAvailability) -> Option<String> {
    availability
        .local_gemma
        .last_error
        .clone()
        .or_else(|| availability.sandbox.last_scan_error.clone())
}

fn render_sandbox_status_lines(availability: &ProviderAvailability) -> Vec<String> {
    let mut lines = Vec::new();
    let sandbox = &availability.sandbox;
    let supported = sandbox_supported_execution_classes(availability);
    let ready = sandbox_ready_execution_classes(availability);
    let runtimes = sandbox_runtime_kinds(availability, false);
    let profiles = sandbox_profile_ids(availability);
    if !supported.is_empty()
        || !runtimes.is_empty()
        || !profiles.is_empty()
        || sandbox.last_scan_error.is_some()
    {
        lines.push(format!(
            "sandbox_execution_classes: {}",
            comma_or_none(supported.as_slice())
        ));
        lines.push(format!(
            "sandbox_ready_classes: {}",
            comma_or_none(ready.as_slice())
        ));
        lines.push(format!(
            "sandbox_runtimes: {}",
            comma_or_none(runtimes.as_slice())
        ));
        lines.push(format!(
            "sandbox_profiles: {}",
            comma_or_none(profiles.as_slice())
        ));
        if let Some(last_scan_error) = sandbox.last_scan_error.as_deref() {
            lines.push(format!("sandbox_last_error: {last_scan_error}"));
        }
    }
    lines
}

fn identity_metadata(identity: &NostrIdentity, node_label: &str) -> ProviderIdentityMetadata {
    ProviderIdentityMetadata {
        npub: Some(identity.npub.clone()),
        public_key_hex: Some(identity.public_key_hex.clone()),
        display_name: Some("Pylon".to_string()),
        node_label: Some(node_label.to_string()),
    }
}

fn render_human_status(status: &ProviderStatusResponse) -> String {
    let mut lines = vec![
        format!("state: {}", provider_runtime_state_label(status)),
        format!("desired_mode: {}", status.desired_mode.label()),
    ];
    if let Some(listen_addr) = status.listen_addr.as_deref() {
        lines.push(format!("listen_addr: {listen_addr}"));
    }
    if let Some(snapshot) = status.snapshot.as_ref() {
        let eligible_products = snapshot
            .inventory_rows
            .iter()
            .filter(|row| row.eligible)
            .count();
        lines.push(format!(
            "products: {} visible / {} eligible",
            snapshot.inventory_rows.len(),
            eligible_products
        ));
        lines.push(format!(
            "execution_backend: {}",
            snapshot.runtime.execution_backend_label
        ));
        if let Some(reason_code) = snapshot.runtime.degraded_reason_code.as_deref() {
            lines.push(format!("reason_code: {reason_code}"));
        }
        if let Some(last_error) = snapshot.runtime.last_error.as_deref() {
            lines.push(format!("last_error: {last_error}"));
        }
        if !snapshot.runtime.provider_blocker_codes.is_empty() {
            lines.push(format!(
                "blockers: {}",
                snapshot.runtime.provider_blocker_codes.join(", ")
            ));
        }
        lines.extend(render_sandbox_status_lines(&snapshot.availability));
    }
    lines.join("\n")
}

pub fn load_config_or_default(path: &Path) -> Result<PylonConfig> {
    if path.exists() {
        return load_config(path);
    }
    let base_dir = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(default_home_dir);
    Ok(default_config(base_dir.as_path()))
}

pub async fn load_config_and_status(
    config_path: &Path,
) -> Result<(PylonConfig, ProviderStatusResponse)> {
    let config = load_config_or_default(config_path)?;
    let status = load_status_or_detect(config_path).await?;
    Ok((config, status))
}

pub async fn report_provider_presence_heartbeat_for_snapshot(
    config_path: &Path,
    session_id: &str,
    snapshot: &ProviderPersistedSnapshot,
) -> Result<()> {
    let config = load_config_required(config_path)?;
    let identity = ensure_identity(config.identity_path.as_path())?;
    let client = provider_presence_client()?;
    report_provider_presence_heartbeat(
        &client,
        config_path,
        &config,
        &identity,
        session_id,
        snapshot,
    )
    .await
}

pub async fn report_provider_presence_offline_for_config(
    config_path: &Path,
    session_id: &str,
) -> Result<()> {
    let config = load_config_required(config_path)?;
    let identity = ensure_identity(config.identity_path.as_path())?;
    let client = provider_presence_client()?;
    report_provider_presence_offline(&client, &config, &identity, session_id).await
}

pub async fn resolve_local_gemma_chat_target(config_path: &Path) -> Result<LocalGemmaChatTarget> {
    ensure_local_setup(config_path)?;
    let (config, status) = load_config_and_status(config_path).await?;
    resolve_local_gemma_chat_target_from_status(&config, &status)
}

pub fn resolve_local_gemma_chat_target_from_snapshot(
    config: &PylonConfig,
    snapshot: &ProviderPersistedSnapshot,
) -> Result<LocalGemmaChatTarget> {
    let _ = config;
    if let Some(model) = gemma_ready_model(&snapshot.availability.local_gemma) {
        return Ok(LocalGemmaChatTarget {
            backend: LocalGemmaChatBackend::LocalRuntime,
            model,
        });
    }

    bail!("local Gemma weights are not loaded");
}

pub async fn run_local_gemma_chat_stream<F>(
    config_path: &Path,
    prompt: &str,
    emit: F,
) -> Result<LocalGemmaChatTarget>
where
    F: FnMut(LocalGemmaChatEvent),
{
    run_local_gemma_chat_messages_stream(
        config_path,
        &[LocalGemmaChatMessage::user(prompt.trim())],
        emit,
    )
    .await
}

pub async fn run_local_gemma_chat_messages_stream<F>(
    config_path: &Path,
    messages: &[LocalGemmaChatMessage],
    emit: F,
) -> Result<LocalGemmaChatTarget>
where
    F: FnMut(LocalGemmaChatEvent),
{
    if messages
        .iter()
        .all(|message| message.content.trim().is_empty())
    {
        bail!("chat prompt is empty");
    }

    ensure_local_setup(config_path)?;
    let (config, status) = load_config_and_status(config_path).await?;
    let target = resolve_local_gemma_chat_target_from_status(&config, &status)?;
    stream_local_gemma_chat_messages_target(config_path, &target, messages, emit).await
}

pub async fn stream_local_gemma_chat_target<F>(
    config_path: &Path,
    target: &LocalGemmaChatTarget,
    prompt: &str,
    emit: F,
) -> Result<LocalGemmaChatTarget>
where
    F: FnMut(LocalGemmaChatEvent),
{
    stream_local_gemma_chat_messages_target(
        config_path,
        target,
        &[LocalGemmaChatMessage::user(prompt.trim())],
        emit,
    )
    .await
}

pub async fn stream_local_gemma_chat_messages_target<F>(
    config_path: &Path,
    target: &LocalGemmaChatTarget,
    messages: &[LocalGemmaChatMessage],
    mut emit: F,
) -> Result<LocalGemmaChatTarget>
where
    F: FnMut(LocalGemmaChatEvent),
{
    if messages
        .iter()
        .all(|message| message.content.trim().is_empty())
    {
        bail!("chat prompt is empty");
    }

    ensure_local_setup(config_path)?;
    let config = load_config_or_default(config_path)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build pylon chat client")?;

    emit(LocalGemmaChatEvent::Started {
        target: target.clone(),
    });
    match target.backend {
        LocalGemmaChatBackend::LocalRuntime => {
            stream_local_gemma_chat(&client, &config, target, messages, &mut emit).await?;
        }
    }
    emit(LocalGemmaChatEvent::Finished {
        target: target.clone(),
    });
    Ok(target.clone())
}

fn resolve_local_gemma_chat_target_from_status(
    config: &PylonConfig,
    status: &ProviderStatusResponse,
) -> Result<LocalGemmaChatTarget> {
    let Some(snapshot) = status.snapshot.as_ref() else {
        bail!("local Gemma weights are not loaded");
    };
    resolve_local_gemma_chat_target_from_snapshot(config, snapshot)
}

fn gemma_ready_model(health: &ProviderBackendHealth) -> Option<String> {
    if let Some(model) = health.ready_model.as_deref() {
        if is_gemma_model(model) {
            return Some(model.to_string());
        }
    }
    health
        .available_models
        .iter()
        .find(|model| is_gemma_model(model.as_str()))
        .cloned()
}

fn is_gemma_model(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.contains("gemma")
}

async fn stream_local_gemma_chat<F>(
    client: &reqwest::Client,
    config: &PylonConfig,
    target: &LocalGemmaChatTarget,
    messages: &[LocalGemmaChatMessage],
    emit: &mut F,
) -> Result<()>
where
    F: FnMut(LocalGemmaChatEvent),
{
    let url = format!(
        "{}/api/chat",
        config.local_gemma_base_url.trim_end_matches('/')
    );
    let payload = json!({
        "model": target.model,
        "messages": messages
            .iter()
            .map(|message| {
                json!({
                    "role": message.role.api_label(),
                    "content": message.content,
                })
            })
            .collect::<Vec<_>>(),
        "stream": true,
    });
    let mut response = client
        .post(url)
        .json(&payload)
        .send()
        .await
        .context("failed to send local Gemma chat request")?;
    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!(
            "local Gemma chat failed: {}",
            http_error_message(body.as_str())
        );
    }

    let mut pending = String::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .context("failed reading local Gemma chat stream")?
    {
        pending.push_str(String::from_utf8_lossy(&chunk).as_ref());
        while let Some(line_end) = pending.find('\n') {
            let line = pending[..line_end].trim().to_string();
            pending.drain(..=line_end);
            if line.is_empty() {
                continue;
            }
            if let Some(delta) = decode_ollama_chat_delta(line.as_str())? {
                emit(LocalGemmaChatEvent::Delta(delta));
            }
        }
    }

    let trailing = pending.trim();
    if !trailing.is_empty() {
        if let Some(delta) = decode_ollama_chat_delta(trailing)? {
            emit(LocalGemmaChatEvent::Delta(delta));
        }
    }

    Ok(())
}

fn decode_ollama_chat_delta(line: &str) -> Result<Option<String>> {
    let payload = serde_json::from_str::<Value>(line)
        .with_context(|| format!("invalid local Gemma stream chunk: {line}"))?;
    Ok(decode_ollama_chat_chunk_from_payload(&payload).delta)
}

fn decode_ollama_chat_chunk(line: &str) -> Result<LocalGemmaChatChunk> {
    let payload = serde_json::from_str::<Value>(line)
        .with_context(|| format!("invalid local Gemma stream chunk: {line}"))?;
    Ok(decode_ollama_chat_chunk_from_payload(&payload))
}

fn decode_ollama_chat_chunk_from_payload(payload: &Value) -> LocalGemmaChatChunk {
    let delta = payload
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let completion = (payload.get("done").and_then(Value::as_bool) == Some(true)).then(|| {
        let decode_s = parse_ollama_duration_seconds(payload, "eval_duration");
        let output_tokens = payload
            .get("eval_count")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        LocalGemmaChatCompletionMetrics {
            total_s: parse_ollama_duration_seconds(payload, "total_duration"),
            load_s: parse_ollama_duration_seconds(payload, "load_duration"),
            prompt_s: parse_ollama_duration_seconds(payload, "prompt_eval_duration"),
            decode_s,
            output_tokens,
            decode_tok_s: match (output_tokens, decode_s) {
                (Some(output_tokens), Some(decode_s))
                    if output_tokens > 0 && decode_s.is_finite() && decode_s > 0.0 =>
                {
                    Some(output_tokens as f64 / decode_s)
                }
                _ => None,
            },
        }
    });
    LocalGemmaChatChunk { delta, completion }
}

fn http_error_message(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "request failed".to_string();
    }
    serde_json::from_str::<Value>(trimmed)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .or_else(|| {
                    value
                        .get("message")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
        })
        .unwrap_or_else(|| trimmed.to_string())
}

fn report_context(status: &ProviderStatusResponse) -> ReportContext {
    ReportContext {
        state: provider_runtime_state_label(status),
        desired_mode: status.desired_mode.label().to_string(),
        listen_addr: status.listen_addr.clone(),
    }
}

fn products_from_status(
    config: &PylonConfig,
    status: &ProviderStatusResponse,
) -> Vec<ProviderAdvertisedProduct> {
    status
        .snapshot
        .as_ref()
        .map(|snapshot| products_from_availability(config, &snapshot.availability))
        .unwrap_or_default()
}

fn products_from_availability(
    config: &PylonConfig,
    availability: &ProviderAvailability,
) -> Vec<ProviderAdvertisedProduct> {
    derive_provider_products(availability, &config.inventory_controls)
        .into_iter()
        .filter(|product| product.product.backend_label() != "apple_foundation_models")
        .collect()
}

fn public_product_entries(products: Vec<ProviderAdvertisedProduct>) -> Vec<ProductEntry> {
    products
        .into_iter()
        .map(|product| ProductEntry {
            product_id: product.product.product_id().to_string(),
            display_label: product.product.display_label().to_string(),
            compute_family: product.product.compute_family_label().to_string(),
            backend: product.product.backend_label().to_string(),
            enabled: product.enabled,
            backend_ready: product.backend_ready,
            eligible: product.eligible,
            capability_summary: product.capability_summary,
            price_floor_sats: product.price_floor_sats,
            terms_label: product.terms_label,
            forward_terms_label: product.forward_terms_label,
        })
        .collect()
}

fn backend_entry(
    backend_id: &str,
    display_label: &str,
    health_state: String,
    health: &ProviderBackendHealth,
    products: &[ProviderAdvertisedProduct],
) -> BackendEntry {
    let launch_product_ids = products
        .iter()
        .map(|product| product.product.product_id().to_string())
        .collect::<Vec<_>>();
    let eligible_product_ids = products
        .iter()
        .filter(|product| product.eligible)
        .map(|product| product.product.product_id().to_string())
        .collect::<Vec<_>>();
    BackendEntry {
        backend_id: backend_id.to_string(),
        display_label: display_label.to_string(),
        health_state,
        reachable: health.reachable,
        ready: health.ready,
        ready_model: health.ready_model.clone(),
        available_models: health.available_models.clone(),
        availability_message: health.availability_message.clone(),
        launch_product_ids,
        eligible_product_ids,
        supported_execution_classes: Vec::new(),
        ready_execution_classes: Vec::new(),
        runtime_kinds: Vec::new(),
        ready_runtime_kinds: Vec::new(),
        profile_ids: Vec::new(),
        last_error: health.last_error.clone(),
    }
}

fn local_gemma_health_state(config: &PylonConfig, health: &ProviderBackendHealth) -> String {
    if !config.inventory_controls.local_gemma_inference_enabled
        && !config.inventory_controls.local_gemma_embeddings_enabled
    {
        return "disabled".to_string();
    }
    if health.ready {
        return "healthy".to_string();
    }
    if health.reachable && health.available_models.is_empty() {
        return "misconfigured".to_string();
    }
    if !health.reachable || health.last_error.is_some() {
        return "unavailable".to_string();
    }
    "misconfigured".to_string()
}

fn sandbox_controls_enabled(config: &PylonConfig) -> bool {
    let controls = &config.inventory_controls;
    controls.sandbox_container_exec_enabled
        || controls.sandbox_python_exec_enabled
        || controls.sandbox_node_exec_enabled
        || controls.sandbox_posix_exec_enabled
}

fn sandbox_supported_execution_classes(availability: &ProviderAvailability) -> Vec<String> {
    availability
        .sandbox
        .declared_execution_classes()
        .into_iter()
        .map(|execution_class| execution_class.product_id().to_string())
        .collect()
}

fn sandbox_ready_execution_classes(availability: &ProviderAvailability) -> Vec<String> {
    availability
        .sandbox
        .ready_execution_classes()
        .into_iter()
        .map(|execution_class| execution_class.product_id().to_string())
        .collect()
}

fn sandbox_runtime_kinds(availability: &ProviderAvailability, ready_only: bool) -> Vec<String> {
    let kinds = if ready_only {
        availability.sandbox.ready_runtime_kinds()
    } else {
        availability.sandbox.detected_runtime_kinds()
    };
    kinds
        .into_iter()
        .map(|runtime_kind| runtime_kind.id().to_string())
        .collect()
}

fn sandbox_profile_ids(availability: &ProviderAvailability) -> Vec<String> {
    availability
        .sandbox
        .profiles
        .iter()
        .map(|profile| profile.profile_id.clone())
        .collect()
}

fn sandbox_health_state(
    config: &PylonConfig,
    availability: &ProviderAvailability,
    products: &[ProviderAdvertisedProduct],
) -> String {
    if !sandbox_controls_enabled(config) {
        return "disabled".to_string();
    }
    if availability.sandbox.last_scan_error.is_some() {
        return "error".to_string();
    }
    if products.iter().any(|product| product.eligible) {
        return "healthy".to_string();
    }
    if availability.sandbox.profiles.is_empty() {
        return if availability.sandbox.detected_runtime_kinds().is_empty() {
            "unsupported".to_string()
        } else {
            "misconfigured".to_string()
        };
    }
    if availability.sandbox.ready_runtime_kinds().is_empty() {
        return "unavailable".to_string();
    }
    "misconfigured".to_string()
}

fn sandbox_backend_entry(
    config: &PylonConfig,
    availability: &ProviderAvailability,
    products: &[ProviderAdvertisedProduct],
) -> BackendEntry {
    let visible_product_ids = products
        .iter()
        .map(|product| product.product.product_id().to_string())
        .collect::<Vec<_>>();
    let eligible_product_ids = products
        .iter()
        .filter(|product| product.eligible)
        .map(|product| product.product.product_id().to_string())
        .collect::<Vec<_>>();
    BackendEntry {
        backend_id: "sandbox".to_string(),
        display_label: "Declared sandbox runtime".to_string(),
        health_state: sandbox_health_state(config, availability, products),
        reachable: !availability.sandbox.detected_runtime_kinds().is_empty(),
        ready: !availability.sandbox.ready_runtime_kinds().is_empty(),
        ready_model: None,
        available_models: Vec::new(),
        availability_message: availability.sandbox.last_scan_error.clone().or_else(|| {
            if availability.sandbox.profiles.is_empty() {
                Some("no declared sandbox profiles".to_string())
            } else {
                None
            }
        }),
        launch_product_ids: visible_product_ids,
        eligible_product_ids,
        supported_execution_classes: sandbox_supported_execution_classes(availability),
        ready_execution_classes: sandbox_ready_execution_classes(availability),
        runtime_kinds: sandbox_runtime_kinds(availability, false),
        ready_runtime_kinds: sandbox_runtime_kinds(availability, true),
        profile_ids: sandbox_profile_ids(availability),
        last_error: availability.sandbox.last_scan_error.clone(),
    }
}

async fn load_backend_report(config_path: &Path) -> Result<BackendReport> {
    let (config, status) = load_config_and_status(config_path).await?;
    let availability = if config_path.exists() {
        try_live_json::<ProviderAvailability>(&config, "/v1/backend-health")
            .await?
            .or_else(|| {
                status
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.availability.clone())
            })
            .unwrap_or_default()
    } else {
        ProviderAvailability::default()
    };
    let products = products_from_availability(&config, &availability);
    let local_gemma_products = products
        .iter()
        .filter(|product| product.product.backend_label() == "local_gemma")
        .cloned()
        .collect::<Vec<_>>();
    let sandbox_products = products
        .iter()
        .filter(|product| product.product.backend_label() == "sandbox")
        .cloned()
        .collect::<Vec<_>>();
    Ok(BackendReport {
        context: report_context(&status),
        backends: vec![
            backend_entry(
                "local_gemma",
                "Local Gemma",
                local_gemma_health_state(&config, &availability.local_gemma),
                &availability.local_gemma,
                local_gemma_products.as_slice(),
            ),
            sandbox_backend_entry(&config, &availability, sandbox_products.as_slice()),
        ],
    })
}

async fn load_inventory_report(
    config_path: &Path,
    limit: Option<usize>,
) -> Result<InventoryReport> {
    let (config, status) = load_config_and_status(config_path).await?;
    let snapshot_rows = take_limited_rows(
        status
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.inventory_rows.clone())
            .unwrap_or_default(),
        limit,
    );
    let rows = if config_path.exists() {
        if let Some(rows) =
            try_live_json::<Vec<ProviderInventoryRow>>(&config, inventory_endpoint(limit).as_str())
                .await?
        {
            rows
        } else if let Some(store) = open_existing_store(&config)? {
            let stored_rows = store
                .load_inventory_rows(limit)
                .map_err(anyhow::Error::msg)?;
            merge_inventory_rows(snapshot_rows, stored_rows, limit)
        } else if !snapshot_rows.is_empty() {
            snapshot_rows
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    Ok(InventoryReport {
        context: report_context(&status),
        rows,
    })
}

fn merge_inventory_rows(
    snapshot_rows: Vec<ProviderInventoryRow>,
    stored_rows: Vec<ProviderInventoryRow>,
    limit: Option<usize>,
) -> Vec<ProviderInventoryRow> {
    if stored_rows.is_empty() {
        return take_limited_rows(snapshot_rows, limit);
    }

    let mut merged = BTreeMap::new();
    for row in stored_rows {
        merged.insert(row.target.product_id().to_string(), row);
    }
    let page_capacity = limit.unwrap_or(usize::MAX);
    for row in snapshot_rows {
        let product_id = row.target.product_id().to_string();
        if let Some(existing) = merged.get(&product_id) {
            if inventory_row_truth_rank(existing) <= inventory_row_truth_rank(&row) {
                merged.insert(product_id, row);
            }
        } else if merged.len() < page_capacity {
            merged.insert(product_id, row);
        }
    }
    take_limited_rows(merged.into_values().collect(), limit)
}

fn inventory_row_truth_rank(row: &ProviderInventoryRow) -> (bool, bool, bool) {
    (row.eligible, row.backend_ready, row.enabled)
}

async fn load_product_report(config_path: &Path) -> Result<ProductReport> {
    let (config, status) = load_config_and_status(config_path).await?;
    let products = public_product_entries(products_from_status(&config, &status));
    Ok(ProductReport {
        context: report_context(&status),
        products,
    })
}

pub fn load_relay_report(config_path: &Path) -> Result<RelayReport> {
    let config = load_or_create_config(config_path)?;
    let ledger = load_ledger(config_path)?;
    let mut relays = config
        .relay_urls
        .iter()
        .map(|url| {
            let state = ledger
                .relay_state
                .iter()
                .find(|entry| entry.url == *url)
                .cloned();
            RelayEntry {
                url: url.clone(),
                state: state
                    .as_ref()
                    .map(|entry| entry.connection_state.clone())
                    .unwrap_or_else(|| "unknown".to_string()),
                auth_state: state
                    .as_ref()
                    .map(|entry| entry.auth_state.clone())
                    .unwrap_or_else(|| "disabled".to_string()),
                detail: state.as_ref().and_then(|entry| entry.last_detail.clone()),
                last_error: state.as_ref().and_then(|entry| entry.last_error.clone()),
                last_connected_at_ms: state.as_ref().and_then(|entry| entry.last_connected_at_ms),
                updated_at_ms: state.as_ref().map(|entry| entry.updated_at_ms),
            }
        })
        .collect::<Vec<_>>();
    relays.sort_by(|left, right| left.url.cmp(&right.url));
    Ok(RelayReport {
        relay_config: RelayConfigReport {
            connect_timeout_seconds: config.relay_connect_timeout_seconds,
            auth_enabled: config.relay_auth_enabled,
            ledger_path: default_ledger_path(config_path).display().to_string(),
        },
        relays,
    })
}

pub fn add_configured_relay(config_path: &Path, url: &str) -> Result<RelayReport> {
    let mut config = load_or_create_config(config_path)?;
    let normalized = validate_and_normalize_relay_url(url)?;
    if config
        .relay_urls
        .iter()
        .any(|existing| existing == &normalized)
    {
        bail!("relay already configured: {normalized}");
    }
    config.relay_urls.push(normalized.clone());
    config.relay_urls.sort();
    save_config(config_path, &config)?;
    mutate_ledger(config_path, |ledger| {
        ledger.set_relay_config(
            config.relay_urls.clone(),
            config.relay_connect_timeout_seconds,
        );
        ledger.push_relay_activity(PylonRelayActivity {
            at_ms: now_epoch_ms() as u64,
            url: Some(normalized.clone()),
            kind: "relay.added".to_string(),
            detail: "configured relay added".to_string(),
        });
        Ok(())
    })?;
    load_relay_report(config_path)
}

pub fn remove_configured_relay(config_path: &Path, url: &str) -> Result<RelayReport> {
    let mut config = load_or_create_config(config_path)?;
    let normalized = normalize_relay_url(url);
    let original_len = config.relay_urls.len();
    config.relay_urls.retain(|existing| existing != &normalized);
    if config.relay_urls.len() == original_len {
        bail!("relay is not configured: {normalized}");
    }
    save_config(config_path, &config)?;
    mutate_ledger(config_path, |ledger| {
        ledger.set_relay_config(
            config.relay_urls.clone(),
            config.relay_connect_timeout_seconds,
        );
        ledger.relay_state.retain(|entry| entry.url != normalized);
        ledger.push_relay_activity(PylonRelayActivity {
            at_ms: now_epoch_ms() as u64,
            url: Some(normalized.clone()),
            kind: "relay.removed".to_string(),
            detail: "configured relay removed".to_string(),
        });
        Ok(())
    })?;
    load_relay_report(config_path)
}

pub async fn refresh_relay_report(config_path: &Path) -> Result<RelayReport> {
    let config = ensure_local_setup(config_path)?;
    let identity = if config.relay_auth_enabled {
        Some(load_identity_from_path(config.identity_path.as_path())?)
    } else {
        None
    };
    mutate_ledger(config_path, |ledger| {
        ledger.set_relay_config(
            config.relay_urls.clone(),
            config.relay_connect_timeout_seconds,
        );
        Ok(())
    })?;

    for relay_url in &config.relay_urls {
        refresh_single_relay(config_path, &config, identity.as_ref(), relay_url.as_str()).await?;
    }

    load_relay_report(config_path)
}

async fn refresh_single_relay(
    config_path: &Path,
    config: &PylonConfig,
    identity: Option<&NostrIdentity>,
    relay_url: &str,
) -> Result<()> {
    let normalized = normalize_relay_url(relay_url);
    mutate_ledger(config_path, |ledger| {
        ledger.upsert_relay_state(PylonRelayState {
            url: normalized.clone(),
            connection_state: "connecting".to_string(),
            auth_state: if config.relay_auth_enabled {
                "enabled".to_string()
            } else {
                "disabled".to_string()
            },
            last_detail: Some("attempting relay connection".to_string()),
            last_error: None,
            last_connected_at_ms: None,
            updated_at_ms: now_epoch_ms() as u64,
        });
        ledger.push_relay_activity(PylonRelayActivity {
            at_ms: now_epoch_ms() as u64,
            url: Some(normalized.clone()),
            kind: "relay.refresh".to_string(),
            detail: "starting relay connectivity probe".to_string(),
        });
        Ok(())
    })?;

    let connection = RelayConnection::with_config(
        normalized.as_str(),
        RelayConfig {
            connect_timeout: Duration::from_secs(config.relay_connect_timeout_seconds.max(1)),
            nip42_identity: identity.map(|identity| RelayAuthIdentity {
                private_key_hex: identity.private_key_hex.clone(),
            }),
        },
    );
    match connection {
        Ok(connection) => {
            let mut attempt = 0_u8;
            loop {
                attempt = attempt.saturating_add(1);
                match connection.connect().await {
                    Ok(()) => {
                        let state = match connection.state().await {
                            ConnectionState::Connected => "connected",
                            ConnectionState::Connecting => "connecting",
                            ConnectionState::Disconnected => "disconnected",
                        };
                        let (auth_state, auth_detail, auth_error) =
                            observe_relay_auth_state(&connection, config.relay_auth_enabled).await;
                        let _ = connection.disconnect().await;
                        mutate_ledger(config_path, |ledger| {
                            ledger.upsert_relay_state(PylonRelayState {
                                url: normalized.clone(),
                                connection_state: if attempt > 1 {
                                    "reconnected".to_string()
                                } else {
                                    state.to_string()
                                },
                                auth_state: auth_state.clone(),
                                last_detail: Some(auth_detail.clone()),
                                last_error: auth_error.clone(),
                                last_connected_at_ms: Some(now_epoch_ms() as u64),
                                updated_at_ms: now_epoch_ms() as u64,
                            });
                            ledger.push_relay_activity(PylonRelayActivity {
                                at_ms: now_epoch_ms() as u64,
                                url: Some(normalized.clone()),
                                kind: if attempt > 1 {
                                    "relay.reconnected".to_string()
                                } else {
                                    "relay.connected".to_string()
                                },
                                detail: auth_detail.clone(),
                            });
                            if auth_state == "challenged" {
                                ledger.push_relay_activity(PylonRelayActivity {
                                    at_ms: now_epoch_ms() as u64,
                                    url: Some(normalized.clone()),
                                    kind: "relay.auth_challenge".to_string(),
                                    detail: "relay issued AUTH challenge and Pylon responded"
                                        .to_string(),
                                });
                            }
                            if let Some(auth_error) = auth_error.as_ref() {
                                ledger.push_relay_activity(PylonRelayActivity {
                                    at_ms: now_epoch_ms() as u64,
                                    url: Some(normalized.clone()),
                                    kind: "relay.auth_error".to_string(),
                                    detail: auth_error.clone(),
                                });
                            }
                            Ok(())
                        })?;
                        break;
                    }
                    Err(error) if attempt < 2 => {
                        mutate_ledger(config_path, |ledger| {
                            ledger.push_relay_activity(PylonRelayActivity {
                                at_ms: now_epoch_ms() as u64,
                                url: Some(normalized.clone()),
                                kind: "relay.reconnect_attempt".to_string(),
                                detail: error.to_string(),
                            });
                            Ok(())
                        })?;
                        tokio::time::sleep(Duration::from_millis(150)).await;
                    }
                    Err(error) => {
                        mutate_ledger(config_path, |ledger| {
                            ledger.upsert_relay_state(PylonRelayState {
                                url: normalized.clone(),
                                connection_state: "error".to_string(),
                                auth_state: if config.relay_auth_enabled {
                                    "enabled".to_string()
                                } else {
                                    "disabled".to_string()
                                },
                                last_detail: Some("relay connectivity probe failed".to_string()),
                                last_error: Some(error.to_string()),
                                last_connected_at_ms: None,
                                updated_at_ms: now_epoch_ms() as u64,
                            });
                            ledger.push_relay_activity(PylonRelayActivity {
                                at_ms: now_epoch_ms() as u64,
                                url: Some(normalized.clone()),
                                kind: if attempt > 1 {
                                    "relay.reconnect_failed".to_string()
                                } else {
                                    "relay.connect_error".to_string()
                                },
                                detail: error.to_string(),
                            });
                            Ok(())
                        })?;
                        break;
                    }
                }
            }
        }
        Err(error) => {
            mutate_ledger(config_path, |ledger| {
                ledger.upsert_relay_state(PylonRelayState {
                    url: normalized.clone(),
                    connection_state: "error".to_string(),
                    auth_state: if config.relay_auth_enabled {
                        "enabled".to_string()
                    } else {
                        "disabled".to_string()
                    },
                    last_detail: Some("relay URL is invalid".to_string()),
                    last_error: Some(error.to_string()),
                    last_connected_at_ms: None,
                    updated_at_ms: now_epoch_ms() as u64,
                });
                ledger.push_relay_activity(PylonRelayActivity {
                    at_ms: now_epoch_ms() as u64,
                    url: Some(normalized.clone()),
                    kind: "relay.invalid".to_string(),
                    detail: error.to_string(),
                });
                Ok(())
            })?;
        }
    }

    Ok(())
}

async fn observe_relay_auth_state(
    connection: &RelayConnection,
    auth_enabled: bool,
) -> (String, String, Option<String>) {
    if !auth_enabled {
        return (
            "disabled".to_string(),
            "relay auth disabled in local config".to_string(),
            None,
        );
    }

    let deadline = Instant::now() + Duration::from_millis(250);
    let mut challenged = false;
    let mut auth_error = None;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let wait = remaining.min(Duration::from_millis(50));
        match tokio::time::timeout(wait, connection.recv()).await {
            Ok(Ok(Some(RelayMessage::Auth(_)))) => {
                challenged = true;
            }
            Ok(Ok(Some(RelayMessage::Notice(notice)))) => {
                if notice.to_ascii_lowercase().contains("auth") {
                    auth_error = Some(notice);
                    break;
                }
            }
            Ok(Ok(Some(_))) | Ok(Ok(None)) | Ok(Err(_)) | Err(_) => {}
        }
    }

    if let Some(error) = auth_error {
        return (
            "failed".to_string(),
            "relay auth failed during the last refresh".to_string(),
            Some(error),
        );
    }
    if challenged {
        return (
            "challenged".to_string(),
            "relay issued AUTH and Pylon responded with the local node identity".to_string(),
            None,
        );
    }
    (
        "enabled".to_string(),
        "relay auth identity is ready for challenge flows".to_string(),
        None,
    )
}

async fn load_sandbox_report(config_path: &Path, limit: Option<usize>) -> Result<SandboxReport> {
    let (config, status) = load_config_and_status(config_path).await?;
    let (runtimes, profiles, last_scan_error) = if config_path.exists() {
        let runtimes = if let Some(runtimes) = try_live_json::<Vec<ProviderSandboxRuntimeHealth>>(
            &config,
            sandbox_runtimes_endpoint(limit).as_str(),
        )
        .await?
        {
            runtimes
        } else if let Some(store) = open_existing_store(&config)? {
            store
                .load_sandbox_runtimes(limit)
                .map_err(anyhow::Error::msg)?
        } else {
            take_limited_rows(
                status
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.availability.sandbox.runtimes.clone())
                    .unwrap_or_default(),
                limit,
            )
        };
        let profiles = if let Some(profiles) = try_live_json::<Vec<ProviderSandboxProfile>>(
            &config,
            sandbox_profiles_endpoint(limit).as_str(),
        )
        .await?
        {
            profiles
        } else if let Some(store) = open_existing_store(&config)? {
            store
                .load_sandbox_profiles(limit)
                .map_err(anyhow::Error::msg)?
        } else {
            take_limited_rows(
                status
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.availability.sandbox.profiles.clone())
                    .unwrap_or_default(),
                limit,
            )
        };
        let last_scan_error = status
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.availability.sandbox.last_scan_error.clone());
        (runtimes, profiles, last_scan_error)
    } else {
        (Vec::new(), Vec::new(), None)
    };

    let supported_execution_classes = profiles
        .iter()
        .map(|profile| profile.execution_class.product_id().to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let ready_execution_classes = profiles
        .iter()
        .filter(|profile| profile.runtime_ready)
        .map(|profile| profile.execution_class.product_id().to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    Ok(SandboxReport {
        context: report_context(&status),
        supported_execution_classes,
        ready_execution_classes,
        last_scan_error,
        runtimes,
        profiles,
    })
}

pub async fn load_jobs_report(config_path: &Path, limit: Option<usize>) -> Result<JobsReport> {
    let (config, status) = load_config_and_status(config_path).await?;
    let jobs = if config_path.exists() {
        if let Some(jobs) =
            try_live_json::<Vec<ProviderRecentJob>>(&config, jobs_endpoint(limit).as_str()).await?
        {
            jobs
        } else if let Some(store) = open_existing_store(&config)? {
            store.load_recent_jobs(limit).map_err(anyhow::Error::msg)?
        } else {
            take_limited_rows(
                status
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.recent_jobs.clone())
                    .unwrap_or_default(),
                limit,
            )
        }
    } else {
        Vec::new()
    };
    let jobs = merge_ledger_recent_jobs(jobs, &load_ledger(config_path).unwrap_or_default(), limit);
    Ok(JobsReport {
        context: report_context(&status),
        jobs,
    })
}

pub async fn load_earnings_report(config_path: &Path) -> Result<EarningsReport> {
    let (config, status) = load_config_and_status(config_path).await?;
    let earnings = if config_path.exists() {
        if let Some(earnings) =
            try_live_json::<Option<ProviderEarningsSummary>>(&config, "/v1/earnings").await?
        {
            earnings
        } else if let Some(store) = open_existing_store(&config)? {
            store
                .load_status()
                .map_err(anyhow::Error::msg)?
                .snapshot
                .and_then(|snapshot| snapshot.earnings)
        } else {
            status
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.earnings.clone())
        }
    } else {
        None
    };
    let earnings = merge_ledger_earnings(
        earnings,
        &load_ledger(config_path).unwrap_or_default(),
        &status,
    );
    Ok(EarningsReport {
        context: report_context(&status),
        earnings,
    })
}

pub async fn load_receipts_report(
    config_path: &Path,
    limit: Option<usize>,
) -> Result<ReceiptsReport> {
    let (config, status) = load_config_and_status(config_path).await?;
    let receipts = if config_path.exists() {
        if let Some(receipts) =
            try_live_json::<Vec<ProviderReceiptSummary>>(&config, receipts_endpoint(limit).as_str())
                .await?
        {
            receipts
        } else if let Some(store) = open_existing_store(&config)? {
            store.load_receipts(limit).map_err(anyhow::Error::msg)?
        } else {
            take_limited_rows(
                status
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.receipts.clone())
                    .unwrap_or_default(),
                limit,
            )
        }
    } else {
        Vec::new()
    };
    let receipts = merge_ledger_receipts(
        receipts,
        &load_ledger(config_path).unwrap_or_default(),
        limit,
    );
    Ok(ReceiptsReport {
        context: report_context(&status),
        receipts,
    })
}

pub fn load_relay_activity_report(
    config_path: &Path,
    limit: Option<usize>,
) -> Result<RelayActivityReport> {
    let mut entries = load_ledger(config_path).unwrap_or_default().relay_activity;
    entries.sort_by(|left, right| right.at_ms.cmp(&left.at_ms));
    if let Some(limit) = limit {
        entries.truncate(limit);
    }
    Ok(RelayActivityReport { entries })
}

pub async fn load_payout_report(config_path: &Path, limit: Option<u32>) -> Result<PayoutReport> {
    let config = load_or_create_config(config_path)?;
    let earnings = load_earnings_report(config_path).await?;
    let ledger = load_ledger(config_path).unwrap_or_default();
    let wallet_balance = match load_wallet_status_report(config_path).await {
        Ok(report) => report.balance,
        Err(_) => WalletBalanceSnapshot {
            total_sats: ledger.wallet.last_balance_sats.unwrap_or(0),
            ..WalletBalanceSnapshot::default()
        },
    };
    let mut withdrawals = ledger.payouts;
    withdrawals.sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
    if let Some(limit) = limit {
        withdrawals.truncate(limit as usize);
    }
    let earnings = earnings.earnings.unwrap_or_default();
    Ok(PayoutReport {
        payout_destination: config.payout_destination,
        wallet_balance,
        earnings_lifetime_sats: earnings.lifetime_sats,
        earnings_sats_today: earnings.sats_today,
        jobs_today: earnings.jobs_today,
        last_job_result: earnings.last_job_result,
        withdrawals,
    })
}

pub async fn run_payout_withdrawal(
    config_path: &Path,
    payment_request: &str,
    amount_sats: Option<u64>,
) -> Result<PayoutWithdrawalReport> {
    let config = load_or_create_config(config_path)?;
    let payout_destination = config.payout_destination.clone();
    let payment_request = payment_request.trim().to_string();
    if payment_request.is_empty() {
        bail!("payout withdraw requires a payment request");
    }
    let result =
        pay_payout_invoice_report(config_path, payment_request.as_str(), amount_sats).await;
    match result {
        Ok(report) => {
            let detail = "provider withdrawal submitted".to_string();
            mutate_ledger(config_path, |ledger| {
                ledger.wallet.last_balance_sats = Some(report.post_balance.total_sats);
                ledger.wallet.last_balance_at_ms = Some(now_epoch_ms() as u64);
                ledger.upsert_wallet_payment(report.payment.clone());
                ledger.upsert_payout(PylonLedgerPayout {
                    payout_id: format!("withdrawal:{}", report.payment_id),
                    payment_id: Some(report.payment_id.clone()),
                    status: report.payment.status.clone(),
                    amount_sats: Some(report.payment.amount_sats),
                    fees_sats: Some(report.payment.fees_sats),
                    invoice: Some(payment_request.clone()),
                    payout_destination: payout_destination.clone(),
                    detail: Some(detail.clone()),
                    created_at_ms: now_epoch_ms() as u64,
                    updated_at_ms: now_epoch_ms() as u64,
                });
                ledger.push_relay_activity(PylonRelayActivity {
                    at_ms: now_epoch_ms() as u64,
                    url: None,
                    kind: "payout.withdrawal_submitted".to_string(),
                    detail: format!(
                        "provider withdrawal {} submitted for {}",
                        report.payment_id, payment_request
                    ),
                });
                Ok(())
            })?;
            Ok(PayoutWithdrawalReport {
                payout_destination,
                payment_id: report.payment_id,
                status: report.payment.status,
                amount_sats: report.payment.amount_sats,
                fees_sats: report.payment.fees_sats,
                invoice: payment_request,
                post_balance: report.post_balance,
                detail: Some(detail),
            })
        }
        Err(error) => {
            let error_string = error.to_string();
            let payout_id = format!("withdrawal-failed:{}", now_epoch_ms());
            mutate_ledger(config_path, |ledger| {
                ledger.upsert_payout(PylonLedgerPayout {
                    payout_id,
                    payment_id: None,
                    status: "failed".to_string(),
                    amount_sats,
                    fees_sats: None,
                    invoice: Some(payment_request.clone()),
                    payout_destination: payout_destination.clone(),
                    detail: Some(error_string.clone()),
                    created_at_ms: now_epoch_ms() as u64,
                    updated_at_ms: now_epoch_ms() as u64,
                });
                ledger.push_relay_activity(PylonRelayActivity {
                    at_ms: now_epoch_ms() as u64,
                    url: None,
                    kind: "payout.withdrawal_failed".to_string(),
                    detail: format!("provider withdrawal failed: {error_string}"),
                });
                Ok(())
            })?;
            Err(error)
        }
    }
}

async fn pay_payout_invoice_report(
    config_path: &Path,
    payment_request: &str,
    amount_sats: Option<u64>,
) -> Result<WalletPayReport> {
    #[cfg(test)]
    {
        if let Some(slot) = TEST_PAYOUT_PAY_HOOK.get() {
            if let Some(hook) = slot.lock().expect("test payout pay hook lock").as_ref() {
                return hook(payment_request, amount_sats);
            }
        }
    }
    pay_wallet_invoice_report(config_path, payment_request, amount_sats).await
}

fn merge_ledger_recent_jobs(
    mut jobs: Vec<ProviderRecentJob>,
    ledger: &PylonLedger,
    limit: Option<usize>,
) -> Vec<ProviderRecentJob> {
    let mut seen = jobs
        .iter()
        .map(|job| job.job_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    for job in ledger_provider_recent_jobs(ledger) {
        if seen.insert(job.job_id.clone()) {
            jobs.push(job);
        }
    }
    jobs.sort_by(|left, right| {
        right
            .completed_at_epoch_seconds
            .cmp(&left.completed_at_epoch_seconds)
    });
    take_limited_rows(jobs, limit)
}

fn merge_ledger_earnings(
    base: Option<ProviderEarningsSummary>,
    ledger: &PylonLedger,
    status: &ProviderStatusResponse,
) -> Option<ProviderEarningsSummary> {
    if !ledger.jobs.iter().any(|job| job.direction == "provider")
        && !ledger
            .settlements
            .iter()
            .any(|settlement| settlement.direction == "provider")
    {
        return base;
    }
    let mut earnings = base
        .or_else(|| {
            status
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.earnings.clone())
        })
        .unwrap_or_else(default_earnings_summary);
    let current_day = (now_epoch_ms() as u64) / 86_400_000;
    let provider_jobs = ledger
        .jobs
        .iter()
        .filter(|job| job.direction == "provider")
        .collect::<Vec<_>>();
    let settled = ledger
        .settlements
        .iter()
        .filter(|settlement| settlement.direction == "provider" && settlement.status == "settled")
        .collect::<Vec<_>>();
    earnings.lifetime_sats = settled
        .iter()
        .map(|settlement| msats_to_sats_rounded_up(settlement.amount_msats))
        .sum();
    earnings.sats_today = settled
        .iter()
        .filter(|settlement| settlement.updated_at_ms / 86_400_000 == current_day)
        .map(|settlement| msats_to_sats_rounded_up(settlement.amount_msats))
        .sum();
    earnings.jobs_today = settled
        .iter()
        .filter(|settlement| settlement.updated_at_ms / 86_400_000 == current_day)
        .count() as u64;
    if let Some(latest_job) = provider_jobs.first() {
        earnings.last_job_result = latest_job.status.clone();
    }
    let terminal_jobs = provider_jobs
        .iter()
        .filter(|job| {
            matches!(
                job.status.as_str(),
                "settled"
                    | "completed_local"
                    | "failed_local"
                    | "publish_failed"
                    | "invoice_failed"
                    | "delivery_failed_after_payment"
            )
        })
        .count();
    let successful_jobs = provider_jobs
        .iter()
        .filter(|job| matches!(job.status.as_str(), "settled" | "completed_local"))
        .count();
    if terminal_jobs > 0 {
        earnings.completion_ratio_bps =
            Some(((successful_jobs * 10_000) / terminal_jobs).min(10_000) as u16);
    }
    let payable_jobs = provider_jobs
        .iter()
        .filter(|job| job.bid_msats.is_some())
        .count();
    if payable_jobs > 0 {
        earnings.payout_success_ratio_bps =
            Some(((settled.len() * 10_000) / payable_jobs).min(10_000) as u16);
    }
    Some(earnings)
}

fn merge_ledger_receipts(
    mut receipts: Vec<ProviderReceiptSummary>,
    ledger: &PylonLedger,
    limit: Option<usize>,
) -> Vec<ProviderReceiptSummary> {
    let mut seen = receipts
        .iter()
        .map(|receipt| receipt.receipt_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    for receipt in ledger_receipt_summaries(ledger) {
        if seen.insert(receipt.receipt_id.clone()) {
            receipts.push(receipt);
        }
    }
    receipts.sort_by(|left, right| right.created_at_ms.cmp(&left.created_at_ms));
    take_limited_rows(receipts, limit)
}

fn ledger_provider_recent_jobs(ledger: &PylonLedger) -> Vec<ProviderRecentJob> {
    ledger
        .jobs
        .iter()
        .filter(|job| job.direction == "provider")
        .map(|job| ProviderRecentJob {
            job_id: job.id.clone(),
            request_id: job.request_event_id.clone(),
            status: job.status.clone(),
            demand_source: "nostr_nip90".to_string(),
            product_id: None,
            compute_family: Some("text_generation".to_string()),
            backend_family: None,
            sandbox_execution_class: None,
            sandbox_profile_id: None,
            sandbox_profile_digest: None,
            sandbox_termination_reason: None,
            completed_at_epoch_seconds: job.updated_at_ms / 1000,
            payout_sats: if job.status == "settled" {
                msats_to_sats_rounded_up(job.amount_msats.unwrap_or(0))
            } else {
                0
            },
            payment_pointer: job
                .payment_id
                .clone()
                .or_else(|| job.bolt11.clone())
                .unwrap_or_else(|| "none".to_string()),
            failure_reason: job.error_detail.clone(),
            delivery_proof_id: job.result_event_id.clone(),
        })
        .collect()
}

fn ledger_receipt_summaries(ledger: &PylonLedger) -> Vec<ProviderReceiptSummary> {
    ledger
        .settlements
        .iter()
        .filter(|settlement| settlement.direction == "provider")
        .map(|settlement| {
            let reason_code = settlement.status.to_ascii_uppercase();
            let failed = settlement.status.contains("failed");
            ProviderReceiptSummary {
                receipt_id: settlement.settlement_id.clone(),
                receipt_type: match settlement.status.as_str() {
                    "settled" => "earn.job.settled.v1".to_string(),
                    "payment_received" => "nip90.provider.payment_received.v1".to_string(),
                    _ => "nip90.provider.settlement.v1".to_string(),
                },
                created_at_ms: settlement.updated_at_ms as i64,
                canonical_hash: settlement_canonical_hash(settlement),
                compute_family: Some("text_generation".to_string()),
                backend_family: None,
                sandbox_execution_class: None,
                sandbox_profile_id: None,
                sandbox_profile_digest: None,
                sandbox_termination_reason: None,
                reason_code: Some(reason_code),
                failure_reason: failed.then(|| {
                    settlement
                        .receipt_detail
                        .clone()
                        .unwrap_or_else(|| "delivery failed after payment".to_string())
                }),
                severity: Some(if failed { "warn" } else { "low" }.to_string()),
                notional_sats: Some(msats_to_sats_rounded_up(settlement.amount_msats)),
                liability_premium_sats: Some(0),
                work_unit_id: Some(settlement.job_id.clone()),
            }
        })
        .collect()
}

fn settlement_canonical_hash(settlement: &PylonSettlementRecord) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(settlement.settlement_id.as_bytes());
    hasher.update(b"|");
    hasher.update(settlement.job_id.as_bytes());
    hasher.update(b"|");
    hasher.update(settlement.status.as_bytes());
    hasher.update(b"|");
    hasher.update(settlement.amount_msats.to_string().as_bytes());
    if let Some(payment_reference) = settlement.payment_reference.as_deref() {
        hasher.update(b"|");
        hasher.update(payment_reference.as_bytes());
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn msats_to_sats_rounded_up(amount_msats: u64) -> u64 {
    amount_msats.saturating_add(999) / 1000
}

fn open_existing_store(config: &PylonConfig) -> Result<Option<ProviderPersistenceStore>> {
    if !config.admin_db_path.exists() {
        return Ok(None);
    }
    let admin_config = provider_admin_config(config)?;
    Ok(Some(
        ProviderPersistenceStore::open(&admin_config).map_err(anyhow::Error::msg)?,
    ))
}

fn inventory_endpoint(limit: Option<usize>) -> String {
    format!("/v1/inventory?limit={}", limit.unwrap_or(32))
}

fn sandbox_runtimes_endpoint(limit: Option<usize>) -> String {
    format!("/v1/sandbox/runtimes?limit={}", limit.unwrap_or(32))
}

fn sandbox_profiles_endpoint(limit: Option<usize>) -> String {
    format!("/v1/sandbox/profiles?limit={}", limit.unwrap_or(32))
}

fn jobs_endpoint(limit: Option<usize>) -> String {
    format!("/v1/jobs?limit={}", limit.unwrap_or(32))
}

fn receipts_endpoint(limit: Option<usize>) -> String {
    format!("/v1/receipts?limit={}", limit.unwrap_or(32))
}

fn take_limited_rows<T>(mut values: Vec<T>, limit: Option<usize>) -> Vec<T> {
    if let Some(limit) = limit {
        values.truncate(limit);
    }
    values
}

fn render_report_context(context: &ReportContext) -> Vec<String> {
    let mut lines = vec![
        format!("state: {}", context.state),
        format!("desired_mode: {}", context.desired_mode),
    ];
    if let Some(listen_addr) = context.listen_addr.as_deref() {
        lines.push(format!("listen_addr: {listen_addr}"));
    }
    lines
}

fn render_backend_report(report: &BackendReport) -> String {
    let mut lines = render_report_context(&report.context);
    for backend in &report.backends {
        lines.push(String::new());
        lines.push(format!("backend: {}", backend.backend_id));
        lines.push(format!("display_label: {}", backend.display_label));
        lines.push(format!("health_state: {}", backend.health_state));
        lines.push(format!(
            "launch_products: {}",
            comma_or_none(backend.launch_product_ids.as_slice())
        ));
        lines.push(format!(
            "eligible_products: {}",
            comma_or_none(backend.eligible_product_ids.as_slice())
        ));
        if !backend.supported_execution_classes.is_empty() {
            lines.push(format!(
                "supported_execution_classes: {}",
                comma_or_none(backend.supported_execution_classes.as_slice())
            ));
        }
        if !backend.ready_execution_classes.is_empty() {
            lines.push(format!(
                "ready_execution_classes: {}",
                comma_or_none(backend.ready_execution_classes.as_slice())
            ));
        }
        if !backend.runtime_kinds.is_empty() {
            lines.push(format!(
                "runtime_kinds: {}",
                comma_or_none(backend.runtime_kinds.as_slice())
            ));
        }
        if !backend.ready_runtime_kinds.is_empty() {
            lines.push(format!(
                "ready_runtime_kinds: {}",
                comma_or_none(backend.ready_runtime_kinds.as_slice())
            ));
        }
        if !backend.profile_ids.is_empty() {
            lines.push(format!(
                "profile_ids: {}",
                comma_or_none(backend.profile_ids.as_slice())
            ));
        }
        lines.push(format!(
            "ready_model: {}",
            backend.ready_model.as_deref().unwrap_or("none")
        ));
        lines.push(format!(
            "available_models: {}",
            comma_or_none(backend.available_models.as_slice())
        ));
        if let Some(message) = backend.availability_message.as_deref() {
            lines.push(format!("availability_message: {message}"));
        }
        if let Some(last_error) = backend.last_error.as_deref() {
            lines.push(format!("last_error: {last_error}"));
        }
    }
    lines.join("\n")
}

fn render_inventory_report(report: &InventoryReport) -> String {
    let mut lines = render_report_context(&report.context);
    for row in &report.rows {
        lines.push(String::new());
        lines.push(format!("product: {}", row.target.product_id()));
        lines.push(format!("enabled: {}", row.enabled));
        lines.push(format!("backend_ready: {}", row.backend_ready));
        lines.push(format!("eligible: {}", row.eligible));
        lines.push(format!("delivery_state: {}", row.delivery_state));
        lines.push(format!(
            "quantity: total={} reserved={} available={}",
            row.total_quantity, row.reserved_quantity, row.available_quantity
        ));
        lines.push(format!("capability: {}", row.capability_summary));
    }
    lines.join("\n")
}

fn render_product_report(report: &ProductReport) -> String {
    let mut lines = render_report_context(&report.context);
    for product in &report.products {
        lines.push(String::new());
        lines.push(format!("product: {}", product.product_id));
        lines.push(format!("display_label: {}", product.display_label));
        lines.push(format!("family: {}", product.compute_family));
        lines.push(format!("backend: {}", product.backend));
        lines.push(format!("enabled: {}", product.enabled));
        lines.push(format!("backend_ready: {}", product.backend_ready));
        lines.push(format!("eligible: {}", product.eligible));
        lines.push(format!("price_floor_sats: {}", product.price_floor_sats));
        lines.push(format!("terms: {}", product.terms_label));
        lines.push(format!("forward_terms: {}", product.forward_terms_label));
        lines.push(format!("capability: {}", product.capability_summary));
    }
    lines.join("\n")
}

pub fn render_relay_report(report: &RelayReport) -> String {
    let mut lines = vec![
        format!(
            "connect_timeout_seconds: {}",
            report.relay_config.connect_timeout_seconds
        ),
        format!("auth_enabled: {}", report.relay_config.auth_enabled),
        format!("ledger_path: {}", report.relay_config.ledger_path),
    ];
    if report.relays.is_empty() {
        lines.push(String::new());
        lines.push("relays: none configured".to_string());
        return lines.join("\n");
    }
    for relay in &report.relays {
        lines.push(String::new());
        lines.push(format!("relay: {}", relay.url));
        lines.push(format!("state: {}", relay.state));
        lines.push(format!("auth_state: {}", relay.auth_state));
        if let Some(detail) = relay.detail.as_deref() {
            lines.push(format!("detail: {detail}"));
        }
        if let Some(last_error) = relay.last_error.as_deref() {
            lines.push(format!("last_error: {last_error}"));
        }
        if let Some(last_connected_at_ms) = relay.last_connected_at_ms {
            lines.push(format!("last_connected_at_ms: {last_connected_at_ms}"));
        }
        if let Some(updated_at_ms) = relay.updated_at_ms {
            lines.push(format!("updated_at_ms: {updated_at_ms}"));
        }
    }
    lines.join("\n")
}

fn normalize_relay_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn validate_and_normalize_relay_url(url: &str) -> Result<String> {
    let normalized = normalize_relay_url(url);
    if normalized.is_empty() {
        bail!("relay URL cannot be empty");
    }
    let _ = RelayConnection::with_config(normalized.as_str(), RelayConfig::default())
        .with_context(|| format!("invalid relay URL: {normalized}"))?;
    Ok(normalized)
}

pub fn render_jobs_report(report: &JobsReport) -> String {
    let mut lines = render_report_context(&report.context);
    for job in &report.jobs {
        lines.push(String::new());
        lines.push(format!("job_id: {}", job.job_id));
        lines.push(format!("status: {}", job.status));
        lines.push(format!("demand_source: {}", job.demand_source));
        lines.push(format!(
            "product_id: {}",
            job.product_id.as_deref().unwrap_or("none")
        ));
        if let Some(compute_family) = job.compute_family.as_deref() {
            lines.push(format!("compute_family: {compute_family}"));
        }
        if let Some(backend_family) = job.backend_family.as_deref() {
            lines.push(format!("backend_family: {backend_family}"));
        }
        if let Some(execution_class) = job.sandbox_execution_class.as_deref() {
            lines.push(format!("sandbox_execution_class: {execution_class}"));
        }
        if let Some(profile_id) = job.sandbox_profile_id.as_deref() {
            lines.push(format!("sandbox_profile_id: {profile_id}"));
        }
        if let Some(profile_digest) = job.sandbox_profile_digest.as_deref() {
            lines.push(format!("sandbox_profile_digest: {profile_digest}"));
        }
        if let Some(termination_reason) = job.sandbox_termination_reason.as_deref() {
            lines.push(format!("sandbox_termination_reason: {termination_reason}"));
        }
        lines.push(format!("payout_sats: {}", job.payout_sats));
        if let Some(failure_reason) = job.failure_reason.as_deref() {
            lines.push(format!("failure_reason: {failure_reason}"));
        }
    }
    lines.join("\n")
}

pub fn render_earnings_report(report: &EarningsReport) -> String {
    let mut lines = render_report_context(&report.context);
    match report.earnings.as_ref() {
        Some(earnings) => {
            lines.push(String::new());
            lines.push(format!("sats_today: {}", earnings.sats_today));
            lines.push(format!("lifetime_sats: {}", earnings.lifetime_sats));
            lines.push(format!("jobs_today: {}", earnings.jobs_today));
            lines.push(format!(
                "online_uptime_seconds: {}",
                earnings.online_uptime_seconds
            ));
            lines.push(format!("last_job_result: {}", earnings.last_job_result));
        }
        None => {
            lines.push(String::new());
            lines.push("earnings: none".to_string());
        }
    }
    lines.join("\n")
}

pub fn render_payout_report(report: &PayoutReport) -> String {
    let mut lines = vec![
        format!(
            "payout_destination: {}",
            report.payout_destination.as_deref().unwrap_or("none")
        ),
        format!("wallet_total_sats: {}", report.wallet_balance.total_sats),
        format!(
            "wallet_lightning_sats: {}",
            report.wallet_balance.lightning_sats
        ),
        format!(
            "wallet_onchain_sats: {}",
            report.wallet_balance.onchain_sats
        ),
        format!("earned_lifetime_sats: {}", report.earnings_lifetime_sats),
        format!("earned_sats_today: {}", report.earnings_sats_today),
        format!("jobs_today: {}", report.jobs_today),
        format!("last_job_result: {}", report.last_job_result),
        format!("withdrawals: {}", report.withdrawals.len()),
    ];
    for payout in &report.withdrawals {
        lines.push(String::new());
        lines.push(format!("payout_id: {}", payout.payout_id));
        lines.push(format!("status: {}", payout.status));
        lines.push(format!(
            "payment_id: {}",
            payout.payment_id.as_deref().unwrap_or("none")
        ));
        lines.push(format!(
            "amount_sats: {}",
            payout
                .amount_sats
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
        lines.push(format!(
            "fees_sats: {}",
            payout
                .fees_sats
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
        lines.push(format!(
            "invoice: {}",
            payout.invoice.as_deref().unwrap_or("none")
        ));
        if let Some(detail) = payout.detail.as_deref() {
            lines.push(format!("detail: {detail}"));
        }
    }
    lines.join("\n")
}

pub fn render_payout_withdrawal_report(report: &PayoutWithdrawalReport) -> String {
    let mut lines = vec![
        format!(
            "payout_destination: {}",
            report.payout_destination.as_deref().unwrap_or("none")
        ),
        format!("payment_id: {}", report.payment_id),
        format!("status: {}", report.status),
        format!("amount_sats: {}", report.amount_sats),
        format!("fees_sats: {}", report.fees_sats),
        format!("invoice: {}", report.invoice),
        format!(
            "post_balance_total_sats: {}",
            report.post_balance.total_sats
        ),
    ];
    if let Some(detail) = report.detail.as_deref() {
        lines.push(format!("detail: {detail}"));
    }
    lines.join("\n")
}

pub fn render_receipts_report(report: &ReceiptsReport) -> String {
    let mut lines = render_report_context(&report.context);
    for receipt in &report.receipts {
        lines.push(String::new());
        lines.push(format!("receipt_id: {}", receipt.receipt_id));
        lines.push(format!("receipt_type: {}", receipt.receipt_type));
        lines.push(format!("canonical_hash: {}", receipt.canonical_hash));
        lines.push(format!("created_at_ms: {}", receipt.created_at_ms));
        if let Some(compute_family) = receipt.compute_family.as_deref() {
            lines.push(format!("compute_family: {compute_family}"));
        }
        if let Some(backend_family) = receipt.backend_family.as_deref() {
            lines.push(format!("backend_family: {backend_family}"));
        }
        if let Some(execution_class) = receipt.sandbox_execution_class.as_deref() {
            lines.push(format!("sandbox_execution_class: {execution_class}"));
        }
        if let Some(profile_id) = receipt.sandbox_profile_id.as_deref() {
            lines.push(format!("sandbox_profile_id: {profile_id}"));
        }
        if let Some(profile_digest) = receipt.sandbox_profile_digest.as_deref() {
            lines.push(format!("sandbox_profile_digest: {profile_digest}"));
        }
        if let Some(termination_reason) = receipt.sandbox_termination_reason.as_deref() {
            lines.push(format!("sandbox_termination_reason: {termination_reason}"));
        }
        if let Some(reason_code) = receipt.reason_code.as_deref() {
            lines.push(format!("reason_code: {reason_code}"));
        }
        if let Some(failure_reason) = receipt.failure_reason.as_deref() {
            lines.push(format!("failure_reason: {failure_reason}"));
        }
        if let Some(notional_sats) = receipt.notional_sats {
            lines.push(format!("notional_sats: {notional_sats}"));
        }
    }
    lines.join("\n")
}

pub fn render_relay_activity_report(report: &RelayActivityReport) -> String {
    if report.entries.is_empty() {
        return "activity: none".to_string();
    }
    let mut lines = vec![format!("activity_entries: {}", report.entries.len())];
    for entry in &report.entries {
        lines.push(String::new());
        lines.push(format!("at_ms: {}", entry.at_ms));
        lines.push(format!("kind: {}", entry.kind));
        lines.push(format!(
            "relay: {}",
            entry.url.as_deref().unwrap_or("local")
        ));
        lines.push(format!("detail: {}", entry.detail));
    }
    lines.join("\n")
}

fn render_sandbox_report(report: &SandboxReport) -> String {
    let mut lines = render_report_context(&report.context);
    lines.push(String::new());
    lines.push(format!(
        "supported_execution_classes: {}",
        comma_or_none(report.supported_execution_classes.as_slice())
    ));
    lines.push(format!(
        "ready_execution_classes: {}",
        comma_or_none(report.ready_execution_classes.as_slice())
    ));
    if let Some(last_scan_error) = report.last_scan_error.as_deref() {
        lines.push(format!("last_scan_error: {last_scan_error}"));
    }
    for runtime in &report.runtimes {
        let supported_execution_classes = runtime
            .supported_execution_classes
            .iter()
            .map(|execution_class| execution_class.product_id().to_string())
            .collect::<Vec<_>>();
        lines.push(String::new());
        lines.push(format!("runtime_kind: {}", runtime.runtime_kind.id()));
        lines.push(format!("detected: {}", runtime.detected));
        lines.push(format!("ready: {}", runtime.ready));
        lines.push(format!(
            "supported_execution_classes: {}",
            comma_or_none(supported_execution_classes.as_slice())
        ));
        if let Some(binary_name) = runtime.binary_name.as_deref() {
            lines.push(format!("binary_name: {binary_name}"));
        }
        if let Some(binary_path) = runtime.binary_path.as_deref() {
            lines.push(format!("binary_path: {binary_path}"));
        }
        if let Some(runtime_version) = runtime.runtime_version.as_deref() {
            lines.push(format!("runtime_version: {runtime_version}"));
        }
        if let Some(last_error) = runtime.last_error.as_deref() {
            lines.push(format!("last_error: {last_error}"));
        }
    }
    for profile in &report.profiles {
        lines.push(String::new());
        lines.push(format!("profile_id: {}", profile.profile_id));
        lines.push(format!(
            "execution_class: {}",
            profile.execution_class.product_id()
        ));
        lines.push(format!("profile_digest: {}", profile.profile_digest));
        lines.push(format!("runtime_kind: {}", profile.runtime_kind.id()));
        lines.push(format!("runtime_ready: {}", profile.runtime_ready));
        lines.push(format!("network_mode: {}", profile.network_mode));
        lines.push(format!("filesystem_mode: {}", profile.filesystem_mode));
        lines.push(format!("timeout_limit_s: {}", profile.timeout_limit_s));
        if let Some(accelerator_policy) = profile.accelerator_policy.as_deref() {
            lines.push(format!("accelerator_policy: {accelerator_policy}"));
        }
    }
    lines.join("\n")
}

fn comma_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

#[derive(Debug, Serialize)]
struct NexusProviderPresenceHeartbeatRequest {
    nostr_pubkey_hex: String,
    session_id: String,
    node_label: Option<String>,
    client_version: Option<String>,
    relay_urls: Vec<String>,
    products: Vec<String>,
    eligible_product_count: u64,
    ready_model: Option<String>,
    runtime_state: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostic_summaries: Vec<ProviderDiagnosticSummary>,
}

#[derive(Debug, Serialize)]
struct NexusProviderPresenceOfflineRequest {
    nostr_pubkey_hex: String,
    session_id: String,
}

async fn report_provider_presence_heartbeat(
    client: &reqwest::Client,
    config_path: &Path,
    config: &PylonConfig,
    identity: &NostrIdentity,
    session_id: &str,
    snapshot: &ProviderPersistedSnapshot,
) -> Result<()> {
    let request = NexusProviderPresenceHeartbeatRequest {
        nostr_pubkey_hex: identity.public_key_hex.clone(),
        session_id: session_id.to_string(),
        node_label: Some(config.node_label.clone()),
        client_version: Some(format!("pylon/{}", env!("CARGO_PKG_VERSION"))),
        relay_urls: config.relay_urls.clone(),
        products: snapshot
            .inventory_rows
            .iter()
            .filter(|row| row.eligible)
            .map(|row| row.target.product_id().to_string())
            .collect(),
        eligible_product_count: snapshot
            .inventory_rows
            .iter()
            .filter(|row| row.eligible)
            .count() as u64,
        ready_model: gemma_ready_model(&snapshot.availability.local_gemma),
        runtime_state: snapshot
            .runtime
            .authoritative_status
            .clone()
            .or_else(|| Some(snapshot.runtime.mode.label().to_string())),
        diagnostic_summaries: load_latest_provider_diagnostic_summaries(config_path),
    };
    post_nexus_provider_presence(
        client,
        config,
        "/api/provider-presence/heartbeat",
        &request,
        "heartbeat",
    )
    .await
}

async fn report_provider_presence_offline(
    client: &reqwest::Client,
    config: &PylonConfig,
    identity: &NostrIdentity,
    session_id: &str,
) -> Result<()> {
    let request = NexusProviderPresenceOfflineRequest {
        nostr_pubkey_hex: identity.public_key_hex.clone(),
        session_id: session_id.to_string(),
    };
    post_nexus_provider_presence(
        client,
        config,
        "/api/provider-presence/offline",
        &request,
        "offline",
    )
    .await
}

async fn post_nexus_provider_presence<T: Serialize>(
    client: &reqwest::Client,
    config: &PylonConfig,
    path: &str,
    payload: &T,
    action: &str,
) -> Result<()> {
    let url = nexus_control_url(config, path);
    let response = client
        .post(url.as_str())
        .json(payload)
        .send()
        .await
        .with_context(|| format!("failed to post pylon provider presence {action} to {url}"))?;
    if !response.status().is_success() {
        let detail = response
            .text()
            .await
            .unwrap_or_else(|_| "failed to decode nexus provider presence error".to_string());
        bail!("nexus provider presence {action} failed: {detail}");
    }
    Ok(())
}

fn provider_presence_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("failed to build pylon provider-presence client")
}

fn nexus_control_url(config: &PylonConfig, path: &str) -> String {
    format!(
        "{}/{}",
        config.nexus_control_base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

async fn try_live_status(config: &PylonConfig) -> Result<Option<ProviderStatusResponse>> {
    let client = admin_client()?;
    let url = format!("http://{}/v1/status", config.admin_listen_addr);
    let response = match client.get(url.as_str()).send().await {
        Ok(response) => response,
        Err(error) if is_local_control_unavailable(&error) => return Ok(None),
        Err(error) => return Err(anyhow!("failed to query pylon admin status: {error}")),
    };
    if !response.status().is_success() {
        let payload = response
            .json::<Value>()
            .await
            .unwrap_or_else(|_| json!({"error": "failed to decode provider status error"}));
        bail!(
            "provider admin status request failed: {}",
            api_error_detail(&payload)
        );
    }
    let status = response
        .json::<ProviderStatusResponse>()
        .await
        .context("failed to decode provider admin status response")?;
    Ok(Some(status))
}

async fn try_live_json<T: DeserializeOwned>(
    config: &PylonConfig,
    endpoint: &str,
) -> Result<Option<T>> {
    let client = admin_client()?;
    let url = format!("http://{}{}", config.admin_listen_addr, endpoint);
    let response = match client.get(url.as_str()).send().await {
        Ok(response) => response,
        Err(error) if is_local_control_unavailable(&error) => return Ok(None),
        Err(error) => {
            return Err(anyhow!(
                "failed to query pylon admin endpoint {}: {error}",
                endpoint
            ));
        }
    };
    if !response.status().is_success() {
        let payload = response
            .json::<Value>()
            .await
            .unwrap_or_else(|_| json!({"error": "failed to decode provider admin error"}));
        bail!(
            "provider admin endpoint {} failed: {}",
            endpoint,
            api_error_detail(&payload)
        );
    }
    let value = response
        .json::<T>()
        .await
        .with_context(|| format!("failed to decode pylon admin endpoint {}", endpoint))?;
    Ok(Some(value))
}

async fn try_live_control(config: &PylonConfig, action: ProviderControlAction) -> Result<bool> {
    let client = admin_client()?;
    let endpoint = match action {
        ProviderControlAction::Online => "online",
        ProviderControlAction::Offline => "offline",
        ProviderControlAction::Pause => "pause",
        ProviderControlAction::Resume => "resume",
    };
    let url = format!("http://{}/v1/{endpoint}", config.admin_listen_addr);
    let response = match client.post(url.as_str()).send().await {
        Ok(response) => response,
        Err(error) if is_local_control_unavailable(&error) => return Ok(false),
        Err(error) => {
            return Err(anyhow!(
                "failed to call pylon admin {} endpoint: {error}",
                action.label()
            ));
        }
    };
    if !response.status().is_success() {
        let payload = response
            .json::<Value>()
            .await
            .unwrap_or_else(|_| json!({"error": "failed to decode provider control error"}));
        bail!(
            "provider admin {} failed: {}",
            action.label(),
            api_error_detail(&payload)
        );
    }
    response
        .json::<ProviderStatusResponse>()
        .await
        .with_context(|| format!("failed to decode {} control response", action.label()))?;
    Ok(true)
}

fn admin_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("failed to build pylon admin client")
}

fn is_local_control_unavailable(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout() || error.to_string().contains("Connection refused")
}

fn api_error_detail(payload: &Value) -> String {
    let code = payload.get("code").and_then(Value::as_str);
    let error = payload
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("unknown error");
    let current_state = payload.get("current_state").and_then(Value::as_str);
    match (code, current_state) {
        (Some(code), Some(current_state)) => {
            format!("{code}: {error} (current_state={current_state})")
        }
        (Some(code), None) => format!("{code}: {error}"),
        (None, _) => error.to_string(),
    }
}

async fn detect_availability(config: &PylonConfig) -> Result<ProviderAvailability> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("failed to build pylon health-check client")?;
    let local_gemma = detect_local_gemma(&client, config).await;
    let sandbox = detect_sandbox_supply(
        &ProviderSandboxDetectionConfig::default()
            .with_declared_profiles(config.declared_sandbox_profiles.clone()),
    );
    Ok(ProviderAvailability {
        local_gemma,
        apple_foundation_models: ProviderBackendHealth::default(),
        apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
        adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
        pooled_inference: ProviderPooledInferenceAvailability::default(),
        sandbox,
    })
}

async fn detect_local_gemma(
    client: &reqwest::Client,
    config: &PylonConfig,
) -> ProviderBackendHealth {
    if !config.inventory_controls.local_gemma_inference_enabled
        && !config.inventory_controls.local_gemma_embeddings_enabled
    {
        return ProviderBackendHealth {
            last_action: Some("disabled by config".to_string()),
            ..ProviderBackendHealth::default()
        };
    }
    let url = format!(
        "{}/api/tags",
        config.local_gemma_base_url.trim_end_matches('/')
    );
    let response = match client.get(url.as_str()).send().await {
        Ok(response) => response,
        Err(error) => {
            return ProviderBackendHealth {
                reachable: false,
                ready: false,
                last_error: Some(format!(
                    "local Gemma runtime not reachable at {url}; start a local runtime serving /api/tags or update local_gemma_base_url ({error})"
                )),
                last_action: Some("health check failed".to_string()),
                ..ProviderBackendHealth::default()
            };
        }
    };
    let payload = match response.json::<Value>().await {
        Ok(payload) => payload,
        Err(error) => {
            return ProviderBackendHealth {
                reachable: true,
                ready: false,
                last_error: Some(format!(
                    "local Gemma runtime at {url} returned an invalid /api/tags payload; verify the runtime speaks the expected API or update local_gemma_base_url ({error})"
                )),
                last_action: Some("invalid local Gemma health payload".to_string()),
                ..ProviderBackendHealth::default()
            };
        }
    };
    let models = payload
        .get("models")
        .and_then(Value::as_array)
        .map(|models| {
            models
                .iter()
                .filter_map(|model| {
                    model
                        .get("name")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let gemma_models = models
        .into_iter()
        .filter(|model| is_gemma_model(model))
        .collect::<Vec<_>>();
    let ready = !gemma_models.is_empty();
    ProviderBackendHealth {
        reachable: true,
        ready,
        configured_model: gemma_models.first().cloned(),
        ready_model: gemma_models.first().cloned(),
        available_models: gemma_models,
        last_error: (!ready).then(|| {
            format!(
                "local Gemma runtime at {url} is reachable, but no Gemma 4 model is loaded. Downloaded GGUF files alone do not make supply eligible; start the runtime with a Gemma model or update local_gemma_base_url."
            )
        }),
        last_action: Some(if ready {
            "health check ready".to_string()
        } else {
            "gemma models not loaded".to_string()
        }),
        availability_message: Some(if ready {
            "gemma_ready".to_string()
        } else {
            "gemma_missing".to_string()
        }),
        ..ProviderBackendHealth::default()
    }
}

fn apply_config_set(config: &mut PylonConfig, key: &str, value: &str) -> Result<()> {
    match key {
        "node_label" => config.node_label = value.to_string(),
        "payout_destination" => {
            config.payout_destination = if value.trim().is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        "admin_listen_addr" => config.admin_listen_addr = value.to_string(),
        "nexus_control_base_url" => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                bail!("nexus_control_base_url must not be empty");
            }
            config.nexus_control_base_url = trimmed.to_string();
        }
        "relay_connect_timeout_seconds" => {
            config.relay_connect_timeout_seconds = value
                .parse::<u64>()
                .with_context(|| format!("invalid relay_connect_timeout_seconds: {value}"))?;
        }
        "relay_auth_enabled" => {
            config.relay_auth_enabled = parse_bool(value)?;
        }
        "wallet_network" => config.wallet_network = value.trim().to_string(),
        "wallet_api_key_env" => {
            config.wallet_api_key_env = if value.trim().is_empty() {
                None
            } else {
                Some(value.trim().to_string())
            };
        }
        "buyer_auto_pay_enabled" => {
            config.buyer_auto_pay_enabled = parse_bool(value)?;
        }
        "wallet_storage_dir" => config.wallet_storage_dir = PathBuf::from(value.trim()),
        "local_gemma_base_url" | "ollama_base_url" => {
            config.local_gemma_base_url = value.to_string();
        }
        "backend.local_gemma_inference_enabled"
        | "backend.gpt_oss_inference_enabled"
        | "backend.ollama_inference_enabled" => {
            config.inventory_controls.local_gemma_inference_enabled = parse_bool(value)?;
        }
        "backend.local_gemma_embeddings_enabled"
        | "backend.gpt_oss_embeddings_enabled"
        | "backend.ollama_embeddings_enabled" => {
            config.inventory_controls.local_gemma_embeddings_enabled = parse_bool(value)?;
        }
        "backend.sandbox_container_exec_enabled" => {
            config.inventory_controls.sandbox_container_exec_enabled = parse_bool(value)?;
        }
        "backend.sandbox_python_exec_enabled" => {
            config.inventory_controls.sandbox_python_exec_enabled = parse_bool(value)?;
        }
        "backend.sandbox_node_exec_enabled" => {
            config.inventory_controls.sandbox_node_exec_enabled = parse_bool(value)?;
        }
        "backend.sandbox_posix_exec_enabled" => {
            config.inventory_controls.sandbox_posix_exec_enabled = parse_bool(value)?;
        }
        other => bail!("unsupported config key: {other}"),
    }
    Ok(())
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.trim() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        other => bail!("invalid boolean value: {other}"),
    }
}

fn now_epoch_ms() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_millis()).unwrap_or(i64::MAX),
        Err(_) => 0,
    }
}

fn random_token() -> String {
    hex::encode(rand::random::<[u8; 16]>())
}

#[cfg(test)]
type TestPayoutPayHook = Box<dyn Fn(&str, Option<u64>) -> Result<WalletPayReport> + Send + Sync>;

#[cfg(test)]
static TEST_PAYOUT_PAY_HOOK: std::sync::OnceLock<std::sync::Mutex<Option<TestPayoutPayHook>>> =
    std::sync::OnceLock::new();

#[cfg(test)]
fn set_test_payout_pay_hook(hook: Option<TestPayoutPayHook>) {
    let slot = TEST_PAYOUT_PAY_HOOK.get_or_init(|| std::sync::Mutex::new(None));
    *slot.lock().expect("test payout pay hook lock") = hook;
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{
        AnnouncementAction, BuyerJobSubmitRequest, Cli, Command, DEFAULT_GEMMA_BENCH_PROMPT,
        DEFAULT_GEMMA_DIAGNOSTIC_ID, GemmaBenchExecutionMode, GemmaBenchmarkMode,
        GemmaBenchmarkRequest, GemmaBenchmarkSelector, GemmaCommand, GemmaDiagnosticReceipt,
        GemmaDiagnosticReport, GemmaDiagnosticRequest, GemmaDiagnosticResult,
        GemmaDiagnosticRunReceipt, GemmaDownloadEvent, GemmaDownloadTransport, GemmaSelector,
        LocalGemmaChatBackend, LocalGemmaChatEvent, LocalGemmaChatMessage, PylonConfig,
        PylonWalletInvoiceRecord, PylonWalletPaymentRecord, WalletInvoiceReport,
        WalletRuntimeSurface, WalletSubcommand, add_configured_relay, apply_config_set,
        apply_control_command, build_snapshot_from_availability, default_config,
        download_gemma_model_from_base_url, download_gemma_model_from_base_url_with_transport,
        ensure_identity, gemma_diagnostic_latest_report_path, gemma_download_spec,
        gemma_local_installations, inventory_rows, load_backend_report, load_earnings_report,
        load_inventory_report, load_jobs_report, load_latest_gemma_diagnostic_report, load_ledger,
        load_or_create_config, load_product_report, load_receipts_report, load_relay_report,
        load_sandbox_report, load_status_or_detect, mutate_ledger, parse_args,
        planned_gemma_benchmark_modes, provider_admin_config, psionic_gemma_benchmark_command_args,
        publish_announcement_report, refresh_relay_report, remove_configured_relay,
        render_human_status, render_public_config_json, render_sandbox_report,
        report_provider_presence_heartbeat_for_snapshot,
        report_provider_presence_offline_for_config, resolve_local_gemma_chat_target_from_status,
        run_cli, run_gemma_diagnostic_command, run_local_gemma_chat_messages_stream,
        run_local_gemma_chat_stream, run_provider_requests, save_config,
        save_gemma_diagnostic_report, scan_provider_requests, submit_buyer_job, watch_buyer_jobs,
    };
    use futures_util::{SinkExt, StreamExt};
    use openagents_provider_substrate::{
        ProviderAdapterTrainingContributorAvailability, ProviderAppleAdapterHostingAvailability,
        ProviderAvailability, ProviderBackendHealth, ProviderControlAction, ProviderDesiredMode,
        ProviderEarningsSummary, ProviderInventoryControls, ProviderPersistenceStore,
        ProviderPooledInferenceAvailability, ProviderReceiptSummary, ProviderRecentJob,
        ProviderSandboxAvailability, ProviderSandboxExecutionClass, ProviderSandboxProfile,
        ProviderSandboxProfileSpec, ProviderSandboxRuntimeHealth, ProviderSandboxRuntimeKind,
        provider_runtime_state_label,
    };
    use serde_json::{Value, json};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    fn ensure(condition: bool, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        if condition {
            Ok(())
        } else {
            Err(std::io::Error::other(message.to_string()).into())
        }
    }

    #[test]
    fn parse_args_supports_status_json() -> Result<(), Box<dyn std::error::Error>> {
        let cli = parse_args(vec!["status".to_string(), "--json".to_string()])?;
        ensure(
            cli.command == Command::Status { json: true },
            "status --json should parse into json status command",
        )
    }

    #[test]
    fn parse_args_supports_lifecycle_commands() -> Result<(), Box<dyn std::error::Error>> {
        ensure(
            parse_args(vec!["online".to_string()])?.command == Command::Online,
            "online should parse into the online command",
        )?;
        ensure(
            parse_args(vec!["offline".to_string()])?.command == Command::Offline,
            "offline should parse into the offline command",
        )?;
        ensure(
            parse_args(vec!["pause".to_string()])?.command == Command::Pause,
            "pause should parse into the pause command",
        )?;
        ensure(
            parse_args(vec!["resume".to_string()])?.command == Command::Resume,
            "resume should parse into the resume command",
        )
    }

    #[test]
    fn config_set_updates_backend_flags() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = PylonConfig {
            inventory_controls: ProviderInventoryControls::default(),
            ..default_config(std::path::Path::new("/tmp/pylon-test"))
        };
        apply_config_set(&mut config, "backend.sandbox_python_exec_enabled", "true")?;
        ensure(
            config.inventory_controls.sandbox_python_exec_enabled,
            "config set should update sandbox python toggle",
        )
    }

    #[test]
    fn public_config_json_omits_legacy_apple_fm_surface() -> Result<(), Box<dyn std::error::Error>>
    {
        let config = default_config(std::path::Path::new("/tmp/pylon-test"));
        let json = render_public_config_json(&config)?;

        ensure(
            json.contains("\"local_gemma_base_url\""),
            "public config should expose the local Gemma base URL",
        )?;
        ensure(
            json.contains("\"nexus_control_base_url\""),
            "public config should expose the Nexus control base URL",
        )?;
        ensure(
            json.contains("\"local_gemma_inference_enabled\""),
            "public config should expose local Gemma inventory toggles",
        )?;
        ensure(
            !json.contains("apple_fm_inference_enabled")
                && !json.contains("apple_fm_adapter_hosting_enabled")
                && !json.contains("apple_fm_base_url"),
            "public config should omit legacy Apple FM-only config surface",
        )
    }

    #[test]
    fn config_set_rejects_legacy_apple_fm_surface() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = default_config(std::path::Path::new("/tmp/pylon-test"));

        ensure(
            apply_config_set(&mut config, "backend.apple_fm_inference_enabled", "false").is_err(),
            "config set should reject legacy Apple FM backend toggles",
        )?;
        ensure(
            apply_config_set(&mut config, "apple_fm_base_url", "http://127.0.0.1:11435").is_err(),
            "config set should reject legacy Apple FM base URL overrides",
        )
    }

    #[test]
    fn config_set_updates_payout_destination() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = default_config(std::path::Path::new("/tmp/pylon-test"));
        apply_config_set(&mut config, "payout_destination", "lnurlp:alice")?;
        ensure(
            config.payout_destination.as_deref() == Some("lnurlp:alice"),
            "config set should update payout destination",
        )
    }

    #[test]
    fn config_set_updates_relay_auth_toggle() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = default_config(std::path::Path::new("/tmp/pylon-test"));
        apply_config_set(&mut config, "relay_auth_enabled", "false")?;
        ensure(
            !config.relay_auth_enabled,
            "config set should update the relay auth toggle",
        )
    }

    #[test]
    fn config_set_updates_nexus_control_base_url() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = default_config(std::path::Path::new("/tmp/pylon-test"));
        apply_config_set(
            &mut config,
            "nexus_control_base_url",
            "https://nexus.example.com",
        )?;
        ensure(
            config.nexus_control_base_url == "https://nexus.example.com",
            "config set should update nexus_control_base_url",
        )
    }

    #[test]
    fn config_set_updates_wallet_fields() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = default_config(std::path::Path::new("/tmp/pylon-test"));
        apply_config_set(&mut config, "wallet_network", "regtest")?;
        apply_config_set(&mut config, "wallet_api_key_env", "PYLON_SPARK_KEY")?;
        apply_config_set(&mut config, "wallet_storage_dir", "/tmp/pylon-wallet")?;
        ensure(
            config.wallet_network == "regtest",
            "config set should update wallet_network",
        )?;
        ensure(
            config.wallet_api_key_env.as_deref() == Some("PYLON_SPARK_KEY"),
            "config set should update wallet_api_key_env",
        )?;
        ensure(
            config.wallet_storage_dir == std::path::Path::new("/tmp/pylon-wallet"),
            "config set should update wallet_storage_dir",
        )
    }

    #[test]
    fn config_set_updates_buyer_auto_pay_toggle() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = default_config(std::path::Path::new("/tmp/pylon-test"));
        apply_config_set(&mut config, "buyer_auto_pay_enabled", "true")?;
        ensure(
            config.buyer_auto_pay_enabled,
            "config set should update buyer_auto_pay_enabled",
        )
    }

    #[test]
    fn load_config_hydrates_missing_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        std::fs::write(
            config_path.as_path(),
            r#"{
  "schema_version": 1,
  "node_label": "pylon",
  "payout_destination": null,
  "identity_path": "/tmp/pylon-test/identity.mnemonic",
  "admin_db_path": "/tmp/pylon-test/provider-admin.sqlite",
  "admin_listen_addr": "127.0.0.1:9468",
  "relay_urls": ["wss://relay.example.com"]
}"#,
        )?;
        let config = super::load_config(config_path.as_path())?;
        ensure(
            config.wallet_storage_dir.ends_with("spark"),
            "missing wallet storage should hydrate from the default config",
        )?;
        ensure(
            config.local_gemma_base_url == "http://127.0.0.1:11434",
            "missing local Gemma endpoint config should hydrate from the default config",
        )?;
        ensure(
            config.nexus_control_base_url == "https://nexus.openagents.com",
            "missing nexus_control_base_url should hydrate from the default config",
        )?;
        ensure(
            config.inventory_controls.local_gemma_inference_enabled
                && !config.inventory_controls.apple_fm_inference_enabled
                && !config.inventory_controls.apple_fm_adapter_hosting_enabled,
            "missing inventory controls should hydrate the Gemma-first Pylon defaults",
        )
    }

    #[test]
    fn load_config_accepts_legacy_local_runtime_aliases() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        std::fs::write(
            config_path.as_path(),
            r#"{
  "schema_version": 1,
  "node_label": "pylon",
  "payout_destination": null,
  "identity_path": "/tmp/pylon-test/identity.mnemonic",
  "admin_db_path": "/tmp/pylon-test/provider-admin.sqlite",
  "admin_listen_addr": "127.0.0.1:9468",
  "wallet_storage_dir": "/tmp/pylon-test/spark",
  "ollama_base_url": "http://127.0.0.1:11435",
  "inventory_controls": {
    "gpt_oss_inference_enabled": false,
    "gpt_oss_embeddings_enabled": false,
    "apple_fm_inference_enabled": false,
    "apple_fm_adapter_hosting_enabled": false,
    "sandbox_container_exec_enabled": false,
    "sandbox_python_exec_enabled": false,
    "sandbox_node_exec_enabled": false,
    "sandbox_posix_exec_enabled": false
  }
}"#,
        )?;

        let config = super::load_config(config_path.as_path())?;

        ensure(
            config.local_gemma_base_url == "http://127.0.0.1:11435",
            "legacy ollama_base_url should hydrate the local Gemma base URL",
        )?;
        ensure(
            !config.inventory_controls.local_gemma_inference_enabled
                && !config.inventory_controls.local_gemma_embeddings_enabled,
            "legacy gpt_oss inventory flags should hydrate local Gemma controls",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn provider_presence_reports_online_and_offline_to_nexus()
    -> Result<(), Box<dyn std::error::Error>> {
        let recorded_requests = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
        let recorded_requests_for_server = Arc::clone(&recorded_requests);
        let nexus_base_url = start_mock_http_server(move |method, path, body| {
            let payload = serde_json::from_str::<Value>(body.as_str())
                .unwrap_or_else(|_| json!({"raw_body": body}));
            recorded_requests_for_server
                .lock()
                .expect("provider presence request log")
                .push((format!("{method} {path}"), payload));
            (200, "application/json", "{\"ok\":true}".to_string())
        })
        .await?;

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = default_config(temp_dir.path());
        config.nexus_control_base_url = nexus_base_url;
        save_config(config_path.as_path(), &config)?;
        let identity = ensure_identity(config.identity_path.as_path())?;
        let mut diagnostic_report = GemmaDiagnosticReport {
            schema_version: 1,
            report_kind: "pylon.gemma_diagnostic.report.v1".to_string(),
            selector: "gemma-4-e4b".to_string(),
            diagnostic_id: DEFAULT_GEMMA_DIAGNOSTIC_ID.to_string(),
            measured_at_unix_ms: 1_762_000_000_000,
            repeats: 2,
            report_path: String::new(),
            results: vec![GemmaDiagnosticResult {
                model_id: "gemma-4-e4b".to_string(),
                label: "Gemma 4 E4B".to_string(),
                runtime_model: Some("gemma4:e4b".to_string()),
                runtime_backend: "local_runtime".to_string(),
                status: "completed".to_string(),
                reason: None,
                model_cached: true,
                ready_in_runtime: true,
                receipt: Some(GemmaDiagnosticReceipt {
                    schema_version: 1,
                    report_kind: "pylon.gemma_diagnostic.receipt.v1".to_string(),
                    diagnostic_id: DEFAULT_GEMMA_DIAGNOSTIC_ID.to_string(),
                    measured_at_unix_ms: 1_762_000_000_000,
                    model_id: "gemma-4-e4b".to_string(),
                    runtime_model: "gemma4:e4b".to_string(),
                    runtime_backend: "local_runtime".to_string(),
                    load_s: Some(0.25),
                    mean_total_s: 1.5,
                    mean_ttft_s: Some(0.2),
                    mean_decode_tok_s: Some(12.5),
                    output_tokens: 24,
                    repeats: 2,
                    runs: vec![GemmaDiagnosticRunReceipt {
                        run_index: 0,
                        output_tokens: 24,
                        total_s: 1.5,
                        ttft_s: Some(0.2),
                        decode_tok_s: Some(12.5),
                        load_s: Some(0.25),
                        output_text: "mesh".to_string(),
                    }],
                }),
            }],
        };
        save_gemma_diagnostic_report(config_path.as_path(), &mut diagnostic_report)?;
        let snapshot = build_snapshot_from_availability(
            &config,
            Some(&identity),
            ProviderDesiredMode::Online,
            None,
            ProviderAvailability {
                local_gemma: ready_health("gemma4:e4b", &["gemma4:e4b"], Some("gemma_ready")),
                apple_foundation_models: ProviderBackendHealth::default(),
                apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
                adapter_training_contributor:
                    ProviderAdapterTrainingContributorAvailability::default(),
                pooled_inference: ProviderPooledInferenceAvailability::default(),
                sandbox: ProviderSandboxAvailability::default(),
            },
            None,
        );

        report_provider_presence_heartbeat_for_snapshot(
            config_path.as_path(),
            "session-test",
            &snapshot,
        )
        .await?;
        report_provider_presence_offline_for_config(config_path.as_path(), "session-test").await?;

        let requests = recorded_requests
            .lock()
            .expect("provider presence request log")
            .clone();
        ensure(
            requests.len() == 2,
            "provider presence should send one heartbeat and one offline report",
        )?;
        ensure(
            requests[0].0 == "POST /api/provider-presence/heartbeat",
            "first provider presence request should hit the heartbeat endpoint",
        )?;
        ensure(
            requests[0].1["nostr_pubkey_hex"] == json!(identity.public_key_hex),
            "heartbeat should include the Pylon public key",
        )?;
        ensure(
            requests[0].1["session_id"] == "session-test",
            "heartbeat should include the provider presence session id",
        )?;
        ensure(
            requests[0].1["eligible_product_count"] == 1,
            "heartbeat should include the eligible product count",
        )?;
        ensure(
            requests[0].1["products"]
                .as_array()
                .is_some_and(|products| products.len() == 1),
            "heartbeat should include the eligible launch product ids",
        )?;
        ensure(
            requests[0].1["ready_model"] == "gemma4:e4b",
            "heartbeat should include the ready model",
        )?;
        ensure(
            requests[0].1["diagnostic_summaries"]
                .as_array()
                .is_some_and(|rows| rows.len() == 1),
            "heartbeat should include the latest retained diagnostic summaries",
        )?;
        ensure(
            requests[0].1["diagnostic_summaries"][0]["model_id"] == "gemma-4-e4b"
                && requests[0].1["diagnostic_summaries"][0]["mean_total_s"] == json!(1.5)
                && requests[0].1["diagnostic_summaries"][0]["mean_decode_tok_s"] == json!(12.5),
            "heartbeat should serialize diagnostic throughput metrics through the Nexus payload",
        )?;
        ensure(
            requests[1].0 == "POST /api/provider-presence/offline",
            "second provider presence request should hit the offline endpoint",
        )?;
        ensure(
            requests[1].1["nostr_pubkey_hex"] == json!(identity.public_key_hex)
                && requests[1].1["session_id"] == "session-test",
            "offline report should include the same identity and session",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn status_reports_unconfigured_before_init() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");

        let status = load_status_or_detect(config_path.as_path()).await?;

        ensure(
            status.desired_mode == ProviderDesiredMode::Offline,
            "unconfigured status should default desired mode to offline",
        )?;
        ensure(
            provider_runtime_state_label(&status) == "unconfigured",
            "status should report an unconfigured runtime before init",
        )?;
        let human = render_human_status(&status);
        ensure(
            human.contains("state: unconfigured"),
            "human-readable status should include the unconfigured state",
        )?;
        ensure(
            status.snapshot.as_ref().is_some_and(|snapshot| {
                snapshot
                    .runtime
                    .provider_blocker_codes
                    .contains(&"CONFIG_MISSING".to_string())
            }),
            "unconfigured status should include a machine-readable CONFIG_MISSING blocker",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn status_json_uses_local_gemma_schema_and_omits_inert_apple_branch()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let config = load_or_create_config(config_path.as_path())?;
        ensure_identity(config.identity_path.as_path())?;

        let status = load_status_or_detect(config_path.as_path()).await?;
        let json = serde_json::to_value(&status)?;
        let availability = json
            .get("snapshot")
            .and_then(|value| value.get("availability"))
            .ok_or_else(|| std::io::Error::other("missing availability"))?;

        ensure(
            availability.get("local_gemma").is_some(),
            "status JSON should expose local_gemma availability",
        )?;
        ensure(
            availability.get("gpt_oss").is_none(),
            "status JSON should not expose the legacy gpt_oss availability key",
        )?;
        ensure(
            availability.get("apple_foundation_models").is_none(),
            "status JSON should omit inert Apple FM availability",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn local_control_transitions_cover_success_retry_and_failure_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.local_gemma_base_url = "http://127.0.0.1:9".to_string();
        save_config(config_path.as_path(), &config)?;
        ensure_identity(config.identity_path.as_path())?;

        let pause_error = match apply_control_command(
            config_path.as_path(),
            ProviderControlAction::Pause,
        )
        .await
        {
            Ok(_) => {
                return Err(std::io::Error::other(
                    "pause should fail while the provider is offline",
                )
                .into());
            }
            Err(error) => error,
        };
        ensure(
            pause_error.to_string().contains("provider_not_online"),
            "pause failure should expose a machine-readable transition code",
        )?;

        let online_status =
            apply_control_command(config_path.as_path(), ProviderControlAction::Online).await?;
        ensure(
            online_status.desired_mode == ProviderDesiredMode::Online,
            "online should persist the online desired mode",
        )?;
        ensure(
            provider_runtime_state_label(&online_status) == "degraded",
            "without a ready backend, online should still report degraded rather than healthy",
        )?;

        let retried_online_status =
            apply_control_command(config_path.as_path(), ProviderControlAction::Online).await?;
        ensure(
            retried_online_status.desired_mode == ProviderDesiredMode::Online,
            "repeated online should be an idempotent retry",
        )?;

        let paused_status =
            apply_control_command(config_path.as_path(), ProviderControlAction::Pause).await?;
        ensure(
            paused_status.desired_mode == ProviderDesiredMode::Paused,
            "pause should persist the paused desired mode",
        )?;
        ensure(
            provider_runtime_state_label(&paused_status) == "paused",
            "pause should surface the paused runtime state",
        )?;

        let resumed_status =
            apply_control_command(config_path.as_path(), ProviderControlAction::Resume).await?;
        ensure(
            resumed_status.desired_mode == ProviderDesiredMode::Online,
            "resume should restore the online desired mode",
        )?;

        let offline_status =
            apply_control_command(config_path.as_path(), ProviderControlAction::Offline).await?;
        ensure(
            offline_status.desired_mode == ProviderDesiredMode::Offline,
            "offline should persist the offline desired mode",
        )?;
        ensure(
            provider_runtime_state_label(&offline_status) == "offline",
            "offline should surface the offline runtime state when no supply is ready",
        )
    }

    #[test]
    fn parse_args_supports_observability_commands() -> Result<(), Box<dyn std::error::Error>> {
        ensure(
            parse_args(vec!["backends".to_string(), "--json".to_string()])?.command
                == Command::Backends { json: true },
            "backends should parse with --json",
        )?;
        ensure(
            parse_args(vec![
                "inventory".to_string(),
                "--limit".to_string(),
                "5".to_string(),
            ])?
            .command
                == Command::Inventory {
                    json: false,
                    limit: Some(5),
                },
            "inventory should parse with --limit",
        )?;
        ensure(
            parse_args(vec![
                "jobs".to_string(),
                "--json".to_string(),
                "--limit".to_string(),
                "2".to_string(),
            ])?
            .command
                == Command::Jobs {
                    json: true,
                    limit: Some(2),
                },
            "jobs should parse with json and limit flags",
        )?;
        ensure(
            parse_args(vec![
                "receipts".to_string(),
                "--limit".to_string(),
                "3".to_string(),
            ])?
            .command
                == Command::Receipts {
                    json: false,
                    limit: Some(3),
                },
            "receipts should parse with a list limit",
        )?;
        ensure(
            parse_args(vec![
                "activity".to_string(),
                "--json".to_string(),
                "--limit".to_string(),
                "4".to_string(),
            ])?
            .command
                == Command::Activity {
                    json: true,
                    limit: Some(4),
                },
            "activity should parse with json and limit flags",
        )?;
        ensure(
            parse_args(vec![
                "sandbox".to_string(),
                "--json".to_string(),
                "--limit".to_string(),
                "2".to_string(),
            ])?
            .command
                == Command::Sandbox {
                    json: true,
                    limit: Some(2),
                },
            "sandbox should parse with json and limit flags",
        )
    }

    #[test]
    fn parse_args_supports_relay_commands() -> Result<(), Box<dyn std::error::Error>> {
        ensure(
            parse_args(vec!["relays".to_string(), "--json".to_string()])?.command
                == Command::Relays { json: true },
            "relays should parse with --json",
        )?;
        ensure(
            parse_args(vec![
                "relay".to_string(),
                "add".to_string(),
                "wss://relay.damus.io".to_string(),
            ])?
            .command
                == Command::RelayAdd {
                    url: "wss://relay.damus.io".to_string(),
                },
            "relay add should parse the relay URL",
        )?;
        ensure(
            parse_args(vec![
                "relay".to_string(),
                "remove".to_string(),
                "wss://relay.damus.io".to_string(),
            ])?
            .command
                == Command::RelayRemove {
                    url: "wss://relay.damus.io".to_string(),
                },
            "relay remove should parse the relay URL",
        )?;
        ensure(
            parse_args(vec![
                "relay".to_string(),
                "refresh".to_string(),
                "--json".to_string(),
            ])?
            .command
                == Command::RelayRefresh { json: true },
            "relay refresh should parse with --json",
        )
    }

    #[test]
    fn parse_args_supports_announce_commands() -> Result<(), Box<dyn std::error::Error>> {
        ensure(
            parse_args(vec!["announce".to_string()])?.command
                == Command::Announcement {
                    action: AnnouncementAction::Show,
                    json: false,
                },
            "announce should default to show",
        )?;
        ensure(
            parse_args(vec![
                "announce".to_string(),
                "publish".to_string(),
                "--json".to_string(),
            ])?
            .command
                == Command::Announcement {
                    action: AnnouncementAction::Publish,
                    json: true,
                },
            "announce publish should parse with --json",
        )?;
        ensure(
            parse_args(vec!["announce".to_string(), "refresh".to_string()])?.command
                == Command::Announcement {
                    action: AnnouncementAction::Refresh,
                    json: false,
                },
            "announce refresh should parse",
        )
    }

    #[test]
    fn parse_args_supports_provider_scan() -> Result<(), Box<dyn std::error::Error>> {
        ensure(
            parse_args(vec![
                "provider".to_string(),
                "scan".to_string(),
                "--seconds".to_string(),
                "9".to_string(),
                "--json".to_string(),
            ])?
            .command
                == Command::ProviderScan {
                    seconds: 9,
                    json: true,
                },
            "provider scan should parse seconds and json flags",
        )
    }

    #[test]
    fn parse_args_supports_provider_run() -> Result<(), Box<dyn std::error::Error>> {
        ensure(
            parse_args(vec![
                "provider".to_string(),
                "run".to_string(),
                "--seconds".to_string(),
                "7".to_string(),
            ])?
            .command
                == Command::ProviderRun {
                    seconds: 7,
                    json: false,
                },
            "provider run should parse seconds",
        )
    }

    #[test]
    fn parse_args_supports_job_submit() -> Result<(), Box<dyn std::error::Error>> {
        ensure(
            parse_args(vec![
                "job".to_string(),
                "submit".to_string(),
                "--bid-msats".to_string(),
                "21000".to_string(),
                "--model".to_string(),
                "gemma4:e4b".to_string(),
                "--provider".to_string(),
                "provider-001".to_string(),
                "hello".to_string(),
                "buyer".to_string(),
                "--json".to_string(),
            ])?
            .command
                == Command::JobSubmit {
                    request: BuyerJobSubmitRequest {
                        prompt: Some("hello buyer".to_string()),
                        request_json: None,
                        bid_msats: Some(21_000),
                        model: Some("gemma4:e4b".to_string()),
                        provider_pubkey: Some("provider-001".to_string()),
                        output_mime: None,
                    },
                    json: true,
                },
            "job submit should parse buyer submission flags and prompt",
        )?;
        ensure(
            parse_args(vec![
                "job".to_string(),
                "submit".to_string(),
                "--request-json".to_string(),
                "{\"prompt\":\"json\"}".to_string(),
            ])?
            .command
                == Command::JobSubmit {
                    request: BuyerJobSubmitRequest {
                        prompt: None,
                        request_json: Some("{\"prompt\":\"json\"}".to_string()),
                        bid_msats: None,
                        model: None,
                        provider_pubkey: None,
                        output_mime: None,
                    },
                    json: false,
                },
            "job submit should parse structured payload mode",
        )
    }

    #[test]
    fn parse_args_supports_job_watch() -> Result<(), Box<dyn std::error::Error>> {
        ensure(
            parse_args(vec![
                "job".to_string(),
                "watch".to_string(),
                "job-001".to_string(),
                "--seconds".to_string(),
                "12".to_string(),
                "--json".to_string(),
            ])?
            .command
                == Command::JobWatch {
                    request_event_id: Some("job-001".to_string()),
                    seconds: 12,
                    json: true,
                },
            "job watch should parse request id, seconds, and json flags",
        )
    }

    #[test]
    fn parse_args_supports_job_payment_controls() -> Result<(), Box<dyn std::error::Error>> {
        ensure(
            parse_args(vec![
                "job".to_string(),
                "history".to_string(),
                "--limit".to_string(),
                "6".to_string(),
                "--json".to_string(),
            ])?
            .command
                == Command::JobHistory {
                    limit: Some(6),
                    json: true,
                },
            "job history should parse limit and json flags",
        )?;
        ensure(
            parse_args(vec![
                "job".to_string(),
                "replay".to_string(),
                "job-replay-001".to_string(),
            ])?
            .command
                == Command::JobReplay {
                    request_event_id: "job-replay-001".to_string(),
                    json: false,
                },
            "job replay should parse a retained request id",
        )?;
        ensure(
            parse_args(vec![
                "job".to_string(),
                "approve".to_string(),
                "job-approve-001".to_string(),
                "--json".to_string(),
            ])?
            .command
                == Command::JobApprove {
                    request_event_id: "job-approve-001".to_string(),
                    json: true,
                },
            "job approve should parse request id and json flag",
        )?;
        ensure(
            parse_args(vec![
                "job".to_string(),
                "deny".to_string(),
                "job-deny-001".to_string(),
            ])?
            .command
                == Command::JobDeny {
                    request_event_id: "job-deny-001".to_string(),
                    json: false,
                },
            "job deny should parse request id",
        )?;
        ensure(
            parse_args(vec![
                "job".to_string(),
                "policy".to_string(),
                "auto".to_string(),
                "--json".to_string(),
            ])?
            .command
                == Command::JobPolicy {
                    mode: super::BuyerPaymentPolicyMode::Auto,
                    json: true,
                },
            "job policy should parse mode and json flag",
        )
    }

    #[test]
    fn parse_args_supports_payout_commands() -> Result<(), Box<dyn std::error::Error>> {
        ensure(
            parse_args(vec![
                "payout".to_string(),
                "--limit".to_string(),
                "7".to_string(),
                "--json".to_string(),
            ])?
            .command
                == Command::Payout {
                    limit: Some(7),
                    json: true,
                },
            "payout should parse limit and json flags",
        )?;
        ensure(
            parse_args(vec![
                "payout".to_string(),
                "withdraw".to_string(),
                "lnbc21payout".to_string(),
                "--amount-sats".to_string(),
                "21".to_string(),
                "--json".to_string(),
            ])?
            .command
                == Command::PayoutWithdraw {
                    payment_request: "lnbc21payout".to_string(),
                    amount_sats: Some(21),
                    json: true,
                },
            "payout withdraw should parse invoice, amount, and json flag",
        )
    }

    #[test]
    fn parse_args_supports_wallet_commands() -> Result<(), Box<dyn std::error::Error>> {
        ensure(
            parse_args(vec![
                "wallet".to_string(),
                "status".to_string(),
                "--json".to_string(),
            ])?
            .command
                == Command::Wallet {
                    command: WalletSubcommand::Status { json: true },
                },
            "wallet status should parse with --json",
        )?;
        ensure(
            parse_args(vec![
                "wallet".to_string(),
                "invoice".to_string(),
                "21".to_string(),
                "--description".to_string(),
                "earn".to_string(),
            ])?
            .command
                == Command::Wallet {
                    command: WalletSubcommand::Invoice {
                        amount_sats: 21,
                        description: Some("earn".to_string()),
                        expiry_seconds: None,
                        json: false,
                    },
                },
            "wallet invoice should parse with description",
        )?;
        ensure(
            parse_args(vec![
                "wallet".to_string(),
                "history".to_string(),
                "--limit".to_string(),
                "5".to_string(),
            ])?
            .command
                == Command::Wallet {
                    command: WalletSubcommand::History {
                        limit: Some(5),
                        json: false,
                    },
                },
            "wallet history should parse with a limit",
        )
    }

    #[test]
    fn parse_args_supports_gemma_commands() -> Result<(), Box<dyn std::error::Error>> {
        ensure(
            parse_args(vec!["gemma".to_string()])?.command
                == Command::Gemma {
                    command: GemmaCommand::List { json: false },
                },
            "gemma should default to the catalog view",
        )?;
        ensure(
            parse_args(vec![
                "gemma".to_string(),
                "download".to_string(),
                "remaining".to_string(),
                "--json".to_string(),
            ])?
            .command
                == Command::Gemma {
                    command: GemmaCommand::Download {
                        selector: GemmaSelector::Remaining,
                        transport: GemmaDownloadTransport::Auto,
                        json: true,
                    },
                },
            "gemma download should parse the remaining selector",
        )?;
        ensure(
            parse_args(vec![
                "gemma".to_string(),
                "download".to_string(),
                "gemma-4-e4b".to_string(),
                "--transport".to_string(),
                "curl".to_string(),
            ])?
            .command
                == Command::Gemma {
                    command: GemmaCommand::Download {
                        selector: GemmaSelector::Model("gemma-4-e4b".to_string()),
                        transport: GemmaDownloadTransport::Curl,
                        json: false,
                    },
                },
            "gemma download should parse explicit transport overrides",
        )?;
        ensure(
            parse_args(vec![
                "gemma".to_string(),
                "benchmark".to_string(),
                "all".to_string(),
                "--mode".to_string(),
                "matrix".to_string(),
                "--peer-base-url".to_string(),
                "http://127.0.0.1:18080".to_string(),
                "--download-missing".to_string(),
                "--repeats".to_string(),
                "2".to_string(),
                "--json".to_string(),
            ])?
            .command
                == Command::Gemma {
                    command: GemmaCommand::Benchmark {
                        selector: GemmaBenchmarkSelector::All,
                        request: GemmaBenchmarkRequest {
                            mode: GemmaBenchmarkMode::Matrix,
                            backend: None,
                            peer_base_url: Some("http://127.0.0.1:18080".to_string()),
                            split_layer: None,
                            prompt: DEFAULT_GEMMA_BENCH_PROMPT.to_string(),
                            max_output_tokens: 96,
                            repeats: 2,
                            download_missing: true,
                        },
                        json: true,
                    },
                },
            "gemma benchmark should parse matrix-mode orchestration flags",
        )?;
        ensure(
            parse_args(vec![
                "gemma".to_string(),
                "diagnose".to_string(),
                "gemma-4-e4b".to_string(),
                "--max-output-tokens".to_string(),
                "24".to_string(),
                "--repeats".to_string(),
                "2".to_string(),
                "--download-missing".to_string(),
                "--json".to_string(),
            ])?
            .command
                == Command::Gemma {
                    command: GemmaCommand::Diagnose {
                        selector: GemmaBenchmarkSelector::Model("gemma-4-e4b".to_string()),
                        request: GemmaDiagnosticRequest {
                            diagnostic_id: DEFAULT_GEMMA_DIAGNOSTIC_ID.to_string(),
                            prompt: DEFAULT_GEMMA_BENCH_PROMPT.to_string(),
                            max_output_tokens: 24,
                            repeats: 2,
                            download_missing: true,
                        },
                        json: true,
                    },
                },
            "gemma diagnose should parse first-run diagnostic flags",
        )
    }

    #[test]
    fn gemma_benchmark_matrix_marks_dense_split_without_peer_as_skipped()
    -> Result<(), Box<dyn std::error::Error>> {
        let spec = gemma_download_spec("gemma-4-e4b")
            .ok_or_else(|| std::io::Error::other("missing gemma-4-e4b spec"))?;
        let plans = planned_gemma_benchmark_modes(
            spec,
            &GemmaBenchmarkRequest {
                mode: GemmaBenchmarkMode::Matrix,
                backend: None,
                peer_base_url: None,
                split_layer: None,
                prompt: DEFAULT_GEMMA_BENCH_PROMPT.to_string(),
                max_output_tokens: 96,
                repeats: 1,
                download_missing: false,
            },
        )?;
        ensure(
            plans
                == vec![
                    (GemmaBenchExecutionMode::Single, None),
                    (
                        GemmaBenchExecutionMode::DistributedDense,
                        Some(String::from("distributed-dense requires --peer-base-url")),
                    ),
                ],
            "matrix planning should keep the dense split row explicit when the peer is missing",
        )
    }

    #[test]
    fn gemma_benchmark_refuses_single_node_sparse_request() -> Result<(), Box<dyn std::error::Error>>
    {
        let spec = gemma_download_spec("gemma-4-26b-a4b")
            .ok_or_else(|| std::io::Error::other("missing gemma-4-26b-a4b spec"))?;
        let error = planned_gemma_benchmark_modes(
            spec,
            &GemmaBenchmarkRequest {
                mode: GemmaBenchmarkMode::Single,
                backend: None,
                peer_base_url: None,
                split_layer: None,
                prompt: DEFAULT_GEMMA_BENCH_PROMPT.to_string(),
                max_output_tokens: 96,
                repeats: 1,
                download_missing: false,
            },
        )
        .expect_err("sparse 26b should refuse single-node mode");
        ensure(
            error
                .to_string()
                .contains("does not support single-node execution"),
            "sparse 26b should fail closed on single-node requests",
        )
    }

    #[test]
    fn gemma_benchmark_command_keeps_cargo_run_prefix() -> Result<(), Box<dyn std::error::Error>> {
        let args = psionic_gemma_benchmark_command_args(
            std::path::Path::new("/tmp/psionic"),
            std::path::Path::new("/tmp/model.gguf"),
            GemmaBenchExecutionMode::Single,
            &GemmaBenchmarkRequest {
                mode: GemmaBenchmarkMode::Single,
                backend: Some(String::from("metal")),
                peer_base_url: None,
                split_layer: None,
                prompt: DEFAULT_GEMMA_BENCH_PROMPT.to_string(),
                max_output_tokens: 96,
                repeats: 1,
                download_missing: false,
            },
            std::path::Path::new("/tmp/receipt.json"),
        );

        ensure(
            args.first().is_some_and(|value| value == "run"),
            "benchmark command should keep the cargo run prefix",
        )?;
        ensure(
            args.get(1).is_some_and(|value| value == "--quiet"),
            "benchmark command should keep the expected cargo flags after run",
        )?;
        ensure(
            args.iter().any(|value| value == "--example"),
            "benchmark command should still target the gemma4_bench example",
        )?;
        ensure(
            args.iter().any(|value| value == "--json-out"),
            "benchmark command should still request a JSON receipt",
        )
    }

    #[test]
    fn relay_config_commands_mutate_the_config_and_report() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");

        let added = add_configured_relay(config_path.as_path(), "wss://relay.example.com/")?;
        ensure(
            added
                .relays
                .iter()
                .any(|relay| relay.url == "wss://relay.example.com"),
            "relay add should normalize and report the configured relay",
        )?;

        let removed = remove_configured_relay(config_path.as_path(), "wss://relay.example.com")?;
        ensure(
            !removed
                .relays
                .iter()
                .any(|relay| relay.url == "wss://relay.example.com"),
            "relay remove should remove the relay from the report",
        )?;

        let report = load_relay_report(config_path.as_path())?;
        ensure(
            report
                .relays
                .iter()
                .all(|relay| relay.url != "wss://relay.example.com"),
            "relay report should reflect the removed relay",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn relay_refresh_records_invalid_url_errors() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.relay_urls = vec!["not-a-url".to_string()];
        save_config(config_path.as_path(), &config)?;

        let report = refresh_relay_report(config_path.as_path()).await?;
        let relay = report
            .relays
            .first()
            .ok_or_else(|| std::io::Error::other("relay report missing invalid relay entry"))?;
        ensure(
            relay.state == "error",
            "invalid relay refresh should record an error state",
        )?;
        ensure(
            relay
                .last_error
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
            "invalid relay refresh should surface the validation failure",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn relay_refresh_records_auth_challenges() -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let relay_url = format!("ws://{addr}");
        let challenge = "pylon-auth-challenge".to_string();

        let server = tokio::spawn({
            let challenge = challenge.clone();
            async move {
                let (stream, _) = listener.accept().await.expect("accept websocket client");
                let mut ws = accept_async(stream)
                    .await
                    .expect("upgrade websocket connection");
                let auth_frame = serde_json::json!(["AUTH", challenge]);
                ws.send(Message::Text(auth_frame.to_string().into()))
                    .await
                    .expect("send auth challenge");
                while let Some(message) = ws.next().await {
                    let Ok(Message::Text(_)) = message else {
                        continue;
                    };
                    break;
                }
            }
        });

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.relay_urls = vec![relay_url];
        config.relay_auth_enabled = true;
        save_config(config_path.as_path(), &config)?;

        let report = refresh_relay_report(config_path.as_path()).await?;
        let relay = report
            .relays
            .first()
            .ok_or_else(|| std::io::Error::other("relay report missing auth relay entry"))?;
        ensure(
            relay.auth_state == "challenged",
            "relay refresh should record an auth challenge when the relay sends AUTH",
        )?;
        ensure(
            relay.detail.as_deref()
                == Some("relay issued AUTH and Pylon responded with the local node identity"),
            "relay refresh should explain the auth challenge outcome",
        )?;
        let ledger = load_ledger(config_path.as_path())?;
        ensure(
            ledger
                .relay_activity
                .iter()
                .any(|entry| entry.kind == "relay.auth_challenge"),
            "relay activity should retain an auth challenge record",
        )?;

        server.abort();
        Ok(())
    }

    #[test]
    fn load_relay_activity_report_reads_retained_activity() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        load_or_create_config(config_path.as_path())?;
        mutate_ledger(config_path.as_path(), |ledger| {
            ledger.push_relay_activity(super::PylonRelayActivity {
                at_ms: 10,
                url: Some("wss://relay.example.com".to_string()),
                kind: "nip90.job_submitted".to_string(),
                detail: "submitted request relay-activity-001".to_string(),
            });
            ledger.push_relay_activity(super::PylonRelayActivity {
                at_ms: 20,
                url: None,
                kind: "payout.withdrawal_submitted".to_string(),
                detail: "provider withdrawal payment-001 submitted".to_string(),
            });
            Ok(())
        })?;

        let report = super::load_relay_activity_report(config_path.as_path(), Some(1))?;
        ensure(
            report.entries.len() == 1
                && report.entries[0].kind == "payout.withdrawal_submitted"
                && report.entries[0].detail == "provider withdrawal payment-001 submitted",
            "relay activity report should project retained activity ordered by most recent update",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn publish_announcement_persists_handler_event() -> Result<(), Box<dyn std::error::Error>>
    {
        let ollama_listener = TcpListener::bind("127.0.0.1:0").await?;
        let ollama_addr = ollama_listener.local_addr()?;
        let ollama_server = tokio::spawn(async move {
            let (mut stream, _) = ollama_listener.accept().await.expect("accept ollama");
            let mut request = vec![0u8; 4096];
            let _ = stream
                .read(&mut request)
                .await
                .expect("read ollama request");
            let body = json!({
                "models": [{"name": "gemma4-e4b-local:latest"}]
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write ollama response");
        });

        let relay_listener = TcpListener::bind("127.0.0.1:0").await?;
        let relay_addr = relay_listener.local_addr()?;
        let relay_url = format!("ws://{relay_addr}");
        let relay_server = tokio::spawn(async move {
            let (stream, _) = relay_listener.accept().await.expect("accept relay client");
            let mut ws = accept_async(stream).await.expect("upgrade relay websocket");
            while let Some(message) = ws.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                let value: Value = serde_json::from_str(payload.as_str()).expect("parse event");
                if value[0] == "EVENT" {
                    return value;
                }
            }
            panic!("relay did not receive an EVENT frame");
        });

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.relay_urls = vec![relay_url];
        config.local_gemma_base_url = format!("http://{ollama_addr}");
        save_config(config_path.as_path(), &config)?;

        let report = publish_announcement_report(config_path.as_path(), false).await?;
        ensure(
            report.handler_event_id.is_some(),
            "publish should return a handler event id",
        )?;
        ensure(
            report.model.as_deref() == Some("gemma4-e4b-local:latest"),
            "publish should advertise the ready Gemma model",
        )?;

        let payload = relay_server.await?;
        ensure(
            payload[0] == "EVENT",
            "relay payload should be an EVENT frame",
        )?;
        ensure(
            payload[1]["kind"] == 31990,
            "announcement should publish a kind:31990 event",
        )?;

        let ledger = load_ledger(config_path.as_path())?;
        ensure(
            ledger.announcements.len() == 1,
            "publish should persist the handler announcement",
        )?;
        ensure(
            ledger
                .relay_activity
                .iter()
                .any(|entry| entry.kind == "announcement.published"),
            "publish should append a relay activity record",
        )?;

        ollama_server.await?;
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_buyer_job_publishes_request_and_persists_ledger()
    -> Result<(), Box<dyn std::error::Error>> {
        let relay_listener = TcpListener::bind("127.0.0.1:0").await?;
        let relay_addr = relay_listener.local_addr()?;
        let relay_url = format!("ws://{relay_addr}");
        let relay_server = tokio::spawn(async move {
            let (stream, _) = relay_listener.accept().await.expect("accept relay client");
            let mut ws = accept_async(stream).await.expect("upgrade relay websocket");
            while let Some(message) = ws.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                let value: Value =
                    serde_json::from_str(payload.as_str()).expect("parse buyer event frame");
                if value[0] == "EVENT" {
                    return value[1].clone();
                }
            }
            panic!("relay did not receive a buyer EVENT frame");
        });

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.relay_urls = vec![relay_url.clone()];
        save_config(config_path.as_path(), &config)?;

        let report = submit_buyer_job(
            config_path.as_path(),
            BuyerJobSubmitRequest {
                prompt: Some("hello buyer".to_string()),
                request_json: None,
                bid_msats: Some(21_000),
                model: Some("gemma4:e4b".to_string()),
                provider_pubkey: Some("provider-001".to_string()),
                output_mime: None,
            },
        )
        .await?;
        ensure(
            report.status == "submitted" && report.request_event_id == report.job_id,
            "buyer submit should return a submitted request report",
        )?;

        let event = relay_server.await?;
        let tags = event["tags"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("buyer request tags missing"))?;
        let has_tag = |expected: &[&str]| {
            tags.iter().any(|tag| {
                tag.as_array().is_some_and(|values| {
                    values
                        .iter()
                        .map(|value| value.as_str().unwrap_or_default())
                        .collect::<Vec<_>>()
                        == expected
                })
            })
        };
        ensure(
            event["kind"] == 5050,
            "buyer submit should publish a kind:5050 event",
        )?;
        ensure(
            has_tag(&["i", "hello buyer", "text"]),
            "buyer submit should publish the text input",
        )?;
        ensure(
            has_tag(&["param", "model", "gemma4:e4b"]),
            "buyer submit should publish the model param",
        )?;
        ensure(
            has_tag(&["bid", "21000"]),
            "buyer submit should publish the bid tag",
        )?;
        ensure(
            has_tag(&["p", "provider-001"]),
            "buyer submit should publish the targeted provider preference",
        )?;

        let ledger = load_ledger(config_path.as_path())?;
        let job = ledger
            .jobs
            .iter()
            .find(|job| job.id == report.job_id)
            .ok_or_else(|| std::io::Error::other("missing buyer job ledger entry"))?;
        ensure(
            job.direction == "buyer"
                && job.status == "submitted"
                && job.prompt.as_deref() == Some("hello buyer")
                && job.request_event_id.as_deref() == Some(report.request_event_id.as_str())
                && job.provider_pubkey.as_deref() == Some("provider-001")
                && job.bid_msats == Some(21_000),
            "buyer submit should persist the outbound job locally",
        )?;
        ensure(
            ledger
                .relay_activity
                .iter()
                .any(|entry| entry.kind == "nip90.job_submitted"),
            "buyer submit should persist relay activity",
        )?;
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_buyer_job_accepts_structured_payload_json()
    -> Result<(), Box<dyn std::error::Error>> {
        let relay_listener = TcpListener::bind("127.0.0.1:0").await?;
        let relay_addr = relay_listener.local_addr()?;
        let relay_url = format!("ws://{relay_addr}");
        let relay_server = tokio::spawn(async move {
            let (stream, _) = relay_listener.accept().await.expect("accept relay client");
            let mut ws = accept_async(stream).await.expect("upgrade relay websocket");
            while let Some(message) = ws.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                let value: Value =
                    serde_json::from_str(payload.as_str()).expect("parse buyer event frame");
                if value[0] == "EVENT" {
                    return value[1].clone();
                }
            }
            panic!("relay did not receive a structured buyer EVENT frame");
        });

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.relay_urls = vec![relay_url];
        save_config(config_path.as_path(), &config)?;

        let request_json = json!({
            "inputs": [{"type": "text", "data": "json prompt"}],
            "params": {"temperature": "0.2", "max_tokens": "64"},
            "model": "gemma4:json",
            "provider": "provider-json",
            "bid_msats": 12000,
            "output": "text/plain"
        })
        .to_string();
        let report = submit_buyer_job(
            config_path.as_path(),
            BuyerJobSubmitRequest {
                prompt: None,
                request_json: Some(request_json.clone()),
                bid_msats: None,
                model: None,
                provider_pubkey: None,
                output_mime: None,
            },
        )
        .await?;
        ensure(
            report.model.as_deref() == Some("gemma4:json")
                && report.provider_pubkey.as_deref() == Some("provider-json")
                && report.output_mime.as_deref() == Some("text/plain"),
            "structured buyer submit should surface JSON-derived fields",
        )?;

        let event = relay_server.await?;
        let tags = event["tags"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("structured buyer request tags missing"))?;
        let has_tag = |expected: &[&str]| {
            tags.iter().any(|tag| {
                tag.as_array().is_some_and(|values| {
                    values
                        .iter()
                        .map(|value| value.as_str().unwrap_or_default())
                        .collect::<Vec<_>>()
                        == expected
                })
            })
        };
        ensure(
            has_tag(&["i", "json prompt", "text"]),
            "structured buyer submit should publish the input payload",
        )?;
        ensure(
            has_tag(&["param", "temperature", "0.2"])
                && has_tag(&["param", "max_tokens", "64"])
                && has_tag(&["param", "model", "gemma4:json"]),
            "structured buyer submit should publish JSON params",
        )?;
        ensure(
            has_tag(&["output", "text/plain"])
                && has_tag(&["bid", "12000"])
                && has_tag(&["p", "provider-json"]),
            "structured buyer submit should publish output, bid, and provider tags",
        )?;

        let ledger = load_ledger(config_path.as_path())?;
        let job = ledger
            .jobs
            .iter()
            .find(|job| job.id == report.job_id)
            .ok_or_else(|| std::io::Error::other("missing structured buyer job"))?;
        ensure(
            job.prompt.as_deref() == Some(request_json.as_str())
                && job.result_preview.as_deref() == Some("json prompt")
                && job.model.as_deref() == Some("gemma4:json"),
            "structured buyer submit should persist the raw JSON payload locally",
        )?;
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn watch_buyer_jobs_persists_feedback_and_result_updates()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        let identity = ensure_identity(config.identity_path.as_path())?;

        let relay_listener = TcpListener::bind("127.0.0.1:0").await?;
        let relay_addr = relay_listener.local_addr()?;
        let relay_url = format!("ws://{relay_addr}");
        config.relay_urls = vec![relay_url];
        save_config(config_path.as_path(), &config)?;

        mutate_ledger(config_path.as_path(), |ledger| {
            let mut job = super::PylonLedgerJob::new("buyer-watch-001", "buyer", 5050, "submitted");
            job.request_event_id = Some("buyer-watch-001".to_string());
            job.customer_pubkey = Some(identity.public_key_hex.clone());
            job.prompt = Some("watch me".to_string());
            ledger.upsert_job(job);
            Ok(())
        })?;

        let customer_pubkey = identity.public_key_hex.clone();
        let relay_server = tokio::spawn(async move {
            let (stream, _) = relay_listener.accept().await.expect("accept relay client");
            let mut ws = accept_async(stream).await.expect("upgrade relay websocket");
            while let Some(message) = ws.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                if !payload.contains("\"REQ\"") {
                    continue;
                }
                let payment_required = json!(["EVENT", "watch", {
                    "id": "buyer-feedback-001",
                    "pubkey": "provider-pubkey-001",
                    "created_at": 1_760_000_500u64,
                    "kind": 7000,
                    "tags": [
                        ["status", "payment-required", "lightning settlement required"],
                        ["e", "buyer-watch-001", "wss://relay.example.com"],
                        ["p", customer_pubkey],
                        ["amount", "21000", "lnbc21000n1buyerwatch"]
                    ],
                    "content": "",
                    "sig": "66".repeat(64)
                }]);
                let result = json!(["EVENT", "watch", {
                    "id": "buyer-result-001",
                    "pubkey": "provider-pubkey-001",
                    "created_at": 1_760_000_501u64,
                    "kind": 6050,
                    "tags": [
                        ["e", "buyer-watch-001", "wss://relay.example.com"],
                        ["p", customer_pubkey]
                    ],
                    "content": "final retained result",
                    "sig": "77".repeat(64)
                }]);
                ws.send(Message::Text(payment_required.to_string().into()))
                    .await
                    .expect("send payment-required feedback");
                ws.send(Message::Text(result.to_string().into()))
                    .await
                    .expect("send buyer result");
                break;
            }
        });

        let report =
            watch_buyer_jobs(config_path.as_path(), Some("buyer-watch-001"), 1, |_| {}).await?;
        ensure(
            report.feedback_count == 1
                && report.result_count == 1
                && report.entries.iter().any(|entry| {
                    entry.event_kind == "feedback"
                        && entry.status == "payment-required"
                        && entry.amount_msats == Some(21_000)
                        && entry.bolt11.as_deref() == Some("lnbc21000n1buyerwatch")
                })
                && report.entries.iter().any(|entry| {
                    entry.event_kind == "result"
                        && entry.status == "result_received"
                        && entry
                            .result_preview
                            .as_deref()
                            .is_some_and(|value| value.contains("final retained result"))
                }),
            "buyer watch should capture payment-required feedback and the retained result",
        )?;

        let ledger = load_ledger(config_path.as_path())?;
        let job = ledger
            .jobs
            .iter()
            .find(|job| job.id == "buyer-watch-001")
            .ok_or_else(|| std::io::Error::other("missing buyer watch ledger job"))?;
        ensure(
            job.status == "result_received"
                && job
                    .feedback_event_ids
                    .iter()
                    .any(|id| id == "buyer-feedback-001")
                && job.result_event_id.as_deref() == Some("buyer-result-001")
                && job.amount_msats == Some(21_000)
                && job.bolt11.as_deref() == Some("lnbc21000n1buyerwatch")
                && job
                    .result_preview
                    .as_deref()
                    .is_some_and(|value| value.contains("final retained result")),
            "buyer watch should persist feedback and result state into the ledger",
        )?;
        ensure(
            ledger
                .relay_activity
                .iter()
                .any(|entry| entry.kind == "nip90.feedback_received")
                && ledger
                    .relay_activity
                    .iter()
                    .any(|entry| entry.kind == "nip90.result_received"),
            "buyer watch should persist relay activity for feedback and results",
        )?;

        relay_server.await?;
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn approve_buyer_job_payment_submits_wallet_payment_and_persists_outcome()
    -> Result<(), Box<dyn std::error::Error>> {
        let _guard = super::nip90_runtime::lock_test_runtime();
        super::nip90_runtime::set_test_wallet_pay_hook(Some(Box::new(|bolt11, amount_sats| {
            if bolt11 != "lnbc21000n1buyerapprove" || amount_sats != Some(21) {
                return Err(std::io::Error::other(
                    "buyer approve should pass the retained invoice and rounded sats",
                )
                .into());
            }
            Ok(super::WalletPayReport {
                runtime: WalletRuntimeSurface::default(),
                payment_id: "payment-send-001".to_string(),
                payment: PylonWalletPaymentRecord {
                    payment_id: "payment-send-001".to_string(),
                    direction: "send".to_string(),
                    status: "completed".to_string(),
                    amount_sats: 21,
                    fees_sats: 1,
                    method: "lightning".to_string(),
                    description: Some("buyer invoice approval".to_string()),
                    invoice: Some(bolt11.to_string()),
                    created_at_ms: 1_762_100_000_000,
                    updated_at_ms: 1_762_100_000_000,
                },
                post_balance: super::WalletBalanceSnapshot::default(),
            })
        })));

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        load_or_create_config(config_path.as_path())?;
        mutate_ledger(config_path.as_path(), |ledger| {
            let mut job =
                super::PylonLedgerJob::new("buyer-approve-001", "buyer", 5050, "payment_required");
            job.request_event_id = Some("buyer-approve-001".to_string());
            job.provider_pubkey = Some("provider-pubkey-approve".to_string());
            job.amount_msats = Some(21_000);
            job.bolt11 = Some("lnbc21000n1buyerapprove".to_string());
            ledger.upsert_job(job);
            Ok(())
        })?;

        let report =
            super::approve_buyer_job_payment(config_path.as_path(), "buyer-approve-001").await?;
        ensure(
            report.provider_pubkey.as_deref() == Some("provider-pubkey-approve")
                && report.status == "payment_submitted"
                && report.amount_msats == Some(21_000)
                && report.bolt11.as_deref() == Some("lnbc21000n1buyerapprove")
                && report.payment_id.as_deref() == Some("payment-send-001"),
            "buyer approve should surface the provider, invoice, and payment outcome",
        )?;

        let ledger = load_ledger(config_path.as_path())?;
        let job = ledger
            .jobs
            .iter()
            .find(|job| job.id == "buyer-approve-001")
            .ok_or_else(|| std::io::Error::other("missing approved buyer job"))?;
        ensure(
            job.status == "payment_submitted"
                && job.payment_id.as_deref() == Some("payment-send-001")
                && job.provider_pubkey.as_deref() == Some("provider-pubkey-approve"),
            "buyer approve should persist payment submission on the retained job",
        )?;
        ensure(
            ledger.settlements.iter().any(|settlement| {
                settlement.job_id == "buyer-approve-001"
                    && settlement.direction == "buyer"
                    && settlement.status == "payment_submitted"
                    && settlement.payment_reference.as_deref() == Some("payment-send-001")
            }) && ledger
                .relay_activity
                .iter()
                .any(|entry| entry.kind == "nip90.payment_submitted"),
            "buyer approve should persist settlement and relay activity",
        )?;

        super::nip90_runtime::set_test_wallet_pay_hook(None);
        Ok(())
    }

    #[test]
    fn deny_buyer_job_payment_persists_denial() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        load_or_create_config(config_path.as_path())?;
        mutate_ledger(config_path.as_path(), |ledger| {
            let mut job =
                super::PylonLedgerJob::new("buyer-deny-001", "buyer", 5050, "payment_required");
            job.request_event_id = Some("buyer-deny-001".to_string());
            job.provider_pubkey = Some("provider-pubkey-deny".to_string());
            job.amount_msats = Some(34_000);
            job.bolt11 = Some("lnbc34000n1buyerdeny".to_string());
            ledger.upsert_job(job);
            Ok(())
        })?;

        let report = super::deny_buyer_job_payment(config_path.as_path(), "buyer-deny-001")?;
        ensure(
            report.provider_pubkey.as_deref() == Some("provider-pubkey-deny")
                && report.status == "payment_denied"
                && report.bolt11.as_deref() == Some("lnbc34000n1buyerdeny"),
            "buyer deny should surface the retained invoice and provider",
        )?;

        let ledger = load_ledger(config_path.as_path())?;
        let job = ledger
            .jobs
            .iter()
            .find(|job| job.id == "buyer-deny-001")
            .ok_or_else(|| std::io::Error::other("missing denied buyer job"))?;
        ensure(
            job.status == "payment_denied"
                && job.error_detail.as_deref() == Some("buyer denied invoice"),
            "buyer deny should persist the denied state",
        )?;
        ensure(
            ledger.settlements.iter().any(|settlement| {
                settlement.job_id == "buyer-deny-001"
                    && settlement.direction == "buyer"
                    && settlement.status == "payment_denied"
            }) && ledger
                .relay_activity
                .iter()
                .any(|entry| entry.kind == "nip90.payment_denied"),
            "buyer deny should persist settlement and relay activity",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn watch_buyer_jobs_auto_pays_when_policy_is_enabled()
    -> Result<(), Box<dyn std::error::Error>> {
        let _guard = super::nip90_runtime::lock_test_runtime();
        super::nip90_runtime::set_test_wallet_pay_hook(Some(Box::new(|bolt11, amount_sats| {
            if bolt11 != "lnbc21000n1buyerauto" || amount_sats != Some(21) {
                return Err(std::io::Error::other(
                    "auto-pay should forward the retained invoice and rounded sats",
                )
                .into());
            }
            Ok(super::WalletPayReport {
                runtime: WalletRuntimeSurface::default(),
                payment_id: "payment-auto-001".to_string(),
                payment: PylonWalletPaymentRecord {
                    payment_id: "payment-auto-001".to_string(),
                    direction: "send".to_string(),
                    status: "completed".to_string(),
                    amount_sats: 21,
                    fees_sats: 1,
                    method: "lightning".to_string(),
                    description: Some("buyer auto pay".to_string()),
                    invoice: Some(bolt11.to_string()),
                    created_at_ms: 1_762_100_100_000,
                    updated_at_ms: 1_762_100_100_000,
                },
                post_balance: super::WalletBalanceSnapshot::default(),
            })
        })));

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        let identity = ensure_identity(config.identity_path.as_path())?;

        let relay_listener = TcpListener::bind("127.0.0.1:0").await?;
        let relay_addr = relay_listener.local_addr()?;
        let relay_url = format!("ws://{relay_addr}");
        config.relay_urls = vec![relay_url];
        config.buyer_auto_pay_enabled = true;
        save_config(config_path.as_path(), &config)?;

        mutate_ledger(config_path.as_path(), |ledger| {
            let mut job = super::PylonLedgerJob::new("buyer-auto-001", "buyer", 5050, "submitted");
            job.request_event_id = Some("buyer-auto-001".to_string());
            job.customer_pubkey = Some(identity.public_key_hex.clone());
            ledger.upsert_job(job);
            Ok(())
        })?;

        let customer_pubkey = identity.public_key_hex.clone();
        let relay_server = tokio::spawn(async move {
            let (stream, _) = relay_listener.accept().await.expect("accept relay client");
            let mut ws = accept_async(stream).await.expect("upgrade relay websocket");
            while let Some(message) = ws.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                if !payload.contains("\"REQ\"") {
                    continue;
                }
                let payment_required = json!(["EVENT", "watch", {
                    "id": "buyer-feedback-auto-001",
                    "pubkey": "provider-pubkey-auto",
                    "created_at": 1_760_000_600u64,
                    "kind": 7000,
                    "tags": [
                        ["status", "payment-required", "lightning settlement required"],
                        ["e", "buyer-auto-001", "wss://relay.example.com"],
                        ["p", customer_pubkey],
                        ["amount", "21000", "lnbc21000n1buyerauto"]
                    ],
                    "content": "",
                    "sig": "88".repeat(64)
                }]);
                ws.send(Message::Text(payment_required.to_string().into()))
                    .await
                    .expect("send auto-pay feedback");
                break;
            }
        });

        let report =
            watch_buyer_jobs(config_path.as_path(), Some("buyer-auto-001"), 1, |_| {}).await?;
        ensure(
            report.entries.iter().any(|entry| {
                entry.event_kind == "feedback"
                    && entry.status == "payment-required"
                    && entry.provider_pubkey.as_deref() == Some("provider-pubkey-auto")
            }) && report.entries.iter().any(|entry| {
                entry.event_kind == "payment"
                    && entry.status == "payment_submitted"
                    && entry.event_id == "payment-auto-001"
                    && entry.provider_pubkey.as_deref() == Some("provider-pubkey-auto")
            }),
            "buyer watch should append an auto-pay payment entry after payment-required feedback",
        )?;

        let ledger = load_ledger(config_path.as_path())?;
        let job = ledger
            .jobs
            .iter()
            .find(|job| job.id == "buyer-auto-001")
            .ok_or_else(|| std::io::Error::other("missing auto-paid buyer job"))?;
        ensure(
            job.status == "payment_submitted"
                && job.provider_pubkey.as_deref() == Some("provider-pubkey-auto")
                && job.payment_id.as_deref() == Some("payment-auto-001"),
            "buyer auto-pay should persist submitted payment state on the retained job",
        )?;
        ensure(
            ledger.settlements.iter().any(|settlement| {
                settlement.job_id == "buyer-auto-001"
                    && settlement.status == "payment_submitted"
                    && settlement.payment_reference.as_deref() == Some("payment-auto-001")
            }),
            "buyer auto-pay should persist a retained settlement record",
        )?;

        relay_server.await?;
        super::nip90_runtime::set_test_wallet_pay_hook(None);
        Ok(())
    }

    #[test]
    fn load_buyer_job_history_reads_retained_buyer_jobs() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        load_or_create_config(config_path.as_path())?;
        mutate_ledger(config_path.as_path(), |ledger| {
            let mut older =
                super::PylonLedgerJob::new("buyer-history-001", "buyer", 5050, "submitted");
            older.request_event_id = Some("buyer-history-001".to_string());
            older.provider_pubkey = Some("provider-history-001".to_string());
            ledger.upsert_job(older);
            if let Some(existing) = ledger
                .jobs
                .iter_mut()
                .find(|job| job.id == "buyer-history-001")
            {
                existing.updated_at_ms = 10;
            }

            let mut newer =
                super::PylonLedgerJob::new("buyer-history-002", "buyer", 5050, "result_received");
            newer.request_event_id = Some("buyer-history-002".to_string());
            newer.provider_pubkey = Some("provider-history-002".to_string());
            newer.result_preview = Some("history result".to_string());
            ledger.upsert_job(newer);
            if let Some(existing) = ledger
                .jobs
                .iter_mut()
                .find(|job| job.id == "buyer-history-002")
            {
                existing.updated_at_ms = 20;
            }

            let provider_job =
                super::PylonLedgerJob::new("provider-history-ignore", "provider", 5050, "settled");
            ledger.upsert_job(provider_job);
            Ok(())
        })?;

        let report = super::load_buyer_job_history(config_path.as_path(), Some(1))?;
        ensure(
            report.total_count == 2
                && report.entries.len() == 1
                && report.entries[0].request_event_id == "buyer-history-002"
                && report.entries[0].provider_pubkey.as_deref() == Some("provider-history-002")
                && report.entries[0].result_preview.as_deref() == Some("history result"),
            "buyer history should return retained buyer jobs ordered by most recent update",
        )
    }

    #[test]
    fn load_buyer_job_replay_projects_settlement_and_activity()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        load_or_create_config(config_path.as_path())?;
        mutate_ledger(config_path.as_path(), |ledger| {
            let mut job =
                super::PylonLedgerJob::new("buyer-replay-001", "buyer", 5050, "result_received");
            job.request_event_id = Some("buyer-replay-001".to_string());
            job.provider_pubkey = Some("provider-replay-001".to_string());
            job.bid_msats = Some(21_000);
            job.amount_msats = Some(21_000);
            job.payment_id = Some("payment-replay-001".to_string());
            job.feedback_event_ids = vec!["feedback-replay-001".to_string()];
            job.result_event_id = Some("result-replay-001".to_string());
            job.result_preview = Some("replayed result".to_string());
            ledger.upsert_job(job);
            ledger.upsert_settlement(super::PylonSettlementRecord {
                settlement_id: "buyer-settlement-replay-001".to_string(),
                job_id: "buyer-replay-001".to_string(),
                direction: "buyer".to_string(),
                status: "payment_submitted".to_string(),
                amount_msats: 21_000,
                payment_reference: Some("payment-replay-001".to_string()),
                receipt_detail: Some("buyer approved and submitted invoice payment".to_string()),
                created_at_ms: 40,
                updated_at_ms: 50,
            });
            ledger.push_relay_activity(super::PylonRelayActivity {
                at_ms: 41,
                url: Some("wss://relay.example.com".to_string()),
                kind: "nip90.job_submitted".to_string(),
                detail: "submitted buyer request buyer-replay-001".to_string(),
            });
            ledger.push_relay_activity(super::PylonRelayActivity {
                at_ms: 42,
                url: Some("wss://relay.example.com".to_string()),
                kind: "nip90.payment_submitted".to_string(),
                detail: "submitted buyer payment payment-replay-001 for request buyer-replay-001"
                    .to_string(),
            });
            Ok(())
        })?;

        let report = super::load_buyer_job_replay(config_path.as_path(), "buyer-replay-001")?;
        ensure(
            report.entry.request_event_id == "buyer-replay-001"
                && report.entry.provider_pubkey.as_deref() == Some("provider-replay-001")
                && report.settlement_status.as_deref() == Some("payment_submitted")
                && report
                    .settlement_detail
                    .as_deref()
                    .is_some_and(|value| value.contains("buyer approved"))
                && report.feedback_event_ids == vec!["feedback-replay-001".to_string()]
                && report.result_event_id.as_deref() == Some("result-replay-001")
                && report.activity.len() == 2
                && report.activity[0].kind == "nip90.job_submitted"
                && report.activity[1].kind == "nip90.payment_submitted",
            "buyer replay should project the retained job, settlement, and matching activity",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_payout_withdrawal_persists_history() -> Result<(), Box<dyn std::error::Error>> {
        let _guard = super::nip90_runtime::lock_test_runtime();
        super::set_test_payout_pay_hook(Some(Box::new(|bolt11, amount_sats| {
            if bolt11 != "lnbc21withdraw" || amount_sats != Some(21) {
                return Err(std::io::Error::other(
                    "payout withdrawal should pass the retained invoice and amount",
                )
                .into());
            }
            Ok(super::WalletPayReport {
                runtime: WalletRuntimeSurface::default(),
                payment_id: "payment-withdraw-001".to_string(),
                payment: PylonWalletPaymentRecord {
                    payment_id: "payment-withdraw-001".to_string(),
                    direction: "send".to_string(),
                    status: "completed".to_string(),
                    amount_sats: 21,
                    fees_sats: 1,
                    method: "lightning".to_string(),
                    description: Some("provider withdrawal".to_string()),
                    invoice: Some(bolt11.to_string()),
                    created_at_ms: 1_762_200_000_000,
                    updated_at_ms: 1_762_200_000_000,
                },
                post_balance: super::WalletBalanceSnapshot {
                    total_sats: 144,
                    spark_sats: 144,
                    lightning_sats: 144,
                    onchain_sats: 0,
                },
            })
        })));

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.payout_destination = Some("lnurlp:provider".to_string());
        save_config(config_path.as_path(), &config)?;
        mutate_ledger(config_path.as_path(), |ledger| {
            ledger.wallet.last_balance_sats = Some(200);
            Ok(())
        })?;

        let report =
            super::run_payout_withdrawal(config_path.as_path(), "lnbc21withdraw", Some(21)).await?;
        ensure(
            report.payout_destination.as_deref() == Some("lnurlp:provider")
                && report.payment_id == "payment-withdraw-001"
                && report.status == "completed"
                && report.amount_sats == 21
                && report.fees_sats == 1
                && report.invoice == "lnbc21withdraw"
                && report.post_balance.total_sats == 144,
            "payout withdrawal should surface the retained destination and payment outcome",
        )?;

        let ledger = load_ledger(config_path.as_path())?;
        ensure(
            ledger.wallet.last_balance_sats == Some(144)
                && ledger
                    .wallet
                    .payments
                    .iter()
                    .any(|payment| payment.payment_id == "payment-withdraw-001")
                && ledger.payouts.iter().any(|payout| {
                    payout.payout_id == "withdrawal:payment-withdraw-001"
                        && payout.payment_id.as_deref() == Some("payment-withdraw-001")
                        && payout.status == "completed"
                        && payout.invoice.as_deref() == Some("lnbc21withdraw")
                        && payout.payout_destination.as_deref() == Some("lnurlp:provider")
                })
                && ledger
                    .relay_activity
                    .iter()
                    .any(|entry| entry.kind == "payout.withdrawal_submitted"),
            "payout withdrawal should persist payment history, payout history, and relay activity",
        )?;

        super::set_test_payout_pay_hook(None);
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn provider_scan_filters_targeted_requests() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let config = load_or_create_config(config_path.as_path())?;
        let identity = ensure_identity(config.identity_path.as_path())?;

        let ollama_listener = TcpListener::bind("127.0.0.1:0").await?;
        let ollama_addr = ollama_listener.local_addr()?;
        let ollama_server = tokio::spawn(async move {
            let (mut stream, _) = ollama_listener.accept().await.expect("accept ollama");
            let mut request = vec![0u8; 4096];
            let _ = stream
                .read(&mut request)
                .await
                .expect("read ollama request");
            let body = json!({
                "models": [{"name": "gemma4-e4b-local:latest"}]
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write ollama response");
        });

        let relay_listener = TcpListener::bind("127.0.0.1:0").await?;
        let relay_addr = relay_listener.local_addr()?;
        let relay_url = format!("ws://{relay_addr}");
        let provider_pubkey = identity.public_key_hex.clone();
        let relay_server = tokio::spawn(async move {
            let (stream, _) = relay_listener.accept().await.expect("accept relay client");
            let mut ws = accept_async(stream).await.expect("upgrade relay websocket");
            while let Some(message) = ws.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                if !payload.contains("\"REQ\"") {
                    continue;
                }
                let matching = json!(["EVENT", "scan", {
                    "id": "match-job-001",
                    "pubkey": "buyer-pubkey-001",
                    "created_at": 1_760_000_100u64,
                    "kind": 5050,
                    "tags": [
                        ["i", "hello from the mesh", "text"],
                        ["bid", "2000"],
                        ["p", provider_pubkey]
                    ],
                    "content": "",
                    "sig": "00".repeat(64)
                }]);
                ws.send(Message::Text(matching.to_string().into()))
                    .await
                    .expect("send matching request");
                let targeted_elsewhere = json!(["EVENT", "scan", {
                    "id": "drop-job-001",
                    "pubkey": "buyer-pubkey-002",
                    "created_at": 1_760_000_101u64,
                    "kind": 5050,
                    "tags": [
                        ["i", "hello elsewhere", "text"],
                        ["bid", "3000"],
                        ["p", "different-provider"]
                    ],
                    "content": "",
                    "sig": "11".repeat(64)
                }]);
                ws.send(Message::Text(targeted_elsewhere.to_string().into()))
                    .await
                    .expect("send dropped request");
                let broadcast = json!(["EVENT", "scan", {
                    "id": "broadcast-job-001",
                    "pubkey": "buyer-pubkey-003",
                    "created_at": 1_760_000_102u64,
                    "kind": 5050,
                    "tags": [
                        ["i", "hello broadcast", "text"],
                        ["bid", "2500"]
                    ],
                    "content": "",
                    "sig": "22".repeat(64)
                }]);
                ws.send(Message::Text(broadcast.to_string().into()))
                    .await
                    .expect("send broadcast request");
                return;
            }
            panic!("relay did not receive a REQ frame");
        });

        let mut config = load_or_create_config(config_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.relay_urls = vec![relay_url];
        config.local_gemma_base_url = format!("http://{ollama_addr}");
        save_config(config_path.as_path(), &config)?;

        let report = scan_provider_requests(config_path.as_path(), 1).await?;
        ensure(
            report.matched_count == 2,
            "scan should keep matching and broadcast jobs",
        )?;
        ensure(
            report.dropped_count == 1,
            "scan should drop the job targeted at a different provider",
        )?;
        ensure(
            report
                .entries
                .iter()
                .any(|entry| entry.drop_reason.as_deref() == Some("targeted_elsewhere")),
            "scan should surface the targeted_elsewhere reason",
        )?;

        let ledger = load_ledger(config_path.as_path())?;
        ensure(
            ledger.jobs.len() == 3,
            "scan should persist all observed jobs",
        )?;
        ensure(
            ledger
                .relay_activity
                .iter()
                .any(|entry| entry.kind == "nip90.request_dropped"),
            "scan should persist dropped-request activity",
        )?;

        relay_server.await?;
        ollama_server.await?;
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn provider_run_processes_matching_request_locally()
    -> Result<(), Box<dyn std::error::Error>> {
        let _guard = super::nip90_runtime::lock_test_runtime();
        let base_url =
            start_mock_http_server(
                |method, path, body| match (method.as_str(), path.as_str()) {
                    ("GET", "/api/tags") => (
                        200,
                        "application/json",
                        json!({
                            "models": [
                                {"name": "gemma4:e4b"}
                            ]
                        })
                        .to_string(),
                    ),
                    ("POST", "/api/chat") => {
                        let request: serde_json::Value =
                            serde_json::from_str(body.as_str()).expect("valid ollama chat body");
                        assert_eq!(request["model"], json!("gemma4:e4b"));
                        assert_eq!(request["messages"][0]["content"], json!("hello from buyer"));
                        (
                            200,
                            "application/x-ndjson",
                            concat!(
                                "{\"message\":{\"content\":\"mesh \"},\"done\":false}\n",
                                "{\"message\":{\"content\":\"reply\"},\"done\":false}\n",
                                "{\"done\":true}\n"
                            )
                            .to_string(),
                        )
                    }
                    _ => (500, "text/plain", "unexpected request".to_string()),
                },
            )
            .await?;

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let config = load_or_create_config(config_path.as_path())?;
        let identity = ensure_identity(config.identity_path.as_path())?;

        let relay_listener = TcpListener::bind("127.0.0.1:0").await?;
        let relay_addr = relay_listener.local_addr()?;
        let relay_url = format!("ws://{relay_addr}");
        let provider_pubkey = identity.public_key_hex.clone();
        let relay_server = tokio::spawn(async move {
            let (scan_stream, _) = relay_listener.accept().await.expect("accept scan client");
            let mut scan_ws = accept_async(scan_stream)
                .await
                .expect("upgrade scan websocket");
            while let Some(message) = scan_ws.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                if !payload.contains("\"REQ\"") {
                    continue;
                }
                let matching = json!(["EVENT", "run", {
                    "id": "run-job-001",
                    "pubkey": "buyer-pubkey-001",
                    "created_at": 1_760_000_200u64,
                    "kind": 5050,
                    "tags": [
                        ["i", "hello from buyer", "text"],
                        ["param", "model", "gemma4:e4b"],
                        ["p", provider_pubkey]
                    ],
                    "content": "",
                    "sig": "33".repeat(64)
                }]);
                scan_ws
                    .send(Message::Text(matching.to_string().into()))
                    .await
                    .expect("send matching request");
                break;
            }
            drop(scan_ws);

            let (publish_stream, _) = relay_listener
                .accept()
                .await
                .expect("accept publish client");
            let mut publish_ws = accept_async(publish_stream)
                .await
                .expect("upgrade publish websocket");
            let mut published = Vec::new();
            while let Some(message) = publish_ws.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                if !payload.contains("\"EVENT\"") {
                    continue;
                }
                let value: serde_json::Value =
                    serde_json::from_str(payload.as_str()).expect("parse published event");
                published.push(value[1].clone());
                if published.len() == 2 {
                    return published;
                }
            }
            panic!("relay did not receive the published feedback and result events");
        });

        let mut config = load_or_create_config(config_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.relay_urls = vec![relay_url];
        config.local_gemma_base_url = base_url;
        save_config(config_path.as_path(), &config)?;
        let _ = apply_control_command(config_path.as_path(), ProviderControlAction::Online).await?;

        let report = run_provider_requests(config_path.as_path(), 1).await?;
        ensure(
            report.accepted_count == 1 && report.completed_count == 1,
            "provider run should accept and complete the matching request",
        )?;
        ensure(
            report.entries.iter().any(|entry| {
                entry.request_event_id == "run-job-001"
                    && entry.status == "completed"
                    && entry
                        .result_preview
                        .as_deref()
                        .is_some_and(|value| value.contains("mesh reply"))
                    && entry.feedback_event_ids.len() == 1
                    && entry.result_event_id.is_some()
            }),
            "provider run report should surface the local result preview",
        )?;

        let published = relay_server.await?;
        ensure(
            published.len() == 2,
            "provider run should publish both a feedback event and a result event",
        )?;
        ensure(
            published.iter().any(|event| event["kind"] == 7000),
            "provider run should publish kind:7000 processing feedback",
        )?;
        ensure(
            published.iter().any(|event| event["kind"] == 6050),
            "provider run should publish kind:6050 job results",
        )?;

        let ledger = load_ledger(config_path.as_path())?;
        let job = ledger
            .jobs
            .iter()
            .find(|job| job.id == "run-job-001")
            .ok_or_else(|| std::io::Error::other("missing processed provider job"))?;
        ensure(
            job.status == "completed_local",
            "provider run should persist the completed local lifecycle state",
        )?;
        ensure(
            job.result_preview
                .as_deref()
                .is_some_and(|value| value.contains("mesh reply")),
            "provider run should persist the streamed local result preview",
        )?;
        ensure(
            job.feedback_event_ids.len() == 1 && job.result_event_id.is_some(),
            "provider run should persist the published feedback and result event ids",
        )?;
        ensure(
            ledger
                .relay_activity
                .iter()
                .any(|entry| entry.kind == "nip90.result_published"),
            "provider run should persist result publication activity",
        )?;
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn provider_run_publishes_payment_required_feedback_and_persists_invoice()
    -> Result<(), Box<dyn std::error::Error>> {
        let _guard = super::nip90_runtime::lock_test_runtime();
        super::nip90_runtime::set_test_wallet_invoice_hook(Some(Box::new(
            |amount_sats, description, _expiry_seconds| {
                Ok(WalletInvoiceReport {
                    runtime: WalletRuntimeSurface::default(),
                    invoice: PylonWalletInvoiceRecord {
                        invoice_id: "invoice-001".to_string(),
                        amount_sats,
                        status: "created".to_string(),
                        payment_request: "lnbc3000n1pyloninvoice".to_string(),
                        description,
                        created_at_ms: 1_762_000_000_000,
                        updated_at_ms: 1_762_000_000_000,
                    },
                })
            },
        )));

        let base_url =
            start_mock_http_server(
                |method, path, _body| match (method.as_str(), path.as_str()) {
                    ("GET", "/api/tags") => (
                        200,
                        "application/json",
                        json!({
                            "models": [
                                {"name": "gemma4:e4b"}
                            ]
                        })
                        .to_string(),
                    ),
                    _ => (500, "text/plain", "unexpected request".to_string()),
                },
            )
            .await?;

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let config = load_or_create_config(config_path.as_path())?;
        let identity = ensure_identity(config.identity_path.as_path())?;

        let relay_listener = TcpListener::bind("127.0.0.1:0").await?;
        let relay_addr = relay_listener.local_addr()?;
        let relay_url = format!("ws://{relay_addr}");
        let provider_pubkey = identity.public_key_hex.clone();
        let relay_server = tokio::spawn(async move {
            let (scan_stream, _) = relay_listener.accept().await.expect("accept scan client");
            let mut scan_ws = accept_async(scan_stream)
                .await
                .expect("upgrade scan websocket");
            while let Some(message) = scan_ws.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                if !payload.contains("\"REQ\"") {
                    continue;
                }
                let matching = json!(["EVENT", "run", {
                    "id": "run-job-pay-001",
                    "pubkey": "buyer-pubkey-001",
                    "created_at": 1_760_000_210u64,
                    "kind": 5050,
                    "tags": [
                        ["i", "pay before run", "text"],
                        ["param", "model", "gemma4:e4b"],
                        ["bid", "24000"],
                        ["p", provider_pubkey]
                    ],
                    "content": "",
                    "sig": "44".repeat(64)
                }]);
                scan_ws
                    .send(Message::Text(matching.to_string().into()))
                    .await
                    .expect("send matching request");
                break;
            }
            drop(scan_ws);

            let (publish_stream, _) = relay_listener
                .accept()
                .await
                .expect("accept publish client");
            let mut publish_ws = accept_async(publish_stream)
                .await
                .expect("upgrade publish websocket");
            while let Some(message) = publish_ws.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                if !payload.contains("\"EVENT\"") {
                    continue;
                }
                let value: serde_json::Value =
                    serde_json::from_str(payload.as_str()).expect("parse published event");
                return value[1].clone();
            }
            panic!("relay did not receive the payment-required feedback event");
        });

        let mut config = load_or_create_config(config_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.relay_urls = vec![relay_url];
        config.local_gemma_base_url = base_url;
        save_config(config_path.as_path(), &config)?;
        let _ = apply_control_command(config_path.as_path(), ProviderControlAction::Online).await?;

        let report = run_provider_requests(config_path.as_path(), 1).await?;
        ensure(
            report.accepted_count == 1
                && report.payment_required_count == 1
                && report.completed_count == 0,
            "provider run should stop at payment-required for priced jobs",
        )?;
        ensure(
            report.entries.iter().any(|entry| {
                entry.request_event_id == "run-job-pay-001"
                    && entry.status == "payment_required"
                    && entry.amount_msats == Some(21_000)
                    && entry.bolt11.as_deref() == Some("lnbc3000n1pyloninvoice")
                    && entry.feedback_event_ids.len() == 1
                    && entry.result_event_id.is_none()
            }),
            "provider run should surface the invoice-bearing payment-required state",
        )?;

        let published = relay_server.await?;
        ensure(
            published["kind"] == 7000,
            "provider run should publish kind:7000 payment-required feedback",
        )?;
        ensure(
            published["tags"].as_array().is_some_and(|tags| {
                tags.iter().any(|tag| {
                    tag.as_array().is_some_and(|items| {
                        items.len() >= 2 && items[0] == "status" && items[1] == "payment-required"
                    })
                })
            }),
            "payment-required feedback should carry the payment-required status tag",
        )?;
        ensure(
            published["tags"].as_array().is_some_and(|tags| {
                tags.iter().any(|tag| {
                    tag.as_array().is_some_and(|items| {
                        items.len() >= 3
                            && items[0] == "amount"
                            && items[1] == "21000"
                            && items[2] == "lnbc3000n1pyloninvoice"
                    })
                })
            }),
            "payment-required feedback should carry amount and bolt11 tags",
        )?;

        let ledger = load_ledger(config_path.as_path())?;
        let job = ledger
            .jobs
            .iter()
            .find(|job| job.id == "run-job-pay-001")
            .ok_or_else(|| std::io::Error::other("missing payment-required provider job"))?;
        ensure(
            job.status == "payment_required",
            "provider run should persist the payment-required lifecycle state",
        )?;
        ensure(
            job.amount_msats == Some(21_000)
                && job.bolt11.as_deref() == Some("lnbc3000n1pyloninvoice")
                && job.feedback_event_ids.len() == 1
                && job.result_event_id.is_none(),
            "provider run should persist invoice amount, bolt11, and published feedback",
        )?;
        ensure(
            ledger
                .relay_activity
                .iter()
                .any(|entry| entry.kind == "nip90.job_payment_required"),
            "provider run should persist payment-required relay activity",
        )?;

        super::nip90_runtime::set_test_wallet_invoice_hook(None);
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn provider_run_settles_paid_request_and_projects_retained_views()
    -> Result<(), Box<dyn std::error::Error>> {
        let _guard = super::nip90_runtime::lock_test_runtime();
        super::nip90_runtime::set_test_wallet_invoice_hook(Some(Box::new(
            |amount_sats, description, _expiry_seconds| {
                Ok(WalletInvoiceReport {
                    runtime: WalletRuntimeSurface::default(),
                    invoice: PylonWalletInvoiceRecord {
                        invoice_id: "invoice-002".to_string(),
                        amount_sats,
                        status: "created".to_string(),
                        payment_request: "lnbc21000n1pyloninvoice".to_string(),
                        description,
                        created_at_ms: 1_762_000_100_000,
                        updated_at_ms: 1_762_000_100_000,
                    },
                })
            },
        )));
        super::nip90_runtime::set_test_wallet_payments_hook(Some(Box::new(|| Ok(Vec::new()))));

        let base_url =
            start_mock_http_server(
                |method, path, body| match (method.as_str(), path.as_str()) {
                    ("GET", "/api/tags") => (
                        200,
                        "application/json",
                        json!({
                            "models": [
                                {"name": "gemma4:e4b"}
                            ]
                        })
                        .to_string(),
                    ),
                    ("POST", "/api/chat") => {
                        let request: serde_json::Value =
                            serde_json::from_str(body.as_str()).expect("valid ollama chat body");
                        assert_eq!(request["model"], json!("gemma4:e4b"));
                        assert_eq!(request["messages"][0]["content"], json!("paid run"));
                        (
                            200,
                            "application/x-ndjson",
                            concat!(
                                "{\"message\":{\"content\":\"paid \"},\"done\":false}\n",
                                "{\"message\":{\"content\":\"reply\"},\"done\":false}\n",
                                "{\"done\":true}\n"
                            )
                            .to_string(),
                        )
                    }
                    _ => (500, "text/plain", "unexpected request".to_string()),
                },
            )
            .await?;

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let config = load_or_create_config(config_path.as_path())?;
        let identity = ensure_identity(config.identity_path.as_path())?;

        let relay_listener = TcpListener::bind("127.0.0.1:0").await?;
        let relay_addr = relay_listener.local_addr()?;
        let relay_url = format!("ws://{relay_addr}");
        let provider_pubkey = identity.public_key_hex.clone();
        let relay_server = tokio::spawn(async move {
            let matching = || {
                json!(["EVENT", "run", {
                    "id": "run-job-pay-002",
                    "pubkey": "buyer-pubkey-002",
                    "created_at": 1_760_000_220u64,
                    "kind": 5050,
                    "tags": [
                        ["i", "paid run", "text"],
                        ["param", "model", "gemma4:e4b"],
                        ["bid", "24000"],
                        ["p", provider_pubkey]
                    ],
                    "content": "",
                    "sig": "55".repeat(64)
                }])
            };

            let (scan_stream_1, _) = relay_listener.accept().await.expect("accept scan client 1");
            let mut scan_ws_1 = accept_async(scan_stream_1)
                .await
                .expect("upgrade scan websocket 1");
            while let Some(message) = scan_ws_1.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                if payload.contains("\"REQ\"") {
                    scan_ws_1
                        .send(Message::Text(matching().to_string().into()))
                        .await
                        .expect("send first matching request");
                    break;
                }
            }
            drop(scan_ws_1);

            let (publish_stream_1, _) = relay_listener
                .accept()
                .await
                .expect("accept publish client 1");
            let mut publish_ws_1 = accept_async(publish_stream_1)
                .await
                .expect("upgrade publish websocket 1");
            let first_published = loop {
                let Some(message) = publish_ws_1.next().await else {
                    panic!("missing first publish");
                };
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                if !payload.contains("\"EVENT\"") {
                    continue;
                }
                let value: serde_json::Value =
                    serde_json::from_str(payload.as_str()).expect("parse first published event");
                break value[1].clone();
            };
            drop(publish_ws_1);

            let (scan_stream_2, _) = relay_listener.accept().await.expect("accept scan client 2");
            let mut scan_ws_2 = accept_async(scan_stream_2)
                .await
                .expect("upgrade scan websocket 2");
            while let Some(message) = scan_ws_2.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                if payload.contains("\"REQ\"") {
                    scan_ws_2
                        .send(Message::Text(matching().to_string().into()))
                        .await
                        .expect("send second matching request");
                    break;
                }
            }
            drop(scan_ws_2);

            let (publish_stream_2, _) = relay_listener
                .accept()
                .await
                .expect("accept publish client 2");
            let mut publish_ws_2 = accept_async(publish_stream_2)
                .await
                .expect("upgrade publish websocket 2");
            let mut second_published = Vec::new();
            while let Some(message) = publish_ws_2.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                if !payload.contains("\"EVENT\"") {
                    continue;
                }
                let value: serde_json::Value =
                    serde_json::from_str(payload.as_str()).expect("parse second published event");
                second_published.push(value[1].clone());
                if second_published.len() == 2 {
                    break;
                }
            }
            (first_published, second_published)
        });

        let mut config = load_or_create_config(config_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.relay_urls = vec![relay_url];
        config.local_gemma_base_url = base_url;
        save_config(config_path.as_path(), &config)?;
        let _ = apply_control_command(config_path.as_path(), ProviderControlAction::Online).await?;

        let first_report = run_provider_requests(config_path.as_path(), 1).await?;
        ensure(
            first_report.payment_required_count == 1 && first_report.completed_count == 0,
            "first provider run should stop at payment-required",
        )?;

        super::nip90_runtime::set_test_wallet_payments_hook(Some(Box::new(|| {
            Ok(vec![PylonWalletPaymentRecord {
                payment_id: "payment-recv-001".to_string(),
                direction: "receive".to_string(),
                status: "completed".to_string(),
                amount_sats: 21,
                fees_sats: 0,
                method: "lightning".to_string(),
                description: Some("pylon nip90 run-job-pay-002".to_string()),
                invoice: Some("lnbc21000n1pyloninvoice".to_string()),
                created_at_ms: 1_762_000_120_000,
                updated_at_ms: 1_762_000_120_000,
            }])
        })));

        let second_report = run_provider_requests(config_path.as_path(), 1).await?;
        ensure(
            second_report.completed_count == 1 && second_report.settled_count == 1,
            "second provider run should complete and settle the paid request",
        )?;
        ensure(
            second_report.entries.iter().any(|entry| {
                entry.request_event_id == "run-job-pay-002"
                    && entry.status == "settled"
                    && entry.payment_id.as_deref() == Some("payment-recv-001")
                    && entry.settlement_id.is_some()
                    && entry
                        .result_preview
                        .as_deref()
                        .is_some_and(|value| value.contains("paid reply"))
                    && entry.result_event_id.is_some()
            }),
            "second provider run should surface settled payment and result publication",
        )?;

        let ledger = load_ledger(config_path.as_path())?;
        let job = ledger
            .jobs
            .iter()
            .find(|job| job.id == "run-job-pay-002")
            .ok_or_else(|| std::io::Error::other("missing settled provider job"))?;
        ensure(
            job.status == "settled"
                && job.payment_id.as_deref() == Some("payment-recv-001")
                && job.settlement_id.is_some()
                && job.result_event_id.is_some(),
            "ledger should persist settled provider job state",
        )?;

        let jobs_report = load_jobs_report(config_path.as_path(), Some(4)).await?;
        ensure(
            jobs_report.jobs.iter().any(|job| {
                job.job_id == "run-job-pay-002"
                    && job.status == "settled"
                    && job.payout_sats == 21
                    && job.payment_pointer == "payment-recv-001"
            }),
            "jobs report should include the retained settled provider job",
        )?;

        let earnings_report = load_earnings_report(config_path.as_path()).await?;
        ensure(
            earnings_report.earnings.as_ref().is_some_and(|earnings| {
                earnings.lifetime_sats == 21
                    && earnings.jobs_today == 1
                    && earnings.last_job_result == "settled"
            }),
            "earnings report should project retained provider settlement totals",
        )?;

        let receipts_report = load_receipts_report(config_path.as_path(), Some(4)).await?;
        ensure(
            receipts_report.receipts.iter().any(|receipt| {
                receipt.work_unit_id.as_deref() == Some("run-job-pay-002")
                    && receipt.reason_code.as_deref() == Some("SETTLED")
                    && receipt.notional_sats == Some(21)
            }),
            "receipts report should include the retained settlement receipt",
        )?;

        let (first_published, second_published) = relay_server.await?;
        ensure(
            first_published["kind"] == 7000,
            "first publish should be payment-required feedback",
        )?;
        ensure(
            second_published.len() == 2
                && second_published.iter().any(|event| event["kind"] == 7000)
                && second_published.iter().any(|event| event["kind"] == 6050),
            "second publish should include processing feedback and a result event",
        )?;

        super::nip90_runtime::set_test_wallet_invoice_hook(None);
        super::nip90_runtime::set_test_wallet_payments_hook(None);
        Ok(())
    }

    fn ready_health(
        ready_model: &str,
        available_models: &[&str],
        availability_message: Option<&str>,
    ) -> ProviderBackendHealth {
        ProviderBackendHealth {
            reachable: true,
            ready: true,
            configured_model: Some(ready_model.to_string()),
            ready_model: Some(ready_model.to_string()),
            available_models: available_models
                .iter()
                .map(|model| (*model).to_string())
                .collect(),
            last_error: None,
            last_action: Some("health check ready".to_string()),
            availability_message: availability_message.map(str::to_string),
            latency_ms_p50: Some(110),
        }
    }

    fn seed_observability_snapshot(
        config_path: &std::path::Path,
    ) -> Result<PylonConfig, Box<dyn std::error::Error>> {
        let mut config = load_or_create_config(config_path)?;
        let identity = ensure_identity(config.identity_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.inventory_controls.sandbox_python_exec_enabled = true;
        config.declared_sandbox_profiles = vec![ProviderSandboxProfileSpec {
            profile_id: "python-batch".to_string(),
            execution_class: ProviderSandboxExecutionClass::PythonExec,
            runtime_family: "python3".to_string(),
            runtime_version: Some("Python 3.11.8".to_string()),
            sandbox_engine: "local_subprocess".to_string(),
            os_family: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpu_limit: 2,
            memory_limit_mb: 2048,
            disk_limit_mb: 4096,
            timeout_limit_s: 120,
            network_mode: "none".to_string(),
            filesystem_mode: "workspace_only".to_string(),
            workspace_mode: "ephemeral".to_string(),
            artifact_output_mode: "declared_paths_only".to_string(),
            secrets_mode: "none".to_string(),
            allowed_binaries: vec!["python3".to_string()],
            toolchain_inventory: vec!["python3".to_string()],
            container_image: None,
            runtime_image_digest: None,
            accelerator_policy: None,
        }];
        save_config(config_path, &config)?;

        let availability = ProviderAvailability {
            local_gemma: ready_health("gemma4:e4b", &["gemma4:e4b"], Some("gemma_ready")),
            apple_foundation_models: ProviderBackendHealth::default(),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
            pooled_inference: ProviderPooledInferenceAvailability::default(),
            sandbox: ProviderSandboxAvailability {
                runtimes: vec![ProviderSandboxRuntimeHealth {
                    runtime_kind: ProviderSandboxRuntimeKind::Python,
                    detected: true,
                    ready: true,
                    binary_name: Some("python3".to_string()),
                    binary_path: Some("/usr/bin/python3".to_string()),
                    runtime_version: Some("Python 3.11.8".to_string()),
                    supported_execution_classes: vec![
                        ProviderSandboxExecutionClass::PythonExec,
                    ],
                    last_error: None,
                }],
                profiles: vec![ProviderSandboxProfile {
                    profile_id: "python-batch".to_string(),
                    profile_digest: "sha256:python-profile".to_string(),
                    execution_class: ProviderSandboxExecutionClass::PythonExec,
                    runtime_family: "python3".to_string(),
                    runtime_version: "Python 3.11.8".to_string(),
                    sandbox_engine: "local_subprocess".to_string(),
                    os_family: std::env::consts::OS.to_string(),
                    arch: std::env::consts::ARCH.to_string(),
                    cpu_limit: 2,
                    memory_limit_mb: 2048,
                    disk_limit_mb: 4096,
                    timeout_limit_s: 120,
                    network_mode: "none".to_string(),
                    filesystem_mode: "workspace_only".to_string(),
                    workspace_mode: "ephemeral".to_string(),
                    artifact_output_mode: "declared_paths_only".to_string(),
                    secrets_mode: "none".to_string(),
                    allowed_binaries: vec!["python3".to_string()],
                    toolchain_inventory: vec!["python3".to_string()],
                    container_image: None,
                    runtime_image_digest: None,
                    accelerator_policy: None,
                    runtime_kind: ProviderSandboxRuntimeKind::Python,
                    runtime_ready: true,
                    runtime_binary_path: Some("/usr/bin/python3".to_string()),
                    capability_summary: "backend=sandbox execution=sandbox.python.exec family=sandbox_execution profile_id=python-batch".to_string(),
                }],
                last_scan_error: None,
            },
        };
        let mut snapshot = build_snapshot_from_availability(
            &config,
            Some(&identity),
            ProviderDesiredMode::Online,
            None,
            availability.clone(),
            None,
        );
        snapshot.inventory_rows = inventory_rows(
            &super::products_from_availability(&config, &availability),
            ProviderDesiredMode::Online,
        );
        snapshot.recent_jobs = vec![
            ProviderRecentJob {
                job_id: "job-1".to_string(),
                request_id: Some("req-1".to_string()),
                status: "settled".to_string(),
                demand_source: "open_network".to_string(),
                product_id: Some("psionic.local.inference.gemma.single_node".to_string()),
                compute_family: Some("inference".to_string()),
                backend_family: Some("local_gemma".to_string()),
                sandbox_execution_class: None,
                sandbox_profile_id: None,
                sandbox_profile_digest: None,
                sandbox_termination_reason: None,
                completed_at_epoch_seconds: 1_762_300_030,
                payout_sats: 42,
                payment_pointer: "payment-1".to_string(),
                failure_reason: None,
                delivery_proof_id: Some("proof-1".to_string()),
            },
            ProviderRecentJob {
                job_id: "job-2".to_string(),
                request_id: Some("req-2".to_string()),
                status: "failed".to_string(),
                demand_source: "open_network".to_string(),
                product_id: Some("sandbox.python.exec".to_string()),
                compute_family: Some("sandbox_execution".to_string()),
                backend_family: Some("sandbox".to_string()),
                sandbox_execution_class: Some("sandbox.python.exec".to_string()),
                sandbox_profile_id: Some("python-batch".to_string()),
                sandbox_profile_digest: Some("sha256:python-profile".to_string()),
                sandbox_termination_reason: Some("timeout".to_string()),
                completed_at_epoch_seconds: 1_762_300_032,
                payout_sats: 0,
                payment_pointer: "payment-2".to_string(),
                failure_reason: Some("sandbox execution exceeded timeout".to_string()),
                delivery_proof_id: Some("proof-2".to_string()),
            },
        ];
        snapshot.receipts = vec![
            ProviderReceiptSummary {
                receipt_id: "receipt-1".to_string(),
                receipt_type: "earn.job.settled.v1".to_string(),
                created_at_ms: 1_762_300_030_500,
                canonical_hash: "sha256:receipt-1".to_string(),
                compute_family: Some("inference".to_string()),
                backend_family: Some("local_gemma".to_string()),
                sandbox_execution_class: None,
                sandbox_profile_id: None,
                sandbox_profile_digest: None,
                sandbox_termination_reason: None,
                reason_code: Some("SETTLED".to_string()),
                failure_reason: None,
                severity: Some("low".to_string()),
                notional_sats: Some(42),
                liability_premium_sats: Some(0),
                work_unit_id: Some("work-unit-1".to_string()),
            },
            ProviderReceiptSummary {
                receipt_id: "receipt-2".to_string(),
                receipt_type: "sandbox.execution.delivery.v1".to_string(),
                created_at_ms: 1_762_300_032_500,
                canonical_hash: "sha256:receipt-2".to_string(),
                compute_family: Some("sandbox_execution".to_string()),
                backend_family: Some("sandbox".to_string()),
                sandbox_execution_class: Some("sandbox.python.exec".to_string()),
                sandbox_profile_id: Some("python-batch".to_string()),
                sandbox_profile_digest: Some("sha256:python-profile".to_string()),
                sandbox_termination_reason: Some("timeout".to_string()),
                reason_code: Some("SANDBOX_TIMEOUT".to_string()),
                failure_reason: Some("sandbox execution exceeded timeout".to_string()),
                severity: Some("warn".to_string()),
                notional_sats: Some(0),
                liability_premium_sats: Some(0),
                work_unit_id: Some("work-unit-2".to_string()),
            },
        ];
        snapshot.earnings = Some(ProviderEarningsSummary {
            sats_today: 42,
            lifetime_sats: 420,
            jobs_today: 1,
            online_uptime_seconds: 45,
            last_job_result: "settled".to_string(),
            first_job_latency_seconds: Some(8),
            completion_ratio_bps: Some(10_000),
            payout_success_ratio_bps: Some(10_000),
            avg_wallet_confirmation_latency_seconds: Some(3),
        });
        snapshot.config_metadata.push(super::ProviderJsonEntry {
            key: "test_marker".to_string(),
            value: json!("observability"),
        });

        let admin_config = provider_admin_config(&config)?;
        let mut store = ProviderPersistenceStore::open(&admin_config)?;
        store.set_listen_addr(config.admin_listen_addr.as_str())?;
        store.set_desired_mode(ProviderDesiredMode::Online)?;
        store.persist_snapshot(&snapshot)?;
        Ok(config)
    }

    async fn start_mock_http_server<F>(responder: F) -> std::io::Result<String>
    where
        F: Fn(String, String, String) -> (u16, &'static str, String) + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let request = read_http_request(&mut stream).await;
                let (status, content_type, body) = match request {
                    Ok((method, path, body)) => responder(method, path, body),
                    Err(error) => (
                        500,
                        "text/plain",
                        format!("failed to read test request: {error}"),
                    ),
                };
                let _ = write_http_response(&mut stream, status, content_type, body.as_str()).await;
            }
        });
        Ok(format!("http://{addr}"))
    }

    async fn read_http_request(
        stream: &mut TcpStream,
    ) -> std::io::Result<(String, String, String)> {
        let mut buffer = Vec::new();
        let mut header_end = None;
        while header_end.is_none() {
            let mut chunk = [0_u8; 1024];
            let read = stream.read(&mut chunk).await?;
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            header_end = find_header_end(buffer.as_slice());
        }
        let Some(header_end) = header_end else {
            return Err(std::io::Error::other("missing request headers"));
        };
        let head = String::from_utf8(buffer[..header_end].to_vec())
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let mut lines = head.lines();
        let request_line = lines
            .next()
            .ok_or_else(|| std::io::Error::other("missing request line"))?;
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts
            .next()
            .ok_or_else(|| std::io::Error::other("missing request method"))?
            .to_string();
        let path = request_parts
            .next()
            .ok_or_else(|| std::io::Error::other("missing request path"))?
            .to_string();

        let content_length = lines
            .filter_map(|line| line.split_once(':'))
            .find_map(|(name, value)| {
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>())
            })
            .transpose()
            .map_err(|error| std::io::Error::other(error.to_string()))?
            .unwrap_or(0);

        let body_start = match buffer[header_end..].strip_prefix(b"\r\n\r\n") {
            Some(_) => header_end + 4,
            None => header_end + 2,
        };
        while buffer.len() < body_start + content_length {
            let mut chunk = vec![0_u8; body_start + content_length - buffer.len()];
            let read = stream.read(&mut chunk).await?;
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
        }
        let body = String::from_utf8(buffer[body_start..body_start + content_length].to_vec())
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        Ok((method, path, body))
    }

    fn find_header_end(buffer: &[u8]) -> Option<usize> {
        buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .or_else(|| buffer.windows(2).position(|window| window == b"\n\n"))
    }

    async fn write_http_response(
        stream: &mut TcpStream,
        status: u16,
        content_type: &str,
        body: &str,
    ) -> std::io::Result<()> {
        let status_text = match status {
            200 => "OK",
            500 => "Internal Server Error",
            _ => "OK",
        };
        let response = format!(
            "HTTP/1.1 {status} {status_text}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).await?;
        stream.flush().await?;
        Ok(())
    }

    fn collect_chat_deltas(events: &[LocalGemmaChatEvent]) -> String {
        events
            .iter()
            .filter_map(|event| match event {
                LocalGemmaChatEvent::Delta(value) => Some(value.as_str()),
                LocalGemmaChatEvent::Started { .. } | LocalGemmaChatEvent::Finished { .. } => None,
            })
            .collect::<String>()
    }

    #[test]
    fn resolve_local_gemma_chat_target_prefers_ready_local_gemma_runtime()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let config = load_or_create_config(config_path.as_path())?;
        let identity = ensure_identity(config.identity_path.as_path())?;
        let availability = ProviderAvailability {
            local_gemma: ready_health("gemma4:e4b", &["gemma4:e4b"], None),
            apple_foundation_models: ProviderBackendHealth::default(),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
            pooled_inference: ProviderPooledInferenceAvailability::default(),
            sandbox: ProviderSandboxAvailability::default(),
        };
        let status = super::ProviderStatusResponse {
            listen_addr: Some(config.admin_listen_addr.clone()),
            desired_mode: ProviderDesiredMode::Offline,
            snapshot: Some(build_snapshot_from_availability(
                &config,
                Some(&identity),
                ProviderDesiredMode::Offline,
                None,
                availability,
                None,
            )),
        };

        let target = resolve_local_gemma_chat_target_from_status(&config, &status)?;
        ensure(
            target.backend == LocalGemmaChatBackend::LocalRuntime && target.model == "gemma4:e4b",
            "resolver should use the ready local Gemma runtime",
        )
    }

    #[test]
    fn gemma_local_installations_reflect_cached_files() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let spec = gemma_download_spec("gemma-4-e4b")
            .ok_or_else(|| std::io::Error::other("missing gemma-4-e4b spec"))?;
        let path = super::gemma_model_path(config_path.as_path(), spec);
        std::fs::create_dir_all(
            path.parent()
                .ok_or_else(|| std::io::Error::other("missing model parent"))?,
        )?;
        std::fs::write(path.as_path(), b"gguf")?;

        let installations = gemma_local_installations(config_path.as_path());
        let found = installations
            .into_iter()
            .find(|installation| installation.spec.id == "gemma-4-e4b")
            .ok_or_else(|| std::io::Error::other("gemma-4-e4b installation missing"))?;

        ensure(
            found.installed,
            "cached GGUF should mark the model installed",
        )?;
        ensure(
            found.file_bytes == Some(4),
            "cached GGUF should report its local file size",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gemma_download_emits_progress_and_writes_cache()
    -> Result<(), Box<dyn std::error::Error>> {
        let spec = gemma_download_spec("gemma-4-e2b")
            .ok_or_else(|| std::io::Error::other("missing gemma-4-e2b spec"))?;
        let payload = "x".repeat(16_384);
        let expected_path = format!(
            "/{}/resolve/main/{}?download=true",
            spec.repo_id, spec.filename
        );
        let base_url = start_mock_http_server(move |method, path, _body| {
            if method == "GET" && path == expected_path {
                return (200, "application/octet-stream", payload.clone());
            }
            (500, "text/plain", "unexpected request".to_string())
        })
        .await?;

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut events = Vec::new();
        let final_path = download_gemma_model_from_base_url(
            config_path.as_path(),
            spec.id,
            base_url.as_str(),
            |event| events.push(event),
        )
        .await?;

        ensure(
            final_path.exists(),
            "download should write the final GGUF file",
        )?;
        let file_bytes = std::fs::metadata(final_path.as_path())?.len();
        ensure(
            file_bytes == 16_384,
            "downloaded GGUF should preserve the full response body",
        )?;
        ensure(
            events
                .iter()
                .any(|event| matches!(event, GemmaDownloadEvent::Started { spec, .. } if spec.id == "gemma-4-e2b")),
            "download should emit a started event",
        )?;
        ensure(
            events.iter().any(|event| matches!(
                event,
                GemmaDownloadEvent::Progress {
                    spec,
                    downloaded_bytes,
                    total_bytes
                } if spec.id == "gemma-4-e2b" && *downloaded_bytes == 16_384 && *total_bytes == Some(16_384)
            )),
            "download should emit a progress event with byte totals",
        )?;
        ensure(
            events.iter().any(|event| {
                matches!(
                    event,
                    GemmaDownloadEvent::Finished {
                        spec,
                        file_bytes,
                        ..
                    } if spec.id == "gemma-4-e2b" && *file_bytes == 16_384
                )
            }),
            "download should emit a finished event",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gemma_download_supports_explicit_curl_transport()
    -> Result<(), Box<dyn std::error::Error>> {
        if std::process::Command::new("curl")
            .arg("--version")
            .output()
            .is_err()
        {
            return Ok(());
        }

        let spec = gemma_download_spec("gemma-4-e2b")
            .ok_or_else(|| std::io::Error::other("missing gemma-4-e2b spec"))?;
        let payload = "y".repeat(8_192);
        let expected_path = format!(
            "/{}/resolve/main/{}?download=true",
            spec.repo_id, spec.filename
        );
        let base_url = start_mock_http_server(move |method, path, _body| {
            if method == "GET" && path == expected_path {
                return (200, "application/octet-stream", payload.clone());
            }
            (500, "text/plain", "unexpected request".to_string())
        })
        .await?;

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut events = Vec::new();
        let final_path = download_gemma_model_from_base_url_with_transport(
            config_path.as_path(),
            spec.id,
            base_url.as_str(),
            GemmaDownloadTransport::Curl,
            |event| events.push(event),
        )
        .await?;

        ensure(
            std::fs::metadata(final_path.as_path())?.len() == 8_192,
            "curl transport should preserve the full GGUF payload",
        )?;
        ensure(
            events.iter().any(|event| {
                matches!(
                    event,
                    GemmaDownloadEvent::Finished {
                        spec,
                        file_bytes,
                        ..
                    } if spec.id == "gemma-4-e2b" && *file_bytes == 8_192
                )
            }),
            "curl transport should still emit a finished event with the final byte size",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn local_gemma_chat_stream_emits_deltas() -> Result<(), Box<dyn std::error::Error>> {
        let base_url =
            start_mock_http_server(
                |method, path, body| match (method.as_str(), path.as_str()) {
                    ("GET", "/api/tags") => (
                        200,
                        "application/json",
                        json!({
                            "models": [
                                {"name": "gemma4:e4b"}
                            ]
                        })
                        .to_string(),
                    ),
                    ("POST", "/api/chat") => {
                        let request: serde_json::Value =
                            serde_json::from_str(body.as_str()).expect("valid ollama chat body");
                        assert_eq!(request["model"], json!("gemma4:e4b"));
                        assert_eq!(request["messages"][0]["content"], json!("hello"));
                        (
                            200,
                            "application/x-ndjson",
                            concat!(
                                "{\"message\":{\"content\":\"hello \"},\"done\":false}\n",
                                "{\"message\":{\"content\":\"world\"},\"done\":false}\n",
                                "{\"done\":true}\n"
                            )
                            .to_string(),
                        )
                    }
                    _ => (500, "text/plain", "unexpected request".to_string()),
                },
            )
            .await?;

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.local_gemma_base_url = base_url;
        save_config(config_path.as_path(), &config)?;

        let mut events = Vec::new();
        let target = run_local_gemma_chat_stream(config_path.as_path(), "hello", |event| {
            events.push(event);
        })
        .await?;

        ensure(
            target.backend == LocalGemmaChatBackend::LocalRuntime && target.model == "gemma4:e4b",
            "chat stream should resolve the local Gemma runtime target",
        )?;
        ensure(
            collect_chat_deltas(events.as_slice()) == "hello world",
            "chat stream should emit the streamed local runtime deltas in order",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn local_gemma_chat_stream_sends_prior_messages() -> Result<(), Box<dyn std::error::Error>>
    {
        let base_url =
            start_mock_http_server(
                |method, path, body| match (method.as_str(), path.as_str()) {
                    ("GET", "/api/tags") => (
                        200,
                        "application/json",
                        json!({
                            "models": [
                                {"name": "gemma4:e4b"}
                            ]
                        })
                        .to_string(),
                    ),
                    ("POST", "/api/chat") => {
                        let request: serde_json::Value =
                            serde_json::from_str(body.as_str()).expect("valid ollama chat body");
                        assert_eq!(request["messages"][0]["role"], json!("user"));
                        assert_eq!(request["messages"][0]["content"], json!("who are you"));
                        assert_eq!(request["messages"][1]["role"], json!("assistant"));
                        assert_eq!(request["messages"][1]["content"], json!("I am Gemma 4."),);
                        assert_eq!(request["messages"][2]["role"], json!("user"));
                        assert_eq!(
                            request["messages"][2]["content"],
                            json!("say that in french"),
                        );
                        (
                            200,
                            "application/x-ndjson",
                            concat!(
                                "{\"message\":{\"content\":\"Je suis Gemma 4.\"},\"done\":false}\n",
                                "{\"done\":true}\n"
                            )
                            .to_string(),
                        )
                    }
                    _ => (500, "text/plain", "unexpected request".to_string()),
                },
            )
            .await?;

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.local_gemma_base_url = base_url;
        save_config(config_path.as_path(), &config)?;

        let mut events = Vec::new();
        let target = run_local_gemma_chat_messages_stream(
            config_path.as_path(),
            &[
                LocalGemmaChatMessage::user("who are you"),
                LocalGemmaChatMessage::assistant("I am Gemma 4."),
                LocalGemmaChatMessage::user("say that in french"),
            ],
            |event| events.push(event),
        )
        .await?;

        ensure(
            target.backend == LocalGemmaChatBackend::LocalRuntime && target.model == "gemma4:e4b",
            "chat stream should still resolve the local Gemma runtime target",
        )?;
        ensure(
            collect_chat_deltas(events.as_slice()) == "Je suis Gemma 4.",
            "chat stream should preserve the streamed reply when prior turns are present",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gemma_diagnose_persists_latest_report_with_runtime_metrics()
    -> Result<(), Box<dyn std::error::Error>> {
        let recorded_requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let recorded_requests_for_server = Arc::clone(&recorded_requests);
        let base_url =
            start_mock_http_server(
                move |method, path, body| match (method.as_str(), path.as_str()) {
                    ("GET", "/api/tags") => (
                        200,
                        "application/json",
                        json!({
                            "models": [
                                {"name": "gemma4-e4b-local:latest"}
                            ]
                        })
                        .to_string(),
                    ),
                    ("POST", "/api/chat") => {
                        let request: serde_json::Value =
                            serde_json::from_str(body.as_str()).expect("valid diagnostic body");
                        recorded_requests_for_server
                            .lock()
                            .expect("diagnostic request log")
                            .push(request);
                        (
                            200,
                            "application/x-ndjson",
                            concat!(
                                "{\"message\":{\"content\":\"hello \"},\"done\":false}\n",
                                "{\"message\":{\"content\":\"world\"},\"done\":false}\n",
                                "{\"done\":true,\"total_duration\":5000000000,\"load_duration\":1000000000,\"prompt_eval_duration\":500000000,\"eval_duration\":2000000000,\"eval_count\":20}\n"
                            )
                            .to_string(),
                        )
                    }
                    _ => (500, "text/plain", "unexpected request".to_string()),
                },
            )
            .await?;

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.local_gemma_base_url = base_url;
        save_config(config_path.as_path(), &config)?;
        ensure_identity(config.identity_path.as_path())?;

        let report = run_gemma_diagnostic_command(
            config_path.as_path(),
            &GemmaBenchmarkSelector::Model("gemma-4-e4b".to_string()),
            &GemmaDiagnosticRequest {
                diagnostic_id: DEFAULT_GEMMA_DIAGNOSTIC_ID.to_string(),
                prompt: DEFAULT_GEMMA_BENCH_PROMPT.to_string(),
                max_output_tokens: 24,
                repeats: 2,
                download_missing: false,
            },
        )
        .await?;

        ensure(
            report.results.len() == 1,
            "diagnostic report should keep one result for a single selected model",
        )?;
        let result = &report.results[0];
        ensure(
            result.status == "completed",
            "diagnostic report should mark a loaded local runtime model as completed",
        )?;
        ensure(
            result.runtime_model.as_deref() == Some("gemma4-e4b-local:latest"),
            "diagnostic report should retain the exact runtime model that answered the probe",
        )?;

        let receipt = result
            .receipt
            .as_ref()
            .ok_or_else(|| std::io::Error::other("missing diagnostic receipt"))?;
        ensure(
            receipt.repeats == 2 && receipt.runs.len() == 2,
            "diagnostic receipt should retain one row per repeated run",
        )?;
        ensure(
            receipt.output_tokens == 20,
            "diagnostic receipt should use the runtime eval_count as output token truth",
        )?;
        ensure(
            receipt
                .mean_ttft_s
                .is_some_and(|value| value >= 0.0 && value <= receipt.mean_total_s),
            "diagnostic receipt should retain a bounded first-token latency",
        )?;
        ensure(
            receipt
                .mean_decode_tok_s
                .is_some_and(|value| (value - 10.0).abs() < 0.001),
            "diagnostic receipt should derive decode throughput from eval_count and eval_duration",
        )?;
        ensure(
            receipt.load_s == Some(1.0) && (receipt.mean_total_s - 5.0).abs() < 0.001,
            "diagnostic receipt should keep runtime total and load durations in seconds",
        )?;
        ensure(
            receipt
                .runs
                .iter()
                .all(|run| run.output_text == "hello world"),
            "diagnostic receipt should retain the streamed text for each run",
        )?;

        let report_path = gemma_diagnostic_latest_report_path(config_path.as_path());
        ensure(
            report.report_path == report_path.display().to_string() && report_path.exists(),
            "diagnostic command should persist the latest report under the retained diagnostics path",
        )?;
        let loaded = load_latest_gemma_diagnostic_report(config_path.as_path())?
            .ok_or_else(|| std::io::Error::other("missing latest diagnostic report"))?;
        ensure(
            loaded == report,
            "loading the latest diagnostic report should round-trip the saved JSON payload",
        )?;

        let requests = recorded_requests
            .lock()
            .expect("diagnostic request log")
            .clone();
        ensure(
            requests.len() == 2,
            "diagnostic command should hit the local runtime once per requested repeat",
        )?;
        ensure(
            requests.iter().all(|request| {
                request["model"] == json!("gemma4-e4b-local:latest")
                    && request["options"]["num_predict"] == json!(24)
                    && request["stream"] == json!(true)
            }),
            "diagnostic command should send the resolved runtime model and requested output cap",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gemma_diagnose_records_runtime_endpoint_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.local_gemma_base_url = "http://127.0.0.1:9".to_string();
        save_config(config_path.as_path(), &config)?;
        ensure_identity(config.identity_path.as_path())?;

        let report = run_gemma_diagnostic_command(
            config_path.as_path(),
            &GemmaBenchmarkSelector::Model("gemma-4-e4b".to_string()),
            &GemmaDiagnosticRequest {
                diagnostic_id: DEFAULT_GEMMA_DIAGNOSTIC_ID.to_string(),
                prompt: DEFAULT_GEMMA_BENCH_PROMPT.to_string(),
                max_output_tokens: 16,
                repeats: 1,
                download_missing: false,
            },
        )
        .await?;

        ensure(
            report.results.len() == 1,
            "diagnostic failure report should still keep the selected model row",
        )?;
        let result = &report.results[0];
        ensure(
            result.status == "failed" && result.receipt.is_none(),
            "diagnostic command should fail closed when the local runtime endpoint is unreachable",
        )?;
        let reason = result
            .reason
            .as_deref()
            .ok_or_else(|| std::io::Error::other("missing diagnostic failure reason"))?;
        ensure(
            reason.contains("http://127.0.0.1:9/api/tags")
                && reason.contains("update local_gemma_base_url"),
            "diagnostic failure should name the exact local runtime endpoint and remediation path",
        )?;
        let loaded = load_latest_gemma_diagnostic_report(config_path.as_path())?
            .ok_or_else(|| std::io::Error::other("missing latest diagnostic report"))?;
        ensure(
            loaded.results[0].status == "failed",
            "diagnostic failure should still persist the retained report for later export",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gemma_diagnose_skips_models_not_loaded_in_runtime()
    -> Result<(), Box<dyn std::error::Error>> {
        let recorded_requests = Arc::new(Mutex::new(0usize));
        let recorded_requests_for_server = Arc::clone(&recorded_requests);
        let base_url = start_mock_http_server(move |method, path, _body| {
            match (method.as_str(), path.as_str()) {
                ("GET", "/api/tags") => (
                    200,
                    "application/json",
                    json!({
                        "models": [
                            {"name": "gemma4-e2b-local:latest"}
                        ]
                    })
                    .to_string(),
                ),
                ("POST", "/api/chat") => {
                    *recorded_requests_for_server
                        .lock()
                        .expect("diagnostic request count") += 1;
                    (200, "application/x-ndjson", "{\"done\":true}\n".to_string())
                }
                _ => (500, "text/plain", "unexpected request".to_string()),
            }
        })
        .await?;

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.local_gemma_base_url = base_url;
        save_config(config_path.as_path(), &config)?;
        ensure_identity(config.identity_path.as_path())?;

        let report = run_gemma_diagnostic_command(
            config_path.as_path(),
            &GemmaBenchmarkSelector::Model("gemma-4-e4b".to_string()),
            &GemmaDiagnosticRequest {
                diagnostic_id: DEFAULT_GEMMA_DIAGNOSTIC_ID.to_string(),
                prompt: DEFAULT_GEMMA_BENCH_PROMPT.to_string(),
                max_output_tokens: 16,
                repeats: 1,
                download_missing: false,
            },
        )
        .await?;

        let result = report
            .results
            .first()
            .ok_or_else(|| std::io::Error::other("missing diagnostic result"))?;
        ensure(
            result.status == "skipped" && result.receipt.is_none(),
            "diagnostic command should skip models that are not loaded in the current runtime",
        )?;
        ensure(
            result.ready_in_runtime == false
                && result
                    .reason
                    .as_deref()
                    .is_some_and(|reason| reason.contains("not loaded in the local runtime")),
            "diagnostic skip should explain that downloaded weights alone are not enough",
        )?;
        ensure(
            *recorded_requests.lock().expect("diagnostic request count") == 0,
            "diagnostic skip should not hit /api/chat when the requested model is not loaded",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn local_gemma_detection_ignores_non_gemma_models()
    -> Result<(), Box<dyn std::error::Error>> {
        let base_url =
            start_mock_http_server(
                |method, path, _body| match (method.as_str(), path.as_str()) {
                    ("GET", "/api/tags") => (
                        200,
                        "application/json",
                        json!({
                            "models": [
                                {"name": "qwen35-27b-local:latest"},
                                {"name": "nomic-embed-text:latest"}
                            ]
                        })
                        .to_string(),
                    ),
                    _ => (500, "text/plain", "unexpected request".to_string()),
                },
            )
            .await?;

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.local_gemma_base_url = base_url;
        save_config(config_path.as_path(), &config)?;
        ensure_identity(config.identity_path.as_path())?;

        let status = load_status_or_detect(config_path.as_path()).await?;
        ensure(
            status.snapshot.as_ref().is_some_and(|snapshot| {
                !snapshot.availability.local_gemma.ready
                    && snapshot
                        .availability
                        .local_gemma
                        .available_models
                        .is_empty()
            }),
            "non-Gemma models should not mark local Gemma supply ready",
        )?;
        ensure(
            render_human_status(&status).contains("LOCAL_GEMMA_UNAVAILABLE"),
            "status should surface the local Gemma blocker when only non-Gemma models are present",
        )?;
        ensure(
            status.snapshot.as_ref().is_some_and(|snapshot| {
                snapshot
                    .availability
                    .local_gemma
                    .last_error
                    .as_deref()
                    .is_some_and(|error| {
                        error.contains("Downloaded GGUF files alone do not make supply eligible")
                    })
            }),
            "status should explain that cached GGUFs are not enough without a loaded Gemma runtime",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn local_gemma_detection_reports_actionable_runtime_endpoint_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.local_gemma_base_url = "http://127.0.0.1:9".to_string();
        save_config(config_path.as_path(), &config)?;
        ensure_identity(config.identity_path.as_path())?;

        let status = load_status_or_detect(config_path.as_path()).await?;
        let last_error = status
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.availability.local_gemma.last_error.as_deref())
            .ok_or_else(|| std::io::Error::other("missing local Gemma error"))?;

        ensure(
            last_error.contains("local Gemma runtime not reachable at http://127.0.0.1:9/api/tags"),
            "status should name the exact local Gemma endpoint that failed",
        )?;
        ensure(
            last_error.contains("update local_gemma_base_url"),
            "status should explain the config remediation path for a failed local Gemma endpoint",
        )?;
        ensure(
            render_human_status(&status).contains("local Gemma runtime not reachable"),
            "human status should surface the actionable local Gemma runtime error",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn doctor_report_uses_canonical_product_ids_and_hides_legacy_apple_surface()
    -> Result<(), Box<dyn std::error::Error>> {
        let base_url =
            start_mock_http_server(
                |method, path, _body| match (method.as_str(), path.as_str()) {
                    ("GET", "/api/tags") => (
                        200,
                        "application/json",
                        json!({
                            "models": [
                                {"name": "gemma4-e4b-local:latest"}
                            ]
                        })
                        .to_string(),
                    ),
                    _ => (500, "text/plain", "unexpected request".to_string()),
                },
            )
            .await?;

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.local_gemma_base_url = base_url;
        save_config(config_path.as_path(), &config)?;
        ensure_identity(config.identity_path.as_path())?;

        let output = run_cli(Cli {
            command: Command::Doctor,
            config_path: config_path.clone(),
        })
        .await?
        .ok_or_else(|| std::io::Error::other("missing doctor output"))?;
        let json: Value = serde_json::from_str(output.as_str())?;
        let products = json
            .get("products")
            .and_then(Value::as_array)
            .ok_or_else(|| std::io::Error::other("missing doctor products"))?;

        ensure(
            !output.contains("\"product\": \"gpt_oss_inference\""),
            "doctor should not leak legacy gpt_oss product enum names",
        )?;
        ensure(
            products
                .iter()
                .all(|product| product.get("product").is_none()),
            "doctor should expose canonical product_id fields rather than enum variant names",
        )?;
        ensure(
            products.iter().any(|product| {
                product.get("product_id").and_then(Value::as_str)
                    == Some("psionic.local.inference.gemma.single_node")
            }),
            "doctor should expose the canonical Gemma product id",
        )?;
        ensure(
            products.iter().all(|product| {
                product.get("backend").and_then(Value::as_str) != Some("apple_foundation_models")
            }),
            "doctor should hide the legacy Apple FM-only surface from standalone Pylon onboarding",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn backend_and_product_reports_preserve_launch_family_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        seed_observability_snapshot(config_path.as_path())?;

        let backend_report = load_backend_report(config_path.as_path()).await?;
        let product_report = load_product_report(config_path.as_path()).await?;

        let local_gemma = backend_report
            .backends
            .iter()
            .find(|backend| backend.backend_id == "local_gemma")
            .ok_or_else(|| std::io::Error::other("missing local_gemma backend entry"))?;
        ensure(
            local_gemma.launch_product_ids
                == vec!["psionic.local.inference.gemma.single_node".to_string()],
            "local Gemma backend should expose the canonical Gemma inference product",
        )?;
        ensure(
            product_report
                .products
                .iter()
                .all(|product| product.backend != "apple_foundation_models"),
            "product report should hide the legacy Apple FM backend from Pylon",
        )?;
        ensure(
            product_report.products.iter().any(|product| {
                product.product_id == "psionic.local.inference.gemma.single_node"
                    && product.capability_summary.contains("backend=local_gemma")
            }),
            "product report should preserve the local Gemma capability summary",
        )?;
        let sandbox = backend_report
            .backends
            .iter()
            .find(|backend| backend.backend_id == "sandbox")
            .ok_or_else(|| std::io::Error::other("missing sandbox backend entry"))?;
        ensure(
            sandbox.supported_execution_classes == vec!["sandbox.python.exec".to_string()],
            "sandbox backend should expose declared execution classes",
        )?;
        ensure(
            sandbox.profile_ids == vec!["python-batch".to_string()],
            "sandbox backend should expose declared profile ids",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn inventory_report_prefers_fresh_detected_snapshot_over_stale_store_rows()
    -> Result<(), Box<dyn std::error::Error>> {
        let base_url =
            start_mock_http_server(
                |method, path, _body| match (method.as_str(), path.as_str()) {
                    ("GET", "/api/tags") => (
                        200,
                        "application/json",
                        json!({
                            "models": [
                                {"name": "gemma4:e4b"}
                            ]
                        })
                        .to_string(),
                    ),
                    _ => (500, "text/plain", "unexpected request".to_string()),
                },
            )
            .await?;

        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let mut config = load_or_create_config(config_path.as_path())?;
        let identity = ensure_identity(config.identity_path.as_path())?;
        config.admin_listen_addr = "127.0.0.1:0".to_string();
        config.local_gemma_base_url = base_url;
        save_config(config_path.as_path(), &config)?;

        let stale_snapshot = build_snapshot_from_availability(
            &config,
            Some(&identity),
            ProviderDesiredMode::Online,
            None,
            ProviderAvailability::default(),
            None,
        );
        let admin_config = provider_admin_config(&config)?;
        let mut store = ProviderPersistenceStore::open(&admin_config)?;
        store.set_listen_addr(config.admin_listen_addr.as_str())?;
        store.set_desired_mode(ProviderDesiredMode::Online)?;
        store.persist_snapshot(&stale_snapshot)?;

        let status = load_status_or_detect(config_path.as_path()).await?;
        ensure(
            status.snapshot.as_ref().is_some_and(|snapshot| {
                snapshot.inventory_rows.iter().any(|row| {
                    row.target.product_id() == "psionic.local.inference.gemma.single_node"
                        && row.eligible
                })
            }),
            "fresh status should detect ready local Gemma supply even when the persisted store is stale",
        )?;

        let inventory_report = load_inventory_report(config_path.as_path(), Some(8)).await?;
        ensure(
            inventory_report.rows.iter().any(|row| {
                row.target.product_id() == "psionic.local.inference.gemma.single_node"
                    && row.eligible
            }),
            "inventory should use the fresh detected snapshot instead of stale persisted rows when the local admin service is down",
        )?;
        ensure(
            !inventory_report.rows.iter().any(|row| {
                row.target.product_id() == "psionic.local.inference.gemma.single_node"
                    && !row.eligible
            }),
            "inventory should not regress to stale ineligible rows after the runtime becomes ready",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn inventory_jobs_earnings_and_receipts_reports_round_trip_store_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        seed_observability_snapshot(config_path.as_path())?;

        let inventory_report = load_inventory_report(config_path.as_path(), Some(8)).await?;
        let jobs_report = load_jobs_report(config_path.as_path(), Some(4)).await?;
        let earnings_report = load_earnings_report(config_path.as_path()).await?;
        let receipts_report = load_receipts_report(config_path.as_path(), Some(2)).await?;

        ensure(
            inventory_report.rows.iter().any(|row| {
                row.target.product_id() == "psionic.local.inference.gemma.single_node"
                    && row.eligible
            }),
            "inventory report should show eligible local Gemma supply",
        )?;
        ensure(
            inventory_report.rows.iter().any(|row| {
                row.target.product_id()
                    == "psionic.remote_sandbox.sandbox_execution.python_exec.sandbox_isolated"
                    && row.eligible
            }),
            "inventory report should show eligible sandbox supply when profiles are declared",
        )?;
        ensure(
            jobs_report.jobs.len() == 2
                && jobs_report.jobs.iter().any(|job| {
                    job.product_id.as_deref() == Some("psionic.local.inference.gemma.single_node")
                }),
            "jobs report should surface persisted recent jobs",
        )?;
        let sandbox_job = jobs_report
            .jobs
            .iter()
            .find(|job| job.product_id.as_deref() == Some("sandbox.python.exec"))
            .ok_or_else(|| std::io::Error::other("missing sandbox job row"))?;
        ensure(
            sandbox_job.sandbox_execution_class.as_deref() == Some("sandbox.python.exec")
                && sandbox_job.failure_reason.as_deref()
                    == Some("sandbox execution exceeded timeout"),
            "jobs report should surface sandbox failure classification and reason",
        )?;
        ensure(
            earnings_report
                .earnings
                .as_ref()
                .is_some_and(|earnings| earnings.lifetime_sats == 420),
            "earnings report should surface persisted earnings",
        )?;
        ensure(
            receipts_report.receipts.len() == 2
                && receipts_report
                    .receipts
                    .iter()
                    .any(|receipt| receipt.receipt_id == "receipt-1"),
            "receipts report should surface persisted receipts",
        )?;
        let sandbox_receipt = receipts_report
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_id == "receipt-2")
            .ok_or_else(|| std::io::Error::other("missing sandbox receipt row"))?;
        ensure(
            sandbox_receipt.sandbox_profile_id.as_deref() == Some("python-batch")
                && sandbox_receipt.sandbox_termination_reason.as_deref() == Some("timeout"),
            "receipts report should surface sandbox receipt integrity fields",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sandbox_reports_surface_profiles_status_and_failures()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        seed_observability_snapshot(config_path.as_path())?;

        let status = load_status_or_detect(config_path.as_path()).await?;
        let status_render = render_human_status(&status);
        ensure(
            status_render.contains("sandbox_execution_classes: sandbox.python.exec"),
            "status should surface supported sandbox execution classes",
        )?;
        ensure(
            status_render.contains("sandbox_profiles: python-batch"),
            "status should surface declared sandbox profile ids",
        )?;

        let sandbox_report = load_sandbox_report(config_path.as_path(), Some(4)).await?;
        ensure(
            sandbox_report.supported_execution_classes == vec!["sandbox.python.exec".to_string()],
            "sandbox report should expose declared execution classes",
        )?;
        ensure(
            sandbox_report.profiles.first().is_some_and(|profile| {
                profile.profile_id == "python-batch" && profile.runtime_ready
            }),
            "sandbox report should expose runtime-ready declared profiles",
        )?;

        let rendered = render_sandbox_report(&sandbox_report);
        ensure(
            rendered.contains("runtime_kind: python"),
            "sandbox report should render runtime kinds",
        )?;
        ensure(
            rendered.contains("profile_digest: sha256:python-profile"),
            "sandbox report should render profile digests for verification",
        )?;
        ensure(
            rendered.contains("execution_class: sandbox.python.exec"),
            "sandbox report should render execution classes for policy matching",
        )
    }
}
